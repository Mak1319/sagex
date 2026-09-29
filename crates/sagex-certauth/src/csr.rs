//! CSR validation: base64/length checks, trial KEM encapsulation, and
//! proof-of-possession (PoP) verification of the client's ML-DSA key.
//!
//! The PoP message is domain-separated (`sagex-pop-v1`) and length-prefixed,
//! binding identity + both public keys so a signature cannot be transplanted
//! across identities or keys.

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{
    ca::valid_identity,
    mldsa::{DSA_PK_LEN, DSA_SIG_LEN, KEM_PK_MAX_LEN, verify_raw},
};

/// Domain separator for PoP messages (v1).
pub const POP_TAG: &[u8] = b"sagex-pop-v1";

#[derive(Debug)]
pub enum CsrError {
    BadIdentity,
    BadBase64(&'static str),
    BadLength(&'static str),
    BadKemKey,
    BadPoP,
}

impl fmt::Display for CsrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadIdentity => write!(f, "invalid identity"),
            Self::BadBase64(w) => write!(f, "invalid base64 in {w}"),
            Self::BadLength(w) => write!(f, "invalid length in {w}"),
            Self::BadKemKey => write!(f, "ML-KEM public key rejected"),
            Self::BadPoP => write!(f, "proof-of-possession signature invalid"),
        }
    }
}

impl std::error::Error for CsrError {}

#[derive(Deserialize)]
pub struct CsrRequest {
    pub permit: String,
    pub csr: CsrBody,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct CsrBody {
    pub identity: String,
    pub key_kem_b64: String,
    pub key_dsa_b64: String,
    pub self_sig_b64: String,
}

/// Canonical PoP message: TAG || len||identity || len||kem || dsa.
pub fn pop_message(identity: &str, key_kem: &[u8], key_dsa: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(
        POP_TAG.len() + 8 + identity.len() + key_kem.len() + key_dsa.len(),
    );
    m.extend_from_slice(POP_TAG);
    m.extend_from_slice(&(identity.len() as u32).to_le_bytes());
    m.extend_from_slice(identity.as_bytes());
    m.extend_from_slice(&(key_kem.len() as u32).to_le_bytes());
    m.extend_from_slice(key_kem);
    m.extend_from_slice(key_dsa);
    m
}

fn unb64(field: &'static str, s: &str) -> Result<Vec<u8>, CsrError> {
    STANDARD.decode(s).map_err(|_| CsrError::BadBase64(field))
}

/// Validated, decoded CSR material ready for enrollment.
pub struct ValidCsr {
    pub identity: String,
    pub key_kem: Vec<u8>,
    pub key_dsa: Vec<u8>,
}

pub fn validate(body: &CsrBody) -> Result<ValidCsr, CsrError> {
    if !valid_identity(&body.identity) {
        return Err(CsrError::BadIdentity);
    }
    let key_kem = unb64("key_kem_b64", &body.key_kem_b64)?;
    let key_dsa = unb64("key_dsa_b64", &body.key_dsa_b64)?;
    let self_sig = unb64("self_sig_b64", &body.self_sig_b64)?;
    if key_kem.is_empty() || key_kem.len() > KEM_PK_MAX_LEN {
        return Err(CsrError::BadLength("key_kem_b64"));
    }
    if key_dsa.len() != DSA_PK_LEN {
        return Err(CsrError::BadLength("key_dsa_b64"));
    }
    if self_sig.len() != DSA_SIG_LEN {
        return Err(CsrError::BadLength("self_sig_b64"));
    }
    // Cryptographic KEM validation: trial encapsulation proves the bytes are
    // a well-formed public key for our parameter set (and usable for it).
    if sagex_crypto::pqc::kem::Implementation::encapsulate(key_kem.clone()).is_err() {
        return Err(CsrError::BadKemKey);
    }
    let msg = pop_message(&body.identity, &key_kem, &key_dsa);
    if !verify_raw(&key_dsa, &msg, &self_sig) {
        return Err(CsrError::BadPoP);
    }
    Ok(ValidCsr {
        identity: body.identity.clone(),
        key_kem,
        key_dsa,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sagex_crypto::pqc::{dsa, kem};

    #[test]
    fn pop_roundtrip_ok() {
        let pw = b"client-password";
        let (_k_enc, k_pk) = kem::KeyGen::generate_from_password(pw).unwrap();
        let (d_enc, d_pk) = dsa::KeyGen::generate_from_password(pw).unwrap();
        let msg = pop_message("alice", &k_pk, &d_pk);
        let sig =
            dsa::Implement::sign_from_password(d_enc, pw, &msg, crate::mldsa::CTX).unwrap();
        let body = CsrBody {
            identity: "alice".into(),
            key_kem_b64: STANDARD.encode(&k_pk),
            key_dsa_b64: STANDARD.encode(&d_pk),
            self_sig_b64: STANDARD.encode(sig.as_bytes()),
        };
        let valid = validate(&body).expect("valid CSR must pass");
        assert_eq!(valid.identity, "alice");
        assert_eq!(valid.key_dsa, d_pk);
    }

    #[test]
    fn validate_rejects_transplanted_pop() {
        // PoP for "alice" must not validate as a CSR for "bob".
        let pw = b"client-password";
        let (_k_enc, k_pk) = kem::KeyGen::generate_from_password(pw).unwrap();
        let (d_enc, d_pk) = dsa::KeyGen::generate_from_password(pw).unwrap();
        let msg = pop_message("alice", &k_pk, &d_pk);
        let sig =
            dsa::Implement::sign_from_password(d_enc, pw, &msg, crate::mldsa::CTX).unwrap();
        let body = CsrBody {
            identity: "bob".into(),
            key_kem_b64: STANDARD.encode(&k_pk),
            key_dsa_b64: STANDARD.encode(&d_pk),
            self_sig_b64: STANDARD.encode(sig.as_bytes()),
        };
        assert!(matches!(validate(&body), Err(CsrError::BadPoP)));
    }

    #[test]
    fn validate_rejects_bad_identity() {
        let body = CsrBody {
            identity: "evil;cn=x".into(),
            key_kem_b64: String::new(),
            key_dsa_b64: String::new(),
            self_sig_b64: String::new(),
        };
        assert!(matches!(validate(&body), Err(CsrError::BadIdentity)));
    }
}
