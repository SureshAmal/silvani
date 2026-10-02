pub mod components;
pub mod history;
pub mod views;

use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::WindowExt;
use gpui_kit::prelude::*;
use gpui_kit::*;

use silvani_meta::image_cache::ImageCacheManager;
use silvani_meta::models::{AnimeItem, DashboardData};
use silvani_meta::AnimeMetadataClient;
use silvani_player::MpvPlayer;
use silvani_scraper::models::{AudioType, Episode};
use silvani_scraper::{AnimeStreamService, ScraperEngine};

use crate::components::category_tabs::AnimeCategory;
use crate::components::stream_settings_bar::SelectedAudio;
use crate::history::{HistoryItem, WatchHistoryManager};
use crate::views::browse::BrowseView;
use crate::views::details::{DetailsView, PlayEpisodeRequest};
use crate::views::history_view::HistoryView;

#[derive(Clone)]
pub enum CurrentPage {
    Browse,
    AnimeDetails {
        item: Box<AnimeItem>,
        scraped_episodes: Vec<Episode>,
        selected_provider: String,
        provider_anime_id: Option<String>,
        selected_audio: SelectedAudio,
        selected_resolution: String,
        is_loading_episodes: bool,
    },
    History,
}

pub struct SearchView {
    search_input: Entity<InputState>,
    episode_search_input: Entity<InputState>,
    current_episode_page: usize,
    meta_client: Arc<AnimeMetadataClient>,
    stream_service: Arc<AnimeStreamService>,
    image_cache: Arc<ImageCacheManager>,
    watch_history: WatchHistoryManager,
    dashboard_data: Option<DashboardData>,
    current_items: Vec<AnimeItem>,
    current_category: AnimeCategory,
    current_page: CurrentPage,
    is_loading: bool,
    _subscriptions: Vec<Subscription>,
}

impl SearchView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search anime...")
        });

        let episode_search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Filter episodes (e.g. 12)...")
        });

        let search_sub = cx.subscribe_in(&search_input, window, |this, _, event, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.search(window, cx);
            }
        });

        let ep_sub = cx.subscribe_in(&episode_search_input, window, |this, _, event, _window, cx| {
            if let InputEvent::Change = event {
                this.current_episode_page = 1;
                cx.notify();
            }
        });

        let meta_client = Arc::new(AnimeMetadataClient::new().expect("Failed to init metadata client"));
        let scraper_engine = Arc::new(ScraperEngine::new().expect("Failed to init scraper engine"));
        let stream_service = Arc::new(AnimeStreamService::new(scraper_engine));
        let image_cache = Arc::new(ImageCacheManager::new());
        let watch_history = WatchHistoryManager::new().expect("Failed to init watch history database");

        let mut view = Self {
            search_input,
            episode_search_input,
            current_episode_page: 1,
            meta_client,
            stream_service,
            image_cache,
            watch_history,
            dashboard_data: None,
            current_items: Vec::new(),
            current_category: AnimeCategory::Trending,
            current_page: CurrentPage::Browse,
            is_loading: true,
            _subscriptions: vec![search_sub, ep_sub],
        };

        view.load_dashboard(window, cx);
        view
    }

    pub fn load_dashboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_loading = true;
        cx.notify();

        let client = Arc::clone(&self.meta_client);
        let cache = Arc::clone(&self.image_cache);

        cx.spawn_in(window, async move |this, cx| {
            let dashboard = client.get_dashboard(24).await;

            this.update_in(cx, |this, window, cx| {
                if let Ok(data) = dashboard {
                    this.current_items = data.trending.clone();
                    let items_to_cache = data.trending.clone();
                    this.dashboard_data = Some(data);

                    // Prefetch covers concurrently in background safely on Tokio runtime and notify on completion
                    let prefetch_task = cache.prefetch_covers(&items_to_cache);
                    cx.spawn_in(window, async move |this, cx| {
                        let _ = prefetch_task.await;
                        this.update_in(cx, |_this, _window, cx| {
                            cx.notify();
                        }).ok();
                    }).detach();
                }
                this.is_loading = false;
                cx.notify();
            }).ok();
        }).detach();
    }

    pub fn select_category(&mut self, category: AnimeCategory, window: &mut Window, cx: &mut Context<Self>) {
        if category == AnimeCategory::History {
            self.current_page = CurrentPage::History;
            self.current_category = category;
            cx.notify();
            return;
        }

        self.current_page = CurrentPage::Browse;
        self.current_category = category.clone();

        let cache = Arc::clone(&self.image_cache);

        match category {
            AnimeCategory::Trending => {
                if let Some(data) = &self.dashboard_data {
                    self.current_items = data.trending.clone();
                    cx.notify();
                }
            }
            AnimeCategory::PopularSeason => {
                if let Some(data) = &self.dashboard_data {
                    self.current_items = data.popular_season.clone();
                    cx.notify();
                }
            }
            AnimeCategory::Upcoming => {
                if let Some(data) = &self.dashboard_data {
                    self.current_items = data.upcoming.clone();
                    cx.notify();
                }
            }
            AnimeCategory::TopRated => {
                if let Some(data) = &self.dashboard_data {
                    self.current_items = data.top_rated.clone();
                    cx.notify();
                }
            }
            AnimeCategory::History => {}
            AnimeCategory::Genre(genre) => {
                self.is_loading = true;
                cx.notify();

                let client = Arc::clone(&self.meta_client);
                let g = genre.clone();
                cx.spawn_in(window, async move |this, cx| {
                    let items = client.get_by_genre(&g, 1, 24).await.unwrap_or_default();
                    let items_c = items.clone();
                    this.update_in(cx, |this, window, cx| {
                        this.current_items = items;
                        this.is_loading = false;
                        cx.notify();

                        let prefetch_task = cache.prefetch_covers(&items_c);
                        cx.spawn_in(window, async move |this, cx| {
                            let _ = prefetch_task.await;
                            this.update_in(cx, |_this, _window, cx| {
                                cx.notify();
                            }).ok();
                        }).detach();
                    }).ok();
                }).detach();
            }
            AnimeCategory::SearchResults(_) => {}
        }
    }

    pub fn search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().to_string();
        let query = query.trim().to_string();
        if query.is_empty() {
            return;
        }

        self.current_page = CurrentPage::Browse;
        self.current_category = AnimeCategory::SearchResults(query.clone());
        self.current_items.clear();
        self.is_loading = true;
        cx.notify();

        let client = Arc::clone(&self.meta_client);
        let cache = Arc::clone(&self.image_cache);

        cx.spawn_in(window, async move |this, cx| {
            let results = client.search(&query, 1, 24).await.unwrap_or_default();
            let results_c = results.clone();

            this.update_in(cx, |this, window, cx| {
                this.current_items = results;
                this.is_loading = false;
                cx.notify();

                let prefetch_task = cache.prefetch_covers(&results_c);
                cx.spawn_in(window, async move |this, cx| {
                    let _ = prefetch_task.await;
                    this.update_in(cx, |_this, _window, cx| {
                        cx.notify();
                    }).ok();
                }).detach();
            }).ok();
        }).detach();
    }

    pub fn switch_provider(&mut self, new_provider: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let CurrentPage::AnimeDetails {
            item,
            scraped_episodes,
            selected_provider,
            provider_anime_id,
            is_loading_episodes,
            ..
        } = &mut self.current_page
        {
            *selected_provider = new_provider.to_string();
            *provider_anime_id = None;
            scraped_episodes.clear();
            *is_loading_episodes = true;
            self.current_episode_page = 1;
            cx.notify();

            let title = item.display_title().to_string();
            let prov = new_provider.to_string();
            let engine = Arc::clone(&self.scraper_engine);

            cx.spawn_in(window, async move |this, cx| {
                let results = engine.search(&title).await;
                let target = results.into_iter().find(|r| r.provider.eq_ignore_ascii_case(&prov));

                if let Some(matched) = target {
                    let aid = matched.id.clone();
                    let episodes = engine.episodes(&prov, &aid).await.unwrap_or_default();
                    this.update_in(cx, |this, _window, cx| {
                        if let CurrentPage::AnimeDetails {
                            scraped_episodes,
                            provider_anime_id,
                            is_loading_episodes,
                            ..
                        } = &mut this.current_page
                        {
                            *scraped_episodes = episodes;
                            *provider_anime_id = Some(aid);
                            *is_loading_episodes = false;
                            cx.notify();
                        }
                    }).ok();
                } else {
                    this.update_in(cx, |this, _window, cx| {
                        if let CurrentPage::AnimeDetails { is_loading_episodes, .. } = &mut this.current_page {
                            *is_loading_episodes = false;
                            cx.notify();
                        }
                    }).ok();
                }
            }).detach();
        }
    }

    pub fn open_anime_details(&mut self, item: AnimeItem, window: &mut Window, cx: &mut Context<Self>) {
        let title = item.display_title().to_string();
        let engine = Arc::clone(&self.scraper_engine);
        let meta_client = Arc::clone(&self.meta_client);
        let anime_id = item.id.clone();
        let has_relations = !item.relations.is_empty();

        self.current_episode_page = 1;
        self.episode_search_input.update(cx, |input, cx| {
            input.clean(window, cx);
        });

        self.current_page = CurrentPage::AnimeDetails {
            item: Box::new(item),
            scraped_episodes: Vec::new(),
            selected_provider: "hianime".to_string(),
            provider_anime_id: None,
            selected_audio: SelectedAudio::Sub,
            selected_resolution: "1080p".to_string(),
            is_loading_episodes: true,
        };
        cx.notify();

        // If relations are not populated, fetch full details for seasons
        if !has_relations {
            let aid = anime_id.clone();
            cx.spawn_in(window, async move |this, cx| {
                if let Ok(Some(details)) = meta_client.get_details(&aid).await {
                    this.update_in(cx, |this, _window, cx| {
                        if let CurrentPage::AnimeDetails { item, .. } = &mut this.current_page
                            && item.id == details.id
                        {
                            item.relations = details.relations;
                            if item.description.is_none() {
                                item.description = details.description;
                            }
                            cx.notify();
                        }
                    }).ok();
                }
            }).detach();
        }

        // Fetch episodes from active providers in background with channel & relevance verification
        cx.spawn_in(window, async move |this, cx| {
            let results = engine.search(&title).await;
            let target = results.iter().find(|r| r.provider == "hianime")
                .or_else(|| results.first());

            if let Some(first_match) = target {
                let provider = first_match.provider.clone();
                let aid = first_match.id.clone();

                let episodes = engine.episodes(&provider, &aid).await.unwrap_or_default();

                this.update_in(cx, |this, _window, cx| {
                    if let CurrentPage::AnimeDetails {
                        scraped_episodes,
                        selected_provider,
                        provider_anime_id,
                        is_loading_episodes,
                        ..
                    } = &mut this.current_page
                    {
                        *scraped_episodes = episodes;
                        *selected_provider = provider;
                        *provider_anime_id = Some(aid);
                        *is_loading_episodes = false;
                        cx.notify();
                    }
                }).ok();
            } else {
                this.update_in(cx, |this, _window, cx| {
                    if let CurrentPage::AnimeDetails { is_loading_episodes, .. } = &mut this.current_page {
                        *is_loading_episodes = false;
                        cx.notify();
                    }
                }).ok();
            }
        }).detach();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn play_episode(
        &mut self,
        anime_id: &str,
        anime_title: &str,
        ep_number: f32,
        ep_id: Option<String>,
        provider: &str,
        audio_type: AudioType,
        resolution: &str,
        cover_image: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = anime_title.to_string();
        let engine = Arc::clone(&self.scraper_engine);
        let prov = provider.to_string();
        let res = resolution.to_string();

        // Record to persistent watch history
        let _ = self.watch_history.record_watched(
            anime_id,
            ep_number,
            &title,
            None,
            cover_image.as_deref(),
        );

        window.push_notification(
            format!("Fetching {} ({:?}, {}) Ep {}...", title, audio_type, res, ep_number),
            cx,
        );

        cx.spawn_in(window, async move |_this, cx| {
            let play_res = if let Some(eid) = ep_id {
                if prov == "muse_india" || prov == "ani_one_asia" {
                    MpvPlayer::play_youtube(&format!("{} - Episode {}", title, ep_number), &eid, Some(&res))
                } else {
                    match engine.stream(&prov, &eid, audio_type).await {
                        Ok(source) => MpvPlayer::play_stream(
                            &format!("{} - Episode {}", title, ep_number),
                            &source,
                            Some(&res),
                        ),
                        Err(e) => Err(silvani_player::PlayerError::ProcessSpawn(
                            std::io::Error::other(e.to_string()),
                        )),
                    }
                }
            } else {
                // Fallback: search providers directly for this episode
                let results = engine.search(&title).await;
                let target = results
                    .iter()
                    .find(|r| r.provider.eq_ignore_ascii_case(&prov))
                    .cloned()
                    .or_else(|| results.first().cloned());

                if let Some(first_match) = target {
                    let prov = first_match.provider;
                    let aid = first_match.id;

                    if prov == "muse_india" || prov == "ani_one_asia" {
                        MpvPlayer::play_youtube(&format!("{} - Episode {}", title, ep_number), &aid, Some(&res))
                    } else {
                        match engine.episodes(&prov, &aid).await {
                            Ok(eps) => {
                                let target_ep = eps
                                    .iter()
                                    .find(|e| (e.number - ep_number).abs() < 0.1)
                                    .or_else(|| eps.first());

                                if let Some(ep) = target_ep {
                                    match engine.stream(&prov, &ep.id, audio_type).await {
                                        Ok(source) => MpvPlayer::play_stream(
                                            &format!("{} - Episode {}", title, ep.number),
                                            &source,
                                            Some(&res),
                                        ),
                                        Err(e) => Err(silvani_player::PlayerError::ProcessSpawn(
                                            std::io::Error::other(e.to_string()),
                                        )),
                                    }
                                } else {
                                    Err(silvani_player::PlayerError::NoStreams)
                                }
                            }
                            Err(e) => Err(silvani_player::PlayerError::ProcessSpawn(
                                std::io::Error::other(e.to_string()),
                            )),
                        }
                    }
                } else {
                    Err(silvani_player::PlayerError::NoStreams)
                }
            };

            cx.update(|window, cx| {
                match play_res {
                    Ok(_) => window.push_notification(format!("Streaming Episode {} in MPV!", ep_number), cx),
                    Err(e) => window.push_notification(format!("Playback error: {}", e), cx),
                }
            }).ok();
        }).detach();
    }
}

impl Render for SearchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.current_page {
            CurrentPage::Browse => {
                BrowseView::new(
                    &self.search_input,
                    self.current_category.clone(),
                    self.current_items.clone(),
                    self.is_loading,
                    Arc::clone(&self.image_cache),
                )
                .on_search(cx.listener(|this, _, window, cx| {
                    this.search(window, cx);
                }))
                .on_history(cx.listener(|this, _, _window, cx| {
                    this.current_page = CurrentPage::History;
                    this.current_category = AnimeCategory::History;
                    cx.notify();
                }))
                .on_select_category(cx.listener(|this, cat: &AnimeCategory, window, cx| {
                    this.select_category(cat.clone(), window, cx);
                }))
                .on_open_details(cx.listener(|this, item: &AnimeItem, window, cx| {
                    this.open_anime_details(item.clone(), window, cx);
                }))
                .into_any_element()
            }
            CurrentPage::AnimeDetails {
                item,
                scraped_episodes,
                selected_provider,
                selected_audio,
                selected_resolution,
                is_loading_episodes,
                ..
            } => {
                let anime_id = item.id.clone();
                let anime_title = item.display_title().to_string();
                let cover_image = item.cover_image.clone();

                DetailsView::new(
                    *item.clone(),
                    scraped_episodes.clone(),
                    *is_loading_episodes,
                    selected_provider.clone(),
                    selected_audio.clone(),
                    selected_resolution.clone(),
                    &self.episode_search_input,
                    self.current_episode_page,
                    self.watch_history.clone(),
                    Arc::clone(&self.image_cache),
                )
                .on_back(cx.listener(|this, _, _window, cx| {
                    this.current_page = CurrentPage::Browse;
                    cx.notify();
                }))
                .on_play_episode(cx.listener(move |this, req: &PlayEpisodeRequest, window, cx| {
                    this.play_episode(
                        &anime_id,
                        &anime_title,
                        req.number,
                        req.ep_id.clone(),
                        &req.provider,
                        req.audio.to_audio_type(),
                        &req.resolution,
                        cover_image.clone(),
                        window,
                        cx,
                    );
                }))
                .on_select_season(cx.listener(|this, rel_id: &str, window, cx| {
                    let meta = Arc::clone(&this.meta_client);
                    let aid = rel_id.to_string();
                    this.is_loading = true;
                    cx.notify();
                    cx.spawn_in(window, async move |this, cx| {
                        if let Ok(Some(item)) = meta.get_details(&aid).await {
                            this.update_in(cx, |this, window, cx| {
                                this.open_anime_details(item, window, cx);
                            }).ok();
                        }
                    }).detach();
                }))
                .on_page_change(cx.listener(|this, page: &usize, _window, cx| {
                    this.current_episode_page = *page;
                    cx.notify();
                }))
                .on_provider_change(cx.listener(|this, new_prov: &str, window, cx| {
                    this.switch_provider(new_prov, window, cx);
                }))
                .on_audio_change(cx.listener(|this, new_audio: &SelectedAudio, _window, cx| {
                    if let CurrentPage::AnimeDetails { selected_audio, .. } = &mut this.current_page {
                        *selected_audio = new_audio.clone();
                        cx.notify();
                    }
                }))
                .on_resolution_change(cx.listener(|this, new_res: &str, _window, cx| {
                    if let CurrentPage::AnimeDetails { selected_resolution, .. } = &mut this.current_page {
                        *selected_resolution = new_res.to_string();
                        cx.notify();
                    }
                }))
                .into_any_element()
            }
            CurrentPage::History => {
                HistoryView::new(self.watch_history.clone())
                    .on_back(cx.listener(|this, _, _window, cx| {
                        this.current_page = CurrentPage::Browse;
                        this.current_category = AnimeCategory::Trending;
                        cx.notify();
                    }))
                    .on_resume(cx.listener(|this, hist_item: &HistoryItem, window, cx| {
                        let title = hist_item.anime_title.clone();
                        let ep = hist_item.episode_number;
                        let aid = hist_item.anime_id.clone();
                        let cover = hist_item.cover_image.clone();
                        this.play_episode(
                            &aid,
                            &title,
                            ep,
                            None,
                            "hianime",
                            AudioType::Sub,
                            "1080p",
                            cover,
                            window,
                            cx,
                        );
                    }))
                    .on_clear(cx.listener(|this, _, _window, cx| {
                        let _ = this.watch_history.clear_history();
                        cx.notify();
                    }))
                    .into_any_element()
            }
        }
    }
}

pub fn run() {
    let http_client = std::sync::Arc::new(
        reqwest_client::ReqwestClient::user_agent(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Silvani/1.0",
        )
        .unwrap_or_else(|_| reqwest_client::ReqwestClient::new()),
    );

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_http_client(http_client.clone())
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_http_client(http_client);

            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| SearchView::new(window, cx))
            })
            .expect("Failed to open window");
        });
}
