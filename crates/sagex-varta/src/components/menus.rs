use gpui::{App, IntoElement, RenderOnce, Window, div, prelude::*};

use crate::{
    components::{IconKind, icon_el},
    theme::palette as p,
};

/// Hover action strip on a message: reply / react / forward / delete.
#[derive(IntoElement)]
pub struct MessageActionBar;

impl RenderOnce for MessageActionBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .gap_1()
            .child(icon_el(IconKind::Reply, 14.0, false, false))
            .child(icon_el(IconKind::React, 14.0, false, false))
            .child(icon_el(IconKind::Forward, 14.0, false, false))
            .child(icon_el(IconKind::Delete, 14.0, false, true))
    }
}

#[allow(dead_code)]
struct MenuEntry {
    icon: IconKind,
    label: &'static str,
    hint: &'static str,
    danger: bool,
}

/// Floating context menu (MSG // CONTEXT_OPS) from the screenshot.
/// Shown on message `...` hover in a later interaction pass.
#[derive(IntoElement)]
pub struct ContextMenu;

impl RenderOnce for ContextMenu {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let entries = [
            MenuEntry {
                icon: IconKind::Reply,
                label: "Reply in Thread",
                hint: "^R",
                danger: false,
            },
            MenuEntry {
                icon: IconKind::Quote,
                label: "Quote Message",
                hint: "#Q",
                danger: false,
            },
            MenuEntry {
                icon: IconKind::Forward,
                label: "Forward",
                hint: "^F",
                danger: false,
            },
            MenuEntry {
                icon: IconKind::Pin,
                label: "Pin to Channel",
                hint: "#P",
                danger: false,
            },
            MenuEntry {
                icon: IconKind::Copy,
                label: "Copy Message Text",
                hint: "#C",
                danger: false,
            },
            MenuEntry {
                icon: IconKind::Link,
                label: "Copy Message ID",
                hint: "^C",
                danger: false,
            },
            MenuEntry {
                icon: IconKind::View,
                label: "Inspect Telemetry / Node",
                hint: "^I",
                danger: false,
            },
            MenuEntry {
                icon: IconKind::Delete,
                label: "Delete Message",
                hint: "X",
                danger: true,
            },
        ];
        let mut list = div().flex().flex_col();
        for e in entries {
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .text_sm()
                    .cursor_pointer()
                    .text_color(if e.danger { p::danger() } else { p::ink() })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(icon_el(e.icon, 14.0, false, e.danger))
                            .child(e.label.to_owned()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(p::ink_faint())
                            .child(e.hint.to_owned()),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .w(gpui::px(260.0))
            .border_1()
            .border_color(p::hairline())
            .bg(p::panel())
            .child(
                div()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(p::hairline())
                    .text_xs()
                    .text_color(p::ink_dim())
                    .child("MSG // CONTEXT_OPS".to_owned()),
            )
            .child(list)
    }
}
