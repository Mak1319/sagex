//! Dismissible action-feedback pill.

use gpui::{
    App, ClickEvent, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div,
};
use gpui_component::ActiveTheme;
use std::rc::Rc;

/// Centered muted pill. Controlled: parent owns the text + dismissal.
#[derive(IntoElement)]
pub struct NoticePill {
    style: StyleRefinement,
    text: String,
    on_dismiss: Option<super::ClickHandler>,
}

impl NoticePill {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            style: StyleRefinement::default(),
            text: text.into(),
            on_dismiss: None,
        }
    }

    pub fn on_dismiss(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl Styled for NoticePill {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for NoticePill {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut pill = div()
            .id("notice-pill")
            .rounded_full()
            .bg(theme.muted)
            .px_3()
            .py_1()
            .text_xs()
            .text_color(theme.muted_foreground)
            .cursor_pointer()
            .child(self.text);
        if let Some(handler) = self.on_dismiss {
            pill = pill.on_click(move |evt, window, cx: &mut App| (handler)(evt, window, cx));
        }
        div().flex().w_full().justify_center().pb_1().child(pill)
    }
}
