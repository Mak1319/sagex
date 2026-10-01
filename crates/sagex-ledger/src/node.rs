use std::collections::HashMap;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::NodeConfig;
use crate::identity::Identity;
use crate::model::{Block, DecryptionRecord};
use crate::net::{InboundFrame, PeerLinks};
use crate::pbft::{Pbft, PbftAction};
use crate::proto::{Envelope, Message, Response, ResponseEnvelope};
use crate::store::Store;

type ReplyTx = mpsc::UnboundedSender<ResponseEnvelope>;

pub struct Node {
    cfg: NodeConfig,
    id: Identity,
    store: Store,
    pbft: Pbft,
    links: PeerLinks,
    /// watermark -> waiting client replies (each Submit originator)
    pending: HashMap<String, Vec<(Option<u64>, ReplyTx)>>,
    /// peer pubkeys for vote verification
    peer_keys: HashMap<u64, String>,
    last_commit: std::time::Instant,
}

impl Node {
    pub fn new(cfg: NodeConfig) -> anyhow::Result<Self> {
        let id = Identity::from_keypair_b64(
            cfg.node.id,
            &cfg.node.secret_key,
            &cfg.node.pubkey,
        )?;
        let store = Store::open(&cfg.node.db)?;
        // Resume seq from tip.
        let start_seq = store
            .tip()?
            .map(|t| t.seq + 1)
            .unwrap_or(1);
        let quorum = cfg.quorum();
        let f = cfg.consensus.f;
        let peers_cfg = cfg.peers.clone();
        let my_id = cfg.node.id;
        let leader_fn: Box<dyn Fn(u64) -> u64 + Send + Sync> = Box::new(move |view| {
            let mut ids: Vec<u64> = peers_cfg.iter().map(|p| p.id).collect();
            ids.push(my_id);
            ids.sort_unstable();
            ids[(view as usize) % ids.len()]
        });
        let mut peer_keys = HashMap::new();
        for p in &cfg.peers {
            if !p.pubkey.is_empty() {
                peer_keys.insert(p.id, p.pubkey.clone());
            }
        }
        let node_id = cfg.node.id;
        let node_pubkey = cfg.node.pubkey.clone();
        // Own key: leader sig self-verified implicitly; store for completeness.
        peer_keys.insert(node_id, node_pubkey);

        Ok(Self {
            cfg,
            id,
            store,
            pbft: Pbft::new(node_id, f, quorum, start_seq, leader_fn),
            links: PeerLinks::default(),
            pending: HashMap::new(),
            peer_keys,
            last_commit: std::time::Instant::now(),
        })
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        // Startup chain self-check.
        let n = self.store.verify_chain().unwrap_or(0);
        info!(
            "node {} starting: height blocks={} view={} quorum={}",
            self.cfg.node.id,
            n,
            self.pbft.view,
            self.pbft.quorum
        );
        let listener = TcpListener::bind(&self.cfg.node.listen).await?;
        info!("node {} listening on {}", self.cfg.node.id, self.cfg.node.listen);
        let (tx, rx) = mpsc::unbounded_channel::<InboundFrame>();
        // Accept + dial concurrently.
        let links = self.links.clone();
        let my_id = self.cfg.node.id;
        tokio::spawn(crate::net::accept_loop(listener, links.clone(), tx.clone(), my_id));
        crate::net::spawn_dialers(my_id, self.cfg.peer_addrs(), links.clone(), tx.clone());

        self.event_loop(rx).await
    }

    async fn event_loop(
        &mut self,
        mut rx: mpsc::UnboundedReceiver<InboundFrame>,
    ) -> anyhow::Result<()> {
        let timeout = std::time::Duration::from_millis(self.cfg.consensus.view_timeout_ms.max(1000));
        loop {
            tokio::select! {
                Some(frame) = rx.recv() => {
                    self.handle_frame(frame).await;
                }
                _ = tokio::time::sleep(timeout) => {
                    self.maybe_view_change().await;
                }
            }
        }
    }

    fn verify_peer(&self, node_id: u64, msg: &[u8], sig: &str) -> anyhow::Result<()> {
        if node_id == self.cfg.node.id {
            // Self messages were produced locally; trust but still verify cryptographically.
            Identity::verify_with_pubkey(&self.cfg.node.pubkey, msg, sig)
        } else if let Some(pk) = self.peer_keys.get(&node_id) {
            Identity::verify_with_pubkey(pk, msg, sig)
        } else {
            anyhow::bail!("unknown peer {node_id} (missing pubkey)")
        }
    }

    fn verify_leader(&self, leader: u64, msg: &[u8], sig: &str) -> anyhow::Result<()> {
        self.verify_peer(leader, msg, sig)
    }

    async fn exec_actions(&mut self, actions: Vec<PbftAction>) {
        for a in actions {
            match a {
                PbftAction::Broadcast(env) => {
                    let kind = match &env.msg {
                        Message::PrePrepare { .. } => "PrePrepare",
                        Message::Prepare { .. } => "Prepare",
                        Message::Commit { .. } => "Commit",
                        _ => "other",
                    };
                    info!("node {} broadcast {kind}", self.cfg.node.id);
                    self.links.broadcast(env).await;
                }
                PbftAction::SendTo(id, env) => {
                    if id == self.cfg.node.id {
                        // loopback: feed directly
                        let _ = self.handle_peer_envelope(env, None).await;
                    } else if !self.links.send_to(id, env.clone()).await {
                        warn!("no link to peer {id}, dropping msg");
                    }
                }
                PbftAction::Commit(block) => self.commit_block(block).await,
            }
        }
    }

    fn remember_pending(&mut self, record: &DecryptionRecord, req_id: Option<u64>, reply: Option<ReplyTx>) {
        if let Some(r) = reply {
            self.pending
                .entry(record.watermark.clone())
                .or_default()
                .push((req_id, r));
        }
    }

    async fn handle_frame(&mut self, frame: InboundFrame) {
        let InboundFrame { env, reply, peer_id } = frame;
        let req_id = env.req_id;
        match env.msg {
            Message::Hello { .. } => {} // handled at net layer
            Message::Submit { record } => {
                self.on_submit(record, req_id, reply, false).await;
            }
            Message::ForwardSubmit { record, origin: _ } => {
                self.on_submit(record, req_id, reply, true).await;
            }
            Message::PrePrepare { view, seq, block, sig } => {
                // Only leader may send PrePrepare. Verify signature BEFORE
                // touching pbft mutably (avoids borrow conflict).
                let leader = self.pbft.leader();
                let digest = block.header.hash.clone();
                let vres = self.verify_leader(
                    leader,
                    &Identity::preprepare_bytes(view, seq, &digest),
                    &sig,
                );
                if let Err(e) = vres {
                    warn!("PrePrepare bad sig: {e:#}");
                    return;
                }
                let tip = self.store.tip().ok().flatten();
                // Clone identity fields needed: sign inside pbft needs &Identity;
                // borrow self.id immutably while pbft mutably is OK via split.
                let (pbft, id) = (&mut self.pbft, &self.id);
                let res = pbft.on_preprepare(
                    view,
                    seq,
                    block,
                    &sig,
                    tip.as_ref(),
                    id,
                    |_, _| Ok(()),
                );
                match res {
                    Ok(acts) => self.exec_actions(acts).await,
                    Err(e) => warn!("PrePrepare rejected: {e:#}"),
                }
            }
            Message::Prepare { view, seq, digest, node_id, sig } => {
                let vres = self.verify_peer(
                    node_id,
                    &Identity::vote_bytes(view, seq, &digest),
                    &sig,
                );
                if let Err(e) = vres {
                    warn!("Prepare bad sig: {e:#}");
                    return;
                }
                let (pbft, id) = (&mut self.pbft, &self.id);
                let res = pbft.on_prepare(
                    view,
                    seq,
                    &digest,
                    node_id,
                    &sig,
                    id,
                    |_, _| Ok(()),
                );
                match res {
                    Ok(acts) => self.exec_actions(acts).await,
                    Err(e) => warn!("Prepare rejected: {e:#}"),
                }
            }
            Message::Commit { view, seq, digest, node_id, sig } => {
                let vres = self.verify_peer(
                    node_id,
                    &Identity::vote_bytes(view, seq, &digest),
                    &sig,
                );
                if let Err(e) = vres {
                    warn!("Commit bad sig: {e:#}");
                    return;
                }
                let (pbft, id) = (&mut self.pbft, &self.id);
                let res = pbft.on_commit(
                    view,
                    seq,
                    &digest,
                    node_id,
                    &sig,
                    id,
                    |_, _| Ok(()),
                );
                match res {
                    Ok(acts) => self.exec_actions(acts).await,
                    Err(e) => warn!("Commit rejected: {e:#}"),
                }
            }
            Message::ViewChange { new_view, node_id } => {
                if new_view > self.pbft.view {
                    info!("view change {node_id} -> {new_view} (was {})", self.pbft.view);
                    // Minimal: jump to max seen (full PBFT would require 2f+1
                    // ViewChange before moving). Clear per-view voting state.
                    self.pbft.set_view(new_view);
                    let _ = peer_id;
                }
            }
            Message::QueryWatermark { watermark } => {
                if let Some(reply) = reply {
                    match self.store.get_by_watermark(&watermark) {
                        Ok(Some(b)) => {
                            let _ = reply.send(ResponseEnvelope {
                                req_id,
                                resp: Response::QueryResult {
                                    blocks: vec![b],
                                    error: None,
                                },
                            });
                        }
                        Ok(None) => {
                            let _ = reply.send(ResponseEnvelope {
                                req_id,
                                resp: Response::QueryResult {
                                    blocks: vec![],
                                    error: Some("not found".into()),
                                },
                            });
                        }
                        Err(e) => {
                            let _ = reply.send(ResponseEnvelope {
                                req_id,
                                resp: Response::QueryResult {
                                    blocks: vec![],
                                    error: Some(e.to_string()),
                                },
                            });
                        }
                    }
                }
            }
            Message::QueryUser { user_id } => {
                if let Some(reply) = reply {
                    match self.store.get_by_user(&user_id) {
                        Ok(blocks) => {
                            let _ = reply.send(ResponseEnvelope {
                                req_id,
                                resp: Response::QueryResult { blocks, error: None },
                            });
                        }
                        Err(e) => {
                            let _ = reply.send(ResponseEnvelope {
                                req_id,
                                resp: Response::QueryResult {
                                    blocks: vec![],
                                    error: Some(e.to_string()),
                                },
                            });
                        }
                    }
                }
            }
            Message::GetBlock { index } => {
                if let Some(reply) = reply {
                    match self.store.get_by_index(index) {
                        Ok(Some(b)) => {
                            let _ = reply.send(ResponseEnvelope {
                                req_id,
                                resp: Response::QueryResult {
                                    blocks: vec![b],
                                    error: None,
                                },
                            });
                        }
                        Ok(None) => {
                            let _ = reply.send(ResponseEnvelope {
                                req_id,
                                resp: Response::QueryResult {
                                    blocks: vec![],
                                    error: Some("not found".into()),
                                },
                            });
                        }
                        Err(e) => {
                            let _ = reply.send(ResponseEnvelope {
                                req_id,
                                resp: Response::QueryResult {
                                    blocks: vec![],
                                    error: Some(e.to_string()),
                                },
                            });
                        }
                    }
                }
            }
            Message::Status {} => {
                if let Some(reply) = reply {
                    let tip = self.store.tip().ok().flatten();
                    let _ = reply.send(ResponseEnvelope {
                        req_id,
                        resp: Response::StatusResult {
                            node_id: self.cfg.node.id,
                            height: tip.as_ref().map(|t| t.index + 1).unwrap_or(0),
                            tip_hash: tip
                                .as_ref()
                                .map(|t| t.hash.clone())
                                .unwrap_or_else(|| "GENESIS".into()),
                            view: self.pbft.view,
                            seq: self.pbft.next_seq,
                        },
                    });
                }
            }
            Message::GetLogs { limit, level } => {
                // Auditor tail: served straight from the ring buffer, no
                // consensus involved. Restriction is enforced one hop up, at
                // the gateway's `GET /logs` (Bearer CA permit).
                if let Some(reply) = reply {
                    let entries = crate::logbuf::LogBuffer::global()
                        .snapshot(level.as_deref(), limit as usize);
                    let _ = reply.send(ResponseEnvelope {
                        req_id,
                        resp: Response::LogsResult { entries, error: None },
                    });
                }
            }
        }
    }

    /// Shared handler for locally-looped messages (currently unused path kept for clarity).
    async fn handle_peer_envelope(
        &mut self,
        _env: Envelope,
        _reply: Option<ReplyTx>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_submit(
        &mut self,
        record: DecryptionRecord,
        req_id: Option<u64>,
        reply: Option<ReplyTx>,
        forwarded: bool,
    ) {
        if let Err(e) = record.validate() {
            if let Some(r) = reply {
                let _ = r.send(ResponseEnvelope {
                    req_id,
                    resp: Response::Reply {
                        ok: false,
                        block_index: None,
                        block_hash: None,
                        error: Some(format!("invalid record: {e:#}")),
                    },
                });
            }
            return;
        }
        // Replay / duplicate suppression.
        if self.store.has_watermark(&record.watermark).unwrap_or(false) {
            if let Some(r) = reply {
                match self.store.get_by_watermark(&record.watermark) {
                    Ok(Some(b)) => {
                        let _ = r.send(ResponseEnvelope {
                            req_id,
                            resp: Response::Reply {
                                ok: true,
                                block_index: Some(b.header.index),
                                block_hash: Some(b.header.hash),
                                error: Some("duplicate: already committed".into()),
                            },
                        });
                    }
                    _ => {
                        let _ = r.send(ResponseEnvelope {
                            req_id,
                            resp: Response::Reply {
                                ok: false,
                                block_index: None,
                                block_hash: None,
                                error: Some("duplicate watermark".into()),
                            },
                        });
                    }
                }
            }
            return;
        }

        if self.pbft.is_leader() {
            info!(
                "node {} proposing wm={} view={}",
                self.cfg.node.id, record.watermark, self.pbft.view
            );
            self.remember_pending(&record, req_id, reply);
            let tip = self.store.tip().ok().flatten();
            match self.pbft.propose(record.clone(), tip.as_ref(), &self.id) {
                Ok(acts) => self.exec_actions(acts).await,
                Err(e) => {
                    warn!("propose failed: {e:#}");
                    if let Some(waiters) = self.pending.remove(&record.watermark) {
                        for (rid, r) in waiters {
                            let _ = r.send(ResponseEnvelope {
                                req_id: rid,
                                resp: Response::Reply {
                                    ok: false,
                                    block_index: None,
                                    block_hash: None,
                                    error: Some(e.to_string()),
                                },
                            });
                        }
                    }
                }
            }
        } else {
            // Replica: remember local waiter, forward to leader (unless already forwarded to avoid loops).
            self.remember_pending(&record, req_id, reply);
            let leader = self.pbft.leader();
            info!(
                "node {} forwarding wm={} to leader {leader} view={}",
                self.cfg.node.id, record.watermark, self.pbft.view
            );
            let env = Envelope {
                req_id: None,
                from: Some(self.cfg.node.id),
                msg: Message::ForwardSubmit { record, origin: self.cfg.node.id },
            };
            if !self.links.send_to(leader, env).await {
                warn!("no link to leader {leader}; client will retry / view-change");
            }
            let _ = forwarded;
        }
    }

    async fn commit_block(&mut self, block: Block) {
        let wm = block.record.watermark.clone();
        match self.store.insert_block(&block) {
            Ok(()) => {
                self.last_commit = std::time::Instant::now();
                info!(
                    "node {} committed idx={} hash={} wm={}",
                    self.cfg.node.id,
                    block.header.index,
                    &block.header.hash[..12.min(block.header.hash.len())],
                    wm
                );
            }
            Err(e) => {
                // Duplicate commit after restart/replay: treat as benign.
                let msg = e.to_string();
                if msg.contains("UNIQUE") || msg.contains("duplicate") || msg.contains("already") {
                    info!("commit skipped (already stored) idx={}: {msg}", block.header.index);
                } else {
                    warn!("commit failed idx={}: {e:#}", block.header.index);
                    return;
                }
            }
        }
        if let Some(waiters) = self.pending.remove(&wm) {
            for (rid, r) in waiters {
                let _ = r.send(ResponseEnvelope {
                    req_id: rid,
                    resp: Response::Reply {
                        ok: true,
                        block_index: Some(block.header.index),
                        block_hash: Some(block.header.hash.clone()),
                        error: None,
                    },
                });
            }
        }
    }

    async fn maybe_view_change(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        if self.last_commit.elapsed()
            < std::time::Duration::from_millis(self.cfg.consensus.view_timeout_ms.max(1000))
        {
            return;
        }
        let new_view = self.pbft.view + 1;
        warn!(
            "node {} view timeout, proposing view {new_view}",
            self.cfg.node.id
        );
        self.links
            .broadcast(Envelope {
                req_id: None,
                from: Some(self.cfg.node.id),
                msg: Message::ViewChange {
                    new_view,
                    node_id: self.cfg.node.id,
                },
            })
            .await;
        // Clear per-view state but keep pending waiters so clients are retried.
        let pending = std::mem::take(&mut self.pending);
        self.pbft.set_view(new_view);
        self.pending = pending;
        self.last_commit = std::time::Instant::now();
        // If we became leader, re-propose pending watermarks? We lack full records
        // (only watermarks kept). Clients (register gate) must resubmit after view change —
        // document this. Keep waiters so a resubmitted commit still resolves them.
    }
}

pub async fn run_node(cfg_path: &str) -> anyhow::Result<()> {
    let cfg = NodeConfig::from_file(cfg_path)?;
    Node::new(cfg)?.run().await
}
