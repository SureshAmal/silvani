pub mod components;
pub mod history;
pub mod settings;
pub mod views;

use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{
    v_flex, ActiveTheme, FocusTrapElement, Theme, ThemeMode, ThemeRegistry,
};
use gpui_kit::prelude::*;
use gpui_kit::*;


use silvani_meta::image_cache::ImageCacheManager;
use silvani_meta::models::{AnimeItem, DashboardData};
use silvani_meta::AnimeMetadataClient;
use silvani_player::{MediaPlayer, MpvPlayer, PlaybackStatus, PlayerController, PlayerEvent};
use silvani_scraper::models::{AudioType, Episode};
use silvani_scraper::{AnimeStreamService, ScraperEngine};

use crate::components::category_tabs::AnimeCategory;
use crate::components::stream_settings_bar::SelectedAudio;
use crate::components::theme_picker::{ThemePickerDelegate, ThemePickerState};
use crate::components::VideoPlayerControls;
use crate::history::{HistoryItem, WatchHistoryManager};
use crate::settings::AppSettings;
use crate::views::browse::BrowseView;
use crate::views::details::{DetailsView, PlayEpisodeRequest};
use crate::views::history_view::HistoryView;
use crate::views::settings_view::SettingsView;
use crate::views::player_view::EmbeddedPlayerView;
use gpui_kit::component::list::{List, ListEvent, ListState};

gpui_kit::actions!(silvani, [ToggleThemePicker, ReloadApp]);

#[derive(Clone)]
pub struct ActivePlaybackState {
    pub title: String,
    pub controller: Option<PlayerController>,
    pub status: PlaybackStatus,
    pub current_frame: Option<Arc<gpui::RenderImage>>,
    pub error: Option<String>,
}

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
    EmbeddedPlayer {
        anime_item: Box<AnimeItem>,
        episodes: Vec<Episode>,
        ep_number: f32,
        ep_title: String,
        provider: String,
        audio: String,
        resolution: String,
        is_loading: bool,
    },
    History,
    Settings,
}

pub struct SearchView {
    search_input: Entity<InputState>,
    episode_search_input: Entity<InputState>,
    current_episode_page: usize,
    meta_client: Arc<AnimeMetadataClient>,
    stream_service: Arc<AnimeStreamService>,
    player: Arc<dyn MediaPlayer>,
    image_cache: Arc<ImageCacheManager>,
    watch_history: WatchHistoryManager,
    app_settings: AppSettings,
    dashboard_data: Option<DashboardData>,
    current_items: Vec<AnimeItem>,
    current_category: AnimeCategory,
    current_page: CurrentPage,
    active_playback: Option<ActivePlaybackState>,
    is_loading: bool,
    is_player_fullscreen: bool,
    last_mouse_activity: std::time::Instant,
    seek_slider: Entity<SliderState>,
    volume_slider: Entity<SliderState>,
    is_seeking_drag: bool,
    theme_picker: Option<ThemePickerState>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}



impl SearchView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
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

        let cache_dir = settings::config_dir().join("cache");
        let _ = std::fs::create_dir_all(&cache_dir);
        let meta_db = cache_dir.join("meta.db");
        let scraper_db = cache_dir.join("scraper.db");

        let meta_client = Arc::new(
            AnimeMetadataClient::with_cache_file(meta_db.to_str().unwrap_or(""))
                .or_else(|_| AnimeMetadataClient::new())
                .expect("Failed to init metadata client"),
        );
        let scraper_engine = Arc::new(
            ScraperEngine::with_cache_file(scraper_db.to_str().unwrap_or(""))
                .or_else(|_| ScraperEngine::new())
                .expect("Failed to init scraper engine"),
        );
        let stream_service = Arc::new(AnimeStreamService::new(scraper_engine));
        let player: Arc<dyn MediaPlayer> = Arc::new(MpvPlayer);
        let image_cache = Arc::new(ImageCacheManager::new());
        let watch_history = WatchHistoryManager::new().expect("Failed to init watch history database");
        let app_settings = settings::load_settings();

        let seek_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(0.1)
                .default_value(0.0)
        });

        let volume_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(100.0)
        });

        let seek_sub = cx.subscribe_in(&seek_slider, window, |this, _, event: &SliderEvent, _window, cx| {
            match event {
                SliderEvent::Change(val) => {
                    this.is_seeking_drag = true;
                    let target_secs = val.start() as f64;
                    if let Some(ref mut pb) = this.active_playback {
                        pb.status.position_secs = target_secs;
                    }
                }
                SliderEvent::Release(val) => {
                    this.is_seeking_drag = false;
                    let target_secs = val.start() as f64;
                    if let Some(ref mut pb) = this.active_playback {
                        pb.status.position_secs = target_secs;
                        if let Some(ref ctrl) = pb.controller {
                            let _ = ctrl.seek_absolute(target_secs);
                        }
                    }
                }
            }
            cx.notify();
        });

        let vol_sub = cx.subscribe_in(&volume_slider, window, |this, _, event: &SliderEvent, _window, cx| {
            let target_vol = match event {
                SliderEvent::Change(val) | SliderEvent::Release(val) => val.start() as f64,
            };
            if let Some(ref mut pb) = this.active_playback {
                pb.status.volume = target_vol;
                if let Some(ref ctrl) = pb.controller {
                    let _ = ctrl.set_volume(target_vol);
                }
            }
            cx.notify();
        });

        let mut view = Self {
            search_input,
            episode_search_input,
            current_episode_page: 1,
            meta_client,
            stream_service,
            player,
            image_cache,
            watch_history,
            app_settings,
            dashboard_data: None,
            current_items: Vec::new(),
            current_category: AnimeCategory::Trending,
            current_page: CurrentPage::Browse,
            active_playback: None,
            is_loading: true,
            is_player_fullscreen: false,
            last_mouse_activity: std::time::Instant::now(),
            seek_slider,
            volume_slider,
            is_seeking_drag: false,
            theme_picker: None,
            focus_handle,
            _subscriptions: vec![search_sub, ep_sub, seek_sub, vol_sub],
        };



        window.focus(&view.focus_handle, cx);
        view.load_dashboard(window, cx);
        view
    }

    pub fn toggle_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_picker.is_some() {
            self.theme_picker = None;
            window.focus(&self.focus_handle, cx);
            cx.notify();
            return;
        }

        let themes = settings::available_themes(cx);
        let delegate = ThemePickerDelegate::new(themes);
        let list = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));
        let trap_handle = cx.focus_handle();

        list.update(cx, |state, cx| {
            state.focus(window, cx);
        });

        let _subscription = cx.subscribe_in(&list, window, |this, state, event: &ListEvent, window, cx| {
            match event {
                ListEvent::Confirm(ix) => {
                    if let Some(theme) = state.read(cx).delegate().get_theme(*ix) {
                        let name = theme.name.to_string();
                        settings::apply_theme(&name, cx);
                        this.app_settings.theme = name;
                    }
                    this.theme_picker = None;
                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                }
                ListEvent::Cancel => {
                    this.theme_picker = None;
                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                }
                _ => {}
            }
        });

        self.theme_picker = Some(ThemePickerState { list, trap_handle, _subscription });
        cx.notify();
    }

    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.meta_client.clear_cache();
        self.stream_service.clear_cache();

        match &self.current_page {
            CurrentPage::Browse => {
                match &self.current_category {
                    AnimeCategory::SearchResults(_) => self.search(window, cx),
                    AnimeCategory::Genre(genre) => {
                        let g = genre.clone();
                        self.fetch_genre(&g, window, cx);
                    }
                    _ => self.load_dashboard(window, cx),
                }
            }
            CurrentPage::AnimeDetails { item, selected_provider, selected_audio, .. } => {
                let aid = item.id.clone();
                let title = item.display_title().to_string();
                let prov = selected_provider.clone();
                let audio = selected_audio.to_audio_type();
                let service = Arc::clone(&self.stream_service);
                let meta_client = Arc::clone(&self.meta_client);

                if let CurrentPage::AnimeDetails { is_loading_episodes, .. } = &mut self.current_page {
                    *is_loading_episodes = true;
                }
                cx.notify();

                // Re-fetch details / relations fresh
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

                // Re-fetch episodes fresh
                cx.spawn_in(window, async move |this, cx| {
                    let res = service.fetch_episodes(&title, &prov, audio).await;
                    this.update_in(cx, |this, _window, cx| {
                        if let CurrentPage::AnimeDetails {
                            scraped_episodes,
                            provider_anime_id,
                            is_loading_episodes,
                            ..
                        } = &mut this.current_page
                        {
                            if let Ok((aid, episodes)) = res {
                                *scraped_episodes = episodes;
                                *provider_anime_id = Some(aid);
                            }
                            *is_loading_episodes = false;
                            cx.notify();
                        }
                    }).ok();
                }).detach();
            }
            CurrentPage::EmbeddedPlayer { .. } => {
                cx.notify();
            }
            CurrentPage::History => {
                cx.notify();
            }
            CurrentPage::Settings => {
                self.app_settings = settings::load_settings();
                cx.notify();
            }
        }
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
                    this.dashboard_data = Some(data.clone());
                    this.current_items = match &this.current_category {
                        AnimeCategory::Trending => data.trending.clone(),
                        AnimeCategory::PopularSeason => data.popular_season.clone(),
                        AnimeCategory::Upcoming => data.upcoming.clone(),
                        AnimeCategory::TopRated => data.top_rated.clone(),
                        _ => data.trending.clone(),
                    };
                    let items_to_cache = this.current_items.clone();

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

        if category == AnimeCategory::Settings {
            self.current_page = CurrentPage::Settings;
            self.current_category = category;
            cx.notify();
            return;
        }

        let is_same = matches!(self.current_page, CurrentPage::Browse) && self.current_category == category;
        self.current_page = CurrentPage::Browse;
        self.current_category = category.clone();

        if is_same {
            return;
        }

        match category {
            AnimeCategory::Trending => {
                if let Some(data) = &self.dashboard_data {
                    self.current_items = data.trending.clone();
                    cx.notify();
                } else {
                    self.load_dashboard(window, cx);
                }
            }
            AnimeCategory::PopularSeason => {
                if let Some(data) = &self.dashboard_data {
                    self.current_items = data.popular_season.clone();
                    cx.notify();
                } else {
                    self.load_dashboard(window, cx);
                }
            }
            AnimeCategory::Upcoming => {
                if let Some(data) = &self.dashboard_data {
                    self.current_items = data.upcoming.clone();
                    cx.notify();
                } else {
                    self.load_dashboard(window, cx);
                }
            }
            AnimeCategory::TopRated => {
                if let Some(data) = &self.dashboard_data {
                    self.current_items = data.top_rated.clone();
                    cx.notify();
                } else {
                    self.load_dashboard(window, cx);
                }
            }
            AnimeCategory::History => {}
            AnimeCategory::Genre(genre) => {
                self.fetch_genre(&genre, window, cx);
            }
            AnimeCategory::SearchResults(_) => {}
            AnimeCategory::Settings => {}
        }
    }

    pub fn fetch_genre(&mut self, genre: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.is_loading = true;
        cx.notify();

        let client = Arc::clone(&self.meta_client);
        let cache = Arc::clone(&self.image_cache);
        let g = genre.to_string();
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
        let mut play_req = None;

        match &mut self.current_page {
            CurrentPage::AnimeDetails {
                item,
                scraped_episodes,
                selected_provider,
                provider_anime_id,
                selected_audio,
                is_loading_episodes,
                ..
            } => {
                if *is_loading_episodes {
                    return;
                }
                *selected_provider = new_provider.to_string();
                *provider_anime_id = None;
                scraped_episodes.clear();
                *is_loading_episodes = true;
                self.current_episode_page = 1;
                cx.notify();

                let title = item.display_title().to_string();
                let prov = new_provider.to_string();
                let audio_type = selected_audio.to_audio_type();
                let service = Arc::clone(&self.stream_service);

                cx.spawn_in(window, async move |this, cx| {
                    let res = service.fetch_episodes(&title, &prov, audio_type).await;
                    this.update_in(cx, |this, _window, cx| {
                        if let CurrentPage::AnimeDetails {
                            scraped_episodes,
                            provider_anime_id,
                            is_loading_episodes,
                            ..
                        } = &mut this.current_page
                        {
                            if let Ok((aid, episodes)) = res {
                                *scraped_episodes = episodes;
                                *provider_anime_id = Some(aid);
                            } else {
                                scraped_episodes.clear();
                            }
                            *is_loading_episodes = false;
                            cx.notify();
                        }
                    }).ok();
                }).detach();
            }
            CurrentPage::EmbeddedPlayer {
                anime_item,
                ep_number,
                provider,
                audio,
                resolution,
                is_loading,
                ..
            } => {
                let item_clone = anime_item.clone();
                let ep_num = *ep_number;
                let prov = new_provider.to_string();
                let aud_str = audio.clone();
                let aud_type = match aud_str.as_str() {
                    "dub" => AudioType::Dub,
                    "hindi" => AudioType::Hindi,
                    _ => AudioType::Sub,
                };
                let res = resolution.clone();

                *provider = prov.clone();
                *is_loading = true;
                cx.notify();

                play_req = Some((item_clone, ep_num, prov, aud_type, res));
            }
            _ => {}
        }

        if let Some((item_clone, ep_num, prov, aud_type, res)) = play_req {
            self.play_episode(
                &item_clone.id,
                item_clone.display_title(),
                ep_num,
                None,
                &prov,
                aud_type,
                &res,
                item_clone.cover_image.clone(),
                window,
                cx,
            );
        }
    }

    pub fn switch_audio(&mut self, new_audio: SelectedAudio, window: &mut Window, cx: &mut Context<Self>) {
        let mut play_req = None;

        match &mut self.current_page {
            CurrentPage::AnimeDetails {
                item,
                scraped_episodes,
                selected_provider,
                provider_anime_id,
                selected_audio,
                is_loading_episodes,
                ..
            } => {
                if *is_loading_episodes {
                    return;
                }
                *selected_audio = new_audio.clone();

                // Automatically adapt provider for Hindi Dub
                if new_audio == SelectedAudio::Hindi
                    && *selected_provider != "animeworldindia"
                    && *selected_provider != "desidubanime"
                    && *selected_provider != "muse_india"
                {
                    *selected_provider = "animeworldindia".to_string();
                } else if new_audio != SelectedAudio::Hindi
                    && (*selected_provider == "animeworldindia" || *selected_provider == "desidubanime")
                {
                    *selected_provider = "hianime".to_string();
                }

                *provider_anime_id = None;
                scraped_episodes.clear();
                *is_loading_episodes = true;
                self.current_episode_page = 1;
                cx.notify();

                let title = item.display_title().to_string();
                let prov = selected_provider.clone();
                let audio_type = new_audio.to_audio_type();
                let service = Arc::clone(&self.stream_service);

                cx.spawn_in(window, async move |this, cx| {
                    let res = service.fetch_episodes(&title, &prov, audio_type).await;
                    this.update_in(cx, |this, _window, cx| {
                        if let CurrentPage::AnimeDetails {
                            scraped_episodes,
                            provider_anime_id,
                            is_loading_episodes,
                            ..
                        } = &mut this.current_page
                        {
                            if let Ok((aid, episodes)) = res {
                                *scraped_episodes = episodes;
                                *provider_anime_id = Some(aid);
                            } else {
                                scraped_episodes.clear();
                            }
                            *is_loading_episodes = false;
                            cx.notify();
                        }
                    }).ok();
                }).detach();
            }
            CurrentPage::EmbeddedPlayer {
                anime_item,
                ep_number,
                provider,
                audio,
                resolution,
                is_loading,
                ..
            } => {
                let item_clone = anime_item.clone();
                let ep_num = *ep_number;
                let mut prov = provider.clone();
                if new_audio == SelectedAudio::Hindi
                    && prov != "animeworldindia"
                    && prov != "desidubanime"
                    && prov != "muse_india"
                {
                    prov = "animeworldindia".to_string();
                    *provider = prov.clone();
                } else if new_audio != SelectedAudio::Hindi
                    && (prov == "animeworldindia" || prov == "desidubanime")
                {
                    prov = "hianime".to_string();
                    *provider = prov.clone();
                }
                *audio = new_audio.to_string();
                *is_loading = true;
                cx.notify();

                play_req = Some((item_clone, ep_num, prov, new_audio.to_audio_type(), resolution.clone()));
            }
            _ => {}
        }

        if let Some((item_clone, ep_num, prov, aud_type, res)) = play_req {
            self.play_episode(
                &item_clone.id,
                item_clone.display_title(),
                ep_num,
                None,
                &prov,
                aud_type,
                &res,
                item_clone.cover_image.clone(),
                window,
                cx,
            );
        }
    }

    pub fn open_anime_details(&mut self, item: AnimeItem, window: &mut Window, cx: &mut Context<Self>) {
        let title = item.display_title().to_string();
        let service = Arc::clone(&self.stream_service);
        let meta_client = Arc::clone(&self.meta_client);
        let anime_id = item.id.clone();
        let has_relations = !item.relations.is_empty();

        let default_prov = self.app_settings.get_default_provider();
        let default_aud = match self.app_settings.default_audio.to_lowercase().as_str() {
            "dub" => SelectedAudio::Dub,
            "hindi" | "hindidub" => SelectedAudio::Hindi,
            _ => SelectedAudio::Sub,
        };
        let default_res = if self.app_settings.default_resolution.is_empty() {
            "1080p".to_string()
        } else {
            self.app_settings.default_resolution.clone()
        };

        self.current_episode_page = 1;
        self.episode_search_input.update(cx, |input, cx| {
            input.clean(window, cx);
        });

        self.current_page = CurrentPage::AnimeDetails {
            item: Box::new(item),
            scraped_episodes: Vec::new(),
            selected_provider: default_prov.clone(),
            provider_anime_id: None,
            selected_audio: default_aud.clone(),
            selected_resolution: default_res,
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

        // Fetch episodes via stream_service
        let fetch_aud = default_aud.to_audio_type();
        cx.spawn_in(window, async move |this, cx| {
            let res = service.fetch_episodes(&title, &default_prov, fetch_aud).await;
            this.update_in(cx, |this, _window, cx| {
                if let CurrentPage::AnimeDetails {
                    scraped_episodes,
                    provider_anime_id,
                    is_loading_episodes,
                    ..
                } = &mut this.current_page
                {
                    if let Ok((aid, episodes)) = res {
                        *scraped_episodes = episodes;
                        *provider_anime_id = Some(aid);
                    }
                    *is_loading_episodes = false;
                    cx.notify();
                }
            }).ok();
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
        let service = Arc::clone(&self.stream_service);
        let player = Arc::clone(&self.player);
        let prov = provider.to_string();
        let res = resolution.to_string();
        let history = self.watch_history.clone();
        let anime_id = anime_id.to_string();
        if let Some(ref pb) = self.active_playback {
            if let Some(ref ctrl) = pb.controller {
                let _ = ctrl.quit();
            }
        }
        self.active_playback = None;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PlayerEvent>();

        let ep_title = format!("{} - Episode {:.0}", title, ep_number);
        let title_c = ep_title.clone();
        let aid_c = anime_id.clone();
        let cov_c = cover_image.clone();
        let hist_c = history.clone();

        let service_prefetch = Arc::clone(&service);
        let prov_c = prov.clone();
        let title_prefetch = title.clone();
        let mut has_prefetched_next = false;

        cx.spawn_in(window, async move |this, cx| {
            while let Some(event) = rx.recv().await {
                this.update_in(cx, |this, _window, cx| {
                    match event {
                        PlayerEvent::Started(controller) => {
                            if let Some(ref mut pb) = this.active_playback {
                                pb.controller = Some(PlayerController::Socket(controller.clone()));
                            } else {
                                this.active_playback = Some(ActivePlaybackState {
                                    title: title_c.clone(),
                                    controller: Some(PlayerController::Socket(controller.clone())),
                                    status: PlaybackStatus::default(),
                                    current_frame: None,
                                    error: None,
                                });
                            }
                            cx.notify();
                        }
                        PlayerEvent::EmbeddedStarted(handle) => {
                            if let Some(ref mut pb) = this.active_playback {
                                pb.controller = Some(PlayerController::Embedded(handle.clone()));
                            } else {
                                this.active_playback = Some(ActivePlaybackState {
                                    title: title_c.clone(),
                                    controller: Some(PlayerController::Embedded(handle.clone())),
                                    status: PlaybackStatus::default(),
                                    current_frame: None,
                                    error: None,
                                });
                            }
                            cx.notify();
                        }
                        PlayerEvent::FrameReady(frame) => {
                            let img_buf = image::RgbaImage::from_raw(frame.width, frame.height, frame.data);
                            if let Some(buf) = img_buf {
                                let render_img = Arc::new(gpui::RenderImage::new(smallvec::smallvec![image::Frame::new(buf)]));
                                if let Some(ref mut playback) = this.active_playback {
                                    playback.current_frame = Some(render_img);
                                } else {
                                    this.active_playback = Some(ActivePlaybackState {
                                        title: title_c.clone(),
                                        controller: None,
                                        status: PlaybackStatus::default(),
                                        current_frame: Some(render_img),
                                        error: None,
                                    });
                                }
                                if let CurrentPage::EmbeddedPlayer { is_loading, .. } = &mut this.current_page {
                                    *is_loading = false;
                                }
                                cx.notify();
                            }
                        }
                        PlayerEvent::StatusUpdate(status) => {
                            if let Some(ref mut playback) = this.active_playback {
                                playback.status = status.clone();
                            }
                            if let CurrentPage::EmbeddedPlayer { is_loading, .. } = &mut this.current_page {
                                if this.active_playback.as_ref().map_or(false, |p| p.current_frame.is_some()) {
                                    *is_loading = false;
                                }
                            }
                            if status.duration_secs > 0.0 {
                                let dur = status.duration_secs as f32;
                                let pos = status.position_secs as f32;
                                this.seek_slider.update(cx, |s, cx| {
                                    if (s.max_value() - dur).abs() > 0.5 {
                                        *s = SliderState::new().min(0.0).max(dur).step(0.1).default_value(pos);
                                    } else if !this.is_seeking_drag {
                                        s.set_value(pos, _window, cx);
                                    }
                                });
                            }
                            let vol = status.volume as f32;
                            this.volume_slider.update(cx, |s, cx| {
                                s.set_value(vol, _window, cx);
                            });
                            cx.notify();
                        }
                        PlayerEvent::Finished | PlayerEvent::Error(_) => {
                            this.active_playback = None;
                            if let CurrentPage::EmbeddedPlayer { is_loading, .. } = &mut this.current_page {
                                *is_loading = false;
                            }
                            cx.notify();
                        }
                        PlayerEvent::Progress(pct) => {
                            let _ = hist_c.record_watched(
                                &aid_c,
                                ep_number,
                                &title_c,
                                None,
                                cov_c.as_deref(),
                                pct,
                            );
                            if pct >= 80.0 && !has_prefetched_next {
                                has_prefetched_next = true;
                                let s_pf = Arc::clone(&service_prefetch);
                                let t_pf = title_prefetch.clone();
                                let p_pf = prov_c.clone();
                                let next_ep_num = ep_number + 1.0;
                                tokio::spawn(async move {
                                    eprintln!("[silvani-ui] Pre-fetching stream for next episode ({:.0}) in background...", next_ep_num);
                                    let _ = s_pf.resolve_stream(&t_pf, &p_pf, None, next_ep_num, audio_type).await;
                                });
                            }
                        }
                        _ => {}
                    }
                }).ok();
            }
        }).detach();

        let on_event = Arc::new(move |event: PlayerEvent| {
            let _ = tx.send(event);
        });

        cx.spawn_in(window, async move |this, cx| {
            eprintln!(
                "[silvani-ui] Resolving stream: anime='{}', ep={:.0}, provider={}, audio={:?}, res={}",
                title, ep_number, prov, audio_type, res
            );
            let play_res = if prov == "muse_india" || prov == "ani_one_asia" {
                if let Some(eid) = ep_id {
                    eprintln!("[silvani-ui] Launching YouTube player with eid: {}", eid);
                    player.play_youtube(
                        &format!("{} - Episode {:.0}", title, ep_number),
                        &eid,
                        Some(&res),
                        on_event,
                    )
                } else {
                    let (_aid, eps) = service.fetch_episodes(&title, &prov, audio_type).await.unwrap_or_default();
                    let target_ep = eps.iter().find(|e| (e.number - ep_number).abs() < 0.1).or_else(|| eps.first());
                    if let Some(ep) = target_ep {
                        eprintln!("[silvani-ui] Launching YouTube player with fetched ep id: {}", ep.id);
                        player.play_youtube(
                            &format!("{} - Episode {:.0}", title, ep.number),
                            &ep.id,
                            Some(&res),
                            on_event,
                        )
                    } else {
                        eprintln!("[silvani-ui] No YouTube episodes found");
                        Err(silvani_player::PlayerError::NoStreams)
                    }
                }
            } else {
                let resolve_fut = service.resolve_stream(&title, &prov, ep_id.as_deref(), ep_number, audio_type);
                match tokio::time::timeout(std::time::Duration::from_secs(15), resolve_fut).await {
                    Ok(Ok(source)) => {
                        let stream_url = source.streams.first().map(|s| s.url.as_str()).unwrap_or("none");
                        eprintln!(
                            "[silvani-ui] Stream resolved successfully! streams: {}, first url: {}, subs: {}",
                            source.streams.len(),
                            stream_url,
                            source.subtitles.len()
                        );
                        player.play_stream(
                            &format!("{} - Episode {:.0}", title, ep_number),
                            &source,
                            Some(&res),
                            on_event,
                        )
                    }
                    Ok(Err(e)) => {
                        eprintln!("[silvani-ui] Stream resolve failed: {}", e);
                        Err(silvani_player::PlayerError::ProcessSpawn(
                            std::io::Error::other(e.to_string()),
                        ))
                    }
                    Err(_) => {
                        eprintln!("[silvani-ui] Stream resolve timed out after 15 seconds");
                        Err(silvani_player::PlayerError::ProcessSpawn(
                            std::io::Error::other("Stream resolve timed out (15s)"),
                        ))
                    }
                }
            };

            if let Err(err) = play_res {
                eprintln!("[silvani-ui] Playback launch error: {:?}", err);
                this.update_in(cx, |this, _window, cx| {
                    if let CurrentPage::EmbeddedPlayer { is_loading, .. } = &mut this.current_page {
                        *is_loading = false;
                    }
                    let err_str = match &err {
                        silvani_player::PlayerError::NoStreams => "Failed to load stream: No streams found".to_string(),
                        _ => "Failed to load stream".to_string(),
                    };
                    if let Some(ref mut pb) = this.active_playback {
                        pb.error = Some(err_str);
                    } else {
                        this.active_playback = Some(ActivePlaybackState {
                            title: title.clone(),
                            controller: None,
                            status: PlaybackStatus::default(),
                            current_frame: None,
                            error: Some(err_str),
                        });
                    }
                    cx.notify();
                }).ok();
            }
        }).detach();
    }

    pub fn toggle_player_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_player_fullscreen = !self.is_player_fullscreen;
        if self.is_player_fullscreen {
            if !window.is_fullscreen() {
                window.toggle_fullscreen();
            }
        } else {
            if window.is_fullscreen() {
                window.toggle_fullscreen();
            }
        }
        cx.notify();
    }
}

impl Render for SearchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let content = match &self.current_page {
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
                .on_settings(cx.listener(|this, _, _window, cx| {
                    this.current_page = CurrentPage::Settings;
                    this.current_category = AnimeCategory::Settings;
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
                .available_providers({
                    let filtered = self.stream_service.engine().filter_providers_by_audio(selected_audio.to_audio_type());
                    let enabled_list = self.app_settings.enabled_providers();
                    enabled_list.into_iter().filter(|(id, _)| {
                        filtered.iter().any(|p| p.name() == id || p.id() == id)
                    }).collect()
                })
                .on_back(cx.listener(|this, _, _window, cx| {
                    this.current_page = CurrentPage::Browse;
                    cx.notify();
                }))
                .on_play_episode(cx.listener(move |this, req: &PlayEpisodeRequest, window, cx| {
                    eprintln!(
                        "[silvani-ui] Episode clicked: anime='{}' (id: {}), ep={:.0}, provider={}, audio={:?}, res={}",
                        anime_title, anime_id, req.number, req.provider, req.audio, req.resolution
                    );
                    if let CurrentPage::AnimeDetails { item, scraped_episodes, .. } = &this.current_page {
                        let eps = scraped_episodes.clone();
                        let a_item = item.clone();
                        let target_ep = eps.iter().find(|e| (e.number - req.number).abs() < 0.1);
                        let ep_title = target_ep.and_then(|e| e.title.clone()).unwrap_or_else(|| format!("Episode {:.0}", req.number));
                        
                        this.current_page = CurrentPage::EmbeddedPlayer {
                            anime_item: a_item,
                            episodes: eps,
                            ep_number: req.number,
                            ep_title,
                            provider: req.provider.clone(),
                            audio: req.audio.to_string(),
                            resolution: req.resolution.clone(),
                            is_loading: true,
                        };
                        cx.notify();
                    }

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
                .on_audio_change(cx.listener(|this, new_audio: &SelectedAudio, window, cx| {
                    this.switch_audio(new_audio.clone(), window, cx);
                }))
                .on_resolution_change(cx.listener(|this, new_res: &str, _window, cx| {
                    if let CurrentPage::AnimeDetails { selected_resolution, .. } = &mut this.current_page {
                        *selected_resolution = new_res.to_string();
                        cx.notify();
                    }
                }))
                .into_any_element()
            }
            CurrentPage::EmbeddedPlayer {
                anime_item,
                episodes,
                ep_number,
                ep_title,
                provider,
                audio,
                resolution,
                is_loading,
            } => {
                let status = self.active_playback.as_ref().map(|p| p.status.clone()).unwrap_or_default();
                let controller = self.active_playback.as_ref().and_then(|p| p.controller.clone());
                let current_frame = self.active_playback.as_ref().and_then(|p| p.current_frame.clone());
                let a_item = (**anime_item).clone();
                let eps = episodes.clone();
                let prov = provider.clone();
                let aud = audio.clone();
                let res = resolution.clone();

                let a_back = a_item.clone();
                let a_switch = a_item.clone();

                let is_hud_visible = self.last_mouse_activity.elapsed() < std::time::Duration::from_millis(2500) || status.paused;



                let pb_error = self.active_playback.as_ref().and_then(|p| p.error.clone());

                let ep_retry_anime = a_item.clone();
                let ep_retry_ep_num = *ep_number;
                let ep_retry_prov = prov.clone();
                let ep_retry_aud_type = match aud.to_lowercase().as_str() {
                    "dub" => AudioType::Dub,
                    "hindi" => AudioType::Hindi,
                    _ => AudioType::Sub,
                };
                let ep_retry_res = res.clone();

                EmbeddedPlayerView::new(
                    a_item,
                    *ep_number,
                    ep_title.clone(),
                    prov.clone(),
                    aud.clone(),
                    res.clone(),
                    status,
                    controller,
                    current_frame,
                    *is_loading,
                    self.is_player_fullscreen,
                    is_hud_visible,
                    pb_error,
                    self.is_seeking_drag,
                    eps.clone(),
                    Arc::clone(&self.image_cache),
                    self.seek_slider.clone(),
                    self.volume_slider.clone(),
                )
                .on_retry(cx.listener(move |this: &mut SearchView, _, window, cx| {
                    this.play_episode(
                        &ep_retry_anime.id,
                        ep_retry_anime.display_title(),
                        ep_retry_ep_num,
                        None,
                        &ep_retry_prov,
                        ep_retry_aud_type,
                        &ep_retry_res,
                        ep_retry_anime.cover_image.clone(),
                        window,
                        cx,
                    );
                }))
                .on_mouse_move(cx.listener(|this, _, _window, cx| {
                    let was_hud_visible = this.last_mouse_activity.elapsed() < std::time::Duration::from_millis(2500);
                    this.last_mouse_activity = std::time::Instant::now();
                    if !was_hud_visible {
                        cx.notify();
                    }
                }))
                .on_toggle_fullscreen(cx.listener(|this, _, window, cx| {
                    this.toggle_player_fullscreen(window, cx);
                }))
                .on_back(cx.listener(move |this: &mut SearchView, _, window, cx| {
                    if this.is_player_fullscreen {
                        this.is_player_fullscreen = false;
                        window.toggle_fullscreen();
                    }
                    if let Some(ref pb) = this.active_playback {
                        if let Some(ref ctrl) = pb.controller {
                            let _ = ctrl.quit();
                        }
                    }
                    this.active_playback = None;
                    this.open_anime_details(a_back.clone(), window, cx);
                }))
                .on_switch_episode(cx.listener(move |this: &mut SearchView, target_ep: &f32, window, cx| {
                    let ep_val = *target_ep;
                    let aid = a_switch.id.clone();
                    let atitle = a_switch.display_title().to_string();
                    let cover = a_switch.cover_image.clone();
                    let ep_obj = eps.iter().find(|e| (e.number - ep_val).abs() < 0.1);
                    let eid = ep_obj.map(|e| e.id.clone());
                    let etitle = ep_obj.and_then(|e| e.title.clone()).unwrap_or_else(|| format!("Episode {:.0}", ep_val));

                    let aud_type = match aud.to_lowercase().as_str() {
                        "dub" => AudioType::Dub,
                        "hindi" => AudioType::Hindi,
                        _ => AudioType::Sub,
                    };

                    this.current_page = CurrentPage::EmbeddedPlayer {
                        anime_item: Box::new(a_switch.clone()),
                        episodes: eps.clone(),
                        ep_number: ep_val,
                        ep_title: etitle,
                        provider: prov.clone(),
                        audio: aud.clone(),
                        resolution: res.clone(),
                        is_loading: true,
                    };
                    cx.notify();

                    this.play_episode(
                        &aid,
                        &atitle,
                        ep_val,
                        eid,
                        &prov,
                        aud_type,
                        &res,
                        cover,
                        window,
                        cx,
                    );
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
                        let default_prov = this.app_settings.get_default_provider();
                        this.play_episode(
                            &aid,
                            &title,
                            ep,
                            None,
                            &default_prov,
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
            CurrentPage::Settings => {
                SettingsView::new(self.app_settings.clone())
                    .on_back(cx.listener(|this, _, _window, cx| {
                        this.current_page = CurrentPage::Browse;
                        this.current_category = AnimeCategory::Trending;
                        cx.notify();
                    }))
                    .on_theme_select(cx.listener(|this, theme_name: &str, _window, cx| {
                        settings::apply_theme(theme_name, cx);
                        this.app_settings = settings::load_settings();
                        cx.notify();
                    }))
                    .on_toggle_mode(cx.listener(|this, _, _window, cx| {
                        let new_mode = if Theme::global(cx).is_dark() {
                            ThemeMode::Light
                        } else {
                            ThemeMode::Dark
                        };
                        Theme::change(new_mode, None, cx);
                        this.app_settings = settings::load_settings();
                        cx.notify();
                    }))
                    .on_update_settings(cx.listener(|this, new_s: &AppSettings, _window, cx| {
                        let _ = settings::save_settings(new_s);
                        this.app_settings = new_s.clone();
                        cx.notify();
                    }))
                    .into_any_element()
            }
        };

        let mut shell = v_flex()
            .id("app-root-shell")
            .size_full()
            .relative()
            .track_focus(&self.focus_handle)
            .key_context("SearchView")
            .on_action(cx.listener(|this, _: &ToggleThemePicker, window, cx| {
                this.toggle_theme_picker(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReloadApp, window, cx| {
                this.reload(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if matches!(this.current_page, CurrentPage::EmbeddedPlayer { .. }) {
                    let key = event.keystroke.key.to_lowercase();
                    match key.as_str() {
                        "f" | "f11" => {
                            this.toggle_player_fullscreen(window, cx);
                        }
                        "escape" => {
                            if this.is_player_fullscreen {
                                this.toggle_player_fullscreen(window, cx);
                            }
                        }
                        _ => {
                            if let Some(ref pb) = this.active_playback {
                                if let Some(ref ctrl) = pb.controller {
                                    match key.as_str() {
                                        "space" | " " => {
                                            let _ = ctrl.toggle_pause();
                                            cx.notify();
                                        }
                                        "left" | "arrowleft" => {
                                            let _ = ctrl.seek_relative(-10.0);
                                            cx.notify();
                                        }
                                        "right" | "arrowright" => {
                                            let _ = ctrl.seek_relative(10.0);
                                            cx.notify();
                                        }
                                        "up" | "arrowup" => {
                                            let cur = ctrl.get_status().volume;
                                            let new_vol = (cur + 5.0).min(100.0);
                                            let _ = ctrl.set_volume(new_vol);
                                            if let Some(ref mut pb_mut) = this.active_playback {
                                                pb_mut.status.volume = new_vol;
                                            }
                                            cx.notify();
                                        }
                                        "down" | "arrowdown" => {
                                            let cur = ctrl.get_status().volume;
                                            let new_vol = (cur - 5.0).max(0.0);
                                            let _ = ctrl.set_volume(new_vol);
                                            if let Some(ref mut pb_mut) = this.active_playback {
                                                pb_mut.status.volume = new_vol;
                                            }
                                            cx.notify();
                                        }
                                        "m" => {
                                            let cur = ctrl.get_status().volume;
                                            let new_vol = if cur > 0.0 { 0.0 } else { 100.0 };
                                            let _ = ctrl.set_volume(new_vol);
                                            if let Some(ref mut pb_mut) = this.active_playback {
                                                pb_mut.status.volume = new_vol;
                                            }
                                            cx.notify();
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .child(content),
            );

        if let Some(ref picker) = self.theme_picker {
            shell = shell.child(
                div()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(theme.background.opacity(0.75))
                    .flex()
                    .justify_center()
                    .items_start()
                    .pt_16()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                        this.theme_picker = None;
                        window.focus(&this.focus_handle, cx);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w_72()
                            .h_72()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.popover)
                            .shadow_lg()
                            .overflow_hidden()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(
                                List::new(&picker.list)
                                    .search_placeholder("search")
                            )
                            .focus_trap("theme-picker-trap", &picker.trap_handle),
                    ),
            );
        }

        if !matches!(self.current_page, CurrentPage::EmbeddedPlayer { .. }) && let Some(ref playback) = self.active_playback {
            let title = playback.title.clone();
            let status = playback.status.clone();
            let controller = playback.controller.clone();

            shell = shell.child(
                div()
                    .absolute()
                    .bottom_4()
                    .left_4()
                    .right_4()
                    .occlude()
                    .child(
                        VideoPlayerControls::new("active-video-player-controls", title, status)
                            .controller(controller)
                            .on_close(cx.listener(|this, _, _window, cx| {
                                if let Some(ref playback) = this.active_playback {
                                    if let Some(ref ctrl) = playback.controller {
                                        let _ = ctrl.quit();
                                    }
                                }
                                this.active_playback = None;
                                cx.notify();
                            })),
                    ),
            );
        }

        shell
    }
}

pub fn run(http_client: Arc<dyn gpui::http_client::HttpClient>) {
    let _ = settings::ensure_config_files();
    let initial_settings = settings::load_settings();
    let theme_dir = settings::theme_dir();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .with_http_client(http_client.clone())
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_http_client(http_client);

            // Preload user themes from $HOME/.config/silvani/theme/
            settings::preload_custom_themes(cx);

            // Apply configured theme from setting.json
            settings::apply_theme(&initial_settings.theme, cx);

            // Watch user themes directory for live reload (Matugen wallpaper change auto-updates)
            settings::watch_theme_changes(cx);
            let _ = ThemeRegistry::watch_dir(theme_dir, cx, |_cx| {});

            cx.bind_keys([
                KeyBinding::new("ctrl-p", ToggleThemePicker, None),
                KeyBinding::new("cmd-p", ToggleThemePicker, None),
                KeyBinding::new("ctrl-r", ReloadApp, None),
                KeyBinding::new("cmd-r", ReloadApp, None),
            ]);

            let (window_handle, search_view) = gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| SearchView::new(window, cx))
            })
            .expect("Failed to open window");

            window_handle.update(cx, |_, window, cx| {
                window.activate_window();
                search_view.update(cx, |this, cx| {
                    window.focus(&this.focus_handle, cx);
                });
            }).ok();
        });
}


