use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Result, ScraperError};
use crate::models::{AnimeSearchResult, AudioType, StreamSource};

#[derive(Clone)]
pub struct CacheStore {
    conn: Arc<Mutex<Connection>>,
}

impl CacheStore {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| ScraperError::Network(e.to_string()))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| ScraperError::Network(e.to_string()))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS search_cache (
                query TEXT NOT NULL,
                provider TEXT NOT NULL,
                results_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (query, provider)
            );
            CREATE TABLE IF NOT EXISTS stream_cache (
                provider TEXT NOT NULL,
                episode_id TEXT NOT NULL,
                audio_type TEXT NOT NULL,
                stream_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (provider, episode_id, audio_type)
            );
            "#,
        )
        .map_err(|e| ScraperError::Parse(e.to_string()))?;
        Ok(())
    }

    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    pub fn get_search(&self, query: &str, provider: &str, ttl_secs: u64) -> Option<Vec<AnimeSearchResult>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT results_json, created_at FROM search_cache WHERE query = ?1 AND provider = ?2")
            .ok()?;

        let (json, created_at): (String, i64) = stmt
            .query_row(params![query, provider], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok()?;

        if (Self::now() - created_at) > (ttl_secs as i64) {
            return None;
        }

        serde_json::from_str(&json).ok()
    }

    pub fn set_search(&self, query: &str, provider: &str, results: &[AnimeSearchResult]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(results).map_err(|e| ScraperError::Parse(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO search_cache (query, provider, results_json, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![query, provider, json, Self::now()],
        )
        .map_err(|e| ScraperError::Parse(e.to_string()))?;
        Ok(())
    }

    pub fn get_stream(&self, provider: &str, episode_id: &str, audio: &AudioType, ttl_secs: u64) -> Option<StreamSource> {
        let conn = self.conn.lock().unwrap();
        let audio_str = match audio {
            AudioType::Sub => "sub",
            AudioType::Dub => "dub",
        };
        let mut stmt = conn
            .prepare("SELECT stream_json, created_at FROM stream_cache WHERE provider = ?1 AND episode_id = ?2 AND audio_type = ?3")
            .ok()?;

        let (json, created_at): (String, i64) = stmt
            .query_row(params![provider, episode_id, audio_str], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .ok()?;

        if (Self::now() - created_at) > (ttl_secs as i64) {
            return None;
        }

        serde_json::from_str(&json).ok()
    }

    pub fn set_stream(&self, provider: &str, episode_id: &str, audio: &AudioType, stream: &StreamSource) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let audio_str = match audio {
            AudioType::Sub => "sub",
            AudioType::Dub => "dub",
        };
        let json = serde_json::to_string(stream).map_err(|e| ScraperError::Parse(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO stream_cache (provider, episode_id, audio_type, stream_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![provider, episode_id, audio_str, json, Self::now()],
        )
        .map_err(|e| ScraperError::Parse(e.to_string()))?;
        Ok(())
    }
}
