use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Debug, Deserialize)]
pub struct Settings {
    pub bind_addr: String,
    #[allow(dead_code)]
    pub ca_cert_pem_path: String,
    pub chatsrv_iss: String,
    pub audience: String,
    /// ML-DSA-65 verifying key of sagex-chatsrv.
    /// Preferred: path to a copy of the chatsrv `*.pub` file (postcard
    /// PublicFileFormatExternal); `key_dsa` is extracted from it.
    /// Fallback: inline hex in `chatsrv_dsa_pubkey_hex` (must not be empty
    /// or the REPLACE_WITH_* placeholder when the file path is unset).
    #[serde(default)]
    pub chatsrv_pub_path: String,
    #[serde(default)]
    pub chatsrv_dsa_pubkey_hex: String,
    pub cert_ttl_days: u64,
    pub issuer_cn: String,
    pub mongodb_db: String,
    pub mongodb_collection: String,
}

impl Settings {
    pub fn load(config_path: &str) -> anyhow::Result<Self> {
        let text = fs::read_to_string(config_path)?;
        Ok(toml::from_str(&text)?)
    }
}

#[derive(Clone, Debug)]
pub struct Secrets {
    pub ca_privkey_path: String,
    pub ca_privkey_password: String,
    pub mongodb_uri: String,
}

impl Secrets {
    pub fn load() -> Self {
        Self {
            ca_privkey_path: std::env::var("CA_PRIVKEY_PATH").unwrap_or_default(),
            ca_privkey_password: std::env::var("CA_PRIVKEY_PASSWORD").unwrap_or_default(),
            mongodb_uri: std::env::var("MONGODB_URI").unwrap_or_default(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub secrets: Secrets,
    pub mongo: Option<mongodb::Collection<UserRecord>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserRecord {
    pub user_name: String,
    pub key_kem: mongodb::bson::Binary,
    pub key_dsa: mongodb::bson::Binary,
    pub cert_pem: String,
    pub issued_at: i64,
}

pub fn hex_decode(s: &str) -> anyhow::Result<Vec<u8>> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out = Vec::with_capacity(s.len() / 2);
    let b = s.as_bytes();
    if b.len() % 2 != 0 {
        anyhow::bail!("odd hex length");
    }
    for i in (0..b.len()).step_by(2) {
        let h = u8::from_str_radix(std::str::from_utf8(&b[i..i + 2])?, 16)?;
        out.push(h);
    }
    Ok(out)
}

/// Resolve the chatsrv ML-DSA verifying key: `.pub` file first, hex fallback.
pub fn chatsrv_verify_key(settings: &Settings) -> anyhow::Result<Vec<u8>> {
    if !settings.chatsrv_pub_path.trim().is_empty() {
        let bytes = std::fs::read(&settings.chatsrv_pub_path)?;
        let publ = sagex_format::format::PublicFileFormatExternal::from_bytes(&bytes)
            .map_err(|e| anyhow::anyhow!("parse chatsrv .pub: {e}"))?;
        if publ.internal.key_dsa.is_empty() {
            anyhow::bail!("chatsrv .pub has empty key_dsa");
        }
        return Ok(publ.internal.key_dsa);
    }
    let hex = settings.chatsrv_dsa_pubkey_hex.trim();
    if hex.is_empty() || hex.starts_with("REPLACE_WITH") {
        anyhow::bail!("no chatsrv key pinned: set chatsrv_pub_path or chatsrv_dsa_pubkey_hex");
    }
    hex_decode(hex)
}

pub fn config_path_arg() -> String {
    std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string())
}

#[allow(dead_code)]
pub fn ensure_parent(p: &str) {
    if let Some(par) = Path::new(p).parent() {
        if !par.as_os_str().is_empty() {
            let _ = fs::create_dir_all(par);
        }
    }
}
