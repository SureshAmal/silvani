use std::future::Future;
use std::pin::Pin;

use crate::error::Result;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ProviderPriority {
    #[default]
    High = 1,
    Medium = 2,
    Low = 3,
}

#[derive(Debug, Clone)]
pub struct ProviderMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub supported_audio: Vec<AudioType>,
    pub priority: ProviderPriority,
    pub is_official: bool,
    pub primary_domain: &'static str,
    pub mirror_domains: Vec<&'static str>,
}

pub trait AnimeProvider: Send + Sync {
    fn id(&self) -> &'static str {
        self.metadata().id
    }

    fn name(&self) -> &'static str {
        self.metadata().name
    }

    fn display_name(&self) -> &'static str {
        self.metadata().display_name
    }

    fn supported_audio(&self) -> Vec<AudioType> {
        self.metadata().supported_audio
    }

    fn priority(&self) -> ProviderPriority {
        self.metadata().priority
    }

    fn metadata(&self) -> ProviderMetadata;

    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>>;
    fn episodes<'a>(&'a self, anime_id: &'a str) -> BoxFuture<'a, Result<Vec<Episode>>>;
    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>>;
}
