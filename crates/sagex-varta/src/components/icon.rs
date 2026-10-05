use gpui::{App, Div, IntoElement, RenderOnce, Window, div, prelude::*, px, svg};

use super::icon_kind::IconKind;
use crate::theme::palette as p;

/// Shared glyph builder.
pub fn icon_el(kind: IconKind, size: f32, active: bool, danger: bool) -> Div {
    let color = if danger {
        p::danger()
    } else if active {
        p::primary()
    } else {
        p::ink_dim()
    };
    div().child(
        svg()
            .path(kind.file())
            .w(px(size))
            .h(px(size))
            .text_color(color),
    )
}

/// Single-color SVG glyph — real GPUI component, rendered once.
#[derive(IntoElement)]
pub struct Icon {
    pub kind: IconKind,
    pub size: f32,
    pub active: bool,
    pub danger: bool,
}

impl Icon {
    pub fn new(kind: IconKind) -> Self {
        Self {
            kind,
            size: 18.0,
            active: false,
            danger: false,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }

    /// Red destructive tint (delete affordances).
    #[allow(dead_code)]
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        icon_el(self.kind, self.size, self.active, self.danger)
    }
}
