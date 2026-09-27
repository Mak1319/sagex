#![allow(dead_code)]
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants},
    input::Input,
    label::Label,
    v_flex,
};

use crate::app::{AuthApp, AuthPage, auth_card, auth_footer, auth_header, auth_notice};

impl AuthApp {
    pub(crate) fn render_signup(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let notice = self.notice.clone();
        auth_card()
            .child(auth_header(
                cx,
                "Create your account",
                Some("Already have an account?"),
                Some("Sign in"),
                "signup-goto-login",
                cx.listener(|this, _, _, cx| {
                    this.navigate(AuthPage::Login, cx);
                }),
            ))
            .child(auth_notice(cx, &notice))
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Name").text_sm())
                    .child(Input::new(&self.signup_name).cleanable(true).w_full()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Email").text_sm())
                    .child(Input::new(&self.signup_email).cleanable(true).w_full()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Password").text_sm())
                    .child(Input::new(&self.signup_password).mask_toggle().w_full()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Confirm password").text_sm())
                    .child(Input::new(&self.signup_confirm).mask_toggle().w_full()),
            )
            .child(
                Button::new("signup-submit")
                    .primary()
                    .label("Create Account")
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let name = this.signup_name.read(cx).value();
                        let email = this.signup_email.read(cx).value();
                        let p1 = this.signup_password.read(cx).value();
                        let p2 = this.signup_confirm.read(cx).value();
                        window.prevent_default();
                        if name.trim().is_empty() || email.trim().is_empty() || p1.is_empty() {
                            this.notice = Some("Fill in name, email and password.".to_string());
                            cx.notify();
                            return;
                        }
                        if p1 != p2 {
                            this.notice = Some("Passwords don't match.".to_string());
                            cx.notify();
                            return;
                        }
                        println!("signup submit name={} email={}", name, email);
                        // Email verification step.
                        this.navigate(AuthPage::Otp, cx);
                        this.notice = Some(format!("Code sent to {}", email.trim()));
                        cx.notify();
                    })),
            )
            .child(auth_footer(cx))
            .into_any_element()
    }
}
