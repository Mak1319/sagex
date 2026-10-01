//! Unread-count pill for chat rows.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder, px, rgb,
};

/// Green count badge. Renders nothing when `count` is 0.
#[derive(IntoElement)]
pub struct UnreadBadge {
    style: StyleRefinement,
    count: usize,
}

impl UnreadBadge {
    pub fn new(count: usize) -> Self {
        Self {
            style: StyleRefinement::default(),
            count,
        }
    }
}

impl Styled for UnreadBadge {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for UnreadBadge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().when(self.count > 0, |t| {
            t.child(
                div()
                    .size(px(20.))
                    .flex_shrink_0()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(super::colors::ACCENT_GREEN))
                    .text_color(rgb(0xffffff))
                    .text_xs()
                    .child(format!("{}", self.count)),
            )
        })
    }
}
