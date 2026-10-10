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
        let lim = limit.min(20);
        let endpoint = format!("/trending/anime?limit={}", lim);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_popular(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let lim = limit.min(20);
        let endpoint = format!("/anime?page[limit]={}&sort=-user_count", lim);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_upcoming(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let lim = limit.min(20);
        let endpoint = format!(
            "/anime?page[limit]={}&filter[status]=unreleased&sort=-user_count",
            lim
        );
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_top(&self, limit: u32) -> Result<Vec<AnimeItem>> {
        let lim = limit.min(20);
        let endpoint = format!("/anime?page[limit]={}&sort=-average_rating", lim);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<AnimeItem>> {
        let lim = limit.min(20);
        let encoded = Self::url_encode(query);
        let endpoint = format!("/anime?page[limit]={}&filter[text]={}", lim, encoded);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_by_genre(&self, genre: &str, limit: u32) -> Result<Vec<AnimeItem>> {
        let lim = limit.min(20);
        let encoded = Self::url_encode(&genre.to_lowercase());
        let endpoint = format!("/anime?page[limit]={}&filter[categories]={}", lim, encoded);
        let json = self.get_json(&endpoint).await?;
        let items = json["data"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_kitsu_item).collect())
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn fetch_episode_titles(
        &self,
        id: &str,
    ) -> Result<std::collections::BTreeMap<u32, String>> {
        let id: u64 = id.strip_prefix("kitsu-").unwrap_or(id).parse()?;
        let mut offset = 0;
        let mut titles = std::collections::BTreeMap::new();
        loop {
            let response = self
                .get_json(&format!(
                    "/anime/{id}/episodes?page[limit]=20&page[offset]={offset}"
                ))
                .await?;
            let count = response["data"].as_array().map_or(0, Vec::len);
            titles.extend(parse_episode_titles(&response));
            if count == 0 || response["links"]["next"].as_str().is_none() {
                break;
            }
            offset += count;
        }
        Ok(titles)
    }

    pub async fn resolve_mal_id(&self, id: &str) -> Result<Option<i64>> {
        let id: u64 = id.strip_prefix("kitsu-").unwrap_or(id).parse()?;
        let json = self
            .get_json(&format!("/anime/{id}/mappings?page[limit]=20"))
            .await?;
        Ok(json["data"].as_array().and_then(|mappings| {
            mappings.iter().find_map(|mapping| {
                let attributes = &mapping["attributes"];
                if attributes["externalSite"].as_str() != Some("myanimelist/anime") {
                    return None;
                }
                attributes["externalId"]
                    .as_str()?
                    .parse::<i64>()
                    .ok()
                    .filter(|id| *id > 0)
            })
        }))
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

fn parse_episode_titles(response: &Value) -> std::collections::BTreeMap<u32, String> {
    response["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|episode| {
            let attributes = &episode["attributes"];
            let number = u32::try_from(attributes["number"].as_u64()?).ok()?;
            let placeholder = format!("Episode {number}");
            let title = [
                &attributes["titles"]["en_us"],
                &attributes["titles"]["en"],
                &attributes["canonicalTitle"],
            ]
            .into_iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .find(|title| !title.is_empty() && !title.eq_ignore_ascii_case(&placeholder))?;
            Some((number, title.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod episode_titles_tests {
    #[test]
    fn parses_native_episode_names_and_numbers() {
        let response = serde_json::json!({"data": [
            {"attributes": {"number": 1, "canonicalTitle": "Asteroid Blues", "titles": {"en": "Asteroid Blues"}}},
            {"attributes": {"number": 2, "canonicalTitle": "Stray Dog Strut"}},
            {"attributes": {"number": 3, "canonicalTitle": "Episode 3"}},
            {"attributes": {"number": 4, "canonicalTitle": "Gateway Shuffle", "titles": {"en_us": "  ", "en": "Episode 4"}}},
        ]});
        let titles = super::parse_episode_titles(&response);
        assert_eq!(titles[&1], "Asteroid Blues");
        assert_eq!(titles[&2], "Stray Dog Strut");
        assert!(!titles.contains_key(&3));
        assert_eq!(titles[&4], "Gateway Shuffle");
    }
}
