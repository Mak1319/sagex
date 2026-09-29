//! Server-issued JOSE permits, bound to this CA's key material.
//!
//! The wire logic (JWS shape, claims, verification rules) lives in
//! [`sagex_auth`] — shared verbatim with sagex-gateway, so "the same
//! strategy as CA auth" is literally the same code. This module only adapts
//! it to [`CaMaterial`]/[`MlDsa65Signer`].

pub use sagex_auth::{
    PermitClaims, PermitError, PERMIT_ALG, PERMIT_TYP, PERMIT_KIND_SERVER, PERMIT_KIND_USER,
    now_secs,
};

use signature::Signer;

use crate::{ca::CaMaterial, mldsa::MlDsa65Signer};

/// Mint a permit for `subject` with jurisdiction `kind`
/// ([`PERMIT_KIND_USER`] / [`PERMIT_KIND_SERVER`]), valid for `ttl_secs`,
/// signed with the CA key.
pub fn mint(
    ca: &CaMaterial,
    signer: &MlDsa65Signer,
    subject: &str,
    kind: &str,
    ttl_secs: u64,
) -> Result<String, PermitError> {
    mint_for(&ca.identity, signer, subject, kind, ttl_secs)
}

/// Mint without a [`CaMaterial`]: for handlers holding only the identity
/// and an in-memory signer.
pub fn mint_for(
    ca_identity: &str,
    signer: &MlDsa65Signer,
    subject: &str,
    kind: &str,
    ttl_secs: u64,
) -> Result<String, PermitError> {
    sagex_auth::mint(ca_identity, subject, kind, ttl_secs, |input| {
        signer
            .try_sign(input)
            .map(|s| s.0)
            .map_err(|e| format!("{e:?}"))
    })
}

/// Verify a permit against the loaded CA. Returns the claims on success.
pub fn verify(
    token: &str,
    ca_identity: &str,
    ca_dsa_public: &[u8],
) -> Result<PermitClaims, PermitError> {
    sagex_auth::verify(token, ca_identity, ca_dsa_public)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ca::CaMaterial;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Proves the adapter wiring (CA material -> shared mint/verify);
    /// the rule matrix itself is tested in `sagex-auth`.
    #[test]
    fn adapter_roundtrip_via_ca_material() {
        let dir = std::env::temp_dir().join(format!(
            "sagex-permit-adapter-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (ca, _) =
            CaMaterial::load_or_generate(&dir, b"test-password", "test-ca").unwrap();
        let signer = ca.signer(b"test-password").unwrap();
        let tok = mint(&ca, &signer, "alice", PERMIT_KIND_USER, 3600).unwrap();
        let claims = verify(&tok, &ca.identity, &ca.dsa_public).unwrap();
        assert_eq!(claims.sub, "alice");
        assert_eq!(claims.iss, "test-ca");
    }
}
