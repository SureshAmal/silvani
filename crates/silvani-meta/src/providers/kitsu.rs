use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

use crate::models::{AnimeItem, AnimeTitle, DashboardData};

const KITSU_BASE_URL: &str = "https://kitsu.io/api/edge";

pub struct KitsuProvider {
    client: Client,
}

impl Default for KitsuProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KitsuProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .tcp_nodelay(true)
            .pool_max_idle_per_host(10)
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub fn parse_kitsu_item(node: &Value) -> AnimeItem {
        let raw_id = node["id"].as_str().unwrap_or("0");
        let id = format!("kitsu-{}", raw_id);

        let attrs = &node["attributes"];
        let titles = &attrs["titles"];

        let english = titles["en"]
            .as_str()
            .or_else(|| attrs["canonicalTitle"].as_str())
            .map(|s| s.to_string());
        let romaji = titles["en_jp"].as_str().map(|s| s.to_string());
        let native = titles["ja_jp"].as_str().map(|s| s.to_string());

        let title = AnimeTitle {
            english,
            romaji,
            native,
        };

        let poster = &attrs["posterImage"];
        let cover_image = poster["medium"]
            .as_str()
            .or_else(|| poster["large"].as_str())
            .or_else(|| poster["small"].as_str())
            .map(|s| s.to_string());

        let thumbnail_image = poster["small"]
            .as_str()
            .or_else(|| poster["tiny"].as_str())
            .map(|s| s.to_string());

        let banner = &attrs["coverImage"];
        let banner_image = banner["large"]
            .as_str()
            .or_else(|| banner["original"].as_str())
            .map(|s| s.to_string());

        let score = attrs["averageRating"]
            .as_str()
            .and_then(|r| r.parse::<f32>().ok())
            .map(|r| r / 10.0);

        let episodes = attrs["episodeCount"].as_u64().map(|e| e as u32);
        let season_year = attrs["startDate"]
            .as_str()
            .and_then(|d| d.get(..4))
            .and_then(|y| y.parse::<i32>().ok());

        let format = attrs["subtype"].as_str().map(|s| s.to_uppercase());
        let status = attrs["status"].as_str().map(|s| s.to_uppercase());
        let description = attrs["synopsis"]
            .as_str()
            .or_else(|| attrs["description"].as_str())
            .map(|s| s.to_string());

        AnimeItem {
            id,
            mal_id: None,
            title,
            cover_image,
            thumbnail_image,
            banner_image,
            color: None,
            score,
            episodes,
            season: None,
            season_year,
            format,
            status,
            genres: Vec::new(),
            description,
            studio: None,
            next_airing_episode: None,
            next_airing_at: None,
            relations: Vec::new(),
        }
    }

    async fn get_json(&self, endpoint: &str) -> Result<Value> {
        let url = format!("{}{}", KITSU_BASE_URL, endpoint);
        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.api+json")
            .header("Content-Type", "application/vnd.api+json")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Kitsu API error: status {}", resp.status()));
        }

        let body: Value = resp.json().await?;
        Ok(body)
    }

    fn url_encode(input: &str) -> String {
        let mut encoded = String::new();
        for byte in input.bytes() {
            match byte {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(byte as char);
                }
                _ => {
                    encoded.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        encoded
    }

    pub async fn fetch_trending(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let endpoint = format!("/trending/anime?limit={}", limit);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_popular(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let endpoint = format!("/anime?page[limit]={}&sort=-user_count", limit);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_upcoming(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let endpoint = format!(
            "/anime?page[limit]={}&filter[status]=unreleased&sort=-user_count",
            limit
        );
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_top(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let endpoint = format!("/anime?page[limit]={}&sort=-average_rating", limit);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<AnimeItem>> {
        let encoded = Self::url_encode(query);
        let endpoint = format!("/anime?page[limit]={}&filter[text]={}", limit, encoded);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_by_genre(&self, genre: &str, limit: u32) -> Result<Vec<AnimeItem>> {
        let encoded = Self::url_encode(&genre.to_lowercase());
        let endpoint = format!("/anime?page[limit]={}&filter[categories]={}", limit, encoded);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_details(&self, id: &str) -> Result<Option<AnimeItem>> {
        let kitsu_id = id.strip_prefix("kitsu-").unwrap_or(id);
        let endpoint = format!("/anime/{}", kitsu_id);
        let json = self.get_json(&endpoint).await?;
        if let Some(data) = json.get("data") {
            Ok(Some(Self::parse_kitsu_item(data)))
        } else {
            Ok(None)
        }
    }

    pub async fn fetch_dashboard(&self, limit: u32) -> Result<DashboardData> {
        let (trending_res, popular_res, upcoming_res, top_res) = tokio::join!(
            self.fetch_trending(limit),
            self.fetch_popular(limit),
            self.fetch_upcoming(limit),
            self.fetch_top(limit)
        );

        let trending = trending_res.unwrap_or_default();
        let popular_season = popular_res.unwrap_or_default();
        let upcoming = upcoming_res.unwrap_or_default();
        let top_rated = top_res.unwrap_or_default();

        if trending.is_empty() && popular_season.is_empty() {
            return Err(anyhow!("Failed to fetch dashboard from Kitsu"));
        }

        Ok(DashboardData {
            trending,
            popular_season,
            upcoming,
            top_rated,
        })
    }
}
