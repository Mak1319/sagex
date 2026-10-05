use gpui::{App, IntoElement, RenderOnce, Window, div, prelude::*};

use crate::{
    components::{Icon, IconKind, UnreadCount, icon_el},
    theme::palette as p,
};

/// Click-to-focus search box showing the live query.
#[derive(IntoElement)]
pub struct SearchInput {
    pub value: String,
    pub focused: bool,
}

impl RenderOnce for SearchInput {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .border_1()
            .border_color(if self.focused {
                p::primary()
            } else {
                p::hairline()
            })
            .cursor_pointer()
            .text_sm()
            .text_color(if self.value.is_empty() {
                p::ink_faint()
            } else {
                p::ink()
            })
            .child(icon_el(IconKind::Search, 15.0, self.focused, false))
            .child(if self.value.is_empty() {
                "Search conversations...  ( / )".to_owned()
            } else {
                self.value
            })
    }
}

/// One metadata cell of the inspector 2×2 matrix.
#[derive(IntoElement)]
pub struct MetaCell {
    pub key: String,
    pub value: String,
}

impl RenderOnce for MetaCell {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .p_2()
            .border_1()
            .border_color(p::hairline())
            .bg(p::panel())
            .child(div().text_xs().text_color(p::ink_faint()).child(self.key))
            .child(div().text_xs().text_color(p::ink()).child(self.value))
    }
}

/// One thread row in the inspector.
#[derive(IntoElement)]
pub struct ThreadRow {
    pub title: String,
    pub meta: String,
    pub count: Option<u32>,
}

impl RenderOnce for ThreadRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .p_2()
            .border_1()
            .border_color(p::hairline())
            .bg(p::panel())
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().text_color(p::ink()).child(self.title))
                    .child(match self.count {
                        Some(n) => UnreadCount { n }.into_any_element(),
                        None => div().into_any_element(),
                    }),
            )
            .child(div().text_xs().text_color(p::ink_faint()).child(self.meta))
    }
}

/// One attachment row in the inspector.
#[derive(IntoElement)]
pub struct AttachmentRow {
    pub name: String,
    pub size: String,
    pub kind: IconKind,
}

impl RenderOnce for AttachmentRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .p_2()
            .border_1()
            .border_color(p::hairline())
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(self.kind))
                    .child(div().text_sm().text_color(p::ink()).child(self.name)),
            )
            .child(div().text_xs().text_color(p::ink_faint()).child(self.size))
    }
}
