use gpui::App;

use super::{
    ChatApp,
    model::{Chat, ChatFilter, ChatKind, Message, MessageKind, MessageStatus, extract_link},
};

impl ChatApp {
    pub(super) fn active_pos(&self) -> Option<usize> {
        self.chats.iter().position(|c| c.id == self.active_id)
    }

    pub(super) fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().to_lowercase()
    }

    pub(super) fn visible(&self, cx: &App) -> Vec<usize> {
        let q = self.query(cx);
        let mut ids: Vec<usize> = self
            .chats
            .iter()
            .filter(|c| match self.filter {
                ChatFilter::All => !c.archived,
                ChatFilter::Unread => !c.archived && c.unread > 0,
                ChatFilter::Favourites => !c.archived && c.fav,
                ChatFilter::Groups => !c.archived && c.kind == ChatKind::Group,
                ChatFilter::Archived => c.archived,
            })
            .filter(|c| {
                q.is_empty()
                    || c.name.to_lowercase().contains(&q)
                    || c.preview().to_lowercase().contains(&q)
            })
            .map(|c| c.id)
            .collect();
        // pinned chats first, stable otherwise
        ids.sort_by_key(|id| {
            let pinned = self
                .chats
                .iter()
                .find(|c| c.id == *id)
                .map(|c| !c.chat_pinned)
                .unwrap_or(true);
            (pinned, *id)
        });
        ids
    }

    pub(super) fn push_message(&mut self, chat_id: usize, sender: &str, text: String, mine: bool) {
        let id = self.next_msg;
        self.next_msg += 1;
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == chat_id) {
            c.messages.push(Message {
                id,
                sender: sender.to_string(),
                link: extract_link(&text),
                text: text.clone(),
                time: "now".to_string(),
                mine,
                date: "Today".to_string(),
                kind: MessageKind::Text,
                reactions: vec![],
                ticks: MessageStatus::Sent,
                deleted: false,
                server_id: None,
            });
            c.last_time = "now".to_string();
            let _ = text;
        }
    }

    /// Push a typed (non-text) message; `text` feeds list preview + search.
    pub(super) fn push_kind(&mut self, chat_id: usize, kind: MessageKind, text: String) {
        let id = self.next_msg;
        self.next_msg += 1;
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == chat_id) {
            c.messages.push(Message {
                id,
                sender: "You".to_string(),
                link: None,
                text,
                time: "now".to_string(),
                mine: true,
                date: "Today".to_string(),
                kind,
                reactions: vec![],
                ticks: MessageStatus::Sent,
                deleted: false,
                server_id: None,
            });
            c.last_time = "now".to_string();
        }
    }

    pub(super) fn close_menus(&mut self) {
        self.show_emoji = false;
        self.show_attach = false;
        self.show_chat_menu = false;
        self.chat_menu_sub = None;
        self.msg_menu = None;
        self.msg_menu_at = None;
        self.row_menu = None;
        self.row_menu_sub = None;
    }

    pub(super) fn active_chat_mut(&mut self) -> Option<&mut Chat> {
        let id = self.active_id;
        self.chats.iter_mut().find(|c| c.id == id)
    }

    pub(super) fn find_msg_mut(&mut self, msg_id: usize) -> Option<&mut Message> {
        self.chats
            .iter_mut()
            .flat_map(|c| c.messages.iter_mut())
            .find(|m| m.id == msg_id)
    }

    pub(super) fn find_msg(&self, msg_id: usize) -> Option<Message> {
        self.chats
            .iter()
            .flat_map(|c| c.messages.iter())
            .find(|m| m.id == msg_id)
            .cloned()
    }

    /// Viewport-clamped top for a floating menu near a scroll child:
    /// centers on the row, clamped on-screen. `menu_h` estimates height.
    fn clamp_menu_top(handle: &gpui::ScrollHandle, ix: usize, menu_h: f32) -> f32 {
        let vp = f32::from(handle.bounds().size.height);
        match handle.bounds_for_item(ix) {
            Some(b) => {
                let center = f32::from(b.top()) - f32::from(handle.bounds().top())
                    + f32::from(b.size.height) / 2.0
                    - f32::from(handle.offset().y);
                (center - menu_h / 2.0).clamp(8.0, (vp - menu_h - 8.0).max(8.0))
            }
            // first layout: center vertically
            None => ((vp - menu_h).max(0.0)) / 2.0,
        }
    }

    /// Snapshot anchor for a bubble menu: (top-px in scroll wrapper,
    /// mine-side). Frozen at open time so scrolling never moves the menu.
    pub(super) fn bubble_anchor_for(&self, mid: usize) -> Option<(f32, bool)> {
        let pos = self.active_pos()?;
        let chat = self.chats.get(pos)?;
        // child index mirrors the bubbles loop (date pills + rows in order)
        let mut last_date = String::new();
        let mut child_ix = 0usize;
        let mut found: Option<(usize, bool)> = None;
        for m in chat.messages.iter() {
            if m.date != last_date {
                last_date = m.date.clone();
                child_ix += 1;
            }
            if m.id == mid {
                found = Some((child_ix, m.mine));
                break;
            }
            child_ix += 1;
        }
        let (ix, mine) = found?;
        Some((Self::clamp_menu_top(&self.msg_scroll, ix, 470.0), mine))
    }

    /// Snapshot anchor for a row menu: top-px in the list wrapper.
    pub(super) fn row_anchor_for(&self, cx: &App, cid: usize) -> Option<f32> {
        let ix = self.visible(cx).iter().position(|id| *id == cid)?;
        Some(Self::clamp_menu_top(&self.list_scroll, ix, 340.0))
    }
}
