//! `Desk` — the single root view: state, home/config, and all actions.
//! Network runs on the background executor via `net::spawn_bg`.
//! Rendering lives in `views`.

use gpui::{Context, Entity, Window, prelude::*};
use gpui_component::input::InputState;

use crate::{
    api::{Group, Message, Session},
    home::Home,
    net::spawn_bg,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Auth,
    Main,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Chat,
    Docs,
    Tokens,
}

impl Panel {
    pub fn as_str(self) -> &'static str {
        match self {
            Panel::Chat => "chat",
            Panel::Docs => "docs",
            Panel::Tokens => "tokens",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "docs" => Panel::Docs,
            "tokens" => Panel::Tokens,
            _ => Panel::Chat,
        }
    }
}

pub struct Desk {
    pub home: Home,
    pub base_url: String,
    pub screen: Screen,
    pub register_mode: bool,
    pub session: Option<Session>,
    pub groups: Vec<Group>,
    pub active_group: Option<String>,
    pub messages: Vec<Message>,
    pub panel: Panel,
    pub status: String,
    pub ca_token_text: String,
    pub pubkey_text: String,
    pub dl_url_text: String,
    // input entities
    pub user_input: Entity<InputState>,
    pub pass_input: Entity<InputState>,
    pub composer_input: Entity<InputState>,
    pub group_name_input: Entity<InputState>,
    pub group_members_input: Entity<InputState>,
    pub add_member_input: Entity<InputState>,
    pub file_input: Entity<InputState>,
    pub dlkey_input: Entity<InputState>,
}

impl Desk {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, home: Home, base_url: String) -> Self {
        let mk = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let panel = home
            .state
            .panel
            .as_deref()
            .map(Panel::from_str)
            .unwrap_or(Panel::Chat);
        let mut desk = Self {
            home,
            base_url,
            screen: Screen::Auth,
            register_mode: false,
            session: None,
            groups: vec![],
            active_group: None,
            messages: vec![],
            panel,
            status: "sign in to begin".to_string(),
            ca_token_text: String::new(),
            pubkey_text: String::new(),
            dl_url_text: String::new(),
            user_input: mk("username", window, cx),
            pass_input: mk("password", window, cx),
            composer_input: mk("message (sent as raw E2EE placeholder bytes)", window, cx),
            group_name_input: mk("new group name", window, cx),
            group_members_input: mk("members, comma separated", window, cx),
            add_member_input: mk("username to add", window, cx),
            file_input: mk("/path/to/file.enc", window, cx),
            dlkey_input: mk("object key to download", window, cx),
        };
        desk.auto_login(cx);
        desk
    }

    fn api_params(&self) -> (String, u64) {
        (
            self.base_url.clone(),
            self.home.config.request_timeout_secs,
        )
    }

    /// Persist UI state (screen-independent bits) to state.json.
    pub(crate) fn persist(&mut self) {
        self.home.state.active_group = self.active_group.clone();
        self.home.state.panel = Some(self.panel.as_str().to_string());
        if let Err(e) = self.home.save_state() {
            self.status = format!("state save: {e}");
        }
    }

    pub fn set_status(&mut self, s: impl Into<String>, _cx: &mut Context<Self>) {
        self.status = s.into();
    }

    pub fn active_group_name(&self) -> String {
        self.active_group
            .as_ref()
            .and_then(|id| self.groups.iter().find(|g| &g.object_key == id))
            .map(|g| g.name.clone())
            .unwrap_or_default()
    }

    // ---------- auth ----------

    pub fn start_login(
        &mut self,
        username: String,
        password: String,
        register: bool,
        cx: &mut Context<Self>,
    ) {
        let (base, timeout) = self.api_params();
        self.set_status("signing in…", cx);
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                if register {
                    api.register(&username, &password)?;
                }
                let login = api.login(&username, &password)?;
                let _ = crate::secrets::store_refresh(&username, &login.refresh_token);
                Ok::<_, String>(crate::api::Session {
                    base_url: base.clone(),
                    object_key: login.object_key,
                    username: login.username,
                    access_token: login.access_token,
                    refresh_token: login.refresh_token,
                })
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| desk.apply_session(res, cx)).ok();
            },
        );
    }

    pub fn apply_session(&mut self, res: Result<Session, String>, cx: &mut Context<Self>) {
        match res {
            Ok(s) => {
                self.home.state.last_user = Some(s.username.clone());
                self.session = Some(s);
                self.screen = Screen::Main;
                self.set_status("signed in", cx);
                self.persist();
                self.refresh_groups(cx);
            }
            Err(e) => self.set_status(format!("auth failed: {e}"), cx),
        }
    }

    /// Silent boot login from the keyring refresh token (if any).
    pub fn auto_login(&mut self, cx: &mut Context<Self>) {
        let username = match self.home.state.last_user.clone() {
            Some(u) if !u.is_empty() => u,
            _ => return,
        };
        let stored = match crate::secrets::load_refresh(&username) {
            Ok(Some(t)) => t,
            Ok(None) => return,
            Err(e) => {
                self.set_status(format!("keyring: {e}"), cx);
                return;
            }
        };
        let (base, timeout) = self.api_params();
        self.set_status(format!("restoring {username}…"), cx);
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                // Blank access token forces the refresh-and-retry path.
                let mut s = crate::api::Session {
                    base_url: base.clone(),
                    object_key: String::new(),
                    username: username.clone(),
                    access_token: String::new(),
                    refresh_token: stored,
                };
                api.groups(&mut s).map(|_| s)
            },
            |this, res: Result<Session, String>, cx| {
                this.update(cx, |desk, cx| match res {
                    Ok(s) => {
                        // Resolve identity from the refreshed session.
                        desk.session = Some(s);
                        desk.screen = Screen::Main;
                        desk.set_status("session restored", cx);
                        desk.refresh_groups(cx);
                    }
                    Err(e) => desk.set_status(format!("auto-login failed, sign in: {e}"), cx),
                })
                .ok();
            },
        );
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.session.take() {
            let _ = crate::secrets::clear_refresh(&s.username);
        }
        self.home.state.last_user = None;
        self.groups.clear();
        self.messages.clear();
        self.active_group = None;
        self.screen = Screen::Auth;
        self.set_status("signed out", cx);
        self.persist();
    }

    // ---------- groups + messages ----------

    pub fn refresh_groups(&mut self, cx: &mut Context<Self>) {
        let (base, timeout, session) = match &self.session {
            Some(s) => (self.base_url.clone(), self.home.config.request_timeout_secs, s.clone()),
            None => return,
        };
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                let mut s = session;
                api.groups(&mut s).map(|g| (s, g))
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| desk.apply_groups(res, cx)).ok();
            },
        );
    }

    pub fn refresh_messages(&mut self, cx: &mut Context<Self>) {
        let (base, timeout, session, gid) = match (&self.session, &self.active_group) {
            (Some(s), Some(g)) => (
                self.base_url.clone(),
                self.home.config.request_timeout_secs,
                s.clone(),
                g.clone(),
            ),
            _ => return,
        };
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                let mut s = session;
                api.messages(&mut s, &gid).map(|m| (s, m))
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| match res {
                    Ok((s, msgs)) => {
                        desk.session = Some(s);
                        desk.messages = msgs;
                        desk.set_status("messages loaded", cx);
                    }
                    Err(e) => desk.set_status(format!("messages: {e}"), cx),
                })
                .ok();
            },
        );
    }

    pub fn start_send(&mut self, body: String, cx: &mut Context<Self>) {
        let (base, timeout, session, gid) = match (&self.session, &self.active_group) {
            (Some(s), Some(g)) => (
                self.base_url.clone(),
                self.home.config.request_timeout_secs,
                s.clone(),
                g.clone(),
            ),
            _ => return,
        };
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                let mut s = session;
                api.post_message(&mut s, &gid, &body).map(|m| (s, m))
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| desk.apply_posted(res, cx)).ok();
            },
        );
    }

    pub fn apply_posted(
        &mut self,
        res: Result<(Session, Message), String>,
        cx: &mut Context<Self>,
    ) {
        match res {
            Ok((s, m)) => {
                self.session = Some(s);
                self.messages.push(m);
                self.set_status("sent", cx);
            }
            Err(e) => self.set_status(format!("send: {e}"), cx),
        }
    }

    pub fn apply_groups(
        &mut self,
        res: Result<(Session, Vec<Group>), String>,
        cx: &mut Context<Self>,
    ) {
        match res {
            Ok((s, groups)) => {
                self.session = Some(s);
                self.groups = groups;
                let still_there = self
                    .active_group
                    .as_ref()
                    .is_some_and(|id| self.groups.iter().any(|g| &g.object_key == id));
                if !still_there {
                    // Prefer the persisted group when it still exists.
                    let restored = self
                        .home
                        .state
                        .active_group
                        .clone()
                        .filter(|id| self.groups.iter().any(|g| &g.object_key == id));
                    self.active_group = restored
                        .or_else(|| self.groups.first().map(|g| g.object_key.clone()));
                }
                self.set_status("groups updated", cx);
                self.persist();
                self.refresh_messages(cx);
            }
            Err(e) => self.set_status(format!("groups: {e}"), cx),
        }
    }

    pub fn start_create_group(
        &mut self,
        name: String,
        members: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let (base, timeout, session) = match &self.session {
            Some(s) => (self.base_url.clone(), self.home.config.request_timeout_secs, s.clone()),
            None => return,
        };
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                let mut s = session;
                let _ = api.create_group(&mut s, &name, &members)?;
                api.groups(&mut s).map(|g| (s, g))
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| desk.apply_groups(res, cx)).ok();
            },
        );
    }

    pub fn start_add_member(&mut self, username: String, cx: &mut Context<Self>) {
        let (base, timeout, session, gid) = match (&self.session, &self.active_group) {
            (Some(s), Some(g)) => (
                self.base_url.clone(),
                self.home.config.request_timeout_secs,
                s.clone(),
                g.clone(),
            ),
            _ => return,
        };
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                let mut s = session;
                let _ = api.add_members(&mut s, &gid, &[username])?;
                api.groups(&mut s).map(|g| (s, g))
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| desk.apply_groups(res, cx)).ok();
            },
        );
    }

    pub fn start_leave(&mut self, cx: &mut Context<Self>) {
        let (base, timeout, session, gid) = match (&self.session, &self.active_group) {
            (Some(s), Some(g)) => (
                self.base_url.clone(),
                self.home.config.request_timeout_secs,
                s.clone(),
                g.clone(),
            ),
            _ => return,
        };
        let me = session.object_key.clone();
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                let mut s = session;
                let _ = api.remove_member(&mut s, &gid, &me)?;
                api.groups(&mut s).map(|g| (s, g))
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| desk.apply_groups(res, cx)).ok();
            },
        );
    }

    // ---------- documents ----------

    pub fn start_upload(&mut self, path: String, cx: &mut Context<Self>) {
        let (base, timeout, session) = match &self.session {
            Some(s) => (self.base_url.clone(), self.home.config.request_timeout_secs, s.clone()),
            None => return,
        };
        let filename = std::path::Path::new(&path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "upload.enc".to_string());
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                let mut s = session;
                let u = api.upload_url(&mut s, &filename)?;
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                api.put_bytes(&u.url, bytes)?;
                Ok::<_, String>((s, u.key))
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| match res {
                    Ok((s, key)) => {
                        desk.session = Some(s);
                        desk.set_status(format!("uploaded as {key}"), cx);
                    }
                    Err(e) => desk.set_status(format!("upload: {e}"), cx),
                })
                .ok();
            },
        );
    }

    pub fn start_download(&mut self, key: String, cx: &mut Context<Self>) {
        let (base, timeout, session) = match &self.session {
            Some(s) => (self.base_url.clone(), self.home.config.request_timeout_secs, s.clone()),
            None => return,
        };
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                let mut s = session;
                api.download_url(&mut s, &key).map(|u| (s, u.url))
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| match res {
                    Ok((s, url)) => {
                        desk.session = Some(s);
                        desk.dl_url_text = url;
                        desk.set_status("download URL ready", cx);
                    }
                    Err(e) => desk.set_status(format!("download: {e}"), cx),
                })
                .ok();
            },
        );
    }

    // ---------- tokens ----------

    pub fn start_ca_token(&mut self, cx: &mut Context<Self>) {
        let (base, timeout, session) = match &self.session {
            Some(s) => (self.base_url.clone(), self.home.config.request_timeout_secs, s.clone()),
            None => return,
        };
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                let mut s = session;
                api.ca_token(&mut s).map(|t| (s, t))
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| match res {
                    Ok((s, t)) => {
                        desk.session = Some(s);
                        desk.ca_token_text =
                            format!("{}\n{}\n{}", t.token_type, t.payload, t.signature);
                        desk.set_status("CA token fetched", cx);
                    }
                    Err(e) => desk.set_status(format!("CA token: {e}"), cx),
                })
                .ok();
            },
        );
    }

    pub fn start_pubkey(&mut self, cx: &mut Context<Self>) {
        let (base, timeout) = self.api_params();
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, timeout).map_err(|e| e)?;
                api.service_pubkey()
            },
            |this, res, cx| {
                this.update(cx, |desk, cx| match res {
                    Ok(p) => {
                        desk.pubkey_text = p.key_dsa;
                        desk.set_status("pubkey fetched", cx);
                    }
                    Err(e) => desk.set_status(format!("pubkey: {e}"), cx),
                })
                .ok();
            },
        );
    }

    // ---------- panel ----------

    pub fn switch_panel(&mut self, panel: Panel, cx: &mut Context<Self>) {
        self.panel = panel;
        self.persist();
        cx.notify();
    }
}
