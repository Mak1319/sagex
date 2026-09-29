//! Permit verification for `POST /register` — the same strategy as CA
//! auth in sagex-certauth, literally the same code (`sagex-auth`).
//!
//! When enabled, intake requires a server-issued JOSE permit minted offline
//! with the CA key: the permit signature is verified, `kid`/`iss` must pin
//! this deployment's CA identity, the permit must be unexpired, and
//! `claims.sub` must equal the record's `user_id` (plain string equality —
//! `user_id` stays free-form, no charset rules). Each permit (`jti`) is
//! single-use, tracked in the gateway's own SQLite store.

use base64::{Engine, engine::general_purpose::STANDARD};

/// Permit verifier: pure config + the CA public key. No secrets here.
#[derive(Debug, Clone)]
pub struct AuthVerifier {
    pub enabled: bool,
    pub ca_id: String,
    pub ca_pubkey: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthConfigError {
    #[error("auth.enabled but [auth].ca_id is empty")]
    EmptyCaId,
    #[error("auth.enabled but [auth].ca_pubkey_b64 is empty (paste the CA ML-DSA-65 public key)")]
    EmptyCaKey,
    #[error("bad [auth].ca_pubkey_b64: {0}")]
    BadCaKey(String),
}

impl AuthVerifier {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ca_id: String::new(),
            ca_pubkey: Vec::new(),
        }
    }

    /// Build from config values. Fails fast on missing/malformed key material
    /// when enabled, so a misconfigured gateway refuses to start instead of
    /// running unprotected. `ca_pubkey_b64` is STANDARD base64 of the raw
    /// ML-DSA-65 public key (same bytes as certauth's `.pub` `key_dsa`).
    pub fn new(
        enabled: bool,
        ca_id: &str,
        ca_pubkey_b64: &str,
    ) -> Result<Self, AuthConfigError> {
        if !enabled {
            return Ok(Self::disabled());
        }
        if ca_id.trim().is_empty() {
            return Err(AuthConfigError::EmptyCaId);
        }
        if ca_pubkey_b64.trim().is_empty() {
            return Err(AuthConfigError::EmptyCaKey);
        }
        let ca_pubkey = STANDARD
            .decode(ca_pubkey_b64.trim())
            .map_err(|e| AuthConfigError::BadCaKey(e.to_string()))?;
        if ca_pubkey.len() != sagex_auth::DSA_PUBLIC_KEY_BYTES {
            return Err(AuthConfigError::BadCaKey(format!(
                "expected {} bytes, got {}",
                sagex_auth::DSA_PUBLIC_KEY_BYTES,
                ca_pubkey.len()
            )));
        }
        Ok(Self {
            enabled: true,
            ca_id: ca_id.trim().to_string(),
            ca_pubkey,
        })
    }

    /// Verify a permit token. Only meaningful when enabled; returns the
    /// claims (caller enforces `sub == record.user_id` and JTI single-use).
    pub fn verify_permit(
        &self,
        token: &str,
    ) -> Result<sagex_auth::PermitClaims, sagex_auth::PermitError> {
        debug_assert!(self.enabled);
        sagex_auth::verify(token, &self.ca_id, &self.ca_pubkey)
    }
}
