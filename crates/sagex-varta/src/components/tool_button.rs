//! Toolbar icon button — manual `gpui-base` expansion only.
//!
//! 28px muted glyph button for the composer toolbar. Separate type from
//! `BarButton` so the two surfaces can diverge later.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::theme;

type ToolHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ToolButton {
    id: SharedString,
    icon: SharedString,
    on_click: Option<ToolHandler>,
}

impl ToolButton {
    pub fn new(id: impl Into<SharedString>, icon: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
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

impl RenderOnce for ToolButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut button = BaseButton::new(ElementId::Name(self.id))
            .w(px(28.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::radius::DEFAULT))
            .text_color(theme::rgb(theme::colors::MUTED));
        if let Some(handler) = self.on_click {
            button = button.on_click(move |event, window, cx| handler(event, window, cx));
        }
        button.child(
            svg()
                .path(self.icon)
                .w(px(16.0))
                .h(px(16.0))
                .text_color(theme::rgb(theme::colors::MUTED)),
        )
    }
}
