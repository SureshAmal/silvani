use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

use crate::models::{AnimeItem, AnimeTitle, DashboardData};

const JIKAN_BASE_URL: &str = "https://api.jikan.moe/v4";

pub struct JikanProvider {
    client: Client,
}

impl Default for JikanProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl JikanProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub fn parse_jikan_item(node: &Value) -> AnimeItem {
        let mal_id = node["mal_id"].as_i64();
        let id = mal_id.map(|id| format!("mal-{}", id)).unwrap_or_default();

        let title = AnimeTitle {
            english: node["title_english"].as_str().map(|s| s.to_string()),
            romaji: node["title"].as_str().map(|s| s.to_string()),
            native: node["title_japanese"].as_str().map(|s| s.to_string()),
        };

        let cover_image = node["images"]["webp"]["large_image_url"]
            .as_str()
            .or_else(|| node["images"]["jpg"]["large_image_url"].as_str())
            .map(|s| s.to_string());

        let thumbnail_image = node["images"]["webp"]["image_url"]
            .as_str()
            .or_else(|| node["images"]["jpg"]["image_url"].as_str())
            .map(|s| s.to_string());

        let score = node["score"].as_f64().map(|s| s as f32);
        let episodes = node["episodes"].as_u64().map(|e| e as u32);
        let season = node["season"].as_str().map(|s| s.to_uppercase());
        let season_year = node["year"].as_i64().map(|y| y as i32);
        let format = node["type"].as_str().map(|s| s.to_string());
        let status = node["status"].as_str().map(|s| s.to_string());

        let genres = node["genres"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|g| g["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let description = node["synopsis"].as_str().map(|s| s.to_string());

        let studio = node["studios"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|st| st["name"].as_str().map(|s| s.to_string()));

        AnimeItem {
            id,
            mal_id,
            title,
            cover_image,
            thumbnail_image,
            banner_image: None,
            color: None,
            score,
            episodes,
            season,
            season_year,
            format,
            status,
            genres,
            description,
            studio,
            next_airing_episode: None,
            next_airing_at: None,
            relations: Vec::new(),
        }
    }

    async fn get_json(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", JIKAN_BASE_URL, path);
        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Jikan API error: status {}", resp.status()));
        }

        let body: Value = resp.json().await?;
        Ok(body)
    }

    pub async fn fetch_top(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let path = format!("/top/anime?limit={}", limit);
        let json = self.get_json(&path).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_jikan_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_season_now(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let path = format!("/seasons/now?limit={}", limit);
        let json = self.get_json(&path).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_jikan_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_upcoming(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let path = format!("/seasons/upcoming?limit={}", limit);
        let json = self.get_json(&path).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_jikan_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<AnimeItem>> {
        let path = format!("/anime?q={}&limit={}", query, limit);
        let json = self.get_json(&path).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_jikan_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_dashboard(&self, limit: u32) -> Result<DashboardData> {
        let top = self.fetch_top(limit).await.unwrap_or_default();
        let season = self.fetch_season_now(limit).await.unwrap_or_default();
        let upcoming = self.fetch_upcoming(limit).await.unwrap_or_default();

        Ok(DashboardData {
            trending: season.clone(),
            popular_season: season,
            upcoming,
            top_rated: top,
        })
    }
}
