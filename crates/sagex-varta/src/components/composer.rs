use std::rc::Rc;

use gpui::{App, ClickEvent, IntoElement, Render, Window, div, prelude::*};

use crate::{
    components::{IconKind, icon_el},
    theme::palette as p,
};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Bottom message composer: mode tab + draft + toolbar + SEND.
/// Real GPUI view (`impl Render`).
pub struct Composer {
    pub draft: String,
    pub markdown_tab: bool,
    pub on_tab: Option<ClickHandler>,
    pub on_send: Option<ClickHandler>,
}

impl Composer {
    pub fn new(draft: String, markdown_tab: bool) -> Self {
        Self {
            draft,
            markdown_tab,
            on_tab: None,
            on_send: None,
        }
    }

    pub fn on_tab(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_tab = Some(Rc::new(f));
        self
    }

    pub fn on_send(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_send = Some(Rc::new(f));
        self
    }
}

impl Render for Composer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let markdown_tab = self.markdown_tab;
        let mut tab = div()
            .id("composer-tab")
            .px_2()
            .py_1()
            .text_xs()
            .cursor_pointer()
            .border_1()
            .border_color(p::primary())
            .text_color(p::ink())
            .child(if markdown_tab { "MARKDOWN*".to_owned() } else { "MESSAGE".to_owned() });
        if let Some(f) = self.on_tab.clone() {
            tab = tab.on_click(move |ev, window, cx| f(ev, window, cx));
        }
        let send_label = div()
            .id("composer-send-label")
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .py_1()
            .text_sm()
            .bg(p::primary())
            .text_color(gpui::rgb(0xffffff))
            .cursor_pointer()
            .child("SEND".to_owned())
            .child(icon_el(IconKind::Send, 14.0, false, false));
        let mut send = div().flex().items_center().bg(p::primary());
        if let Some(f) = self.on_send.clone() {
            let g = f.clone();
            let labeled = send_label.on_click(move |ev, window, cx| g(ev, window, cx));
            send = send.child(labeled).child(
                div()
                    .px_1()
                    .py_1()
                    .text_sm()
                    .text_color(gpui::rgb(0xffffff))
                    .child("▾".to_owned()),
            );
        } else {
            send = send.child(send_label);
        }
        let draft = std::mem::take(&mut self.draft);
        div()
            .p_3()
            .gap_1()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(p::hairline())
            .bg(p::panel())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(tab)
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .text_color(p::ink_faint())
                                    .child(
                                        if markdown_tab {
                                            "MESSAGE"
                                        } else {
                                            "MARKDOWN"
                                        }
                                        .to_owned(),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_xs()
                            .border_1()
                            .border_color(p::hairline())
                            .text_color(p::ink_dim())
                            .child("SYNTAX: GLSL / MD ACTIVE".to_owned()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .px_2()
                            .py_1()
                            .text_sm()
                            .border_1()
                            .border_color(p::hairline())
                            .text_color(if draft.is_empty() {
                                p::ink_faint()
                            } else {
                                p::ink()
                            })
                            .child(if draft.is_empty() {
                                "Type a message...  (Enter sends)".to_owned()
                            } else {
                                draft
                            }),
                    )
                    .child(send),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(icon_el(IconKind::Attach, 15.0, false, false))
                    .child(icon_el(IconKind::Image, 15.0, false, false))
                    .child(icon_el(IconKind::Code, 15.0, false, false))
                    .child(icon_el(IconKind::Bold, 15.0, false, false))
                    .child(icon_el(IconKind::Italic, 15.0, false, false))
                    .child(icon_el(IconKind::Strike, 15.0, false, false))
                    .child(icon_el(IconKind::List, 15.0, false, false))
                    .child(icon_el(IconKind::Number, 15.0, false, false))
                    .child(icon_el(IconKind::Link, 15.0, false, false))
                    .child(icon_el(IconKind::Mention, 15.0, false, false)),
            )
    }
}
