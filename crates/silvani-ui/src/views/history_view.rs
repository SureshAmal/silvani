use std::rc::Rc;

use gpui_kit::base::StyledExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::label::Label;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::history::{HistoryItem, WatchHistoryManager};

pub type ClickHandler = Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>;
pub type ResumeHandler = Option<Rc<dyn Fn(&HistoryItem, &mut Window, &mut App)>>;

#[derive(IntoElement)]
pub struct HistoryView {
    watch_history: WatchHistoryManager,
    on_back: ClickHandler,
    on_resume: ResumeHandler,
    on_clear: ClickHandler,
}

impl HistoryView {
    pub fn new(watch_history: WatchHistoryManager) -> Self {
        Self {
            watch_history,
            on_back: None,
            on_resume: None,
            on_clear: None,
        }
    }

    pub fn on_back(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_back = Some(Rc::new(handler));
        self
    }

    pub fn on_resume(
        mut self,
        handler: impl Fn(&HistoryItem, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resume = Some(Rc::new(handler));
        self
    }

    pub fn on_clear(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_clear = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for HistoryView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let history_items = self.watch_history.get_recent_history(50);
        let on_back = self.on_back.clone();
        let on_resume = self.on_resume.clone();
        let on_clear = self.on_clear.clone();

        v_flex()
            .id("history-view-container")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .p_6()
            .gap_4()
            .overflow_y_scroll()
            // Top Nav
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        Button::new("history-back-btn")
                            .outline()
                            .label("← Back to Browse")
                            .when_some(on_back, |b, handler| {
                                b.on_click(move |ev, win, cx| handler(ev, win, cx))
                            }),
                    )
                    .child(
                        Button::new("clear-history-btn")
                            .danger()
                            .small()
                            .label("Clear History")
                            .when_some(on_clear, |b, handler| {
                                b.on_click(move |ev, win, cx| handler(ev, win, cx))
                            }),
                    ),
            )
            .child(
                Label::new("Watch History")
                    .text_2xl()
                    .font_bold(),
            )
            .when(history_items.is_empty(), |d| {
                d.child(
                    v_flex()
                        .items_center()
                        .justify_center()
                        .h(px(300.))
                        .gap_2()
                        .child(
                            Label::new("No watch history yet.")
                                .text_lg()
                                .text_color(theme.muted_foreground),
                        ),
                )
            })
            .when(!history_items.is_empty(), |d| {
                d.child(
                    v_flex()
                        .gap_2()
                        .w_full()
                        .children(history_items.into_iter().enumerate().map(|(idx, item)| {
                            let item_c = item.clone();
                            let resume_cb = on_resume.clone();

                            h_flex()
                                .id(ElementId::NamedInteger("history-row".into(), idx as u64))
                                .items_center()
                                .justify_between()
                                .p_3()
                                .rounded_md()
                                .bg(theme.secondary)
                                .border_1()
                                .border_color(theme.border)
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_3()
                                        .child(
                                            Tag::primary()
                                                .child(format!("EP {}", item.episode_number)),
                                        )
                                        .child(
                                            v_flex()
                                                .child(
                                                    Label::new(item.anime_title.clone())
                                                        .font_semibold(),
                                                )
                                                .when_some(item.episode_title.clone(), |v, ep_t| {
                                                    v.child(
                                                        Label::new(ep_t)
                                                            .text_xs()
                                                            .text_color(theme.muted_foreground),
                                                    )
                                                }),
                                        ),
                                )
                                .child(
                                    Button::new(format!("resume-btn-{}", idx))
                                        .primary()
                                        .small()
                                        .label("Resume")
                                        .when_some(resume_cb, move |b, handler| {
                                            let it = item_c.clone();
                                            b.on_click(move |_ev, win, cx| handler(&it, win, cx))
                                        }),
                                )
                        })),
                )
            })
    }
}
