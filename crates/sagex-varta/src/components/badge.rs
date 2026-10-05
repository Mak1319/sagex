use gpui::{App, Div, IntoElement, RenderOnce, Window, div, prelude::*};

use crate::theme::palette as p;

pub fn badge_el(text: &str) -> Div {
    div()
        .px_2()
        .py_px()
        .border_1()
        .border_color(p::hairline())
        .bg(p::panel_hi())
        .text_color(p::ink_dim())
        .text_xs()
        .child(text.to_owned())
}

pub fn unread_el(n: u32) -> Div {
    div()
        .px_1()
        .bg(p::primary())
        .text_color(gpui::rgb(0xffffff))
        .text_xs()
        .child(format!("{}", n))
}

/// Mono metadata tag (`CORE-ALGO`, `GLSL ES 3.0`, …).
#[derive(IntoElement)]
pub struct Badge {
    pub text: String,
}

impl Badge {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        badge_el(&self.text)
    }
}

/// Blue unread counter pill.
#[derive(IntoElement)]
pub struct UnreadCount {
    pub n: u32,
}

impl RenderOnce for UnreadCount {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        unread_el(self.n)
    }
}
