use gpui::{App, IntoElement, RenderOnce, Window, div, prelude::*};

use crate::theme::palette as p;

/// Mono uppercase section header (`PINNED (3)`, `CHANNEL METADATA`).
#[derive(IntoElement)]
pub struct SectionHead {
    pub text: String,
}

impl SectionHead {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for SectionHead {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .px_3()
            .pt_2()
            .text_xs()
            .text_color(p::ink_faint())
            .child(self.text)
    }
}
