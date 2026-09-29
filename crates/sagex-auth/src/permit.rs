//! Server-issued JOSE permits (JWS-shaped, ML-DSA-signed).
//!
//! A permit proves its holder is a customer of *a specific* server: it is
//! minted offline with that server's ML-DSA private key and verified against
//! the server's public key. `kid`/`iss` must equal the issuing server's
//! identity, so permits minted by any other server are rejected.
//!
//! NOTE: `"alg":"ML-DSA-65"` is a sagex extension. No RFC JOSE registry
//! defines ML-DSA, so generic JOSE libraries can parse but not verify these
//! tokens; verification must use [`verify`].
//!
//! Minting is key-agnostic: the caller supplies the raw signature bytes via
//! `sign` (sagex-certauth passes a closure over its CA signer), so this
//! crate never touches private key material.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand_core::{OsRng, RngCore};
use rustpq::ml_dsa::mldsa65;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

pub const PERMIT_ALG: &str = "ML-DSA-65";
pub const PERMIT_TYP: &str = "sagex-permit/v1";

/// Raw ML-DSA-65 public key length in bytes. Verifiers use this to fail fast
/// on misconfigured keys instead of failing the first request.
pub const DSA_PUBLIC_KEY_BYTES: usize = mldsa65::PUBLIC_KEY_BYTES;

/// FIPS 204 context mixed into permit signatures. Empty: the signed payload
/// (JWS signing input) is already domain-separated by construction.
pub const PERMIT_CTX: &[u8] = b"";

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
    BadKind(String),
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
            Self::BadKind(k) => write!(f, "bad permit kind: {k}"),
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
    /// Permit jurisdiction: `"user"` (enrollment + ledger intake) or
    /// `"server"` (service surfaces like auditor logs). Gates enforce it;
    /// a server permit can never enroll a user certificate. Required —
    /// kindless legacy permits fail closed.
    pub kind: String,
}

/// The two permit jurisdictions. See [`PermitClaims::kind`].
pub const PERMIT_KIND_USER: &str = "user";
pub const PERMIT_KIND_SERVER: &str = "server";

fn valid_kind(kind: &str) -> bool {
    kind == PERMIT_KIND_USER || kind == PERMIT_KIND_SERVER
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
/// `kind` must be [`PERMIT_KIND_USER`] or [`PERMIT_KIND_SERVER`].
///
/// `sign` receives the JWS signing input (`b64u(protected).b64u(payload)`)
/// and must return raw ML-DSA-65 signature bytes made with the issuing
/// server's private key.
pub fn mint(
    server_identity: &str,
    subject: &str,
    kind: &str,
    ttl_secs: u64,
    sign: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
) -> Result<String, PermitError> {
    if !valid_kind(kind) {
        return Err(PermitError::BadKind(kind.to_string()));
    }
    let now = now_secs();
    let protected = Protected {
        alg: PERMIT_ALG.to_string(),
        typ: PERMIT_TYP.to_string(),
        kid: server_identity.to_string(),
    };
    let claims = PermitClaims {
        sub: subject.to_string(),
        iss: server_identity.to_string(),
        iat: now,
        exp: now.saturating_add(ttl_secs),
        jti: random_jti(),
        kind: kind.to_string(),
    };
    let p1 = b64url(serde_json::to_vec(&protected).map_err(PermitError::BadJson)?.as_slice());
    let p2 = b64url(serde_json::to_vec(&claims).map_err(PermitError::BadJson)?.as_slice());
    let input = format!("{p1}.{p2}");
    let sig = sign(input.as_bytes()).map_err(PermitError::Crypto)?;
    if sig.len() != mldsa65::SIGNATURE_BYTES {
        return Err(PermitError::Crypto(format!(
            "bad signature length: {}",
            sig.len()
        )));
    }
    Ok(format!("{input}.{}", b64url(&sig)))
}

/// Verify a permit against the issuing server's identity and ML-DSA-65
/// public key. Returns the claims on success. The `kid`/`iss` pin is what
/// makes this a *permitted-server* check rather than a generic signature
/// check.
pub fn verify(
    token: &str,
    server_identity: &str,
    server_dsa_public: &[u8],
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
    // Required field: kindless legacy permits fail closed here as BadJson
    // ("missing field `kind`"). Unknown values fail as BadKind below.
    if !valid_kind(&claims.kind) {
        return Err(PermitError::BadKind(claims.kind));
    }
    if protected.kid != server_identity || claims.iss != server_identity {
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
    let pk = mldsa65::PublicKey::from_bytes(server_dsa_public)
        .map_err(|e| PermitError::Crypto(format!("{e:?}")))?;
    let signature = mldsa65::Signature::from_bytes(&sig)
        .map_err(|_| PermitError::BadSignature)?;
    mldsa65::verify(&pk, input.as_bytes(), PERMIT_CTX, &signature)
        .map_err(|_| PermitError::BadSignature)?;
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ephemeral test issuer: fresh ML-DSA-65 keypair, no passwords involved.
    struct TestIssuer {
        identity: String,
        sk: mldsa65::SecretKey,
        pk: Vec<u8>,
    }

    impl TestIssuer {
        fn new(identity: &str) -> Self {
            let (pk, sk) = mldsa65::generate(&mut OsRng);
            Self {
                identity: identity.to_string(),
                sk,
                pk: pk.as_bytes().to_vec(),
            }
        }

        fn mint(&self, subject: &str, kind: &str, ttl_secs: u64) -> String {
            mint(&self.identity, subject, kind, ttl_secs, |input| {
                mldsa65::sign(&self.sk, input, PERMIT_CTX, &mut OsRng)
                    .map(|s| s.as_bytes().to_vec())
                    .map_err(|e| format!("{e:?}"))
            })
            .expect("mint")
        }

        fn verify(&self, token: &str) -> Result<PermitClaims, PermitError> {
            verify(token, &self.identity, &self.pk)
        }
    }

    #[test]
    fn roundtrip_ok() {
        let ca = TestIssuer::new("test-ca");
        let tok = ca.mint("alice", PERMIT_KIND_USER, 3600);
        let claims = ca.verify(&tok).unwrap();
        assert_eq!(claims.sub, "alice");
        assert_eq!(claims.iss, "test-ca");
    }

    #[test]
    fn foreign_server_rejected() {
        let a = TestIssuer::new("server-a");
        let b = TestIssuer::new("server-b");
        let tok = a.mint("alice", PERMIT_KIND_USER, 3600);
        assert!(matches!(
            b.verify(&tok),
            Err(PermitError::ForeignServer { .. })
        ));
        // Cross-key confusion is also rejected, not just the label check:
        assert!(matches!(
            verify(&tok, "server-a", &b.pk),
            Err(PermitError::BadSignature)
        ));
    }

    #[test]
    fn expired_rejected() {
        let ca = TestIssuer::new("test-ca");
        let tok = ca.mint("alice", PERMIT_KIND_USER, 0);
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(matches!(ca.verify(&tok), Err(PermitError::Expired)));
    }

    #[test]
    fn tampered_payload_rejected() {
        let ca = TestIssuer::new("test-ca");
        let tok = ca.mint("alice", PERMIT_KIND_USER, 3600);
        let mut parts: Vec<String> = tok.split('.').map(|s| s.to_string()).collect();
        let flip = if parts[1].starts_with('A') { "B" } else { "A" };
        parts[1].replace_range(0..1, flip);
        let bad = parts.join(".");
        assert!(ca.verify(&bad).is_err());
    }

    #[test]
    fn wrong_key_length_rejected() {
        let ca = TestIssuer::new("test-ca");
        let tok = ca.mint("alice", PERMIT_KIND_USER, 3600);
        assert!(matches!(
            verify(&tok, "test-ca", &[0u8; 16]),
            Err(PermitError::Crypto(_))
        ));
    }

    #[test]
    fn mint_rejects_bad_signer_output() {
        let ca = TestIssuer::new("test-ca");
        let res = mint(&ca.identity, "alice", PERMIT_KIND_USER, 60, |_| Ok(vec![0u8; 8]));
        assert!(matches!(res, Err(PermitError::Crypto(_))));
    }

    #[test]
    fn kind_matrix() {
        let ca = TestIssuer::new("test-ca");
        // Unknown kind refused at mint time.
        assert!(matches!(
            mint(&ca.identity, "alice", "admin", 60, |_| Ok(vec![0u8; 8])),
            Err(PermitError::BadKind(_))
        ));
        // Server kind roundtrips with its label intact.
        let tok = ca.mint("ledger-auditor", PERMIT_KIND_SERVER, 3600);
        let claims = ca.verify(&tok).unwrap();
        assert_eq!(claims.kind, PERMIT_KIND_SERVER);
        assert_eq!(claims.sub, "ledger-auditor");
        // Kindless legacy token (hand-built without the claim) fails closed.
        let legacy = {
            let header = serde_json::json!({"alg": PERMIT_ALG, "typ": PERMIT_TYP, "kid": "test-ca"});
            let payload = serde_json::json!({
                "sub": "alice", "iss": "test-ca",
                "iat": now_secs(), "exp": now_secs() + 3600,
                "jti": "legacy-jti",
            });
            format!("{}.{}.{}", b64url(&serde_json::to_vec(&header).unwrap()), b64url(&serde_json::to_vec(&payload).unwrap()), b64url(&[0u8; 8]))
        };
        assert!(ca.verify(&legacy).is_err(), "kindless permits must fail");
    }
}
