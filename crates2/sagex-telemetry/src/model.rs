//! `Telemetry` — the single root view: state, home/config, and all actions.
//! Lookup only: fetch/search/verify ledger records via the gateway.
//! Network runs on the background executor via `net::spawn_bg`.
//! Rendering lives in `views`.

use gpui::{Context, Entity, Window, prelude::*};
use gpui_component::input::InputState;

use crate::{
    api::Block,
    home::Home,
    net::spawn_bg,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Main,
}

pub struct Telemetry {
    pub home: Home,
    pub gateway_url: String,
    pub screen: Screen,
    pub username: String,
    /// Login bearer key (memory-only; persisted in the OS keyring).
    pub bearer: String,
    /// One-time display of a freshly generated login key.
    pub fresh_key: String,
    /// This user's ML-DSA verifying key hex (for cross-checking records).
    pub my_vk_hex: String,
    pub blocks: Vec<Block>,
    pub filter: String,
    pub active_watermark: Option<String>,
    pub verdict: String,
    pub status: String,
    // input entities
    pub user_input: Entity<InputState>,
    pub search_input: Entity<InputState>,
}

impl Telemetry {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        home: Home,
        gateway_url: String,
    ) -> Self {
        let mk = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let last_user = home.state.last_user.clone().unwrap_or_default();
        let user_input = mk("username (e.g. alice-test)", window, cx);
        if !last_user.is_empty() {
            user_input.update(cx, |input, cx| {
                input.set_value(last_user.clone(), window, cx);
            });
        }
        let mut t = Self {
            home,
            gateway_url,
            screen: Screen::Login,
            username: String::new(),
            bearer: String::new(),
            fresh_key: String::new(),
            my_vk_hex: String::new(),
            blocks: vec![],
            filter: String::new(),
            active_watermark: None,
            verdict: String::new(),
            status: String::new(),
            user_input,
            search_input: mk("search watermark_id or recipient_id…", window, cx),
        };
        t.auto_login(cx);
        t
    }

    fn api_params(&self) -> (String, String, u64) {
        (
            self.gateway_url.clone(),
            self.bearer.clone(),
            self.home.config.request_timeout_secs,
        )
    }

    pub fn set_status(&mut self, s: impl Into<String>, cx: &mut Context<Self>) {
        self.status = s.into();
        cx.notify();
    }

    pub fn persist(&self) {
        let _ = self.home.save_state();
    }

    /// Filtered view of fetched blocks by the current search text.
    pub fn filtered(&self, query: &str) -> Vec<Block> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.blocks.clone();
        }
        self.blocks
            .iter()
            .filter(|b| {
                b.record.watermark_id.to_lowercase().contains(&q)
                    || b.record.recipient_id.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    pub fn active_block(&self) -> Option<Block> {
        self.active_watermark
            .as_ref()
            .and_then(|id| self.blocks.iter().find(|b| &b.record.watermark_id == id))
            .cloned()
    }

    // ---------- login ----------

    /// Silent boot login from the keyring login key (if any).
    pub fn auto_login(&mut self, cx: &mut Context<Self>) {
        let username = match self.home.state.last_user.clone() {
            Some(u) if !u.is_empty() => u,
            _ => return,
        };
        let stored = match crate::secrets::load_login_key(&username) {
            Ok(Some(t)) => t,
            Ok(None) => return,
            Err(e) => {
                self.set_status(format!("keyring: {e}"), cx);
                return;
            }
        };
        let (base, _, timeout) = self.api_params();
        self.username = username.clone();
        self.bearer = stored.clone();
        self.set_status(format!("restoring {username}…"), cx);
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, &stored, timeout)?;
                api.health()?;
                api.records()
            },
            |this, res: Result<Vec<crate::api::Block>, String>, cx| {
                this.update(cx, |t, cx| match res {
                    Ok(blocks) => {
                        t.screen = Screen::Main;
                        t.apply_records(Ok(blocks), cx);
                    }
                    Err(e) => t.set_status(format!("auto-login failed: {e}"), cx),
                })
                .ok();
            },
        );
    }

    /// Generate a fresh login key + DSA pair, store secrets in the keyring.
    pub fn start_generate_key(&mut self, username: String, cx: &mut Context<Self>) {
        if username.trim().is_empty() {
            self.set_status("enter a username first", cx);
            return;
        }
        self.set_status("generating keys…", cx);
        spawn_bg(
            cx,
            move || {
                let login_key = crate::keys::gen_login_key();
                // Static demo wrapping password; the seed handle stays in the keyring.
                let (seed_hex, vk_hex) =
                    crate::keys::gen_dsa_pair(b"sagex-telemetry-demo")?;
                crate::secrets::store_login_key(&username, &login_key)?;
                crate::secrets::store_dsa_seed(&username, &seed_hex)?;
                Ok::<_, String>((username, login_key, vk_hex))
            },
            |this, res, cx| {
                this.update(cx, |t, cx| t.apply_generated(res, cx)).ok();
            },
        );
    }

    fn apply_generated(
        &mut self,
        res: Result<(String, String, String), String>,
        cx: &mut Context<Self>,
    ) {
        match res {
            Ok((username, login_key, vk_hex)) => {
                self.username = username.clone();
                self.bearer = login_key.clone();
                self.fresh_key = login_key;
                self.my_vk_hex = vk_hex;
                self.home.state.last_user = Some(username);
                self.persist();
                self.set_status(
                    "keys generated — copy the login key to the gateway operator",
                    cx,
                );
            }
            Err(e) => self.set_status(format!("keygen failed: {e}"), cx),
        }
    }

    /// Sign in with the keyring-stored login key (health-checks the gateway).
    pub fn start_login(&mut self, username: String, cx: &mut Context<Self>) {
        if username.trim().is_empty() {
            self.set_status("enter a username", cx);
            return;
        }
        self.set_status("signing in…", cx);
        let (base, _, timeout) = self.api_params();
        spawn_bg(
            cx,
            move || {
                let stored = crate::secrets::load_login_key(&username)?
                    .ok_or_else(|| "no login key for this user — generate one first".to_string())?;
                let api = crate::api::Api::new(&base, &stored, timeout)?;
                api.health()?;
                let vk_hint = crate::secrets::load_dsa_seed(&username)?.unwrap_or_default();
                Ok::<_, String>((username, stored, base, vk_hint))
            },
            |this, res, cx| {
                this.update(cx, |t, cx| t.apply_login(res, cx)).ok();
            },
        );
    }

    fn apply_login(
        &mut self,
        res: Result<(String, String, String, String), String>,
        cx: &mut Context<Self>,
    ) {
        match res {
            Ok((username, bearer, _base, _seed_hint)) => {
                self.username = username.clone();
                self.bearer = bearer;
                self.fresh_key.clear();
                // Reload our verifying-key hint is not recoverable from the seed
                // handle; keep whatever was generated this session.
                self.home.state.last_user = Some(username);
                self.screen = Screen::Main;
                self.set_status("signed in — fetching records…", cx);
                self.persist();
                self.refresh_records(cx);
            }
            Err(e) => self.set_status(format!("login failed: {e}"), cx),
        }
    }

    // ---------- lookup (read-only) ----------

    /// Fetch the full chain from the gateway.
    pub fn refresh_records(&mut self, cx: &mut Context<Self>) {
        let (base, bearer, timeout) = self.api_params();
        self.set_status("fetching records…", cx);
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, &bearer, timeout)?;
                api.records()
            },
            |this, res: Result<Vec<Block>, String>, cx| {
                this.update(cx, |t, cx| t.apply_records(res, cx)).ok();
            },
        );
    }

    fn apply_records(&mut self, res: Result<Vec<Block>, String>, cx: &mut Context<Self>) {
        match res {
            Ok(blocks) => {
                let n = blocks.len();
                self.blocks = blocks;
                if self
                    .active_watermark
                    .as_ref()
                    .is_none_or(|id| !self.blocks.iter().any(|b| &b.record.watermark_id == id))
                {
                    self.active_watermark =
                        self.blocks.first().map(|b| b.record.watermark_id.clone());
                }
                self.set_status(format!("{n} records"), cx);
            }
            Err(e) => self.set_status(format!("fetch failed: {e}"), cx),
        }
    }

    /// Fetch one block by watermark id (exact lookup).
    pub fn lookup_exact(&mut self, watermark_id: String, cx: &mut Context<Self>) {
        if watermark_id.trim().is_empty() {
            return;
        }
        let (base, bearer, timeout) = self.api_params();
        self.set_status(format!("looking up {watermark_id}…"), cx);
        spawn_bg(
            cx,
            move || {
                let api = crate::api::Api::new(&base, &bearer, timeout)?;
                api.record(&watermark_id)
            },
            |this, res: Result<Block, String>, cx| {
                this.update(cx, |t, cx| t.apply_lookup(res, cx)).ok();
            },
        );
    }

    fn apply_lookup(&mut self, res: Result<Block, String>, cx: &mut Context<Self>) {
        match res {
            Ok(b) => {
                let id = b.record.watermark_id.clone();
                if !self.blocks.iter().any(|x| x.record.watermark_id == id) {
                    self.blocks.push(b);
                }
                self.active_watermark = Some(id);
                self.verdict.clear();
                self.set_status("record found", cx);
            }
            Err(e) => self.set_status(format!("lookup failed: {e}"), cx),
        }
    }

    /// Full verify of the selected block: inclusion + hash-link + ML-DSA.
    pub fn start_verify(&mut self, cx: &mut Context<Self>) {
        let Some(block) = self.active_block() else {
            self.set_status("select a record first", cx);
            return;
        };
        let chain = self.blocks.clone();
        // Re-fetch the chain in the background so inclusion is checked live.
        let (base, bearer, timeout) = self.api_params();
        self.set_status("verifying…", cx);
        spawn_bg(
            cx,
            move || {
                let live = crate::api::Api::new(&base, &bearer, timeout)
                    .and_then(|api| api.records())
                    .unwrap_or(chain);
                let v = crate::verify::verify(&block, &live);
                let id = block.record.watermark_id.clone();
                let ok = v.ok();
                (v.detail, ok, id)
            },
            |this, (detail, ok, id): (String, bool, String), cx| {
                this.update(cx, |t, cx| {
                    t.verdict = format!(
                        "{id}: {} — {detail}",
                        if ok { "VERIFIED" } else { "FAILED" }
                    );
                    t.set_status(
                        if ok { "signature + chain verified" } else { "verification FAILED" },
                        cx,
                    );
                })
                .ok();
            },
        );
    }
}
