use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};

use crate::models::{AnimeItem, AnimeTitle, DashboardData};

const ANILIST_GRAPHQL_URL: &str = "https://graphql.anilist.co";

pub struct AniListProvider {
    client: Client,
}

impl Default for AniListProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AniListProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .tcp_nodelay(true)
            .pool_max_idle_per_host(10)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    async fn execute_query(&self, query: &str, variables: Value) -> Result<Value> {
        let response = self
            .client
            .post(ANILIST_GRAPHQL_URL)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&json!({
                "query": query,
                "variables": variables
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("AniList API error: status {}", response.status()));
        }

        let json_body: Value = response.json().await?;
        if let Some(errors) = json_body.get("errors") {
            return Err(anyhow!("AniList GraphQL error: {}", errors));
        }

        Ok(json_body)
    }

    pub fn parse_media_item(node: &Value) -> AnimeItem {
        let id = node["id"].as_i64().unwrap_or(0).to_string();
        let mal_id = node["idMal"].as_i64();

        let title = AnimeTitle {
            english: node["title"]["english"].as_str().map(|s| s.to_string()),
            romaji: node["title"]["romaji"].as_str().map(|s| s.to_string()),
            native: node["title"]["native"].as_str().map(|s| s.to_string()),
        };

        let cover_image = node["coverImage"]["large"]
            .as_str()
            .or_else(|| node["coverImage"]["extraLarge"].as_str())
            .map(|s| s.to_string());

        let thumbnail_image = node["coverImage"]["large"]
            .as_str()
            .or_else(|| node["coverImage"]["medium"].as_str())
            .map(|s| s.to_string());

        let banner_image = node["bannerImage"].as_str().map(|s| s.to_string());
        let color = node["coverImage"]["color"].as_str().map(|s| s.to_string());

        let score = node["averageScore"]
            .as_f64()
            .map(|s| (s / 10.0) as f32);

        let episodes = node["episodes"].as_u64().map(|e| e as u32);
        let season = node["season"].as_str().map(|s| s.to_string());
        let season_year = node["seasonYear"].as_i64().map(|y| y as i32);
        let format = node["format"].as_str().map(|s| s.to_string());
        let status = node["status"].as_str().map(|s| s.to_string());

        let genres = node["genres"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|g| g.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let description = node["description"].as_str().map(|s| s.to_string());

        let studio = node["studios"]["nodes"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|st| st["name"].as_str().map(|s| s.to_string()));

        let next_airing_episode = node["nextAiringEpisode"]["episode"].as_u64().map(|e| e as u32);
        let next_airing_at = node["nextAiringEpisode"]["airingAt"].as_i64();

        let relations = node["relations"]["edges"]
            .as_array()
            .map(|edges| {
                edges
                    .iter()
                    .filter_map(|edge| {
                        let rel_type = edge["relationType"].as_str().unwrap_or("RELATED").to_string();
                        let n = &edge["node"];
                        let format = n["format"].as_str().unwrap_or("");
                        if format == "MANGA" || format == "NOVEL" || format == "ONE_SHOT" {
                            return None;
                        }
                        let rel_id = n["id"].as_i64()?.to_string();
                        let title = AnimeTitle {
                            english: n["title"]["english"].as_str().map(|s| s.to_string()),
                            romaji: n["title"]["romaji"].as_str().map(|s| s.to_string()),
                            native: n["title"]["native"].as_str().map(|s| s.to_string()),
                        };
                        let cover_image = n["coverImage"]["large"]
                            .as_str()
                            .or_else(|| n["coverImage"]["medium"].as_str())
                            .map(|s| s.to_string());
                        let episodes = n["episodes"].as_u64().map(|e| e as u32);
                        let season = n["season"].as_str().map(|s| s.to_string());
                        let season_year = n["seasonYear"].as_i64().map(|y| y as i32);

                        Some(crate::models::RelatedAnime {
                            id: rel_id,
                            relation_type: rel_type,
                            title,
                            format: Some(format.to_string()),
                            episodes,
                            season,
                            season_year,
                            cover_image,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        AnimeItem {
            id,
            mal_id,
            title,
            cover_image,
            thumbnail_image,
            banner_image,
            color,
            score,
            episodes,
            season,
            season_year,
            format,
            status,
            genres,
            description,
            studio,
            next_airing_episode,
            next_airing_at,
            relations,
        }
    }

    pub async fn fetch_dashboard(&self, per_page: u32) -> Result<DashboardData> {
        let query = r#"
        query($perPage: Int) {
          trending: Page(page: 1, perPage: $perPage) {
            media(type: ANIME, sort: TRENDING_DESC) {
              id idMal title { english romaji native }
              coverImage { extraLarge large color }
              averageScore season seasonYear format status genres
            }
          }
          popular: Page(page: 1, perPage: $perPage) {
            media(type: ANIME, status: RELEASING, sort: POPULARITY_DESC) {
              id idMal title { english romaji native }
              coverImage { extraLarge large color }
              averageScore season seasonYear format status genres
            }
          }
          upcoming: Page(page: 1, perPage: $perPage) {
            media(type: ANIME, status: NOT_YET_RELEASED, sort: POPULARITY_DESC) {
              id idMal title { english romaji native }
              coverImage { extraLarge large color }
              averageScore season seasonYear format status genres
            }
          }
          top: Page(page: 1, perPage: $perPage) {
            media(type: ANIME, sort: SCORE_DESC) {
              id idMal title { english romaji native }
              coverImage { extraLarge large color }
              averageScore season seasonYear format status genres
            }
          }
        }
        "#;

        let res = self.execute_query(query, json!({ "perPage": per_page })).await?;
        let data = &res["data"];

        let parse_list = |key: &str| -> Vec<AnimeItem> {
            data[key]["media"]
                .as_array()
                .map(|arr| arr.iter().map(Self::parse_media_item).collect())
                .unwrap_or_default()
        };

        Ok(DashboardData {
            trending: parse_list("trending"),
            popular_season: parse_list("popular"),
            upcoming: parse_list("upcoming"),
            top_rated: parse_list("top"),
        })
    }

    pub async fn search(&self, query_str: &str, page: u32, per_page: u32) -> Result<Vec<AnimeItem>> {
        let query = r#"
        query($search: String, $page: Int, $perPage: Int) {
          Page(page: $page, perPage: $perPage) {
            media(type: ANIME, search: $search, sort: SEARCH_MATCH) {
              id idMal title { english romaji native }
              coverImage { extraLarge large color }
              bannerImage averageScore episodes season seasonYear format status genres
              description(asHtml: false)
              studios(isMain: true) { nodes { name } }
              nextAiringEpisode { episode airingAt }
            }
          }
        }
        "#;

        let res = self
            .execute_query(
                query,
                json!({
                    "search": query_str,
                    "page": page,
                    "perPage": per_page
                }),
            )
            .await?;

        let items = res["data"]["Page"]["media"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_media_item).collect())
            .unwrap_or_default();

        Ok(items)
    }

    pub async fn fetch_by_genre(&self, genre: &str, page: u32, per_page: u32) -> Result<Vec<AnimeItem>> {
        let query = r#"
        query($genre: String, $page: Int, $perPage: Int) {
          Page(page: $page, perPage: $perPage) {
            media(type: ANIME, genre: $genre, sort: POPULARITY_DESC) {
              id idMal title { english romaji native }
              coverImage { extraLarge large color }
              bannerImage averageScore episodes season seasonYear format status genres
              description(asHtml: false)
              studios(isMain: true) { nodes { name } }
              nextAiringEpisode { episode airingAt }
            }
          }
        }
        "#;

        let res = self
            .execute_query(
                query,
                json!({
                    "genre": genre,
                    "page": page,
                    "perPage": per_page
                }),
            )
            .await?;

        let items = res["data"]["Page"]["media"]
            .as_array()
            .map(|arr| arr.iter().map(Self::parse_media_item).collect())
            .unwrap_or_default();

        Ok(items)
    }

    pub async fn fetch_details(&self, id: &str) -> Result<Option<AnimeItem>> {
        let query = r#"
        query($id: Int) {
          Media(id: $id, type: ANIME) {
            id idMal title { english romaji native }
            coverImage { extraLarge large color }
            bannerImage averageScore episodes season seasonYear format status genres
            description(asHtml: false)
            studios(isMain: true) { nodes { name } }
            nextAiringEpisode { episode airingAt }
            relations {
              edges {
                relationType
                node {
                  id
                  title { english romaji native }
                  format
                  status
                  episodes
                  season
                  seasonYear
                  coverImage { large medium }
                }
              }
            }
          }
        }
        "#;

        let id_int: i64 = id.parse().unwrap_or(0);
        let res = self.execute_query(query, json!({ "id": id_int })).await?;

        let media = &res["data"]["Media"];
        if media.is_null() {
            Ok(None)
        } else {
            Ok(Some(Self::parse_media_item(media)))
        }
    }
}
