use std::future::Future;
use std::pin::Pin;

use crate::error::Result;
use crate::models::{AnimeSearchResult, AudioType, Episode, StreamSource};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait AnimeProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn search<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<Vec<AnimeSearchResult>>>;
    fn episodes<'a>(&'a self, anime_id: &'a str) -> BoxFuture<'a, Result<Vec<Episode>>>;
    fn stream<'a>(&'a self, episode_id: &'a str, audio: AudioType) -> BoxFuture<'a, Result<StreamSource>>;
}
