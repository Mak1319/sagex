#![allow(dead_code)]
use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::InputState,
    input::OtpState,
    v_flex,
};

use crate::backend::{ApiClient, SessionStore, StoredSession};
use crate::component::ThemeToggle;

/// Emitted by `AuthApp` when a flow completes with a live session.
#[derive(Debug, Clone)]
pub enum AuthEvent {
    SignedIn(StoredSession),
}

impl gpui::EventEmitter<AuthEvent> for AuthApp {}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthPage {
    #[default]
    Login,
    Signup,
    Forgot,
    Otp,
    Reset,
}

/// Top-level auth router. Holds every form entity up-front so page
/// switches don't need a `Window` to (re)create `InputState`s.
pub struct AuthApp {
    pub page: AuthPage,
    pub login_email: Entity<InputState>,
    pub login_password: Entity<InputState>,
    pub signup_name: Entity<InputState>,
    pub signup_email: Entity<InputState>,
    pub signup_password: Entity<InputState>,
    pub signup_confirm: Entity<InputState>,
    pub forgot_email: Entity<InputState>,
    pub otp: Entity<OtpState>,
    pub reset_password: Entity<InputState>,
    pub reset_confirm: Entity<InputState>,
    pub remember_me: bool,
    pub theme_mode: crate::component::ThemeChoice,
    pub notice: Option<String>,
    pub(crate) appearance_sub: Option<Subscription>,
    // ---- live backend wiring ----
    pub api: ApiClient,
    pub store: SessionStore,
    /// email awaiting OTP + which flow it belongs to (signup|login|reset)
    pub pending_email: String,
    pub pending_purpose: String,
    /// a network call is in flight; submit buttons show busy labels
    pub busy: bool,
}

impl AuthApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        login_email: Entity<InputState>,
        login_password: Entity<InputState>,
        signup_name: Entity<InputState>,
        signup_email: Entity<InputState>,
        signup_password: Entity<InputState>,
        signup_confirm: Entity<InputState>,
        forgot_email: Entity<InputState>,
        otp: Entity<OtpState>,
        reset_password: Entity<InputState>,
        reset_confirm: Entity<InputState>,
        api: ApiClient,
        store: SessionStore,
    ) -> Self {
        Self {
            page: AuthPage::Login,
            login_email,
            login_password,
            signup_name,
            signup_email,
            signup_password,
            signup_confirm,
            forgot_email,
            otp,
            reset_password,
            reset_confirm,
            remember_me: true,
            theme_mode: crate::component::ThemeChoice::System,
            notice: None,
            appearance_sub: None,
            api,
            store,
            pending_email: String::new(),
            pending_purpose: "signup".to_string(),
            busy: false,
        }
    }

    /// Build a persistable session from tokens + profile, save, and emit.
    pub(crate) fn signed_in(
        &mut self,
        tokens: crate::backend::Tokens,
        me: crate::backend::UserDto,
        cx: &mut Context<Self>,
    ) {
        let sess = StoredSession {
            access_token: tokens.access_token,
            access_expires_at: tokens.access_expires_at,
            refresh_token: tokens.refresh_token,
            refresh_expires_at: tokens.refresh_expires_at,
            user_id: me.id,
            email: me.email,
            saved_at: crate::backend::now_unix(),
        };
        self.store.set(sess.clone());
        self.busy = false;
        cx.emit(AuthEvent::SignedIn(sess));
        cx.notify();
    }

    pub(crate) fn fail(&mut self, err: crate::backend::BackendError, cx: &mut Context<Self>) {
        self.busy = false;
        self.notice = Some(err.to_string());
        cx.notify();
    }

    pub fn navigate(&mut self, page: AuthPage, cx: &mut Context<Self>) {
        self.page = page;
        self.notice = None;
        cx.notify();
    }

    fn render_theme_bar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .w_full()
            .px_4()
            .pt_4()
            .child(
                ThemeToggle::new(self.theme_mode)
                    .horizontal()
                    .on_select(cx.listener(
                        |this, mode: &crate::component::ThemeChoice, window, cx| {
                            this.theme_mode = *mode;
                            mode.apply(window, cx);
                            cx.notify();
                        },
                    )),
            )
    }
}

impl Render for AuthApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let page: gpui::AnyElement = match self.page {
            AuthPage::Login => self.render_login(window, cx).into_any_element(),
            AuthPage::Signup => self.render_signup(window, cx).into_any_element(),
            AuthPage::Forgot => self.render_forgot(window, cx).into_any_element(),
            AuthPage::Otp => self.render_otp(window, cx).into_any_element(),
            AuthPage::Reset => self.render_reset(window, cx).into_any_element(),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(self.render_theme_bar(window, cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_6()
                    .p_6()
                    .child(page),
            )
    }
}

/// Shared centered card: `w-full max-w-sm (384px) + flex-col gap-6`.
pub fn auth_card() -> gpui::Div {
    div().w_full().max_w(px(384.)).flex().flex_col().gap_6()
}

/// Shared logo + title + optional subtitle row.
pub fn auth_header(
    cx: &App,
    title: &str,
    subtitle_prefix: Option<&str>,
    subtitle_link: Option<&str>,
    link_id: &'static str,
    on_link: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    v_flex()
        .items_center()
        .gap_2()
        .text_center()
        .child(
            div()
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .text_color(theme.foreground)
                .child(Icon::new(IconName::GalleryVerticalEnd).size_6()),
        )
        .child(
            div()
                .text_xl()
                .font_bold()
                .text_color(theme.foreground)
                .child(title.to_string()),
        )
        .when_some(
            subtitle_prefix.zip(subtitle_link),
            |this, (prefix, link)| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(prefix.to_string())
                        .child(
                            Button::new(link_id)
                                .link()
                                .small()
                                .label(link.to_string())
                                .on_click(on_link),
                        ),
                )
            },
        )
}

/// Small muted notice / error line under the header. No-op when `None`.
pub fn auth_notice(cx: &App, notice: &Option<String>) -> impl IntoElement {
    let theme = cx.theme().clone();
    div().when_some(notice.clone(), |this, msg| {
        this.child(
            div()
                .w_full()
                .text_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(msg),
        )
    })
}

/// Muted footer used on email/password pages.
pub fn auth_footer(cx: &App) -> impl IntoElement {
    let theme = cx.theme().clone();
    div()
        .px_6()
        .text_center()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child("By clicking continue, you agree to our Terms of Service and Privacy Policy.")
}

/// Back-to-login link row used on secondary pages.
pub fn back_to_login(cx: &mut Context<AuthApp>) -> impl IntoElement {
    div().flex().items_center().justify_center().w_full().child(
        Button::new("back-to-login")
            .link()
            .small()
            .label("Back to login")
            .on_click(cx.listener(|this, _, _, cx| {
                this.navigate(AuthPage::Login, cx);
            })),
    )
}
