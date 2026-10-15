//! Labeled auth input — manual `gpui-base` expansion only.
//!
//! Uppercase mono label with a blue tick and an optional header-right slot
//! (e.g. a forgot-key link), over an editable field built on the
//! `gpui-base` input engine: leading glyph, live text, trailing action
//! slot. All state stays parent-owned; this is a pure `RenderOnce` view.

use gpui_kit::base::input::{Input, InputState};
use gpui_kit::*;

use crate::theme;

#[derive(IntoElement)]
pub struct FormField {
    label: SharedString,
    state: Entity<InputState>,
    leading: Option<SharedString>,
    header_extra: Option<AnyElement>,
    trailing: Option<AnyElement>,
    font: Option<SharedString>,
}

impl FormField {
    pub fn new(label: impl Into<SharedString>, state: Entity<InputState>) -> Self {
        Self {
            label: label.into(),
            state,
            leading: None,
            header_extra: None,
            trailing: None,
            font: None,
        }
    }

    pub fn leading(mut self, icon: impl Into<SharedString>) -> Self {
        self.leading = Some(icon.into());
        self
    }

    pub fn header_extra(mut self, extra: impl IntoElement) -> Self {
        self.header_extra = Some(extra.into_any_element());
        self
    }

    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// Override the value font (e.g. mono for secrets). Unset keeps the
    /// default UI font, so existing call sites are unaffected.
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font = Some(family.into());
        self
    }
}

impl RenderOnce for FormField {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(div().w(px(6.0)).h(px(6.0)).bg(theme::rgb(theme::colors::PRIMARY)))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::rgb(theme::colors::INK))
                            .child(self.label),
                    ),
            );
        if let Some(extra) = self.header_extra {
            header = header.child(extra);
        }
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .w_full()
            .px_3()
            .py_2()
            .bg(theme::rgb(theme::colors::SURFACE_LOW))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L2))
            .rounded(px(theme::radius::DEFAULT))
            .on_mouse_down_out(|_, window, cx| window.blur(cx));
        if let Some(icon) = self.leading {
            row = row.child(
                svg()
                    .path(icon)
                    .w(px(14.0))
                    .h(px(14.0))
                    .flex_shrink_0()
                    .text_color(theme::rgb(theme::colors::MUTED)),
            );
        }
        row = row.child({
            let mut text = div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::rgb(theme::colors::INK));
            if let Some(family) = self.font {
                text = text.font_family(family);
            }
            text.child(Input::new(&self.state))
        });
        if let Some(trailing) = self.trailing {
            row = row.child(trailing);
        }
        div().flex().flex_col().gap_1().w_full().child(header).child(row)
    }
}
