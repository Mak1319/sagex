//! Inspector sheet content — manual `gpui-base` expansion only.

use gpui_kit::*;

use crate::theme;

/// Body for the right-side resizable sheet.
#[derive(IntoElement)]
pub struct InspectorContent {}

impl InspectorContent {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for InspectorContent {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for InspectorContent {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .size_full()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child("ITEM INSPECTOR"),
            )
            .child(
                div()
                    .p_2()
                    .rounded(px(theme::radius::DEFAULT))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .bg(theme::rgb(theme::colors::SURFACE_LOW))
                    .text_size(px(12.0))
                    .text_color(theme::rgb(theme::colors::INK))
                    .child("No selection"),
            )
    }
}
