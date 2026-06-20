use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use serde_json::json;

use fetchr_core::db::{DbManager, Task, TaskType, HistoryItem};
use fetchr_core::yt_dlp::{BinManager, YtDlpEngine, UpdateChannel};
use fetchr_core::queue::QueueOrchestrator;
use fetchr_core::capabilities::get_capabilities;
use fetchr_core::presets::get_default_presets;

// Tauri event bridge adapter mapping Core events to IPC websockets
struct TauriEventDispatcher {
    app_handle: AppHandle,
}

impl fetchr_core::queue::EventDispatcher for TauriEventDispatcher {
    fn dispatch_task_update(&self, task: Task) {
        let _ = self.app_handle.emit("task-update", task);
    }

    fn dispatch_queue_complete(&self) {
        let _ = self.app_handle.emit("queue-complete", ());
    }
}

struct AppState {
    db: Arc<DbManager>,
    bin_manager: Arc<BinManager>,
    queue: Arc<QueueOrchestrator>,
    // Hold the guard to keep structured file logs writing in background
    _log_guard: std::sync::Mutex<Option<tracing_appender::non_blocking::WorkerGuard>>,
}

// ==========================================
// Tauri Command RPC Routing
// ==========================================

#[tauri::command]
async fn analyze_url(
    state: State<'_, AppState>,
    url: String,
    cookies_browser: Option<String>
) -> Result<serde_json::Value, String> {
    let custom_path = state.db.get_setting("custom_yt_dlp_path").unwrap_or(None);
    let engine = YtDlpEngine::new((*state.bin_manager).clone(), custom_path);

    match engine.extract_metadata(&url, cookies_browser.as_deref()).await {
        Ok(meta) => {
            let caps = get_capabilities(&url);
            Ok(json!({
                "metadata": meta,
                "capabilities": caps
            }))
        }
        Err(e) => Err(format!("Analysis failed: {}", e)),
    }
}

#[tauri::command]
async fn add_download_task(
    state: State<'_, AppState>,
    url: String,
    task_type: TaskType
) -> Result<Task, String> {
    state.queue.add_task(url, task_type).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cancel_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.queue.cancel_task(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn resume_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.queue.resume_task(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_task(&id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_queue(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    state.db.load_all_tasks().map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_queue(state: State<'_, AppState>) -> Result<(), String> {
    state.db.clear_queue().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_history(state: State<'_, AppState>, search: Option<String>) -> Result<Vec<HistoryItem>, String> {
    state.db.load_history(search.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_history_item(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_history_item(&id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn locate_file(path: String) -> Result<(), String> {
    let path_buf = std::path::PathBuf::from(&path);
    if !path_buf.exists() {
        return Err("File does not exist on disk.".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .status();
        match status {
            Ok(s) if s.success() => Ok(()),
            _ => Err("Failed to reveal file in Finder.".to_string()),
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        let path_norm = path.replace("/", "\\");
        let status = std::process::Command::new("explorer")
            .arg("/select,")
            .arg(path_norm)
            .spawn();
        match status {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to reveal file in Explorer: {}", e)),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(parent) = path_buf.parent() {
            let status = std::process::Command::new("xdg-open")
                .arg(parent.to_string_lossy().to_string())
                .status();
            match status {
                Ok(s) if s.success() => Ok(()),
                _ => Err("Failed to open directory.".to_string()),
            }
        } else {
            Err("Failed to resolve parent directory.".to_string())
        }
    }
}

#[tauri::command]
async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.db.clear_history().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let dl_dir = state.db.get_setting("download_directory").unwrap_or(None);
    let max_tasks = state.db.get_setting("max_concurrent_tasks").unwrap_or(None);
    let skip_dup = state.db.get_setting("skip_previously_downloaded").unwrap_or(None);
    let browser = state.db.get_setting("cookies_browser").unwrap_or(None);
    let channel = state.db.get_setting("yt_dlp_channel").unwrap_or(None);
    let advanced = state.db.get_setting("advanced_mode").unwrap_or(None);
    let custom_flags = state.db.get_setting("custom_yt_dlp_flags").unwrap_or(None);
    let custom_yt = state.db.get_setting("custom_yt_dlp_path").unwrap_or(None);

    let default_dl = dirs::download_dir()
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_else(|| "./downloads".to_string());

    Ok(json!({
        "download_directory": dl_dir.unwrap_or(default_dl),
        "max_concurrent_tasks": max_tasks.unwrap_or_else(|| "2".to_string()).parse::<usize>().unwrap_or(2),
        "skip_previously_downloaded": skip_dup.unwrap_or_else(|| "false".to_string()) == "true",
        "cookies_browser": browser.unwrap_or_default(),
        "yt_dlp_channel": channel.unwrap_or_else(|| "Stable".to_string()),
        "advanced_mode": advanced.unwrap_or_else(|| "false".to_string()) == "true",
        "custom_yt_dlp_flags": custom_flags.unwrap_or_default(),
        "custom_yt_dlp_path": custom_yt.unwrap_or_default(),
    }))
}

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: serde_json::Value) -> Result<(), String> {
    if let Some(dl_dir) = settings["download_directory"].as_str() {
        state.db.save_setting("download_directory", dl_dir).ok();
    }
    if let Some(max_tasks) = settings["max_concurrent_tasks"].as_u64() {
        state.db.save_setting("max_concurrent_tasks", &max_tasks.to_string()).ok();
        state.queue.set_max_concurrency(max_tasks as usize).await;
    }
    if let Some(skip) = settings["skip_previously_downloaded"].as_bool() {
        state.db.save_setting("skip_previously_downloaded", &skip.to_string()).ok();
    }
    if let Some(browser) = settings["cookies_browser"].as_str() {
        state.db.save_setting("cookies_browser", browser).ok();
    }
    if let Some(channel) = settings["yt_dlp_channel"].as_str() {
        state.db.save_setting("yt_dlp_channel", channel).ok();
    }
    if let Some(advanced) = settings["advanced_mode"].as_bool() {
        state.db.save_setting("advanced_mode", &advanced.to_string()).ok();
    }
    if let Some(flags) = settings["custom_yt_dlp_flags"].as_str() {
        state.db.save_setting("custom_yt_dlp_flags", flags).ok();
    }
    if let Some(custom_yt) = settings["custom_yt_dlp_path"].as_str() {
        state.db.save_setting("custom_yt_dlp_path", custom_yt).ok();
    }
    Ok(())
}

#[tauri::command]
async fn get_presets() -> Result<serde_json::Value, String> {
    Ok(json!(get_default_presets()))
}

#[tauri::command]
async fn run_self_check(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let custom_yt = state.db.get_setting("custom_yt_dlp_path").unwrap_or(None);
    
    let yt_dlp_status = match state.bin_manager.resolve_yt_dlp_binary(custom_yt.as_deref()) {
        Ok(path) => {
            match state.bin_manager.get_yt_dlp_version(custom_yt.as_deref()) {
                Ok(v) => json!({ "status": "OK", "path": path, "version": v }),
                Err(e) => json!({ "status": "ERROR", "path": path, "error": format!("Could not query version: {}", e) }),
            }
        }
        Err(e) => json!({ "status": "MISSING", "error": e.to_string() })
    };

    let ffmpeg_status = match state.bin_manager.resolve_ffmpeg_binary() {
        Ok(path) => json!({ "status": "OK", "path": path }),
        Err(e) => json!({ "status": "MISSING", "error": e.to_string() })
    };

    let db_integrity = state.db.verify_integrity();

    Ok(json!({
        "yt_dlp": yt_dlp_status,
        "ffmpeg": ffmpeg_status,
        "database": if db_integrity { "OK" } else { "CORRUPT" },
        "bin_dir": state.bin_manager.get_bin_dir()
    }))
}

#[tauri::command]
async fn force_yt_dlp_update(state: State<'_, AppState>) -> Result<String, String> {
    let channel_str = state.db.get_setting("yt_dlp_channel").unwrap_or(None);
    let channel = match channel_str.as_deref() {
        Some("Beta") => UpdateChannel::Beta,
        Some("Nightly") => UpdateChannel::Nightly,
        _ => UpdateChannel::Stable,
    };

    match state.bin_manager.download_yt_dlp(channel).await {
        Ok(path) => {
            let ver = state.bin_manager.get_yt_dlp_version(None).unwrap_or_else(|_| "Unknown".to_string());
            Ok(format!("Successfully downloaded and updated yt-dlp to version {} at {:?}", ver, path))
        }
        Err(e) => Err(format!("Update failed: {}", e))
    }
}

#[tauri::command]
async fn force_ffmpeg_update(state: State<'_, AppState>) -> Result<String, String> {
    match state.bin_manager.download_ffmpeg_and_ffprobe().await {
        Ok(_) => Ok("Successfully downloaded and updated FFmpeg and FFprobe.".to_string()),
        Err(e) => Err(format!("FFmpeg update failed: {}", e))
    }
}

// ==========================================
// Tauri Builder Setup
// ==========================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Harden PATH environment variable on macOS/Linux to include homebrew and conda python paths
    #[cfg(unix)]
    {
        if let Ok(current_path) = std::env::var("PATH") {
            let mut new_paths = vec![
                "/opt/homebrew/bin".to_string(),
                "/usr/local/bin".to_string(),
                "/opt/anaconda3/bin".to_string(),
                "/opt/miniconda3/bin".to_string(),
            ];
            
            if let Some(home) = dirs::home_dir() {
                new_paths.push(home.join("anaconda3").join("bin").to_string_lossy().to_string());
                new_paths.push(home.join("opt").join("anaconda3").join("bin").to_string_lossy().to_string());
                new_paths.push(home.join("miniconda3").join("bin").to_string_lossy().to_string());
            }
            
            new_paths.push(current_path);
            let combined_path = new_paths.join(":");
            std::env::set_var("PATH", combined_path);
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            analyze_url,
            add_download_task,
            cancel_download,
            resume_download,
            delete_task,
            get_queue,
            clear_queue,
            get_history,
            delete_history_item,
            locate_file,
            clear_history,
            get_settings,
            save_settings,
            get_presets,
            run_self_check,
            force_yt_dlp_update,
            force_ffmpeg_update
        ])
        .setup(|app| {
            // Determine portable mode trigger
            let portable_mode = std::path::Path::new("portable.txt").exists()
                || std::env::args().any(|arg| arg == "--portable");

            // Startup dynamic structured logger (safely wrap without panicking)
            let log_guard = match fetchr_core::logger::init_logger(portable_mode, true) {
                Ok(guard) => Some(guard),
                Err(e) => {
                    eprintln!("Warning: Failed to initialize logger: {}", e);
                    None
                }
            };

            // Locate database destination
            let db_path = if portable_mode {
                std::path::PathBuf::from("./config/fetchr.db")
            } else {
                match app.path().app_config_dir() {
                    Ok(config_dir) => config_dir.join("fetchr.db"),
                    Err(_) => std::path::PathBuf::from("fetchr_fallback.db"),
                }
            };

            // Double-harden SQLite connection: fallback to memory database instead of crashing
            let db = {
                let physical_db = Arc::new(DbManager::new(db_path));
                match physical_db.initialize() {
                    Ok(_) => physical_db,
                    Err(e) => {
                        eprintln!("Database initialization failed: {}. Falling back to in-memory database.", e);
                        let in_memory_db = Arc::new(DbManager::new(std::path::PathBuf::from(":memory:")));
                        let _ = in_memory_db.initialize();
                        in_memory_db
                    }
                }
            };

            let bin_dir = if portable_mode {
                std::path::PathBuf::from("./bin")
            } else {
                #[cfg(target_os = "android")]
                {
                    app.path().app_local_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("/data/local/tmp")).join("bin")
                }
                #[cfg(not(target_os = "android"))]
                {
                    match dirs::home_dir() {
                        Some(home) => home.join(".videosaver").join("bin"),
                        None => std::path::PathBuf::from("./.videosaver").join("bin"),
                    }
                }
            };
            let bin_manager = Arc::new(BinManager::new(bin_dir));

            // Auto-scaffold bin directories
            std::fs::create_dir_all(bin_manager.get_bin_dir()).ok();

            // Startup background binary auto-update task (except on mobile/Android)
            #[cfg(not(target_os = "android"))]
            {
                let bin_manager_clone = bin_manager.clone();
                let db_clone = db.clone();
                tauri::async_runtime::spawn(async move {
                    let custom_yt = db_clone.get_setting("custom_yt_dlp_path").unwrap_or(None);
                    if bin_manager_clone.resolve_yt_dlp_binary(custom_yt.as_deref()).is_err() {
                        tracing::info!("yt-dlp missing at startup. Running auto-download...");
                        let channel_str = db_clone.get_setting("yt_dlp_channel").unwrap_or(None);
                        let channel = match channel_str.as_deref() {
                            Some("Beta") => UpdateChannel::Beta,
                            Some("Nightly") => UpdateChannel::Nightly,
                            _ => UpdateChannel::Stable,
                        };
                        if let Err(e) = bin_manager_clone.download_yt_dlp(channel).await {
                            tracing::error!("Failed to auto-download yt-dlp: {}", e);
                        }
                    }

                    if bin_manager_clone.resolve_ffmpeg_binary().is_err() {
                        tracing::info!("ffmpeg missing at startup. Running auto-download...");
                        if let Err(e) = bin_manager_clone.download_ffmpeg_and_ffprobe().await {
                            tracing::error!("Failed to auto-download ffmpeg/ffprobe: {}", e);
                        }
                    }
                });
            }

            let dispatcher = Arc::new(TauriEventDispatcher {
                app_handle: app.handle().clone(),
            });

            let queue = Arc::new(QueueOrchestrator::new(
                db.clone(),
                bin_manager.clone(),
                dispatcher,
            ));

            // Startup schedule loops
            queue.clone().start();

            // Set dynamic concurrency on launch
            let saved_concurrency = db.get_setting("max_concurrent_tasks").unwrap_or(None);
            if let Some(limit_str) = saved_concurrency {
                if let Ok(limit) = limit_str.parse::<usize>() {
                    let q = queue.clone();
                    tauri::async_runtime::spawn(async move {
                        q.set_max_concurrency(limit).await;
                    });
                }
            }

            // Bind State globally
            app.manage(AppState {
                db,
                bin_manager,
                queue,
                _log_guard: std::sync::Mutex::new(log_guard),
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error while running tauri application");
}
