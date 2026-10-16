//! Inspector sheet content — manual `gpui-base` expansion only.

use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::WindowExt;
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
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child("ITEM INSPECTOR"),
                    )
                    .child(
                        BaseButton::new(ElementId::Name("inspector-close".into()))
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(2.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .hover(|s| s.bg(theme::rgb(theme::colors::ROW_HOVER)))
                            .on_click(|_, window, cx| window.close_sheet(cx))
                            .child(
                                svg()
                                    .path("icons/x-mark.svg")
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .text_color(theme::rgb(theme::colors::MUTED)),
                            ),
                    ),
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
