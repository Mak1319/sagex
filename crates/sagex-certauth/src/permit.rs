//! Server-issued JOSE permits (JWS-shaped, ML-DSA-signed).
//!
//! A permit proves its holder is a customer of *this* server: it is minted
//! offline with the CA private key and verified at CSR time against the
//! loaded CA public key. `kid`/`iss` must equal this server's CA identity,
//! so permits minted by any other server are rejected.
//!
//! NOTE: `"alg":"ML-DSA-65"` is a sagex extension. No RFC JOSE registry
//! defines ML-DSA, so generic JOSE libraries can parse but not verify these
//! tokens; verification must use [`verify`].

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use signature::Signer;
use std::{fmt, time::{SystemTime, UNIX_EPOCH}};

use crate::{
    ca::CaMaterial,
    mldsa::{MlDsa65Signer, verify_raw},
};

pub const PERMIT_ALG: &str = "ML-DSA-65";
pub const PERMIT_TYP: &str = "sagex-permit/v1";

#[derive(Debug)]
pub enum PermitError {
    BadFormat,
    BadBase64(base64::DecodeError),
    BadJson(serde_json::Error),
    WrongAlg(String),
    ForeignServer { kid: String, iss: String },
    Expired,
    NotYetValid,
    BadSignature,
    Crypto(String),
}

impl fmt::Display for PermitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadFormat => write!(f, "malformed permit token"),
            Self::BadBase64(e) => write!(f, "permit base64 error: {e}"),
            Self::BadJson(e) => write!(f, "permit JSON error: {e}"),
            Self::WrongAlg(a) => write!(f, "unexpected permit alg: {a}"),
            Self::ForeignServer { kid, iss } => {
                write!(f, "permit not issued by this server (kid={kid}, iss={iss})")
            }
            Self::Expired => write!(f, "permit expired"),
            Self::NotYetValid => write!(f, "permit not yet valid"),
            Self::BadSignature => write!(f, "permit signature invalid"),
            Self::Crypto(e) => write!(f, "permit crypto error: {e}"),
        }
    }
}

impl std::error::Error for PermitError {}

#[derive(Serialize, Deserialize)]
struct Protected {
    alg: String,
    typ: String,
    kid: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PermitClaims {
    pub sub: String,
    pub iss: String,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs()
}

fn b64url(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

fn unb64url(s: &str) -> Result<Vec<u8>, PermitError> {
    URL_SAFE_NO_PAD.decode(s).map_err(PermitError::BadBase64)
}

fn random_jti() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    b64url(&bytes)
}

/// Mint a permit for `subject`, valid for `ttl_secs` from now.
pub fn mint(
    ca: &CaMaterial,
    signer: &MlDsa65Signer,
    subject: &str,
    ttl_secs: u64,
) -> Result<String, PermitError> {
    let now = now_secs();
    let protected = Protected {
        alg: PERMIT_ALG.to_string(),
        typ: PERMIT_TYP.to_string(),
        kid: ca.identity.clone(),
    };
    let claims = PermitClaims {
        sub: subject.to_string(),
        iss: ca.identity.clone(),
        iat: now,
        exp: now.saturating_add(ttl_secs),
        jti: random_jti(),
    };
    let p1 = b64url(serde_json::to_vec(&protected).map_err(PermitError::BadJson)?.as_slice());
    let p2 = b64url(serde_json::to_vec(&claims).map_err(PermitError::BadJson)?.as_slice());
    let input = format!("{p1}.{p2}");
    let sig = signer
        .try_sign(input.as_bytes())
        .map_err(|e| PermitError::Crypto(format!("{e:?}")))?;
    Ok(format!("{input}.{}", b64url(&sig.0)))
}

/// Verify a permit against the loaded CA. Returns the claims on success.
/// The `kid`/`iss` pin is what makes this a *permitted-server* check rather
/// than a generic signature check.
pub fn verify(
    token: &str,
    ca_identity: &str,
    ca_dsa_public: &[u8],
) -> Result<PermitClaims, PermitError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(PermitError::BadFormat);
    }
    let protected: Protected =
        serde_json::from_slice(&unb64url(parts[0])?).map_err(PermitError::BadJson)?;
    if protected.alg != PERMIT_ALG {
        return Err(PermitError::WrongAlg(protected.alg));
    }
    let claims: PermitClaims =
        serde_json::from_slice(&unb64url(parts[1])?).map_err(PermitError::BadJson)?;
    if protected.kid != ca_identity || claims.iss != ca_identity {
        return Err(PermitError::ForeignServer {
            kid: protected.kid,
            iss: claims.iss,
        });
    }
    let now = now_secs();
    if now < claims.iat {
        return Err(PermitError::NotYetValid);
    }
    if now > claims.exp {
        return Err(PermitError::Expired);
    }
    let sig = unb64url(parts[2])?;
    let input = format!("{}.{}", parts[0], parts[1]);
    if !verify_raw(ca_dsa_public, input.as_bytes(), &sig) {
        return Err(PermitError::BadSignature);
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ca::CaMaterial;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_ca(dir_suffix: &str, identity: &str) -> (CaMaterial, MlDsa65Signer) {
        let dir = std::env::temp_dir().join(format!(
            "sagex-permit-test-{}-{}-{}",
            dir_suffix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (ca, _) =
            CaMaterial::load_or_generate(&dir, b"test-password", identity).unwrap();
        let signer = ca.signer(b"test-password").unwrap();
        (ca, signer)
    }

    #[test]
    fn roundtrip_ok() {
        let (ca, signer) = test_ca("a", "test-ca");
        let tok = mint(&ca, &signer, "alice", 3600).unwrap();
        let claims = verify(&tok, &ca.identity, &ca.dsa_public).unwrap();
        assert_eq!(claims.sub, "alice");
        assert_eq!(claims.iss, "test-ca");
    }

    #[test]
    fn foreign_server_rejected() {
        let (ca_a, signer_a) = test_ca("b1", "server-a");
        let (_ca_b, _) = test_ca("b2", "server-b");
        let tok = mint(&ca_a, &signer_a, "alice", 3600).unwrap();
        assert!(matches!(
            verify(&tok, "server-b", &ca_a.dsa_public),
            Err(PermitError::ForeignServer { .. })
        ));
    }

    #[test]
    fn expired_rejected() {
        let (ca, signer) = test_ca("c", "test-ca");
        let tok = mint(&ca, &signer, "alice", 0).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(matches!(verify(&tok, &ca.identity, &ca.dsa_public), Err(PermitError::Expired)));
    }

    #[test]
    fn tampered_payload_rejected() {
        let (ca, signer) = test_ca("d", "test-ca");
        let tok = mint(&ca, &signer, "alice", 3600).unwrap();
        let mut parts: Vec<String> = tok.split('.').map(|s| s.to_string()).collect();
        let flip = if parts[1].starts_with('A') { "B" } else { "A" };
        parts[1].replace_range(0..1, flip);
        let bad = parts.join(".");
        assert!(verify(&bad, &ca.identity, &ca.dsa_public).is_err());
    }
}
