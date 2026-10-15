//! Mono link — manual `gpui-base` expansion only.
//!
//! Small uppercase primary text button for auth-page links
//! (forgot key, register, sign in).

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::theme;

type LinkHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct MonoLink {
    label: SharedString,
    on_click: Option<LinkHandler>,
}

impl MonoLink {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
        }
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MonoLink {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut button = BaseButton::new(ElementId::Name("mono-link".into()))
            .text_size(px(10.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme::rgb(theme::colors::PRIMARY));
        if let Some(handler) = self.on_click {
            button = button.on_click(move |event, window, cx| handler(event, window, cx));
        }
        button.child(self.label)
    }
}
