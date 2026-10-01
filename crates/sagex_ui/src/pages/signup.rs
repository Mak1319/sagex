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
                    .label(if self.busy {
                        "Creating…"
                    } else {
                        "Create Account"
                    })
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let name = this.signup_name.read(cx).value().trim().to_string();
                        let email = this.signup_email.read(cx).value().trim().to_string();
                        let p1 = this.signup_password.read(cx).value().to_string();
                        let p2 = this.signup_confirm.read(cx).value().to_string();
                        window.prevent_default();
                        if name.is_empty() || email.is_empty() || p1.is_empty() {
                            this.notice = Some("Fill in name, email and password.".to_string());
                            cx.notify();
                            return;
                        }
                        if p1 != p2 {
                            this.notice = Some("Passwords don't match.".to_string());
                            cx.notify();
                            return;
                        }
                        if this.busy {
                            return;
                        }
                        this.busy = true;
                        this.notice = None;
                        // Stash for vault unlock / device enrollment after
                        // the OTP step (in-memory only, zeroized after use).
                        this.enroll.set_password(p1.clone());
                        cx.notify();
                        let api = this.api.clone();
                        // signup_name doubles as the username for the API.
                        let username = name.clone();
                        crate::backend::request(
                            cx,
                            async move {
                                api.signup(&email, &username, &p1, None).await?;
                                Ok::<_, crate::backend::BackendError>(email)
                            },
                            |this, res, cx| match res {
                                Ok(email) => {
                                    this.busy = false;
                                    this.pending_email = email.clone();
                                    this.pending_purpose = "signup".to_string();
                                    this.navigate(AuthPage::Otp, cx);
                                    this.notice = Some(format!("Code sent to {email}"));
                                    cx.notify();
                                }
                                Err(e) => this.fail(e, cx),
                            },
                        )
                        .detach();
                    })),
            )
            .child(auth_footer(cx))
            .into_any_element()
    }
}
