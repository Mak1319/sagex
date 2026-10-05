use std::rc::Rc;

use gpui::{App, ClickEvent, IntoElement, Render, Window, div, prelude::*};

use super::{avatar::avatar_el, badge::unread_el, icon::icon_el};
use crate::{components::IconKind, models::Channel, theme::palette as p};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// One conversation-list entry. Real GPUI view (`impl Render`),
/// clickable via stored handler.
pub struct ChannelRow {
    pub channel: Channel,
    pub active: bool,
    pub on_click: Option<ClickHandler>,
}

impl ChannelRow {
    pub fn new(channel: Channel) -> Self {
        Self {
            channel,
            active: false,
            on_click: None,
        }
    }

    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl Render for ChannelRow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let c = &self.channel;
        let eid = c.id.0 as usize;
        let label = if c.pinned {
            format!("* {}", c.name)
        } else {
            c.name.to_string()
        };
        let mut el = div()
            .id(eid)
            .flex()
            .gap_2()
            .px_3()
            .py_2()
            .cursor_pointer()
            .items_center()
            .text_sm()
            .text_color(if self.active { p::ink() } else { p::ink_dim() })
            .child(avatar_el(c.tag, self.active))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(label)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(p::ink_faint())
                                            .child(c.time.to_string()),
                                    )
                                    .child(if c.unread > 0 {
                                        unread_el(c.unread)
                                    } else if c.pinned {
                                        icon_el(IconKind::Pin, 13.0, true, false)
                                    } else {
                                        div()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_color(p::ink_faint())
                            .child(c.preview.to_string()),
                    ),
            );
        if self.active {
            el = el.bg(p::panel_hi()).border_l_2().border_color(p::primary());
        }
        if let Some(f) = self.on_click.clone() {
            el = el.on_click(move |ev, window, cx| f(ev, window, cx));
        }
        el
    }
}
