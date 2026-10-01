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
    pub(crate) fn render_reset(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let notice = self.notice.clone();
        auth_card()
            .child(auth_header(
                cx,
                "Set a new password",
                None,
                None,
                "reset-noop",
                |_, _, _| {},
            ))
            .child(auth_notice(cx, &notice))
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("New password").text_sm())
                    .child(Input::new(&self.reset_password).mask_toggle().w_full()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Confirm password").text_sm())
                    .child(Input::new(&self.reset_confirm).mask_toggle().w_full()),
            )
            .child(
                Button::new("reset-submit")
                    .primary()
                    .label(if self.busy {
                        "Saving…"
                    } else {
                        "Save new password"
                    })
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let p1 = this.reset_password.read(cx).value().to_string();
                        let p2 = this.reset_confirm.read(cx).value().to_string();
                        window.prevent_default();
                        if p1.is_empty() {
                            this.notice = Some("Enter a new password.".to_string());
                            cx.notify();
                            return;
                        }
                        if p1 != p2 {
                            this.notice = Some("Passwords don't match.".to_string());
                            cx.notify();
                            return;
                        }
                        let code = this.otp.read(cx).value().to_string();
                        if code.chars().count() != 6 {
                            this.notice =
                                Some("Go back and verify the 6-digit code first.".to_string());
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
                        crate::backend::request(
                            cx,
                            async move { api.reset_password(&email, &code, &p1).await },
                            |this, res: Result<(), crate::backend::BackendError>, cx| match res {
                                Ok(()) => {
                                    this.busy = false;
                                    this.navigate(AuthPage::Login, cx);
                                    this.notice = Some(
                                        "Password updated. Sign in on all devices.".to_string(),
                                    );
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
