//! Shared bordered section card with title header and body slot.

use gpui_kit::*;
use crate::theme;

/// Reusable section card: title bar with accent tick plus optional trailing control.
#[derive(IntoElement)]
pub struct SectionCard {
    title: SharedString,
    trailing: Option<AnyElement>,
    body: AnyElement,
}

impl SectionCard {
    pub fn new(title: impl Into<SharedString>, body: impl IntoElement) -> Self {
        Self { title: title.into(), trailing: None, body: body.into_any_element() }
    }

    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }
}

impl RenderOnce for SectionCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut header = div()
            .flex().flex_row().items_center().gap_2()
            .child(div().w(px(6.0)).h(px(6.0)).bg(theme::rgb(theme::colors::PRIMARY)))
            .child(
                div().font_family(theme::fonts::MONO).text_size(px(10.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::rgb(theme::colors::INK))
                    .child(self.title),
            );
        if let Some(t) = self.trailing {
            header = div().flex().flex_row().items_center().justify_between()
                .w_full().child(header).child(t);
        } else {
            header = div().flex().flex_row().items_center().w_full().child(header);
        }
        div().flex().flex_col().w_full()
            .rounded(px(theme::radius::DEFAULT))
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_1().border_color(theme::rgb(theme::colors::GRID_L1))
            .child(
                div().flex().flex_row().items_center().w_full().px_4().py_2()
                    .bg(theme::rgb(theme::colors::SURFACE_LOW))
                    .border_b_1().border_color(theme::rgb(theme::colors::GRID_L1))
                    .child(header),
            )
            .child(div().w_full().p_4().child(self.body))
    }
}
