use gpui::{App, Div, IntoElement, RenderOnce, Window, div, prelude::*, px};

use crate::theme::palette as p;

pub fn avatar_el(initials: &str, active: bool) -> Div {
    div()
        .w(px(32.0))
        .h(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(if active { p::primary() } else { p::hairline() })
        .bg(if active { p::primary() } else { p::panel_hi() })
        .text_color(if active {
            gpui::rgb(0xffffff)
        } else {
            p::ink_dim()
        })
        .child(initials.to_owned())
}

/// Circular initial tile. Active channel gets the primary fill.
#[derive(IntoElement)]
pub struct Avatar {
    pub initials: String,
    pub active: bool,
}

impl Avatar {
    pub fn new(initials: impl Into<String>) -> Self {
        Self {
            initials: initials.into(),
            active: false,
        }
    }

    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        avatar_el(&self.initials, self.active)
    }
}
