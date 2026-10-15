//! Three-state theme slider — manual `gpui-base` expansion only.
//!
//! Track matches `assets/icons/theme-three-state.svg` (45x106, `#F1F5F9`
//! on `#E2E8F0`). Glyphs come from heroicons `24/outline` (`moon.svg`,
//! `sun.svg`, `chat-bubble-left.svg`) with `stroke="currentColor"` so the
//! active tint (`#004AC6`) vs idle (`#64748B`) is applied via `text_color`.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::theme;

type ChangeHandler = Rc<dyn Fn(u8, &mut Window, &mut App)>;

const MOON: &str = "icons/moon.svg";
const SUN: &str = "icons/sun.svg";
const CHAT: &str = "icons/chat-bubble-left.svg";

/// Index: 0 = dark (moon), 1 = light (sun), 2 = chat.
#[derive(IntoElement)]
pub struct ThreeStateSlider {
    active: u8,
    on_change: Option<ChangeHandler>,
}

impl ThreeStateSlider {
    pub fn new() -> Self {
        Self {
            active: 1,
            on_change: None,
        }
    }

    pub fn active(mut self, index: u8) -> Self {
        self.active = index.min(2);
        self
    }

    pub fn on_change(mut self, handler: impl Fn(u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    pub fn reader_active(&self) -> u8 {
        self.active
    }
}

impl Default for ThreeStateSlider {
    fn default() -> Self {
        Self::new()
    }
}

fn slider_cell(index: u8, icon_path: &'static str, is_active: bool, on_change: Option<ChangeHandler>) -> impl IntoElement {
    let fg = if is_active {
        theme::rgb(0x004ac6)
    } else {
        theme::rgb(theme::colors::MUTED)
    };
    let mut cell = BaseButton::new(ElementId::Name(
        SharedString::from(format!("theme-slider-{index}")),
    ))
    .w(px(28.0))
    .h(px(28.0))
    .flex()
    .items_center()
    .justify_center()
    .rounded(px(theme::radius::DEFAULT))
    .text_color(fg);
    if is_active {
        cell = cell
            .bg(theme::rgb(0xffffff))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L1));
    }
    if let Some(handler) = on_change {
        cell = cell.on_click(move |_, window, cx| handler(index, window, cx));
    }
    cell.child(
        svg()
            .path(icon_path)
            .w(px(16.0))
            .h(px(16.0))
            .text_color(fg),
    )
}

impl RenderOnce for ThreeStateSlider {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let active = self.active;
        div()
            .id(ElementId::Name("theme-slider".into()))
            .w(px(45.0))
            .h(px(106.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .w(px(38.0))
                    .h(px(102.0))
                    .bg(theme::rgb(0xf1f5f9))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .rounded(px(theme::radius::DEFAULT))
                    .child(slider_cell(0, MOON, active == 0, self.on_change.clone()))
                    .child(slider_cell(1, SUN, active == 1, self.on_change.clone()))
                    .child(slider_cell(2, CHAT, active == 2, self.on_change)),
            )
    }
}
