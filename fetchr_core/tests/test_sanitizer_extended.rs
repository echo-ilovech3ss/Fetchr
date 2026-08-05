use fetchr_core::sanitizer::{is_path_safe, resolve_path_collision, sanitize_filename};

#[test]
fn test_sanitize_filename_emojis_and_symbols() {
    let raw = "🔥 CRAZY VIDEO! (2026) #trending @user & more %100";
    let sanitized = sanitize_filename(raw);
    assert_eq!(sanitized, "CRAZY VIDEO_ (2026) numtrending atuser and more pct100");
}

#[test]
fn test_sanitize_filename_windows_reserved() {
    assert_eq!(sanitize_filename("CON"), "fetchr_CON");
    assert_eq!(sanitize_filename("aux.txt"), "fetchr_aux.txt");
    assert_eq!(sanitize_filename("PRN"), "fetchr_PRN");
    assert_eq!(sanitize_filename("NUL"), "fetchr_NUL");
    assert_eq!(sanitize_filename("COM1"), "fetchr_COM1");
    assert_eq!(sanitize_filename("LPT1"), "fetchr_LPT1");
}

#[test]
fn test_sanitize_filename_length_truncation() {
    let long_name = "a".repeat(300);
    let sanitized = sanitize_filename(&long_name);
    assert!(sanitized.len() <= 200);
}

#[test]
fn test_resolve_path_collision_new_file() {
    let dir = std::env::temp_dir().join("fetchr_collision_test_1");
    std::fs::create_dir_all(&dir).ok();

    let path = resolve_path_collision(&dir, "my_video", "mp4");
    assert_eq!(path, dir.join("my_video.mp4"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_resolve_path_collision_duplicate_files() {
    let dir = std::env::temp_dir().join("fetchr_collision_test_2");
    std::fs::create_dir_all(&dir).ok();

    let first = dir.join("cool_clip.mp4");
    std::fs::write(&first, b"dummy content").unwrap();

    let path2 = resolve_path_collision(&dir, "cool_clip", "mp4");
    assert_eq!(path2, dir.join("cool_clip (1).mp4"));

    std::fs::write(&path2, b"dummy content 2").unwrap();

    let path3 = resolve_path_collision(&dir, "cool_clip", "mp4");
    assert_eq!(path3, dir.join("cool_clip (2).mp4"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_is_path_safe_valid_and_traversal() {
    let base = std::env::temp_dir().join("fetchr_safe_base");
    std::fs::create_dir_all(&base).ok();

    let valid_target = base.join("subdir").join("file.mp4");
    assert!(is_path_safe(&base, &valid_target));

    let unsafe_target = base.join("..").join("secret.txt");
    assert!(!is_path_safe(&base, &unsafe_target));

    std::fs::remove_dir_all(&base).ok();
}
