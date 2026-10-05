use gpui::{App, Div, IntoElement, RenderOnce, Window, div, prelude::*, px};

use crate::{models::Presence, theme::palette as p};

/// Filled status dot — the one family needing fixed fill colors,
/// which single-color SVG tint cannot express.
pub fn presence_el(status: Presence) -> Div {
    let color = match status {
        Presence::Online => p::ok(),
        Presence::Away => p::warn(),
        Presence::Offline => p::ink_faint(),
    };
    div().w(px(8.0)).h(px(8.0)).rounded_full().bg(color)
}

#[derive(IntoElement)]
pub struct PresenceDot {
    pub status: Presence,
}

impl RenderOnce for PresenceDot {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        presence_el(self.status)
    }
}
