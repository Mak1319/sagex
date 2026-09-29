//! Root view: auth-gated on first launch, straight into chat when a
//! persisted JOSE session (≤ `SAGEX_SESSION_MAX_AGE_DAYS`, root `.env`)
//! refreshes successfully.

use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div,
};
use gpui_component::{ActiveTheme, StyledExt as _, input::InputState, input::OtpState};

use crate::app::{AuthApp, AuthEvent};
use crate::backend::{
    ApiClient, BackendConfig, BackendError, SessionStore, StoredSession, request,
};
use crate::chat::{ChatApp, ChatEvent};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShellMode {
    Loading,
    Auth,
    Chat,
}

/// All `InputState`s are created up-front in `main` (they need a `Window`);
/// auth inputs move into `AuthApp` immediately, chat inputs wait in the shell.
pub struct ShellInputs {
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
    pub search: Entity<InputState>,
    pub composer: Entity<InputState>,
    pub emoji_search: Entity<InputState>,
    pub user_search: Entity<InputState>,
    pub room_name: Entity<InputState>,
    pub profile_name: Entity<InputState>,
}

pub struct AppShell {
    mode: ShellMode,
    status: String,
    auth: Entity<AuthApp>,
    chat: Option<Entity<ChatApp>>,
    api: ApiClient,
    store: SessionStore,
    max_age_days: i64,
    search: Option<Entity<InputState>>,
    composer: Option<Entity<InputState>>,
    emoji_search: Option<Entity<InputState>>,
    user_search: Option<Entity<InputState>>,
    room_name: Option<Entity<InputState>>,
    profile_name: Option<Entity<InputState>>,
    pub(crate) appearance_sub: Option<Subscription>,
}

impl AppShell {
    pub fn new(
        cfg: BackendConfig,
        api: ApiClient,
        store: SessionStore,
        inputs: ShellInputs,
        cx: &mut Context<Self>,
    ) -> Self {
        let auth = cx.new(|_| {
            AuthApp::new(
                inputs.login_email,
                inputs.login_password,
                inputs.signup_name,
                inputs.signup_email,
                inputs.signup_password,
                inputs.signup_confirm,
                inputs.forgot_email,
                inputs.otp,
                inputs.reset_password,
                inputs.reset_confirm,
                api.clone(),
                store.clone(),
            )
        });
        cx.subscribe(&auth, |this, _, ev: &AuthEvent, cx| match ev {
            AuthEvent::SignedIn(sess) => this.enter_chat(sess.clone(), cx),
        })
        .detach();
        let mut this = Self {
            mode: ShellMode::Loading,
            status: format!("Connecting to {}…", cfg.server_url),
            auth,
            chat: None,
            api,
            store,
            max_age_days: cfg.session_max_age_days,
            search: Some(inputs.search),
            composer: Some(inputs.composer),
            emoji_search: Some(inputs.emoji_search),
            user_search: Some(inputs.user_search),
            room_name: Some(inputs.room_name),
            profile_name: Some(inputs.profile_name),
            appearance_sub: None,
        };
        this.try_auto_login(cx);
        this
    }

    /// Whether the visible view follows the OS appearance (System mode).
    pub fn follows_system(&self, cx: &gpui::App) -> bool {
        match self.mode {
            ShellMode::Auth => self.auth.read(cx).theme_mode.is_system(),
            ShellMode::Chat => self
                .chat
                .as_ref()
                .map(|c| c.read(cx).theme_mode.is_system())
                .unwrap_or(true),
            ShellMode::Loading => true,
        }
    }

    fn try_auto_login(&mut self, cx: &mut Context<Self>) {
        let Some(sess) = self.store.usable(self.max_age_days) else {
            self.mode = ShellMode::Auth;
            self.status = "Sign in to continue.".to_string();
            cx.notify();
            return;
        };
        let api = self.api.clone();
        let rt = sess.refresh_token.clone();
        request(
            cx,
            async move {
                let tokens = api.refresh(&rt).await?;
                let me = api.me(&tokens.access_token).await?;
                Ok::<_, BackendError>((tokens, me))
            },
            |this, res: Result<_, BackendError>, cx| match res {
                Ok((tokens, me)) => {
                    let sess = StoredSession {
                        access_token: tokens.access_token.clone(),
                        access_expires_at: tokens.access_expires_at,
                        refresh_token: tokens.refresh_token.clone(),
                        refresh_expires_at: tokens.refresh_expires_at,
                        user_id: me.id,
                        email: me.email,
                        saved_at: crate::backend::now_unix(),
                    };
                    this.store.set(sess.clone());
                    this.enter_chat(sess, cx);
                }
                Err(_) => {
                    this.mode = ShellMode::Auth;
                    this.status = "Session expired — sign in again.".to_string();
                    cx.notify();
                }
            },
        )
        .detach();
    }

    fn enter_chat(&mut self, sess: StoredSession, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let store = self.store.clone();
        let chat = cx.new(|_| {
            ChatApp::new(
                self.search.take().expect("chat inputs consumed twice"),
                self.composer.take().expect("chat inputs consumed twice"),
                self.emoji_search
                    .take()
                    .expect("chat inputs consumed twice"),
                self.user_search.take().expect("chat inputs consumed twice"),
                self.room_name.take().expect("chat inputs consumed twice"),
                self.profile_name
                    .take()
                    .expect("chat inputs consumed twice"),
                api,
                store,
            )
        });
        cx.subscribe(&chat, |this, _, ev: &ChatEvent, cx| match ev {
            ChatEvent::SignedOut => {
                this.chat = None;
                this.mode = ShellMode::Auth;
                this.status = "Signed out.".to_string();
                cx.notify();
            }
        })
        .detach();
        chat.update(cx, |c, cx| c.boot(sess, cx));
        self.chat = Some(chat);
        self.mode = ShellMode::Chat;
        cx.notify();
    }
}

impl Render for AppShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        match self.mode {
            ShellMode::Chat => {
                if let Some(chat) = self.chat.clone() {
                    div()
                        .size_full()
                        .bg(theme.background)
                        .child(chat)
                        .into_any_element()
                } else {
                    self.render_splash(cx).into_any_element()
                }
            }
            ShellMode::Auth => div()
                .size_full()
                .bg(theme.background)
                .child(self.auth.clone())
                .into_any_element(),
            ShellMode::Loading => self.render_splash(cx).into_any_element(),
        }
    }
}

impl AppShell {
    fn render_splash(&self, cx: &mut gpui::App) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .bg(theme.background)
            .child(
                div()
                    .text_lg()
                    .font_bold()
                    .text_color(theme.foreground)
                    .child("sagex chat"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(self.status.clone()),
            )
    }
}
