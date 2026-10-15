//! Icon rail — manual `gpui-base` expansion only (Styled + theme).
//!
//! Composes [`ThreeStateSlider`] (top) and [`SettingsButton`] (bottom).
//! State lives in the parent view; the rail only forwards callbacks.

use std::rc::Rc;

use gpui_kit::*;

use crate::components::settings_button::SettingsButton;
use crate::components::theme_slider::ThreeStateSlider;
use crate::theme;

type ThemeHandler = Rc<dyn Fn(u8, &mut Window, &mut App)>;
type SettingsHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct IconRail {
    theme: u8,
    on_theme: Option<ThemeHandler>,
    on_settings: Option<SettingsHandler>,
}

impl IconRail {
    pub fn new() -> Self {
        Self {
            theme: 1,
            on_theme: None,
            on_settings: None,
        }
    }

    pub fn theme(mut self, index: u8) -> Self {
        self.theme = index.min(2);
        self
    }

    pub fn on_theme(mut self, handler: impl Fn(u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_theme = Some(Rc::new(handler));
        self
    }

    pub fn on_settings(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_settings = Some(Rc::new(handler));
        self
    }

    pub fn reader_theme(&self) -> u8 {
        self.theme
    }
}

impl Default for IconRail {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for IconRail {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut slider = ThreeStateSlider::new().active(self.theme);
        if let Some(handler) = self.on_theme {
            slider = slider.on_change(move |index, window, cx| handler(index, window, cx));
        }
        let mut settings = SettingsButton::new();
        if let Some(handler) = self.on_settings {
            settings = settings.on_click(move |event, window, cx| handler(event, window, cx));
        }
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .py_2()
            .px_0()
            .w(px(56.0))
            .h_full()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_r_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .child(div().flex_1())
            .child(slider)
            .child(settings)
    }
}
