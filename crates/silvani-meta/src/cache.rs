use anyhow::Result;
use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct MetaCache {
    conn: Arc<Mutex<Connection>>,
}

impl MetaCache {
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_tables(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        Self::init_tables(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn init_tables(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS metadata_cache (
                cache_key TEXT PRIMARY KEY,
                payload TEXT NOT NULL,
                cached_at INTEGER NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    pub fn get(&self, key: &str, max_age_secs: u64) -> Option<String> {
        let conn = self.conn.lock().ok()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs() as i64;

        let mut stmt = conn
            .prepare("SELECT payload, cached_at FROM metadata_cache WHERE cache_key = ?")
            .ok()?;

        let mut rows = stmt.query(params![key]).ok()?;
        if let Some(row) = rows.next().ok()? {
            let payload: String = row.get(0).ok()?;
            let cached_at: i64 = row.get(1).ok()?;
            if (now - cached_at) <= max_age_secs as i64 {
                return Some(payload);
            }
        }

        None
    }

    pub fn set(&self, key: &str, payload: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        conn.execute(
            "INSERT OR REPLACE INTO metadata_cache (cache_key, payload, cached_at) VALUES (?, ?, ?)",
            params![key, payload, now],
        )?;

        Ok(())
    }
}
