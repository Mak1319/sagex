//! License composer deck — manual `gpui-base` expansion only.
//!
//! Spec rebuild: fixed header (working collapse chevron), scrollable
//! attachment accordion container, access accordion bar (`ACCESS (n/5)` +
//! avatar stack + chevron), filter bar with editable input, scrollable
//! recipient container with `DeckCheckbox` rows, fixed footer. All
//! interactive state (expanded row, collapsed section, access section,
//! checked set, filter query) is owned by the parent and passed in as
//! props — this stays a pure `RenderOnce` view.
//!
//! Layering: `gpui-base` primitives (`div`, `BaseButton`) for layout plus
//! our own filter field built on `gpui-base` input engine
//! (`gpui_kit::base::input::{Input, InputState}`). No `gpui-component`
//! usage anywhere; icons resolve from `assets/icons/` (Iconoir first,
//! Feather second, Material last).

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::input::{Input, InputState};
use gpui_kit::*;

use crate::components::attachment_item::AttachmentItem;
use crate::components::avatar::Avatar;
use crate::components::deck_badge::DeckBadge;
use crate::components::recipient_item::RecipientItem;
use crate::theme;

type DeckHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type IndexHandler = Rc<dyn Fn(usize, &ClickEvent, &mut Window, &mut App)>;

const FOLDER: &str = "icons/folder.svg";
const LOCK: &str = "icons/lock-closed.svg";
const PHOTO: &str = "icons/photo.svg";
const CODE: &str = "icons/code-bracket.svg";
const SEARCH: &str = "icons/magnifying-glass.svg";
const UP: &str = "icons/chevron-up.svg";
const DOWN: &str = "icons/chevron-down.svg";
const CLOSE: &str = "icons/x-mark.svg";

pub const RECIPIENT_COUNT: usize = 5;

#[derive(IntoElement)]
pub struct LicenseComposer {
    title: SharedString,
    on_close: Option<DeckHandler>,
    on_confirm: Option<DeckHandler>,
    expanded: Option<usize>,
    on_attach_toggle: Option<IndexHandler>,
    collapsed: bool,
    on_collapsed_toggle: Option<DeckHandler>,
    access_collapsed: bool,
    on_access_toggle: Option<DeckHandler>,
    checked: [bool; RECIPIENT_COUNT],
    on_recipient_toggle: Option<IndexHandler>,
    filter: Option<Entity<InputState>>,
}

impl LicenseComposer {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            on_close: None,
            on_confirm: None,
            expanded: None,
            on_attach_toggle: None,
            collapsed: false,
            on_collapsed_toggle: None,
            access_collapsed: false,
            on_access_toggle: None,
            checked: [true, true, true, false, false],
            on_recipient_toggle: None,
            filter: None,
        }
    }

    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    pub fn on_confirm(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }

    pub fn expanded(mut self, expanded: Option<usize>) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn on_attach_toggle(
        mut self,
        handler: impl Fn(usize, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_attach_toggle = Some(Rc::new(handler));
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn on_collapsed_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_collapsed_toggle = Some(Rc::new(handler));
        self
    }

    pub fn access_collapsed(mut self, collapsed: bool) -> Self {
        self.access_collapsed = collapsed;
        self
    }

    pub fn on_access_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_access_toggle = Some(Rc::new(handler));
        self
    }

    pub fn filter(mut self, filter: Entity<InputState>) -> Self {
        self.filter = Some(filter);
        self
    }

    pub fn checked(mut self, checked: [bool; RECIPIENT_COUNT]) -> Self {
        self.checked = checked;
        self
    }

    pub fn on_recipient_toggle(
        mut self,
        handler: impl Fn(usize, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_recipient_toggle = Some(Rc::new(handler));
        self
    }
}

fn attachment(
    ix: usize,
    expanded: Option<usize>,
    handler: &Option<IndexHandler>,
    item: AttachmentItem,
) -> AttachmentItem {
    let item = item.expanded(expanded == Some(ix));
    if let Some(on_toggle) = handler {
        let on_toggle = on_toggle.clone();
        item.on_toggle(move |event, window, cx| on_toggle(ix, event, window, cx))
    } else {
        item
    }
}

fn recipient(
    ix: usize,
    checked: bool,
    handler: &Option<IndexHandler>,
    item: RecipientItem,
) -> RecipientItem {
    let item = item.checked(checked).unlocked(checked);
    if let Some(on_toggle) = handler {
        let on_toggle = on_toggle.clone();
        item.on_toggle(move |event, window, cx| on_toggle(ix, event, window, cx))
    } else {
        item
    }
}

impl RenderOnce for LicenseComposer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let unlocked = self.checked.iter().filter(|v| **v).count();
        let access_label: SharedString =
            format!("ACCESS ({}/{})", unlocked, RECIPIENT_COUNT).into();
        let query: String = self
            .filter
            .as_ref()
            .map(|f| f.read(cx).value().to_string())
            .unwrap_or_default();
        let needle = query.to_lowercase();

        let mut collapse = BaseButton::new(ElementId::Name("license-collapse".into()))
            .w(px(24.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::radius::DEFAULT))
            .text_color(theme::rgb(theme::colors::MUTED));
        if let Some(handler) = self.on_collapsed_toggle {
            collapse = collapse.on_click(move |event, window, cx| handler(event, window, cx));
        }
        let collapse_icon = if self.collapsed { DOWN } else { UP };

        let mut access_toggle = BaseButton::new(ElementId::Name("license-access-toggle".into()))
            .w(px(24.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::radius::DEFAULT))
            .text_color(theme::rgb(theme::colors::MUTED));
        if let Some(handler) = self.on_access_toggle {
            access_toggle =
                access_toggle.on_click(move |event, window, cx| handler(event, window, cx));
        }
        let access_icon = if self.access_collapsed { DOWN } else { UP };

        let mut close = BaseButton::new(ElementId::Name("license-close".into()))
            .w(px(24.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::radius::DEFAULT))
            .text_color(theme::rgb(theme::colors::MUTED));
        if let Some(handler) = self.on_close {
            close = close.on_click(move |event, window, cx| handler(event, window, cx));
        }
        let mut confirm = BaseButton::new(ElementId::Name("license-confirm".into()))
            .px_3()
            .py_1()
            .rounded(px(theme::radius::DEFAULT))
            .bg(theme::rgb(theme::colors::PRIMARY_CONTAINER))
            .text_color(theme::rgb(theme::colors::ON_PRIMARY))
            .text_size(px(12.0))
            .child("Share Protected");
        if let Some(handler) = self.on_confirm {
            confirm = confirm.on_click(move |event, window, cx| handler(event, window, cx));
        }

        let mut deck = div()
            .w_full()
            .max_w(px(591.0))
            .max_h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .rounded(px(1.5))
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .overflow_hidden()
            // Header (spec 0-26, #F8FAFC)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .h(px(32.0))
                    .flex_shrink_0()
                    .bg(theme::rgb(0xf8fafc))
                    .border_b_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .child(
                        svg()
                            .path(FOLDER)
                            .w(px(14.0))
                            .h(px(14.0))
                            .text_color(theme::rgb(theme::colors::PRIMARY)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_size(px(12.0))
                            .text_color(theme::rgb(theme::colors::PRIMARY))
                            .child(self.title),
                    )
                    .child(DeckBadge::total("4.5MB TOTAL"))
                    .child(DeckBadge::keys("5 KEYS"))
                    .child(collapse.child(
                        svg()
                            .path(collapse_icon)
                            .w(px(14.0))
                            .h(px(14.0))
                            .text_color(theme::rgb(theme::colors::MUTED)),
                    ))
                    .child(close.child(
                        svg()
                            .path(CLOSE)
                            .w(px(14.0))
                            .h(px(14.0))
                            .text_color(theme::rgb(theme::colors::MUTED)),
                    )),
            );

        // Container 1 — scrollable attachment accordion (collapsible)
        if !self.collapsed {
            deck = deck.child(
                div()
                    .id(ElementId::Name("license-attachments".into()))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .scrollbar_width(px(0.0))
                    .child(attachment(
                        0,
                        self.expanded,
                        &self.on_attach_toggle,
                        AttachmentItem::new("attach-0", "Scheme clearance policy")
                            .policy("scheme · clearance policy")
                            .icon(LOCK, theme::colors::PRIMARY)
                            .badge(DeckBadge::restricted("RESTRICTED"))
                            .size("1.2MB")
                            .keys("5 keys")
                            .detail("cipher AES-256-GCM · 5/5 keys · policy restricted")
                            .tinted(true),
                    ))
                    .child(attachment(
                        1,
                        self.expanded,
                        &self.on_attach_toggle,
                        AttachmentItem::new("attach-1", "board-mockup.png")
                            .policy("png · image attachment")
                            .icon(PHOTO, 0x2563eb)
                            .badge(DeckBadge::crypto("AES-256-GCM"))
                            .size("1.8MB")
                            .keys("inherit")
                            .detail("cipher AES-256-GCM · 1.8MB · exif stripped")
                            .tinted(false),
                    ))
                    .child(attachment(
                        2,
                        self.expanded,
                        &self.on_attach_toggle,
                        AttachmentItem::new("attach-2", "relay-diff.patch")
                            .policy("patch · code attachment")
                            .icon(CODE, theme::colors::PRIMARY)
                            .badge(DeckBadge::neutral("RW-213"))
                            .size("88KB")
                            .keys("inherit")
                            .detail("signed by relay · signature verified")
                            .tinted(false),
                    ))
                    .child(attachment(
                        3,
                        self.expanded,
                        &self.on_attach_toggle,
                        AttachmentItem::new("attach-3", "manifest.json")
                            .policy("json · signed manifest")
                            .icon(CODE, theme::colors::SUCCESS)
                            .badge(DeckBadge::valid("VALID"))
                            .size("12KB")
                            .keys("inherit")
                            .detail("signature valid · manifest pinned")
                            .tinted(false),
                    )),
            );
        }

        // Access bar — accordion header: ACCESS (n/5) + stack + chevron.
        // Toggles the recipient section (filter bar + container 2).
        deck = deck.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .h(px(29.0))
                .flex_shrink_0()
                .bg(theme::rgb(0xf8fafc))
                .border_y_1()
                .border_color(theme::rgb(theme::colors::GRID_L1))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::rgb(theme::colors::INK))
                        .child(access_label),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(Avatar::new("A").bg(0xd1fae5).fg(0x047857).size(19.0).presence(0x10b981))
                        .child(div().w(px(19.0)).h(px(19.0)).ml(px(-6.0)).child(
                            Avatar::new("P").bg(0xfee2e2).fg(0xb91c1c).size(19.0).presence(0x10b981),
                        ))
                        .child(div().w(px(19.0)).h(px(19.0)).ml(px(-6.0)).child(
                            Avatar::new("Y").bg(0xdbeafe).fg(0x004ac6).size(19.0).presence(theme::colors::PRIMARY),
                        ))
                        .child(div().w(px(19.0)).h(px(19.0)).ml(px(-6.0)).child(
                            Avatar::new("M").bg(0xf1f5f9).fg(theme::colors::MUTED).size(19.0).presence(0x64748b),
                        ))
                        .child(div().w(px(19.0)).h(px(19.0)).ml(px(-6.0)).child(
                            Avatar::new("K").bg(0xf1f5f9).fg(theme::colors::MUTED).size(19.0).presence(0x64748b),
                        )),
                )
                .child(div().flex_1())
                .child(access_toggle.child(
                    svg()
                        .path(access_icon)
                        .w(px(14.0))
                        .h(px(14.0))
                        .text_color(theme::rgb(theme::colors::MUTED)),
                )),
        );

        // Filter bar (fixed, heads container 2) — own editable field on the
        // `gpui-base` input engine. No `gpui-component` involved.
        if !self.access_collapsed {
            let filter_field: AnyElement = match self.filter {
                Some(state) => div()
                    .flex_1()
                    .text_size(px(11.0))
                    .text_color(theme::rgb(theme::colors::INK))
                    .child(Input::new(&state))
                    .into_any_element(),
                None => div()
                    .flex_1()
                    .text_size(px(11.0))
                    .text_color(theme::rgb(theme::colors::MUTED))
                    .child("Filter recipients…")
                    .into_any_element(),
            };
            deck = deck.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .h(px(29.0))
                    .flex_shrink_0()
                    .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                    .border_b_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .on_mouse_down_out(|_, window, cx| window.blur(cx))
                    .child(
                        svg()
                            .path(SEARCH)
                            .w(px(14.0))
                            .h(px(14.0))
                            .text_color(theme::rgb(theme::colors::MUTED)),
                    )
                    .child(filter_field),
            );
        }

        // Container 2 — scrollable recipients (under the access accordion).
        // Live-filtered by the filter bar query; indices stay stable so
        // toggles still hit the right recipient.
        let names = [
            "Ava Stone",
            "Priya Nair",
            "Yuki Tanaka",
            "Mira Shah",
            "Kai Rao",
        ];
        let handles = [
            "ava · 2 keys",
            "priya · 2 keys",
            "yuki · 1 key",
            "mira · pending",
            "kai · pending",
        ];
        let hits = |i: usize| {
            needle.is_empty()
                || names[i].to_lowercase().contains(&needle)
                || handles[i].to_lowercase().contains(&needle)
        };
        if !self.access_collapsed {
            let mut rows: Vec<AnyElement> = Vec::new();
            if hits(0) {
                rows.push(
                    recipient(
                        0,
                        self.checked[0],
                        &self.on_recipient_toggle,
                        RecipientItem::new("recip-0", "Ava Stone")
                            .handle("ava · 2 keys")
                            .avatar("A", 0xd1fae5, 0x047857)
                            .presence(0x10b981),
                    )
                    .into_any_element(),
                );
            }
            if hits(1) {
                rows.push(
                    recipient(
                        1,
                        self.checked[1],
                        &self.on_recipient_toggle,
                        RecipientItem::new("recip-1", "Priya Nair")
                            .handle("priya · 2 keys")
                            .avatar("P", 0xfee2e2, 0xb91c1c)
                            .presence(0x10b981),
                    )
                    .into_any_element(),
                );
            }
            if hits(2) {
                rows.push(
                    recipient(
                        2,
                        self.checked[2],
                        &self.on_recipient_toggle,
                        RecipientItem::new("recip-2", "Yuki Tanaka")
                            .handle("yuki · 1 key")
                            .avatar("Y", 0xdbeafe, 0x004ac6)
                            .presence(theme::colors::PRIMARY),
                    )
                    .into_any_element(),
                );
            }
            if hits(3) {
                rows.push(
                    recipient(
                        3,
                        self.checked[3],
                        &self.on_recipient_toggle,
                        RecipientItem::new("recip-3", "Mira Shah")
                            .handle("mira · pending")
                            .avatar("M", 0xf1f5f9, theme::colors::MUTED)
                            .presence(0x64748b),
                    )
                    .into_any_element(),
                );
            }
            if hits(4) {
                rows.push(
                    recipient(
                        4,
                        self.checked[4],
                        &self.on_recipient_toggle,
                        RecipientItem::new("recip-4", "Kai Rao")
                            .handle("kai · pending")
                            .avatar("K", 0xf1f5f9, theme::colors::MUTED)
                            .presence(0x64748b),
                    )
                    .into_any_element(),
                );
            }
            deck = deck.child(
                div()
                    .id(ElementId::Name("license-recipients".into()))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .scrollbar_width(px(0.0))
                    .children(rows),
            );
        }
        deck = deck
            // Footer (fixed)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .child(confirm),
            );
        deck
    }
}
