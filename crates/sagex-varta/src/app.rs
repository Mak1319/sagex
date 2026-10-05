use std::collections::HashMap;

use crate::models::{Channel, ChannelId, Message, MsgBody, Route};

pub struct AppState {
    pub route: Route,
    pub channels: Vec<Channel>,
    pub feed: HashMap<ChannelId, Vec<Message>>,
    pub search: String,
    pub search_focus: bool,
    pub draft: String,
    pub markdown_tab: bool,
    pub inspector_open: bool,
    pub show_context: bool,
    pub show_actions: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            route: Route::Channel(ChannelId(4)),
            channels: crate::data::channels(),
            feed: crate::data::messages(),
            search: String::new(),
            search_focus: false,
            draft: String::new(),
            markdown_tab: false,
            inspector_open: true,
            show_context: false,
            show_actions: false,
        }
    }

    pub fn navigate(&mut self, route: Route) {
        self.route = route;
        if let Route::Channel(id) = route {
            for c in self.channels.iter_mut() {
                if c.id == id {
                    c.unread = 0;
                }
            }
        }
    }

    pub fn active_channel(&self) -> Option<&Channel> {
        match self.route {
            Route::Channel(id) => self.channels.iter().find(|c| c.id == id),
            _ => None,
        }
    }

    pub fn send_draft(&mut self) {
        let text = std::mem::take(&mut self.draft);
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let owned: &'static str = Box::leak(text.into_boxed_str());
        let msg = Message {
            author: "You",
            time: "now",
            badge: "AUTHOR",
            mine: true,
            body: MsgBody::Text(owned),
        };
        if let Route::Channel(id) = self.route {
            self.feed.entry(id).or_default().push(msg);
        }
    }
}
