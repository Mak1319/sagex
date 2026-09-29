//! Verification of chatsrv (BLS) JOSE access tokens by third parties
//! (certauth permit issuance / CSR triple bind).
//!
//! Chatsrv tokens are JWS-compact, `alg: "ML-DSA-65"` (RFC 9964), signed
//! with the chatsrv key over `b64u(header).b64u(payload)` with domain
//! separation context `b"sagex-chatsrv/jose-mldsa65-v1"`. The verifier pins
//! the expected `kid` and requires `purpose == "access"` plus a present,
//! well-formed `username` claim (Phase A — no backward compatibility:
//! tokens without it fail closed).

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rustpq::ml_dsa::mldsa65;
use serde::Deserialize;
use std::fmt;

use super::username::valid_username;

/// Domain separation bound into every chatsrv ML-DSA signature.
/// MUST match `sagex-chatsrv/src/jose_mldsa.rs::SIG_CONTEXT`.
pub const CHATSRV_SIG_CTX: &[u8] = b"sagex-chatsrv/jose-mldsa65-v1";
pub const CHATSRV_ALG: &str = "ML-DSA-65";

#[derive(Debug)]
pub enum ChatsrvTokenError {
    BadFormat,
    BadBase64(base64::DecodeError),
    BadJson(serde_json::Error),
    WrongAlg(String),
    WrongKid { want: String, got: String },
    WrongPurpose(String),
    Expired,
    NotYetValid,
    MissingUsername,
    BadUsername(String),
    BadSignature,
    BadPublicKey(String),
}

impl fmt::Display for ChatsrvTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadFormat => write!(f, "malformed chatsrv token"),
            Self::BadBase64(e) => write!(f, "token base64 error: {e}"),
            Self::BadJson(e) => write!(f, "token JSON error: {e}"),
            Self::WrongAlg(a) => write!(f, "unexpected token alg: {a}"),
            Self::WrongKid { want, got } => {
                write!(f, "token kid {got:?} does not match pinned {want:?}")
            }
            Self::WrongPurpose(p) => write!(f, "wrong token purpose: {p}"),
            Self::Expired => write!(f, "token expired"),
            Self::NotYetValid => write!(f, "token not yet valid"),
            Self::MissingUsername => write!(f, "token has no username claim"),
            Self::BadUsername(u) => write!(f, "bad username claim: {u}"),
            Self::BadSignature => write!(f, "token signature invalid"),
            Self::BadPublicKey(e) => write!(f, "bad chatsrv public key: {e}"),
        }
    }
}

impl std::error::Error for ChatsrvTokenError {}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    username: Option<String>,
    iat: u64,
    exp: u64,
    purpose: String,
}

fn unb64url(s: &str) -> Result<Vec<u8>, ChatsrvTokenError> {
    URL_SAFE_NO_PAD.decode(s).map_err(ChatsrvTokenError::BadBase64)
}

/// Verify a chatsrv access token against the pinned key. Returns
/// `(sub, username)` on success.
pub fn verify(
    token: &str,
    chatsrv_dsa_public: &[u8],
    expected_kid: &str,
) -> Result<(String, String), ChatsrvTokenError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ChatsrvTokenError::BadFormat);
    }
    let header: Header =
        serde_json::from_slice(&unb64url(parts[0])?).map_err(ChatsrvTokenError::BadJson)?;
    if header.alg != CHATSRV_ALG {
        return Err(ChatsrvTokenError::WrongAlg(header.alg));
    }
    if header.kid != expected_kid {
        return Err(ChatsrvTokenError::WrongKid {
            want: expected_kid.to_string(),
            got: header.kid,
        });
    }
    let claims: Claims =
        serde_json::from_slice(&unb64url(parts[1])?).map_err(ChatsrvTokenError::BadJson)?;
    if claims.purpose != "access" {
        return Err(ChatsrvTokenError::WrongPurpose(claims.purpose));
    }
    let username = claims.username.ok_or(ChatsrvTokenError::MissingUsername)?;
    if !valid_username(&username) {
        return Err(ChatsrvTokenError::BadUsername(username));
    }
    let now = super::permit::now_secs();
    if now < claims.iat {
        return Err(ChatsrvTokenError::NotYetValid);
    }
    if now > claims.exp {
        return Err(ChatsrvTokenError::Expired);
    }
    let sig = unb64url(parts[2])?;
    let pk = mldsa65::PublicKey::from_bytes(chatsrv_dsa_public)
        .map_err(|e| ChatsrvTokenError::BadPublicKey(format!("{e:?}")))?;
    let signature =
        mldsa65::Signature::from_bytes(&sig).map_err(|_| ChatsrvTokenError::BadSignature)?;
    let input = format!("{}.{}", parts[0], parts[1]);
    mldsa65::verify(&pk, input.as_bytes(), CHATSRV_SIG_CTX, &signature)
        .map_err(|_| ChatsrvTokenError::BadSignature)?;
    Ok((claims.sub, username))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::OsRng;

    struct TestChatsrv {
        kid: String,
        sk: mldsa65::SecretKey,
        pk: Vec<u8>,
    }

    impl TestChatsrv {
        fn mint(&self, username: Option<&str>, purpose: &str, ttl: i64) -> String {
            let now = super::super::permit::now_secs() as i64;
            let header = serde_json::json!({"alg": CHATSRV_ALG, "typ": "JWT", "kid": self.kid});
            let mut payload = serde_json::json!({
                "sub": "0123456789abcdef01234567",
                "iat": now, "exp": now + ttl,
                "jti": "test-jti", "purpose": purpose,
            });
            if let Some(u) = username {
                payload["username"] = serde_json::Value::String(u.into());
            }
            let p1 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
            let p2 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
            let input = format!("{p1}.{p2}");
            let sig =
                mldsa65::sign(&self.sk, input.as_bytes(), CHATSRV_SIG_CTX, &mut OsRng).unwrap();
            format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.as_bytes()))
        }
    }

    fn issuer(kid: &str) -> TestChatsrv {
        let (pk, sk) = mldsa65::generate(&mut OsRng);
        TestChatsrv {
            kid: kid.into(),
            sk,
            pk: pk.as_bytes().to_vec(),
        }
    }

    #[test]
    fn roundtrip_ok() {
        let c = issuer("test-kid");
        let tok = c.mint(Some("alice"), "access", 900);
        let (sub, user) = verify(&tok, &c.pk, "test-kid").unwrap();
        assert_eq!(sub, "0123456789abcdef01234567");
        assert_eq!(user, "alice");
    }

    #[test]
    fn rejects_missing_username_wrong_kid_purpose_expiry_tamper() {
        let c = issuer("test-kid");
        let other = issuer("other-kid");
        // Missing username fails closed (no backward compat).
        assert!(matches!(
            verify(&c.mint(None, "access", 900), &c.pk, "test-kid"),
            Err(ChatsrvTokenError::MissingUsername)
        ));
        // Wrong kid.
        assert!(matches!(
            verify(&other.mint(Some("alice"), "access", 900), &other.pk, "test-kid"),
            Err(ChatsrvTokenError::WrongKid { .. })
        ));
        // Wrong purpose (refresh token at an access gate).
        assert!(matches!(
            verify(&c.mint(Some("alice"), "refresh", 900), &c.pk, "test-kid"),
            Err(ChatsrvTokenError::WrongPurpose(_))
        ));
        // Expired.
        let tok = c.mint(Some("alice"), "access", 0);
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(matches!(
            verify(&tok, &c.pk, "test-kid"),
            Err(ChatsrvTokenError::Expired)
        ));
        // Tampered payload.
        let tok = c.mint(Some("alice"), "access", 900);
        let mut parts: Vec<String> = tok.split('.').map(|s| s.to_string()).collect();
        let flip = if parts[1].starts_with('A') { "B" } else { "A" };
        parts[1].replace_range(0..1, flip);
        assert!(verify(&parts.join("."), &c.pk, "test-kid").is_err());
        // Wrong key: token minted by `other` under its own kid, verified
        // against a different key with the kid pin satisfied -> signature
        // check is what fails.
        let foreign = other.mint(Some("alice"), "access", 900);
        assert!(matches!(
            verify(&foreign, &c.pk, "other-kid"),
            Err(ChatsrvTokenError::BadSignature)
        ));
        // And the positive control: other's own key verifies.
        let (_, user) = verify(&foreign, &other.pk, "other-kid").unwrap();
        assert_eq!(user, "alice");
    }
}
