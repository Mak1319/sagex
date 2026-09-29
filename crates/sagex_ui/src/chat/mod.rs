pub mod bubbles;
pub mod composer;
pub mod emoji;
pub mod info;
pub mod list;
pub mod menus;
pub mod model;
pub mod panels;
pub mod rail;
#[allow(dead_code)]
pub mod seed;
pub mod state;
pub mod sync;
pub mod view;

use std::collections::{HashMap, HashSet};

use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, ScrollHandle, Styled, Window, div,
};
use gpui_component::{ActiveTheme, input::InputState};
use tokio::sync::mpsc as tmpsc;

use crate::backend::{ApiClient, SessionStore, StoredSession, UserDto, WsCmd};

use model::{Chat, ChatFilter};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChatMenuSub {
    Mute,
    List,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnStatus {
    #[default]
    Connecting,
    Online,
    Offline,
}

/// Emitted when the local session dies (401 / logout) so the shell can
/// return to the auth gate.
#[derive(Debug, Clone)]
pub enum ChatEvent {
    SignedOut,
}

impl gpui::EventEmitter<ChatEvent> for ChatApp {}

pub struct ChatApp {
    chats: Vec<Chat>,
    active_id: usize,
    filter: ChatFilter,
    search: Entity<InputState>,
    composer: Entity<InputState>,
    emoji_search: Entity<InputState>,
    show_emoji: bool,
    show_attach: bool,
    emoji_tab: usize,
    show_gif: bool,
    show_chat_menu: bool,
    chat_menu_sub: Option<ChatMenuSub>,
    msg_menu: Option<usize>,
    msg_menu_at: Option<(f32, bool)>,
    reply_to: Option<(String, String, u32)>,
    row_menu: Option<(usize, f32)>,
    row_menu_sub: Option<ChatMenuSub>,
    show_group_info: bool,
    pinned: Vec<usize>,
    starred: Vec<usize>,
    notice: Option<String>,
    // Explicit persistent scroll handles: the self-managed Scrollable
    // wrapper loses track (no thumb, no wheel) when content changes, so
    // each scroll region owns a stable handle created once here.
    msg_scroll: ScrollHandle,
    list_scroll: ScrollHandle,
    emoji_scroll: ScrollHandle,
    next_id: usize,
    next_msg: usize,
    pub theme_mode: crate::component::ThemeChoice,
    // ---- live backend ----
    pub api: ApiClient,
    pub store: SessionStore,
    pub sess: Option<StoredSession>,
    pub me: Option<UserDto>,
    pub conn: ConnStatus,
    pub loading: bool,
    /// local chat id -> server room hex
    pub room_server: HashMap<usize, String>,
    /// server message ids already displayed (WS dedup)
    pub known_msgs: HashSet<String>,
    /// user id hex -> display name
    pub names: HashMap<String, String>,
    pub ws_tx: Option<tmpsc::UnboundedSender<WsCmd>>,
    /// room hex -> presence detail ("online: ...")
    pub presence: HashMap<String, String>,
    /// room hex -> (sender name, unix ts) typing indicator
    pub typing: HashMap<String, (String, i64)>,
    pub show_profile: bool,
    pub show_new_chat: bool,
    pub new_is_dm: bool,
    pub user_search: Entity<InputState>,
    pub room_name: Entity<InputState>,
    pub profile_name: Entity<InputState>,
    pub user_results: Vec<UserDto>,
    pub new_members: Vec<String>,
}

impl ChatApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        search: Entity<InputState>,
        composer: Entity<InputState>,
        emoji_search: Entity<InputState>,
        user_search: Entity<InputState>,
        room_name: Entity<InputState>,
        profile_name: Entity<InputState>,
        api: ApiClient,
        store: SessionStore,
    ) -> Self {
        Self {
            active_id: 0,
            filter: ChatFilter::All,
            search,
            composer,
            emoji_search,
            show_emoji: false,
            show_attach: false,
            emoji_tab: 0,
            show_gif: false,
            show_chat_menu: false,
            chat_menu_sub: None,
            msg_menu: None,
            msg_menu_at: None,
            reply_to: None,
            row_menu: None,
            row_menu_sub: None,
            show_group_info: false,
            pinned: vec![],
            starred: vec![],
            notice: None,
            msg_scroll: ScrollHandle::new(),
            list_scroll: ScrollHandle::new(),
            emoji_scroll: ScrollHandle::new(),
            next_id: 1,
            next_msg: 1,
            theme_mode: crate::component::ThemeChoice::System,
            chats: vec![],
            api,
            store,
            sess: None,
            me: None,
            conn: ConnStatus::Connecting,
            loading: true,
            room_server: HashMap::new(),
            known_msgs: HashSet::new(),
            names: HashMap::new(),
            ws_tx: None,
            presence: HashMap::new(),
            typing: HashMap::new(),
            show_profile: false,
            show_new_chat: false,
            new_is_dm: true,
            user_search,
            room_name,
            profile_name,
            user_results: vec![],
            new_members: vec![],
        }
    }

    pub fn my_id(&self) -> String {
        self.me.as_ref().map(|m| m.id.clone()).unwrap_or_default()
    }

    pub fn my_name(&self) -> String {
        self.me
            .as_ref()
            .and_then(|m| m.display_name.clone())
            .filter(|s| !s.is_empty())
            .or_else(|| self.me.as_ref().map(|m| m.username.clone()))
            .unwrap_or_else(|| "You".to_string())
    }

    pub fn display_name_of(&self, user_id: &str) -> String {
        if user_id == self.my_id() {
            return self.my_name();
        }
        self.names
            .get(user_id)
            .cloned()
            .unwrap_or_else(|| user_id.chars().take(8).collect::<String>())
    }
}

impl Render for ChatApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        // placeholder text depends on active state; set once per render is cheap
        self.search.update(cx, |s, cx| {
            s.set_placeholder("Search or start a new chat", window, cx)
        });
        self.composer
            .update(cx, |s, cx| s.set_placeholder("Type a message", window, cx));
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.background)
            .child(self.render_rail(window, cx))
            .child(self.render_list(window, cx))
            .child(self.render_main(window, cx))
    }
}
