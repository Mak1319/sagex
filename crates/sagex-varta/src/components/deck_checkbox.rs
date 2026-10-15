//! Deck checkbox — manual `gpui-base` expansion only.
//!
//! Standalone toggle so recipient rows (and future filters) share one
//! behavior: blue fill + white check when on, white + grey border when off.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::theme;

type CheckHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

const CHECK: &str = "icons/check.svg";

#[derive(IntoElement)]
pub struct DeckCheckbox {
    id: SharedString,
    checked: bool,
    on_toggle: Option<CheckHandler>,
}

impl DeckCheckbox {
    pub fn new(id: impl Into<SharedString>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            on_toggle: None,
        }
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DeckCheckbox {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut button = BaseButton::new(ElementId::Name(self.id))
            .w(px(18.0))
            .h(px(18.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::radius::DEFAULT));
        if let Some(handler) = self.on_toggle {
            button = button.on_click(move |event, window, cx| handler(event, window, cx));
        }
        let mut box_el = div()
            .w(px(14.0))
            .h(px(14.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(2.0))
            .border_1();
        if self.checked {
            box_el = box_el
                .bg(theme::rgb(theme::colors::PRIMARY))
                .border_color(theme::rgb(theme::colors::PRIMARY))
                .child(
                    svg()
                        .path(CHECK)
                        .w(px(10.0))
                        .h(px(10.0))
                        .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                );
        } else {
            box_el = box_el
                .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
                .border_color(theme::rgb(0x6b7280));
        }
        button.child(box_el)
    }
}
