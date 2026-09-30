//! Reusable shadcn-style elements (mirrors sagex-desk tokens).

use gpui::{Div, Entity, div, prelude::*, rgb};
use gpui_component::{button::Button, input::{Input, InputState}, label::Label};

pub fn bg_app() -> gpui::Rgba {
    rgb(0x09090b)
}
pub fn bg_panel() -> gpui::Rgba {
    rgb(0x101014)
}
pub fn bg_card() -> gpui::Rgba {
    rgb(0x131318)
}
pub fn border() -> gpui::Rgba {
    gpui::rgba(0xffffff1a)
}
pub fn text_main() -> gpui::Rgba {
    rgb(0xfafafa)
}
pub fn text_muted() -> gpui::Rgba {
    rgb(0xa1a1aa)
}

/// Primary action button (label + click wired by the caller).
pub fn btn(id: &'static str, label: &str) -> Button {
    Button::new(id).label(label)
}

/// Secondary (outline) button.
pub fn btn_secondary(id: &'static str, label: &str) -> Button {
    Button::new(id).label(label).outline()
}

/// Full-width text field bound to an input entity.
pub fn field(input: &Entity<InputState>) -> Div {
    div().w_full().child(Input::new(input))
}

/// Bordered card container.
pub fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .bg(bg_card())
        .border_1()
        .border_color(border())
        .rounded_lg()
        .p_4()
}

/// Section heading.
pub fn section_title(t: &str) -> Div {
    div().child(Label::new(t).text_lg().text_color(text_main()))
}

/// Muted one-liner.
pub fn muted(t: &str) -> Div {
    div()
        .text_sm()
        .text_color(text_muted())
        .child(t.to_string())
}

/// Scrollable mono block for keys / records / verdicts (`id` must be unique).
pub fn mono_block(id: &'static str, text: &str) -> impl gpui::IntoElement {
    div()
        .w_full()
        .max_h(gpui::px(220.0))
        .id(id)
        .overflow_y_scroll()
        .bg(rgb(0x000000))
        .border_1()
        .border_color(border())
        .rounded_md()
        .p_3()
        .text_xs()
        .text_color(text_muted())
        .child(text.to_string())
}

/// Bottom status line.
pub fn status_bar(status: &str) -> Div {
    div()
        .w_full()
        .border_t_1()
        .border_color(border())
        .px_4()
        .py_2()
        .text_xs()
        .text_color(text_muted())
        .child(status.to_string())
}
