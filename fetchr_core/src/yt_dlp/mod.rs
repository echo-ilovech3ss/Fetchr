pub mod bin_manager;
pub mod progress_parser;

pub use bin_manager::{BinManager, UpdateChannel};
pub use progress_parser::{CompositeProgressParser, DownloadProgress, ProgressParser};

use std::path::PathBuf;
use std::process::Stdio;
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
        #[cfg(target_os = "android")]
        {
            tracing::info!("Running on Android: bypassing yt-dlp execution and falling back to oEmbed metadata extraction.");
            self.extract_metadata_oembed(url).await
        }

        #[cfg(not(target_os = "android"))]
        {
            let yt_dlp_path = match self.get_executable_path() {
                Ok(p) => p,
                Err(e) => {
                    tracing::info!("yt-dlp binary is missing or could not be resolved: {}. Falling back to oEmbed metadata extraction.", e);
                    return self.extract_metadata_oembed(url).await;
                }
            };
            let mut cmd = Command::new(yt_dlp_path);
            #[cfg(target_os = "windows")]
            {
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

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

            let output = match cmd.output().await {
                Ok(out) => out,
                Err(e) => {
                    tracing::warn!("Failed to spawn/execute yt-dlp: {}. Falling back to oEmbed metadata extraction.", e);
                    return self.extract_metadata_oembed(url).await;
                }
            };

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
                tracing::warn!("yt-dlp metadata extraction failed: {}. Falling back to oEmbed metadata extraction.", err);
                return self.extract_metadata_oembed(url).await;
            }

            let raw_json: serde_json::Value = match serde_json::from_slice(&output.stdout) {
                Ok(json) => json,
                Err(e) => {
                    tracing::warn!("Failed to parse yt-dlp JSON output: {}. Falling back to oEmbed metadata extraction.", e);
                    return self.extract_metadata_oembed(url).await;
                }
            };

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

    /// Extract basic metadata via public oEmbed API for YouTube, Vimeo, or TikTok.
    pub async fn extract_metadata_oembed(&self, url: &str) -> Result<MediaMetadata> {
        let encoded_url: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();
        let oembed_url = if url.contains("youtube.com") || url.contains("youtu.be") {
            format!("https://www.youtube.com/oembed?url={}&format=json", encoded_url)
        } else if url.contains("vimeo.com") {
            format!("https://vimeo.com/api/oembed.json?url={}", encoded_url)
        } else if url.contains("tiktok.com") {
            format!("https://www.tiktok.com/oembed?url={}", encoded_url)
        } else {
            return Err(anyhow!("Unsupported platform for offline/sandbox scanning. Direct yt-dlp required."));
        };

        let response = reqwest::get(&oembed_url).await?;
        if !response.status().is_success() {
            return Err(anyhow!("Failed to query metadata: HTTP {}", response.status()));
        }

        let json: serde_json::Value = response.json().await?;

        let title = json["title"].as_str().unwrap_or("Untitled Media").to_string();
        let uploader = json["author_name"].as_str().map(|s| s.to_string());
        let uploader_url = json["author_url"].as_str().map(|s| s.to_string());
        let thumbnail_url = json["thumbnail_url"].as_str().map(|s| s.to_string());
        let extractor = json["provider_name"].as_str().unwrap_or("Generic").to_string().to_lowercase();
        let duration = json["duration"].as_f64().unwrap_or(0.0);

        let formats = vec![MediaFormat {
            format_id: "best".to_string(),
            ext: "mp4".to_string(),
            resolution: Some("Auto".to_string()),
            filesize: None,
            vcodec: Some("h264".to_string()),
            acodec: Some("aac".to_string()),
            note: Some("Direct download format".to_string()),
        }];

        Ok(MediaMetadata {
            title,
            description: None,
            duration,
            uploader,
            uploader_url,
            thumbnail_url,
            webpage_url: url.to_string(),
            formats,
            extractor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yt_dlp::bin_manager::BinManager;

    #[tokio::test]
    async fn test_youtube_metadata() {
        let home = dirs::home_dir().unwrap();
        let bin_dir = home.join(".videosaver").join("bin");
        let bin_manager = BinManager::new(bin_dir);
        let engine = YtDlpEngine::new(bin_manager, None);
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        
        match engine.extract_metadata(url, None).await {
            Ok(meta) => {
                println!("Metadata: {:?}", meta);
                assert!(meta.extractor == "youtube" || meta.extractor == "generic");
            }
            Err(e) => {
                panic!("Failed to extract metadata: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_youtube_metadata_oembed() {
        let home = dirs::home_dir().unwrap();
        let bin_dir = home.join(".videosaver").join("bin");
        let bin_manager = BinManager::new(bin_dir);
        let engine = YtDlpEngine::new(bin_manager, None);
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        
        match engine.extract_metadata_oembed(url).await {
            Ok(meta) => {
                println!("oEmbed Metadata: {:?}", meta);
                assert_eq!(meta.extractor, "youtube");
            }
            Err(e) => {
                panic!("Failed to extract oEmbed metadata: {}", e);
            }
        }
    }
}

