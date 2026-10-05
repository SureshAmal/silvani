use std::time::Duration;
use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, UPGRADE_INSECURE_REQUESTS, USER_AGENT,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{Result, ScraperError};

pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct ScraperHttpClient {
    client: reqwest::Client,
}

impl Default for ScraperHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ScraperHttpClient {
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(DEFAULT_USER_AGENT),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
            ),
        );
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));
        headers.insert(
            "Sec-Ch-Ua",
            HeaderValue::from_static(
                "\"Chromium\";v=\"124\", \"Google Chrome\";v=\"124\", \"Not-A.Brand\";v=\"99\"",
            ),
        );
        headers.insert("Sec-Ch-Ua-Mobile", HeaderValue::from_static("?0"));
        headers.insert("Sec-Ch-Ua-Platform", HeaderValue::from_static("\"Windows\""));
        headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("document"));
        headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
        headers.insert("Sec-Fetch-Site", HeaderValue::from_static("none"));
        headers.insert("Sec-Fetch-User", HeaderValue::from_static("?1"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .tcp_nodelay(true)
            .timeout(Duration::from_secs(4))
            .connect_timeout(Duration::from_secs(6))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { client }
    }

    pub fn raw_client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Determines if response indicates a Cloudflare block, challenge, or rate limit
    pub fn is_cloudflare_challenge(status: reqwest::StatusCode, body: &str) -> bool {
        if (status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::SERVICE_UNAVAILABLE)
            && (body.contains("Just a moment...")
                || body.contains("_cf_chl_opt")
                || body.contains("challenges.cloudflare.com")
                || body.contains("cf-mitigated: challenge"))
        {
            return true;
        }
        false
    }

    /// Fetches HTML from a list of mirror URLs in sequence.
    /// If a mirror is challenged by Cloudflare or fails, automatically tries the next mirror.
    pub async fn get_html_with_mirrors(
        &self,
        urls: &[String],
        referer: Option<&str>,
    ) -> Result<(String, String)> {
        let mut last_error = None;

        for url in urls {
            let mut req = self.client.get(url);
            if let Some(ref_val) = referer {
                req = req.header("Referer", ref_val);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let effective_url = resp.url().to_string();

                    match resp.text().await {
                        Ok(body) => {
                            if Self::is_cloudflare_challenge(status, &body) {
                                last_error = Some(ScraperError::CloudflareBlocked(format!(
                                    "Cloudflare challenge detected on {url}"
                                )));
                                continue;
                            }

                            if !status.is_success() {
                                last_error = Some(ScraperError::Network(format!(
                                    "HTTP error {status} for {url}"
                                )));
                                continue;
                            }

                            return Ok((effective_url, body));
                        }
                        Err(e) => {
                            last_error = Some(ScraperError::Network(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(ScraperError::Network(e.to_string()));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ScraperError::NotFound("All mirror URLs failed or were blocked by Cloudflare".into())
        }))
    }

    /// Fetches JSON from a list of mirror URLs in sequence.
    pub async fn get_json_with_mirrors<T: DeserializeOwned>(
        &self,
        urls: &[String],
        referer: Option<&str>,
    ) -> Result<(String, T)> {
        let mut last_error = None;

        for url in urls {
            let mut req = self.client.get(url).header("Accept", "application/json");
            if let Some(ref_val) = referer {
                req = req.header("Referer", ref_val);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let effective_url = resp.url().to_string();

                    match resp.text().await {
                        Ok(text) => {
                            if Self::is_cloudflare_challenge(status, &text) {
                                last_error = Some(ScraperError::CloudflareBlocked(format!(
                                    "Cloudflare challenge on {url}"
                                )));
                                continue;
                            }

                            if !status.is_success() {
                                last_error = Some(ScraperError::Network(format!(
                                    "HTTP error {status} on {url}"
                                )));
                                continue;
                            }

                            match serde_json::from_str::<T>(&text) {
                                Ok(data) => return Ok((effective_url, data)),
                                Err(e) => {
                                    last_error = Some(ScraperError::Parse(format!(
                                        "JSON parse error for {url}: {e}"
                                    )));
                                }
                            }
                        }
                        Err(e) => last_error = Some(ScraperError::Network(e.to_string())),
                    }
                }
                Err(e) => last_error = Some(ScraperError::Network(e.to_string())),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ScraperError::NotFound("All mirror URLs failed or returned invalid JSON".into())
        }))
    }

    /// Executes a GraphQL query against an endpoint with required CORS and Referer headers
    pub async fn post_graphql<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        query: &str,
        variables: Option<serde_json::Value>,
        referer: Option<&str>,
        origin: Option<&str>,
    ) -> Result<T> {
        #[derive(Serialize)]
        struct GraphQLRequest<'a> {
            query: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            variables: Option<serde_json::Value>,
        }

        let body = GraphQLRequest { query, variables };

        let mut req = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "*/*")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Dest", "empty");

        if let Some(ref_val) = referer {
            req = req.header("Referer", ref_val);
        }
        if let Some(orig_val) = origin {
            req = req.header("Origin", orig_val);
        }

        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| ScraperError::Network(e.to_string()))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ScraperError::Network(e.to_string()))?;

        if Self::is_cloudflare_challenge(status, &text) {
            return Err(ScraperError::CloudflareBlocked(format!(
                "Cloudflare challenge encountered on {endpoint}"
            )));
        }

        if !status.is_success() {
            return Err(ScraperError::Network(format!(
                "GraphQL request failed with HTTP {status}: {text}"
            )));
        }

        serde_json::from_str::<T>(&text).map_err(|e| {
            ScraperError::Parse(format!("Failed to parse GraphQL response: {e}. Raw: {text}"))
        })
    }
}
