use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc};

use crate::proto::Envelope;
use tracing::{info, warn};

/// Outbound links to peers: node_id -> sender of Envelopes (JSON-lines).
#[derive(Clone, Default)]
pub struct PeerLinks {
    inner: Arc<RwLock<HashMap<u64, mpsc::UnboundedSender<Envelope>>>>,
}

impl PeerLinks {
    pub async fn insert(&self, id: u64, tx: mpsc::UnboundedSender<Envelope>) {
        self.inner.write().await.insert(id, tx);
    }
    pub async fn remove(&self, id: u64) {
        self.inner.write().await.remove(&id);
    }
    pub async fn send_to(&self, id: u64, env: Envelope) -> bool {
        if let Some(tx) = self.inner.read().await.get(&id).cloned() {
            tx.send(env).is_ok()
        } else {
            false
        }
    }
    pub async fn broadcast(&self, env: Envelope) {
        for tx in self.inner.read().await.values().cloned().collect::<Vec<_>>() {
            let _ = tx.send(env.clone());
        }
    }
    pub async fn connected(&self) -> Vec<u64> {
        self.inner.read().await.keys().cloned().collect()
    }
}

pub async fn write_envelope(w: &mut (impl AsyncWriteExt + Unpin), env: &Envelope) -> anyhow::Result<()> {
    let mut s = serde_json::to_string(env)?;
    s.push('\n');
    w.write_all(s.as_bytes()).await?;
    Ok(())
}

/// Spawn a writer task for one socket; returns the sender to queue envelopes.
pub fn spawn_writer(
    write_half: tokio::net::tcp::OwnedWriteHalf,
) -> mpsc::UnboundedSender<Envelope> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Envelope>();
    tokio::spawn(async move {
        let mut w = write_half;
        while let Some(env) = rx.recv().await {
            match serde_json::to_string(&env) {
                Ok(mut s) => {
                    s.push('\n');
                    if w.write_all(s.as_bytes()).await.is_err() {
                        break;
                    }
                }
                Err(e) => warn!("encode envelope failed: {e}"),
            }
        }
    });
    tx
}

/// Read loop for one socket: parses JSON-lines Envelopes, forwards to handler.
pub async fn read_loop<R>(
    read_half: R,
    peer_label: String,
    mut on_env: impl FnMut(Envelope) -> anyhow::Result<()> + Send + 'static,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(read_half).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<Envelope>(line) {
                    Ok(env) => {
                        if let Err(e) = on_env(env) {
                            warn!("handler error from {peer_label}: {e:#}");
                        }
                    }
                    Err(e) => warn!("bad JSON from {peer_label}: {e}"),
                }
            }
            Ok(None) => break, // EOF
            Err(e) => {
                warn!("read error from {peer_label}: {e}");
                break;
            }
        }
    }
    info!("connection closed: {peer_label}");
}

pub async fn accept_loop(
    listener: TcpListener,
    links: PeerLinks,
    inbound_tx: mpsc::UnboundedSender<InboundFrame>,
    my_id: u64,
) {
    loop {
        match listener.accept().await {
            Ok((sock, addr)) => {
                let links = links.clone();
                let inbound_tx = inbound_tx.clone();
                tokio::spawn(async move {
                    handle_inbound_socket(sock, addr.to_string(), links, inbound_tx, my_id).await;
                });
            }
            Err(e) => {
                warn!("accept error: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
    }
}

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

/// One accepted socket can be either a peer link (Hello first) or a client.
/// We peek the first envelope: Hello -> register as peer link; else client msg.
async fn handle_inbound_socket(
    sock: TcpStream,
    label: String,
    links: PeerLinks,
    inbound_tx: mpsc::UnboundedSender<InboundFrame>,
    my_id: u64,
) {
    let (rh, wh) = sock.into_split();
    let mut lines = BufReader::new(rh).lines();
    // writer for replies / peer sends
    let (reply_tx, mut reply_rx) = mpsc::unbounded_channel::<crate::proto::ResponseEnvelope>();
    let (peer_tx, mut peer_rx) = mpsc::unbounded_channel::<Envelope>();
    // Need first line to classify.
    let first = match lines.next_line().await {
        Ok(Some(l)) => l,
        _ => return,
    };
    let first_trim = first.trim();
    // Try Hello?
    if let Ok(env) = serde_json::from_str::<Envelope>(first_trim) {
        if matches!(env.msg, crate::proto::Message::Hello { .. }) {
            let peer_id = match env.msg {
                crate::proto::Message::Hello { node_id } => node_id,
                _ => unreachable!(),
            };
            if peer_id == my_id {
                return; // self-dial, ignore
            }
            info!("peer connected inbound: {peer_id} via {label}");
            links.insert(peer_id, peer_tx.clone()).await;
            // Send our Hello back so the dialer can register us too.
            let hello_back = Envelope {
                req_id: None,
                from: Some(my_id),
                msg: crate::proto::Message::Hello { node_id: my_id },
            };
            let _ = peer_tx.send(hello_back);
            // Writer mux: peer envelopes.
            let mut w = wh;
            let writer = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        Some(env) = peer_rx.recv() => {
                            if write_envelope(&mut w, &env).await.is_err() { break; }
                        }
                        else => break,
                    }
                }
            });
            // Reader: forward peer envelopes (skip Hello duplicates).
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<Envelope>(line) {
                            Ok(env) => {
                                if matches!(env.msg, crate::proto::Message::Hello { .. }) {
                                    continue;
                                }
                                let _ = inbound_tx.send(InboundFrame {
                                    env,
                                    reply: None,
                                    peer_id: Some(peer_id),
                                });
                            }
                            Err(e) => warn!("bad peer JSON: {e}"),
                        }
                    }
                    _ => break,
                }
            }
            links.remove(peer_id).await;
            writer.abort();
            info!("peer inbound closed: {peer_id}");
            return;
        }
        // Not hello: treat as client first message.
        let reply_tx_c = reply_tx.clone();
        let inbound_tx_c = inbound_tx.clone();
        // dispatch first client envelope
        let _ = inbound_tx_c.send(InboundFrame {
            env,
            reply: Some(reply_tx_c.clone()),
            peer_id: None,
        });
        // writer for client responses
        let w: OwnedWriteHalf = wh;
        let _ = read_remainder_client(lines, w, inbound_tx_c, reply_tx_c, &mut reply_rx).await;
        return;
    }
    // Unparseable first line: ignore.
    warn!("unparseable first line from {label}");
}

async fn read_remainder_client(
    mut lines: tokio::io::Lines<BufReader<OwnedReadHalf>>,
    mut w: OwnedWriteHalf,
    inbound_tx: mpsc::UnboundedSender<InboundFrame>,
    reply_tx: mpsc::UnboundedSender<crate::proto::ResponseEnvelope>,
    reply_rx: &mut mpsc::UnboundedReceiver<crate::proto::ResponseEnvelope>,
) -> anyhow::Result<()> {
    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        let t = l.trim();
                        if t.is_empty() { continue; }
                        match serde_json::from_str::<Envelope>(t) {
                            Ok(env) => {
                                let _ = inbound_tx.send(InboundFrame {
                                    env, reply: Some(reply_tx.clone()), peer_id: None,
                                });
                            }
                            Err(e) => {
                                let err = crate::proto::ResponseEnvelope {
                                    req_id: None,
                                    resp: crate::proto::Response::Error { error: format!("bad envelope: {e}") },
                                };
                                let mut s = serde_json::to_string(&err).unwrap();
                                s.push('\n');
                                w.write_all(s.as_bytes()).await?;
                            }
                        }
                    }
                    _ => break,
                }
            }
            Some(resp) = reply_rx.recv() => {
                let mut s = serde_json::to_string(&resp).unwrap();
                s.push('\n');
                if w.write_all(s.as_bytes()).await.is_err() { break; }
            }
        }
    }
    Ok(())
}

/// Dial peers with retry; on connect, Hello-handshake and register link.
pub fn spawn_dialers(
    my_id: u64,
    peer_addrs: HashMap<u64, String>,
    links: PeerLinks,
    inbound_tx: mpsc::UnboundedSender<InboundFrame>,
) {
    for (pid, addr) in peer_addrs {
        if pid == my_id {
            continue;
        }
        let links = links.clone();
        let inbound_tx = inbound_tx.clone();
        tokio::spawn(async move {
            loop {
                match TcpStream::connect(&addr).await {
                    Ok(sock) => {
                        let (rh, wh) = sock.into_split();
                        let (peer_tx, mut peer_rx) = mpsc::unbounded_channel::<Envelope>();
                        // Send Hello first.
                        let hello = Envelope {
                            req_id: None,
                            from: Some(my_id),
                            msg: crate::proto::Message::Hello { node_id: my_id },
                        };
                        let mut w = wh;
                        if write_envelope(&mut w, &hello).await.is_err() {
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            continue;
                        }
                        links.insert(pid, peer_tx.clone()).await;
                        info!("peer connected outbound: {pid} @ {addr}");
                        // writer
                        let writer = tokio::spawn(async move {
                            while let Some(env) = peer_rx.recv().await {
                                if write_envelope(&mut w, &env).await.is_err() {
                                    break;
                                }
                            }
                        });
                        // reader
                        let mut lines = BufReader::new(rh).lines();
                        loop {
                            match lines.next_line().await {
                                Ok(Some(line)) => {
                                    let t = line.trim();
                                    if t.is_empty() {
                                        continue;
                                    }
                                    match serde_json::from_str::<Envelope>(t) {
                                        Ok(env) => {
                                            if matches!(
                                                env.msg,
                                                crate::proto::Message::Hello { .. }
                                            ) {
                                                continue;
                                            }
                                            let _ = inbound_tx.send(InboundFrame {
                                                env,
                                                reply: None,
                                                peer_id: Some(pid),
                                            });
                                        }
                                        Err(e) => warn!("bad peer JSON: {e}"),
                                    }
                                }
                                _ => break,
                            }
                        }
                        links.remove(pid).await;
                        writer.abort();
                        warn!("peer link lost: {pid}, redialing…");
                    }
                    Err(_) => {
                        // peer not up yet; quiet retry
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    }
}

/// Message into the single-threaded consensus event loop.
pub struct InboundFrame {
    pub env: Envelope,
    pub reply: Option<mpsc::UnboundedSender<crate::proto::ResponseEnvelope>>,
    pub peer_id: Option<u64>,
}
