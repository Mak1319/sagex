//! Waveform footer — manual `gpui-base` expansion only.
//!
//! Audio-style visualizer: caller-supplied normalized bar heights. Bars
//! share space by flex ratio (count is dynamic), played bars take the
//! accent, the rest the grid tone. Elapsed/total label trails the bars.

use gpui_kit::*;

use crate::theme;

#[derive(IntoElement)]
pub struct WaveformBar {
    bars: Vec<f32>,
    played: f32,
    elapsed: SharedString,
    total: SharedString,
    accent: u32,
}

impl WaveformBar {
    pub fn new(
        bars: Vec<f32>,
        played: f32,
        elapsed: impl Into<SharedString>,
        total: impl Into<SharedString>,
    ) -> Self {
        Self {
            bars,
            played: played.clamp(0.0, 1.0),
            elapsed: elapsed.into(),
            total: total.into(),
            accent: theme::colors::SUCCESS,
        }
    }

    pub fn accent(mut self, accent: u32) -> Self {
        self.accent = accent;
        self
    }
}

impl RenderOnce for WaveformBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let played_count = (self.played * self.bars.len() as f32).floor() as usize;
        let bars: Vec<AnyElement> = self
            .bars
            .iter()
            .enumerate()
            .map(|(ix, height)| {
                let h = px(2.0 + height.clamp(0.0, 1.0) * 14.0);
                let tone = if ix < played_count {
                    self.accent
                } else {
                    theme::colors::GRID_L2
                };
                div().flex_1().h(h).bg(theme::rgb(tone)).into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .w_full()
            .px_2()
            .py_1()
            .bg(theme::rgb(0xf8fafc))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .rounded(px(2.0))
            .child(
                div()
                    .flex_1()
                    .h(px(16.0))
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap_1()
                    .children(bars),
            )
            .child(
                div()
                    .text_size(px(8.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child(SharedString::from(format!("{} / {}", self.elapsed, self.total))),
            )
    }
}
