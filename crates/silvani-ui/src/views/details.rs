use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::StyledExt;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::pagination::Pagination;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use silvani_meta::image_cache::ImageCacheManager;
use silvani_meta::models::AnimeItem;
use silvani_scraper::models::Episode;

use crate::components::episode_item::EpisodeItem;
use crate::components::season_selector::SeasonSelector;
use crate::components::stream_settings_bar::{
    AudioChangeHandler, ProviderChangeHandler, ResolutionChangeHandler, SelectedAudio,
    StreamSettingsBar,
};
use crate::history::WatchHistoryManager;

#[derive(Clone)]
pub struct PlayEpisodeRequest {
    pub number: f32,
    pub ep_id: Option<String>,
    pub provider: String,
    pub audio: SelectedAudio,
    pub resolution: String,
}

pub type ClickHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;
pub type PlayHandler = Option<Rc<dyn Fn(&PlayEpisodeRequest, &mut Window, &mut App)>>;
pub type SeasonHandler = Option<Rc<dyn Fn(&str, &mut Window, &mut App)>>;
pub type PageHandler = Option<Rc<dyn Fn(&usize, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct DetailsView {
    item: AnimeItem,
    scraped_episodes: Vec<Episode>,
    is_loading_episodes: bool,
    selected_provider: String,
    selected_audio: SelectedAudio,
    selected_resolution: String,
    episode_search_input: Entity<InputState>,
    current_episode_page: usize,
    episodes_per_page: usize,
    watch_history: WatchHistoryManager,
    image_cache: Arc<ImageCacheManager>,
    on_back: ClickHandler,
    on_play_episode: PlayHandler,
    on_select_season: SeasonHandler,
    on_page_change: PageHandler,
    on_audio_change: AudioChangeHandler,
    on_provider_change: ProviderChangeHandler,
    on_resolution_change: ResolutionChangeHandler,
}

impl DetailsView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        item: AnimeItem,
        scraped_episodes: Vec<Episode>,
        is_loading_episodes: bool,
        selected_provider: impl Into<String>,
        selected_audio: SelectedAudio,
        selected_resolution: impl Into<String>,
        episode_search_input: &Entity<InputState>,
        current_episode_page: usize,
        watch_history: WatchHistoryManager,
        image_cache: Arc<ImageCacheManager>,
    ) -> Self {
        Self {
            item,
            scraped_episodes,
            is_loading_episodes,
            selected_provider: selected_provider.into(),
            selected_audio,
            selected_resolution: selected_resolution.into(),
            episode_search_input: episode_search_input.clone(),
            current_episode_page,
            episodes_per_page: 48,
            watch_history,
            image_cache,
            on_back: None,
            on_play_episode: None,
            on_select_season: None,
            on_page_change: None,
            on_audio_change: None,
            on_provider_change: None,
            on_resolution_change: None,
        }
    }

    pub fn on_back(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_back = Some(Rc::new(handler));
        self
    }

    pub fn on_play_episode(
        mut self,
        handler: impl Fn(&PlayEpisodeRequest, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_play_episode = Some(Rc::new(handler));
        self
    }

    pub fn on_select_season(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select_season = Some(Rc::new(handler));
        self
    }

    pub fn on_page_change(
        mut self,
        handler: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_page_change = Some(Rc::new(handler));
        self
    }

    pub fn on_audio_change(
        mut self,
        handler: impl Fn(&SelectedAudio, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_audio_change = Some(Rc::new(handler));
        self
    }

    pub fn on_provider_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_provider_change = Some(Rc::new(handler));
        self
    }

    pub fn on_resolution_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resolution_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DetailsView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let title = self.item.display_title().to_string();
        let season = self.item.season_display();
        let score_opt = self.item.score_display();
        let status = self.item.status.clone().unwrap_or_else(|| "Unknown".into());
        let studio = self.item.studio.clone();
        let genres = self.item.genres.clone();
        let clean_desc = self.item.clean_description();
        let on_back = self.on_back.clone();
        let on_play = self.on_play_episode.clone();
        let on_season = self.on_select_season.clone();
        let on_page = self.on_page_change.clone();
        let prov_name = self.selected_provider.clone();
        let cur_audio = self.selected_audio.clone();
        let cur_res = self.selected_resolution.clone();

        // Cached poster path fallback
        let poster_source: Option<ImageSource> = match self.image_cache.get_cached_path(&self.item.id) {
            Some(p) if p.exists() => Some(ImageSource::from(p)),
            _ => self.item.cover_image.as_ref().map(|url| ImageSource::from(url.clone())),
        };

        // Filter episodes based on user's search query (Real scraped episodes only)
        let query = self.episode_search_input.read(cx).value().trim().to_lowercase();
        let all_episodes = self.scraped_episodes.clone();

        let filtered_episodes: Vec<Episode> = if query.is_empty() {
            all_episodes
        } else {
            all_episodes
                .into_iter()
                .filter(|ep| {
                    let ep_num_str = format!("{}", ep.number);
                    ep_num_str.contains(&query)
                        || ep
                            .title
                            .as_ref()
                            .map(|t| t.to_lowercase().contains(&query))
                            .unwrap_or(false)
                })
                .collect()
        };

        let total_episodes = filtered_episodes.len();
        let total_pages = ((total_episodes as f32) / (self.episodes_per_page as f32))
            .ceil()
            .max(1.0) as usize;
        let current_page = self.current_episode_page.clamp(1, total_pages);
        let start_idx = (current_page - 1) * self.episodes_per_page;
        let end_idx = (start_idx + self.episodes_per_page).min(total_episodes);
        let page_episodes = if start_idx < total_episodes {
            &filtered_episodes[start_idx..end_idx]
        } else {
            &[]
        };

        let last_watched_num = self.watch_history.get_last_watched_episode(&self.item.id);

        v_flex()
            .id("details-view-container")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .p_6()
            .gap_5()
            .overflow_y_scroll()
            // Top: Back Navigation
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .child(
                        Button::new("back-btn")
                            .outline()
                            .label("← Back to Browse")
                            .when_some(on_back, |b, handler| {
                                b.on_click(move |ev, win, cx| handler(ev, win, cx))
                            }),
                    )
                    .when_some(last_watched_num, |d, ep_num| {
                        d.child(
                            Tag::info().child(format!("Resume: Ep {}", ep_num)),
                        )
                    }),
            )
            // Header Section: Cover Poster + Anime Metadata (Strictly min_w_0 to avoid overflow)
            .child(
                h_flex()
                    .gap_6()
                    .w_full()
                    .when_some(poster_source, |d, src| {
                        d.child(
                            img(src)
                                .w_56()
                                .h_72()
                                .rounded(theme.radius)
                                .object_fit(ObjectFit::Cover)
                                .flex_shrink_0(),
                        )
                    })
                    .child(
                        v_flex()
                            .gap_3()
                            .flex_1()
                            .min_w_0()
                            .child(
                                Label::new(title.clone())
                                    .text_2xl()
                                    .font_bold()
                                    .line_clamp(2),
                            )
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(Tag::secondary().child(season))
                                    .when_some(score_opt, |d, s| {
                                        d.child(Tag::primary().child(format!("★ {}", s)))
                                    })
                                    .child(Tag::info().child(status))
                                    .when_some(studio, |d, st| {
                                        d.child(Tag::secondary().child(format!("Studio: {}", st)))
                                    })
                                    .children(genres.into_iter().map(|g| {
                                        Tag::secondary().child(g)
                                    })),
                            )
                            // Clean Wrapped Synopsis using TextView::markdown with min_w_0
                            .child(
                                v_flex()
                                    .gap_1()
                                    .w_full()
                                    .min_w_0()
                                    .child(
                                        Label::new("Synopsis")
                                            .text_sm()
                                            .font_semibold()
                                            .text_color(theme.muted_foreground),
                                    )
                                    .child(
                                        TextView::markdown("anime-synopsis-md", clean_desc)
                                            .w_full()
                                            .selectable(true),
                                    ),
                            ),
                    ),
            )
            // Franchise Seasons & Relations
            .child(
                SeasonSelector::new("franchise-seasons", title)
                    .relations(self.item.relations.clone())
                    .when_some(on_season, |s, handler| {
                        s.on_select(move |rel_id, win, cx| handler(rel_id, win, cx))
                    }),
            )
            // Stream Settings Bar: Audio (Sub/Dub/Hindi), Provider, and Quality/Resolution Selectors
            .child({
                let mut bar = StreamSettingsBar::new(
                    "stream-settings-bar",
                    self.selected_audio.clone(),
                    self.selected_provider.clone(),
                    self.selected_resolution.clone(),
                );
                if let Some(h) = self.on_audio_change {
                    bar = bar.on_audio_change(move |audio, win, cx| h(audio, win, cx));
                }
                if let Some(h) = self.on_provider_change {
                    bar = bar.on_provider_change(move |prov, win, cx| h(prov, win, cx));
                }
                if let Some(h) = self.on_resolution_change {
                    bar = bar.on_resolution_change(move |res, win, cx| h(res, win, cx));
                }
                bar
            })
            // Episodes Header, Search Filter, and Pagination Controls
            .child(
                v_flex()
                    .gap_3()
                    .w_full()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        Label::new(format!("Episodes ({})", total_episodes))
                                            .text_lg()
                                            .font_bold(),
                                    )
                                    .child(
                                        Tag::primary()
                                            .child(format!("Provider: {}", prov_name))
                                    )
                                    .when(self.is_loading_episodes, |d| {
                                        d.child(Spinner::new())
                                            .child(Label::new("Fetching provider episodes...").text_xs())
                                    }),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .w_64()
                                    .child(
                                        Input::new(&self.episode_search_input)
                                            .cleanable(true),
                                    ),
                            ),
                    )
                    // Pagination Bar (when more than 1 page)
                    .when(total_pages > 1, |d| {
                        let page_cb = on_page.clone();
                        d.child(
                            h_flex()
                                .justify_center()
                                .w_full()
                                .child(
                                    Pagination::new("details-episode-pagination")
                                        .current_page(current_page)
                                        .total_pages(total_pages)
                                        .when_some(page_cb, |p, handler| {
                                            p.on_click(move |new_page, win, cx| {
                                                handler(new_page, win, cx);
                                            })
                                        }),
                                ),
                        )
                    }),
            )
            // Paginated Episodes Grid or Loading/Empty State
            .when(self.is_loading_episodes, |d| {
                d.child(
                    v_flex()
                        .items_center()
                        .justify_center()
                        .p_8()
                        .gap_3()
                        .w_full()
                        .child(Spinner::new().large())
                        .child(
                            Label::new(format!("Searching available episodes on {} ({:?})...", prov_name, cur_audio))
                                .text_color(theme.muted_foreground),
                        ),
                )
            })
            .when(!self.is_loading_episodes && page_episodes.is_empty(), |d| {
                d.child(
                    v_flex()
                        .items_center()
                        .justify_center()
                        .p_8()
                        .gap_3()
                        .w_full()
                        .child(
                            Label::new(format!("No episodes found on {} for {:?}.", prov_name, cur_audio))
                                .font_semibold()
                                .text_color(theme.muted_foreground),
                        )
                        .child(
                            Label::new("Try switching the Audio option (Sub / Dub / Hindi Dub) or select a different Provider above.")
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        ),
                )
            })
            .when(!self.is_loading_episodes && !page_episodes.is_empty(), |d| {
                d.child(
                    h_flex()
                        .flex_wrap()
                        .gap_3()
                        .w_full()
                        .children(page_episodes.iter().map(|ep| {
                            let ep_num = ep.number;
                            let ep_id = Some(ep.id.clone());
                            let prov = prov_name.clone();
                            let audio = cur_audio.clone();
                            let res = cur_res.clone();
                            let progress = self.watch_history.get_progress(&self.item.id, ep_num);
                            let play_cb = on_play.clone();

                            EpisodeItem::new(
                                ElementId::Name(format!("ep-item-{}", ep_num).into()),
                                ep_num,
                            )
                            .title(ep.title.clone())
                            .progress(progress)
                            .when_some(play_cb, move |item, handler| {
                                let req = PlayEpisodeRequest {
                                    number: ep_num,
                                    ep_id: ep_id.clone(),
                                    provider: prov.clone(),
                                    audio: audio.clone(),
                                    resolution: res.clone(),
                                };
                                item.on_play(move |_ev, win, cx| {
                                    handler(&req, win, cx);
                                })
                            })
                        })),
                )
            })
    }
}
