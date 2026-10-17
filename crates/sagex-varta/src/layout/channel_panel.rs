//! Left panel shell — manual `gpui-base` expansion only.
//!
//! New-conversation combobox on top (base primitives only), then the
//! fixed search bar (53px) + pinned header (27px), stateful
//! [`ConversationList`] fills the rest. No sizing here; the parent
//! `resizable_panel()` owns the width.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::components::pinned_header::PinnedHeader;
use crate::components::side_search_bar::SideSearchBar;
use crate::layout::conversation_list::ConversationList;
use crate::theme;
use gpui_kit::component::input::InputState;

/// Menu callbacks carry the real window: the outside-click closer cannot
/// fabricate a click event, so no handler invents one.
type NewHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Panel shell. Holds the scrollable list and search entities.
#[derive(IntoElement)]
//(!) Combbox
pub struct ChannelPanel {
    list: Entity<ConversationList>,
    search: Entity<InputState>,
    new_open: bool,
    on_new_toggle: Option<NewHandler>,
    on_new_single: Option<NewHandler>,
    on_new_group: Option<NewHandler>,
}

impl ChannelPanel {
    pub fn new(list: Entity<ConversationList>, search: Entity<InputState>) -> Self {
        Self {
            list,
            search,
            new_open: false,
            on_new_toggle: None,
            on_new_single: None,
            on_new_group: None,
        }
    }

    pub fn new_menu_open(mut self, open: bool) -> Self {
        self.new_open = open;
        self
    }

    pub fn on_new_toggle(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new_toggle = Some(Rc::new(handler));
        self
    }

    pub fn on_new_single(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new_single = Some(Rc::new(handler));
        self
    }

    pub fn on_new_group(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new_group = Some(Rc::new(handler));
        self
    }
}

/// New-conversation menu option — RenderOnce so no free function builds
/// component trees.
#[derive(IntoElement)]
struct NewMenuOption {
    id: SharedString,
    icon: SharedString,
    label: SharedString,
    on_select: Option<NewHandler>,
}

impl NewMenuOption {
    fn new(
        id: impl Into<SharedString>,
        icon: impl Into<SharedString>,
        label: impl Into<SharedString>,
        on_select: Option<NewHandler>,
    ) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            label: label.into(),
            on_select,
        }
    }
}

impl RenderOnce for NewMenuOption {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut opt = BaseButton::new(ElementId::Name(self.id))
            .w_full()
            .h(px(32.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_start()
            .gap_2()
            .px_3()
            .rounded(px(theme::radius::DEFAULT))
            .text_size(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme::rgb(theme::colors::INK))
            .hover(|s| s.bg(theme::rgb(theme::colors::ROW_HOVER)));
        if let Some(handler) = self.on_select {
            opt = opt.on_click(move |_, window, cx| handler(window, cx));
        }
        opt.child(
            svg()
                .path(self.icon)
                .w(px(14.0))
                .h(px(14.0))
                .flex_shrink_0()
                .text_color(theme::rgb(theme::colors::MUTED)),
        )
        .child(self.label)
    }
}

impl RenderOnce for ChannelPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let pinned = self.list.read(cx).pinned_count();
        let mut new_button = BaseButton::new(ElementId::Name("channel-new".into()))
            .w_full()
            .h(px(32.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap_2()
            .rounded(px(theme::radius::DEFAULT))
            .bg(theme::rgb(theme::colors::PRIMARY))
            .text_size(px(12.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme::rgb(theme::colors::ON_PRIMARY));
        if let Some(handler) = self.on_new_toggle.clone() {
            new_button = new_button.on_click(move |_, window, cx| handler(window, cx));
        }
        new_button = new_button
            .child(
                svg()
                    .path("icons/user-plus.svg")
                    .w(px(14.0))
                    .h(px(14.0))
                    .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
            )
            .child("New conversation")
            .child(
                svg()
                    .path("icons/chevron-down.svg")
                    .w(px(12.0))
                    .h(px(12.0))
                    .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
            );
        let mut col = div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .child(div().w_full().px_3().pt_2().child(new_button));
        if self.new_open {
            let close = self.on_new_toggle.clone();
            col = col.child(
                div()
                    .w_full()
                    .px_3()
                    .pb_1()
                    .on_mouse_down_out(move |_, window, cx| {
                        if let Some(close) = close.clone() {
                            close(window, cx);
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .w_full()
                            .p_1()
                            .rounded(px(theme::radius::DEFAULT))
                            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                            .border_1()
                            .border_color(theme::rgb(theme::colors::GRID_L2))
                            .child(NewMenuOption::new(
                                "channel-new-single",
                                "icons/chat-bubble-left.svg",
                                "Single chat",
                                self.on_new_single.clone(),
                            ))
                            .child(NewMenuOption::new(
                                "channel-new-group",
                                "icons/users.svg",
                                "Group chat",
                                self.on_new_group.clone(),
                            )),
                    ),
            );
        }
        col.child(SideSearchBar::new(self.search))
            .child(PinnedHeader::new().count(pinned))
            .child(self.list.clone())
    }
}
