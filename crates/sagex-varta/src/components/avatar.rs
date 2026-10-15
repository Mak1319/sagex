//! Unified avatar — manual `gpui-base` expansion only.
//!
//! The single avatar for the whole app: a plain square box with no
//! border-radius. Glyph size scales with the box. A 1px border and a
//! presence dot are opt-in (default off) so the base stays a spare box.
//! The presence dot itself stays round — it is a status signal, not the
//! avatar box.

use gpui_kit::*;

use crate::theme;

#[derive(IntoElement)]
pub struct Avatar {
    initial: SharedString,
    bg: u32,
    fg: u32,
    size: f32,
    border: Option<u32>,
    presence: Option<u32>,
}

impl Avatar {
    pub fn new(initial: impl Into<SharedString>) -> Self {
        Self {
            initial: initial.into(),
            bg: theme::colors::INSET,
            fg: theme::colors::MUTED,
            size: 24.0,
            border: None,
            presence: None,
        }
    }

    pub fn bg(mut self, bg: u32) -> Self {
        self.bg = bg;
        self
    }

    pub fn fg(mut self, fg: u32) -> Self {
        self.fg = fg;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn border(mut self, border: u32) -> Self {
        self.border = Some(border);
        self
    }

    pub fn presence(mut self, presence: u32) -> Self {
        self.presence = Some(presence);
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let edge = px(self.size);
        let mut face = div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .bg(theme::rgb(self.bg))
            .text_color(theme::rgb(self.fg))
            .text_size(px(self.size * 0.42))
            .font_weight(FontWeight::BOLD)
            .child(self.initial);
        if let Some(border) = self.border {
            face = face.border_1().border_color(theme::rgb(border));
        }
        let mut root = div()
            .relative()
            .w(edge)
            .h(edge)
            .flex_shrink_0()
            .child(face);
        if let Some(dot) = self.presence {
            let d = px((self.size * 0.29).max(7.0));
            root = root.child(
                div()
                    .absolute()
                    .bottom(px(0.0))
                    .right(px(0.0))
                    .w(d)
                    .h(d)
                    .rounded_full()
                    .bg(theme::rgb(dot))
                    .border_1()
                    .border_color(theme::rgb(0xffffff)),
            );
        }
        root
    }
}
