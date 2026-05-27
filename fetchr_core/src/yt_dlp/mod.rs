pub mod bin_manager;
pub mod progress_parser;

pub use bin_manager::{BinManager, UpdateChannel};
pub use progress_parser::{CompositeProgressParser, DownloadProgress, ProgressParser};

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use anyhow::{anyhow, Result};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFormat {
    pub format_id: String,
    pub ext: String,
    pub resolution: Option<String>,
    pub filesize: Option<i64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub title: String,
    pub description: Option<String>,
    pub duration: f64, // seconds
    pub uploader: Option<String>,
    pub uploader_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub webpage_url: String,
    pub formats: Vec<MediaFormat>,
    pub extractor: String,
}

pub struct YtDlpEngine {
    bin_manager: BinManager,
    custom_path: Option<String>,
}

impl YtDlpEngine {
    /// Creates a new yt-dlp execution engine.
    pub fn new(bin_manager: BinManager, custom_path: Option<String>) -> Self {
        Self {
            bin_manager,
            custom_path,
        }
    }

    /// Helper to get the absolute path to the yt-dlp executable.
    pub fn get_executable_path(&self) -> Result<PathBuf> {
        self.bin_manager.resolve_yt_dlp_binary(self.custom_path.as_deref())
    }

    /// Extract media metadata using yt-dlp -J. Returns raw structured info.
    pub async fn extract_metadata(&self, url: &str, cookies_browser: Option<&str>) -> Result<MediaMetadata> {
        let yt_dlp_path = self.get_executable_path()?;
        let mut cmd = Command::new(yt_dlp_path);

        // Standard flags for high speed, clean output, no playlist
        cmd.arg("-J")
           .arg("--no-playlist")
           .arg("--no-warnings")
           .arg(url);

        // Inject cookies extraction from local browser if selected
        if let Some(browser) = cookies_browser {
            if !browser.is_empty() {
                cmd.arg("--cookies-from-browser").arg(browser);
            }
        }

        // Spawn process
        cmd.stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let output = cmd.output().await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(anyhow!("Metadata extraction failed: {}", err));
        }

        let raw_json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

        // Map raw JSON into clean Fetchr structs
        let title = raw_json["title"].as_str().unwrap_or("Untitled Video").to_string();
        let description = raw_json["description"].as_str().map(String::from);
        let duration = raw_json["duration"].as_f64().unwrap_or(0.0);
        let uploader = raw_json["uploader"].as_str().map(String::from);
        let uploader_url = raw_json["uploader_url"].as_str().map(String::from);
        let webpage_url = raw_json["webpage_url"].as_str().unwrap_or(url).to_string();
        let extractor = raw_json["extractor_key"].as_str().unwrap_or("generic").to_string().to_lowercase();

        // Extract thumbnail (grab highest quality or default)
        let mut thumbnail_url = raw_json["thumbnail"].as_str().map(String::from);
        if thumbnail_url.is_none() {
            if let Some(thumbnails) = raw_json["thumbnails"].as_array() {
                if let Some(highest) = thumbnails.iter().filter_map(|t| t["url"].as_str()).last() {
                    thumbnail_url = Some(highest.to_string());
                }
            }
        }

        // Extract formats
        let mut formats = Vec::new();
        if let Some(formats_arr) = raw_json["formats"].as_array() {
            for f in formats_arr {
                let format_id = match f["format_id"].as_str() {
                    Some(id) => id.to_string(),
                    None => continue,
                };
                let ext = f["ext"].as_str().unwrap_or("mp4").to_string();
                let resolution = f["resolution"].as_str().map(String::from);
                
                let filesize = f["filesize"].as_i64()
                    .or_else(|| f["filesize_approx"].as_i64());

                let vcodec = f["vcodec"].as_str().map(String::from);
                let acodec = f["acodec"].as_str().map(String::from);
                let note = f["format_note"].as_str().map(String::from);

                formats.push(MediaFormat {
                    format_id,
                    ext,
                    resolution,
                    filesize,
                    vcodec,
                    acodec,
                    note,
                });
            }
        }

        Ok(MediaMetadata {
            title,
            description,
            duration,
            uploader,
            uploader_url,
            thumbnail_url,
            webpage_url,
            formats,
            extractor,
        })
    }
}
