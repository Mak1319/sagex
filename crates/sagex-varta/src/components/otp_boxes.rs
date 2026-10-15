//! OTP digit boxes — manual `gpui-base` expansion only.
//!
//! Pure view over a code string: filled digits, an active caret slot and
//! washed empty slots with a dash separator mid-row. The editing engine
//! stays parent-owned; click-to-focus wiring lives with the caller.

use gpui_kit::*;

use crate::theme;

const SLOTS: usize = 6;

#[derive(IntoElement)]
pub struct OtpBoxes {
    code: SharedString,
}

impl OtpBoxes {
    pub fn new(code: impl Into<SharedString>) -> Self {
        Self {
            code: code.into(),
        }
    }
}

fn digit_cell(filled: Option<char>, active: bool) -> AnyElement {
    let mut cell = div()
        .w(px(44.0))
        .h(px(52.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(2.0));
    match (filled, active) {
        (Some(digit), _) => {
            cell = cell
                .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                .border_1()
                .border_color(theme::rgb(theme::colors::GRID_L1))
                .child(
                    div()
                        .text_size(px(20.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::rgb(theme::colors::PRIMARY))
                        .child(SharedString::from(digit.to_string())),
                );
        }
        (None, true) => {
            cell = cell
                .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                .border_1()
                .border_color(theme::rgb(theme::colors::PRIMARY))
                .child(div().w(px(2.0)).h(px(20.0)).bg(theme::rgb(
                    theme::colors::PRIMARY,
                )));
        }
        (None, false) => {
            cell = cell.bg(theme::rgb(theme::colors::INSET));
        }
    }
    cell.into_any_element()
}

impl RenderOnce for OtpBoxes {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let chars: Vec<char> = self.code.chars().take(SLOTS).collect();
        let mut cells: Vec<AnyElement> = Vec::with_capacity(SLOTS + 1);
        for ix in 0..SLOTS {
            if ix == SLOTS / 2 {
                cells.push(
                    div()
                        .text_size(px(14.0))
                        .text_color(theme::rgb(theme::colors::MUTED))
                        .child("–")
                        .into_any_element(),
                );
            }
            let active = ix == chars.len() && chars.len() < SLOTS;
            cells.push(digit_cell(chars.get(ix).copied(), active));
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap_2()
            .w_full()
            .children(cells)
    }
}
