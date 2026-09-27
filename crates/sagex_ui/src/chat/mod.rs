pub mod bubbles;
pub mod composer;
pub mod emoji;
pub mod info;
pub mod list;
pub mod menus;
pub mod model;
pub mod rail;
pub mod seed;
pub mod state;
pub mod view;

use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, ScrollHandle, Styled, Subscription,
    Window, div,
};
use gpui_component::{ActiveTheme, input::InputState};

use model::{Chat, ChatFilter};
use seed::seed_chats;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChatMenuSub {
    Mute,
    List,
}

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
    pub(crate) appearance_sub: Option<Subscription>,
}

impl ChatApp {
    pub fn new(
        search: Entity<InputState>,
        composer: Entity<InputState>,
        emoji_search: Entity<InputState>,
    ) -> Self {
        let chats = seed_chats();
        Self {
            active_id: 1,
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
            next_id: 100,
            next_msg: 1000,
            theme_mode: crate::component::ThemeChoice::System,
            appearance_sub: None,
            chats,
        }
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
