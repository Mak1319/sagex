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
                    .label(if self.busy {
                        "Verifying…"
                    } else {
                        "Verify code"
                    })
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let code = this.otp.read(cx).value().to_string();
                        window.prevent_default();
                        if code.chars().count() != 6 {
                            this.notice = Some("Enter the 6-digit code.".to_string());
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
                        let email = this.pending_email.clone();
                        let purpose = this.pending_purpose.clone();
                        crate::backend::request(
                            cx,
                            async move { api.verify_otp_raw(&email, &code, &purpose).await },
                            |this, res, cx| match res {
                                Ok(v) => {
                                    if this.pending_purpose == "reset" {
                                        this.busy = false;
                                        this.navigate(AuthPage::Reset, cx);
                                        this.notice =
                                            Some("Code verified. Set a new password.".to_string());
                                        cx.notify();
                                    } else if let Some(tokens) =
                                        crate::backend::ApiClient::tokens_of(&v)
                                    {
                                        // signup/login: fetch profile, then enter chat.
                                        let api = this.api.clone();
                                        let access = tokens.access_token.clone();
                                        crate::backend::request(
                                            cx,
                                            async move {
                                                let me = api.me(&access).await?;
                                                Ok::<_, crate::backend::BackendError>((tokens, me))
                                            },
                                            |this, res2, cx| match res2 {
                                                Ok((tokens, me)) => this.signed_in(tokens, me, cx),
                                                Err(e) => this.fail(e, cx),
                                            },
                                        )
                                        .detach();
                                    } else {
                                        this.busy = false;
                                        this.notice = Some("Unexpected server reply.".to_string());
                                        cx.notify();
                                    }
                                }
                                Err(e) => this.fail(e, cx),
                            },
                        )
                        .detach();
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
                            .label(if self.busy {
                                "Sending…"
                            } else {
                                "Resend code"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.busy || this.pending_email.is_empty() {
                                    return;
                                }
                                this.busy = true;
                                this.notice = None;
                                cx.notify();
                                let api = this.api.clone();
                                let email = this.pending_email.clone();
                                let purpose = this.pending_purpose.clone();
                                crate::backend::request(
                                    cx,
                                    async move { api.request_otp(&email, &purpose).await },
                                    |this, res: Result<(), crate::backend::BackendError>, cx| {
                                        match res {
                                            Ok(()) => {
                                                this.busy = false;
                                                this.notice =
                                                    Some("A fresh code was sent.".to_string());
                                                cx.notify();
                                            }
                                            Err(e) => this.fail(e, cx),
                                        }
                                    },
                                )
                                .detach();
                            })),
                    ),
            )
            .child(back_to_login(cx))
            .into_any_element()
    }
}
