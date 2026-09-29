//! Far-left icon rail.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder, px, rgb,
};
use gpui_component::{ActiveTheme, Icon, IconName};

use super::ChatApp;
use crate::component::{ThemeChoice, ThemeToggle};

impl ChatApp {
    pub(super) fn render_rail(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let item = |icon: Icon, active: bool, id: &'static str| {
            div()
                .size(px(40.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(10.))
                .cursor_pointer()
                .when(active, |t| t.bg(theme.muted).text_color(theme.foreground))
                .when(!active, |t| {
                    t.text_color(theme.muted_foreground)
                        .hover(|s| s.bg(theme.muted))
                })
                .id(id)
                .child(icon.size(px(20.)))
        };
        div()
            .w(px(60.))
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .py_3()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(item(
                Icon::empty().path("icons/message-circle.svg"),
                true,
                "rail-chats",
            ))
            .child(item(
                Icon::empty().path("icons/users.svg"),
                false,
                "rail-groups",
            ))
            .child(
                item(Icon::new(IconName::Settings), false, "rail-settings").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.show_profile = !this.show_profile;
                        this.show_new_chat = false;
                        cx.notify();
                    },
                )),
            )
            .child(div().flex_1())
            .child(
                // realtime connection dot: green / amber / red
                div().size(px(8.)).rounded_full().bg(rgb(match self.conn {
                    super::ConnStatus::Online => 0x25d366,
                    super::ConnStatus::Connecting => 0xe6890b,
                    super::ConnStatus::Offline => 0xd3396c,
                })),
            )
            .child(ThemeToggle::new(self.theme_mode).on_select(cx.listener(
                |this, mode: &ThemeChoice, window, cx| {
                    this.theme_mode = *mode;
                    mode.apply(window, cx);
                    cx.notify();
                },
            )))
            .child(
                div()
                    .size(px(32.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x54656f))
                    .text_color(rgb(0xffffff))
                    .text_sm()
                    .cursor_pointer()
                    .id("rail-avatar")
                    .child(crate::chat::model::sender_initials(&self.my_name()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_profile = !this.show_profile;
                        this.show_new_chat = false;
                        cx.notify();
                    })),
            )
    }
}
