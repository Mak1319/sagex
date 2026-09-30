//! Live server sync for `ChatApp`: rooms, history, send, realtime WS,
//! users, profile, logout. All network runs on the dedicated Tokio runtime
//! via `backend::request` — the GPUI thread only applies results.

use std::sync::{Arc, Mutex};

use gpui::Context;
use tokio::sync::mpsc as tmpsc;

use super::{
    ChatApp, ChatEvent, ConnStatus,
    model::{
        Chat, ChatKind, Message, MessageKind, MessageStatus, SealState, extract_link,
        sender_color, sender_initials,
    },
    sealed::{SealedDone, StoredSeal, detect_sealed, run_sealed_receive},
};
use crate::backend::{
    ApiClient, ApiResult, BackendError, MessageDto, RoomDto, StoredSession, WsCmd, WsEvent,
    now_unix, request, start_ws, ws_url_for,
};

/// Boxed continuation run after the access token is known-fresh.
type AfterAuth = Box<dyn FnOnce(&mut ChatApp, &mut Context<ChatApp>) + 'static>;

impl ChatApp {
    // ---------- boot ----------
    /// Demo boot: keep seeds on screen, silently upgrade to live data when
    /// a persisted session (≤ `max_age_days`) still refreshes.
    pub fn boot_demo(&mut self, max_age_days: i64, cx: &mut Context<Self>) {
        let Some(sess) = self.store.usable(max_age_days) else {
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
            |this, res: ApiResult<(crate::backend::Tokens, crate::backend::UserDto)>, cx| {
                match res {
                    Ok((tokens, me)) => {
                        let sess = StoredSession {
                            access_token: tokens.access_token.clone(),
                            access_expires_at: tokens.access_expires_at,
                            refresh_token: tokens.refresh_token.clone(),
                            refresh_expires_at: tokens.refresh_expires_at,
                            user_id: me.id.clone(),
                            email: me.email.clone(),
                            saved_at: crate::backend::now_unix(),
                        };
                        this.store.set(sess.clone());
                        this.boot(sess, cx);
                    }
                    Err(_) => {
                        // stay on demo seeds; conn dot already Offline
                        cx.notify();
                    }
                }
            },
        )
        .detach();
    }
    /// Called once by the shell with a live session: loads profile, users,
    /// rooms, then connects realtime and pulls history for the first room.
    pub fn boot(&mut self, sess: StoredSession, cx: &mut Context<Self>) {
        self.sess = Some(sess);
        self.conn = ConnStatus::Connecting;
        self.loading = true;
        cx.notify();
        let api = self.api.clone();
        let access = self.access_token();
        request(
            cx,
            async move {
                let me = api.me(&access).await?;
                let rooms = api.list_rooms(&access).await?;
                let users = api.list_users(&access, "").await.unwrap_or_default();
                Ok::<_, BackendError>((me, rooms, users))
            },
            |this, res: ApiResult<_>, cx| match res {
                Ok((me, rooms, users)) => {
                    this.apply_boot(me, rooms, users, cx);
                    this.connect_ws(cx);
                    // history for the first room
                    if let Some(id) = this.chats.first().map(|c| c.id) {
                        this.select_remote(id, cx);
                    } else {
                        this.loading = false;
                        cx.notify();
                    }
                }
                Err(e) => {
                    this.loading = false;
                    if e.status == Some(401) {
                        this.signed_out(cx);
                    } else {
                        this.conn = ConnStatus::Offline;
                        this.notice = Some(format!("Offline: {e}"));
                        cx.notify();
                    }
                }
            },
        )
        .detach();
    }

    fn apply_boot(
        &mut self,
        me: crate::backend::UserDto,
        rooms: Vec<RoomDto>,
        users: Vec<crate::backend::UserDto>,
        cx: &mut Context<Self>,
    ) {
        for u in &users {
            let label = u
                .display_name
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| u.username.clone());
            self.names.insert(u.id.clone(), label);
        }
        self.me = Some(me);
        self.chats.clear();
        self.room_server.clear();
        for r in rooms {
            let id = self.next_id;
            self.next_id += 1;
            let name = match (&r.name, r.is_dm) {
                (Some(n), _) if !n.is_empty() => n.clone(),
                _ => self.peer_name(&r),
            };
            let kind = if r.is_dm {
                ChatKind::Dm
            } else {
                ChatKind::Group
            };
            self.room_server.insert(id, r.id.clone());
            self.chats.push(Chat {
                id,
                name: name.clone(),
                subtitle: format!("{} members", r.member_ids.len()),
                kind,
                initials: sender_initials(&name),
                color: sender_color(&name),
                unread: 0,
                fav: false,
                archived: false,
                chat_pinned: false,
                last_time: String::new(),
                messages: vec![],
                server_id: Some(r.id),
            });
        }
        if let Some(first) = self.chats.first().map(|c| c.id) {
            self.active_id = first;
        }
        self.loading = false;
        cx.notify();
    }

    fn peer_name(&self, r: &RoomDto) -> String {
        let me = self.me.as_ref().map(|m| m.id.as_str()).unwrap_or("");
        r.member_ids
            .iter()
            .find(|id| id.as_str() != me)
            .map(|id| self.display_name_of(id))
            .unwrap_or_else(|| "Direct message".to_string())
    }

    // ---------- auth plumbing ----------
    fn access_token(&self) -> String {
        self.sess
            .as_ref()
            .map(|s| s.access_token.clone())
            .unwrap_or_default()
    }

    /// Run `then` with a guaranteed-fresh token (refresh first if needed).
    /// On refresh failure the session is dead → back to the auth gate.
    pub fn ensure_access(&mut self, cx: &mut Context<Self>, then: AfterAuth) {
        let need = self
            .sess
            .as_ref()
            .map(|s| !s.access_valid(60))
            .unwrap_or(true);
        if !need {
            then(self, cx);
            return;
        }
        let api = self.api.clone();
        let refresh = self
            .sess
            .as_ref()
            .map(|s| s.refresh_token.clone())
            .unwrap_or_default();
        request(
            cx,
            async move { api.refresh(&refresh).await },
            |this, res: ApiResult<crate::backend::Tokens>, cx| match res {
                Ok(t) => {
                    this.store.update_tokens(
                        &t.access_token,
                        t.access_expires_at,
                        &t.refresh_token,
                        t.refresh_expires_at,
                    );
                    this.sess = this.store.get().cloned();
                    if this.sess.is_some() {
                        then(this, cx);
                    } else {
                        this.signed_out(cx);
                    }
                }
                Err(_) => this.signed_out(cx),
            },
        )
        .detach();
    }

    fn signed_out(&mut self, cx: &mut Context<Self>) {
        self.store.clear();
        self.sess = None;
        self.ws_tx = None;
        self.conn = ConnStatus::Offline;
        cx.emit(ChatEvent::SignedOut);
        cx.notify();
    }

    fn authed<Fut, T>(
        &mut self,
        cx: &mut Context<Self>,
        op: impl FnOnce(ApiClient, String) -> Fut + Send + 'static,
        done: impl FnOnce(&mut Self, ApiResult<T>, &mut Context<Self>) + 'static,
    ) where
        Fut: std::future::Future<Output = ApiResult<T>> + Send + 'static,
        T: Send + 'static,
    {
        self.ensure_access(
            cx,
            Box::new(move |this, cx| {
                let api = this.api.clone();
                let access = this.access_token();
                request(cx, async move { op(api, access).await }, done).detach();
            }),
        );
    }

    // ---------- rooms / history ----------
    pub fn reload_rooms(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();
        self.authed(
            cx,
            move |api, access| async move { api.list_rooms(&access).await },
            |this, res: ApiResult<Vec<RoomDto>>, cx| {
                this.loading = false;
                match res {
                    Ok(rooms) => {
                        let users: Vec<crate::backend::UserDto> = vec![];
                        let me = match this.me.clone() {
                            Some(m) => m,
                            None => {
                                this.notice = Some("Profile missing — re-login.".into());
                                cx.notify();
                                return;
                            }
                        };
                        // preserve open messages for rooms still present
                        let mut old_msgs = std::collections::HashMap::new();
                        for c in this.chats.drain(..) {
                            if let Some(hex) = c.server_id {
                                old_msgs.insert(hex, c.messages);
                            }
                        }
                        this.room_server.clear();
                        for r in rooms {
                            let id = this.next_id;
                            this.next_id += 1;
                            let name = match (&r.name, r.is_dm) {
                                (Some(n), _) if !n.is_empty() => n.clone(),
                                _ => this.peer_name(&r),
                            };
                            let kind = if r.is_dm {
                                ChatKind::Dm
                            } else {
                                ChatKind::Group
                            };
                            let messages = old_msgs.remove(&r.id).unwrap_or_default();
                            this.room_server.insert(id, r.id.clone());
                            this.chats.push(Chat {
                                id,
                                name: name.clone(),
                                subtitle: format!("{} members", r.member_ids.len()),
                                kind,
                                initials: sender_initials(&name),
                                color: sender_color(&name),
                                unread: 0,
                                fav: false,
                                archived: false,
                                chat_pinned: false,
                                last_time: String::new(),
                                messages,
                                server_id: Some(r.id),
                            });
                        }
                        let _ = (me, users);
                        if this.active_pos().is_none()
                            && let Some(first) = this.chats.first().map(|c| c.id)
                        {
                            this.active_id = first;
                        }
                        cx.notify();
                    }
                    Err(e) => {
                        if e.status == Some(401) {
                            this.signed_out(cx);
                        } else {
                            this.notice = Some(format!("Rooms failed: {e}"));
                            cx.notify();
                        }
                    }
                }
            },
        );
    }

    /// Select a room: switch view, (re)join on WS, pull history when empty.
    pub fn select_remote(&mut self, id: usize, cx: &mut Context<Self>) {
        let prev_hex = self.room_server.get(&self.active_id).cloned();
        self.active_id = id;
        self.close_menus();
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == id) {
            c.unread = 0;
        }
        let hex = self.room_server.get(&id).cloned();
        if let (Some(tx), Some(h)) = (self.ws_tx.clone(), hex.clone())
            && prev_hex.as_deref() != Some(h.as_str())
        {
            if let Some(p) = prev_hex {
                let _ = tx.send(WsCmd::Leave(p));
            }
            let _ = tx.send(WsCmd::Join(h));
        }
        let need_history = hex.is_some()
            && self
                .chats
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.messages.is_empty())
                .unwrap_or(false);
        if need_history {
            self.fetch_history(id, cx);
        }
        cx.notify();
    }

    fn fetch_history(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(hex) = self.room_server.get(&id).cloned() else {
            return;
        };
        self.authed(
            cx,
            move |api, access| async move { api.history(&access, &hex, 30, None).await },
            move             |this, res: ApiResult<Vec<MessageDto>>, cx| match res {
                Ok(msgs) => {
                    for m in msgs {
                        this.insert_server_msg(&m, cx);
                    }
                    cx.notify();
                }
                Err(e) => {
                    if e.status == Some(401) {
                        this.signed_out(cx);
                    } else {
                        this.notice = Some(format!("History failed: {e}"));
                        cx.notify();
                    }
                }
            },
        );
    }

    fn insert_server_msg(&mut self, m: &MessageDto, cx: &mut Context<Self>) {
        if !self.known_msgs.insert(m.id.clone()) {
            return; // already shown (e.g. own POST echo over WS)
        }
        let Some((&local, _)) = self.room_server.iter().find(|(_, h)| *h == &m.room_id) else {
            return;
        };
        // Watermark-flagged content: placeholder + fail-closed pipeline
        // (verify → register → decrypt). Plaintext never renders here.
        if let Some(hit) = detect_sealed(m) {
            self.insert_sealed(m, local, hit.sealed, hit.sender_dsa_pub, cx);
            return;
        }
        let mine = m.sender_id == self.my_id();
        let sender = if mine {
            self.my_name()
        } else {
            self.display_name_of(&m.sender_id)
        };
        let id = self.next_msg;
        self.next_msg += 1;
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == local) {
            c.messages.push(Message {
                id,
                sender,
                link: extract_link(&m.body),
                text: m.body.clone(),
                time: short_time(&m.created_at),
                mine,
                date: "Today".to_string(),
                kind: kind_from_dto(m),
                reactions: m
                    .reactions
                    .iter()
                    .map(|r| r.emoji.clone())
                    .collect(),
                reply_to: m.reply_to.clone(),
                ticks: if mine {
                    MessageStatus::Read
                } else {
                    MessageStatus::Sent
                },
                deleted: m.deleted,
                seal: None,
                server_id: Some(m.id.clone()),
                attachment: None,
            });
            c.last_time = short_time(&m.created_at);
            if local != self.active_id {
                c.unread += 1;
            }
        }
    }

    /// Insert a `⏳ Verifying watermark…` placeholder and kick off the
    /// fail-closed receive pipeline on the network runtime.
    fn insert_sealed(
        &mut self,
        m: &MessageDto,
        local: usize,
        sealed: Vec<u8>,
        sender_pub: Option<Vec<u8>>,
        cx: &mut Context<Self>,
    ) {
        let mine = m.sender_id == self.my_id();
        let sender = if mine {
            self.my_name()
        } else {
            self.display_name_of(&m.sender_id)
        };
        let local_id = self.next_msg;
        self.next_msg += 1;
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == local) {
            c.messages.push(Message {
                id: local_id,
                sender,
                link: None,
                text: "⏳ Verifying watermark…".to_string(),
                time: short_time(&m.created_at),
                mine,
                date: "Today".to_string(),
                kind: MessageKind::Text,
                reactions: vec![],
                reply_to: m.reply_to.clone(),
                ticks: MessageStatus::Sent,
                deleted: false,
                seal: Some(SealState::Pending),
                server_id: Some(m.id.clone()),
                attachment: None,
            });
            c.last_time = short_time(&m.created_at);
            if local != self.active_id {
                c.unread += 1;
            }
        }
        self.sealed_retry.insert(
            local_id,
            StoredSeal {
                server_id: m.id.clone(),
                sealed,
                sender_pub,
            },
        );
        self.start_sealed_flow(local_id, cx);
    }

    /// Explicit user retry for a blocked sealed message. The gateway dedupes
    /// on `watermark`, so resubmission never double-commits.
    pub fn retry_sealed(&mut self, local_id: usize, cx: &mut Context<Self>) {
        if !self.sealed_retry.contains_key(&local_id) {
            return;
        }
        if let Some(msg) = self.find_msg_mut(local_id) {
            msg.seal = Some(SealState::Pending);
            msg.text = "⏳ Verifying watermark…".to_string();
            msg.link = None;
        }
        cx.notify();
        self.start_sealed_flow(local_id, cx);
    }

    /// Run permit → register → decrypt for a retained sealed payload with a
    /// guaranteed-fresh access token.
    fn start_sealed_flow(&mut self, local_id: usize, cx: &mut Context<Self>) {
        let Some(stored) = self.sealed_retry.get(&local_id).cloned() else {
            return;
        };
        let username = self.my_username();
        if username.is_empty() {
            self.apply_sealed_result(
                local_id,
                Err("Sign in to verify watermarked content.".to_string()),
                cx,
            );
            return;
        }
        let gateway = self.gateway.clone();
        let ca = self.ca.clone();
        let device = self.device.clone();
        let auth_server = self.auth_server.clone();
        self.ensure_access(
            cx,
            Box::new(move |this, cx| {
                let access = this.access_token();
                request(
                    cx,
                    async move {
                        run_sealed_receive(
                            gateway,
                            ca,
                            access,
                            device,
                            username,
                            auth_server,
                            stored.server_id.clone(),
                            stored.sealed.clone(),
                            stored.sender_pub.clone(),
                        )
                        .await
                    },
                    move |this, res: Result<SealedDone, String>, cx| {
                        this.apply_sealed_result(local_id, res, cx);
                    },
                )
                .detach();
            }),
        );
    }

    /// Apply the pipeline outcome on the GPUI thread: decrypted text + proof
    /// badge, or a blocked tombstone (plaintext never shown on failure).
    fn apply_sealed_result(
        &mut self,
        local_id: usize,
        res: Result<SealedDone, String>,
        cx: &mut Context<Self>,
    ) {
        match res {
            Ok(done) => {
                self.sealed_retry.remove(&local_id);
                if let Some(msg) = self.find_msg_mut(local_id) {
                    msg.link = extract_link(&done.text);
                    msg.text = done.text.clone();
                    msg.time = "now".to_string();
                    msg.seal = Some(SealState::Done {
                        watermark: done.watermark,
                        status: done.status,
                        block_index: done.block_index,
                    });
                    msg.ticks = MessageStatus::Read;
                }
                cx.notify();
            }
            Err(reason) => {
                if let Some(msg) = self.find_msg_mut(local_id) {
                    msg.text = "🚫 Watermark blocked — content withheld.".to_string();
                    msg.link = None;
                    msg.seal = Some(SealState::Blocked { reason });
                }
                cx.notify();
            }
        }
    }

    // ---------- send ----------
    /// Optimistic send: local bubble now, POST in background, notice on error.
    pub fn send_remote(
        &mut self,
        text: String,
        reply_to: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let id = self.active_id;
        let Some(hex) = self.room_server.get(&id).cloned() else {
            // local-only chat: keep the mock behavior
            self.push_message(id, "You", text, true);
            cx.notify();
            return;
        };
        let local_id = self.next_msg;
        self.next_msg += 1;
        let my_name = self.my_name();
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == id) {
            c.messages.push(Message {
                id: local_id,
                sender: my_name,
                link: extract_link(&text),
                text: text.clone(),
                time: "now".to_string(),
                mine: true,
                date: "Today".to_string(),
                kind: MessageKind::Text,
                reactions: vec![],
                reply_to: reply_to.clone(),
                ticks: MessageStatus::Sent,
                deleted: false,
                seal: None,
                server_id: None,
                attachment: None,
            });
            c.last_time = "now".to_string();
        }
        cx.notify();
        self.authed(
            cx,
            move |api, access| async move {
                api.post_message(&access, &hex, &text, None, None, reply_to.as_deref())
                    .await
            },
            move |this, res: ApiResult<MessageDto>, cx| match res {
                Ok(m) => {
                    this.known_msgs.insert(m.id.clone());
                    if let Some(msg) = this.find_msg_mut(local_id) {
                        msg.server_id = Some(m.id);
                        msg.ticks = MessageStatus::Delivered;
                    }
                    cx.notify();
                }
                Err(e) => {
                    if e.status == Some(401) {
                        this.signed_out(cx);
                    } else {
                        this.notice = Some(format!("Send failed: {e}"));
                        cx.notify();
                    }
                }
            },
        );
    }

    // ---------- realtime ----------
    pub fn connect_ws(&mut self, cx: &mut Context<Self>) {
        let token = self.access_token();
        if token.is_empty() {
            return;
        }
        let (ev_tx, ev_rx) = tmpsc::unbounded_channel();
        let tx = start_ws(
            &crate::backend::net_handle(),
            ws_url_for(self.api.base()),
            token,
            ev_tx,
        );
        // join the active room once connected (supervisor re-joins on reconnect)
        if let Some(hex) = self.room_server.get(&self.active_id).cloned() {
            let _ = tx.send(WsCmd::Join(hex));
        }
        self.ws_tx = Some(tx);
        let rx = Arc::new(Mutex::new(ev_rx));
        cx.spawn(async move |weak, cx| {
            let executor = cx.background_executor().clone();
            loop {
                let rx2 = rx.clone();
                let ev = executor
                    .spawn(async move { rx2.lock().unwrap().blocking_recv() })
                    .await;
                match ev {
                    Some(e) => {
                        if weak.update(cx, |v, cx| v.apply_event(e, cx)).is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        })
        .detach();
    }

    fn apply_event(&mut self, ev: WsEvent, cx: &mut Context<Self>) {
        match ev {
            WsEvent::Connected => {
                self.conn = ConnStatus::Online;
                cx.notify();
            }
            WsEvent::Disconnected { reason } => {
                self.conn = ConnStatus::Offline;
                self.notice = Some(format!("Realtime reconnecting ({reason})"));
                cx.notify();
            }
            WsEvent::Welcome { .. } => {}
            WsEvent::Ack { .. } => {}
            WsEvent::Error { detail } => {
                self.notice = Some(detail);
                cx.notify();
            }
            WsEvent::Presence { room_id, detail } => {
                self.presence.insert(room_id, detail);
                cx.notify();
            }
            WsEvent::Typing { room_id, sender_id } => {
                let name = self.display_name_of(&sender_id);
                self.typing.insert(room_id, (name, now_unix()));
                cx.notify();
            }
            WsEvent::ChatMessage {
                room_id,
                message_id,
                sender_id,
                body,
                kind,
                metadata,
                reply_to,
            } => {
                if !self.room_server.values().any(|h| h == &room_id) {
                    // message for a room we haven't listed (e.g. added
                    // elsewhere) — refresh the room list.
                    self.reload_rooms(cx);
                    return;
                }
                if let Some(ref mid) = message_id {
                    if self.known_msgs.contains(mid) {
                        return; // own POST echo
                    }
                    self.known_msgs.insert(mid.clone());
                }
                let m = MessageDto {
                    id: message_id.unwrap_or_default(),
                    room_id,
                    sender_id,
                    body,
                    created_at: String::new(),
                    kind: kind.unwrap_or_else(|| "text".to_string()),
                    metadata,
                    reply_to,
                    edited_at: None,
                    deleted: false,
                    reactions: vec![],
                    poll: None,
                    pinned: false,
                };
                self.insert_server_msg(&m, cx);
                cx.notify();
            }
        }
    }

    /// Subtitle line: typing > presence > member count.
    pub fn live_subtitle(&self, chat: &Chat) -> String {
        let now = now_unix();
        if let Some(hex) = self.room_server.get(&chat.id) {
            if let Some((who, ts)) = self.typing.get(hex)
                && now - ts < 6
            {
                return format!("{who} is typing…");
            }
            if let Some(p) = self.presence.get(hex) {
                return p.clone();
            }
        }
        chat.subtitle.clone()
    }

    pub fn conn_label(&self) -> &'static str {
        match self.conn {
            ConnStatus::Online => "● online",
            ConnStatus::Connecting => "◌ connecting…",
            ConnStatus::Offline => "○ offline",
        }
    }

    // ---------- logout ----------
    pub fn do_logout(&mut self, all: bool, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let access = self.access_token();
        let refresh = self
            .sess
            .as_ref()
            .map(|s| s.refresh_token.clone())
            .unwrap_or_default();
        request(
            cx,
            async move {
                if all {
                    api.logout_all(&access).await?;
                } else {
                    api.logout(&access, &refresh).await?;
                }
                Ok::<_, BackendError>(())
            },
            |this, _: ApiResult<()>, cx| {
                // local sign-out regardless of server reply
                this.signed_out(cx);
            },
        )
        .detach();
        // optimistic: clear immediately so UI reacts even if net hangs
        self.store.clear();
    }

    // ---------- users / new chat ----------
    pub fn search_users(&mut self, cx: &mut Context<Self>) {
        let q = self.user_search.read(cx).value().trim().to_string();
        self.authed(
            cx,
            move |api, access| async move { api.list_users(&access, &q).await },
            |this, res: ApiResult<Vec<crate::backend::UserDto>>, cx| match res {
                Ok(users) => {
                    for u in &users {
                        let label = u
                            .display_name
                            .clone()
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| u.username.clone());
                        this.names.insert(u.id.clone(), label);
                    }
                    // hide self from results
                    let me = this.my_id();
                    this.user_results = users.into_iter().filter(|u| u.id != me).collect();
                    cx.notify();
                }
                Err(e) => {
                    if e.status == Some(401) {
                        this.signed_out(cx);
                    } else {
                        this.notice = Some(format!("Search failed: {e}"));
                        cx.notify();
                    }
                }
            },
        );
    }

    /// Tap a user in new-chat: DM mode creates/opens the DM immediately,
    /// group mode toggles membership selection.
    pub fn pick_user(&mut self, user_id: String, cx: &mut Context<Self>) {
        if self.new_is_dm {
            self.show_new_chat = false;
            self.create_dm(user_id, cx);
            return;
        }
        if self.new_members.contains(&user_id) {
            self.new_members.retain(|x| x != &user_id);
        } else {
            self.new_members.push(user_id);
        }
        cx.notify();
    }

    fn create_dm(&mut self, user_id: String, cx: &mut Context<Self>) {
        // reuse an existing DM with this peer when present
        let me = self.my_id();
        for c in &self.chats {
            if c.kind == super::model::ChatKind::Dm {
                if let Some(hex) = self.room_server.get(&c.id) {
                    let _ = hex;
                }
                // exact membership check needs a fetch; cheap path: match peer name
                if c.name == self.display_name_of(&user_id) {
                    self.select_remote(c.id, cx);
                    return;
                }
            }
        }
        let _ = me;
        self.authed(
            cx,
            move |api, access| async move { api.create_room(&access, None, true, vec![user_id]).await },
            |this, res: ApiResult<RoomDto>, cx| match res {
                Ok(room) => {
                    this.add_room(room, cx);
                }
                Err(e) => {
                    if e.status == Some(401) {
                        this.signed_out(cx);
                    } else {
                        this.notice = Some(format!("DM failed: {e}"));
                        cx.notify();
                    }
                }
            },
        );
    }

    pub fn create_group(&mut self, cx: &mut Context<Self>) {
        let name = self.room_name.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.notice = Some("Give the group a name.".to_string());
            cx.notify();
            return;
        }
        if self.new_members.is_empty() {
            self.notice = Some("Pick at least one member.".to_string());
            cx.notify();
            return;
        }
        let members = std::mem::take(&mut self.new_members);
        self.show_new_chat = false;
        cx.notify();
        self.authed(
            cx,
            move |api, access| async move {
                api.create_room(&access, Some(&name), false, members).await
            },
            |this, res: ApiResult<RoomDto>, cx| match res {
                Ok(room) => this.add_room(room, cx),
                Err(e) => {
                    if e.status == Some(401) {
                        this.signed_out(cx);
                    } else {
                        this.notice = Some(format!("Create failed: {e}"));
                        cx.notify();
                    }
                }
            },
        );
    }

    fn add_room(&mut self, r: RoomDto, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let name = r
            .name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.peer_name(&r));
        let kind = if r.is_dm {
            ChatKind::Dm
        } else {
            ChatKind::Group
        };
        self.room_server.insert(id, r.id.clone());
        self.chats.insert(
            0,
            Chat {
                id,
                name: name.clone(),
                subtitle: format!("{} members", r.member_ids.len()),
                kind,
                initials: sender_initials(&name),
                color: sender_color(&name),
                unread: 0,
                fav: false,
                archived: false,
                chat_pinned: false,
                last_time: String::new(),
                messages: vec![],
                server_id: Some(r.id),
            },
        );
        self.select_remote(id, cx);
    }

    /// Leave (exit group / delete DM view). Server membership is revoked;
    /// the row drops locally.
    pub fn leave_current(&mut self, cx: &mut Context<Self>) {
        let id = self.active_id;
        let Some(hex) = self.room_server.get(&id).cloned() else {
            self.notice = Some("Local-only chat.".to_string());
            cx.notify();
            return;
        };
        if let Some(tx) = self.ws_tx.clone() {
            let _ = tx.send(WsCmd::Leave(hex.clone()));
        }
        self.authed(
            cx,
            move |api, access| async move { api.leave_room(&access, &hex).await.map(|_| id) },
            |this, res: ApiResult<usize>, cx| match res {
                Ok(left) => {
                    this.chats.retain(|c| c.id != left);
                    this.room_server.remove(&left);
                    if let Some(first) = this.chats.first().map(|c| c.id) {
                        this.select_remote(first, cx);
                    } else {
                        this.active_id = 0;
                        cx.notify();
                    }
                }
                Err(e) => {
                    if e.status == Some(401) {
                        this.signed_out(cx);
                    } else {
                        this.notice = Some(format!("Leave failed: {e}"));
                        cx.notify();
                    }
                }
            },
        );
    }

    // ---------- profile ----------
    pub fn save_profile(&mut self, cx: &mut Context<Self>) {
        let name = self.profile_name.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.notice = Some("Display name can't be empty.".to_string());
            cx.notify();
            return;
        }
        self.authed(
            cx,
            move |api, access| async move { api.update_me(&access, None, Some(&name), None, None).await },
            |this, res: ApiResult<crate::backend::UserDto>, cx| match res {
                Ok(me) => {
                    this.me = Some(me);
                    this.notice = Some("Profile updated.".to_string());
                    cx.notify();
                }
                Err(e) => {
                    if e.status == Some(401) {
                        this.signed_out(cx);
                    } else {
                        this.notice = Some(format!("Profile failed: {e}"));
                        cx.notify();
                    }
                }
            },
        );
    }
}

fn short_time(rfc: &str) -> String {
    if rfc.is_empty() {
        return "now".to_string();
    }
    rfc.chars().take(16).collect::<String>().replace('T', " ")
}

fn meta_str(m: &MessageDto, key: &str) -> String {
    m.metadata
        .as_ref()
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Map a server message kind + metadata onto the local bubble variant.
/// Unknown kinds and malformed metadata degrade to Text (body preserved).
fn kind_from_dto(m: &MessageDto) -> MessageKind {
    match m.kind.as_str() {
        "image" => MessageKind::Image {
            caption: meta_str(m, "caption"),
        },
        "voice" => MessageKind::Voice {
            duration: {
                let d = meta_str(m, "duration");
                if d.is_empty() {
                    "0:00".to_string()
                } else {
                    d
                }
            },
            bars: vec![],
        },
        "document" | "video" => MessageKind::Document {
            name: {
                let n = meta_str(m, "filename");
                if n.is_empty() {
                    meta_str(m, "name")
                } else {
                    n
                }
            },
            size: meta_str(m, "size"),
        },
        "contact" => MessageKind::Contact {
            name: meta_str(m, "name"),
            phone: meta_str(m, "phone"),
        },
        "poll" => match &m.poll {
            Some(p) => MessageKind::Poll {
                question: meta_str(m, "question"),
                options: p.options.iter().map(|o| o.text.clone()).collect(),
                votes: p.options.iter().map(|o| o.votes).collect(),
                my_vote: p.my_vote.map(|v| v as usize),
            },
            None => MessageKind::Text,
        },
        "event" => MessageKind::Event {
            title: meta_str(m, "title"),
            when: meta_str(m, "when"),
        },
        "sticker" => MessageKind::Sticker {
            glyph: meta_str(m, "glyph"),
        },
        _ => MessageKind::Text,
    }
}
