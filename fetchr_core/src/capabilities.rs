use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    pub platform_name: String,
    pub supports_audio: bool,
    pub supports_subtitles: bool,
    pub supports_playlists: bool,
    pub supports_login: bool,
}

/// Detect capabilities dynamically based on the input URL domain
pub fn get_capabilities(url: &str) -> PlatformCapabilities {
    let url_lower = url.to_lowercase();
    
    if url_lower.contains("youtube.com") || url_lower.contains("youtu.be") {
        PlatformCapabilities {
            platform_name: "YouTube".to_string(),
            supports_audio: true,
            supports_subtitles: true,
            supports_playlists: true,
            supports_login: true,
        }
    } else if url_lower.contains("instagram.com") {
        PlatformCapabilities {
            platform_name: "Instagram".to_string(),
            supports_audio: true,
            supports_subtitles: false,
            supports_playlists: false,
            supports_login: true, // Cookies are super important for Instagram!
        }
    } else if url_lower.contains("facebook.com") || url_lower.contains("fb.watch") {
        PlatformCapabilities {
            platform_name: "Facebook".to_string(),
            supports_audio: true,
            supports_subtitles: false,
            supports_playlists: false,
            supports_login: true,
        }
    } else if url_lower.contains("tiktok.com") {
        PlatformCapabilities {
            platform_name: "TikTok".to_string(),
            supports_audio: true,
            supports_subtitles: false,
            supports_playlists: false,
            supports_login: false,
        }
    } else if url_lower.contains("vimeo.com") {
        PlatformCapabilities {
            platform_name: "Vimeo".to_string(),
            supports_audio: true,
            supports_subtitles: true,
            supports_playlists: false,
            supports_login: true,
        }
    } else {
        // Safe generic fallback
        PlatformCapabilities {
            platform_name: "Generic Link".to_string(),
            supports_audio: true,
            supports_subtitles: false,
            supports_playlists: false,
            supports_login: true,
        }
    }
}
