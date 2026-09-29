#![allow(dead_code)]
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Input,
    label::Label,
    v_flex,
};

use crate::app::{AuthApp, AuthPage, auth_card, auth_footer, auth_header, auth_notice};

impl AuthApp {
    pub(crate) fn render_login(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let notice = self.notice.clone();
        auth_card()
            .child(auth_header(
                cx,
                "Welcome back",
                Some("Don't have an account?"),
                Some("Sign up"),
                "login-goto-signup",
                cx.listener(|this, _, _, cx| {
                    this.navigate(AuthPage::Signup, cx);
                }),
            ))
            .child(auth_notice(cx, &notice))
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Email").text_sm())
                    .child(Input::new(&self.login_email).cleanable(true).w_full()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(Label::new("Password").text_sm())
                            .child(
                                Button::new("login-forgot")
                                    .link()
                                    .small()
                                    .label("Forgot password?")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.navigate(AuthPage::Forgot, cx);
                                    })),
                            ),
                    )
                    .child(Input::new(&self.login_password).mask_toggle().w_full()),
            )
            .child(
                Checkbox::new("login-remember")
                    .label("Remember me (30 days)")
                    .checked(self.remember_me)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.remember_me = *checked;
                        this.store.persist = *checked;
                        if !checked {
                            this.store.drop_file();
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("login-submit")
                    .primary()
                    .label(if self.busy { "Signing in…" } else { "Login" })
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let email = this.login_email.read(cx).value().trim().to_string();
                        let pass = this.login_password.read(cx).value().to_string();
                        window.prevent_default();
                        if email.is_empty() || pass.is_empty() {
                            this.notice = Some("Enter your email and password.".to_string());
                            cx.notify();
                            return;
                        }
                        if this.busy {
                            return;
                        }
                        this.busy = true;
                        this.notice = None;
                        // Stash for vault unlock / device enrollment (in-memory
                        // only, zeroized after use; never logged or persisted).
                        this.enroll.set_password(pass.clone());
                        cx.notify();
                        let api = this.api.clone();
                        crate::backend::request(
                            cx,
                            async move {
                                let tokens = api.login(&email, &pass).await?;
                                let me = api.me(&tokens.access_token).await?;
                                Ok::<_, crate::backend::BackendError>((tokens, me))
                            },
                            |this, res, cx| match res {
                                Ok((tokens, me)) => this.signed_in(tokens, me, cx),
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
