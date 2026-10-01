//! `sagex-forensic`: sealed envelopes with signed watermark flags,
//! recipient-unique watermark derivation, and Office embedding.
//!
//! Pipeline discipline (enforced by API shape, tested by the no-plaintext
//! invariant): verify -> register -> decrypt -> embed -> store. Nothing in
//! this crate writes files; callers own storage.

pub mod doc;
pub mod envelope;
pub mod watermark;

pub use doc::{embed_watermark, extract_watermark_id, is_watermarkable, DocError};
pub use envelope::{
    decrypt, from_bytes, key_fingerprint, open_verify, seal, to_bytes, EnvelopeError,
    RecipientEntry, RecipientKey, SealedEnvelope, VerifiedEnvelope, ENVELOPE_VERSION, SIG_CTX,
};
pub use watermark::{
    derive_watermark_id, sha256_bytes, sha256_hex, WatermarkError, WM_DOMAIN, WM_ID_BYTES,
    WM_PREFIX,
};
