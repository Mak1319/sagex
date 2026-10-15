//! Pinned channels header — manual `gpui-base` expansion only.
//!
//! Matches `Pinned Channels Header.svg`: 27px bar, pin glyph `#004AC6`,
//! label `#475569`. Pin glyph is iconoir `pin.svg`.

use gpui_kit::*;

use crate::theme;

const PIN: &str = "icons/pin.svg";

#[derive(IntoElement)]
pub struct PinnedHeader {
    label: SharedString,
    count: usize,
}

impl PinnedHeader {
    pub fn new() -> Self {
        Self {
            label: SharedString::from("PINNED CHANNELS"),
            count: 0,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    pub fn reader_count(&self) -> usize {
        self.count
    }
}

impl Default for PinnedHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for PinnedHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(27.0))
            .w_full()
            .bg(theme::rgb(theme::colors::SURFACE_LOW))
            .border_b_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .child(
                svg()
                    .path(PIN)
                    .w(px(14.0))
                    .h(px(14.0))
                    .text_color(theme::rgb(0x004ac6)),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::rgb(0x475569))
                    .child(self.label),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child(SharedString::from(format!("{}", self.count))),
            )
    }
}
