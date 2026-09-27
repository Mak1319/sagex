use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, prelude::FluentBuilder, px, rgb,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt as _,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};

use super::{ChatApp, model::ChatKind, model::MessageKind};
use crate::component::{ACCENT_GREEN, Avatar};

impl ChatApp {
    pub(super) fn render_group_info(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let pos = self.active_pos();
        let Some(pos) = pos else {
            return div().into_any_element();
        };
        let chat = self.chats[pos].clone();
        let is_group = chat.kind == ChatKind::Group;
        // member count: unique senders + you
        let mut members: Vec<String> = vec![];
        for m in &chat.messages {
            if !m.mine && m.sender != "System" && !members.contains(&m.sender) {
                members.push(m.sender.clone());
            }
        }
        let total = members.len() + 1;
        let media: Vec<(String, &'static str)> = chat
            .messages
            .iter()
            .filter_map(|m| {
                if m.link.is_some() {
                    Some(("link".to_string(), "icons/link.svg"))
                } else if matches!(m.kind, MessageKind::Image { .. }) {
                    Some(("img".to_string(), "icons/image.svg"))
                } else {
                    None
                }
            })
            .collect();
        let media_count = media.len();
        let mut tiles = div().flex().flex_row().flex_wrap().gap_2();
        for (ix, (_, icon)) in media.into_iter().take(6).enumerate() {
            tiles = tiles.child(
                div()
                    .id(("media-tile", ix))
                    .size(px(72.))
                    .rounded_md()
                    .bg(theme.muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme.muted_foreground)
                    .child(Icon::empty().path(icon).large()),
            );
        }
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .id("group-info-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_4()
                            .px_4()
                            .py_3()
                            .child(
                                Button::new("group-info-close")
                                    .ghost()
                                    .icon(Icon::empty().path("icons/x.svg"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_group_info = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_bold()
                                    .text_color(theme.foreground)
                                    .child(if is_group {
                                        "Group info".to_string()
                                    } else {
                                        "Contact info".to_string()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .py_4()
                            .child(
                                Avatar::new(chat.initials.clone())
                                    .color(chat.color)
                                    .diameter(px(96.))
                                    .glyph(px(24.)),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_bold()
                                    .text_color(theme.foreground)
                                    .child(chat.name.clone()),
                            )
                            .when(is_group, |t| {
                                t.child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_1()
                                        .text_sm()
                                        .child(
                                            div()
                                                .text_color(theme.muted_foreground)
                                                .child("Group · ".to_string()),
                                        )
                                        .child(
                                            div()
                                                .text_color(rgb(ACCENT_GREEN))
                                                .child(format!("{total} members")),
                                        ),
                                )
                            })
                            .when(!is_group, |t| {
                                t.child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child(chat.subtitle.clone()),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .gap_6()
                            .py_2()
                            .children(
                                [
                                    ("Voice", "icons/phone.svg"),
                                    ("Video", "icons/video.svg"),
                                    ("Add", "icons/user-plus.svg"),
                                    ("Search", "icons/search.svg"),
                                ]
                                .into_iter()
                                .enumerate()
                                .map(|(ix, (label, icon))| {
                                    div()
                                        .id(("ginfo-act", ix))
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_1()
                                        .cursor_pointer()
                                        .child(
                                            div()
                                                .size(px(48.))
                                                .rounded_full()
                                                .bg(theme.muted)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_color(theme.foreground)
                                                .child(Icon::empty().path(icon).size(px(20.))),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .child(label.to_string()),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.notice = Some(format!("{label} (UI preview)"));
                                            cx.notify();
                                        }))
                                        .into_any_element()
                                }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_3()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(ACCENT_GREEN))
                                    .child("Add group description".to_string()),
                            )
                            .child(
                                Button::new("ginfo-desc-edit")
                                    .ghost()
                                    .small()
                                    .icon(Icon::empty().path("icons/pencil.svg"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.notice =
                                            Some("Edit description (UI preview)".to_string());
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(div().h(px(1.)).mx_4().bg(theme.border))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .px_4()
                            .py_3()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        Icon::empty()
                                            .path("icons/image.svg")
                                            .text_color(theme.muted_foreground),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_color(theme.foreground)
                                            .child("Media, links and docs".to_string()),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child(format!("{media_count}")),
                                    ),
                            )
                            .child(tiles),
                    ),
            )
            .into_any_element()
    }
}
