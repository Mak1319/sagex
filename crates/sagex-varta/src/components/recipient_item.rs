//! Recipient unlock row — manual `gpui-base` expansion only.
//!
//! One scrollable item of the deck's second container: `DeckCheckbox`,
//! avatar + presence, name + handle, UNLOCK/LOCK badge with glyph.

use std::rc::Rc;

use gpui_kit::*;

use crate::components::avatar::Avatar;
use crate::components::deck_badge::DeckBadge;
use crate::components::deck_checkbox::DeckCheckbox;
use crate::theme;

type ItemHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct RecipientItem {
    id: SharedString,
    name: SharedString,
    handle: SharedString,
    initial: SharedString,
    avatar_bg: u32,
    avatar_fg: u32,
    presence: u32,
    checked: bool,
    unlocked: bool,
    on_toggle: Option<ItemHandler>,
}

impl RecipientItem {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            handle: "".into(),
            initial: "?".into(),
            avatar_bg: 0xf1f5f9,
            avatar_fg: theme::colors::MUTED,
            presence: 0x64748b,
            checked: false,
            unlocked: false,
            on_toggle: None,
        }
    }

    pub fn handle(mut self, handle: impl Into<SharedString>) -> Self {
        self.handle = handle.into();
        self
    }

    pub fn avatar(mut self, initial: impl Into<SharedString>, bg: u32, fg: u32) -> Self {
        self.initial = initial.into();
        self.avatar_bg = bg;
        self.avatar_fg = fg;
        self
    }

    pub fn presence(mut self, presence: u32) -> Self {
        self.presence = presence;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn unlocked(mut self, unlocked: bool) -> Self {
        self.unlocked = unlocked;
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for RecipientItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut check = DeckCheckbox::new(format!("{}-check", self.id), self.checked);
        if let Some(handler) = self.on_toggle {
            check = check.on_toggle(move |event, window, cx| handler(event, window, cx));
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_2()
            .w_full()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_b_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .child(check)
            .child(
                Avatar::new(self.initial)
                    .bg(self.avatar_bg)
                    .fg(self.avatar_fg)
                    .size(24.0)
                    .border(theme::colors::SURFACE_LOWEST)
                    .presence(self.presence),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::INK))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(self.name),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(self.handle),
                    ),
            )
            .child(if self.unlocked {
                DeckBadge::unlock("UNLOCK")
            } else {
                DeckBadge::lock("LOCK")
            })
    }
}
