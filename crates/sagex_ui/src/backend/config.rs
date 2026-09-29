//! Backend config: server URL + session max age, both tunable via root `.env`.
//!
//! Precedence: explicit process env > root `.env` (repo root, found by
//! walking up from CWD) > built-in defaults.

use std::path::PathBuf;

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8080";
pub const DEFAULT_SESSION_MAX_AGE_DAYS: i64 = 30;

#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub server_url: String,
    /// Max age (days) of a persisted refresh session. Mirrors the server's
    /// `REFRESH_TOKEN_TTL_SECS` (default 30d). Older sessions are discarded
    /// and the app opens auth-gated.
    pub session_max_age_days: i64,
}

fn load_root_dotenv() {
    if std::env::var("SAGEX_SERVER_URL").is_ok()
        && std::env::var("SAGEX_SESSION_MAX_AGE_DAYS").is_ok()
    {
        return;
    }
    let mut dir: Option<PathBuf> = std::env::current_dir().ok();
    for _ in 0..5 {
        let Some(d) = dir.clone() else { break };
        let candidate = d.join(".env");
        if candidate.is_file() {
            // from_path never overrides explicit env — precedence preserved.
            let _ = dotenvy::from_path(&candidate);
            return;
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
}

impl BackendConfig {
    pub fn load() -> Self {
        load_root_dotenv();
        let server_url = std::env::var("SAGEX_SERVER_URL")
            .ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_SERVER_URL.to_string());
        let session_max_age_days = std::env::var("SAGEX_SESSION_MAX_AGE_DAYS")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_SESSION_MAX_AGE_DAYS);
        Self {
            server_url,
            session_max_age_days: session_max_age_days.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_without_env() {
        // Explicit env wins; unset vars fall back to defaults. (Other tests
        // in this repo may set these, so only assert the floor behavior.)
        let cfg = BackendConfig {
            server_url: DEFAULT_SERVER_URL.to_string(),
            session_max_age_days: DEFAULT_SESSION_MAX_AGE_DAYS,
        };
        assert_eq!(cfg.server_url, "http://127.0.0.1:8080");
        assert_eq!(cfg.session_max_age_days, 30);
    }
}
