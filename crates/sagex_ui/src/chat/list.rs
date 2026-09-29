use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt as _,
    button::{Button, ButtonVariants},
    input::Input,
};

use super::{
    ChatApp, ChatMenuSub,
    model::{Chat, ChatFilter, ChatKind},
};
use crate::component::{Avatar, MenuCard, MenuRow, ScrollThumb, UnreadBadge};

impl ChatApp {
    // ---------- chat list ----------
    pub(super) fn pill(
        &self,
        cx: &mut Context<Self>,
        id: &'static str,
        label: &str,
        f: ChatFilter,
    ) -> impl IntoElement {
        let active = self.filter == f;
        let mut b = Button::new(id).small().label(label.to_string());
        b = if active { b.primary() } else { b.outline() };
        b.on_click(cx.listener(move |this, _, _, cx| {
            this.filter = f;
            cx.notify();
        }))
    }

    pub(super) fn render_row(
        &mut self,
        cx: &mut Context<Self>,
        chat: Chat,
        selected: bool,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let id = chat.id;
        div()
            .id(("chat-row", id))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .when(selected, |t| t.bg(theme.muted))
            .hover(|s| s.bg(theme.muted))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_remote(id, cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    this.close_menus();
                    let y = this.row_anchor_for(cx, id).unwrap_or(8.0);
                    this.row_menu = Some((id, y));
                    cx.notify();
                }),
            )
            .child(
                Avatar::new(chat.initials.clone())
                    .color(chat.color)
                    .diameter(px(44.))
                    .glyph(px(13.)),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(
                                div()
                                    .text_color(theme.foreground)
                                    .truncate()
                                    .child(chat.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .flex_shrink_0()
                                    .text_color(theme.muted_foreground)
                                    .child(chat.last_time.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .truncate()
                                    .text_color(theme.muted_foreground)
                                    .child(chat.preview()),
                            )
                            .child(UnreadBadge::new(chat.unread)),
                    ),
            )
    }

    pub(super) fn render_list(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ids = self.visible(cx);
        let active = self.active_id;
        let by_id: std::collections::HashMap<usize, Chat> =
            self.chats.iter().map(|c| (c.id, c.clone())).collect();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for id in ids {
            if let Some(c) = by_id.get(&id) {
                rows.push(
                    self.render_row(cx, c.clone(), id == active)
                        .into_any_element(),
                );
            }
        }

        div()
            .w(px(380.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(theme.background)
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .pt_4()
                    .pb_2()
                    .child(
                        div()
                            .text_xl()
                            .font_bold()
                            .text_color(theme.foreground)
                            .child("WhatsApp"),
                    )
                    .child(
                        div().flex().flex_row().gap_1().child(
                            Button::new("new-chat")
                                .ghost()
                                .small()
                                .icon(IconName::Plus)
                                .label("New")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    // Real flow: open the new-chat panel (user
                                    // search → DM/group on the server).
                                    this.show_new_chat = true;
                                    this.show_profile = false;
                                    this.search_users(cx);
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .child(
                div()
                    .px_4()
                    .pb_2()
                    .child(Input::new(&self.search).cleanable(true).w_full()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .px_4()
                    .pb_2()
                    .child(self.pill(cx, "f-all", "All", ChatFilter::All))
                    .child(self.pill(cx, "f-unread", "Unread", ChatFilter::Unread))
                    .child(self.pill(cx, "f-fav", "Favourites", ChatFilter::Favourites))
                    .child(self.pill(cx, "f-groups", "Groups", ChatFilter::Groups))
                    .child(self.pill(cx, "f-arch", "Archived", ChatFilter::Archived)),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("chat-list-scroll")
                            .flex_1()
                            .min_h(px(0.))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .pl(px(8.))
                            .pr(px(10.))
                            .pb_4()
                            .overflow_y_scroll()
                            .track_scroll(&self.list_scroll)
                            .children(rows),
                    )
                    .child(ScrollThumb::new(&self.list_scroll, {
                        let mut c = theme.foreground;
                        c.a = 0.3;
                        c
                    }))
                    // row context menu + click-away overlay
                    .when_some(self.row_menu, |t, (cid, y)| {
                        t.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .id("row-menu-dismiss")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_menus();
                                    cx.notify();
                                })),
                        )
                        .child(self.render_row_menu(cx, cid, y))
                    }),
            )
    }

    /// Right-click chat menu (Archive, Mute ›, Pin, Unread, Favourites,
    /// Add to list ›, Clear, Exit/Delete), frozen next to the row.
    pub(super) fn render_row_menu(
        &mut self,
        cx: &mut Context<Self>,
        cid: usize,
        y: f32,
    ) -> gpui::AnyElement {
        let chat = self.chats.iter().find(|c| c.id == cid).cloned();
        let Some(chat) = chat else {
            return div().into_any_element();
        };
        let is_group = chat.kind == ChatKind::Group;
        let mut card = MenuCard::new();
        // (icon, label, action-tag)
        let items: &[(&str, String, u8)] = &[
            (
                "icons/archive.svg",
                if chat.archived {
                    "Unarchive chat".to_string()
                } else {
                    "Archive chat".to_string()
                },
                1,
            ),
            ("icons/bell-off.svg", "Mute notifications".to_string(), 2),
            (
                "icons/pin.svg",
                if chat.chat_pinned {
                    "Unpin chat".to_string()
                } else {
                    "Pin chat".to_string()
                },
                3,
            ),
            ("icons/message-circle.svg", "Mark as unread".to_string(), 4),
            (
                "icons/heart.svg",
                if chat.fav {
                    "Remove from favourites".to_string()
                } else {
                    "Add to favourites".to_string()
                },
                5,
            ),
            ("icons/list-plus.svg", "Add to list".to_string(), 6),
            ("icons/circle-minus.svg", "Clear chat".to_string(), 7),
            (
                if is_group {
                    "icons/log-out.svg"
                } else {
                    "icons/trash-2.svg"
                },
                if is_group {
                    "Exit group".to_string()
                } else {
                    "Delete chat".to_string()
                },
                8,
            ),
        ];
        for (ix, (icon, label, tag)) in items.iter().enumerate() {
            if *tag == 7 {
                card.extend([div()
                    .h(px(1.))
                    .my_1()
                    .bg(cx.theme().border)
                    .into_any_element()]);
            }
            let label = label.clone();
            let tag = *tag;
            let has_sub = tag == 2 || tag == 6;
            let sub_open = (tag == 2 && self.row_menu_sub == Some(ChatMenuSub::Mute))
                || (tag == 6 && self.row_menu_sub == Some(ChatMenuSub::List));
            let mut row = MenuRow::new(("row-menu-row", ix), icon, label)
                .highlighted(sub_open)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.row_menu_action(cid, tag, cx);
                }));
            if has_sub {
                row = row.trailing(
                    Icon::empty()
                        .path("icons/chevron-right.svg")
                        .size(px(14.))
                        .text_color(cx.theme().muted_foreground),
                );
            }
            card.extend([row.into_any_element()]);
        }
        let sub = self.row_menu_sub;
        // list pane is 380px; pin the card to the right edge so it sits
        // on the row. Flyouts open left so they stay inside the pane.
        const LIST_W: f32 = 380.0;
        const MENU_W: f32 = 250.0;
        const FLYOUT_W: f32 = 200.0;
        let left = LIST_W - MENU_W - 8.0;
        div()
            .absolute()
            .top(px(y))
            .left(px(left))
            .w(px(MENU_W))
            .child(card)
            .when_some(sub, |t, which| {
                let opts: Vec<(&str, &str)> = match which {
                    ChatMenuSub::Mute => vec![
                        ("8 hours", "Muted for 8 hours"),
                        ("1 week", "Muted for 1 week"),
                        ("Always", "Muted"),
                    ],
                    ChatMenuSub::List => vec![
                        ("Favorites", "Added to Favorites"),
                        ("Work", "Added to Work"),
                        ("Family", "Added to Family"),
                    ],
                };
                let mut subs = MenuCard::new();
                for (six, (label, done)) in opts.into_iter().enumerate() {
                    let done = done.to_string();
                    subs.extend([MenuRow::new(
                        ("row-menu-sub", six),
                        "icons/chevron-right.svg",
                        label.to_string(),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.notice = Some(done.clone());
                        this.close_menus();
                        cx.notify();
                    }))
                    .into_any_element()]);
                }
                t.child(
                    div()
                        .absolute()
                        .top(px(0.))
                        .left(px(-(FLYOUT_W + 8.0)))
                        .w(px(FLYOUT_W))
                        .child(subs),
                )
            })
            .into_any_element()
    }

    fn row_menu_action(&mut self, cid: usize, tag: u8, cx: &mut Context<Self>) {
        match tag {
            1 => {
                if let Some(c) = self.chats.iter_mut().find(|c| c.id == cid) {
                    c.archived = !c.archived;
                    self.notice = Some(if c.archived {
                        "Chat archived".to_string()
                    } else {
                        "Chat unarchived".to_string()
                    });
                }
                self.close_menus();
            }
            2 => {
                self.row_menu_sub = if self.row_menu_sub == Some(ChatMenuSub::Mute) {
                    None
                } else {
                    Some(ChatMenuSub::Mute)
                };
            }
            3 => {
                if let Some(c) = self.chats.iter_mut().find(|c| c.id == cid) {
                    c.chat_pinned = !c.chat_pinned;
                    self.notice = Some(if c.chat_pinned {
                        "Chat pinned".to_string()
                    } else {
                        "Chat unpinned".to_string()
                    });
                }
                self.close_menus();
            }
            4 => {
                if let Some(c) = self.chats.iter_mut().find(|c| c.id == cid) {
                    c.unread = c.unread.max(1);
                    self.notice = Some("Marked as unread".to_string());
                }
                self.close_menus();
            }
            5 => {
                if let Some(c) = self.chats.iter_mut().find(|c| c.id == cid) {
                    c.fav = !c.fav;
                    self.notice = Some(if c.fav {
                        "Added to favourites".to_string()
                    } else {
                        "Removed from favourites".to_string()
                    });
                }
                self.close_menus();
            }
            6 => {
                self.row_menu_sub = if self.row_menu_sub == Some(ChatMenuSub::List) {
                    None
                } else {
                    Some(ChatMenuSub::List)
                };
            }
            7 => {
                if let Some(c) = self.chats.iter_mut().find(|c| c.id == cid) {
                    c.messages.clear();
                    self.notice = Some("Chat cleared".to_string());
                }
                self.close_menus();
            }
            _ => {
                self.notice = Some("Done".to_string());
                self.close_menus();
            }
        }
        cx.notify();
    }
}
