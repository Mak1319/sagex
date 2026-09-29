//! Persisted JOSE session: refresh token kept for up to
//! `SAGEX_SESSION_MAX_AGE_DAYS` (root `.env`, default 30).
//!
//! File: `$XDG_DATA_HOME/sagex_ui/session.json` (0600 on unix).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub access_token: String,
    pub access_expires_at: i64,
    pub refresh_token: String,
    pub refresh_expires_at: i64,
    pub user_id: String,
    pub email: String,
    /// unix seconds when the session was saved (for max-age enforcement)
    pub saved_at: i64,
}

impl StoredSession {
    pub fn access_valid(&self, skew_secs: i64) -> bool {
        now_unix() < self.access_expires_at - skew_secs
    }
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn default_path() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|h| PathBuf::from(h).join(".local").join("share"))
        })
        .unwrap_or_else(std::env::temp_dir);
    base.join("sagex_ui").join("session.json")
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    path: PathBuf,
    pub persist: bool,
    current: Option<StoredSession>,
}

impl SessionStore {
    pub fn new(persist: bool) -> Self {
        Self::at_path(default_path(), persist)
    }

    pub fn at_path(path: PathBuf, persist: bool) -> Self {
        let current = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        Self {
            path,
            persist,
            current,
        }
    }

    pub fn get(&self) -> Option<&StoredSession> {
        self.current.as_ref()
    }

    /// Fresh enough to attempt auto-login (age + refresh expiry).
    pub fn usable(&self, max_age_days: i64) -> Option<StoredSession> {
        let s = self.current.clone()?;
        if now_unix() - s.saved_at > max_age_days * 86_400 {
            return None;
        }
        if now_unix() >= s.refresh_expires_at {
            return None;
        }
        Some(s)
    }

    pub fn set(&mut self, s: StoredSession) {
        self.current = Some(s.clone());
        if self.persist {
            self.write_file(&s);
        }
    }

    pub fn update_tokens(
        &mut self,
        access: &str,
        access_exp: i64,
        refresh: &str,
        refresh_exp: i64,
    ) {
        if let Some(s) = self.current.as_mut() {
            s.access_token = access.to_string();
            s.access_expires_at = access_exp;
            s.refresh_token = refresh.to_string();
            s.refresh_expires_at = refresh_exp;
            s.saved_at = now_unix();
            let snap = s.clone();
            if self.persist {
                self.write_file(&snap);
            }
        }
    }

    pub fn clear(&mut self) {
        self.current = None;
        self.drop_file();
    }

    /// Forget the persisted file but keep the in-memory session.
    pub fn drop_file(&self) {
        let _ = std::fs::remove_file(&self.path);
    }

    fn write_file(&self, s: &StoredSession) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(s) {
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                let _ = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&self.path)
                    .and_then(|mut f| {
                        use std::io::Write;
                        f.write_all(&bytes)
                    });
            }
            #[cfg(not(unix))]
            {
                let _ = std::fs::write(&self.path, bytes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("sagex_ui_test_{tag}_{}.json", std::process::id()))
    }

    fn sample(saved_at: i64, refresh_exp: i64) -> StoredSession {
        StoredSession {
            access_token: "a".into(),
            access_expires_at: saved_at + 900,
            refresh_token: "r".into(),
            refresh_expires_at: refresh_exp,
            user_id: "u".into(),
            email: "e@x.com".into(),
            saved_at,
        }
    }

    #[test]
    fn persist_roundtrip_and_max_age() {
        let path = tmp_path("sess");
        let _ = std::fs::remove_file(&path);
        let mut s = SessionStore::at_path(path.clone(), true);
        assert!(s.get().is_none());
        let now = now_unix();
        s.set(sample(now, now + 2_592_000));
        // reload from disk
        let s2 = SessionStore::at_path(path.clone(), true);
        assert_eq!(s2.get().unwrap().email, "e@x.com");
        assert!(s2.usable(30).is_some());
        // older than the window → discarded
        let mut s_old = SessionStore::at_path(path.clone(), true);
        s_old.set(sample(now - 2 * 86_400, now + 2_592_000));
        assert!(s_old.usable(30).is_some());
        assert!(s_old.usable(1).is_none());
        // expired refresh → discarded
        let mut s3 = SessionStore::at_path(path.clone(), true);
        s3.set(sample(now, now - 1));
        assert!(s3.usable(30).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn no_persist_writes_no_file() {
        let path = tmp_path("nopersist");
        let _ = std::fs::remove_file(&path);
        let mut s = SessionStore::at_path(path.clone(), false);
        let now = now_unix();
        s.set(sample(now, now + 2_592_000));
        assert!(!path.exists());
        assert!(s.usable(30).is_some());
    }
}
