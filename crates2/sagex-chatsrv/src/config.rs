use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub mongo: MongoConfig,
    pub rustfs: RustFsConfig,
    pub keys: KeysConfig,
    pub auth: AuthConfig,
    pub ca: CaConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MongoConfig {
    pub uri: String,
    #[serde(default = "default_db")]
    pub db: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RustFsConfig {
    pub endpoint: String,
    pub bucket: String,
    #[serde(default = "default_region")]
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    #[serde(default = "default_put_ttl")]
    pub put_url_ttl_secs: u64,
    #[serde(default = "default_get_ttl")]
    pub get_url_ttl_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeysConfig {
    /// Path to the service `<name>.prv` (postcard PrivateExternal)
    pub prv_path: PathBuf,
    /// Path to the service `<name>.pub` (postcard PublicFileFormatExternal)
    pub pub_path: PathBuf,
    /// Env var holding the .prv wrapping password (from env, never TOML)
    #[serde(default = "default_pw_env")]
    pub password_env: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_access_ttl")]
    pub access_ttl_mins: i64,
    #[serde(default = "default_refresh_ttl")]
    pub refresh_ttl_days: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaConfig {
    #[serde(default = "default_audience")]
    pub audience: String,
    #[serde(default = "default_ca_ttl")]
    pub token_ttl_mins: i64,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    3001
}
fn default_db() -> String {
    "sagex_chat".to_string()
}
fn default_region() -> String {
    "us-east-1".to_string()
}
fn default_put_ttl() -> u64 {
    900
}
fn default_get_ttl() -> u64 {
    300
}
fn default_pw_env() -> String {
    "KEY_PASSWORD".to_string()
}
fn default_access_ttl() -> i64 {
    15
}
fn default_refresh_ttl() -> i64 {
    7
}
fn default_audience() -> String {
    "sagex-authsrv".to_string()
}
fn default_ca_ttl() -> i64 {
    5
}

#[derive(Debug)]
pub enum ConfigError {
    Io(String),
    Parse(String),
    Env(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "config io: {e}"),
            Self::Parse(e) => write!(f, "config parse: {e}"),
            Self::Env(e) => write!(f, "config env: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Load `dir/.env` (if present) then parse `config_path` as TOML.
pub fn load(env_dir: &std::path::Path, config_path: &std::path::Path) -> Result<Config, ConfigError> {
    let dotenv_path = env_dir.join(".env");
    if dotenv_path.exists() {
        dotenvy::from_path(&dotenv_path).map_err(|e| ConfigError::Env(e.to_string()))?;
    }
    let text =
        std::fs::read_to_string(config_path).map_err(|e| ConfigError::Io(e.to_string()))?;
    toml::from_str(&text).map_err(|e| ConfigError::Parse(e.to_string()))
}
