use fetchr_core::url_normalizer::{
    detect_platform, get_platform_format_filter, get_platform_yt_dlp_args, normalize_url, Platform,
};

#[test]
fn test_instagram_share_reel_normalization() {
    let raw = "https://www.instagram.com/share/reel/C8XYZ12345/?igsh=MWF5NDlwbW0yeDF3";
    let expected = "https://www.instagram.com/reel/C8XYZ12345/";
    assert_eq!(normalize_url(raw), expected);
}

#[test]
fn test_instagram_share_post_normalization() {
    let raw = "https://www.instagram.com/share/p/C8ABC7890/?igsh=MWF5NDlwbW0yeDF3&utm_source=copy";
    let expected = "https://www.instagram.com/p/C8ABC7890/";
    assert_eq!(normalize_url(raw), expected);
}

#[test]
fn test_instagram_reels_plural_path() {
    let raw = "https://instagram.com/reels/D9FFF000/";
    let expected = "https://www.instagram.com/reel/D9FFF000/";
    assert_eq!(normalize_url(raw), expected);
}

#[test]
fn test_instagram_shortlink_domain() {
    let raw = "https://instagr.am/p/C123456789/";
    let expected = "https://www.instagram.com/p/C123456789/";
    assert_eq!(normalize_url(raw), expected);
}

#[test]
fn test_facebook_share_reel_normalization() {
    let raw = "https://www.facebook.com/share/r/9876543210/?mibextid=wwXIfr";
    let expected = "https://www.facebook.com/reel/9876543210/";
    assert_eq!(normalize_url(raw), expected);
}

#[test]
fn test_facebook_share_video_normalization() {
    let raw = "https://www.facebook.com/share/v/1122334455/?mibextid=wwXIfr";
    let expected = "https://www.facebook.com/watch/?v=1122334455";
    assert_eq!(normalize_url(raw), expected);
}

#[test]
fn test_facebook_mobile_domain_normalization() {
    let raw = "https://m.facebook.com/watch/?v=9988776655&fbclid=IwAR123";
    let expected = "https://www.facebook.com/watch/?v=9988776655";
    assert_eq!(normalize_url(raw), expected);
}

#[test]
fn test_generic_url_tracking_removal() {
    let raw = "https://example.com/video?utm_source=twitter&utm_medium=social&v=123";
    let expected = "https://example.com/video?v=123";
    assert_eq!(normalize_url(raw), expected);
}

#[test]
fn test_missing_scheme_handling() {
    let raw = "instagram.com/reel/C9999/";
    let expected = "https://www.instagram.com/reel/C9999/";
    assert_eq!(normalize_url(raw), expected);
}

#[test]
fn test_empty_and_whitespace_url() {
    assert_eq!(normalize_url("   "), "");
    assert_eq!(normalize_url(""), "");
}

#[test]
fn test_platform_detection_all_platforms() {
    assert_eq!(
        detect_platform("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
        Platform::YouTube
    );
    assert_eq!(
        detect_platform("https://youtu.be/dQw4w9WgXcQ"),
        Platform::YouTube
    );
    assert_eq!(
        detect_platform("https://www.instagram.com/reel/C123/"),
        Platform::Instagram
    );
    assert_eq!(
        detect_platform("https://instagr.am/p/C123/"),
        Platform::Instagram
    );
    assert_eq!(
        detect_platform("https://www.facebook.com/reel/999/"),
        Platform::Facebook
    );
    assert_eq!(
        detect_platform("https://fb.watch/xyz123/"),
        Platform::Facebook
    );
    assert_eq!(
        detect_platform("https://www.tiktok.com/@user/video/123"),
        Platform::TikTok
    );
    assert_eq!(
        detect_platform("https://vimeo.com/123456"),
        Platform::Vimeo
    );
    assert_eq!(
        detect_platform("https://other-site.com/video.mp4"),
        Platform::Generic
    );
}

#[test]
fn test_platform_yt_dlp_args_instagram() {
    let args = get_platform_yt_dlp_args("https://www.instagram.com/reel/C123/");
    assert!(args.contains(&"--user-agent".to_string()));
    assert!(args.contains(&"Referer:https://www.instagram.com/".to_string()));
    assert!(args.contains(&"--no-check-certificates".to_string()));
}

#[test]
fn test_platform_yt_dlp_args_facebook() {
    let args = get_platform_yt_dlp_args("https://www.facebook.com/reel/999/");
    assert!(args.contains(&"--user-agent".to_string()));
    assert!(args.contains(&"Referer:https://www.facebook.com/".to_string()));
    assert!(args.contains(&"--no-check-certificates".to_string()));
}

#[test]
fn test_format_filter_fallback_facebook_and_instagram() {
    let ig_filter = get_platform_format_filter(
        "https://www.instagram.com/reel/C123/",
        Some("bestvideo*+bestaudio/best"),
    );
    assert_eq!(ig_filter, "bestvideo*+bestaudio/best/b/best/hd/sd");

    let fb_filter = get_platform_format_filter(
        "https://www.facebook.com/reel/999/",
        Some("bestvideo[height<=1080]+bestaudio/best[height<=1080]"),
    );
    assert_eq!(
        fb_filter,
        "bestvideo[height<=1080]+bestaudio/best[height<=1080]/b/best/hd/sd"
    );

    let yt_filter = get_platform_format_filter(
        "https://www.youtube.com/watch?v=123",
        Some("bestvideo+bestaudio/best"),
    );
    assert_eq!(yt_filter, "bestvideo+bestaudio/best");
}
