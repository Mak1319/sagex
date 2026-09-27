use gpui::{
    ClipboardItem, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{ActiveTheme, Icon};

use super::{ChatApp, ChatMenuSub, model::ChatKind, model::sender_color};
use crate::component::{MenuCard, MenuRow};

impl ChatApp {
    pub(super) fn render_chat_menu(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let pos = self.active_pos();
        let is_group = pos
            .and_then(|p| self.chats.get(p))
            .map(|c| c.kind == ChatKind::Group)
            .unwrap_or(false);
        let mut rows: Vec<gpui::AnyElement> = vec![];
        // (icon, label, action-tag)
        let items: &[(&str, &str, u8)] = if is_group {
            &[
                ("icons/user-plus.svg", "Add member", 1),
                ("icons/info.svg", "Group info", 2),
                ("icons/search.svg", "Search", 3),
                ("icons/square-check-big.svg", "Select messages", 4),
                ("icons/bell-off.svg", "Mute notifications", 5),
                ("icons/timer.svg", "Disappearing messages", 6),
                ("icons/heart.svg", "Add to favourites", 7),
                ("icons/list-plus.svg", "Add to list", 8),
                ("icons/download.svg", "Export chat", 9),
                ("icons/x.svg", "Close chat", 10),
            ]
        } else {
            &[
                ("icons/info.svg", "Contact info", 2),
                ("icons/search.svg", "Search", 3),
                ("icons/square-check-big.svg", "Select messages", 4),
                ("icons/bell-off.svg", "Mute notifications", 5),
                ("icons/timer.svg", "Disappearing messages", 6),
                ("icons/heart.svg", "Add to favourites", 7),
                ("icons/list-plus.svg", "Add to list", 8),
                ("icons/download.svg", "Export chat", 9),
                ("icons/x.svg", "Close chat", 10),
            ]
        };
        for (ix, (icon, label, tag)) in items.iter().enumerate() {
            let label = label.to_string();
            let tag = *tag;
            let has_sub = tag == 5 || tag == 8;
            let sub_open = (tag == 5 && self.chat_menu_sub == Some(ChatMenuSub::Mute))
                || (tag == 8 && self.chat_menu_sub == Some(ChatMenuSub::List));
            let mut row = MenuRow::new(("chat-menu-row", ix), icon, label)
                .highlighted(sub_open)
                .on_click(cx.listener(move |this, _, _, cx| match tag {
                    5 => {
                        this.chat_menu_sub = if this.chat_menu_sub == Some(ChatMenuSub::Mute) {
                            None
                        } else {
                            Some(ChatMenuSub::Mute)
                        };
                        cx.notify();
                    }
                    8 => {
                        this.chat_menu_sub = if this.chat_menu_sub == Some(ChatMenuSub::List) {
                            None
                        } else {
                            Some(ChatMenuSub::List)
                        };
                        cx.notify();
                    }
                    _ => this.chat_menu_action(tag, cx),
                }));
            if has_sub {
                row = row.trailing(
                    Icon::empty()
                        .path("icons/chevron-right.svg")
                        .size(px(14.))
                        .text_color(cx.theme().muted_foreground),
                );
            }
            rows.push(row.into_any_element());
        }
        // bottom rows after divider
        let bottom: &[(&str, &str, u8)] = if is_group {
            &[
                ("icons/circle-minus.svg", "Clear chat", 11),
                ("icons/log-out.svg", "Exit group", 12),
            ]
        } else {
            &[
                ("icons/circle-minus.svg", "Clear chat", 11),
                ("icons/trash-2.svg", "Delete chat", 12),
            ]
        };
        for (ix, (icon, label, tag)) in bottom.iter().enumerate() {
            let label = label.to_string();
            let tag = *tag;
            rows.push(
                MenuRow::new(("chat-menu-bottom", ix), icon, label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.chat_menu_action(tag, cx);
                    }))
                    .into_any_element(),
            );
        }
        let sub = self.chat_menu_sub;
        let mut card = MenuCard::new();
        card.extend(rows);
        div()
            .absolute()
            .top(px(56.))
            .right(px(12.))
            .w(px(250.))
            .child(card)
            .when_some(sub, |t, which| {
                let (top, items): (f32, Vec<(&str, &str)>) = match which {
                    ChatMenuSub::Mute => (
                        208.,
                        vec![
                            ("8 hours", "Muted for 8 hours"),
                            ("1 week", "Muted for 1 week"),
                            ("Always", "Muted"),
                        ],
                    ),
                    ChatMenuSub::List => (
                        316.,
                        vec![
                            ("Favorites", "Added to Favorites"),
                            ("Work", "Added to Work"),
                            ("Family", "Added to Family"),
                        ],
                    ),
                };
                let mut subs = MenuCard::new();
                for (ix, (label, done)) in items.into_iter().enumerate() {
                    let done = done.to_string();
                    subs.extend([MenuRow::new(
                        ("chat-menu-sub", ix),
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
                        .top(px(top))
                        .right(px(262.))
                        .w(px(200.))
                        .child(subs),
                )
            })
    }

    fn chat_menu_action(&mut self, tag: u8, cx: &mut Context<Self>) {
        match tag {
            2 => {
                self.show_group_info = true;
                self.close_menus();
            }
            7 => {
                if let Some(c) = self.active_chat_mut() {
                    c.fav = !c.fav;
                    self.notice = Some(if c.fav {
                        "Added to favourites".to_string()
                    } else {
                        "Removed from favourites".to_string()
                    });
                }
                self.close_menus();
            }
            10 => {
                // close chat: jump to first chat
                if let Some(first) = self.chats.first().map(|c| c.id) {
                    self.active_id = first;
                }
                self.show_group_info = false;
                self.notice = Some("Chat closed".to_string());
                self.close_menus();
            }
            11 => {
                if let Some(c) = self.active_chat_mut() {
                    c.messages.clear();
                    self.notice = Some("Chat cleared".to_string());
                }
                self.close_menus();
            }
            12 => {
                self.notice = Some("Done".to_string());
                self.close_menus();
            }
            _ => {
                self.notice = Some("Done (UI preview)".to_string());
                self.close_menus();
            }
        }
        cx.notify();
    }
    pub(super) fn render_msg_menu(
        &mut self,
        cx: &mut Context<Self>,
        mid: usize,
    ) -> gpui::AnyElement {
        const REACTS: &[&str] = &["👍", "❤️", "😂", "😮", "😢", "🙏"];
        let Some(m) = self.find_msg(mid) else {
            return div().into_any_element();
        };
        let sender = m.sender.clone();
        let text = m.text.clone();
        let name = if m.mine {
            "yourself".to_string()
        } else {
            sender.clone()
        };
        // reaction pill
        let mut react_row = div().flex().flex_row().items_center().gap_1().px_3().py_2();
        for (rix, r) in REACTS.iter().enumerate() {
            let glyph = r.to_string();
            let label = glyph.clone();
            react_row = react_row.child(
                div()
                    .id(("react", rix))
                    .text_xl()
                    .rounded_md()
                    .cursor_pointer()
                    .px_1()
                    .hover(|s| s.bg(cx.theme().muted))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(m) = this.find_msg_mut(mid) {
                            if m.reactions.contains(&glyph) {
                                m.reactions.retain(|x| x != &glyph);
                            } else {
                                m.reactions.push(glyph.clone());
                            }
                        }
                        this.msg_menu = None;
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        react_row = react_row.child(
            div()
                .id("react-more")
                .text_xl()
                .rounded_md()
                .cursor_pointer()
                .px_1()
                .text_color(cx.theme().muted_foreground)
                .hover(|s| s.bg(cx.theme().muted))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_emoji = true;
                    this.msg_menu = None;
                    cx.notify();
                }))
                .child("+"),
        );
        // menu rows: (icon, label, action-tag)
        let rows: &[(&str, String, u8)] = &[
            ("icons/reply.svg", "Reply".to_string(), 1),
            ("icons/message-circle.svg", "Reply privately".to_string(), 2),
            ("icons/message-circle.svg", format!("Message {sender}"), 3),
            ("icons/copy.svg", "Copy".to_string(), 4),
            ("icons/smile.svg", "React".to_string(), 5),
            ("icons/forward.svg", "Forward".to_string(), 6),
            ("icons/pin.svg", "Pin".to_string(), 7),
            ("icons/sparkles.svg", "Ask Meta AI".to_string(), 8),
            ("icons/star.svg", "Star".to_string(), 9),
            ("icons/flag.svg", "Report".to_string(), 10),
            ("icons/trash-2.svg", "Delete".to_string(), 11),
        ];
        let mut card = MenuCard::new();
        for (ix, (icon, label, tag)) in rows.iter().enumerate() {
            let label = label.clone();
            let tag = *tag;
            // "Message X" only makes sense for others' group messages
            if tag == 3 && (m.mine || sender == "System") {
                continue;
            }
            if tag == 10 {
                card.extend([div()
                    .h(px(1.))
                    .my_1()
                    .bg(cx.theme().border)
                    .into_any_element()]);
            }
            let (t, n, s) = (text.clone(), name.clone(), sender.clone());
            card.extend([MenuRow::new(("msg-menu-row", ix), icon, label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.msg_menu_action(mid, tag, t.clone(), n.clone(), s.clone(), window, cx);
                }))
                .into_any_element()]);
        }
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .rounded_full()
                    .bg(cx.theme().popover)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_lg()
                    .child(react_row),
            )
            .child(card)
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn msg_menu_action(
        &mut self,
        mid: usize,
        tag: u8,
        text: String,
        name: String,
        sender: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match tag {
            1 | 2 => {
                // reply state drives the preview bar above the composer
                let clean = text.replace(['\n', '\r'], " ");
                self.reply_to = Some((name.clone(), clean, sender_color(&sender)));
                if tag == 2 {
                    self.notice = Some(format!("Private reply to {name}"));
                }
                self.msg_menu = None;
            }
            3 => {
                self.notice = Some(format!("Open chat with {name} (UI preview)"));
                self.msg_menu = None;
            }
            4 => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.notice = Some("Copied to clipboard".to_string());
                self.msg_menu = None;
            }
            5 => {
                if let Some(m) = self.find_msg_mut(mid) {
                    if m.reactions.contains(&"❤️".to_string()) {
                        m.reactions.retain(|x| x != "❤️");
                    } else {
                        m.reactions.push("❤️".to_string());
                    }
                }
                self.msg_menu = None;
            }
            6 => {
                self.notice = Some("Forward (UI preview)".to_string());
                self.msg_menu = None;
            }
            7 => {
                if self.pinned.contains(&mid) {
                    self.pinned.retain(|x| x != &mid);
                    self.notice = Some("Unpinned".to_string());
                } else {
                    self.pinned.push(mid);
                    self.notice = Some("Pinned".to_string());
                }
                self.msg_menu = None;
            }
            8 => {
                let id = self.active_id;
                self.push_message(id, "Meta AI", "🤖 I can help with that.".to_string(), false);
                // fix preview/time like a normal incoming message
                if let Some(c) = self.active_chat_mut() {
                    c.last_time = "now".to_string();
                }
                self.msg_menu = None;
            }
            9 => {
                if self.starred.contains(&mid) {
                    self.starred.retain(|x| x != &mid);
                    self.notice = Some("Unstarred".to_string());
                } else {
                    self.starred.push(mid);
                    self.notice = Some("Starred".to_string());
                }
                self.msg_menu = None;
            }
            10 => {
                self.notice = Some("Reported (UI preview)".to_string());
                self.msg_menu = None;
            }
            11 => {
                // WhatsApp-style: keep a "deleted" record in place.
                if let Some(m) = self.find_msg_mut(mid) {
                    m.deleted = true;
                    m.reactions.clear();
                    m.link = None;
                }
                self.pinned.retain(|x| x != &mid);
                self.starred.retain(|x| x != &mid);
                self.notice = Some("Message deleted".to_string());
                self.msg_menu = None;
            }
            _ => {
                self.msg_menu = None;
            }
        }
        cx.notify();
    }
}
