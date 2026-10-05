use serde::Deserialize;

use crate::error::{Result, ScraperError};
use crate::http::ScraperHttpClient;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource, VideoStream};
use crate::provider::{AnimeProvider, BoxFuture, ProviderMetadata, ProviderPriority};

const GRAPHQL_ENDPOINT: &str = "https://api.allanime.day/api";
const REFERER: &str = "https://allanime.to";
const ORIGIN: &str = "https://allanime.to";

#[derive(Deserialize)]
#[allow(dead_code)]
struct GraphQLData<T> {
    data: Option<T>,
    errors: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct ShowsContainer {
    shows: Option<ShowsEdges>,
}

#[derive(Deserialize)]
struct ShowsEdges {
    edges: Option<Vec<ShowEdge>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ShowEdge {
    _id: String,
    name: String,
    thumbnail: Option<String>,
    #[serde(rename = "availableEpisodesDetail")]
    available_episodes_detail: Option<AvailableEpisodesDetail>,
}

#[derive(Deserialize)]
struct AvailableEpisodesDetail {
    #[serde(default)]
    sub: Vec<String>,
    #[serde(default)]
    dub: Vec<String>,
}

pub struct AllAnimeProvider {
    client: ScraperHttpClient,
}

impl Default for AllAnimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AllAnimeProvider {
    pub fn new() -> Self {
        Self {
            client: ScraperHttpClient::new(),
        }
    }
}

impl AnimeProvider for AllAnimeProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            id: "allanime",
            name: "allanime",
            display_name: "AllAnime",
            description: "High-speed GraphQL catalog with extensive Sub & Dub catalog",
            supported_audio: vec![AudioType::Sub, AudioType::Dub],
            priority: ProviderPriority::High,
            is_official: false,
            primary_domain: "allanime.day",
            mirror_domains: vec!["allanime.day", "allanime.to", "api.allanime.day"],
        }
    }

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>> {
        Box::pin(async move {
            let gql_query = r#"query($search:SearchInput,$limit:Int,$page:Int){shows(search:$search,limit:$limit,page:$page){edges{_id,name,thumbnail,availableEpisodesDetail}}}"#;
            let variables = serde_json::json!({
                "search": {
                    "query": query
                },
                "limit": 20,
                "page": 1
            });

            let resp: GraphQLData<ShowsContainer> = self
                .client
                .post_graphql(
                    GRAPHQL_ENDPOINT,
                    gql_query,
                    Some(variables),
                    Some(REFERER),
                    Some(ORIGIN),
                )
                .await?;

            let mut results = Vec::new();
            if let Some(container) = resp.data
                && let Some(shows) = container.shows
                && let Some(edges) = shows.edges
            {
                for edge in edges {
                    results.push(AnimeSearchResult {
                        provider: self.name().to_string(),
                        id: edge._id,
                        title: edge.name,
                        poster_url: edge.thumbnail,
                    });
                }
            }

            Ok(results)
        })
    }

    fn episodes<'a>(&'a self, anime_id: &'a str) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async move {
            let gql_query = r#"query($showId:String!){show(_id:$showId){_id,name,availableEpisodesDetail}}"#;
            let variables = serde_json::json!({
                "showId": anime_id
            });

            #[derive(Deserialize)]
            struct ShowData {
                show: Option<ShowDetail>,
            }

            #[derive(Deserialize)]
            struct ShowDetail {
                #[serde(rename = "availableEpisodesDetail")]
                available_episodes_detail: Option<AvailableEpisodesDetail>,
            }

            let resp: GraphQLData<ShowData> = self
                .client
                .post_graphql(
                    GRAPHQL_ENDPOINT,
                    gql_query,
                    Some(variables),
                    Some(REFERER),
                    Some(ORIGIN),
                )
                .await?;

            let mut episodes = Vec::new();
            if let Some(data) = resp.data
                && let Some(show) = data.show
                && let Some(details) = show.available_episodes_detail
            {
                let mut ep_numbers: Vec<f32> = details
                    .sub
                    .iter()
                    .chain(details.dub.iter())
                    .filter_map(|s| s.parse::<f32>().ok())
                    .collect();

                ep_numbers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                ep_numbers.dedup();

                for num in ep_numbers {
                    episodes.push(Episode {
                        id: format!("{anime_id}::{}", num),
                        number: num,
                        title: Some(format!("Episode {num}")),
                    });
                }
            }

            Ok(episodes)
        })
    }

    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>> {
        Box::pin(async move {
            let parts: Vec<&str> = episode_id.split("::").collect();
            let show_id = parts.first().copied().unwrap_or(episode_id);
            let ep_num = parts.get(1).copied().unwrap_or("1");

            let trans_type = match audio {
                AudioType::Dub => "dub",
                _ => "sub",
            };

            let gql_query = r#"query($showId:String!,$translationType:VaildTranslationTypeEnumType!,$episodeString:String!){episode(showId:$showId,translationType:$translationType,episodeString:$episodeString){episodeString,sourceUrls}}"#;
            let variables = serde_json::json!({
                "showId": show_id,
                "translationType": trans_type,
                "episodeString": ep_num
            });

            #[derive(Deserialize)]
            struct EpStreamData {
                episode: Option<EpStreamInfo>,
            }

            #[derive(Deserialize)]
            struct EpStreamInfo {
                #[serde(rename = "sourceUrls")]
                source_urls: Option<Vec<serde_json::Value>>,
            }

            let resp: Result<GraphQLData<EpStreamData>> = self
                .client
                .post_graphql(
                    GRAPHQL_ENDPOINT,
                    gql_query,
                    Some(variables),
                    Some(REFERER),
                    Some(ORIGIN),
                )
                .await;

            match resp {
                Ok(data) => {
                    if let Some(ep_data) = data.data
                        && let Some(ep) = ep_data.episode
                        && let Some(sources) = ep.source_urls
                    {
                        for s in sources {
                            let source_url = match s {
                                serde_json::Value::String(u) => u,
                                serde_json::Value::Object(map) => {
                                    map.get("sourceUrl").and_then(|v| v.as_str()).unwrap_or_default().to_string()
                                }
                                _ => continue,
                            };

                            let decoded_url = if source_url.starts_with("--") {
                                let hex_part = &source_url[2..];
                                let mut bytes = Vec::new();
                                let mut i = 0;
                                while i + 1 < hex_part.len() {
                                    if let Ok(b) = u8::from_str_radix(&hex_part[i..i + 2], 16) {
                                        bytes.push(b ^ 56); // Hex xor shift
                                    }
                                    i += 2;
                                }
                                String::from_utf8(bytes).unwrap_or(source_url)
                            } else {
                                source_url
                            };

                            if decoded_url.contains(".m3u8") || decoded_url.contains("http") {
                                return Ok(StreamSource {
                                    provider: self.name().to_string(),
                                    audio_type: audio,
                                    language: if trans_type == "dub" {
                                        "English".to_string()
                                    } else {
                                        "Japanese".to_string()
                                    },
                                    streams: vec![VideoStream {
                                        quality: "1080p".to_string(),
                                        url: decoded_url,
                                    }],
                                    subtitles: Vec::new(),
                                    referer: REFERER.to_string(),
                                });
                            }
                        }
                    }
                    Err(ScraperError::NotFound("No valid streams in AllAnime response".into()))
                }
                Err(e) => Err(e),
            }
        })
    }
}
