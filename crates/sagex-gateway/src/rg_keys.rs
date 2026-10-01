//! This gateway's own ML-DSA-65 identity (RG keypair).
//!
//! Raw keys, STANDARD base64 — same convention as ledger node identities.
//! Generated once into `gateway.db` (`rg_identity` table), loaded after.
//! The CA pins the public half at startup; the fingerprint log line is what
//! the operator compares when pasting it into the CA config.

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use rand_core::OsRng;
use rustpq::ml_dsa::mldsa65;
use sha2::{Digest, Sha256};

/// Generate a fresh keypair. Returns `(secret_b64, public_b64)`.
pub fn generate() -> (String, String) {
    let (pk, sk) = mldsa65::generate(&mut OsRng);
    (B64.encode(sk.as_bytes()), B64.encode(pk.as_bytes()))
}

/// SHA-256 hex fingerprint of the raw public key bytes (STANDARD b64 in).
/// Short enough for logs, stable for pin comparison.
pub fn fingerprint(public_b64: &str) -> anyhow::Result<String> {
    let raw = B64
        .decode(public_b64.trim())
        .map_err(|e| anyhow::anyhow!("bad RG public key: {e}"))?;
    if raw.len() != mldsa65::PUBLIC_KEY_BYTES {
        anyhow::bail!(
            "bad RG public key length: {} (want {})",
            raw.len(),
            mldsa65::PUBLIC_KEY_BYTES
        );
    }
    let digest = Sha256::digest(raw);
    Ok(digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>())
}
