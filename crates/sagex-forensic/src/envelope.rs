//! Sealed forensic envelope: signed header (with the watermark flag) +
//! per-recipient DEK/KEK wraps + AES-GCM ciphertext.
//!
//! Wire shape is JSON with base64 fields (debuggable, cross-language).
//! The signature covers everything except itself; intermediaries can SEE
//! the flag but cannot strip or forge it without invalidating the signature.
//!
//! Hash-then-sign: signatures are computed over `SHA256(domain || body)`
//! (see [`prehash_signing_bytes`]), never over raw bodies.
//!
//! Type-level pipeline discipline: [`decrypt`] only accepts a
//! [`VerifiedEnvelope`], so callers cannot decrypt-before-verify by accident.

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use rand_core::{OsRng, RngCore};
use sagex_crypto::{aes::AESHandler, pqc::kem::Implementation as Kem};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ENVELOPE_VERSION: u32 = 1;
/// Domain separation for envelope signatures. Never reused elsewhere.
pub const SIG_CTX: &[u8] = b"sagex-forensic-envelope-v1";
/// Prehash domain: identifies what the digest below was computed over.
const SIG_PREHASH_DOMAIN: &[u8] = b"sagex-envelope-sig-v1";

#[derive(Debug, Clone)]
pub struct RecipientKey {
    pub identity: String,
    pub kem_pub: Vec<u8>,
}

/// SHA-256 hex fingerprint of a recipient KEM public key.
pub fn key_fingerprint(kem_pub: &[u8]) -> String {
    Sha256::digest(kem_pub)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Debug, Clone, Serialize)]
struct SigningBody<'a> {
    v: u32,
    sender: &'a str,
    watermark_required: bool,
    recipients: &'a [RecipientEntry],
    nonce_b64: &'a str,
    ciphertext_b64: &'a str,
}

/// The sealed package as transmitted/stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedEnvelope {
    pub v: u32,
    pub sender: String,
    pub watermark_required: bool,
    pub recipients: Vec<RecipientEntry>,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
    pub signature_b64: String,
}

/// Public view of one sealed recipient entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipientEntry {
    pub identity: String,
    pub key_fingerprint: String,
    pub kem_ct_b64: String,
    pub wrap_nonce_b64: String,
    pub wrap_b64: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EnvelopeError {
    #[error("no recipients")]
    NoRecipients,
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("bad key length")]
    BadKeyLength,
    #[error("unknown recipient: {0}")]
    UnknownRecipient(String),
    #[error("signature invalid")]
    BadSignature,
    #[error("envelope corrupt: {0}")]
    Corrupt(String),
}

fn cryo(e: sagex_crypto::error::SageXCryptoError) -> EnvelopeError {
    EnvelopeError::Crypto(format!("{e:?}"))
}

fn b64e(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

fn b64d(s: &str) -> Result<Vec<u8>, EnvelopeError> {
    B64.decode(s)
        .map_err(|_| EnvelopeError::Corrupt("bad base64".into()))
}

/// Hash-then-sign prehash.
///
/// CRITICAL: rustpq 0.3.0 binds only the first 254 bytes of a signed message
/// (fixed 256-byte internal `msg_prime` buffer in both `sign` and `verify`;
/// see `ml_dsa/sign.rs`). Our signing bodies are kilobytes long, so signing
/// them raw would leave everything past byte 254 UNPROTECTED — tampering
/// with the ciphertext would still verify. Hashing first reduces the signed
/// payload to 32 bytes, fully inside the covered window. Standard practice.
fn prehash_signing_bytes(signing_bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut h = Sha256::new();
    h.update(SIG_PREHASH_DOMAIN);
    h.update(signing_bytes);
    h.finalize().into()
}

/// Derive the 32-byte wrap key from a (possibly hybrid, variable-length)
/// KEM shared secret. Never truncate raw shared secrets.
fn kdf_wrap_key(ss: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut h = Sha256::new();
    h.update(b"sagex-wrap-v1");
    h.update(ss);
    h.finalize().into()
}

/// Seal `plaintext` for `recipients`: fresh random DEK, one KEM wrap per
/// recipient, then sign header+ciphertext with `sign` (caller signs with the
/// sender DSA key; kept as a closure so this crate never touches secrets).
pub fn seal(
    plaintext: &[u8],
    sender: &str,
    watermark_required: bool,
    recipients: &[RecipientKey],
    sign: impl FnOnce(&[u8]) -> Vec<u8>,
) -> Result<SealedEnvelope, EnvelopeError> {
    if recipients.is_empty() {
        return Err(EnvelopeError::NoRecipients);
    }
    let mut dek = [0u8; 32];
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut dek);
    OsRng.fill_bytes(&mut nonce);
    let ciphertext =
        AESHandler::encrypt_raw(&dek, &nonce, plaintext).map_err(cryo)?;

    let mut sealed = Vec::with_capacity(recipients.len());
    for r in recipients {
        let (kem_ct, ss) = Kem::encapsulate_bytes(&r.kem_pub).map_err(cryo)?;
        let ss32 = kdf_wrap_key(&ss);
        let mut wrap_nonce = [0u8; 12];
        OsRng.fill_bytes(&mut wrap_nonce);
        let wrap = AESHandler::encrypt_raw(&ss32, &wrap_nonce, &dek).map_err(cryo)?;
        sealed.push(RecipientEntry {
            identity: r.identity.clone(),
            key_fingerprint: key_fingerprint(&r.kem_pub),
            kem_ct_b64: b64e(&kem_ct),
            wrap_nonce_b64: b64e(&wrap_nonce),
            wrap_b64: b64e(&wrap),
        });
    }

    let nonce_b64 = b64e(&nonce);
    let ciphertext_b64 = b64e(&ciphertext);
    let signing = SigningBody {
        v: ENVELOPE_VERSION,
        sender,
        watermark_required,
        recipients: &sealed,
        nonce_b64: &nonce_b64,
        ciphertext_b64: &ciphertext_b64,
    };
    let signing_bytes = serde_json::to_vec(&signing)
        .map_err(|_| EnvelopeError::Corrupt("signing encode".into()))?;
    let prehash = prehash_signing_bytes(&signing_bytes);
    let signature_b64 = b64e(&sign(&prehash));

    Ok(SealedEnvelope {
        v: ENVELOPE_VERSION,
        sender: sender.to_string(),
        watermark_required,
        recipients: sealed,
        nonce_b64,
        ciphertext_b64,
        signature_b64,
    })
}

/// Verify an envelope against the sender's DSA public key. Returns the
/// verified envelope: header fields are now trustworthy (including
/// `watermark_required`), but NO plaintext has been touched.
pub fn open_verify(
    envelope: &SealedEnvelope,
    sender_dsa_pub: &[u8],
) -> Result<VerifiedEnvelope, EnvelopeError> {
    if envelope.v != ENVELOPE_VERSION {
        return Err(EnvelopeError::Corrupt("unsupported version".into()));
    }
    if envelope.recipients.is_empty() {
        return Err(EnvelopeError::NoRecipients);
    }
    let signing = SigningBody {
        v: envelope.v,
        sender: &envelope.sender,
        watermark_required: envelope.watermark_required,
        recipients: &envelope.recipients,
        nonce_b64: &envelope.nonce_b64,
        ciphertext_b64: &envelope.ciphertext_b64,
    };
    let signing_bytes = serde_json::to_vec(&signing)
        .map_err(|_| EnvelopeError::Corrupt("signing encode".into()))?;
    let prehash = prehash_signing_bytes(&signing_bytes);
    let sig = b64d(&envelope.signature_b64)?;
    sagex_crypto::pqc::dsa::verify_signature_bytes(sender_dsa_pub, &prehash, SIG_CTX, &sig)
        .map_err(|_| EnvelopeError::BadSignature)?;
    Ok(VerifiedEnvelope {
        sender: envelope.sender.clone(),
        watermark_required: envelope.watermark_required,
        recipients: envelope.recipients.clone(),
        nonce_b64: envelope.nonce_b64.clone(),
        ciphertext_b64: envelope.ciphertext_b64.clone(),
    })
}

/// A signature-verified envelope. Plaintext is still untouched — call
/// [`decrypt`] explicitly as the next pipeline step.
#[derive(Debug, Clone)]
pub struct VerifiedEnvelope {
    pub sender: String,
    pub watermark_required: bool,
    pub recipients: Vec<RecipientEntry>,
    nonce_b64: String,
    ciphertext_b64: String,
}

/// Decrypt for `identity` using the recipient's raw KEM secret bytes
/// (obtained by decrypting its own `.prv`). Entry lookup is by identity.
pub fn decrypt(
    verified: &VerifiedEnvelope,
    identity: &str,
    kem_secret: &[u8],
) -> Result<Vec<u8>, EnvelopeError> {
    let entry = verified
        .recipients
        .iter()
        .find(|r| r.identity == identity)
        .ok_or_else(|| EnvelopeError::UnknownRecipient(identity.to_string()))?;
    let kem_ct = b64d(&entry.kem_ct_b64)?;
    let ss = Kem::decapsulate_raw_bytes(kem_secret, &kem_ct).map_err(cryo)?;
    let ss32 = kdf_wrap_key(&ss);
    let wrap_nonce: [u8; 12] = b64d(&entry.wrap_nonce_b64)?
        .as_slice()
        .try_into()
        .map_err(|_| EnvelopeError::Corrupt("bad wrap nonce".into()))?;
    let wrap = b64d(&entry.wrap_b64)?;
    let dek_vec = AESHandler::decrypt_raw(&ss32, &wrap_nonce, &wrap).map_err(cryo)?;
    let dek: [u8; 32] = dek_vec
        .as_slice()
        .try_into()
        .map_err(|_| EnvelopeError::Corrupt("bad DEK unwrap".into()))?;
    let nonce: [u8; 12] = b64d(&verified.nonce_b64)?
        .as_slice()
        .try_into()
        .map_err(|_| EnvelopeError::Corrupt("bad nonce".into()))?;
    let ciphertext = b64d(&verified.ciphertext_b64)?;
    AESHandler::decrypt_raw(&dek, &nonce, &ciphertext).map_err(|_| EnvelopeError::Corrupt("content decrypt failed".into()))
}

/// Serialize an envelope for transport/storage.
pub fn to_bytes(envelope: &SealedEnvelope) -> Vec<u8> {
    serde_json::to_vec(envelope).unwrap_or_default()
}

/// Parse a stored/transmitted envelope (structural only — still unverified).
pub fn from_bytes(bytes: &[u8]) -> Result<SealedEnvelope, EnvelopeError> {
    serde_json::from_slice(bytes).map_err(|_| EnvelopeError::Corrupt("bad envelope JSON".into()))
}
