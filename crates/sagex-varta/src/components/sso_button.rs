//! SSO button — manual `gpui-base` expansion only.
//!
//! Washed secondary button with a leading glyph for alternate auth
//! (Passkey / FIDO2, SSO Gateway), shared by the auth pages.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::theme;

type SsoHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
//(!) Button with Icon Label
pub struct SsoButton {
    id: SharedString,
    icon: SharedString,
    icon_fg: u32,
    label: SharedString,
    on_click: Option<SsoHandler>,
}

impl SsoButton {
    pub fn new(
        id: impl Into<SharedString>,
        icon: impl Into<SharedString>,
        icon_fg: u32,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            icon_fg,
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

impl RenderOnce for SsoButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut button = BaseButton::new(ElementId::Name(self.id))
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap_2()
            .py_2()
            .px_3()
            .rounded(px(theme::radius::DEFAULT))
            .bg(theme::rgb(theme::colors::SURFACE_LOW))
            .border_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .text_size(px(11.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme::rgb(theme::colors::INK));
        if let Some(handler) = self.on_click {
            button = button.on_click(move |event, window, cx| handler(event, window, cx));
        }
        button
            .child(
                svg()
                    .path(self.icon)
                    .w(px(15.0))
                    .h(px(15.0))
                    .text_color(theme::rgb(self.icon_fg)),
            )
            .child(self.label)
    }
}
