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
                    .label(if self.busy {
                        "Sending…"
                    } else {
                        "Send reset code"
                    })
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let email = this.forgot_email.read(cx).value().trim().to_string();
                        window.prevent_default();
                        if email.is_empty() {
                            this.notice = Some("Enter your account email.".to_string());
                            cx.notify();
                            return;
                        }
                        if this.busy {
                            return;
                        }
                        this.busy = true;
                        this.notice = None;
                        cx.notify();
                        let api = this.api.clone();
                        crate::backend::request(
                            cx,
                            async move {
                                api.forgot_password(&email).await?;
                                Ok::<_, crate::backend::BackendError>(email)
                            },
                            |this, res, cx| match res {
                                Ok(email) => {
                                    this.busy = false;
                                    this.pending_email = email.clone();
                                    this.pending_purpose = "reset".to_string();
                                    this.navigate(AuthPage::Otp, cx);
                                    this.notice =
                                        Some("If the email exists, a code was sent.".to_string());
                                    cx.notify();
                                }
                                Err(e) => this.fail(e, cx),
                            },
                        )
                        .detach();
                    })),
            )
            .child(back_to_login(cx))
            .into_any_element()
    }
}
