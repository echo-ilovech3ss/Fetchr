use std::path::{Path, PathBuf};
use rusqlite::{params, Connection, Result};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed,
    Interrupted,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "Pending",
            TaskStatus::Downloading => "Downloading",
            TaskStatus::Paused => "Paused",
            TaskStatus::Completed => "Completed",
            TaskStatus::Failed => "Failed",
            TaskStatus::Interrupted => "Interrupted",
        }
    }
}

impl From<&str> for TaskStatus {
    fn from(s: &str) -> Self {
        match s {
            "Downloading" => TaskStatus::Downloading,
            "Paused" => TaskStatus::Paused,
            "Completed" => TaskStatus::Completed,
            "Failed" => TaskStatus::Failed,
            "Interrupted" => TaskStatus::Interrupted,
            _ => TaskStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum TaskType {
    DownloadVideo { format_preset: String },
    DownloadAudio { audio_format: String },
    FetchMetadataOnly,
    ExtractThumbnail,
    ExtractSubtitles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub task_type: TaskType,
    pub url: String,
    pub status: TaskStatus,
    pub progress: f64,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub file_path: Option<String>,
    pub error_msg: Option<String>,
    pub retry_count: usize,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: String,
    pub title: String,
    pub url: String,
    pub file_path: String,
    pub file_size: i64,
    pub duration: i64,
    pub thumbnail_path: Option<String>,
    pub resolution: Option<String>,
    pub source_site: Option<String>,
    pub download_duration_secs: i64,
    pub completed_at: DateTime<Utc>,
}

pub struct DbManager {
    db_path: PathBuf,
}

impl DbManager {
    /// Creates a new Database Manager at the specified file path.
    pub fn new(path: PathBuf) -> Self {
        Self { db_path: path }
    }

    /// Open a connection to the database.
    fn connect(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        // Enable Write-Ahead Logging (WAL) for better concurrency and crash-resilience
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(conn)
    }

    /// Initializes all database schemas and tables.
    pub fn initialize(&self) -> Result<()> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = self.connect()?;

        // 1. Settings Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        // 2. Download Queue Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS download_queue (
                id TEXT PRIMARY KEY,
                task_type TEXT NOT NULL,
                url TEXT NOT NULL,
                status TEXT NOT NULL,
                progress REAL NOT NULL,
                speed TEXT,
                eta TEXT,
                file_path TEXT,
                error_msg TEXT,
                retry_count INTEGER NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        // Index queue items by status for faster querying
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_queue_status ON download_queue (status)",
            [],
        )?;

        // 3. Download History Table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS download_history (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                duration INTEGER NOT NULL,
                thumbnail_path TEXT,
                resolution TEXT,
                source_site TEXT,
                download_duration_secs INTEGER NOT NULL,
                completed_at TEXT NOT NULL
            )",
            [],
        )?;

        // Index history for faster searches
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_completed ON download_history (completed_at DESC)",
            [],
        )?;

        // Clean up unfinished/crashed tasks from last session:
        // Set all active "Downloading" or "Pending" items back to "Interrupted"
        // so that they can be safely recovered or resumed.
        conn.execute(
            "UPDATE download_queue 
             SET status = 'Interrupted', speed = NULL, eta = NULL 
             WHERE status = 'Downloading'",
            [],
        )?;

        Ok(())
    }

    /// Validate database integrity.
    pub fn verify_integrity(&self) -> bool {
        match self.connect() {
            Ok(conn) => {
                let check: Result<String> = conn.query_row(
                    "PRAGMA integrity_check",
                    [],
                    |row| row.get(0)
                );
                match check {
                    Ok(res) => res == "ok",
                    Err(_) => false
                }
            }
            Err(_) => false
        }
    }

    // ==========================================
    // Settings API
    // ==========================================

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            let val: String = row.get(0)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    pub fn save_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    // ==========================================
    // Queue / Task Orchestration API
    // ==========================================

    pub fn save_task(&self, task: &Task) -> Result<()> {
        let conn = self.connect()?;
        let task_type_json = serde_json::to_string(&task.task_type).unwrap_or_default();
        let created_at_str = task.created_at.to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO download_queue (
                id, task_type, url, status, progress, speed, eta, file_path, error_msg, retry_count, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                task.id,
                task_type_json,
                task.url,
                task.status.as_str(),
                task.progress,
                task.speed,
                task.eta,
                task.file_path,
                task.error_msg,
                task.retry_count as i64,
                created_at_str
            ],
        )?;
        Ok(())
    }

    pub fn load_all_tasks(&self) -> Result<Vec<Task>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, task_type, url, status, progress, speed, eta, file_path, error_msg, retry_count, created_at 
             FROM download_queue 
             ORDER BY created_at ASC"
        )?;

        let task_iter = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let task_type_str: String = row.get(1)?;
            let url: String = row.get(2)?;
            let status_str: String = row.get(3)?;
            let progress: f64 = row.get(4)?;
            let speed: Option<String> = row.get(5)?;
            let eta: Option<String> = row.get(6)?;
            let file_path: Option<String> = row.get(7)?;
            let error_msg: Option<String> = row.get(8)?;
            let retry_count_i64: i64 = row.get(9)?;
            let created_at_str: String = row.get(10)?;

            let task_type: TaskType = serde_json::from_str(&task_type_str).unwrap_or(TaskType::FetchMetadataOnly);
            let status = TaskStatus::from(status_str.as_str());
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Task {
                id,
                task_type,
                url,
                status,
                progress,
                speed,
                eta,
                file_path,
                error_msg,
                retry_count: retry_count_i64 as usize,
                created_at,
            })
        })?;

        let mut tasks = Vec::new();
        for task in task_iter {
            tasks.push(task?);
        }
        Ok(tasks)
    }

    pub fn delete_task(&self, id: &str) -> Result<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM download_queue WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_queue(&self) -> Result<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM download_queue WHERE status = 'Completed' OR status = 'Failed' OR status = 'Interrupted'", [])?;
        Ok(())
    }

    // ==========================================
    // Download History API
    // ==========================================

    pub fn add_to_history(&self, item: &HistoryItem) -> Result<()> {
        let conn = self.connect()?;
        let completed_at_str = item.completed_at.to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO download_history (
                id, title, url, file_path, file_size, duration, thumbnail_path, resolution, source_site, download_duration_secs, completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                item.id,
                item.title,
                item.url,
                item.file_path,
                item.file_size,
                item.duration,
                item.thumbnail_path,
                item.resolution,
                item.source_site,
                item.download_duration_secs,
                completed_at_str
            ],
        )?;
        Ok(())
    }

    pub fn load_history(&self, search_query: Option<&str>) -> Result<Vec<HistoryItem>> {
        let conn = self.connect()?;
        
        let mut query = "SELECT id, title, url, file_path, file_size, duration, thumbnail_path, resolution, source_site, download_duration_secs, completed_at 
                         FROM download_history".to_string();
        
        let has_search = search_query.is_some() && !search_query.unwrap().trim().is_empty();
        if has_search {
            query.push_str(" WHERE title LIKE ?1 OR url LIKE ?1 OR source_site LIKE ?1");
        }
        query.push_str(" ORDER BY completed_at DESC");

        let mut stmt = conn.prepare(&query)?;
        
        let mapper = |row: &rusqlite::Row| self.row_to_history_item(row);

        let mut history = Vec::new();
        if has_search {
            let wild_search = format!("%{}%", search_query.unwrap().trim());
            let rows = stmt.query_map(params![wild_search], mapper)?;
            for item in rows {
                history.push(item?);
            }
        } else {
            let rows = stmt.query_map([], mapper)?;
            for item in rows {
                history.push(item?);
            }
        }
        Ok(history)
    }

    fn row_to_history_item(&self, row: &rusqlite::Row) -> Result<HistoryItem> {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let url: String = row.get(2)?;
        let file_path: String = row.get(3)?;
        let file_size: i64 = row.get(4)?;
        let duration: i64 = row.get(5)?;
        let thumbnail_path: Option<String> = row.get(6)?;
        let resolution: Option<String> = row.get(7)?;
        let source_site: Option<String> = row.get(8)?;
        let download_duration_secs: i64 = row.get(9)?;
        let completed_at_str: String = row.get(10)?;

        let completed_at = DateTime::parse_from_rfc3339(&completed_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(HistoryItem {
            id,
            title,
            url,
            file_path,
            file_size,
            duration,
            thumbnail_path,
            resolution,
            source_site,
            download_duration_secs,
            completed_at,
        })
    }

    pub fn delete_history_item(&self, id: &str) -> Result<()> {
        let conn = self.connect()?;
        
        // Retrieve file_path first so we can remove the file from disk
        let mut stmt = conn.prepare("SELECT file_path FROM download_history WHERE id = ?1")?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            let file_path: String = row.get(0)?;
            let path = std::path::Path::new(&file_path);
            if path.exists() && path.is_file() {
                let _ = std::fs::remove_file(path);
            }
        }
        
        conn.execute("DELETE FROM download_history WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM download_history", [])?;
        Ok(())
    }
}
