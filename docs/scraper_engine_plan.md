# Scraper Architecture & Data Model Plan

High-level architecture and data schemas for multi-provider concurrent anime streaming and local caching.

---

## 1. Domain Structs

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioType {
    Sub,
    Dub,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleTrack {
    pub language: String,
    pub url: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStream {
    pub quality: String, // "1080p", "720p", "360p", "auto"
    pub url: String,     // .m3u8 or .mp4 link
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeSearchResult {
    pub provider: String,
    pub id: String,
    pub title: String,
    pub poster_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub number: f32,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSource {
    pub provider: String,
    pub audio_type: AudioType,
    pub language: String, // "Japanese", "English", "Hindi"
    pub streams: Vec<VideoStream>,
    pub subtitles: Vec<SubtitleTrack>,
    pub referer: String,
}
```

---

## 2. High-Level Engine API

```rust
pub trait AnimeProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn search(&self, query: &str) -> anyhow::Result<Vec<AnimeSearchResult>>;
    async fn episodes(&self, anime_id: &str) -> anyhow::Result<Vec<Episode>>;
    async fn stream(&self, episode_id: &str, audio: AudioType) -> anyhow::Result<StreamSource>;
}
```

### High-Level Consumer Flow
1. `ScraperEngine::search(query)`: Queries all active providers concurrently via `tokio::spawn`, returning aggregate list of matching anime.
2. `ScraperEngine::resolve_episode(anime_name, episode_num)`: Automatically searches all providers, finds matching episode ID, and returns available streaming choices (Sub, Dub, Hindi, resolutions).
3. `ScraperEngine::get_stream(stream_selection)`: Returns `StreamSource` containing playlist URL, referer header, and subtitles for player.

---

## 3. SQLite Embedded Cache Schema

Stores resolved search mappings and stream URLs with timestamp to avoid re-scraping:

```sql
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
```
