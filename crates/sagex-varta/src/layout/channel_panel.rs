//! Left panel shell — manual `gpui-base` expansion only.
//!
//! Fixed search bar (53px) + pinned header (27px) on top, stateful
//! [`ConversationList`] fills the rest. No sizing here; the parent
//! `resizable_panel()` owns the width.

use gpui_kit::*;

use crate::components::pinned_header::PinnedHeader;
use crate::components::side_search_bar::SideSearchBar;
use crate::layout::conversation_list::ConversationList;
use crate::theme;
use gpui_kit::component::input::InputState;

/// Panel shell. Holds the scrollable list and search entities.
#[derive(IntoElement)]
pub struct ChannelPanel {
    list: Entity<ConversationList>,
    search: Entity<InputState>,
}

impl ChannelPanel {
    pub fn new(list: Entity<ConversationList>, search: Entity<InputState>) -> Self {
        Self { list, search }
    }
}

impl RenderOnce for ChannelPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let pinned = self.list.read(cx).pinned_count();
        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .child(SideSearchBar::new(self.search))
            .child(PinnedHeader::new().count(pinned))
            .child(self.list.clone())
    }
}
