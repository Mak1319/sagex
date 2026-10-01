//! Forensic send/receive orchestration for watermarked documents.
//!
//! Split of responsibilities:
//! - `sagex-forensic` (crate): pure crypto — seal/verify/decrypt, watermark
//!   KDF, Office embed/extract. No I/O, no network.
//! - here: vault-backed orchestration — key loading, record signing,
//!   pipeline order (verify → register → decrypt → embed → store).
//!
//! The pipeline functions take explicit bytes/keys and return bytes; they
//! never touch disk themselves, so the no-plaintext invariant is structural.
//! Screens own all file I/O and network calls (via `GatewayClient`).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use binrw::{BinRead, BinWrite};
use sagex_crypto::pqc::dsa;
use std::io::Cursor;
use zeroize::Zeroizing;

use super::gateway::DecryptionRecord;

/// Domain separation for ledger-record signatures (distinct from PoP and
/// envelope contexts so a signature never validates in the wrong protocol).
pub const RECORD_CTX: &[u8] = b"sagex-record-v1";

/// Mint an opaque session id. Process-randomized + time + pid: unique
/// enough for a label, carries no meaning, never reused as a secret.
pub fn new_session_id() -> String {
    let mut h = RandomState::new().build_hasher();
    h.write_u64(std::process::id() as u64);
    h.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    format!("sess-{:016x}", h.finish())
}

/// SHA-256 hex of bytes (file hashes for records).
pub fn file_sha256_hex(data: &[u8]) -> String {
    sagex_forensic::watermark::sha256_hex(data)
}

/// Payload hash for a receipt (confirmed definition): SHA-256 hex of the
/// envelope's `ciphertext_b64` bytes — i.e. the base64 text as transmitted,
/// not the decoded ciphertext and never the plaintext. Computable
/// pre-decrypt, so the fail-closed order (verify → register → decrypt)
/// holds: the gateway sees the commitment before any plaintext exists.
pub fn payload_sha256_hex(sealed: &[u8]) -> Result<String, String> {
    let env =
        sagex_forensic::from_bytes(sealed).map_err(|e| format!("envelope corrupt: {e}"))?;
    Ok(sagex_forensic::watermark::sha256_hex(
        env.ciphertext_b64.as_bytes(),
    ))
}

/// Unsigned receipt fields: verify the package, then derive the watermark
/// and both hashes. Returns `(watermark, file_hash_hex, payload_hash_hex)`.
/// Pure function of sealed bytes + recipient material (offline); the caller
/// builds + signs the record and talks to the gateway.
pub fn receipt_fields(
    sealed: &[u8],
    sender_dsa_pub: &[u8],
    recipient_dsa_pub: &[u8],
    user_id: &str,
    session_id: &str,
) -> Result<(String, String, String), String> {
    verify_package(sealed, sender_dsa_pub)?;
    let (wm, file_hex) =
        derive_receipt_watermark(sealed, recipient_dsa_pub, user_id, session_id)?;
    let payload_hex = payload_sha256_hex(sealed)?;
    Ok((wm, file_hex, payload_hex))
}

/// Seal plaintext for recipients. `sign` signs the envelope signing bytes
/// with the sender DSA key (caller builds it from the vault key, same
/// pattern as CSR PoP signing).
pub fn seal_for(
    plaintext: &[u8],
    sender: &str,
    recipients: &[(String, Vec<u8>)],
    sign: impl FnOnce(&[u8]) -> Vec<u8>,
) -> Result<Vec<u8>, String> {
    let rkeys: Vec<sagex_forensic::RecipientKey> = recipients
        .iter()
        .map(|(id, kem)| sagex_forensic::RecipientKey {
            identity: id.clone(),
            kem_pub: kem.clone(),
        })
        .collect();
    let env = sagex_forensic::seal(plaintext, sender, true, &rkeys, sign)
        .map_err(|e| e.to_string())?;
    Ok(sagex_forensic::to_bytes(&env))
}

/// Verify a sealed package against the sender's DSA public key. Returns the
/// sender identity on success; never decrypts (see `decrypt_for`).
pub fn verify_package(sealed: &[u8], sender_dsa_pub: &[u8]) -> Result<String, String> {
    let env =
        sagex_forensic::from_bytes(sealed).map_err(|e| format!("envelope corrupt: {e}"))?;
    if !env.watermark_required {
        return Err("not flagged for watermarking".into());
    }
    let v = sagex_forensic::open_verify(&env, sender_dsa_pub)
        .map_err(|e| format!("signature invalid: {e}"))?;
    Ok(v.sender)
}

/// Derive the recipient-unique watermark id. Pure function of sealed bytes
/// + recipient identity material (offline, deterministic).
pub fn derive_receipt_watermark(
    sealed: &[u8],
    recipient_dsa_pub: &[u8],
    user_id: &str,
    session_id: &str,
) -> Result<(String, String), String> {
    let file_hash = sagex_forensic::watermark::sha256_bytes(sealed);
    let file_hex: String = file_hash.iter().map(|b| format!("{b:02x}")).collect();
    let wm = sagex_forensic::derive_watermark_id(recipient_dsa_pub, &file_hash, user_id, session_id)
        .map_err(|e| e.to_string())?;
    Ok((wm, file_hex))
}

/// Build the ledger record for a receipt. Canonical bytes replicate
/// `sagex-ledger/src/model.rs::canonical_bytes` exactly (fixed field order
/// via `serde_json::json!`, which sorts keys — same construction).
pub fn build_record(
    watermark: &str,
    session_id: &str,
    user_id: &str,
    file_hash_hex: &str,
    payload_hash_hex: &str,
    auth_server: &str,
    signature_b64: &str,
    timestamp: i64,
) -> DecryptionRecord {
    DecryptionRecord {
        watermark: watermark.to_string(),
        session_id: session_id.to_string(),
        timestamp,
        user_id: user_id.to_string(),
        file_hash: file_hash_hex.to_string(),
        payload_hash: payload_hash_hex.to_string(),
        auth_server: auth_server.to_string(),
        signature: signature_b64.to_string(),
    }
}

/// Canonical bytes the record signature covers. MUST stay byte-identical
/// to the ledger's `canonical_bytes()` (fixed order, no whitespace).
pub fn record_canonical_bytes(r: &DecryptionRecord) -> Vec<u8> {
    let v = serde_json::json!({
        "watermark": r.watermark,
        "session_id": r.session_id,
        "timestamp": r.timestamp,
        "user_id": r.user_id,
        "file_hash": r.file_hash,
        "payload_hash": r.payload_hash,
        "auth_server": r.auth_server,
    });
    serde_json::to_vec(&v).expect("json")
}

/// Session-unlocked device keys for the watermark receive pipeline.
///
/// The vault keeps secrets password-encapsulated at rest; per-receipt record
/// signing (`sign_record`) and KEM decryption both need the password. This
/// holds the minimum decrypted material for one chat session:
/// - `dsa_pub`: watermark-derivation anchor (public, also in `.pub`);
/// - `kem_secret`: raw KEM secret for `decrypt` (decrypted once at unlock);
/// - `dsa_encap` + `password`: per-receipt `sign_from_password` input
///   (the crypto API only signs from an encapsulated key + password, so the
///   password must stay resident; it is `Zeroizing` and cleared on lock).
///
/// Fail closed: no password / no vault keys → no register, no decrypt.
#[derive(Clone)]
pub struct DeviceSession {
    pub identity: String,
    pub dsa_pub: Vec<u8>,
    pub kem_secret: Vec<u8>,
    /// binrw-encoded DSA encapsulation (the crypto API takes `KeyEncapsulation`
    /// by value and it has no `Clone`; decode fresh per signature, mirroring
    /// the enrollment reload pattern).
    dsa_encap_bytes: Vec<u8>,
    password: Zeroizing<Vec<u8>>,
}

fn decode_encap(
    bytes: &[u8],
) -> Result<sagex_crypto::aes::KeyEncapsulation, String> {
    sagex_crypto::aes::KeyEncapsulation::read_le(&mut Cursor::new(bytes))
        .map_err(|e| format!("vault key decode: {e}"))
}

impl DeviceSession {
    /// Unlock from the vault. Fails when keys are missing, belong to another
    /// user, or the password is wrong — callers render "Vault locked".
    pub fn unlock(
        vault: &super::vault::Vault,
        password: &[u8],
        username: &str,
    ) -> Result<Self, String> {
        let publ = vault.load_pub().map_err(|e| format!("vault: {e}"))?;
        if publ.internal.user_name != username {
            return Err("Stored keys belong to another user.".into());
        }
        let prv = vault.load_prv().map_err(|e| format!("vault: {e}"))?;
        let pw = Zeroizing::new(password.to_vec());
        let kem_secret =
            sagex_crypto::aes::AESHandler::decrypt_private_key(&pw, prv.internal.key_encapsulation_kem)
                .map_err(|e| format!("wrong vault password or corrupted keys: {e:?}"))?;
        let mut encap_bytes = Vec::new();
        prv.internal
            .key_encapsulation_dsa
            .write_le(&mut Cursor::new(&mut encap_bytes))
            .map_err(|e| format!("vault key encode: {e}"))?;
        Ok(Self {
            identity: username.to_string(),
            dsa_pub: publ.internal.key_dsa.clone(),
            kem_secret,
            dsa_encap_bytes: encap_bytes,
            password: pw,
        })
    }

    /// Sign a record's canonical bytes with the device DSA key.
    pub fn sign(&self, record: &DecryptionRecord) -> Result<String, String> {
        sign_record(decode_encap(&self.dsa_encap_bytes)?, &self.password, record)
    }
}

/// Sign record canonical bytes with the device DSA key (reload pattern:
/// fresh parse per call, mirrors enrollment signing).
pub fn sign_record(
    dsa_encap: sagex_crypto::aes::KeyEncapsulation,
    password: &[u8],
    record: &DecryptionRecord,
) -> Result<String, String> {
    let pw = Zeroizing::new(password.to_vec());
    let sig = dsa::Implement::sign_from_password(dsa_encap, &pw, &record_canonical_bytes(record), RECORD_CTX)
        .map_err(|e| format!("{e:?}"))?;
    Ok(B64.encode(sig.as_bytes()))
}

/// Decrypt a verified sealed package WITHOUT Office embedding (chat-text
/// path: the registered ledger record IS the watermark; content renders as
/// text). Returns raw plaintext; the caller renders it. Still fail-closed:
/// re-verifies the signature and the flag before touching ciphertext.
pub fn decrypt_verified(
    sealed: &[u8],
    sender_dsa_pub: &[u8],
    recipient_identity: &str,
    kem_secret: &[u8],
) -> Result<Vec<u8>, String> {
    let env =
        sagex_forensic::from_bytes(sealed).map_err(|e| format!("envelope corrupt: {e}"))?;
    let verified = sagex_forensic::open_verify(&env, sender_dsa_pub)
        .map_err(|e| format!("signature invalid: {e}"))?;
    if !verified.watermark_required {
        return Err("not flagged for watermarking".into());
    }
    sagex_forensic::decrypt(&verified, recipient_identity, kem_secret)
        .map_err(|e| format!("decrypt failed: {e}"))
}

/// Decrypt a verified sealed package and embed the watermark, all in
/// memory. Returns marked bytes; the caller decides when/where to store.
pub fn decrypt_and_mark(
    sealed: &[u8],
    sender_dsa_pub: &[u8],
    recipient_identity: &str,
    kem_secret: &[u8],
    watermark_id: &str,
) -> Result<Vec<u8>, String> {
    let env =
        sagex_forensic::from_bytes(sealed).map_err(|e| format!("envelope corrupt: {e}"))?;
    let verified = sagex_forensic::open_verify(&env, sender_dsa_pub)
        .map_err(|e| format!("signature invalid: {e}"))?;
    if !verified.watermark_required {
        return Err("not flagged for watermarking".into());
    }
    let plain = sagex_forensic::decrypt(&verified, recipient_identity, kem_secret)
        .map_err(|e| format!("decrypt failed: {e}"))?;
    sagex_forensic::embed_watermark(&plain, watermark_id).map_err(|e| e.to_string())
}

/// Leak check: extract the watermark id from suspect bytes (None = plain).
pub fn extract_leak_id(data: &[u8]) -> Result<Option<String>, String> {
    sagex_forensic::extract_watermark_id(data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::{BinRead, BinWrite};
    use std::io::Cursor;

    fn encap_bytes(e: &sagex_crypto::aes::KeyEncapsulation) -> Vec<u8> {
        let mut buf = Vec::new();
        e.write_le(&mut Cursor::new(&mut buf)).unwrap();
        buf
    }

    fn encap_from(b: &[u8]) -> sagex_crypto::aes::KeyEncapsulation {
        sagex_crypto::aes::KeyEncapsulation::read_le(&mut Cursor::new(b)).unwrap()
    }

    #[test]
    fn canonical_bytes_match_ledger_layout() {
        // Same macro field list as DecryptionRecord::canonical_bytes(), so
        // both sides produce byte-identical output under one serde_json
        // build (BTreeMap-sorted keys either way). Assert structure plus
        // determinism rather than a hardcoded key order.
        let r = DecryptionRecord {
            watermark: "wm".into(),
            session_id: "s".into(),
            timestamp: 7,
            user_id: "u".into(),
            file_hash: "a".into(),
            payload_hash: "b".into(),
            auth_server: "auth-1".into(),
            signature: "sig".into(),
        };
        let b = record_canonical_bytes(&r);
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["watermark"], "wm");
        assert_eq!(v["session_id"], "s");
        assert_eq!(v["timestamp"], 7);
        assert_eq!(v["user_id"], "u");
        assert_eq!(v["file_hash"], "a");
        assert_eq!(v["payload_hash"], "b");
        assert_eq!(v["auth_server"], "auth-1");
        // Signature is covered by the signature, not part of canonical bytes.
        assert!(v.get("signature").is_none());
        assert_eq!(record_canonical_bytes(&r), b, "deterministic");
    }

    #[test]
    fn session_ids_unique_and_opaque() {
        let a = new_session_id();
        let b = new_session_id();
        assert!(a.starts_with("sess-"));
        assert_ne!(a, b);
    }

    #[test]
    fn seal_verify_decrypt_mark_roundtrip() {
        use sagex_crypto::{aes::AESHandler, pqc::kem};
        let pw = b"forensic-test-pw";
        let (kenc, kpub) = kem::KeyGen::generate_from_password(pw).unwrap();
        let (denc, dpub) = dsa::KeyGen::generate_from_password(pw).unwrap();
        let denc_bytes = encap_bytes(&denc);
        let kenc_bytes = encap_bytes(&kenc);
        // Fresh parse per signing call mirrors the vault-reload pattern.
        let sealed = seal_for(
            b"hello forensic",
            "alice",
            &[("bob".to_string(), kpub.clone())],
            |msg| {
                let e = encap_from(&denc_bytes);
                dsa::Implement::sign_from_password(e, pw, msg, sagex_forensic::SIG_CTX)
                    .unwrap()
                    .as_bytes()
                    .to_vec()
            },
        )
        .unwrap();
        let sender = verify_package(&sealed, &dpub).unwrap();
        assert_eq!(sender, "alice");
        let secret = AESHandler::decrypt_private_key(pw, encap_from(&kenc_bytes)).unwrap();
        // Chat-text path: verify + decrypt without Office embedding.
        let plain = decrypt_verified(&sealed, &dpub, "bob", &secret).unwrap();
        assert_eq!(plain, b"hello forensic");
        // Office-embed path fails closed on non-office plaintext (the embed
        // gate only accepts watermarkable documents; see doc.rs).
        assert!(decrypt_and_mark(&sealed, &dpub, "bob", &secret, "wm-test-1").is_err());
        // Unknown recipient cannot decrypt on either path.
        assert!(decrypt_verified(&sealed, &dpub, "mallory", &secret,).is_err());
        assert!(decrypt_and_mark(&sealed, &dpub, "mallory", &secret, "wm-x").is_err());
    }

    #[test]
    fn payload_hash_is_ciphertext_b64_sha256() {
        use sagex_crypto::pqc::kem;
        let pw = b"forensic-payload-pw";
        let (kenc, kpub) = kem::KeyGen::generate_from_password(pw).unwrap();
        let (denc, _dpub) = dsa::KeyGen::generate_from_password(pw).unwrap();
        let denc_bytes = encap_bytes(&denc);
        let _ = encap_bytes(&kenc);
        let sealed = seal_for(
            b"payload check",
            "alice",
            &[("bob".to_string(), kpub.clone())],
            |msg| {
                let e = encap_from(&denc_bytes);
                dsa::Implement::sign_from_password(e, pw, msg, sagex_forensic::SIG_CTX)
                    .unwrap()
                    .as_bytes()
                    .to_vec()
            },
        )
        .unwrap();
        // Independent recomputation from the envelope JSON field.
        let env: serde_json::Value = serde_json::from_slice(&sealed).unwrap();
        let ct_b64 = env.get("ciphertext_b64").unwrap().as_str().unwrap();
        let expect = sagex_forensic::watermark::sha256_hex(ct_b64.as_bytes());
        assert_eq!(payload_sha256_hex(&sealed).unwrap(), expect);
        // Distinct from the file hash (sealed-package hash).
        let (_, file_hex) =
            derive_receipt_watermark(&sealed, &[9u8; 32], "bob", "s1").unwrap();
        assert_ne!(payload_sha256_hex(&sealed).unwrap(), file_hex);
    }

    #[test]
    fn receipt_fields_verify_before_derive() {
        use sagex_crypto::pqc::kem;
        let pw = b"forensic-receipt-pw";
        let (kenc, kpub) = kem::KeyGen::generate_from_password(pw).unwrap();
        let (denc, dpub) = dsa::KeyGen::generate_from_password(pw).unwrap();
        let (denc2, dpub2) = dsa::KeyGen::generate_from_password(b"other-pw").unwrap();
        let denc_bytes = encap_bytes(&denc);
        let _ = (encap_bytes(&kenc), encap_bytes(&denc2));
        let sealed = seal_for(
            b"receipt check",
            "alice",
            &[("bob".to_string(), kpub.clone())],
            |msg| {
                let e = encap_from(&denc_bytes);
                dsa::Implement::sign_from_password(e, pw, msg, sagex_forensic::SIG_CTX)
                    .unwrap()
                    .as_bytes()
                    .to_vec()
            },
        )
        .unwrap();
        // Wrong sender key -> verify fails, no fields leak.
        assert!(receipt_fields(&sealed, &dpub2, &dpub, "bob", "s1").is_err());
        // Right key -> deterministic fields.
        let (wm1, fh1, ph1) =
            receipt_fields(&sealed, &dpub, &dpub, "bob", "s1").unwrap();
        let (wm2, fh2, ph2) =
            receipt_fields(&sealed, &dpub, &dpub, "bob", "s1").unwrap();
        assert_eq!((wm1.clone(), fh1, ph1), (wm2, fh2, ph2));
        assert!(wm1.starts_with("wm-"));
        // Corrupt envelope -> error, never partial fields.
        let mut bad = sealed.clone();
        bad[10] ^= 0xFF;
        assert!(payload_sha256_hex(&bad).is_err() || receipt_fields(&bad, &dpub, &dpub, "bob", "s1").is_err());
    }
}
