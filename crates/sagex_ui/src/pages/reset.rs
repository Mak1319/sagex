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
                    .label("Save new password")
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let p1 = this.reset_password.read(cx).value();
                        let p2 = this.reset_confirm.read(cx).value();
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
                        println!("reset submit len={}", p1.len());
                        this.navigate(AuthPage::Login, cx);
                        this.notice = Some("Password updated. Sign in.".to_string());
                        cx.notify();
                    })),
            )
            .child(back_to_login(cx))
            .into_any_element()
    }
}
