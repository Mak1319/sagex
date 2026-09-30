//! Full record verification: three independent checks.
//!
//! 1. `inclusion` — the block is present in the gateway `/records` chain
//!    with the same `block_hash`.
//! 2. `hash_link` — `block_hash == sha256(height_le || prev_hash || record_json)`
//!    using the exact field order the ledger node hashed.
//! 3. `pqc_signature` — ML-DSA-65 verify of `signature` over
//!    `recipient_id || watermark_id || doc_hash || session_id`
//!    with `recipient_dsa_pub` from the record.

use sha2::{Digest, Sha256};

use crate::api::{Block, Record};

pub struct Verdict {
    pub inclusion: bool,
    pub hash_link: bool,
    pub pqc_signature: bool,
    pub detail: String,
}

impl Verdict {
    pub fn ok(&self) -> bool {
        self.inclusion && self.hash_link && self.pqc_signature
    }
}

fn hash_block(height: u64, prev: &str, r: &Record) -> Result<String, String> {
    // Field order must match the node's Record struct serialization.
    let v = serde_json::json!({
        "recipient_id": r.recipient_id,
        "watermark_id": r.watermark_id,
        "doc_hash": r.doc_hash,
        "recipient_dsa_pub": r.recipient_dsa_pub,
        "session_id": r.session_id,
        "timestamp_ms": r.timestamp_ms,
        "signature": r.signature,
    });
    let bytes = serde_json::to_vec(&v).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    h.update(height.to_le_bytes());
    h.update(prev.as_bytes());
    h.update(bytes);
    Ok(hex::encode(h.finalize()))
}

pub fn verify(block: &Block, chain: &[Block]) -> Verdict {
    let inclusion = chain
        .iter()
        .any(|b| b.block_hash == block.block_hash && b.record.watermark_id == block.record.watermark_id);

    let hash_link = match hash_block(block.height, &block.prev_hash, &block.record) {
        Ok(h) => h == block.block_hash,
        Err(_) => false,
    };

    let r = &block.record;
    let mut msg = Vec::new();
    msg.extend_from_slice(r.recipient_id.as_bytes());
    msg.extend_from_slice(r.watermark_id.as_bytes());
    msg.extend_from_slice(r.doc_hash.as_bytes());
    msg.extend_from_slice(r.session_id.as_bytes());
    let pqc_signature = match (
        hex::decode(&r.recipient_dsa_pub),
        hex::decode(&r.signature),
    ) {
        (Ok(vk), Ok(sig)) => {
            sagex_crypto::pqc::dsa::verify_signature_bytes(&vk, &msg, &sig).is_ok()
        }
        _ => false,
    };

    let detail = format!(
        "inclusion={} hash_link={} pqc_signature={}",
        ok_str(inclusion),
        ok_str(hash_link),
        ok_str(pqc_signature)
    );
    Verdict { inclusion, hash_link, pqc_signature, detail }
}

fn ok_str(b: bool) -> &'static str {
    if b {
        "OK"
    } else {
        "FAIL"
    }
}
