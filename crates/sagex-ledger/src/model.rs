use serde::{Deserialize, Serialize};

/// Payload stored per block. Submitted by the register gate (sole writer).
/// Field names keep wire compat with `watermark, sessionid, ...` style.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecryptionRecord {
    pub watermark: String,
    #[serde(default, alias = "sessionid")]
    pub session_id: String,
    pub timestamp: i64,
    #[serde(default, alias = "userid")]
    pub user_id: String,
    #[serde(default, alias = "file_hash", alias = "fileHash")]
    pub file_hash: String,
    #[serde(default, alias = "payload_hash", alias = "payloadHash")]
    pub payload_hash: String,
    #[serde(default, alias = "auth_server", alias = "authServer")]
    pub auth_server: String,
    /// Base64 ML-DSA-65 signature by the recipient over the canonical record.
    /// Nodes store it opaquely; full PKI verification happens at the
    /// forensic layer (or here if recipient pubkey is supplied out of band).
    pub signature: String,
}

impl DecryptionRecord {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.watermark.trim().is_empty() {
            anyhow::bail!("watermark is empty");
        }
        if self.session_id.trim().is_empty() {
            anyhow::bail!("session_id is empty");
        }
        if self.user_id.trim().is_empty() {
            anyhow::bail!("user_id is empty");
        }
        if self.file_hash.trim().is_empty() || self.payload_hash.trim().is_empty() {
            anyhow::bail!("file_hash/payload_hash required");
        }
        if self.signature.trim().is_empty() {
            anyhow::bail!("signature is empty");
        }
        Ok(())
    }

    /// Canonical bytes bound by the recipient signature and the block hash.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        // Fixed field order, no whitespace ambiguity.
        let v = serde_json::json!({
            "watermark": self.watermark,
            "session_id": self.session_id,
            "timestamp": self.timestamp,
            "user_id": self.user_id,
            "file_hash": self.file_hash,
            "payload_hash": self.payload_hash,
            "auth_server": self.auth_server,
        });
        serde_json::to_vec(&v).expect("json")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    pub index: u64,
    pub prev_hash: String,
    pub hash: String,
    pub timestamp: i64,
    pub view: u64,
    pub seq: u64,
    pub proposer: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Block {
    pub header: BlockHeader,
    pub record: DecryptionRecord,
}

impl Block {
    pub fn compute_hash(
        index: u64,
        prev_hash: &str,
        timestamp: i64,
        view: u64,
        seq: u64,
        proposer: u64,
        record: &DecryptionRecord,
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(index.to_be_bytes());
        h.update(prev_hash.as_bytes());
        h.update(timestamp.to_be_bytes());
        h.update(view.to_be_bytes());
        h.update(seq.to_be_bytes());
        h.update(proposer.to_be_bytes());
        h.update(record.canonical_bytes());
        hex::encode(h.finalize())
    }

    pub fn genesis(record: DecryptionRecord, proposer: u64) -> Self {
        let ts = chrono::Utc::now().timestamp_millis();
        let hash = Self::compute_hash(0, "GENESIS", ts, 0, 0, proposer, &record);
        Self {
            header: BlockHeader {
                index: 0,
                prev_hash: "GENESIS".into(),
                hash,
                timestamp: ts,
                view: 0,
                seq: 0,
                proposer,
            },
            record,
        }
    }

    pub fn next(
        prev: &BlockHeader,
        view: u64,
        seq: u64,
        proposer: u64,
        record: DecryptionRecord,
    ) -> Self {
        let ts = chrono::Utc::now().timestamp_millis();
        let hash = Self::compute_hash(prev.index + 1, &prev.hash, ts, view, seq, proposer, &record);
        Self {
            header: BlockHeader {
                index: prev.index + 1,
                prev_hash: prev.hash.clone(),
                hash,
                timestamp: ts,
                view,
                seq,
                proposer,
            },
            record,
        }
    }

    pub fn verify_link(&self, prev_hash: &str) -> bool {
        if self.header.prev_hash != prev_hash {
            return false;
        }
        let recomputed = Self::compute_hash(
            self.header.index,
            &self.header.prev_hash,
            self.header.timestamp,
            self.header.view,
            self.header.seq,
            self.header.proposer,
            &self.record,
        );
        recomputed == self.header.hash
    }
}
