use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use anyhow::{anyhow, Result};
use chrono::Utc;
use tracing::{info, warn, error};

use crate::db::{DbManager, Task, TaskStatus, TaskType, HistoryItem};
use crate::yt_dlp::{BinManager, CompositeProgressParser, ProgressParser};
use crate::sanitizer::{resolve_path_collision, is_path_safe};
use crate::presets::get_preset_by_id;
use crate::verification::verify_download;

/// An abstract trait for dispatching real-time updates to the UI layer.
/// This decouples fetchr_core from Tauri's explicit event APIs.
pub trait EventDispatcher: Send + Sync {
    fn dispatch_task_update(&self, task: Task);
    fn dispatch_queue_complete(&self);
}

pub struct QueueOrchestrator {
    db: Arc<DbManager>,
    bin_manager: Arc<BinManager>,
    event_dispatcher: Arc<dyn EventDispatcher>,
    active_processes: Arc<Mutex<HashMap<String, tokio::process::Child>>>,
    max_concurrency: Arc<Mutex<usize>>,
}

impl QueueOrchestrator {
    /// Instantiates a new QueueOrchestrator.
    pub fn new(
        db: Arc<DbManager>,
        bin_manager: Arc<BinManager>,
        event_dispatcher: Arc<dyn EventDispatcher>,
    ) -> Self {
        let max_concurrency = Arc::new(Mutex::new(2)); // Default to 2 concurrent downloads
        Self {
            db,
            bin_manager,
            event_dispatcher,
            active_processes: Arc::new(Mutex::new(HashMap::new())),
            max_concurrency,
        }
    }

    /// Update the maximum concurrency on the fly.
    pub async fn set_max_concurrency(&self, limit: usize) {
        let mut max = self.max_concurrency.lock().await;
        *max = limit;
        info!("Max download concurrency set to {}", limit);
    }

    /// Retrieve active concurrency limit
    pub async fn get_max_concurrency(&self) -> usize {
        *self.max_concurrency.lock().await
    }

    /// Add a new task to the queue and database, then trigger scheduling.
    pub async fn add_task(&self, url: String, task_type: TaskType) -> Result<Task> {
        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            task_type,
            url,
            status: TaskStatus::Pending,
            progress: 0.0,
            speed: None,
            eta: None,
            file_path: None,
            error_msg: None,
            retry_count: 0,
            created_at: Utc::now(),
        };

        self.db.save_task(&task)?;
        self.event_dispatcher.dispatch_task_update(task.clone());
        info!("Added new task {} to queue", task.id);
        Ok(task)
    }

    /// Cancel a running or pending task.
    pub async fn cancel_task(&self, id: &str) -> Result<()> {
        info!("Attempting to cancel task {}", id);
        
        // 1. Terminate running process if active
        let mut active = self.active_processes.lock().await;
        if let Some(mut child) = active.remove(id) {
            info!("Killing running process for task {}", id);
            let _ = child.kill().await;
        }

        // 2. Load task from DB and update status
        let mut tasks = self.db.load_all_tasks()?;
        if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
            task.status = TaskStatus::Paused;
            task.speed = None;
            task.eta = None;
            self.db.save_task(task)?;
            self.event_dispatcher.dispatch_task_update(task.clone());
            info!("Task {} set to Paused/Cancelled", id);
        }

        Ok(())
    }

    /// Resume an interrupted, failed, or paused task.
    pub async fn resume_task(&self, id: &str) -> Result<()> {
        info!("Resuming task {}", id);
        let mut tasks = self.db.load_all_tasks()?;
        if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
            task.status = TaskStatus::Pending;
            task.progress = 0.0;
            task.speed = None;
            task.eta = None;
            task.error_msg = None;
            self.db.save_task(task)?;
            self.event_dispatcher.dispatch_task_update(task.clone());
        }
        Ok(())
    }

    /// Start the scheduler loop.
    pub fn start(self: Arc<Self>) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
                loop {
                    interval.tick().await;
                    if let Err(e) = self.schedule_next_tasks().await {
                        error!("Error in schedule loop: {:?}", e);
                    }
                }
            });
        } else {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async move {
                    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
                    loop {
                        interval.tick().await;
                        if let Err(e) = self.schedule_next_tasks().await {
                            error!("Error in schedule loop: {:?}", e);
                        }
                    }
                });
            });
        }
        info!("Queue Orchestrator background scheduling loop started.");
    }

    /// Core scheduler logic: counts active tasks and spawns pending ones.
    async fn schedule_next_tasks(&self) -> Result<()> {
        let tasks = self.db.load_all_tasks()?;
        
        // Count currently running processes
        let active_count = {
            let active = self.active_processes.lock().await;
            active.len()
        };

        let max_concurrency = self.get_max_concurrency().await;
        if active_count >= max_concurrency {
            return Ok(()); // Slots full
        }

        let open_slots = max_concurrency - active_count;
        let pending_tasks: Vec<Task> = tasks
            .into_iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .take(open_slots)
            .collect();

        for task in pending_tasks {
            let orchestrator = self.clone_orchestrator();
            tokio::spawn(async move {
                if let Err(e) = orchestrator.execute_task(task.clone()).await {
                    error!("Error executing task {}: {:?}", task.id, e);
                }
            });
        }

        Ok(())
    }

    fn clone_orchestrator(&self) -> Arc<Self> {
        // Creates a reference-counted clone of structural pointers
        Arc::new(Self {
            db: self.db.clone(),
            bin_manager: self.bin_manager.clone(),
            event_dispatcher: self.event_dispatcher.clone(),
            active_processes: self.active_processes.clone(),
            max_concurrency: self.max_concurrency.clone(),
        })
    }

    /// Executes a single task wrapping yt-dlp execution and progress parsing.
    async fn execute_task(&self, mut task: Task) -> Result<()> {
        info!("Executing task {} | Type: {:?}", task.id, task.task_type);

        // 1. Mark task as Downloading
        task.status = TaskStatus::Downloading;
        self.db.save_task(&task)?;
        self.event_dispatcher.dispatch_task_update(task.clone());

        // 2. Resolve target download directory
        let downloads_setting = self.db.get_setting("download_directory").unwrap_or(None);
        let downloads_dir = match downloads_setting {
            Some(d) => PathBuf::from(d),
            None => {
                #[cfg(target_os = "android")]
                {
                    self.bin_manager.bin_dir.parent()
                        .map(|p| p.join("downloads"))
                        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/downloads"))
                }
                #[cfg(not(target_os = "android"))]
                {
                    dirs::download_dir().unwrap_or_else(|| PathBuf::from("./downloads"))
                }
            }
        };
        std::fs::create_dir_all(&downloads_dir).ok();

        // 3. Resolve yt-dlp binary
        let custom_yt_dlp = self.db.get_setting("custom_yt_dlp_path").unwrap_or(None);
        let yt_dlp_path = match self.bin_manager.resolve_yt_dlp_binary(custom_yt_dlp.as_deref()) {
            Ok(p) => p,
            Err(e) => {
                info!("yt-dlp missing or could not be resolved: {}. Falling back to direct HTTP download.", e);
                if let Err(err) = self.execute_direct_download(task.clone(), downloads_dir).await {
                    self.mark_task_failed(&mut task, &format!("Direct download error: {}", err)).await;
                }
                return Ok(());
            }
        };

        // 4. Extract URL Metadata first (if not cached or needed)
        // Set cookies browser if saved
        let cookies_browser = self.db.get_setting("cookies_browser").unwrap_or(None);
        let cookies_browser_str = cookies_browser.as_deref().unwrap_or("");

        // Build command arguments
        let mut cmd = Command::new(&yt_dlp_path);
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.arg("--newline")
           .arg("--progress-template")
           .arg("download-json:%(progress)j");

        // Inject resolved ffmpeg location to allow yt-dlp to merge/transcode in launchd GUI execution
        if let Ok(ffmpeg_path) = self.bin_manager.resolve_ffmpeg_binary() {
            if let Some(parent) = ffmpeg_path.parent() {
                cmd.arg("--ffmpeg-location").arg(parent);
                info!("Passing resolved ffmpeg directory to yt-dlp: {:?}", parent);
            }
        } else {
            warn!("Could not resolve ffmpeg binary path for yt-dlp. Fallback may occur.");
        }

        // Inject cookies
        if !cookies_browser_str.is_empty() {
            cmd.arg("--cookies-from-browser").arg(cookies_browser_str);
        }

        // Inject download archive if checked in settings
        let skip_duplicates = self.db.get_setting("skip_previously_downloaded").unwrap_or(None);
        if skip_duplicates.as_deref() == Some("true") {
            let archive_path = downloads_dir.join(".videosaver_download_archive.txt");
            cmd.arg("--download-archive").arg(archive_path);
        }

        // Custom yt-dlp flags from advanced mode settings
        let custom_flags = self.db.get_setting("custom_yt_dlp_flags").unwrap_or(None);
        if let Some(flags) = custom_flags {
            if !flags.trim().is_empty() {
                for flag in flags.split_whitespace() {
                    cmd.arg(flag);
                }
            }
        }

        // Temporary filename layout to prevent path collisions and handle Windows cleanly
        let metadata_engine = crate::yt_dlp::YtDlpEngine::new(
            (*self.bin_manager).clone(), 
            custom_yt_dlp
        );

        let metadata = match metadata_engine.extract_metadata(&task.url, cookies_browser.as_deref()).await {
            Ok(m) => m,
            Err(e) => {
                self.mark_task_failed(&mut task, &format!("Extraction failed: {}", e)).await;
                return Ok(());
            }
        };

        // Determine final output path and sanitization
        // Determine final output path and sanitization
        let extension = match &task.task_type {
            TaskType::DownloadAudio { audio_format } => audio_format.clone(),
            TaskType::DownloadVideo { format_preset } => {
                if format_preset.starts_with("dynamic:") {
                    let parts: Vec<&str> = format_preset.split(':').collect();
                    if parts.len() == 3 {
                        parts[2].to_string()
                    } else {
                        "mp4".to_string()
                    }
                } else if let Some(preset) = get_preset_by_id(format_preset) {
                    preset.merge_format.clone().unwrap_or_else(|| "mp4".to_string())
                } else {
                    "mp4".to_string()
                }
            }
            _ => "mp4".to_string(), // default merge container
        };

        let target_path = resolve_path_collision(&downloads_dir, &metadata.title, &extension);
        
        // Security check
        if !is_path_safe(&downloads_dir, &target_path) {
            self.mark_task_failed(&mut task, "Filesystem Security Error: Path traversal attempt blocked.").await;
            return Ok(());
        }

        let target_path_str = target_path.to_string_lossy().to_string();
        task.file_path = Some(target_path_str.clone());

        // Configure download formats
        match &task.task_type {
            TaskType::DownloadVideo { format_preset } => {
                if format_preset.starts_with("dynamic:") {
                    let parts: Vec<&str> = format_preset.split(':').collect();
                    if parts.len() == 3 {
                        let quality = parts[1]; // e.g. "1080p", "720p", "2160p", "best"
                        let container = parts[2]; // e.g. "mp4", "mkv"
                        
                        let height_limit = if quality.ends_with('p') {
                            let num = &quality[..quality.len() - 1];
                            if num.chars().all(|c| c.is_ascii_digit()) {
                                Some(num)
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        
                        if let Some(h) = height_limit {
                            let format_filter = format!("bestvideo[height<={h}]+bestaudio/best[height<={h}]");
                            cmd.arg("-f").arg(&format_filter);
                            cmd.arg("-S").arg("res");
                            info!("Applying dynamic quality limit height: {} with resolution sorting", h);
                        } else {
                            // "Best Available Quality": explicitly select best separate streams
                            // and sort by resolution to guarantee highest quality on all platforms.
                            // Using bestvideo* (with *) allows video-only + audio merge.
                            cmd.arg("-f").arg("bestvideo*+bestaudio/best");
                            cmd.arg("-S").arg("res");
                            info!("Applying Best Available Quality: bestvideo*+bestaudio/best with -S res sorting");
                        }
                        
                        cmd.arg("--merge-output-format").arg(container);
                        if container == "mp4" {
                            let encoder = self.bin_manager.get_best_h264_encoder();
                            let post_args = format!("ffmpeg:-c:v {} -c:a aac -pix_fmt yuv420p", encoder);
                            cmd.arg("--postprocessor-args").arg(&post_args);
                            info!("Enabling universal {}/AAC/YUV420P transcoding for strict compatibility.", encoder);
                        }
                    } else {
                        cmd.arg("-f").arg("bestvideo+bestaudio/best");
                    }
                } else if let Some(preset) = get_preset_by_id(format_preset) {
                    cmd.arg("-f").arg(&preset.format_filter);
                    if let Some(merge) = &preset.merge_format {
                        cmd.arg("--merge-output-format").arg(merge);
                    }
                } else {
                    cmd.arg("-f").arg("bestvideo+bestaudio/best");
                }
            }
            TaskType::DownloadAudio { audio_format } => {
                cmd.arg("-x").arg("--audio-format").arg(audio_format).arg("--audio-quality").arg("0");
            }
            _ => {
                cmd.arg("-f").arg("best");
            }
        }

        // Output destination template
        cmd.arg("-o").arg(format!("{}.%(ext)s", target_path.with_extension("").to_string_lossy()));
        cmd.arg(&task.url);

        // Configure process redirection
        cmd.stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                self.mark_task_failed(&mut task, &format!("Process spawn error: {}", e)).await;
                return Ok(());
            }
        };

        // Register child process globally to support cancellation
        {
            let mut active = self.active_processes.lock().await;
            active.insert(task.id.clone(), child);
        }

        // Re-read stdout buffer to capture line updates
        let active = self.active_processes.clone();
        let mut child_process = {
            let mut act = active.lock().await;
            act.remove(&task.id).ok_or_else(|| anyhow!("Child process missing"))?
        };

        let stdout = child_process.stdout.take().ok_or_else(|| anyhow!("Stdout capture missing"))?;
        let mut reader = BufReader::new(stdout).lines();
        let parser = CompositeProgressParser::new();

        let db_clone = self.db.clone();
        let dispatcher = self.event_dispatcher.clone();
        let mut task_clone = task.clone();

        let start_time = Utc::now();

        // Process line-by-line progress
        while let Ok(Some(line)) = reader.next_line().await {
            if let Some(progress) = parser.parse(&line) {
                task_clone.progress = progress.percentage;
                task_clone.speed = progress.speed;
                task_clone.eta = progress.eta;

                // Non-blocking quick update to DB and Event Bridge
                let _ = db_clone.save_task(&task_clone);
                dispatcher.dispatch_task_update(task_clone.clone());
            }
        }

        // Wait for child to exit
        let status = child_process.wait().await?;
        
        if status.success() {
            // 5. Verification Layer
            let ffprobe_path = self.bin_manager.resolve_ffprobe_binary().ok();
            
            // Check actual completed file path. yt-dlp might have written slightly different extension
            // e.g. if merge format was mkv but target was mp4. Let's find files starting with the base
            let completed_path = find_actual_downloaded_file(&target_path);

            let verification = verify_download(
                &completed_path,
                ffprobe_path.as_deref(),
                true
            );

            if verification.is_valid {
                task_clone.status = TaskStatus::Completed;
                task_clone.progress = 100.0;
                task_clone.speed = None;
                task_clone.eta = None;
                task_clone.file_path = Some(completed_path.to_string_lossy().to_string());
                
                let _ = db_clone.save_task(&task_clone);
                dispatcher.dispatch_task_update(task_clone.clone());

                // Add to SQLite History logs
                let duration_secs = Utc::now().signed_duration_since(start_time).num_seconds();
                
                let history_item = HistoryItem {
                    id: task_clone.id.clone(),
                    title: metadata.title.clone(),
                    url: task_clone.url.clone(),
                    file_path: completed_path.to_string_lossy().to_string(),
                    file_size: verification.file_size_bytes,
                    duration: verification.duration_seconds.unwrap_or(metadata.duration.round() as i64),
                    thumbnail_path: metadata.thumbnail_url.clone(),
                    resolution: None, // Can query streams if needed
                    source_site: Some(metadata.extractor),
                    download_duration_secs: duration_secs.max(1),
                    completed_at: Utc::now(),
                };

                let _ = db_clone.add_to_history(&history_item);
                info!("Task {} finished successfully & verified.", task_clone.id);
            } else {
                let err_msg = verification.error_msg.unwrap_or_else(|| "Validation failure".to_string());
                self.mark_task_failed(&mut task_clone, &format!("Integrity check failed: {}", err_msg)).await;
            }
        } else {
            // Process failed
            let mut stderr_content = String::new();
            if let Some(stderr) = child_process.stderr.take() {
                let mut err_reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = err_reader.next_line().await {
                    stderr_content.push_str(&line);
                    stderr_content.push('\n');
                }
            }

            let clean_err = if stderr_content.is_empty() {
                "yt-dlp exited with non-zero status code.".to_string()
            } else {
                // Return last 200 chars to prevent massive error block flooding the UI
                let trimmed = stderr_content.trim();
                if trimmed.len() > 300 {
                    format!("...{}", &trimmed[trimmed.len() - 300..])
                } else {
                    trimmed.to_string()
                }
            };

            self.mark_task_failed(&mut task_clone, &clean_err).await;
        }

        // Cleanup
        {
            let mut active = self.active_processes.lock().await;
            active.remove(&task_clone.id);
        }

        // Trigger double-check if all tasks completed to fire general complete event
        let final_tasks = self.db.load_all_tasks()?;
        let active_tasks_count = final_tasks.iter().filter(|t| t.status == TaskStatus::Downloading).count();
        if active_tasks_count == 0 {
            dispatcher.dispatch_queue_complete();
        }

        Ok(())
    }

    async fn mark_task_failed(&self, task: &mut Task, error_msg: &str) {
        warn!("Task {} failed: {}", task.id, error_msg);
        task.status = TaskStatus::Failed;
        task.progress = 0.0;
        task.speed = None;
        task.eta = None;
        task.error_msg = Some(error_msg.to_string());

        let _ = self.db.save_task(task);
        self.event_dispatcher.dispatch_task_update(task.clone());
    }

    async fn execute_direct_download(&self, mut task: Task, downloads_dir: PathBuf) -> Result<()> {
        info!("Executing direct HTTP download for task {} | URL: {}", task.id, task.url);
        
        // Mark as Downloading
        task.status = TaskStatus::Downloading;
        task.progress = 0.0;
        self.db.save_task(&task)?;
        self.event_dispatcher.dispatch_task_update(task.clone());

        // Extract filename from URL or default
        let filename = task.url.split('/').last().unwrap_or("video.mp4");
        let filename_clean = if filename.contains('?') {
            filename.split('?').next().unwrap_or("video.mp4").to_string()
        } else {
            filename.to_string()
        };
        
        let file_ext = if filename_clean.contains('.') {
            filename_clean.split('.').last().unwrap_or("mp4").to_string()
        } else {
            "mp4".to_string()
        };

        // Output destination path
        let dest_path = downloads_dir.join(format!("{}_{}", task.id, filename_clean));
        task.file_path = Some(dest_path.to_string_lossy().to_string());
        self.db.save_task(&task)?;

        // Send HTTP GET request
        let response = match reqwest::get(&task.url).await {
            Ok(r) => r,
            Err(e) => {
                self.mark_task_failed(&mut task, &format!("HTTP request failed: {}", e)).await;
                return Ok(());
            }
        };

        if !response.status().is_success() {
            self.mark_task_failed(&mut task, &format!("Server returned error status: {}", response.status())).await;
            return Ok(());
        }

        // Get content length for progress reporting
        let total_size = response.content_length();
        let mut file = match std::fs::File::create(&dest_path) {
            Ok(f) => f,
            Err(e) => {
                self.mark_task_failed(&mut task, &format!("File creation error: {}", e)).await;
                return Ok(());
            }
        };

        let mut response = response; // make mutable to extract chunks
        let mut downloaded_bytes: u64 = 0;
        let start_time = std::time::Instant::now();
        let mut last_update = std::time::Instant::now();

        use std::io::Write;
        while let Ok(Some(chunk)) = response.chunk().await {
            if let Err(e) = file.write_all(&chunk) {
                self.mark_task_failed(&mut task, &format!("Disk write error: {}", e)).await;
                return Ok(());
            }

            downloaded_bytes += chunk.len() as u64;

            // Rate-limit progress updates to dispatchers (e.g. every 300ms)
            if last_update.elapsed().as_millis() > 300 {
                last_update = std::time::Instant::now();
                if let Some(total) = total_size {
                    let progress = (downloaded_bytes as f64 / total as f64) * 100.0;
                    task.progress = progress;
                    
                    let elapsed = start_time.elapsed().as_secs_f64();
                    if elapsed > 0.0 {
                        let speed_mb = (downloaded_bytes as f64 / (1024.0 * 1024.0)) / elapsed;
                        task.speed = Some(format!("{:.2} MB/s", speed_mb));
                    }
                    
                    self.db.save_task(&task)?;
                    self.event_dispatcher.dispatch_task_update(task.clone());
                }
            }
        }

        // Finalize task as Completed
        task.status = TaskStatus::Completed;
        task.progress = 100.0;
        task.speed = None;
        task.eta = None;
        self.db.save_task(&task)?;
        let duration_secs = start_time.elapsed().as_secs() as i64;
        let history_item = HistoryItem {
            id: task.id.clone(),
            title: filename_clean.clone(),
            url: task.url.clone(),
            file_path: dest_path.to_string_lossy().to_string(),
            file_size: downloaded_bytes as i64,
            duration: 0,
            thumbnail_path: None,
            resolution: Some(file_ext),
            source_site: Some("Direct HTTP".to_string()),
            download_duration_secs: duration_secs.max(1),
            completed_at: Utc::now(),
        };
        let _ = self.db.add_to_history(&history_item);
        self.event_dispatcher.dispatch_task_update(task.clone());
        info!("Direct HTTP download completed for task {}", task.id);
        
        Ok(())
    }
}

/// Helper function. yt-dlp might output slightly different merge container than estimated.
/// We look for files matching base name to see what actual file was created.
fn find_actual_downloaded_file(estimated_path: &Path) -> PathBuf {
    if estimated_path.exists() {
        return estimated_path.to_path_buf();
    }

    let parent = estimated_path.parent().unwrap_or(estimated_path);
    let stem = estimated_path.file_stem().unwrap_or_default().to_string_lossy().to_string();

    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let entry_stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                if entry_stem == stem {
                    return path;
                }
            }
        }
    }

    estimated_path.to_path_buf()
}
