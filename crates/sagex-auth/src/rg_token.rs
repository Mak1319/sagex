//! RG (gateway) authentication tokens for the CA key directory.
//!
//! Short-lived self-signed JWS objects: the gateway proves itself with its
//! own RG keypair (pinned at the CA at startup), so key-directory lookups
//! need no shared secrets and no syncing. Same JOSE/ML-DSA-65 machinery as
//! permits, distinct `typ` + `aud` so tokens cannot be transplanted across
//! surfaces.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rustpq::ml_dsa::mldsa65;
use serde::{Deserialize, Serialize};
use std::fmt;

use super::permit::{now_secs, PERMIT_ALG};

pub const RG_TYP: &str = "sagex-rg/v1";
pub const RG_AUDIENCE: &str = "sagex-ca";
/// Maximum token lifetime in seconds (short-lived by design).
pub const RG_MAX_TTL_SECS: u64 = 300;

#[derive(Debug)]
pub enum RgError {
    BadFormat,
    BadBase64(base64::DecodeError),
    BadJson(serde_json::Error),
    WrongAlg(String),
    WrongTyp(String),
    ForeignRg { kid: String },
    BadAudience(String),
    Expired,
    NotYetValid,
    TooLongLived,
    BadSignature,
    Crypto(String),
}

impl fmt::Display for RgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadFormat => write!(f, "malformed RG token"),
            Self::BadBase64(e) => write!(f, "token base64 error: {e}"),
            Self::BadJson(e) => write!(f, "token JSON error: {e}"),
            Self::WrongAlg(a) => write!(f, "unexpected token alg: {a}"),
            Self::WrongTyp(t) => write!(f, "unexpected token typ: {t}"),
            Self::ForeignRg { kid } => write!(f, "token not from pinned RG (kid={kid})"),
            Self::BadAudience(a) => write!(f, "wrong token audience: {a}"),
            Self::Expired => write!(f, "token expired"),
            Self::NotYetValid => write!(f, "token not yet valid"),
            Self::TooLongLived => write!(f, "token lifetime exceeds maximum"),
            Self::BadSignature => write!(f, "token signature invalid"),
            Self::Crypto(e) => write!(f, "token crypto error: {e}"),
        }
    }
}

impl std::error::Error for RgError {}

#[derive(Serialize, Deserialize)]
struct Protected {
    alg: String,
    typ: String,
    kid: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RgClaims {
    pub aud: String,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
}

fn b64url(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

fn unb64url(s: &str) -> Result<Vec<u8>, RgError> {
    URL_SAFE_NO_PAD.decode(s).map_err(RgError::BadBase64)
}

fn random_jti() -> String {
    use rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    b64url(&bytes)
}

/// Mint an RG auth token. `ttl_secs` is clamped to [`RG_MAX_TTL_SECS`].
/// `sign` receives the JWS signing input and returns raw signature bytes.
pub fn mint(
    rg_id: &str,
    ttl_secs: u64,
    sign: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
) -> Result<String, RgError> {
    let ttl = ttl_secs.min(RG_MAX_TTL_SECS);
    let now = now_secs();
    let protected = Protected {
        alg: PERMIT_ALG.to_string(),
        typ: RG_TYP.to_string(),
        kid: rg_id.to_string(),
    };
    let claims = RgClaims {
        aud: RG_AUDIENCE.to_string(),
        iat: now,
        exp: now.saturating_add(ttl),
        jti: random_jti(),
    };
    let p1 = b64url(serde_json::to_vec(&protected).map_err(RgError::BadJson)?.as_slice());
    let p2 = b64url(serde_json::to_vec(&claims).map_err(RgError::BadJson)?.as_slice());
    let input = format!("{p1}.{p2}");
    let sig = sign(input.as_bytes()).map_err(RgError::Crypto)?;
    if sig.len() != mldsa65::SIGNATURE_BYTES {
        return Err(RgError::Crypto(format!("bad signature length: {}", sig.len())));
    }
    Ok(format!("{input}.{}", b64url(&sig)))
}

/// Verify an RG token against the pinned gateway identity + key.
pub fn verify(token: &str, rg_id: &str, rg_dsa_public: &[u8]) -> Result<RgClaims, RgError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(RgError::BadFormat);
    }
    let protected: Protected =
        serde_json::from_slice(&unb64url(parts[0])?).map_err(RgError::BadJson)?;
    if protected.alg != PERMIT_ALG {
        return Err(RgError::WrongAlg(protected.alg));
    }
    if protected.typ != RG_TYP {
        return Err(RgError::WrongTyp(protected.typ));
    }
    if protected.kid != rg_id {
        return Err(RgError::ForeignRg { kid: protected.kid });
    }
    let claims: RgClaims =
        serde_json::from_slice(&unb64url(parts[1])?).map_err(RgError::BadJson)?;
    if claims.aud != RG_AUDIENCE {
        return Err(RgError::BadAudience(claims.aud));
    }
    let now = now_secs();
    if now < claims.iat {
        return Err(RgError::NotYetValid);
    }
    if now > claims.exp {
        return Err(RgError::Expired);
    }
    if claims.exp.saturating_sub(claims.iat) > RG_MAX_TTL_SECS {
        return Err(RgError::TooLongLived);
    }
    let sig = unb64url(parts[2])?;
    let pk = mldsa65::PublicKey::from_bytes(rg_dsa_public)
        .map_err(|e| RgError::Crypto(format!("{e:?}")))?;
    let signature =
        mldsa65::Signature::from_bytes(&sig).map_err(|_| RgError::BadSignature)?;
    let input = format!("{}.{}", parts[0], parts[1]);
    mldsa65::verify(&pk, input.as_bytes(), super::permit::PERMIT_CTX, &signature)
        .map_err(|_| RgError::BadSignature)?;
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::OsRng;

    struct TestRg {
        id: String,
        sk: mldsa65::SecretKey,
        pk: Vec<u8>,
    }

    impl TestRg {
        fn new(id: &str) -> Self {
            let (pk, sk) = mldsa65::generate(&mut OsRng);
            Self {
                id: id.into(),
                sk,
                pk: pk.as_bytes().to_vec(),
            }
        }

        fn mint(&self, ttl: u64) -> String {
            mint(&self.id, ttl, |input| {
                mldsa65::sign(&self.sk, input, super::super::permit::PERMIT_CTX, &mut OsRng)
                    .map(|s| s.as_bytes().to_vec())
                    .map_err(|e| format!("{e:?}"))
            })
            .unwrap()
        }
    }

    #[test]
    fn roundtrip_ok() {
        let rg = TestRg::new("sagex-gateway-01");
        let tok = rg.mint(60);
        let claims = verify(&tok, "sagex-gateway-01", &rg.pk).unwrap();
        assert_eq!(claims.aud, RG_AUDIENCE);
    }

    #[test]
    fn rejects_foreign_audience_tamper_expiry() {
        let rg = TestRg::new("sagex-gateway-01");
        let other = TestRg::new("other-gw");
        // Foreign RG id.
        assert!(matches!(
            verify(&other.mint(60), "sagex-gateway-01", &other.pk),
            Err(RgError::ForeignRg { .. })
        ));
        // Wrong key under right id.
        assert!(matches!(
            verify(&other.mint(60), "other-gw", &rg.pk),
            Err(RgError::BadSignature)
        ));
        // Tampered payload.
        let tok = rg.mint(60);
        let mut parts: Vec<String> = tok.split('.').map(|s| s.to_string()).collect();
        let flip = if parts[1].starts_with('A') { "B" } else { "A" };
        parts[1].replace_range(0..1, flip);
        assert!(verify(&parts.join("."), "sagex-gateway-01", &rg.pk).is_err());
        // A permit token is not an RG token (typ pin).
        let permit = super::super::permit::mint(
            "sagex-ca",
            "alice",
            super::super::permit::PERMIT_KIND_USER,
            60,
            |input| {
                mldsa65::sign(&rg.sk, input, super::super::permit::PERMIT_CTX, &mut OsRng)
                    .map(|s| s.as_bytes().to_vec())
                    .map_err(|e| format!("{e:?}"))
            },
        )
        .unwrap();
        assert!(matches!(
            verify(&permit, "sagex-gateway-01", &rg.pk),
            Err(RgError::WrongTyp(_))
        ));
    }
}
