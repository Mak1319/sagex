use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window, deferred, div, prelude::FluentBuilder, px, rgb,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt as _,
    button::{Button, ButtonVariants},
};

use super::ChatApp;
use super::model::ChatKind;
use crate::component::ScrollThumb;

impl ChatApp {
    pub(super) fn render_main(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
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
                                    .child(chat.subtitle.clone()),
                            ),
                    )
                    .child(
                        Button::new("call-btn")
                            .ghost()
                            .large()
                            .icon(Icon::empty().path("icons/video.svg").large())
                            .on_click(|_, _, _| println!("video call")),
                    )
                    .child(
                        Button::new("call-caret-btn")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronDown)
                            .on_click(|_, _, _| println!("call options")),
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
            .into_any_element()
    }
}
