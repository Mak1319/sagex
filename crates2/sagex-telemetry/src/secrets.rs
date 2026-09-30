//! OS-keyring storage for telemetry credentials.
//!
//! - Login bearer keys live in the OS keyring: service `sagex-telemetry`,
//!   account = `login:<username>`.
//! - DSA seeds live in the OS keyring: account = `dsa:<username>`.
//! - Nothing secret is ever written to disk.

pub const SERVICE: &str = "sagex-telemetry";

fn account(prefix: &str, username: &str) -> String {
    format!("{prefix}:{username}")
}

pub fn available() -> bool {
    match keyring::Entry::new(SERVICE, "__sagex_probe__") {
        Ok(e) => !matches!(
            e.get_password(),
            Err(keyring::Error::PlatformFailure(_))
                | Err(keyring::Error::NoStorageAccess(_))
        ),
        Err(_) => false,
    }
}

pub fn store_login_key(username: &str, token: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, &account("login", username))
        .map_err(|e| e.to_string())?
        .set_password(token)
        .map_err(|e| e.to_string())
}

pub fn load_login_key(username: &str) -> Result<Option<String>, String> {
    match keyring::Entry::new(SERVICE, &account("login", username)).map_err(|e| e.to_string())? {
        e => match e.get_password() {
            Ok(pw) => Ok(Some(pw)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        },
    }
}

pub fn store_dsa_seed(username: &str, seed_hex: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, &account("dsa", username))
        .map_err(|e| e.to_string())?
        .set_password(seed_hex)
        .map_err(|e| e.to_string())
}

pub fn load_dsa_seed(username: &str) -> Result<Option<String>, String> {
    match keyring::Entry::new(SERVICE, &account("dsa", username)).map_err(|e| e.to_string())? {
        e => match e.get_password() {
            Ok(pw) => Ok(Some(pw)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        },
    }
}
