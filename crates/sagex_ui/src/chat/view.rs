use gpui::{
    Context, ExternalPaths, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, deferred, div, img, prelude::FluentBuilder, px,
    rgb,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt as _,
    button::{Button, ButtonVariants},
};

use super::ChatApp;
use super::attach::PreviewSel;
use super::model::ChatKind;
use crate::component::ScrollThumb;

impl ChatApp {
    pub(super) fn render_main(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        // account panels take over the column even with no chat selected
        if self.show_profile {
            return self.render_profile(cx);
        }
        if self.show_new_chat {
            return self.render_new_chat(cx);
        }
        if self.show_poll {
            return self.render_poll_sheet(cx);
        }
        let pos = self.active_pos();
        let Some(pos) = pos else {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child("Select a chat")
                .into_any_element();
        };
        let chat = self.chats[pos].clone();
        if self.show_group_info {
            return self.render_group_info(cx);
        }
        let is_group = chat.kind == ChatKind::Group;
        let subtitle = format!("{} · {}", self.live_subtitle(&chat), self.conn_label());

        let mut bubbles: Vec<gpui::AnyElement> = vec![];
        let mut last_date = String::new();
        for (ix, m) in chat.messages.iter().enumerate() {
            if m.date != last_date {
                last_date = m.date.clone();
                bubbles.push(
                    div()
                        .flex()
                        .w_full()
                        .justify_center()
                        .py_1()
                        .my(px(4.))
                        .child(
                            div()
                                .rounded_md()
                                .px_3()
                                .py_1()
                                .bg(theme.muted)
                                .text_color(theme.muted_foreground)
                                .text_xs()
                                .child(m.date.clone()),
                        )
                        .into_any_element(),
                );
            }
            // smart grouping: avatar only on the last of a consecutive run
            // from the same sender (incoming group messages).
            let next_same = chat
                .messages
                .get(ix + 1)
                .map(|n| !n.mine && n.sender == m.sender)
                .unwrap_or(false);
            let show_avatar = !m.mine && is_group && !next_same;
            bubbles.push(
                self.render_bubble(cx, m, is_group, show_avatar)
                    .into_any_element(),
            );
        }

        div()
            .flex_1()
            .h_full()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .relative()
            .bg(theme.background)
            .on_drag_move({
                let view = cx.entity();
                move |_: &gpui::DragMoveEvent<ExternalPaths>, _, cx: &mut gpui::App| {
                    view.update(cx, |this: &mut ChatApp, cx| this.note_drag(cx));
                }
            })
            .on_drop({
                let view = cx.entity();
                move |paths: &ExternalPaths, _, cx: &mut gpui::App| {
                    view.update(cx, |this: &mut ChatApp, cx| {
                        this.drop_paths(paths.paths().to_vec(), cx)
                    });
                }
            })
            .child(
                // header
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .child(
                        div()
                            .size(px(40.))
                            .flex_shrink_0()
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(chat.color))
                            .text_color(rgb(0xffffff))
                            .font_bold()
                            .text_sm()
                            .child(chat.initials.clone()),
                    )
                    // clicking identity opens the profile / group info view
                    .child(
                        div()
                            .id("chat-header-identity")
                            .flex_1()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .cursor_pointer()
                            .rounded_md()
                            .px_1()
                            .hover(|s| s.bg(theme.muted))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_menus();
                                this.show_group_info = true;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_color(theme.foreground)
                                    .truncate()
                                    .child(chat.name.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .truncate()
                                    .text_color(theme.muted_foreground)
                                    .child(subtitle),
                            ),
                    )
                    .child(
                        Button::new("chat-search-btn")
                            .ghost()
                            .small()
                            .icon(IconName::Search)
                            .on_click(|_, _, _| println!("chat search")),
                    )
                    .child(
                        Button::new("chat-menu-btn")
                            .ghost()
                            .small()
                            .icon(IconName::EllipsisVertical)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_menus();
                                this.show_chat_menu = true;
                                cx.notify();
                            })),
                    ),
            )
            // header ⋮ dropdown + side flyouts (deferred above messages)
            .when(self.show_chat_menu, |t| {
                t.child(deferred(self.render_chat_menu(cx)).with_priority(100))
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("msg-scroll")
                            .flex_1()
                            .min_h(px(0.))
                            .flex()
                            .flex_col()
                            .px_6()
                            .py_4()
                            .overflow_y_scroll()
                            .track_scroll(&self.msg_scroll)
                            .children(bubbles),
                    )
                    .child(ScrollThumb::new(&self.msg_scroll, {
                        let mut c = theme.foreground;
                        c.a = 0.3;
                        c
                    }))
                    // bubble menu: measured anchor near its row, clamped
                    // on-screen; lives outside the scroll content so the
                    // scrollport can never clip it.
                    .when_some(
                        self.msg_menu.zip(self.msg_menu_at),
                        |t, (mid, (top, mine))| {
                            t.child(
                                deferred(
                                    div()
                                        .absolute()
                                        .top(px(top))
                                        .when(mine, |t| t.right(px(8.)))
                                        .when(!mine, |t| t.left(px(44.)))
                                        .w(px(280.))
                                        .child(self.render_msg_menu(cx, mid)),
                                )
                                .with_priority(100),
                            )
                        },
                    ),
            )
            // click-away overlay (deferred above messages, below menus)
            .when(
                self.show_emoji
                    || self.show_attach
                    || self.show_chat_menu
                    || self.msg_menu.is_some(),
                |t| {
                    t.child(
                        deferred(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .id("panel-dismiss")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_menus();
                                    cx.notify();
                                })),
                        )
                        .with_priority(50),
                    )
                },
            )
            .child(self.render_composer_zone(window, cx))
            // drag & drop highlight (below menus at 30)
            .when(self.drag_hover, |t| {
                let mut dim = theme.muted;
                dim.a = 0.85;
                t.child(
                    deferred(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(dim)
                            .child(
                                div()
                                    .rounded_lg()
                                    .border_2()
                                    .border_color(rgb(crate::component::ACCENT_GREEN))
                                    .bg(theme.popover)
                                    .px_6()
                                    .py_4()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Icon::empty()
                                            .path("icons/download.svg")
                                            .size(px(28.))
                                            .text_color(rgb(crate::component::ACCENT_GREEN)),
                                    )
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_bold()
                                            .text_color(theme.foreground)
                                            .child("Drop files to send"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child("Up to 10 files · 100 MB each"),
                                    ),
                            ),
                    )
                    .with_priority(30),
                )
            })
            // full image preview overlay
            .when_some(self.preview.clone(), |t, prev| {
                t.child(deferred(self.render_preview(cx, prev)).with_priority(200))
            })
            .into_any_element()
    }

    /// Full-size lightbox for an image attachment (click anywhere to close).
    pub(super) fn render_preview(
        &mut self,
        cx: &mut Context<Self>,
        prev: PreviewSel,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut dim = theme.muted;
        dim.a = 0.94;
        let open_path = prev.path.clone();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .bg(dim)
            .id("img-preview-dismiss")
            .on_click(cx.listener(|this, _, _, cx| {
                this.preview = None;
                cx.notify();
            }))
            .child(
                div()
                    .text_lg()
                    .font_bold()
                    .text_color(theme.foreground)
                    .child(prev.name.clone()),
            )
            .child(
                div()
                    .w(px(640.))
                    .h(px(460.))
                    .rounded_lg()
                    .overflow_hidden()
                    .bg(rgb(0x000000))
                    .child(img(prev.path.as_path()).w_full().h_full()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        Button::new("preview-open")
                            .primary()
                            .small()
                            .label("Open externally")
                            .on_click(cx.listener(move |_, _, _, _| {
                                ChatApp::open_path(&open_path);
                            })),
                    )
                    .child(
                        Button::new("preview-close")
                            .ghost()
                            .small()
                            .label("Close")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.preview = None;
                                cx.notify();
                            })),
                    ),
            )
    }
}
