//! Settings button — manual `gpui-base` expansion only.
//!
//! Glyph is iconoir `settings.svg` (`assets/icons/settings-iconoir.svg`,
//! 24px, `stroke="currentColor"`), tinted `#475569` via `text_color` to
//! match `assets/icons/settings-button.svg` (45x52 gear + SETTINGS label).
//! Heroicons `cog-6-tooth.svg` is stored alongside as the 24/outline
//! alternative.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::*;

use crate::theme;

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

const GEAR: &str = "icons/settings-iconoir.svg";

#[derive(IntoElement)]
pub struct SettingsButton {
    on_click: Option<ClickHandler>,
}

impl SettingsButton {
    pub fn new() -> Self {
        Self { on_click: None }
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Default for SettingsButton {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for SettingsButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let gear = theme::rgb(0x475569);
        let mut button = BaseButton::new(ElementId::Name("rail-settings".into()))
            .w(px(45.0))
            .h(px(52.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .rounded(px(theme::radius::DEFAULT))
            .text_color(gear);
        if let Some(handler) = self.on_click {
            button = button.on_click(move |event, window, cx| handler(event, window, cx));
        }
        button
            .child(svg().path(GEAR).w(px(24.0)).h(px(24.0)).text_color(gear))
            .child(div().text_size(px(9.0)).text_color(gear))
    }
}
