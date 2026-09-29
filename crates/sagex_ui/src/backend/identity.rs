//! Device identity: `sagex-format` keypair generation (mirroring
//! `sagex_test`) plus CSR body construction with a proof-of-possession
//! self-signature.
//!
//! The canonical PoP bytes replicate `sagex-certauth/src/csr.rs` EXACTLY —
//! any drift here means the CA rejects the CSR:
//! `b"sagex-pop-v1" || u32LE(identity.len) || identity
//!  || u32LE(kem.len) || kem || dsa`, signed with ML-DSA context `b""`.

use base64::{Engine, engine::general_purpose::STANDARD};
use sagex_crypto::{
    aes::{ITERATIONS, KeyEncapsulation},
    pqc::{dsa, kem},
};
use sagex_format::format::{
    KeyDerivatinMechanism::Password, MAGIC_NUMBER, PrivateExternal, PrivateInternal,
    PublicFileFormatExternal, PublicInternal, VERSION,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Domain separator for PoP messages. MUST match certauth `POP_TAG`.
pub const POP_TAG: &[u8] = b"sagex-pop-v1";
/// ML-DSA context for PoP signatures. MUST match certauth `CTX`.
pub const POP_CTX: &[u8] = b"";

#[derive(Debug)]
pub enum IdentityError {
    BadIdentity(String),
    Crypto(String),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadIdentity(s) => write!(f, "bad identity: {s}"),
            Self::Crypto(e) => write!(f, "identity crypto: {e}"),
        }
    }
}

impl std::error::Error for IdentityError {}

pub type IdentityResult<T> = Result<T, IdentityError>;

/// CSR body with the exact JSON field names the CA expects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsrBody {
    pub identity: String,
    pub key_kem_b64: String,
    pub key_dsa_b64: String,
    pub self_sig_b64: String,
}

/// Generate a fresh `.prv`/`.pub` pair for `user_name`, password-protecting
/// the secrets via KDF exactly like `sagex_test` (and the CA itself).
pub fn generate_identity(
    password: &[u8],
    user_name: &str,
) -> IdentityResult<(PrivateExternal, PublicFileFormatExternal)> {
    if !sagex_auth::valid_username(user_name) {
        return Err(IdentityError::BadIdentity(user_name.to_string()));
    }
    let pw = Zeroizing::new(password.to_vec());
    let (kem_enc, kem_pk) = kem::KeyGen::generate_from_password(&pw)
        .map_err(|e| IdentityError::Crypto(format!("{e:?}")))?;
    let (dsa_enc, dsa_pk) = dsa::KeyGen::generate_from_password(&pw)
        .map_err(|e| IdentityError::Crypto(format!("{e:?}")))?;
    let internal = PrivateInternal {
        magic_number: MAGIC_NUMBER,
        version: VERSION,
        iteration_count: ITERATIONS as usize,
        key_derivation_mechanism_kem: Password,
        key_derivation_mechanism_dsa: Password,
        key_encapsulation_kem: kem_enc,
        key_encapsulation_dsa: dsa_enc,
        user_name: user_name.to_string(),
    };
    let prv = PrivateExternal {
        internal,
        identity: user_name.as_bytes().to_vec(),
    };
    let publ = PublicFileFormatExternal {
        internal: PublicInternal {
            magic_number: MAGIC_NUMBER,
            version: VERSION,
            key_kem: kem_pk,
            key_dsa: dsa_pk,
            user_name: user_name.to_string(),
        },
        ecc: vec![],
        identity: user_name.as_bytes().to_vec(),
    };
    Ok((prv, publ))
}

/// Canonical PoP message. Byte-identical to certauth's `pop_message`.
pub fn pop_message(identity: &str, key_kem: &[u8], key_dsa: &[u8]) -> Vec<u8> {
    let mut m =
        Vec::with_capacity(POP_TAG.len() + 8 + identity.len() + key_kem.len() + key_dsa.len());
    m.extend_from_slice(POP_TAG);
    m.extend_from_slice(&(identity.len() as u32).to_le_bytes());
    m.extend_from_slice(identity.as_bytes());
    m.extend_from_slice(&(key_kem.len() as u32).to_le_bytes());
    m.extend_from_slice(key_kem);
    m.extend_from_slice(key_dsa);
    m
}

/// Build the CSR body: base64 keys plus a PoP self-signature made with the
/// device DSA key. `dsa_encap` is consumed (by value, like the server API);
/// callers holding a loaded `.prv` move
/// `prv.internal.key_encapsulation_dsa` out of it.
pub fn build_csr_body(
    password: &[u8],
    user_name: &str,
    key_kem: &[u8],
    key_dsa: &[u8],
    dsa_encap: KeyEncapsulation,
) -> IdentityResult<CsrBody> {
    if !sagex_auth::valid_username(user_name) {
        return Err(IdentityError::BadIdentity(user_name.to_string()));
    }
    let pw = Zeroizing::new(password.to_vec());
    let msg = pop_message(user_name, key_kem, key_dsa);
    let sig = dsa::Implement::sign_from_password(dsa_encap, &pw, &msg, POP_CTX)
        .map_err(|e| IdentityError::Crypto(format!("{e:?}")))?;
    Ok(CsrBody {
        identity: user_name.to_string(),
        key_kem_b64: STANDARD.encode(key_kem),
        key_dsa_b64: STANDARD.encode(key_dsa),
        self_sig_b64: STANDARD.encode(sig.as_bytes()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_then_csr_verifies_like_the_server() {
        let pw = b"test-password-1";
        let (prv, publ) = generate_identity(pw, "alice").unwrap();
        assert_eq!(prv.internal.user_name, "alice");
        assert_eq!(publ.internal.user_name, "alice");
        assert!(!publ.internal.key_kem.is_empty());
        assert!(!publ.internal.key_dsa.is_empty());

        // Move the encap out of an owned struct, like enroll does.
        let encap = prv.internal.key_encapsulation_dsa;
        let body = build_csr_body(
            pw,
            "alice",
            &publ.internal.key_kem,
            &publ.internal.key_dsa,
            encap,
        )
        .unwrap();
        assert_eq!(body.identity, "alice");

        // Mirror the server's checks: lengths, trial-KEM, PoP verify.
        // Lengths come from the rustpq constants (never hardcoded: the
        // server validates against the same constants).
        use rustpq::ml_dsa::mldsa65;
        let kem = STANDARD.decode(&body.key_kem_b64).unwrap();
        let dsa_pk = STANDARD.decode(&body.key_dsa_b64).unwrap();
        let sig = STANDARD.decode(&body.self_sig_b64).unwrap();
        assert_eq!(dsa_pk.len(), mldsa65::PUBLIC_KEY_BYTES);
        assert_eq!(sig.len(), mldsa65::SIGNATURE_BYTES);
        sagex_crypto::pqc::kem::Implementation::encapsulate(kem.clone()).unwrap();
        let msg = pop_message("alice", &kem, &dsa_pk);
        sagex_crypto::pqc::dsa::verify_signature(dsa_pk, &msg, POP_CTX, rustpq_sig(&sig)).unwrap();
    }

    fn rustpq_sig(bytes: &[u8]) -> rustpq::ml_dsa::mldsa65::Signature {
        rustpq::ml_dsa::mldsa65::Signature::from_bytes(bytes).unwrap()
    }

    #[test]
    fn rejects_bad_identity() {
        assert!(generate_identity(b"pw", "ab").is_err());
        assert!(generate_identity(b"pw", "evil;cn=x").is_err());
    }
}
