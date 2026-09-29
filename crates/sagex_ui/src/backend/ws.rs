//! Realtime channel: a Tokio task (on the network runtime) owns the
//! WebSocket with reconnect + re-join; UI talks to it via channels only.
//!
//! UI → socket: `UnboundedSender<WsCmd>` (Send from any thread).
//! Socket → UI: `UnboundedSender<WsEvent>` drained on a GPUI background
//! task with `blocking_recv` (never blocks the foreground thread).

use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMsg;

#[derive(Debug, Clone)]
pub enum WsCmd {
    Join(String),
    Leave(String),
    Send {
        room_id: String,
        body: String,
    },
    Typing(String),
    Ping,
    /// Send raw client JSON (power users / tests).
    Raw(String),
    Close,
}

#[derive(Debug, Clone)]
pub enum WsEvent {
    Welcome {
        user_id: String,
    },
    Ack {
        room_id: Option<String>,
        detail: String,
    },
    ChatMessage {
        room_id: String,
        message_id: Option<String>,
        sender_id: String,
        body: String,
    },
    Presence {
        room_id: String,
        detail: String,
    },
    Typing {
        room_id: String,
        sender_id: String,
    },
    Error {
        detail: String,
    },
    Connected,
    Disconnected {
        reason: String,
    },
}

fn parse_event(text: &str) -> Option<WsEvent> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let kind = v.get("type")?.as_str()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(|x| x.to_string());
    Some(match kind {
        "welcome" => WsEvent::Welcome {
            user_id: s("user_id").unwrap_or_default(),
        },
        "ack" => WsEvent::Ack {
            room_id: s("room_id"),
            detail: s("detail").unwrap_or_default(),
        },
        "message" => WsEvent::ChatMessage {
            room_id: s("room_id").unwrap_or_default(),
            message_id: s("message_id"),
            sender_id: s("sender_id").unwrap_or_default(),
            body: s("body").unwrap_or_default(),
        },
        "presence" => WsEvent::Presence {
            room_id: s("room_id").unwrap_or_default(),
            detail: s("detail").unwrap_or_default(),
        },
        "typing" => WsEvent::Typing {
            room_id: s("room_id").unwrap_or_default(),
            sender_id: s("sender_id").unwrap_or_default(),
        },
        "error" => WsEvent::Error {
            detail: s("detail").unwrap_or_else(|| "socket error".into()),
        },
        "pong" => WsEvent::Ack {
            room_id: None,
            detail: "pong".into(),
        },
        _ => return None,
    })
}

fn cmd_json(cmd: &WsCmd) -> Option<String> {
    Some(match cmd {
        WsCmd::Join(room) => serde_json::json!({"type":"join","room_id":room}).to_string(),
        WsCmd::Leave(room) => serde_json::json!({"type":"leave","room_id":room}).to_string(),
        WsCmd::Send { room_id, body } => {
            serde_json::json!({"type":"message","room_id":room_id,"body":body}).to_string()
        }
        WsCmd::Typing(room) => serde_json::json!({"type":"typing","room_id":room}).to_string(),
        WsCmd::Ping => serde_json::json!({"type":"ping"}).to_string(),
        WsCmd::Raw(t) => t.clone(),
        WsCmd::Close => return None,
    })
}

/// Spawn the socket supervisor on the network runtime. Returns the
/// command channel; drop it (or send `Close`) to shut down.
pub fn start_ws(
    rt: &tokio::runtime::Handle,
    ws_url: String,
    token: String,
    events: mpsc::UnboundedSender<WsEvent>,
) -> mpsc::UnboundedSender<WsCmd> {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<WsCmd>();
    rt.spawn(async move {
        let mut joined: Vec<String> = vec![];
        let mut backoff = 1u64;
        'outer: loop {
            let url = format!("{ws_url}?token={token}");
            let (mut sock, _) = match tokio_tungstenite::connect_async(&url).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = events.send(WsEvent::Disconnected { reason: format!("connect failed: {e}") });
                    tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
                    backoff = (backoff * 2).min(30);
                    // Drain commands while offline so a Close still exits.
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        if matches!(cmd, WsCmd::Close) {
                            break 'outer;
                        }
                    }
                    continue;
                }
            };
            backoff = 1;
            let _ = events.send(WsEvent::Connected);
            for room in &joined {
                let msg = serde_json::json!({"type":"join","room_id":room}).to_string();
                if sock.send(WsMsg::Text(msg.into())).await.is_err() {
                    continue 'outer;
                }
            }
            loop {
                tokio::select! {
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            None | Some(WsCmd::Close) => break 'outer,
                            Some(c) => {
                                if let WsCmd::Join(r) = &c
                                    && !joined.contains(r)
                                {
                                    joined.push(r.clone());
                                }
                                if let WsCmd::Leave(r) = &c {
                                    joined.retain(|x| x != r);
                                }
                                if let Some(text) = cmd_json(&c)
                                    && sock.send(WsMsg::Text(text.into())).await.is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }
                    msg = sock.next() => {
                        match msg {
                            Some(Ok(WsMsg::Text(t))) => {
                                if let Some(ev) = parse_event(&t) {
                                    let _ = events.send(ev);
                                }
                            }
                            Some(Ok(WsMsg::Ping(p))) => {
                                let _ = sock.send(WsMsg::Pong(p)).await;
                            }
                            Some(Ok(_)) => {}
                            _ => {
                                let _ = events.send(WsEvent::Disconnected { reason: "connection lost".into() });
                                break;
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(30);
        }
    });
    cmd_tx
}

/// Build the WS URL from the REST base (http→ws, https→wss).
pub fn ws_url_for(base: &str) -> String {
    let b = base.trim_end_matches('/');
    let ws = if let Some(rest) = b.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = b.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        b.to_string()
    };
    format!("{ws}/ws/chat")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_url_mapping() {
        assert_eq!(
            ws_url_for("http://127.0.0.1:8080"),
            "ws://127.0.0.1:8080/ws/chat"
        );
        assert_eq!(
            ws_url_for("https://chat.example.com/"),
            "wss://chat.example.com/ws/chat"
        );
    }

    #[test]
    fn event_parsing() {
        let ev = parse_event(
            r#"{"type":"message","room_id":"r","message_id":"m","sender_id":"u","body":"hi"}"#,
        )
        .unwrap();
        assert!(matches!(ev, WsEvent::ChatMessage { .. }));
        let ev = parse_event(r#"{"type":"welcome","user_id":"u"}"#).unwrap();
        assert!(matches!(ev, WsEvent::Welcome { .. }));
        assert!(parse_event(r#"{"type":"nope"}"#).is_none());
        assert!(parse_event("garbage").is_none());
    }
}
