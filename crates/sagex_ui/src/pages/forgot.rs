#![allow(dead_code)]
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    input::Input,
    label::Label,
    v_flex,
};

use crate::app::{AuthApp, AuthPage, auth_card, auth_header, auth_notice, back_to_login};

impl AuthApp {
    pub(crate) fn render_forgot(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let notice = self.notice.clone();
        auth_card()
            .child(auth_header(
                cx,
                "Reset your password",
                Some("Remember it?"),
                Some("Sign in"),
                "forgot-goto-login",
                cx.listener(|this, _, _, cx| {
                    this.navigate(AuthPage::Login, cx);
                }),
            ))
            .child(auth_notice(cx, &notice))
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Email").text_sm())
                    .child(Input::new(&self.forgot_email).cleanable(true).w_full()),
            )
            .child(
                Button::new("forgot-submit")
                    .primary()
                    .label("Send reset code")
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let email = this.forgot_email.read(cx).value();
                        window.prevent_default();
                        if email.trim().is_empty() {
                            this.notice = Some("Enter your account email.".to_string());
                            cx.notify();
                            return;
                        }
                        println!("forgot submit email={}", email);
                        this.navigate(AuthPage::Otp, cx);
                        this.notice = Some(format!("Code sent to {}", email.trim()));
                        cx.notify();
                    })),
            )
            .child(back_to_login(cx))
            .into_any_element()
    }
}
