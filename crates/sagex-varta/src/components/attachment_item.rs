//! Attachment clearance row — manual `gpui-base` expansion only.
//!
//! Accordion item of the deck's first container: icon, name + policy line,
//! badge with glyph, size + key note, chevron button. Expanded state shows
//! the detail slot under the row.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::components::deck_badge::DeckBadge;
use crate::theme;

type ItemHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

const DOWN: &str = "icons/chevron-down.svg";
const UP: &str = "icons/chevron-up.svg";

#[derive(IntoElement)]
pub struct AttachmentItem {
    id: SharedString,
    title: SharedString,
    policy: SharedString,
    icon: SharedString,
    icon_fg: u32,
    badge: Option<DeckBadge>,
    size: SharedString,
    keys: SharedString,
    tinted: bool,
    expanded: bool,
    detail: SharedString,
    on_toggle: Option<ItemHandler>,
}

impl AttachmentItem {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            policy: "".into(),
            icon: "icons/photo.svg".into(),
            icon_fg: theme::colors::PRIMARY,
            badge: None,
            size: "".into(),
            keys: "".into(),
            tinted: false,
            expanded: false,
            detail: "".into(),
            on_toggle: None,
        }
    }

    pub fn policy(mut self, policy: impl Into<SharedString>) -> Self {
        self.policy = policy.into();
        self
    }

    pub fn icon(mut self, icon: &'static str, fg: u32) -> Self {
        self.icon = icon.into();
        self.icon_fg = fg;
        self
    }

    pub fn badge(mut self, badge: DeckBadge) -> Self {
        self.badge = Some(badge);
        self
    }

    pub fn size(mut self, size: impl Into<SharedString>) -> Self {
        self.size = size.into();
        self
    }

    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = keys.into();
        self
    }

    pub fn tinted(mut self, tinted: bool) -> Self {
        self.tinted = tinted;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = detail.into();
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

impl RenderOnce for AttachmentItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut chev = BaseButton::new(ElementId::Name(
            format!("{}-chev", self.id).into(),
        ))
        .w(px(20.0))
        .h(px(20.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(theme::radius::DEFAULT))
        .text_color(theme::rgb(theme::colors::MUTED));
        if let Some(handler) = self.on_toggle {
            chev = chev.on_click(move |event, window, cx| handler(event, window, cx));
        }
        let chev = chev.child(
            svg()
                .path(if self.expanded { UP } else { DOWN })
                .w(px(12.0))
                .h(px(12.0))
                .text_color(theme::rgb(theme::colors::MUTED)),
        );
        let mut outer = div().flex().flex_col().w_full().border_b_1().border_color(
            theme::rgb(theme::colors::GRID_L1),
        );
        if self.tinted {
            outer = outer.bg(theme::rgb(0xeff6ff));
        } else {
            outer = outer.bg(theme::rgb(theme::colors::SURFACE_LOWEST));
        }
        outer = outer.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .py_2()
                .w_full()
                .child(
                    svg()
                        .path(self.icon)
                        .w(px(14.0))
                        .h(px(14.0))
                        .text_color(theme::rgb(self.icon_fg)),
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
                                .child(self.title),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(theme::rgb(theme::colors::MUTED))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(self.policy),
                        ),
                )
                .child(if let Some(badge) = self.badge {
                    div().flex_shrink_0().child(badge)
                } else {
                    div()
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .flex_shrink_0()
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(theme::rgb(theme::colors::MUTED))
                                .child(self.size),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(theme::rgb(theme::colors::MUTED))
                                .child(self.keys),
                        ),
                )
                .child(chev),
        );
        if self.expanded {
            outer = outer.child(
                div()
                    .w_full()
                    .px_2()
                    .pb_2()
                    .pl(px(36.0))
                    .text_size(px(11.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child(self.detail),
            );
        }
        outer
    }
}
