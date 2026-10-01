//! Recipient-unique short watermark IDs via keyed derivation.
//!
//! Construction (v1, versioned by domain separator):
//! ```text
//! watermark = "wm-" || base64url( HMAC-SHA256(
//!     key = recipient_dsa_public_key,
//!     msg = "sagex-wm-v1" || file_hash || recipient_user_id || session_id
//! )[0..18] )
//! ```
//!
//! Properties:
//! - Unique per recipient: the HMAC key IS the recipient public key — same
//!   file + different recipient always diverges, even across renames.
//! - Offline: pure local computation, no network, no new secrets.
//! - Short: `wm-` + 24 chars, far under XML attribute limits.
//! - Non-reversible: one-way HMAC; a leaked file does not reveal its holder
//!   except via ledger lookup. Attribution lives in the ledger, not the ID.
//! - Deterministic: same inputs always reproduce (golden-vector testable).
//! - Per-file (`file_hash` = sealed-package SHA-256, known pre-decryption)
//!   and per-session unique.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

pub const WM_DOMAIN: &[u8] = b"sagex-wm-v1";
pub const WM_PREFIX: &str = "wm-";
/// Raw ID bytes kept (18 -> 24 base64url chars).
pub const WM_ID_BYTES: usize = 18;

#[derive(Debug, thiserror::Error)]
pub enum WatermarkError {
    #[error("empty recipient public key")]
    EmptyKey,
}

/// Derive the watermark ID. All inputs are required; the recipient public
/// key is the uniqueness anchor.
pub fn derive_watermark_id(
    recipient_dsa_pub: &[u8],
    file_hash: &[u8; 32],
    user_id: &str,
    session_id: &str,
) -> Result<String, WatermarkError> {
    if recipient_dsa_pub.is_empty() {
        return Err(WatermarkError::EmptyKey);
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(recipient_dsa_pub)
        .map_err(|_| WatermarkError::EmptyKey)?;
    mac.update(WM_DOMAIN);
    mac.update(file_hash);
    mac.update(user_id.as_bytes());
    mac.update(session_id.as_bytes());
    let out = mac.finalize().into_bytes();
    Ok(format!("{WM_PREFIX}{}", URL_SAFE_NO_PAD.encode(&out[..WM_ID_BYTES])))
}

/// SHA-256 of bytes (file hashing for derivation inputs).
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(data).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_pub() -> Vec<u8> {
        // Fixed 1952-byte stand-in (length realistic; values deterministic).
        (0..1952u32).map(|i| (i.wrapping_mul(2654435761) >> 8) as u8).collect()
    }

    #[test]
    fn golden_vector() {
        let wm = derive_watermark_id(&fixture_pub(), &[0xABu8; 32], "alice", "sess-1").unwrap();
        assert_eq!(wm, "wm-5OnNHIuXdQ9tdFr65ApPGy9c");
    }

    #[test]
    fn determinism_and_shape() {
        let a = derive_watermark_id(&fixture_pub(), &[1u8; 32], "alice", "s1").unwrap();
        let b = derive_watermark_id(&fixture_pub(), &[1u8; 32], "alice", "s1").unwrap();
        assert_eq!(a, b);
        assert!(a.starts_with(WM_PREFIX));
        assert_eq!(a.len(), WM_PREFIX.len() + 24);
        assert!(a[WM_PREFIX.len()..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn diverges_across_recipient_file_session() {
        let k1 = fixture_pub();
        let mut k2 = k1.clone();
        k2[0] ^= 0xFF;
        let base = derive_watermark_id(&k1, &[1u8; 32], "alice", "s1").unwrap();
        // Different recipient key, same everything else.
        assert_ne!(base, derive_watermark_id(&k2, &[1u8; 32], "alice", "s1").unwrap());
        // Same recipient, different file.
        assert_ne!(base, derive_watermark_id(&k1, &[2u8; 32], "alice", "s1").unwrap());
        // Same recipient+file, different session.
        assert_ne!(base, derive_watermark_id(&k1, &[1u8; 32], "alice", "s2").unwrap());
        // Same recipient, different user label.
        assert_ne!(base, derive_watermark_id(&k1, &[1u8; 32], "bob", "s1").unwrap());
    }

    #[test]
    fn non_reversible_and_rejects_empty_key() {
        let k = fixture_pub();
        let wm = derive_watermark_id(&k, &[9u8; 32], "alice", "sess-9").unwrap();
        assert!(!wm.contains("alice"));
        assert!(!wm.contains("sess-9"));
        let pub_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            &k,
        );
        assert!(!wm.contains(&pub_b64[..8]));
        assert!(derive_watermark_id(&[], &[9u8; 32], "alice", "s").is_err());
    }
}
