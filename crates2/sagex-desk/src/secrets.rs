//! OS-keyring storage for the long-lived refresh JWT.
//!
//! - Access tokens stay memory-only (never persisted anywhere).
//! - Refresh tokens go to the OS keyring: service `sagex-desk`,
//!   account = username.
//! - No secret service on the machine → degrade to memory-only and tell
//!   the user (no file fallback — that would defeat the purpose).

pub const SERVICE: &str = "sagex-desk";

/// True when the platform store answers at all (even with "no entry").
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

pub fn store_refresh(username: &str, token: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, username)
        .map_err(|e| e.to_string())?
        .set_password(token)
        .map_err(|e| e.to_string())
}

/// None when nothing was stored; Err only when the store itself is broken.
pub fn load_refresh(username: &str) -> Result<Option<String>, String> {
    match keyring::Entry::new(SERVICE, username).map_err(|e| e.to_string())? {
        e => match e.get_password() {
            Ok(pw) => Ok(Some(pw)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        },
    }
}

pub fn clear_refresh(username: &str) -> Result<(), String> {
    match keyring::Entry::new(SERVICE, username).map_err(|e| e.to_string())? {
        e => match e.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        },
    }
}
