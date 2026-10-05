use std::rc::Rc;

use gpui::{App, ClickEvent, IntoElement, RenderOnce, Window, div, prelude::*, px};

use super::icon::icon_el;
use crate::components::IconKind;

/// 28×28 square action target with optional click handler.
/// Active state gets the primary glow-wash tile.
#[derive(IntoElement)]
pub struct IconButton {
    pub icon: IconKind,
    pub active: bool,
    pub danger: bool,
    pub on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl IconButton {
    pub fn new(icon: IconKind) -> Self {
        Self {
            icon,
            active: false,
            danger: false,
            on_click: None,
        }
    }

    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }

    /// Red destructive tint (delete affordances).
    #[allow(dead_code)]
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let id = match self.icon {
            IconKind::Video => "btn-video",
            IconKind::Call => "btn-call",
            IconKind::Add => "btn-add",
            IconKind::Search => "btn-search",
            IconKind::More => "btn-more",
            IconKind::Chats => "btn-chats",
            IconKind::Settings => "btn-settings",
            IconKind::Moon => "btn-moon",
            IconKind::Sun => "btn-sun",
            IconKind::Monitor => "btn-monitor",
            _ => "btn-icon",
        };
        let mut el = div()
            .id(id)
            .w(px(28.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .rounded(px(4.0))
            .border_1()
            .border_color(if self.active {
                gpui::rgba(0x2563EB66)
            } else {
                gpui::rgba(0x00000000)
            })
            .bg(if self.active {
                gpui::rgba(0x2563EB1F)
            } else {
                gpui::rgba(0x00000000)
            })
            .child(icon_el(self.icon, 18.0, self.active, self.danger));
        if let Some(f) = self.on_click {
            el = el.on_click(move |ev, window, cx| f(ev, window, cx));
        }
        el
    }
}
