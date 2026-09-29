//! WebSocket chat: `GET /ws/chat?token=<access_jwt>`.
//!
//! JSON protocol — client -> server:
//! ```json
//! {"type":"join","room_id":"..."} | {"type":"leave","room_id":"..."}
//! | {"type":"message","room_id":"...","body":"hello"}
//! | {"type":"typing","room_id":"..."} | {"type":"ping"}
//! ```
//! server -> client: `{"type":"welcome"|"ack"|"message"|"presence"|"typing"|"error"|"pong", ...}`

use std::{
    collections::HashSet,
    sync::atomic::{AtomicU64, Ordering},
};

use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use dashmap::{DashMap, DashSet};
use futures::{SinkExt, StreamExt};
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{
    error::AppError,
    models::Message,
    state::{AppState, C_MESSAGES, C_ROOMS},
};

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: String,
}

#[derive(Debug, Deserialize)]
struct ClientMsg {
    #[serde(rename = "type")]
    kind: String,
    room_id: Option<String>,
    body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ServerMsg {
    #[serde(rename = "type")]
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sender_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

pub struct ChatHub {
    next_id: AtomicU64,
    /// socket_id -> sender channel
    peers: DashMap<u64, mpsc::UnboundedSender<String>>,
    /// socket_id -> user id hex
    owners: DashMap<u64, String>,
    /// room_id hex -> socket ids
    rooms: DashMap<String, DashSet<u64>>,
}

impl ChatHub {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            peers: DashMap::new(),
            owners: DashMap::new(),
            rooms: DashMap::new(),
        }
    }

    fn register(&self, user_id: &str, tx: mpsc::UnboundedSender<String>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.peers.insert(id, tx);
        self.owners.insert(id, user_id.to_string());
        id
    }

    fn unregister(&self, sid: u64) {
        self.peers.remove(&sid);
        self.owners.remove(&sid);
        for room in self.rooms.iter() {
            room.value().remove(&sid);
        }
    }

    fn join(&self, room_id: &str, sid: u64) {
        self.rooms
            .entry(room_id.to_string())
            .or_default()
            .insert(sid);
    }

    fn leave(&self, room_id: &str, sid: u64) {
        if let Some(set) = self.rooms.get(room_id) {
            set.remove(&sid);
        }
    }

    fn send_to(&self, sid: u64, msg: &ServerMsg) {
        if let Some(tx) = self.peers.get(&sid) {
            if let Ok(text) = serde_json::to_string(msg) {
                let _ = tx.send(text);
            }
        }
    }

    fn broadcast_room(&self, room_id: &str, msg: &ServerMsg, except: Option<u64>) {
        let text = match serde_json::to_string(msg) {
            Ok(t) => t,
            Err(_) => return,
        };
        if let Some(set) = self.rooms.get(room_id) {
            // Snapshot to avoid holding the lock while sending.
            let members: Vec<u64> = set.iter().map(|s| *s).collect();
            for sid in members {
                if Some(sid) == except {
                    continue;
                }
                if let Some(tx) = self.peers.get(&sid) {
                    let _ = tx.send(text.clone());
                }
            }
        }
    }

    /// Broadcast a pre-serialized JSON text frame (used by REST POST).
    pub fn broadcast_text(&self, room_id: &str, text: String) {
        if let Some(set) = self.rooms.get(room_id) {
            let members: Vec<u64> = set.iter().map(|s| *s).collect();
            for sid in members {
                if let Some(tx) = self.peers.get(&sid) {
                    let _ = tx.send(text.clone());
                }
            }
        }
    }

    fn room_members_online(&self, room_id: &str) -> Vec<String> {
        let mut out = HashSet::new();
        if let Some(set) = self.rooms.get(room_id) {
            for sid in set.iter() {
                if let Some(uid) = self.owners.get(&*sid) {
                    out.insert(uid.clone());
                }
            }
        }
        let mut v: Vec<String> = out.into_iter().collect();
        v.sort();
        v
    }
}

impl Default for ChatHub {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(q): Query<WsQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let claims = state.jose.verify(&q.token, "access")?;
    let user_hex = claims.sub.clone();
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, user_hex)))
}

async fn is_member(state: &AppState, room_hex: &str, user: &ObjectId) -> bool {
    let Ok(room_id) = ObjectId::parse_str(room_hex) else {
        return false;
    };
    let rooms = state.db.collection::<crate::models::Room>(C_ROOMS);
    matches!(
        rooms
            .find_one(doc! { "_id": room_id, "member_ids": user }, None)
            .await,
        Ok(Some(_))
    )
}

async fn handle_socket(socket: WebSocket, state: AppState, user_hex: String) {
    let user_id = match ObjectId::parse_str(&user_hex) {
        Ok(id) => id,
        Err(_) => return,
    };
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let sid = state.hub.register(&user_hex, tx);

    state.hub.send_to(
        sid,
        &ServerMsg {
            kind: "welcome".into(),
            room_id: None,
            body: None,
            sender_id: None,
            message_id: None,
            user_id: Some(user_hex.clone()),
            detail: Some("connected to sagex-chatsrv".into()),
        },
    );

    // Forward hub -> websocket.
    let fwd = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sender.send(WsMessage::Text(text)).await.is_err() {
                break;
            }
        }
    });

    // Client -> hub.
    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        let text = match msg {
            WsMessage::Text(t) => t,
            WsMessage::Close(_) => break,
            WsMessage::Ping(p) => {
                // Best-effort pong via hub channel is awkward; ignore.
                let _ = p;
                continue;
            }
            _ => continue,
        };
        let cmsg: ClientMsg = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(_) => {
                state.hub.send_to(
                    sid,
                    &ServerMsg {
                        kind: "error".into(),
                        room_id: None,
                        body: None,
                        sender_id: None,
                        message_id: None,
                        user_id: None,
                        detail: Some("invalid JSON message".into()),
                    },
                );
                continue;
            }
        };
        handle_client_msg(&state, sid, &user_id, &user_hex, cmsg).await;
    }

    state.hub.unregister(sid);
    fwd.abort();
}

async fn handle_client_msg(
    state: &AppState,
    sid: u64,
    user_id: &ObjectId,
    user_hex: &str,
    cmsg: ClientMsg,
) {
    let room_id_opt = cmsg.room_id.clone();
    let ack = |detail: &str| ServerMsg {
        kind: "ack".into(),
        room_id: room_id_opt.clone(),
        body: None,
        sender_id: None,
        message_id: None,
        user_id: None,
        detail: Some(detail.into()),
    };
    let err = |detail: &str| ServerMsg {
        kind: "error".into(),
        room_id: room_id_opt.clone(),
        body: None,
        sender_id: None,
        message_id: None,
        user_id: None,
        detail: Some(detail.into()),
    };
    match cmsg.kind.as_str() {
        "ping" => state.hub.send_to(
            sid,
            &ServerMsg {
                kind: "pong".into(),
                room_id: None,
                body: None,
                sender_id: None,
                message_id: None,
                user_id: None,
                detail: None,
            },
        ),
        "join" => {
            let Some(room_hex) = cmsg.room_id else {
                state.hub.send_to(sid, &err("room_id required"));
                return;
            };
            if !is_member(state, &room_hex, user_id).await {
                state.hub.send_to(sid, &err("not a room member"));
                return;
            }
            state.hub.join(&room_hex, sid);
            state.hub.send_to(sid, &ack("joined"));
            let online = state.hub.room_members_online(&room_hex);
            state.hub.broadcast_room(
                &room_hex,
                &ServerMsg {
                    kind: "presence".into(),
                    room_id: Some(room_hex.clone()),
                    body: None,
                    sender_id: None,
                    message_id: None,
                    user_id: None,
                    detail: Some(format!("online: {}", online.join(","))),
                },
                None,
            );
        }
        "leave" => {
            let Some(room_hex) = cmsg.room_id else {
                state.hub.send_to(sid, &err("room_id required"));
                return;
            };
            state.hub.leave(&room_hex, sid);
            state.hub.send_to(sid, &ack("left"));
        }
        "typing" => {
            let Some(room_hex) = cmsg.room_id else {
                state.hub.send_to(sid, &err("room_id required"));
                return;
            };
            if !is_member(state, &room_hex, user_id).await {
                state.hub.send_to(sid, &err("not a room member"));
                return;
            }
            state.hub.broadcast_room(
                &room_hex,
                &ServerMsg {
                    kind: "typing".into(),
                    room_id: Some(room_hex.clone()),
                    body: None,
                    sender_id: Some(user_hex.to_string()),
                    message_id: None,
                    user_id: None,
                    detail: None,
                },
                Some(sid),
            );
        }
        "message" => {
            let (Some(room_hex), Some(body)) = (cmsg.room_id.clone(), cmsg.body) else {
                state.hub.send_to(sid, &err("room_id and body required"));
                return;
            };
            let body = body.trim().to_string();
            if body.is_empty() || body.len() > 4000 {
                state.hub.send_to(sid, &err("body must be 1..4000 chars"));
                return;
            }
            let Ok(room_id) = ObjectId::parse_str(&room_hex) else {
                state.hub.send_to(sid, &err("bad room_id"));
                return;
            };
            if !is_member(state, &room_hex, user_id).await {
                state.hub.send_to(sid, &err("not a room member"));
                return;
            }
            let now = DateTime::now();
            let msg = Message {
                id: None,
                room_id,
                sender_id: *user_id,
                body: body.clone(),
                created_at: now,
            };
            let coll = state.db.collection::<Message>(C_MESSAGES);
            match coll.insert_one(msg, None).await {
                Ok(res) => {
                    let mid = res.inserted_id.as_object_id().map(|o| o.to_hex());
                    state.hub.broadcast_room(
                        &room_hex,
                        &ServerMsg {
                            kind: "message".into(),
                            room_id: Some(room_hex.clone()),
                            body: Some(body),
                            sender_id: Some(user_hex.to_string()),
                            message_id: mid,
                            user_id: None,
                            detail: None,
                        },
                        None,
                    );
                }
                Err(e) => {
                    tracing::warn!("ws persist failed: {e}");
                    state.hub.send_to(sid, &err("persist failed"));
                }
            }
        }
        other => state
            .hub
            .send_to(sid, &err(&format!("unknown type: {other}"))),
    }
}
