use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, Result};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum UpdateChannel {
    Stable,
    Beta,
    Nightly,
}

impl UpdateChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            UpdateChannel::Stable => "Stable",
            UpdateChannel::Beta => "Beta",
            UpdateChannel::Nightly => "Nightly",
        }
    }

    pub fn download_url(&self) -> &'static str {
        #[cfg(target_os = "windows")]
        {
            match self {
                UpdateChannel::Stable => "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
                UpdateChannel::Beta => "https://github.com/yt-dlp/yt-dlp-master-builds/releases/latest/download/yt-dlp.exe",
                UpdateChannel::Nightly => "https://github.com/yt-dlp/yt-dlp-nightly-builds/releases/latest/download/yt-dlp.exe",
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            match self {
                UpdateChannel::Stable => "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp",
                UpdateChannel::Beta => "https://github.com/yt-dlp/yt-dlp-master-builds/releases/latest/download/yt-dlp",
                UpdateChannel::Nightly => "https://github.com/yt-dlp/yt-dlp-nightly-builds/releases/latest/download/yt-dlp",
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct BinManager {
    pub bin_dir: PathBuf,
}

impl BinManager {
    pub fn new(bin_dir: PathBuf) -> Self {
        Self { bin_dir }
    }

    /// Retrieve the standard directory for binaries
    pub fn get_bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    /// Checks if a local downloaded yt-dlp binary exists.
    pub fn yt_dlp_local_exists(&self) -> bool {
        self.get_yt_dlp_path().exists()
    }

    /// Returns the target path for the local yt-dlp executable.
    pub fn get_yt_dlp_path(&self) -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            self.bin_dir.join("yt-dlp.exe")
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.bin_dir.join("yt-dlp")
        }
    }

    /// Resolves the absolute path to use for yt-dlp, checking settings, local downloaded bins, and system PATH.
    pub fn resolve_yt_dlp_binary(&self, custom_path: Option<&str>) -> Result<PathBuf> {
        // 1. Check custom path in settings
        if let Some(cp) = custom_path {
            let path = PathBuf::from(cp);
            if path.exists() && path.is_file() {
                return Ok(path);
            }
        }

        // 2. Check local downloaded binary
        let local_path = self.get_yt_dlp_path();
        if local_path.exists() {
            return Ok(local_path);
        }

        // 3. Fallback to system PATH
        if let Ok(system_path) = self.find_in_path("yt-dlp") {
            return Ok(system_path);
        }

        Err(anyhow!("yt-dlp binary could not be found locally or on the system PATH."))
    }

    /// Resolves ffmpeg path checking local downloaded bin, then system PATH.
    pub fn resolve_ffmpeg_binary(&self) -> Result<PathBuf> {
        let local_name = if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" };
        let local_path = self.bin_dir.join(local_name);
        if local_path.exists() {
            return Ok(local_path);
        }

        if let Ok(system_path) = self.find_in_path("ffmpeg") {
            return Ok(system_path);
        }

        Err(anyhow!("ffmpeg binary could not be resolved."))
    }

    /// Dynamically detects the best available H.264 hardware encoder on the system, falling back to libx264.
    pub fn get_best_h264_encoder(&self) -> String {
        let ffmpeg_path = match self.resolve_ffmpeg_binary() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("Could not resolve ffmpeg binary to query encoders. Falling back to libx264.");
                return "libx264".to_string();
            }
        };

        let mut cmd = Command::new(&ffmpeg_path);
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let output = match cmd.arg("-encoders").output() {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!("Failed to run ffmpeg -encoders: {}. Falling back to libx264.", e);
                return "libx264".to_string();
            }
        };

        if !output.status.success() {
            tracing::warn!("ffmpeg -encoders returned non-zero status. Falling back to libx264.");
            return "libx264".to_string();
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);

        // Priority check for hardware/GPU accelerated encoders
        if stdout_str.contains("h264_videotoolbox") {
            tracing::info!("Detected macOS GPU-accelerated encoder: h264_videotoolbox");
            return "h264_videotoolbox".to_string();
        }
        if stdout_str.contains("h264_nvenc") {
            tracing::info!("Detected NVIDIA GPU-accelerated encoder: h264_nvenc");
            return "h264_nvenc".to_string();
        }
        if stdout_str.contains("h264_amf") {
            tracing::info!("Detected AMD GPU-accelerated encoder: h264_amf");
            return "h264_amf".to_string();
        }
        if stdout_str.contains("h264_qsv") {
            tracing::info!("Detected Intel GPU-accelerated encoder: h264_qsv");
            return "h264_qsv".to_string();
        }

        tracing::info!("No hardware-accelerated H.264 encoder detected. Using software encoder: libx264");
        "libx264".to_string()
    }

    /// Resolves ffprobe path checking local downloaded bin, then system PATH.
    pub fn resolve_ffprobe_binary(&self) -> Result<PathBuf> {
        let local_name = if cfg!(target_os = "windows") { "ffprobe.exe" } else { "ffprobe" };
        let local_path = self.bin_dir.join(local_name);
        if local_path.exists() {
            return Ok(local_path);
        }

        if let Ok(system_path) = self.find_in_path("ffprobe") {
            return Ok(system_path);
        }

        Err(anyhow!("ffprobe binary could not be resolved."))
    }

    /// Search system environment path for a binary.
    fn find_in_path(&self, bin_name: &str) -> Result<PathBuf> {
        let full_name = if cfg!(target_os = "windows") {
            format!("{}.exe", bin_name)
        } else {
            bin_name.to_string()
        };

        if let Ok(paths) = std::env::var("PATH") {
            let split_char = if cfg!(target_os = "windows") { ';' } else { ':' };
            for path in paths.split(split_char) {
                let candidate = Path::new(path).join(&full_name);
                if candidate.exists() && candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
        
        // Final desperate check with standard folders on Unix
        if cfg!(unix) {
            for standard_dir in &["/usr/bin", "/usr/local/bin", "/opt/homebrew/bin", "/opt/local/bin"] {
                let candidate = Path::new(standard_dir).join(&full_name);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }

        Err(anyhow!("Binary {} not found on system PATH", bin_name))
    }

    /// Query the version of local/resolved yt-dlp by calling --version
    pub fn get_yt_dlp_version(&self, custom_path: Option<&str>) -> Result<String> {
        let binary_path = self.resolve_yt_dlp_binary(custom_path)?;
        tracing::info!("Querying version for yt-dlp at: {:?}", binary_path);
        let mut cmd = Command::new(&binary_path);
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let output = match cmd.arg("--version").output() {
                Ok(o) => o,
                Err(e) => {
                    tracing::error!("Failed to execute yt-dlp binary at {:?}: {}", binary_path, e);
                    return Err(anyhow!("Failed to execute binary: {}", e));
                }
            };

        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            tracing::info!("yt-dlp version successfully queried: {}", ver);
            Ok(ver)
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            tracing::error!("yt-dlp exited with error status: {}, stderr: {}", output.status, err);
            Err(anyhow!("Failed to query version: {}", err))
        }
    }

    /// Dynamically download yt-dlp from the official github release matching chosen channel.
    pub async fn download_yt_dlp(&self, channel: UpdateChannel) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.bin_dir)?;
        let target_path = self.get_yt_dlp_path();
        let url = channel.download_url();

        // Download via reqwest
        let response = reqwest::get(url).await?;
        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch yt-dlp binary: HTTP {}", response.status()));
        }

        let bytes = response.bytes().await?;
        std::fs::write(&target_path, bytes)?;

        // Critical: Set execute permissions on macOS/Linux
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(&target_path) {
                let mut permissions = metadata.permissions();
                permissions.set_mode(0o755); // rwxr-xr-x
                std::fs::set_permissions(&target_path, permissions)?;
            }
        }

        Ok(target_path)
    }

    /// Dynamically download and extract ffmpeg and ffprobe for the current platform.
    pub async fn download_ffmpeg_and_ffprobe(&self) -> Result<()> {
        #[cfg(target_os = "android")]
        {
            tracing::info!("Auto-updating ffmpeg and ffprobe is not supported natively on Android.");
            return Ok(());
        }

        #[cfg(not(target_os = "android"))]
        {
            std::fs::create_dir_all(&self.bin_dir)?;
            let temp_dir = std::env::temp_dir().join("ffmpeg_download");
            std::fs::create_dir_all(&temp_dir)?;

            #[cfg(target_os = "macos")]
            {
                // Download ffmpeg zip
                let ffmpeg_zip = temp_dir.join("ffmpeg.zip");
                let response = reqwest::get("https://evermeet.cx/ffmpeg/get/zip").await?;
                if !response.status().is_success() {
                    return Err(anyhow!("Failed to fetch macOS ffmpeg: HTTP {}", response.status()));
                }
                std::fs::write(&ffmpeg_zip, response.bytes().await?)?;

                // Unzip ffmpeg
                let extract_status = Command::new("unzip")
                    .arg("-o")
                    .arg(&ffmpeg_zip)
                    .arg("-d")
                    .arg(&temp_dir)
                    .status()?;
                if !extract_status.success() {
                    return Err(anyhow!("Failed to extract macOS ffmpeg zip"));
                }

                // Move ffmpeg
                let ffmpeg_src = temp_dir.join("ffmpeg");
                let ffmpeg_dest = self.bin_dir.join("ffmpeg");
                std::fs::rename(&ffmpeg_src, &ffmpeg_dest)?;

                // Download ffprobe zip
                let ffprobe_zip = temp_dir.join("ffprobe.zip");
                let response = reqwest::get("https://evermeet.cx/ffmpeg/get/ffprobe/zip").await?;
                if !response.status().is_success() {
                    return Err(anyhow!("Failed to fetch macOS ffprobe: HTTP {}", response.status()));
                }
                std::fs::write(&ffprobe_zip, response.bytes().await?)?;

                // Unzip ffprobe
                let extract_status = Command::new("unzip")
                    .arg("-o")
                    .arg(&ffprobe_zip)
                    .arg("-d")
                    .arg(&temp_dir)
                    .status()?;
                if !extract_status.success() {
                    return Err(anyhow!("Failed to extract macOS ffprobe zip"));
                }

                // Move ffprobe
                let ffprobe_src = temp_dir.join("ffprobe");
                let ffprobe_dest = self.bin_dir.join("ffprobe");
                std::fs::rename(&ffprobe_src, &ffprobe_dest)?;

                // Set execute permissions
                use std::os::unix::fs::PermissionsExt;
                for binary in &["ffmpeg", "ffprobe"] {
                    let path = self.bin_dir.join(binary);
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        let mut permissions = metadata.permissions();
                        permissions.set_mode(0o755);
                        std::fs::set_permissions(&path, permissions)?;
                    }
                }
            }

            #[cfg(target_os = "windows")]
            {
                // Download ffmpeg zip from gyan.dev
                let zip_path = temp_dir.join("ffmpeg.zip");
                let response = reqwest::get("https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip").await?;
                if !response.status().is_success() {
                    return Err(anyhow!("Failed to fetch Windows ffmpeg: HTTP {}", response.status()));
                }
                std::fs::write(&zip_path, response.bytes().await?)?;

                // Unzip using powershell
                let extract_status = Command::new("powershell")
                    .arg("-Command")
                    .arg(format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        zip_path.to_string_lossy(),
                        temp_dir.to_string_lossy()
                    ))
                    .status()?;
                if !extract_status.success() {
                    return Err(anyhow!("Failed to extract Windows ffmpeg zip"));
                }

                // Locate the binaries inside the extracted folders
                // The zip contains a folder like `ffmpeg-*-essentials_build/bin/ffmpeg.exe`
                let mut ffmpeg_found = false;
                let mut ffprobe_found = false;
                for entry in std::fs::read_dir(&temp_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() && path.file_name().unwrap_or_default().to_string_lossy().starts_with("ffmpeg-") {
                        let bin_sub_dir = path.join("bin");
                        if bin_sub_dir.exists() {
                            let ffmpeg_exe = bin_sub_dir.join("ffmpeg.exe");
                            let ffprobe_exe = bin_sub_dir.join("ffprobe.exe");
                            if ffmpeg_exe.exists() {
                                std::fs::rename(&ffmpeg_exe, self.bin_dir.join("ffmpeg.exe"))?;
                                ffmpeg_found = true;
                            }
                            if ffprobe_exe.exists() {
                                std::fs::rename(&ffprobe_exe, self.bin_dir.join("ffprobe.exe"))?;
                                ffprobe_found = true;
                            }
                        }
                    }
                }

                if !ffmpeg_found || !ffprobe_found {
                    return Err(anyhow!("Could not locate ffmpeg.exe or ffprobe.exe inside extracted archive"));
                }
            }

            #[cfg(target_os = "linux")]
            {
                // Download ffmpeg tar.xz from johnvansickle
                let tar_path = temp_dir.join("ffmpeg.tar.xz");
                let response = reqwest::get("https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz").await?;
                if !response.status().is_success() {
                    return Err(anyhow!("Failed to fetch Linux ffmpeg: HTTP {}", response.status()));
                }
                std::fs::write(&tar_path, response.bytes().await?)?;

                // Extract tar.xz
                let extract_status = Command::new("tar")
                    .arg("-xf")
                    .arg(&tar_path)
                    .arg("-C")
                    .arg(&temp_dir)
                    .status()?;
                if !extract_status.success() {
                    return Err(anyhow!("Failed to extract Linux ffmpeg tar.xz"));
                }

                // Locate the binaries inside the extracted folders
                let mut ffmpeg_found = false;
                let mut ffprobe_found = false;
                for entry in std::fs::read_dir(&temp_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() && path.file_name().unwrap_or_default().to_string_lossy().starts_with("ffmpeg-") {
                        let ffmpeg_bin = path.join("ffmpeg");
                        let ffprobe_bin = path.join("ffprobe");
                        if ffmpeg_bin.exists() {
                            std::fs::rename(&ffmpeg_bin, self.bin_dir.join("ffmpeg"))?;
                            ffmpeg_found = true;
                        }
                        if ffprobe_bin.exists() {
                            std::fs::rename(&ffprobe_bin, self.bin_dir.join("ffprobe"))?;
                            ffprobe_found = true;
                        }
                    }
                }

                if !ffmpeg_found || !ffprobe_found {
                    return Err(anyhow!("Could not locate ffmpeg or ffprobe inside extracted archive"));
                }

                // Set execute permissions
                use std::os::unix::fs::PermissionsExt;
                for binary in &["ffmpeg", "ffprobe"] {
                    let path = self.bin_dir.join(binary);
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        let mut permissions = metadata.permissions();
                        permissions.set_mode(0o755);
                        std::fs::set_permissions(&path, permissions)?;
                    }
                }
            }

            // Cleanup temp dir
            std::fs::remove_dir_all(&temp_dir).ok();
            Ok(())
        }
    }
}
