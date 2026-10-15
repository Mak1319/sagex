//! Compact scrubber footer — manual `gpui-base` expansion only.
//!
//! Playback progress for video-style cards: flexible track with an accent
//! fill sized by flex ratio (no fixed widths) plus an elapsed/total label.

use gpui_kit::*;

use crate::theme;

#[derive(IntoElement)]
pub struct ScrubberBar {
    progress: f32,
    elapsed: SharedString,
    total: SharedString,
    accent: u32,
}

impl ScrubberBar {
    pub fn new(progress: f32, elapsed: impl Into<SharedString>, total: impl Into<SharedString>) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            elapsed: elapsed.into(),
            total: total.into(),
            accent: theme::colors::PRIMARY,
        }
    }

    pub fn accent(mut self, accent: u32) -> Self {
        self.accent = accent;
        self
    }
}

impl RenderOnce for ScrubberBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let rest = 1.0 - self.progress;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .w_full()
            .child(
                div()
                    .flex_1()
                    .h(px(6.0))
                    .flex()
                    .flex_row()
                    .rounded_full()
                    .bg(theme::rgb(theme::colors::INSET))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .overflow_hidden()
                    .child(div().flex_grow(self.progress).h_full().bg(theme::rgb(self.accent)))
                    .child(div().flex_grow(rest)),
            )
            .child(
                div()
                    .text_size(px(8.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child(SharedString::from(format!("{} / {}", self.elapsed, self.total))),
            )
    }
}
