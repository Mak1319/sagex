//! JOSE (JWS compact serialization) tokens signed with ML-DSA-65.
//!
//! Format follows RFC 7515 (JWS) / RFC 7519 (JWT) with `alg = "ML-DSA-65"`
//! as registered by RFC 9964 (ML-DSA for JOSE and COSE).
//!
//! ```text
//! token = b64u(header) || "." || b64u(payload) || "." || b64u(raw_mldsa65_sig)
//! header = {"alg":"ML-DSA-65","typ":"JWT","kid":"..."}
//! ```
//!
//! Signing uses `rustpq` ML-DSA-65 directly; verification goes through the
//! workspace `sagex-crypto` crate (`pqc::dsa::verify_signature`) so both
//! crates stay in the loop.

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use rand_core::OsRng;
use rustpq::ml_dsa::mldsa65::{generate, sign, PublicKey, SecretKey, Signature};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// JOSE `alg` value for ML-DSA-65 (RFC 9964 §8.1.4.2).
pub const ALG_MLDSA65: &str = "ML-DSA-65";
/// Domain-separation context bound into every ML-DSA signature.
const SIG_CONTEXT: &[u8] = b"sagex-chatsrv/jose-mldsa65-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Header {
    alg: String,
    typ: String,
    kid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user id (ObjectId hex)
    /// Username at mint time (point-in-time binding for CA enrollment).
    /// Required — tokens without it fail closed.
    pub username: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
    pub purpose: String, // "access" | "refresh"
}

pub struct JoseSigner {
    sk: SecretKey,
    pk: Vec<u8>,
    kid: String,
    access_ttl_secs: i64,
    refresh_ttl_secs: i64,
}

impl JoseSigner {
    /// Load from base64 key env vars, or generate an ephemeral dev keypair.
    /// Returns the signer plus `true` when the key is ephemeral.
    pub fn from_env_or_ephemeral(
        sk_b64: Option<&str>,
        pk_b64: Option<&str>,
        kid: String,
        access_ttl_secs: i64,
        refresh_ttl_secs: i64,
    ) -> AppResult<(Self, bool)> {
        if let (Some(sk), Some(pk)) = (sk_b64, pk_b64) {
            let sk_bytes = STANDARD
                .decode(sk.trim())
                .map_err(|e| AppError::Internal(format!("bad MLDSA65_SK_B64: {e}")))?;
            let pk_bytes = STANDARD
                .decode(pk.trim())
                .map_err(|e| AppError::Internal(format!("bad MLDSA65_PK_B64: {e}")))?;
            let sk = SecretKey::from_bytes(&sk_bytes)
                .map_err(|_| AppError::Internal("bad MLDSA65_SK_B64 length".into()))?;
            let pk = PublicKey::from_bytes(&pk_bytes)
                .map_err(|_| AppError::Internal("bad MLDSA65_PK_B64 length".into()))?;
            let _ = pk; // length-checked; bytes kept for JWK export
            return Ok((
                Self {
                    sk,
                    pk: pk_bytes,
                    kid,
                    access_ttl_secs,
                    refresh_ttl_secs,
                },
                false,
            ));
        }
        let (pk, sk) = generate(&mut OsRng);
        Ok((
            Self {
                sk,
                pk: pk.as_bytes().to_vec(),
                kid,
                access_ttl_secs,
                refresh_ttl_secs,
            },
            true,
        ))
    }

    pub fn public_key_b64(&self) -> String {
        STANDARD.encode(&self.pk)
    }

    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// RFC 9964-style public JWK for `GET /api/v1/auth/jwks.json`.
    pub fn jwks(&self) -> serde_json::Value {
        serde_json::json!({
            "keys": [{
                "kty": "ML-DSA",
                "alg": ALG_MLDSA65,
                "use": "sig",
                "kid": self.kid,
                "pub": URL_SAFE_NO_PAD.encode(&self.pk),
            }]
        })
    }

    fn ttl_for(&self, purpose: &str) -> AppResult<i64> {
        match purpose {
            "access" => Ok(self.access_ttl_secs),
            "refresh" => Ok(self.refresh_ttl_secs),
            _ => Err(AppError::BadRequest("unknown token purpose".into())),
        }
    }

    /// Issue a compact JWS for `user_id_hex` + `username` with the given purpose.
    /// Returns `(token, jti, exp_unix)`.
    pub fn issue(
        &self,
        user_id_hex: &str,
        username: &str,
        purpose: &str,
    ) -> AppResult<(String, String, i64)> {
        let ttl = self.ttl_for(purpose)?;
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: user_id_hex.to_string(),
            username: username.to_string(),
            iat: now,
            exp: now + ttl,
            jti: Uuid::new_v4().to_string(),
            purpose: purpose.to_string(),
        };
        let header = Header {
            alg: ALG_MLDSA65.to_string(),
            typ: "JWT".to_string(),
            kid: self.kid.clone(),
        };
        let h = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&header)
                .map_err(|e| AppError::Internal(format!("header encode: {e}")))?,
        );
        let p = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&claims)
                .map_err(|e| AppError::Internal(format!("claims encode: {e}")))?,
        );
        let signing_input = format!("{h}.{p}");
        let sig: Signature = sign(&self.sk, signing_input.as_bytes(), SIG_CONTEXT, &mut OsRng)
            .map_err(|_| AppError::Internal("ML-DSA-65 signing failed".into()))?;
        let s = URL_SAFE_NO_PAD.encode(sig.as_bytes());
        Ok((format!("{signing_input}.{s}"), claims.jti, claims.exp))
    }

    /// Verify a compact JWS, enforcing `alg`, purpose and expiry.
    pub fn verify(&self, token: &str, expected_purpose: &str) -> AppResult<Claims> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(AppError::Unauthorized("malformed token".into()));
        }
        let header_bytes = URL_SAFE_NO_PAD
            .decode(parts[0])
            .map_err(|_| AppError::Unauthorized("bad token header".into()))?;
        let header: Header = serde_json::from_slice(&header_bytes)
            .map_err(|_| AppError::Unauthorized("bad token header".into()))?;
        if header.alg != ALG_MLDSA65 {
            return Err(AppError::Unauthorized("unexpected token alg".into()));
        }
        let payload_bytes = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|_| AppError::Unauthorized("bad token payload".into()))?;
        let claims: Claims = serde_json::from_slice(&payload_bytes)
            .map_err(|_| AppError::Unauthorized("bad token claims".into()))?;
        if claims.purpose != expected_purpose {
            return Err(AppError::Unauthorized("wrong token purpose".into()));
        }
        let now = chrono::Utc::now().timestamp();
        if claims.exp <= now {
            return Err(AppError::Unauthorized("token expired".into()));
        }
        let sig_bytes = URL_SAFE_NO_PAD
            .decode(parts[2])
            .map_err(|_| AppError::Unauthorized("bad token signature".into()))?;
        let sig = Signature::from_bytes(&sig_bytes)
            .map_err(|_| AppError::Unauthorized("bad signature length".into()))?;
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        sagex_crypto::pqc::dsa::verify_signature(
            self.pk.clone(),
            signing_input.as_bytes(),
            SIG_CONTEXT,
            sig,
        )
        .map_err(|_| AppError::Unauthorized("invalid ML-DSA-65 signature".into()))?;
        Ok(claims)
    }
}
