//! Side panel search bar — manual `gpui-base` expansion only.
//!
//! Working text input: framework `component::input::Input` owns focus,
//! editing and keyboard behavior; all presentation (field wash, border,
//! magnifier prefix, shortcut chip) is expanded here from `crate::theme`.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::*;

use crate::theme;

const SEARCH: &str = "icons/magnifying-glass.svg";

#[derive(IntoElement)]
//(!) Input Fields
pub struct SideSearchBar {
    search: Entity<InputState>,
    shortcut: SharedString,
}

impl SideSearchBar {
    pub fn new(search: Entity<InputState>) -> Self {
        Self {
            search,
            shortcut: SharedString::from("⌘K"),
        }
    }

    pub fn shortcut(mut self, text: impl Into<SharedString>) -> Self {
        self.shortcut = text.into();
        self
    }
}

impl RenderOnce for SideSearchBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let muted = theme::rgb(theme::colors::MUTED);
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(53.0))
            .w_full()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_b_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .child(
                div()
                    .id(ElementId::Name("side-search-field".into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_1()
                    .h(px(27.0))
                    .px_2()
                    .rounded(px(1.5))
                    .bg(theme::rgb(theme::colors::INSET))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .on_mouse_down_out(|_, window, cx| window.blur(cx))
                    .child(
                        Input::new(&self.search)
                            .prefix(
                                svg().path(SEARCH).w(px(14.0)).h(px(14.0)).text_color(muted),
                            )
                            .suffix(
                                div()
                                    .px_1()
                                    .rounded(px(1.5))
                                    .border_1()
                                    .border_color(theme::rgb(theme::colors::GRID_L1))
                                    .bg(theme::rgb(0xffffff))
                                    .text_size(px(10.0))
                                    .text_color(muted)
                                    .child(self.shortcut),
                            )
                            .flex_1()
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .text_size(px(12.0)),
                    ),
            )
    }
}
