use chrono::Utc;
use fetchr_core::db::{DbManager, HistoryItem, Task, TaskStatus, TaskType};
use fetchr_core::presets::get_default_presets;

#[test]
fn test_db_manager_crud_operations() {
    let db_path = std::env::temp_dir().join("fetchr_test_crud.db");
    std::fs::remove_file(&db_path).ok();

    let db = DbManager::new(db_path.clone());
    db.initialize().expect("Failed to initialize DB");

    // Save task
    let task = Task {
        id: "test-task-1".to_string(),
        task_type: TaskType::DownloadVideo {
            format_preset: "mp4_1080p".to_string(),
        },
        url: "https://www.instagram.com/reel/C123/".to_string(),
        status: TaskStatus::Pending,
        progress: 0.0,
        speed: None,
        eta: None,
        file_path: None,
        error_msg: None,
        retry_count: 0,
        created_at: Utc::now(),
    };

    db.save_task(&task).expect("Failed to save task");

    let loaded = db.load_all_tasks().expect("Failed to load tasks");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, "test-task-1");
    assert_eq!(loaded[0].url, "https://www.instagram.com/reel/C123/");

    // Update setting
    db.save_setting("download_directory", "/tmp/downloads")
        .expect("Failed to save setting");
    let setting = db.get_setting("download_directory").unwrap();
    assert_eq!(setting, Some("/tmp/downloads".to_string()));

    // Delete task
    db.delete_task("test-task-1")
        .expect("Failed to delete task");
    let after_delete = db.load_all_tasks().unwrap();
    assert_eq!(after_delete.len(), 0);

    std::fs::remove_file(&db_path).ok();
}

#[test]
fn test_history_crud_and_search() {
    let db_path = std::env::temp_dir().join("fetchr_test_history.db");
    std::fs::remove_file(&db_path).ok();

    let db = DbManager::new(db_path.clone());
    db.initialize().expect("Failed to initialize DB");

    let history_item1 = HistoryItem {
        id: "hist-1".to_string(),
        title: "Instagram Viral Reel".to_string(),
        url: "https://www.instagram.com/reel/C123/".to_string(),
        file_path: "/downloads/ig_reel.mp4".to_string(),
        file_size: 10485760,
        duration: 30,
        thumbnail_path: None,
        resolution: Some("1080p".to_string()),
        source_site: Some("instagram".to_string()),
        download_duration_secs: 4,
        completed_at: Utc::now(),
    };

    let history_item2 = HistoryItem {
        id: "hist-2".to_string(),
        title: "Facebook Funny Video".to_string(),
        url: "https://www.facebook.com/watch/?v=999".to_string(),
        file_path: "/downloads/fb_video.mp4".to_string(),
        file_size: 20971520,
        duration: 60,
        thumbnail_path: None,
        resolution: Some("720p".to_string()),
        source_site: Some("facebook".to_string()),
        download_duration_secs: 8,
        completed_at: Utc::now(),
    };

    db.add_to_history(&history_item1).unwrap();
    db.add_to_history(&history_item2).unwrap();

    let all_history = db.load_history(None).unwrap();
    assert_eq!(all_history.len(), 2);

    let search_ig = db.load_history(Some("Instagram")).unwrap();
    assert_eq!(search_ig.len(), 1);
    assert_eq!(search_ig[0].title, "Instagram Viral Reel");

    let search_fb = db.load_history(Some("Facebook")).unwrap();
    assert_eq!(search_fb.len(), 1);
    assert_eq!(search_fb[0].title, "Facebook Funny Video");

    std::fs::remove_file(&db_path).ok();
}

#[test]
fn test_default_presets_validity() {
    let presets = get_default_presets();
    assert!(!presets.is_empty());
    for p in presets {
        assert!(!p.id.is_empty());
        assert!(!p.name.is_empty());
        assert!(!p.format_filter.is_empty());
    }
}
