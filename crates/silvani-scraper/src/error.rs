use std::fmt;

#[derive(Debug, Clone)]
pub enum ScraperError {
    Network(String),
    Parse(String),
    NotFound(String),
    Crypto(String),
    CloudflareBlocked(String),
    Timeout(String),
}

impl fmt::Display for ScraperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(msg) => write!(f, "Network error: {msg}"),
            Self::Parse(msg) => write!(f, "Parse error: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::Crypto(msg) => write!(f, "Crypto error: {msg}"),
            Self::CloudflareBlocked(msg) => write!(f, "Cloudflare challenge/block: {msg}"),
            Self::Timeout(msg) => write!(f, "Operation timed out: {msg}"),
        }
    }
}

impl std::error::Error for ScraperError {}

pub type Result<T> = std::result::Result<T, ScraperError>;
