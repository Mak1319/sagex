use sagex_crypto::aes::KeyEncapsulation;
use zeroize::Zeroizing;

use crate::{config::Config, db::Db};

#[derive(Clone)]
pub struct ServiceKeys {
    /// Service DSA wrapping envelope from the .prv file
    pub enc_dsa: KeyEncapsulation,
    /// Service DSA verifying key bytes (from the .pub file)
    pub vk_pub: Vec<u8>,
    /// .prv wrapping password (from env, wiped on drop)
    pub password: Zeroizing<Vec<u8>>,
}

#[derive(Clone)]
pub struct AppState {
    pub cfg: Config,
    pub db: Db,
    pub jwt_secret: Zeroizing<Vec<u8>>,
    pub svc: ServiceKeys,
}

/// Authenticated caller injected by the auth middleware.
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub object_key: String,
    pub username: String,
}
