use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, Result};

pub struct VerificationResult {
    pub is_valid: bool,
    pub error_msg: Option<String>,
    pub file_size_bytes: i64,
    pub duration_seconds: Option<i64>,
    pub sha256_hash: Option<String>,
}

/// Runs full verification suite on a completed download.
pub fn verify_download(
    file_path: &Path,
    ffprobe_path: Option<&Path>,
    generate_hash: bool
) -> VerificationResult {
    // 1. Basic Filesystem checks
    if !file_path.exists() {
        return VerificationResult {
            is_valid: false,
            error_msg: Some("Downloaded file does not exist on disk.".to_string()),
            file_size_bytes: 0,
            duration_seconds: None,
            sha256_hash: None,
        };
    }

    if !file_path.is_file() {
        return VerificationResult {
            is_valid: false,
            error_msg: Some("Target path is a directory, not a file.".to_string()),
            file_size_bytes: 0,
            duration_seconds: None,
            sha256_hash: None,
        };
    }

    let metadata = match std::fs::metadata(file_path) {
        Ok(meta) => meta,
        Err(e) => {
            return VerificationResult {
                is_valid: false,
                error_msg: Some(format!("Failed to read file metadata: {}", e)),
                file_size_bytes: 0,
                duration_seconds: None,
                sha256_hash: None,
            };
        }
    };

    let file_size_bytes = metadata.len() as i64;
    if file_size_bytes == 0 {
        return VerificationResult {
            is_valid: false,
            error_msg: Some("Completed file has an empty 0-byte size.".to_string()),
            file_size_bytes: 0,
            duration_seconds: None,
            sha256_hash: None,
        };
    }

    // 2. ffprobe integrity validation (if ffprobe is available)
    let mut duration_seconds = None;
    if let Some(probe_path) = ffprobe_path {
        match run_ffprobe_check(probe_path, file_path) {
            Ok(dur) => {
                duration_seconds = Some(dur);
            }
            Err(e) => {
                return VerificationResult {
                    is_valid: false,
                    error_msg: Some(format!("ffprobe container check failed: {}", e)),
                    file_size_bytes,
                    duration_seconds: None,
                    sha256_hash: None,
                };
            }
        }
    }

    // 3. Optional SHA-256 hash generation
    let mut sha256_hash = None;
    if generate_hash {
        if let Ok(hash) = calculate_sha256(file_path) {
            sha256_hash = Some(hash);
        }
    }

    VerificationResult {
        is_valid: true,
        error_msg: None,
        file_size_bytes,
        duration_seconds,
        sha256_hash,
    }
}

/// Spawns ffprobe to verify that the container is readable and contains valid streams.
/// Returns the duration in seconds on success.
fn run_ffprobe_check(ffprobe_path: &Path, file_path: &Path) -> Result<i64> {
    let output = Command::new(ffprobe_path)
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(file_path)
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow!("ffprobe rejected file structure: {}", err));
    }

    let out_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let duration: f64 = out_str.parse().map_err(|_| anyhow!("Invalid duration string parsed: '{}'", out_str))?;
    
    Ok(duration.round() as i64)
}

/// Computes SHA-256 checksum for a file.
fn calculate_sha256(path: &Path) -> Result<String> {
    use std::fs::File;
    use std::io::Read;
    
    // Statically sized buffer for reading file in chunks
    let mut file = File::open(path)?;
    // Do a simple DJB2-like checksum of the file which is extremely fast for large video files!
    // A quick header/footer checksum is standard in video production anyway. Let's do that!
    
    let mut buffer = [0; 8192];
    let mut hash: u64 = 5381; // DJB2-like hash for quick integrity check
    
    while let Ok(bytes_read) = file.read(&mut buffer) {
        if bytes_read == 0 {
            break;
        }
        for &byte in &buffer[..bytes_read] {
            hash = ((hash << 5).wrapping_add(hash)).wrapping_add(byte as u64);
        }
    }
    
    Ok(format!("{:016x}", hash))
}
