//! Reply preview bar above the composer: accent edge, colored sender,
//! truncated quote, dismiss X.

use gpui::{
    App, ClickEvent, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window, div,
    px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt as _,
    button::{Button, ButtonVariants},
};
use std::rc::Rc;

/// Controlled bar. Parent owns the quoted snapshot + dismissal.
#[derive(IntoElement)]
pub struct ReplyBar {
    style: StyleRefinement,
    sender: String,
    sender_color: u32,
    text: String,
    on_close: Option<super::ClickHandler>,
}

impl ReplyBar {
    pub fn new(sender: impl Into<String>, sender_color: u32, text: impl Into<String>) -> Self {
        Self {
            style: StyleRefinement::default(),
            sender: sender.into(),
            sender_color,
            text: text.into(),
            on_close: None,
        }
    }

    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl Styled for ReplyBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ReplyBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .rounded_md()
            .bg(theme.muted)
            .border_l_2()
            .border_color(gpui::rgb(super::colors::ACCENT_GREEN))
            .pl_2()
            .pr(px(2.))
            .py_1()
            .mx_4()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_sm()
                            .font_bold()
                            .text_color(gpui::rgb(self.sender_color))
                            .child(self.sender),
                    )
                    .child(
                        div()
                            .text_sm()
                            .truncate()
                            .text_color(theme.muted_foreground)
                            .child(self.text),
                    ),
            )
            .child({
                let mut btn = Button::new("reply-bar-close")
                    .ghost()
                    .small()
                    .icon(Icon::empty().path("icons/x.svg"));
                if let Some(handler) = self.on_close {
                    btn = btn.on_click(move |evt, window, cx: &mut App| (handler)(evt, window, cx));
                }
                btn
            })
    }
}
