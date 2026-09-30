//! Telemetry home: config + UI state.
//!
//! Base is `$SAGEX_TELEMETRY_HOME` when set (single dir), otherwise XDG:
//! config in `config_dir()/sagex-telemetry`, data in `data_dir()/sagex-telemetry`.
//!
//! ```text
//! <config>/config.toml      gateway_url, timeouts (tracked example in the crate)
//! <data>/state.json         last user
//! ```
//! Secrets never touch disk — login keys + DSA seeds live in the OS keyring.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    #[serde(default = "def_gateway_url")]
    pub gateway_url: String,
    #[serde(default = "def_timeout")]
    pub request_timeout_secs: u64,
    #[serde(default = "def_theme")]
    pub theme: String,
}

fn def_gateway_url() -> String {
    "http://127.0.0.1:8490".to_string()
}
fn def_timeout() -> u64 {
    15
}
fn def_theme() -> String {
    "dark".to_string()
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            gateway_url: def_gateway_url(),
            request_timeout_secs: def_timeout(),
            theme: def_theme(),
        }
    }
}

/// Persisted UI state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelemetryState {
    #[serde(default)]
    pub last_user: Option<String>,
}

pub struct Home {
    #[allow(dead_code)]
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config: TelemetryConfig,
    pub state: TelemetryState,
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<Option<T>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())).map(Some)
}

fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &PathBuf) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Resolve dirs, create them, load (or write defaults for) config+state.
pub fn resolve() -> Result<Home, String> {
    let (config_dir, data_dir) = match std::env::var("SAGEX_TELEMETRY_HOME") {
        Ok(h) if !h.trim().is_empty() => {
            let p = PathBuf::from(h);
            (p.clone(), p)
        }
        _ => {
            let config_base = dirs::config_dir().ok_or("no OS config dir")?;
            let data_base = dirs::data_dir().ok_or("no OS data dir")?;
            (
                config_base.join("sagex-telemetry"),
                data_base.join("sagex-telemetry"),
            )
        }
    };
    for d in [&config_dir, &data_dir] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }

    let config_path = config_dir.join("config.toml");
    let config: TelemetryConfig = match read_toml(&config_path)? {
        Some(c) => c,
        None => {
            let c = TelemetryConfig::default();
            let text = toml::to_string_pretty(&c).map_err(|e| e.to_string())?;
            std::fs::write(&config_path, text)
                .map_err(|e| format!("{}: {e}", config_path.display()))?;
            c
        }
    };
    let state: TelemetryState = read_json(&data_dir.join("state.json"));

    Ok(Home { config_dir, data_dir, config, state })
}

impl Home {
    pub fn save_state(&self) -> Result<(), String> {
        let path = self.data_dir.join("state.json");
        let text = serde_json::to_string_pretty(&self.state).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}
