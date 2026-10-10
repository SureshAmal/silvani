pub mod components;
pub mod history;
mod player_chrome;
mod episode_titles;
use player_chrome::{PlayerChromeState, PlayerCursor, IDLE_TIMEOUT};
pub mod settings;
pub mod views;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt;

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
    player_chrome: PlayerChromeState,
    player_idle_task: Option<Task<()>>,
    player_menu_open: bool,
    seek_slider: Entity<SliderState>,
    volume_slider: Entity<SliderState>,
    is_seeking_drag: bool,
    is_volume_drag: bool,
    pending_seek: Option<(f64, std::time::Instant)>,
    pending_restore: Option<(f64, bool, f64)>,
    playback_generation: Arc<AtomicU64>,
    language_request: u64,
    playback_resolutions: Vec<String>,
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
            InputState::new(window, cx).placeholder("Search episode number or title…")
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
                SliderEvent::Change(_) => {
                    // The slider owns the preview; playback time stays authoritative.
                    this.is_seeking_drag = true;
                }
                SliderEvent::Release(val) => {
                    this.is_seeking_drag = false;
                    let target_secs = val.start() as f64;
                    if let Some(ref mut pb) = this.active_playback {
                        pb.status.position_secs = target_secs;
                        if let Some(ref ctrl) = pb.controller {
                            if ctrl.seek_absolute(target_secs).is_ok() {
                                this.pending_seek = Some((target_secs, std::time::Instant::now()));
                            }
                        }
                    }
                }
            }
            this.record_player_activity(_window, cx);
            cx.notify();
        });

        let vol_sub = cx.subscribe_in(&volume_slider, window, |this, _, event: &SliderEvent, _window, cx| {
            let target_vol = match event {
                SliderEvent::Change(val) => { this.is_volume_drag = true; val.end() as f64 }
                SliderEvent::Release(val) => { this.is_volume_drag = false; val.end() as f64 }
            };
            if let Some(ref mut pb) = this.active_playback {
                pb.status.volume = target_vol;
                if let Some(ref ctrl) = pb.controller {
                    let _ = ctrl.set_volume(target_vol);
                }
            }
            this.record_player_activity(_window, cx);
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
            player_chrome: PlayerChromeState::new(std::time::Instant::now()),
            player_idle_task: None,
            player_menu_open: false,
            seek_slider,
            volume_slider,
            is_seeking_drag: false,
            is_volume_drag: false,
            pending_seek: None,
            pending_restore: None,
            playback_generation: Arc::new(AtomicU64::new(0)),
            language_request: 0,
            playback_resolutions: vec!["auto".into()],
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
                            this.enrich_episode_titles(_window, cx);
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
                            this.enrich_episode_titles(_window, cx);
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
                            this.enrich_episode_titles(_window, cx);
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
                    this.enrich_episode_titles(_window, cx);
                }
            }).ok();
        }).detach();
    }

    fn enrich_episode_titles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (item, provider) = match &self.current_page {
            CurrentPage::AnimeDetails {
                item,
                selected_provider,
                ..
            } => ((**item).clone(), selected_provider.clone()),
            CurrentPage::EmbeddedPlayer {
                anime_item,
                provider,
                ..
            } => ((**anime_item).clone(), provider.clone()),
            _ => return,
        };
        // One Pace uses an edited cut with its own episode numbering.
        if provider == "onepace" {
            return;
        }
        let meta = Arc::clone(&self.meta_client);
        cx.spawn_in(window, async move |this, cx| {
            match meta.get_episode_titles(&item).await {
                Ok(titles) => {
                    this.update_in(cx, |this, _, cx| {
                        match &mut this.current_page {
                            CurrentPage::AnimeDetails {
                                item: current,
                                scraped_episodes,
                                selected_provider,
                                ..
                            } if current.id == item.id && *selected_provider == provider => {
                                episode_titles::enrich(scraped_episodes, &titles);
                            }
                            CurrentPage::EmbeddedPlayer {
                                anime_item,
                                episodes,
                                ep_number,
                                ep_title,
                                provider: current_provider,
                                ..
                            } if anime_item.id == item.id && *current_provider == provider => {
                                episode_titles::enrich(episodes, &titles);
                                if let Some(title) = episodes
                                    .iter()
                                    .find(|episode| episode.number == *ep_number)
                                    .and_then(|episode| episode.title.clone())
                                {
                                    *ep_title = title;
                                }
                            }
                            _ => return,
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(error) => eprintln!(
                    "[silvani-ui] Episode title metadata unavailable for {}: {error}",
                    item.id
                ),
            }
        })
        .detach();
    }

    fn record_player_activity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.current_page, CurrentPage::EmbeddedPlayer { .. }) { return; }
        let was_visible = self.player_chrome.is_visible();
        self.player_chrome.activity(std::time::Instant::now());
        self.player_idle_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(IDLE_TIMEOUT).await;
            this.update_in(cx, |this, window, cx| {
                if !matches!(this.current_page, CurrentPage::EmbeddedPlayer { .. }) { return; }
                let interacting = this.is_seeking_drag || this.is_volume_drag || this.player_menu_open;
                if this.player_chrome.expire(std::time::Instant::now(), interacting) {
                    if this.player_chrome.should_hide_cursor() && window.is_window_hovered()
                        && window.is_window_active()
                        && let Some(cursor) = cx.try_global::<PlayerCursor>()
                    {
                        cursor.hide_until_mouse_moves();
                    }
                    cx.notify();
                }
            }).ok();
        }));
        if !was_visible { cx.notify(); }
    }

    /// A provider language may be a different stream, rather than a muxed audio track.
    pub fn switch_playback_audio(
        &mut self,
        audio: AudioType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let CurrentPage::EmbeddedPlayer {
            anime_item,
            ep_number,
            provider,
            resolution,
            audio: current_audio,
            ..
        } = &self.current_page
        else {
            return;
        };
        if current_audio.eq_ignore_ascii_case(audio.as_str()) {
            return;
        }
        let item = (**anime_item).clone();
        let number = *ep_number;
        let provider = provider.clone();
        let resolution = resolution.clone();
        let service = Arc::clone(&self.stream_service);
        let generation = self.playback_generation.load(Ordering::SeqCst);
        self.language_request += 1;
        let request = self.language_request;
        cx.spawn_in(window, async move |this, cx| {
            // Resolve before replacing the current player, so unavailable audio does not interrupt it.
            let result = tokio::time::timeout(std::time::Duration::from_secs(20), async {
                let (_, episodes) = service.fetch_episodes(item.display_title(), &provider, audio).await?;
                let episode = episodes.iter().find(|ep| (ep.number - number).abs() < 0.1)
                    .ok_or_else(|| silvani_scraper::ScraperError::NotFound("This episode is unavailable in the selected language".into()))?;
                let id = episode.id.clone();
                let title = episode.title.clone().unwrap_or_else(|| format!("Episode {number:.0}"));
                service.resolve_stream(item.display_title(), &provider, Some(&id), number, audio).await?;
                Ok::<_, silvani_scraper::ScraperError>((episodes, id, title))
            }).await;
            this.update_in(cx, |this, window, cx| {
                if this.playback_generation.load(Ordering::SeqCst) != generation || this.language_request != request {
                    return;
                }
                if !matches!(&this.current_page, CurrentPage::EmbeddedPlayer { anime_item, ep_number, .. }
                    if anime_item.id == item.id && (*ep_number - number).abs() < 0.1) { return; }
                match result {
                    Ok(Ok((episodes, id, title))) => {
                        let restore = this.active_playback.as_ref().map(|p| (p.status.position_secs, p.status.paused, p.status.volume));
                        this.current_page = CurrentPage::EmbeddedPlayer {
                            anime_item: Box::new(item.clone()), episodes, ep_number: number, ep_title: title,
                            provider: provider.clone(), audio: audio.as_str().into(), resolution: resolution.clone(), is_loading: true,
                        };
                        this.play_episode(&item.id, item.display_title(), number, Some(id), &provider, audio,
                            &resolution, item.cover_image.clone(), window, cx);
                        this.pending_restore = restore;
                        this.enrich_episode_titles(window, cx);
                    }
                    Ok(Err(error)) => { window.push_notification(Notification::warning(error.to_string()), cx); }
                    Err(_) => { window.push_notification(Notification::warning("Audio change timed out. Try again."), cx); }
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    fn switch_playback_resolution(&mut self, quality: &str, window: &mut Window, cx: &mut Context<Self>) {
        let CurrentPage::EmbeddedPlayer { anime_item, episodes, ep_number, provider, audio, resolution, is_loading, .. } = &mut self.current_page else { return; };
        if quality.eq_ignore_ascii_case(resolution) { return; }
        let item = (**anime_item).clone();
        let number = *ep_number;
        let id = episodes.iter().find(|episode| (episode.number - number).abs() < 0.1).map(|episode| episode.id.clone());
        let provider = provider.clone();
        let audio = match audio.to_lowercase().as_str() { "dub" => AudioType::Dub, "hindi" => AudioType::Hindi, _ => AudioType::Sub };
        *resolution = quality.into();
        *is_loading = true;
        let restore = self.active_playback.as_ref().map(|playback| (playback.status.position_secs, playback.status.paused, playback.status.volume));
        self.play_episode(&item.id, item.display_title(), number, id, &provider, audio, quality, item.cover_image.clone(), window, cx);
        self.pending_restore = restore;
        cx.notify();
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
        self.player_menu_open = false;
        self.record_player_activity(window, cx);
        let generation = self.playback_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let playback_epoch = Arc::clone(&self.playback_generation);
        self.is_seeking_drag = false;
        self.is_volume_drag = false;
        self.pending_seek = None;
        self.pending_restore = None;
        self.seek_slider.update(cx, |slider, cx| {
            *slider = SliderState::new()
                .min(0.0)
                .max(1.0)
                .step(0.1)
                .default_value(0.0);
            cx.notify();
        });
        let title = anime_title.to_string();
        let service = Arc::clone(&self.stream_service);
        let player = Arc::clone(&self.player);
        let prov = provider.to_string();
        let res = resolution.to_string();
        self.playback_resolutions = if provider == "muse_india" || provider == "ani_one_asia" {
            vec!["auto", "1080p", "720p", "480p", "360p"].into_iter().map(str::to_string).collect()
        } else { vec!["auto".into()] };
        let history = self.watch_history.clone();
        let anime_id = anime_id.to_string();
        if let Some(ref pb) = self.active_playback {
            if let Some(ref ctrl) = pb.controller {
                let _ = ctrl.quit();
            }
        }
        self.active_playback = Some(ActivePlaybackState {
            title: title.clone(),
            controller: None,
            status: PlaybackStatus::default(),
            current_frame: None,
            error: None,
        });

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
                    if this.playback_generation.load(Ordering::SeqCst) != generation {
                        match event {
                            PlayerEvent::Started(controller) => { let _ = PlayerController::Socket(controller).quit(); }
                            PlayerEvent::EmbeddedStarted(controller) => { let _ = controller.quit(); }
                            _ => {}
                        }
                        return;
                    }
                    match event {
                        PlayerEvent::Started(controller) => {
                            if let CurrentPage::EmbeddedPlayer { is_loading, .. } = &mut this.current_page { *is_loading = false; }
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
                                if this.active_playback.as_ref().is_some_and(|p| p.current_frame.is_some()) {
                                    *is_loading = false;
                                }
                            }
                            if let Some((position, paused, volume)) = this.pending_restore
                                && status.duration_secs > 0.0
                                && this.active_playback.as_ref().is_some_and(|p| p.current_frame.is_some() || matches!(p.controller, Some(PlayerController::Socket(_))))
                                && let Some(controller) = this.active_playback.as_ref().and_then(|p| p.controller.as_ref())
                            {
                                let _ = controller.seek_absolute(position.min(status.duration_secs));
                                if paused { let _ = controller.pause(); }
                                let _ = controller.set_volume(volume);
                                this.pending_seek = Some((position, std::time::Instant::now()));
                                this.pending_restore = None;
                            }
                            if this.pending_seek.is_some_and(|(target, sent)| (target - status.position_secs).abs() < 1.0 || sent.elapsed().as_secs_f32() > 3.0) {
                                this.pending_seek = None;
                            }
                            if status.duration_secs > 0.0 && !this.is_seeking_drag && this.pending_seek.is_none() {
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
                            if !this.is_volume_drag {
                                this.volume_slider.update(cx, |s, cx| { s.set_value(vol, _window, cx); });
                            }
                            cx.notify();
                        }
                        PlayerEvent::Error(error) => {
                            if let Some(playback) = &mut this.active_playback {
                                playback.error = Some(error);
                                playback.controller = None;
                            }
                            if let CurrentPage::EmbeddedPlayer { is_loading, .. } = &mut this.current_page { *is_loading = false; }
                            cx.notify();
                        }
                        PlayerEvent::Finished => {
                            if let Some(playback) = &mut this.active_playback {
                                playback.controller = None;
                                playback.status.paused = true;
                            }
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

        let event_epoch = Arc::clone(&playback_epoch);
        let on_event = Arc::new(move |event: PlayerEvent| {
            if event_epoch.load(Ordering::SeqCst) != generation {
                match event {
                    PlayerEvent::Started(controller) => {
                        let _ = PlayerController::Socket(controller).quit();
                    }
                    PlayerEvent::EmbeddedStarted(controller) => {
                        let _ = controller.quit();
                    }
                    _ => {}
                }
                return;
            }
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
                    if playback_epoch.load(Ordering::SeqCst) != generation { return; }
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
                        if playback_epoch.load(Ordering::SeqCst) != generation { return; }
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
                        this.update_in(cx, |this, _, cx| {
                            if this.playback_generation.load(Ordering::SeqCst) != generation { return; }
                            let mut qualities = vec!["auto".to_string()];
                            for stream in &source.streams {
                                if !qualities.iter().any(|quality| quality.eq_ignore_ascii_case(&stream.quality)) {
                                    qualities.push(stream.quality.clone());
                                }
                            }
                            if let CurrentPage::EmbeddedPlayer { resolution, .. } = &mut this.current_page
                                && !qualities.iter().any(|quality| quality.eq_ignore_ascii_case(resolution))
                            { *resolution = "auto".into(); }
                            this.playback_resolutions = qualities;
                            cx.notify();
                        }).ok();
                        let stream_url = source.streams.first().map(|s| s.url.as_str()).unwrap_or("none");
                        eprintln!(
                            "[silvani-ui] Stream resolved successfully! streams: {}, first url: {}, subs: {}",
                            source.streams.len(),
                            stream_url,
                            source.subtitles.len()
                        );
                        if playback_epoch.load(Ordering::SeqCst) != generation { return; }
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
                    if this.playback_generation.load(Ordering::SeqCst) != generation { return; }
                    if let CurrentPage::EmbeddedPlayer { is_loading, .. } = &mut this.current_page {
                        *is_loading = false;
                    }
                    let err_str = match &err {
                        silvani_player::PlayerError::NoStreams => "Failed to load stream: No streams found".to_string(),
                        _ => err.to_string(),
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

                let is_hud_visible = self.player_chrome.is_visible();



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
                    eps.clone(),
                    Arc::clone(&self.image_cache),
                    self.seek_slider.clone(),
                    self.volume_slider.clone(),
                )
                .playlist_search(self.episode_search_input.clone())
                .resolutions(self.playback_resolutions.clone(), cx.listener(|this, quality: &str, window, cx| {
                    this.switch_playback_resolution(quality, window, cx);
                }))
                .audio_options(self.stream_service.engine().providers().iter()
                    .find(|p| silvani_scraper::engine::provider_matches(p.id(), &prov))
                    .map(|p| p.supported_audio()).unwrap_or_default())
                .on_audio_change(cx.listener(|this, audio: &AudioType, window, cx| {
                    this.switch_playback_audio(*audio, window, cx);
                }))
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
                .on_mouse_move(cx.listener(|this, _, window, cx| {
                    this.record_player_activity(window, cx);
                }))
                .on_player_hover(cx.listener(|this, inside: &bool, window, cx| {
                    this.player_chrome.set_pointer_inside(*inside);
                    if *inside { this.record_player_activity(window, cx); }
                }))
                .on_activity(cx.listener(|this, _: &(), window, cx| {
                    this.record_player_activity(window, cx);
                }))
                .on_menu_open_change(cx.listener(|this, open: &bool, window, cx| {
                    this.player_menu_open = *open;
                    this.record_player_activity(window, cx);
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
                    this.playback_generation.fetch_add(1, Ordering::SeqCst);
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
                    if this.episode_search_input.read(cx).focus_handle(cx).is_focused(window) { return; }
                    this.record_player_activity(window, cx);
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
                            .sliders(self.seek_slider.clone(), self.volume_slider.clone())
                            .on_close(cx.listener(|this, _, _window, cx| {
                                if let Some(ref playback) = this.active_playback {
                                    if let Some(ref ctrl) = playback.controller {
                                        let _ = ctrl.quit();
                                    }
                                }
                                this.playback_generation.fetch_add(1, Ordering::SeqCst);
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

    let platform = gpui_kit::platform::current_platform(false);
    let player_cursor = PlayerCursor::new(platform.clone());
    gpui_kit::Application::with_platform(platform)
        .with_assets(gpui_kit::assets::AllAssets)
        .with_http_client(http_client.clone())
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_global(player_cursor);
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


