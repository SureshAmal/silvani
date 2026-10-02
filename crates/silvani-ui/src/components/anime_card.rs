use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::base::StyledExt;
use gpui_kit::component::label::Label;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::prelude::*;
use gpui_kit::*;

use silvani_meta::models::AnimeItem;

pub type ClickHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct AnimeCard {
    id: ElementId,
    item: AnimeItem,
    cached_path: Option<PathBuf>,
    on_click: ClickHandler,
}

impl AnimeCard {
    pub fn new(id: impl Into<ElementId>, item: AnimeItem) -> Self {
        Self {
            id: id.into(),
            item,
            cached_path: None,
            on_click: None,
        }
    }

    pub fn cached_path(mut self, path: Option<PathBuf>) -> Self {
        self.cached_path = path;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AnimeCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let title = self.item.display_title().to_string();
        let season = self.item.season_display();
        let score_opt = self.item.score_display();

        // Prefer locally cached image path for maximum performance and offline display
        let image_source: Option<ImageSource> = match &self.cached_path {
            Some(p) if p.exists() => Some(ImageSource::from(p.clone())),
            _ => self.item.cover_image.as_ref().map(|url| ImageSource::from(url.clone())),
        };

        v_flex()
            .id(self.id)
            .w(px(180.))
            .gap_2()
            .p_2()
            .rounded(theme.radius)
            .bg(theme.secondary)
            .border_1()
            .border_color(theme.border)
            .hover(|s| s.bg(theme.accent).border_color(theme.ring))
            .cursor_pointer()
            .when_some(self.on_click, |d, cb| {
                d.on_click(move |ev, win, cx| cb(ev, win, cx))
            })
            .when_some(image_source, |d, src| {
                d.child(
                    img(src)
                        .w_full()
                        .h(px(245.))
                        .rounded(theme.radius)
                        .object_fit(ObjectFit::Cover)
                        .flex_shrink_0(),
                )
            })
            .child(
                Label::new(title)
                    .text_sm()
                    .font_semibold()
                    .line_clamp(1),
            )
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(Tag::secondary().child(season))
                    .when_some(score_opt, |d, score| {
                        d.child(Tag::primary().child(format!("★ {}", score)))
                    }),
            )
    }
}
