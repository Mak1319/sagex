//! Key generation for telemetry login.
//!
//! Two key types ("both"):
//! - Login key: `tlm-<64 hex chars>` bearer token. Stored in the OS keyring
//!   (service `sagex-telemetry`, account = username). The operator adds the
//!   same value to the gateway's accepted-bearer set, which authorizes this
//!   user for login/telemetry. Shown once at generation time.
//! - PQC identity: ML-DSA-65 keypair via `sagex-crypto`, password-wrapped
//!   seed kept in the keyring as hex. Used to cross-check
//!   `recipient_dsa_pub` values seen in ledger records.

use rand::{TryRng, rngs::SysRng};

/// Generate a fresh login bearer key (`tlm-<64 hex>`).
pub fn gen_login_key() -> String {
    let mut bytes = [0u8; 32];
    SysRng.try_fill_bytes(&mut bytes).expect("system RNG failure");
    format!("tlm-{}", hex::encode(bytes))
}

/// Generate an ML-DSA-65 pair. Returns `(seed_hex, verifying_key_hex)`.
/// The seed is the password-wrapped private material handle; keep it secret.
pub fn gen_dsa_pair(password: &[u8]) -> Result<(String, String), String> {
    let (encap, vk) = sagex_crypto::pqc::dsa::KeyGen::generate_from_password(password)
        .map_err(|e| e.to_string())?;
    let seed_hex = hex::encode(serde_json::to_vec(&encap).map_err(|e| e.to_string())?);
    Ok((seed_hex, hex::encode(vk)))
}
