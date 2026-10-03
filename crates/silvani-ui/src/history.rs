use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub anime_id: String,
    pub episode_number: f32,
    pub anime_title: String,
    pub episode_title: Option<String>,
    pub cover_image: Option<String>,
    pub watched_at: i64,
    pub completed: bool,
    pub progress_percent: f32,
}

#[derive(Clone)]
pub struct WatchHistoryManager {
    conn: Arc<Mutex<Connection>>,
}

impl Default for WatchHistoryManager {
    fn default() -> Self {
        Self::new().expect("Failed to initialize WatchHistoryManager")
    }
}

impl WatchHistoryManager {
    pub fn new() -> Result<Self> {
        let base_data = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|_| std::env::temp_dir());
        let db_dir = base_data.join("silvani");
        std::fs::create_dir_all(&db_dir)?;
        let db_path = db_dir.join("history.db");

        let conn = Connection::open(&db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS watch_history (
                anime_id TEXT NOT NULL,
                episode_number REAL NOT NULL,
                anime_title TEXT NOT NULL,
                episode_title TEXT,
                cover_image TEXT,
                watched_at INTEGER NOT NULL,
                completed INTEGER NOT NULL DEFAULT 1,
                progress_percent REAL NOT NULL DEFAULT 0.0,
                PRIMARY KEY (anime_id, episode_number)
            );",
            [],
        )?;

        // Migration for progress_percent column
        let _ = conn.execute(
            "ALTER TABLE watch_history ADD COLUMN progress_percent REAL NOT NULL DEFAULT 0.0;",
            [],
        );

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_watch_history_time ON watch_history(watched_at DESC);",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn record_watched(
        &self,
        anime_id: &str,
        episode_number: f32,
        anime_title: &str,
        episode_title: Option<&str>,
        cover_image: Option<&str>,
        progress_percent: f32,
    ) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let completed = if progress_percent >= 85.0 { 1 } else { 0 };

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO watch_history (anime_id, episode_number, anime_title, episode_title, cover_image, watched_at, completed, progress_percent)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(anime_id, episode_number) DO UPDATE SET
                watched_at = excluded.watched_at,
                anime_title = excluded.anime_title,
                episode_title = excluded.episode_title,
                cover_image = excluded.cover_image,
                completed = MAX(watch_history.completed, excluded.completed),
                progress_percent = MAX(watch_history.progress_percent, excluded.progress_percent);",
            params![anime_id, episode_number, anime_title, episode_title, cover_image, now, completed, progress_percent],
        )?;

        Ok(())
    }

    pub fn get_progress(&self, anime_id: &str, episode_number: f32) -> Option<f32> {
        let conn = self.conn.lock().ok()?;
        let mut stmt = conn
            .prepare("SELECT progress_percent FROM watch_history WHERE anime_id = ?1 AND ABS(episode_number - ?2) < 0.05 LIMIT 1")
            .ok()?;

        stmt.query_row(params![anime_id, episode_number], |row| row.get(0)).ok()
    }

    pub fn is_watched(&self, anime_id: &str, episode_number: f32) -> bool {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return false,
        };

        let mut stmt = match conn.prepare(
            "SELECT 1 FROM watch_history WHERE anime_id = ?1 AND ABS(episode_number - ?2) < 0.05 AND completed = 1 LIMIT 1",
        ) {
            Ok(s) => s,
            Err(_) => return false,
        };

        stmt.exists(params![anime_id, episode_number]).unwrap_or(false)
    }

    pub fn get_last_watched_episode(&self, anime_id: &str) -> Option<f32> {
        let conn = self.conn.lock().ok()?;
        let mut stmt = conn
            .prepare("SELECT episode_number FROM watch_history WHERE anime_id = ?1 ORDER BY watched_at DESC LIMIT 1")
            .ok()?;

        stmt.query_row(params![anime_id], |row| row.get(0)).ok()
    }

    pub fn get_recent_history(&self, limit: usize) -> Vec<HistoryItem> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut stmt = match conn.prepare(
            "SELECT anime_id, episode_number, anime_title, episode_title, cover_image, watched_at, completed, progress_percent
             FROM watch_history
             ORDER BY watched_at DESC
             LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(HistoryItem {
                    anime_id: row.get(0)?,
                    episode_number: row.get(1)?,
                    anime_title: row.get(2)?,
                    episode_title: row.get(3)?,
                    cover_image: row.get(4)?,
                    watched_at: row.get(5)?,
                    completed: row.get::<_, i64>(6)? != 0,
                    progress_percent: row.get::<_, f64>(7).unwrap_or(0.0) as f32,
                })
            })
            .ok();

        rows.map(|r| r.filter_map(Result::ok).collect()).unwrap_or_default()
    }

    pub fn clear_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM watch_history", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_history_flow() {
        let manager = WatchHistoryManager::new().expect("Init history");
        manager.clear_history().expect("Clear history");

        assert!(!manager.is_watched("test-anime-1", 1.0));
        assert_eq!(manager.get_progress("test-anime-1", 1.0), None);

        // Record partial progress: 30% -> not yet completed/watched
        manager
            .record_watched("test-anime-1", 1.0, "Test Anime", Some("Pilot"), None, 30.0)
            .expect("Record partial");

        assert!(!manager.is_watched("test-anime-1", 1.0));
        assert_eq!(manager.get_progress("test-anime-1", 1.0), Some(30.0));

        // Record 90% -> completed/watched
        manager
            .record_watched("test-anime-1", 1.0, "Test Anime", Some("Pilot"), None, 90.0)
            .expect("Record completed");

        assert!(manager.is_watched("test-anime-1", 1.0));
        assert_eq!(manager.get_progress("test-anime-1", 1.0), Some(90.0));
        assert!(!manager.is_watched("test-anime-1", 2.0));

        let last_ep = manager.get_last_watched_episode("test-anime-1");
        assert_eq!(last_ep, Some(1.0));

        let recent = manager.get_recent_history(10);
        assert!(!recent.is_empty());
        assert_eq!(recent[0].anime_title, "Test Anime");
        assert_eq!(recent[0].episode_number, 1.0);
        assert_eq!(recent[0].progress_percent, 90.0);
        assert!(recent[0].completed);

        manager.clear_history().expect("Clear history");
        assert!(!manager.is_watched("test-anime-1", 1.0));
    }
}

