use fetchr_core::capabilities::get_capabilities;
use fetchr_core::yt_dlp::{BinManager, YtDlpEngine};

#[tokio::test]
async fn test_instagram_capabilities_detection() {
    let caps = get_capabilities("https://www.instagram.com/reel/C123456789/");
    assert_eq!(caps.platform_name, "Instagram");
    assert!(caps.supports_audio);
    assert!(caps.supports_login);
    assert!(!caps.supports_playlists);
}

#[tokio::test]
async fn test_facebook_capabilities_detection() {
    let caps = get_capabilities("https://www.facebook.com/reel/987654321/");
    assert_eq!(caps.platform_name, "Facebook");
    assert!(caps.supports_audio);
    assert!(caps.supports_login);
    assert!(!caps.supports_playlists);
}

#[tokio::test]
async fn test_instagram_oembed_fallback_metadata() {
    let temp_dir = std::env::temp_dir().join("fetchr_tests_ig");
    let bin_manager = BinManager::new(temp_dir);
    let engine = YtDlpEngine::new(bin_manager, None);

    let url = "https://www.instagram.com/p/C123456789/";
    let meta = engine.extract_metadata_oembed(url).await;

    assert!(meta.is_ok(), "Instagram metadata extraction fallback failed");
    let metadata = meta.unwrap();
    assert_eq!(metadata.extractor, "instagram");
    assert!(!metadata.title.is_empty());
    assert!(!metadata.formats.is_empty());
}

#[tokio::test]
async fn test_facebook_oembed_fallback_metadata() {
    let temp_dir = std::env::temp_dir().join("fetchr_tests_fb");
    let bin_manager = BinManager::new(temp_dir);
    let engine = YtDlpEngine::new(bin_manager, None);

    let url = "https://www.facebook.com/watch/?v=1234567890";
    let meta = engine.extract_metadata_oembed(url).await;

    assert!(meta.is_ok(), "Facebook metadata extraction fallback failed");
    let metadata = meta.unwrap();
    assert_eq!(metadata.extractor, "facebook");
    assert!(!metadata.title.is_empty());
    assert!(!metadata.formats.is_empty());
}
