use url::Url;

/// Supported social video platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    YouTube,
    Instagram,
    Facebook,
    TikTok,
    Vimeo,
    Generic,
}

/// Detects the target platform from a given URL string.
pub fn detect_platform(url: &str) -> Platform {
    let lower = url.to_lowercase();
    if lower.contains("youtube.com") || lower.contains("youtu.be") {
        Platform::YouTube
    } else if lower.contains("instagram.com") || lower.contains("instagr.am") {
        Platform::Instagram
    } else if lower.contains("facebook.com") || lower.contains("fb.watch") || lower.contains("fb.com") {
        Platform::Facebook
    } else if lower.contains("tiktok.com") {
        Platform::TikTok
    } else if lower.contains("vimeo.com") {
        Platform::Vimeo
    } else {
        Platform::Generic
    }
}

/// Normalizes social media URLs by removing tracking parameters and mapping share/mobile endpoints
/// into clean standard media endpoints that yt-dlp extractors handle cleanly.
pub fn normalize_url(raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut url_str = trimmed.to_string();

    // Ensure scheme is present
    if !url_str.starts_with("http://") && !url_str.starts_with("https://") {
        url_str = format!("https://{}", url_str);
    }

    let mut parsed = match Url::parse(&url_str) {
        Ok(u) => u,
        Err(_) => return url_str,
    };

    let host = match parsed.host_str() {
        Some(h) => h.to_lowercase(),
        None => return url_str,
    };

    // 1. Instagram URL Normalization
    if host.contains("instagram.com") || host.contains("instagr.am") {
        parsed.set_host(Some("www.instagram.com")).ok();
        let path = parsed.path().to_string();

        // Convert /share/reel/SHORTCODE or /share/p/SHORTCODE -> /reel/SHORTCODE or /p/SHORTCODE
        let normalized_path = if path.starts_with("/share/reel/") {
            path.replacen("/share/reel/", "/reel/", 1)
        } else if path.starts_with("/share/p/") {
            path.replacen("/share/p/", "/p/", 1)
        } else if path.starts_with("/reels/") {
            path.replacen("/reels/", "/reel/", 1)
        } else {
            path
        };

        parsed.set_path(&normalized_path);

        // Strip Instagram tracking query parameters
        let filtered_pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .filter(|(k, _)| {
                let key_lower = k.to_lowercase();
                !key_lower.starts_with("igsh")
                    && !key_lower.starts_with("utm_")
                    && key_lower != "ig_rid"
                    && key_lower != "ig_mid"
                    && key_lower != "sfnsn"
            })
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        parsed.query_pairs_mut().clear();
        if !filtered_pairs.is_empty() {
            for (k, v) in filtered_pairs {
                parsed.query_pairs_mut().append_pair(&k, &v);
            }
        } else {
            parsed.set_query(None);
        }

        return parsed.to_string();
    }

    // 2. Facebook URL Normalization
    if host.contains("facebook.com") || host.contains("fb.watch") || host.contains("fb.com") {
        if host.starts_with("m.facebook.com") || host.starts_with("web.facebook.com") || host.starts_with("touch.facebook.com") {
            parsed.set_host(Some("www.facebook.com")).ok();
        }

        let path = parsed.path().to_string();

        // Convert /share/r/ID -> /reel/ID/
        // Convert /share/v/ID -> /watch/?v=ID
        if path.starts_with("/share/r/") {
            let id = path.trim_start_matches("/share/r/").trim_matches('/');
            parsed.set_path(&format!("/reel/{}/", id));
        } else if path.starts_with("/share/v/") {
            let id = path.trim_start_matches("/share/v/").trim_matches('/');
            parsed.set_path("/watch/");
            parsed.query_pairs_mut().append_pair("v", id);
        }

        // Strip Facebook tracking parameters
        let filtered_pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .filter(|(k, _)| {
                let key_lower = k.to_lowercase();
                key_lower != "mibextid"
                    && key_lower != "rdid"
                    && key_lower != "share_id"
                    && key_lower != "fbclid"
                    && key_lower != "ref"
                    && key_lower != "sfnsn"
                    && !key_lower.starts_with("utm_")
            })
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        parsed.query_pairs_mut().clear();
        if !filtered_pairs.is_empty() {
            for (k, v) in filtered_pairs {
                parsed.query_pairs_mut().append_pair(&k, &v);
            }
        } else {
            parsed.set_query(None);
        }

        return parsed.to_string();
    }

    // Generic tracking parameter cleaner
    let filtered_pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| {
            let key_lower = k.to_lowercase();
            !key_lower.starts_with("utm_") && key_lower != "fbclid" && key_lower != "gclid"
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    parsed.query_pairs_mut().clear();
    if !filtered_pairs.is_empty() {
        for (k, v) in filtered_pairs {
            parsed.query_pairs_mut().append_pair(&k, &v);
        }
    } else {
        parsed.set_query(None);
    }

    parsed.to_string()
}

/// Returns platform-specific CLI arguments for yt-dlp to bypass bot detection, rate limits,
/// and header restrictions on platforms like Instagram and Facebook.
pub fn get_platform_yt_dlp_args(url: &str) -> Vec<String> {
    let platform = detect_platform(url);
    let mut args = Vec::new();

    // Modern Chrome User-Agent header for desktop browser impersonation
    let desktop_ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
    let accept_lang = "en-US,en;q=0.9";

    match platform {
        Platform::Instagram => {
            args.push("--user-agent".to_string());
            args.push(desktop_ua.to_string());
            args.push("--add-header".to_string());
            args.push(format!("Accept-Language:{}", accept_lang));
            args.push("--add-header".to_string());
            args.push("Referer:https://www.instagram.com/".to_string());
            args.push("--no-check-certificates".to_string());
        }
        Platform::Facebook => {
            args.push("--user-agent".to_string());
            args.push(desktop_ua.to_string());
            args.push("--add-header".to_string());
            args.push(format!("Accept-Language:{}", accept_lang));
            args.push("--add-header".to_string());
            args.push("Referer:https://www.facebook.com/".to_string());
            args.push("--no-check-certificates".to_string());
        }
        Platform::TikTok => {
            args.push("--user-agent".to_string());
            args.push(desktop_ua.to_string());
            args.push("--add-header".to_string());
            args.push("Referer:https://www.tiktok.com/".to_string());
        }
        _ => {
            args.push("--user-agent".to_string());
            args.push(desktop_ua.to_string());
        }
    }

    args
}

/// Returns optimal format selection string for yt-dlp given a platform and user format string.
/// For Instagram & Facebook, ensures progressive stream fallback (`b/best/hd/sd`) so format matching doesn't error.
pub fn get_platform_format_filter(url: &str, requested_filter: Option<&str>) -> String {
    let platform = detect_platform(url);
    let req = requested_filter.unwrap_or("bestvideo+bestaudio/best");

    match platform {
        Platform::Instagram | Platform::Facebook => {
            if req.contains("bestvideo") {
                format!("{}/b/best/hd/sd", req)
            } else {
                req.to_string()
            }
        }
        _ => req.to_string(),
    }
}
