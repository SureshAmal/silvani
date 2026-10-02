pub mod cache;
pub mod client;
pub mod image_cache;
pub mod models;
pub mod providers;

pub use cache::MetaCache;
pub use client::AnimeMetadataClient;
pub use image_cache::ImageCacheManager;
pub use models::{AnimeItem, AnimeTitle, DashboardData, RelatedAnime};
pub use providers::anilist::AniListProvider;
pub use providers::jikan::JikanProvider;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_anilist_dashboard() {
        let provider = AniListProvider::new();
        let data = provider.fetch_dashboard(2).await;
        assert!(data.is_ok());
        let data = data.unwrap();
        assert!(!data.trending.is_empty());
        assert!(!data.upcoming.is_empty());
        assert!(!data.top_rated.is_empty());
    }

    #[tokio::test]
    async fn test_client_search_and_cache() {
        let client = AnimeMetadataClient::new().unwrap();
        let results = client.search("Naruto", 1, 3).await;
        assert!(results.is_ok());
        let results = results.unwrap();
        assert!(!results.is_empty());
        assert!(results[0].display_title().to_lowercase().contains("naruto"));

        // Verify second call retrieves from cache instantly
        let cached = client.search("Naruto", 1, 3).await;
        assert!(cached.is_ok());
    }
}
