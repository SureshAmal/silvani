use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::h_flex;
use gpui_kit::prelude::*;
use gpui_kit::*;

pub const DEFAULT_GENRES: [&str; 10] = [
    "Action", "Adventure", "Comedy", "Drama", "Fantasy",
    "Horror", "Mystery", "Romance", "Sci-Fi", "Thriller",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimeCategory {
    Trending,
    PopularSeason,
    Upcoming,
    TopRated,
    History,
    Genre(String),
    SearchResults(String),
}

pub type CategorySelectHandler = Option<Rc<dyn Fn(&AnimeCategory, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct CategoryTabs {
    id: ElementId,
    current_category: AnimeCategory,
    on_select: CategorySelectHandler,
}

impl CategoryTabs {
    pub fn new(id: impl Into<ElementId>, current_category: AnimeCategory) -> Self {
        Self {
            id: id.into(),
            current_category,
            on_select: None,
        }
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(&AnimeCategory, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CategoryTabs {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let current = self.current_category.clone();
        let on_select = self.on_select.clone();

        let make_btn = |id: &'static str, label: &'static str, cat: AnimeCategory| {
            let is_active = current == cat;
            let cb = on_select.clone();
            Button::new(id)
                .label(label)
                .when(is_active, |b| b.primary())
                .when_some(cb, move |b, handler| {
                    let cat_clone = cat.clone();
                    b.on_click(move |_ev, win, cx| handler(&cat_clone, win, cx))
                })
        };

        h_flex()
            .id(self.id)
            .items_center()
            .gap_2()
            .w_full()
            .overflow_x_scroll()
            .child(make_btn("tab-trending", "Trending", AnimeCategory::Trending))
            .child(make_btn("tab-season", "Popular Season", AnimeCategory::PopularSeason))
            .child(make_btn("tab-upcoming", "Upcoming", AnimeCategory::Upcoming))
            .child(make_btn("tab-top", "Top Rated", AnimeCategory::TopRated))
            .child(make_btn("tab-history", "History", AnimeCategory::History))
            .children(DEFAULT_GENRES.iter().enumerate().map(|(idx, &genre)| {
                let cat = AnimeCategory::Genre(genre.to_string());
                let is_active = match &current {
                    AnimeCategory::Genre(g) => g == genre,
                    _ => false,
                };
                let cb = on_select.clone();
                Button::new(format!("tab-genre-{}", idx))
                    .label(genre)
                    .when(is_active, |b| b.primary())
                    .when_some(cb, move |b, handler| {
                        let cat_clone = cat.clone();
                        b.on_click(move |_ev, win, cx| handler(&cat_clone, win, cx))
                    })
            }))
    }
}
