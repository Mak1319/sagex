//! Deck pill badge — manual `gpui-base` expansion only.
//!
//! Optional leading glyph beside the label (lock, shield, check).

use gpui_kit::*;

use crate::theme;

#[derive(IntoElement)]
pub struct DeckBadge {
    label: SharedString,
    bg: u32,
    border: u32,
    fg: u32,
    icon: Option<SharedString>,
}

impl DeckBadge {
    pub fn new(label: impl Into<SharedString>, bg: u32, border: u32, fg: u32) -> Self {
        Self {
            label: label.into(),
            bg,
            border,
            fg,
            icon: None,
        }
    }

    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn total(label: impl Into<SharedString>) -> Self {
        Self::new(label, 0xd3e4fe, 0xbfdbfe, theme::colors::PRIMARY)
    }

    pub fn keys(label: impl Into<SharedString>) -> Self {
        Self::new(label, 0xecfdf5, 0xa7f3d0, 0x047857)
    }

    pub fn restricted(label: impl Into<SharedString>) -> Self {
        Self::new(label, 0xfffbeb, 0xfde68a, 0xb45309)
            .icon("icons/lock-closed.svg")
    }

    pub fn crypto(label: impl Into<SharedString>) -> Self {
        Self::new(label, 0xeff6ff, 0xbfdbfe, theme::colors::PRIMARY_DARK)
            .icon("icons/shield-check.svg")
    }

    pub fn valid(label: impl Into<SharedString>) -> Self {
        Self::keys(label).icon("icons/check.svg")
    }

    pub fn unlock(label: impl Into<SharedString>) -> Self {
        Self::keys(label).icon("icons/lock-open.svg")
    }

    pub fn lock(label: impl Into<SharedString>) -> Self {
        Self::neutral(label).icon("icons/lock-closed.svg")
    }

    pub fn neutral(label: impl Into<SharedString>) -> Self {
        Self::new(label, 0xf1f5f9, theme::colors::GRID_L1, 0x475569)
    }
}

impl RenderOnce for DeckBadge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let fg = theme::rgb(self.fg);
        let mut pill = div()
            .flex_shrink_0()
            .h(px(15.0))
            .px_2()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .rounded(px(1.5))
            .bg(theme::rgb(self.bg))
            .border_1()
            .border_color(theme::rgb(self.border))
            .text_size(px(10.0))
            .text_color(fg);
        if let Some(icon) = self.icon {
            pill = pill.child(
                svg()
                    .path(icon)
                    .w(px(10.0))
                    .h(px(10.0))
                    .text_color(theme::rgb(self.fg)),
            );
        }
        pill.child(self.label)
    }
}
