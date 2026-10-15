//! Chat top bar — manual `gpui-base` expansion only.
//!
//! 56px white header: 35px rounded avatar + presence dot, title and
//! subtitle, right icon cluster (video, members, search), divider, kebab.

use gpui_kit::*;

use crate::components::bar_button::BarButton;
use crate::theme;

const VIDEO: &str = "icons/video-camera.svg";
const MEMBERS: &str = "icons/users.svg";
const SEARCH: &str = "icons/magnifying-glass.svg";
const KEBAB: &str = "icons/ellipsis-vertical.svg";

#[derive(IntoElement)]
pub struct ChatTopBar {
    title: SharedString,
    subtitle: SharedString,
}

impl ChatTopBar {
    pub fn new(
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
    ) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
        }
    }
}

impl RenderOnce for ChatTopBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .h(px(56.0))
            .w_full()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_b_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .child(
                div()
                    .relative()
                    .w(px(35.0))
                    .h(px(35.0))
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_full()
                            .rounded(px(7.5))
                            .bg(theme::rgb(0xdbeafe))
                            .border_1()
                            .border_color(theme::rgb(0xbfdbfe))
                            .text_color(theme::rgb(0x1d4ed8))
                            .text_size(px(15.0))
                            .child("M"),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(-1.0))
                            .right(px(-1.0))
                            .w(px(10.0))
                            .h(px(10.0))
                            .rounded_full()
                            .bg(theme::rgb(theme::colors::SUCCESS))
                            .border_2()
                            .border_color(theme::rgb(0xffffff)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(theme::rgb(theme::colors::INK))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(self.title),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(self.subtitle),
                    ),
            )
            .child(BarButton::new("chat-call", VIDEO))
            .child(BarButton::new("chat-members", MEMBERS))
            .child(BarButton::new("chat-search", SEARCH))
            .child(div().w(px(1.0)).h(px(16.0)).bg(theme::rgb(theme::colors::GRID_L1)))
            .child(BarButton::new("chat-more", KEBAB))
    }
}
