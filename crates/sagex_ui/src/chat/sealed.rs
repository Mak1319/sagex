//! Watermark-flagged message detection + sealed-payload extraction.
//!
//! Wire convention (receiver-tolerant, no chatsrv migration needed):
//! - Primary: `kind == "sealed"` with `metadata.envelope` holding the
//!   `SealedEnvelope` JSON (object or JSON string). `metadata.sender_dsa_b64`
//!   optionally carries the sender's raw DSA public key (STANDARD base64) so
//!   the receiver can verify without a key-directory round trip.
//! - Fallback: `body` itself parses as a `SealedEnvelope` with
//!   `watermark_required == true`.
//!
//! Anything else (including envelopes with the flag cleared) returns `None`
//! and follows the normal render path. Flag verification itself
//! (`open_verify`) happens later in the receive pipeline — detection never
//! trusts the flag, it only routes.

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};

use crate::backend::{
    CaClient, DeviceSession, GatewayClient, MessageDto, build_record, decrypt_verified,
    now_unix, receipt_fields,
};

/// A detected flagged message: raw sealed bytes + optional sender key.
pub struct SealedHit {
    /// Raw sealed-package bytes (`file_hash` input, register commitment).
    pub sealed: Vec<u8>,
    /// Sender DSA public key when the sender attached it (`sender_dsa_b64`).
    pub sender_dsa_pub: Option<Vec<u8>>,
}

/// Detect a watermark-flagged message. Returns `None` for normal traffic.
pub fn detect_sealed(m: &MessageDto) -> Option<SealedHit> {
    // Primary: kind + metadata envelope.
    if m.kind == "sealed" {
        if let Some(meta) = m.metadata.as_ref() {
            if let Some(env) = meta.get("envelope") {
                let bytes = if env.is_object() {
                    serde_json::to_vec(env).ok()?
                } else if let Some(s) = env.as_str() {
                    s.as_bytes().to_vec()
                } else {
                    return None;
                };
                let parsed: sagex_forensic::SealedEnvelope =
                    serde_json::from_slice(&bytes).ok()?;
                if !parsed.watermark_required {
                    return None;
                }
                return Some(SealedHit {
                    sealed: sagex_forensic::to_bytes(&parsed),
                    sender_dsa_pub: meta
                        .get("sender_dsa_b64")
                        .and_then(|v| v.as_str())
                        .and_then(|s| B64.decode(s).ok()),
                });
            }
        }
    }
    // Fallback: body is the envelope JSON.
    let trimmed = m.body.trim();
    if trimmed.starts_with('{') {
        if let Ok(parsed) =
            serde_json::from_str::<sagex_forensic::SealedEnvelope>(trimmed)
        {
            if parsed.watermark_required {
                let sender_dsa_pub = m
                    .metadata
                    .as_ref()
                    .and_then(|meta| meta.get("sender_dsa_b64"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| B64.decode(s).ok());
                return Some(SealedHit {
                    sealed: sagex_forensic::to_bytes(&parsed),
                    sender_dsa_pub,
                });
            }
        }
    }
    None
}

/// Display-safe failure mapping for gateway/CA errors on the receive path.
pub fn display_register_error(status: Option<u16>, message: &str) -> String {    match status {
        Some(401) => "Sign in again (permit expired).".to_string(),
        Some(403) => "Permit rejected for this chat identity.".to_string(),
        Some(409) => "Permit already used — tap Retry.".to_string(),
        Some(400) => format!("Registration rejected ({message})."),
        Some(502) => format!("Ledger rejected the record ({message})."),
        Some(_) => format!("Gateway error ({message})."),
        None => {
            if message.contains("Cannot reach") || message.contains("Network error") {
                "Gateway unreachable — tap Retry.".to_string()
            } else {
                message.to_string()
            }
        }
    }
}

/// Retained sealed payload for an explicit user retry (idempotent: the
/// gateway dedupes on `watermark`, so resubmission never double-commits).
#[derive(Clone)]
pub struct StoredSeal {
    pub server_id: String,
    pub sealed: Vec<u8>,
    pub sender_pub: Option<Vec<u8>>,
}

/// Successful receive: decrypted text + registration proof for the badge.
pub struct SealedDone {
    pub text: String,
    pub watermark: String,
    pub status: String,
    pub block_index: Option<u64>,
}

/// Fail-closed receive pipeline (runs on the network runtime, never GPUI):
/// verify → derive → permit → sign → register → decrypt.
///
/// Gate-pass = gateway `committed` / `queued` / `duplicate:true` only.
/// Every other outcome is `Err(reason)` with a display-safe reason and NO
/// plaintext returned.
#[allow(clippy::too_many_arguments)]
pub async fn run_sealed_receive(
    gateway: GatewayClient,
    ca: CaClient,
    access_token: String,
    device: Option<DeviceSession>,
    username: String,
    auth_server: String,
    server_msg_id: String,
    sealed: Vec<u8>,
    sender_pub: Option<Vec<u8>>,
) -> Result<SealedDone, String> {
    let device = device.ok_or_else(|| {
        "Vault locked: sign in with your password to verify watermarked content.".to_string()
    })?;
    if device.identity != username {
        return Err("Device keys belong to another user.".to_string());
    }
    let sender_pub = sender_pub
        .ok_or_else(|| "Unknown sender key — cannot verify watermark.".to_string())?;
    // 1-2. Verify signature + flag, then derive watermark + both hashes.
    // `payload_hash` is sha256 of the transmitted ciphertext_b64 bytes
    // (confirmed definition): computable before decrypt, commits the
    // gateway before any plaintext exists.
    let (watermark, file_hex, payload_hex) = receipt_fields(
        &sealed,
        &sender_pub,
        &device.dsa_pub,
        &username,
        &server_msg_id,
    )?;
    // 3. Fresh single-use user permit (CA binds sub to the chatsrv user).
    let permit = ca
        .request_permit(&access_token)
        .await
        .map_err(|e| display_register_error(e.status, &e.message))?;
    if permit.sub != username {
        return Err("CA issued a permit for another identity.".to_string());
    }
    // 4. Sign the canonical record with the device DSA key.
    let mut record = build_record(
        &watermark,
        &server_msg_id,
        &username,
        &file_hex,
        &payload_hex,
        &auth_server,
        "",
        now_unix() * 1000,
    );
    record.signature = device.sign(&record)?;
    // 5. Register. Only committed/queued/duplicate pass the gate.
    let res = gateway
        .submit(Some(&permit.permit), &record)
        .await
        .map_err(|e| display_register_error(e.status, &e.message))?;
    let gate_pass =
        res.status == "committed" || res.status == "queued" || res.duplicate;
    if !gate_pass {
        return Err(res.detail.unwrap_or_else(|| "Registration failed.".to_string()));
    }
    // 6. Decrypt (re-verifies first) and render as text.
    let plain = decrypt_verified(&sealed, &sender_pub, &device.identity, &device.kem_secret)?;
    Ok(SealedDone {
        text: String::from_utf8_lossy(&plain).into_owned(),
        watermark,
        status: if res.duplicate {
            "committed".to_string()
        } else {
            res.status
        },
        block_index: res.block_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dto(kind: &str, body: &str, metadata: Option<serde_json::Value>) -> MessageDto {
        MessageDto {
            id: "m1".into(),
            room_id: "r1".into(),
            sender_id: "u1".into(),
            body: body.into(),
            created_at: String::new(),
            kind: kind.into(),
            metadata,
            reply_to: None,
            edited_at: None,
            deleted: false,
            reactions: vec![],
            poll: None,
            pinned: false,
        }
    }

    fn envelope_value(flag: bool) -> serde_json::Value {
        json!({
            "v": 1,
            "sender": "alice",
            "watermark_required": flag,
            "recipients": [{
                "identity": "bob",
                "key_fingerprint": "ff",
                "kem_ct_b64": "QUJD",
                "wrap_nonce_b64": "QUJD",
                "wrap_b64": "QUJD"
            }],
            "nonce_b64": "QUJD",
            "ciphertext_b64": "UVFC",
            "signature_b64": "UVFC"
        })
    }

    #[test]
    fn detects_metadata_object_envelope() {
        let m = dto(
            "sealed",
            "sealed file",
            Some(json!({ "envelope": envelope_value(true) })),
        );
        let hit = detect_sealed(&m).expect("flagged envelope");
        assert!(!hit.sealed.is_empty());
        assert!(hit.sender_dsa_pub.is_none());
    }

    #[test]
    fn detects_sender_key_attachment() {
        let m = dto(
            "sealed",
            "sealed file",
            Some(json!({
                "envelope": envelope_value(true),
                "sender_dsa_b64": B64.encode([7u8; 32]),
            })),
        );
        let hit = detect_sealed(&m).expect("flagged envelope");
        assert_eq!(hit.sender_dsa_pub.unwrap(), vec![7u8; 32]);
    }

    #[test]
    fn detects_body_fallback() {
        let body = serde_json::to_string(&envelope_value(true)).unwrap();
        let m = dto("text", &body, None);
        assert!(detect_sealed(&m).is_some());
    }

    #[test]
    fn ignores_unflagged_and_plain() {
        // Flag cleared -> normal path (fail-closed elsewhere, not here).
        let m = dto(
            "sealed",
            "sealed file",
            Some(json!({ "envelope": envelope_value(false) })),
        );
        assert!(detect_sealed(&m).is_none());
        // Plain text.
        assert!(detect_sealed(&dto("text", "hello", None)).is_none());
        // Wrong kind with envelope-shaped body but flag false.
        let body = serde_json::to_string(&envelope_value(false)).unwrap();
        assert!(detect_sealed(&dto("text", &body, None)).is_none());
        // Malformed envelope JSON.
        let m = dto(
            "sealed",
            "x",
            Some(json!({ "envelope": {"v": 1} })),
        );
        assert!(detect_sealed(&m).is_none());
    }

    #[test]
    fn error_mapping() {
        assert!(display_register_error(Some(401), "invalid permit").contains("Sign in again"));
        assert!(display_register_error(Some(409), "used").contains("Retry"));
        assert!(
            display_register_error(None, "Cannot reach the chat server. Is it running?")
                .contains("unreachable")
        );
    }
}
