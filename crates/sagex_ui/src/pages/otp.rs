#![allow(dead_code)]
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    input::OtpInput,
};

use crate::app::{AuthApp, AuthPage, auth_card, auth_header, auth_notice, back_to_login};

impl AuthApp {
    pub(crate) fn render_otp(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let notice = self.notice.clone();
        auth_card()
            .child(auth_header(
                cx,
                "Check your email",
                None,
                None,
                "otp-noop",
                |_, _, _| {},
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .text_center()
                    .child("Enter the 6-digit code we sent you."),
            )
            .child(auth_notice(cx, &notice))
            .child(
                div()
                    .flex()
                    .justify_center()
                    .w_full()
                    .child(OtpInput::new(&self.otp).groups(2).large()),
            )
            .child(
                Button::new("otp-verify")
                    .primary()
                    .label("Verify code")
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let code = this.otp.read(cx).value().to_string();
                        window.prevent_default();
                        if code.chars().count() != 6 {
                            this.notice = Some("Enter the 6-digit code.".to_string());
                            cx.notify();
                            return;
                        }
                        println!("otp verify code={}", code);
                        this.navigate(AuthPage::Reset, cx);
                        this.notice = Some("Code verified. Set a new password.".to_string());
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .w_full()
                    .child(
                        Button::new("otp-resend")
                            .link()
                            .small()
                            .label("Resend code")
                            .on_click(|_, _, _| {
                                println!("otp resend");
                            }),
                    ),
            )
            .child(back_to_login(cx))
            .into_any_element()
    }
}
