//! Chat composer — manual `gpui-base` expansion only.
//!
//! Bordered text box on top; toolbar row below it (attach, photo,
//! divider, bold, italic, lists, link, mention); right side carries the
//! shortcut hint and compact SEND. Framework `Textarea` owns editing.

use std::rc::Rc;

use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::*;

use crate::components::tool_button::ToolButton;
use crate::theme;

type ComposerHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

const CLIP: &str = "icons/paper-clip.svg";
const PHOTO: &str = "icons/photo.svg";
const BOLD: &str = "icons/bold.svg";
const ITALIC: &str = "icons/italic.svg";
const BULLETS: &str = "icons/list-bullet.svg";
const NUMBERED: &str = "icons/numbered-list.svg";
const LINK: &str = "icons/link.svg";
const MENTION: &str = "icons/at-symbol.svg";
const SEND: &str = "icons/paper-airplane.svg";

#[derive(IntoElement)]
//(!) ChatComposer
pub struct ChatComposer {
    state: Entity<TextareaState>,
    on_attach: Option<ComposerHandler>,
    on_send: Option<ComposerHandler>,
}

impl ChatComposer {
    pub fn new(state: Entity<TextareaState>) -> Self {
        Self {
            state,
            on_attach: None,
            on_send: None,
        }
    }

    pub fn on_attach(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_attach = Some(Rc::new(handler));
        self
    }

    pub fn on_send(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_send = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ChatComposer {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut attach = ToolButton::new("composer-attach", CLIP);
        if let Some(handler) = self.on_attach {
            attach = attach.on_click(move |event, window, cx| handler(event, window, cx));
        }
        let mut send = BaseButton::new(ElementId::Name("composer-send".into()))
            .px_3()
            .h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .rounded(px(theme::radius::DEFAULT))
            .bg(theme::rgb(theme::colors::PRIMARY_CONTAINER))
            .text_color(theme::rgb(theme::colors::ON_PRIMARY))
            .text_size(px(12.0));
        if let Some(handler) = self.on_send {
            send = send.on_click(move |event, window, cx| handler(event, window, cx));
        }
        div()
            .flex()
            .flex_col()
            .w_full()
            .p_2()
            .bg(theme::rgb(theme::colors::SURFACE_LOWEST))
            .border_t_1()
            .border_color(theme::rgb(theme::colors::GRID_L1))
            .child(
                div()
                    .w_full()
                    .rounded(px(1.5))
                    .bg(theme::rgb(theme::colors::INSET))
                    .border_1()
                    .border_color(theme::rgb(theme::colors::GRID_L1))
                    .child(
                        div().px_2().py_2().child(
                            Textarea::new(&self.state)
                                .flex_1()
                                .appearance(false)
                                .bordered(false)
                                .text_size(px(13.0)),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .pt_2()
                    .child(attach)
                    .child(ToolButton::new("composer-photo", PHOTO))
                    .child(div().w(px(1.0)).h(px(16.0)).bg(theme::rgb(theme::colors::GRID_L1)))
                    .child(ToolButton::new("composer-bold", BOLD))
                    .child(ToolButton::new("composer-italic", ITALIC))
                    .child(ToolButton::new("composer-bullets", BULLETS))
                    .child(ToolButton::new("composer-numbered", NUMBERED))
                    .child(ToolButton::new("composer-link", LINK))
                    .child(ToolButton::new("composer-mention", MENTION))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::rgb(theme::colors::MUTED))
                            .child("⌘⏎ to send"),
                    )
                    .child(
                        send.child("SEND").child(
                            svg()
                                .path(SEND)
                                .w(px(14.0))
                                .h(px(14.0))
                                .text_color(theme::rgb(theme::colors::ON_PRIMARY)),
                        ),
                    ),
            )
    }
}
