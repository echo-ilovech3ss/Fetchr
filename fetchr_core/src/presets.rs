use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub format_filter: String,
    pub merge_format: Option<String>,
    pub extract_audio: bool,
    pub audio_format: Option<String>,
    pub is_custom: bool,
}

/// Retrieves all default creator presets.
pub fn get_default_presets() -> Vec<Preset> {
    vec![
        Preset {
            id: "editing".to_string(),
            name: "Editing Quality (MKV / Raw)".to_string(),
            description: "Lossless video/audio merges. Best for importing into Premiere / Resolve.".to_string(),
            format_filter: "bestvideo+bestaudio/best".to_string(),
            merge_format: Some("mkv".to_string()),
            extract_audio: false,
            audio_format: None,
            is_custom: false,
        },
        Preset {
            id: "archive".to_string(),
            name: "Archive Quality (MP4)".to_string(),
            description: "Primes maximum resolutions available, merged cleanly to a standard MP4 container.".to_string(),
            format_filter: "bestvideo+bestaudio/best".to_string(),
            merge_format: Some("mp4".to_string()),
            extract_audio: false,
            audio_format: None,
            is_custom: false,
        },
        Preset {
            id: "social".to_string(),
            name: "Social Upload (H.264 / MP4)".to_string(),
            description: "Highly compatible H.264/AAC output within typical social media specs.".to_string(),
            format_filter: "bestvideo[vcodec^=avc1]+bestaudio[acodec^=mp4a]/best[ext=mp4]/best".to_string(),
            merge_format: Some("mp4".to_string()),
            extract_audio: false,
            audio_format: None,
            is_custom: false,
        },
        Preset {
            id: "audio".to_string(),
            name: "Audio Extraction (MP3)".to_string(),
            description: "Extracts highest bitrate audio stream and encodes it directly to 320kbps MP3.".to_string(),
            format_filter: "bestaudio/best".to_string(),
            merge_format: None,
            extract_audio: true,
            audio_format: Some("mp3".to_string()),
            is_custom: false,
        },
        Preset {
            id: "storage_saver".to_string(),
            name: "Storage Saver (720p MP4)".to_string(),
            description: "Limits resolution to 720p or lower to save bandwidth and local SSD space.".to_string(),
            format_filter: "bestvideo[height<=720]+bestaudio/best".to_string(),
            merge_format: Some("mp4".to_string()),
            extract_audio: false,
            audio_format: None,
            is_custom: false,
        },
    ]
}

/// Helper to get a specific preset by its identifier.
pub fn get_preset_by_id(id: &str) -> Option<Preset> {
    get_default_presets().into_iter().find(|p| p.id == id)
}
