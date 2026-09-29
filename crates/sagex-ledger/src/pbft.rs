use std::collections::{HashMap, HashSet};

use crate::identity::Identity;
use crate::model::{Block, BlockHeader, DecryptionRecord};
use crate::proto::Envelope;
use crate::proto::Message;

/// Pure PBFT state machine (N=3f+1, quorum=2f+1). IO-free: methods return actions.
pub struct Pbft {
    pub node_id: u64,
    pub view: u64,
    pub next_seq: u64,
    pub f: u64,
    pub quorum: usize,
    pub leader_fn: Box<dyn Fn(u64) -> u64 + Send + Sync>,

    preprepares: HashMap<(u64, u64), Block>,
    prepares: HashMap<(u64, u64, String), HashSet<u64>>,
    commits: HashMap<(u64, u64, String), HashSet<u64>>,
    sent_prepare: HashSet<(u64, u64)>,
    sent_commit: HashSet<(u64, u64)>,
    committed: HashSet<(u64, u64)>,
    /// watermarks already proposed (replay suppression)
    proposed: HashSet<String>,
}

pub enum PbftAction {
    /// Broadcast envelope to all peers (+ self handling already done by caller)
    Broadcast(Envelope),
    /// Send to a single peer
    SendTo(u64, Envelope),
    /// Block reached 2f+1 commits and should be persisted
    Commit(Block),
}

impl Pbft {
    pub fn new(
        node_id: u64,
        f: u64,
        quorum: usize,
        starting_seq: u64,
        leader_fn: Box<dyn Fn(u64) -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            node_id,
            view: 0,
            next_seq: starting_seq.max(1),
            f,
            quorum,
            leader_fn,
            preprepares: HashMap::new(),
            prepares: HashMap::new(),
            commits: HashMap::new(),
            sent_prepare: HashSet::new(),
            sent_commit: HashSet::new(),
            committed: HashSet::new(),
            proposed: HashSet::new(),
        }
    }

    pub fn leader(&self) -> u64 {
        (self.leader_fn)(self.view)
    }

    pub fn is_leader(&self) -> bool {
        self.leader() == self.node_id
    }

    /// Leader entry point for a new client record.
    /// Returns broadcast PrePrepare (+ implicit own Prepare/Commit handling).
    pub fn propose(
        &mut self,
        record: DecryptionRecord,
        tip: Option<&BlockHeader>,
        identity: &Identity,
    ) -> anyhow::Result<Vec<PbftAction>> {
        if !self.is_leader() {
            anyhow::bail!("not leader");
        }
        if self.proposed.contains(&record.watermark) {
            anyhow::bail!("duplicate proposal in flight: {}", record.watermark);
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        let block = match tip {
            Some(t) => Block::next(t, self.view, seq, self.node_id, record.clone()),
            None => {
                // First block: index 0, prev GENESIS
                let ts = chrono::Utc::now().timestamp_millis();
                let hash = Block::compute_hash(
                    0,
                    "GENESIS",
                    ts,
                    self.view,
                    seq,
                    self.node_id,
                    &record,
                );
                Block {
                    header: crate::model::BlockHeader {
                        index: 0,
                        prev_hash: "GENESIS".into(),
                        hash,
                        timestamp: ts,
                        view: self.view,
                        seq,
                        proposer: self.node_id,
                    },
                    record: record.clone(),
                }
            }
        };
        let digest = block.header.hash.clone();
        self.proposed.insert(record.watermark.clone());
        self.preprepares.insert((self.view, seq), block.clone());
        // Leader counts its own prepare/commit implicitly.
        self.prepares
            .entry((self.view, seq, digest.clone()))
            .or_default()
            .insert(self.node_id);

        let sig = identity.sign(&Identity::preprepare_bytes(self.view, seq, &digest));
        let env = Envelope {
            req_id: None,
            from: Some(self.node_id),
            msg: Message::PrePrepare {
                view: self.view,
                seq,
                block: block.clone(),
                sig,
            },
        };
        let mut acts = vec![PbftAction::Broadcast(env)];
        // Leader also emits its own Prepare + Commit handling via same path as replicas:
        acts.extend(self.maybe_prepare(self.view, seq, &digest, identity));
        Ok(acts)
    }

    fn maybe_prepare(
        &mut self,
        view: u64,
        seq: u64,
        digest: &str,
        identity: &Identity,
    ) -> Vec<PbftAction> {
        let mut acts = Vec::new();
        if self.sent_prepare.contains(&(view, seq)) {
            return acts;
        }
        // Need PrePrepare present (leader has it; replicas insert before calling).
        if !self.preprepares.contains_key(&(view, seq)) {
            return acts;
        }
        self.sent_prepare.insert((view, seq));
        let sig = identity.sign(&Identity::vote_bytes(view, seq, digest));
        acts.push(PbftAction::Broadcast(Envelope {
            req_id: None,
            from: Some(self.node_id),
            msg: Message::Prepare {
                view,
                seq,
                digest: digest.to_string(),
                node_id: self.node_id,
                sig,
            },
        }));
        // Record own prepare vote.
        self.prepares
            .entry((view, seq, digest.to_string()))
            .or_default()
            .insert(self.node_id);
        acts.extend(self.maybe_commit(view, seq, digest, identity));
        acts
    }

    fn maybe_commit(
        &mut self,
        view: u64,
        seq: u64,
        digest: &str,
        identity: &Identity,
    ) -> Vec<PbftAction> {
        let mut acts = Vec::new();
        let n_prepare = self
            .prepares
            .get(&(view, seq, digest.to_string()))
            .map(|s| s.len())
            .unwrap_or(0);
        if n_prepare < self.quorum {
            return acts;
        }
        if self.sent_commit.contains(&(view, seq)) {
            // Still check commit-quorum for Commit action below.
        } else {
            self.sent_commit.insert((view, seq));
            let sig = identity.sign(&Identity::vote_bytes(view, seq, digest));
            acts.push(PbftAction::Broadcast(Envelope {
                req_id: None,
                from: Some(self.node_id),
                msg: Message::Commit {
                    view,
                    seq,
                    digest: digest.to_string(),
                    node_id: self.node_id,
                    sig,
                },
            }));
            self.commits
                .entry((view, seq, digest.to_string()))
                .or_default()
                .insert(self.node_id);
        }
        let n_commit = self
            .commits
            .get(&(view, seq, digest.to_string()))
            .map(|s| s.len())
            .unwrap_or(0);
        if n_commit >= self.quorum && !self.committed.contains(&(view, seq)) {
            self.committed.insert((view, seq));
            if let Some(b) = self.preprepares.get(&(view, seq)).cloned() {
                acts.push(PbftAction::Commit(b));
            }
        }
        acts
    }

    /// Replica path: validate + store PrePrepare, then maybe Prepare.
    /// `verify_leader_sig` closure checks the leader's signature.
    pub fn on_preprepare(
        &mut self,
        view: u64,
        seq: u64,
        block: Block,
        sig: &str,
        tip: Option<&BlockHeader>,
        identity: &Identity,
        verify_leader_sig: impl FnOnce(&[u8], &str) -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<PbftAction>> {
        if view != self.view {
            anyhow::bail!("stale view: got {view} want {}", self.view);
        }
        if block.header.view != view || block.header.seq != seq {
            anyhow::bail!("block view/seq mismatch");
        }
        let digest = block.header.hash.clone();
        verify_leader_sig(&Identity::preprepare_bytes(view, seq, &digest), sig)?;
        // Chain continuity against local tip (allow future blocks to wait? keep strict).
        if let Some(t) = tip {
            // Next expected index must match, unless we already committed it (dup).
            if block.header.index != t.index + 1 && !(block.header.index <= t.index) {
                anyhow::bail!(
                    "non-sequential block index: got {} want {}",
                    block.header.index,
                    t.index + 1
                );
            }
            if block.header.index == t.index + 1 && block.header.prev_hash != t.hash {
                anyhow::bail!("prev_hash mismatch");
            }
            // Recompute hash to prevent faulty leader forging linkage.
            if !block.verify_link(&block.header.prev_hash) {
                anyhow::bail!("block hash invalid");
            }
        } else if block.header.index != 0 || block.header.prev_hash != "GENESIS" {
            anyhow::bail!("first block must be genesis-chained");
        }
        // Duplicate seq with different digest = faulty leader; refuse.
        if let Some(existing) = self.preprepares.get(&(view, seq)) {
            if existing.header.hash != digest {
                anyhow::bail!("conflicting PrePrepare for view {view} seq {seq}");
            }
            return Ok(vec![]);
        }
        self.preprepares.insert((view, seq), block);
        // Track highest seq seen so followers stay roughly aligned.
        if seq >= self.next_seq {
            self.next_seq = seq + 1;
        }
        Ok(self.maybe_prepare(view, seq, &digest, identity))
    }

    pub fn on_prepare(
        &mut self,
        view: u64,
        seq: u64,
        digest: &str,
        node_id: u64,
        sig: &str,
        identity: &Identity,
        verify_peer_sig: impl FnOnce(&[u8], &str) -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<PbftAction>> {
        if view != self.view {
            anyhow::bail!("stale view");
        }
        verify_peer_sig(&Identity::vote_bytes(view, seq, digest), sig)?;
        // Ignore votes for unknown digests (no PrePrepare yet) — store anyway for quorum counting.
        self.prepares
            .entry((view, seq, digest.to_string()))
            .or_default()
            .insert(node_id);
        // Only advance our own Prepare if we know the PrePrepare digest matches.
        // NOTE: after we have sent our own Prepare, later incoming Prepares must
        // still drive the prepare-quorum -> commit check (no early return).
        if let Some(pp) = self.preprepares.get(&(view, seq)) {
            if pp.header.hash == digest && !self.sent_prepare.contains(&(view, seq)) {
                return Ok(self.maybe_prepare(view, seq, digest, identity));
            }
            // Already sent (or digest mismatch): fall through to commit-quorum check.
        }
        // Still check commit in case our prepare already sent.
        Ok(self.maybe_commit(view, seq, digest, identity))
    }

    pub fn on_commit(
        &mut self,
        view: u64,
        seq: u64,
        digest: &str,
        node_id: u64,
        sig: &str,
        identity: &Identity,
        verify_peer_sig: impl FnOnce(&[u8], &str) -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<PbftAction>> {
        if view != self.view {
            anyhow::bail!("stale view");
        }
        verify_peer_sig(&Identity::vote_bytes(view, seq, digest), sig)?;
        self.commits
            .entry((view, seq, digest.to_string()))
            .or_default()
            .insert(node_id);
        Ok(self.maybe_commit(view, seq, digest, identity))
    }

    /// Minimal view-change: increment and clear per-view state.
    pub fn advance_view(&mut self) -> u64 {
        self.set_view(self.view + 1)
    }

    /// Jump to an explicit view (used when observing a higher ViewChange).
    pub fn set_view(&mut self, view: u64) -> u64 {
        self.view = view;
        self.preprepares.clear();
        self.prepares.clear();
        self.commits.clear();
        self.sent_prepare.clear();
        self.sent_commit.clear();
        self.committed.clear();
        self.proposed.clear();
        self.view
    }
}
