use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::input::InputState;
use gpui_kit::component::label::Label;
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::prelude::*;
use gpui_kit::*;
use silvani_meta::image_cache::ImageCacheManager;
use silvani_meta::models::AnimeItem;

use crate::components::anime_card::AnimeCard;
use crate::components::category_tabs::{AnimeCategory, CategoryTabs};
use crate::components::search_bar::SearchBar;

pub type ClickHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;
pub type CategoryHandler = Option<Rc<dyn Fn(&AnimeCategory, &mut Window, &mut App)>>;
pub type AnimeItemHandler = Option<Rc<dyn Fn(&AnimeItem, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct BrowseView {
    search_input: Entity<InputState>,
    current_category: AnimeCategory,
    items: Vec<AnimeItem>,
    is_loading: bool,
    image_cache: Arc<ImageCacheManager>,
    on_search: ClickHandler,
    on_history: ClickHandler,
    on_settings: ClickHandler,
    on_select_category: CategoryHandler,
    on_open_details: AnimeItemHandler,
}

impl BrowseView {
    pub fn new(
        search_input: &Entity<InputState>,
        current_category: AnimeCategory,
        items: Vec<AnimeItem>,
        is_loading: bool,
        image_cache: Arc<ImageCacheManager>,
    ) -> Self {
        Self {
            search_input: search_input.clone(),
            current_category,
            items,
            is_loading,
            image_cache,
            on_search: None,
            on_history: None,
            on_settings: None,
            on_select_category: None,
            on_open_details: None,
        }
    }

    pub fn on_search(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_search = Some(Rc::new(handler));
        self
    }

    pub fn on_history(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_history = Some(Rc::new(handler));
        self
    }

    pub fn on_settings(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_settings = Some(Rc::new(handler));
        self
    }

    pub fn on_select_category(
        mut self,
        handler: impl Fn(&AnimeCategory, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select_category = Some(Rc::new(handler));
        self
    }

    pub fn on_open_details(
        mut self,
        handler: impl Fn(&AnimeItem, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_details = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for BrowseView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let on_open = self.on_open_details.clone();
        let on_search = self.on_search.clone();
        let on_history = self.on_history.clone();
        let on_settings = self.on_settings.clone();
        let on_select = self.on_select_category.clone();
        let image_cache = Arc::clone(&self.image_cache);

        let mut search_bar = SearchBar::new("browse-search-bar", &self.search_input);
        if let Some(h) = on_search {
            search_bar = search_bar.on_search(move |ev, win, cx| h(ev, win, cx));
        }
        if let Some(h) = on_history {
            search_bar = search_bar.on_history(move |ev, win, cx| h(ev, win, cx));
        }
        if let Some(h) = on_settings {
            search_bar = search_bar.on_settings(move |ev, win, cx| h(ev, win, cx));
        }

        let mut category_tabs = CategoryTabs::new("browse-category-tabs", self.current_category.clone());
        if let Some(h) = on_select {
            category_tabs = category_tabs.on_select(move |cat, win, cx| h(cat, win, cx));
        }

        v_flex()
            .id("browse-view-container")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .p_4()
            .gap_4()
            .child(search_bar)
            .child(category_tabs)
            .child(
                v_flex()
                    .id("browse-cards-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .when(self.is_loading, |d| {
                        d.child(
                            h_flex()
                                .flex_wrap()
                                .gap_4()
                                .children((0..12).map(|_| {
                                    Skeleton::new()
                                        .w_48()
                                        .h_72()
                                        .rounded_lg()
                                })),
                        )
                    })
                    .when(!self.is_loading && self.items.is_empty(), |d| {
                        d.child(
                            v_flex()
                                .items_center()
                                .justify_center()
                                .h_72()
                                .gap_2()
                                .child(
                                    Label::new("No anime found.")
                                        .text_lg()
                                        .text_color(theme.muted_foreground),
                                ),
                        )
                    })
                    .when(!self.is_loading && !self.items.is_empty(), |d| {
                        d.child(
                            h_flex()
                                .flex_wrap()
                                .gap_4()
                                .children(self.items.into_iter().map(|item| {
                                    let item_c = item.clone();
                                    let cb = on_open.clone();
                                    let cached = image_cache.get_cached_path(&item.id);

                                    AnimeCard::new(
                                        ElementId::Name(format!("anime-card-{}", item.id).into()),
                                        item,
                                    )
                                    .cached_path(cached)
                                    .when_some(cb, move |c, handler| {
                                        c.on_click(move |_ev, win, cx| handler(&item_c, win, cx))
                                    })
                                })),
                        )
                    }),
            )
    }
}
