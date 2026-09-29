use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub server: ServerSection,
    #[serde(default)]
    pub ledger: LedgerSection,
    #[serde(default)]
    pub outbox: OutboxSection,
    #[serde(default)]
    pub auth: AuthSection,
    #[serde(default)]
    pub logs: LogsSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSection {
    #[serde(default = "default_listen")]
    pub listen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerSection {
    /// Ledger node TCP addresses. Tried in rotating order (failover).
    #[serde(default = "default_nodes")]
    pub nodes: Vec<String>,
    #[serde(default = "default_submit_timeout")]
    pub submit_timeout_ms: u64,
    #[serde(default = "default_query_timeout")]
    pub query_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxSection {
    #[serde(default = "default_db")]
    pub db: String,
    #[serde(default = "default_retry_interval")]
    pub retry_interval_ms: u64,
    /// Max rows the background worker attempts per sweep.
    #[serde(default = "default_batch")]
    pub retry_batch: usize,
}

/// Permit verification for `POST /register` — the same CA-auth strategy as
/// sagex-certauth (`sagex-auth` crate). Secure by default: with `enabled`
/// (the default), the gateway refuses intake without a valid server-issued
/// permit, and refuses to start when key material is missing. Set
/// `enabled = false` only for open air-gapped demos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSection {
    /// Verify permits on intake. Default true.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// CA identity the permits must pin (`kid`/`iss`), e.g. `sagex-ca`.
    #[serde(default = "default_ca_id")]
    pub ca_id: String,
    /// STANDARD base64 of the raw ML-DSA-65 CA public key (same bytes as
    /// certauth's `.pub` `key_dsa`).
    #[serde(default)]
    pub ca_pubkey_b64: String,
}

impl Default for ServerSection {
    fn default() -> Self {
        Self { listen: default_listen() }
    }
}
impl Default for LedgerSection {
    fn default() -> Self {
        Self {
            nodes: default_nodes(),
            submit_timeout_ms: default_submit_timeout(),
            query_timeout_ms: default_query_timeout(),
        }
    }
}
impl Default for OutboxSection {
    fn default() -> Self {
        Self {
            db: default_db(),
            retry_interval_ms: default_retry_interval(),
            retry_batch: default_batch(),
        }
    }
}
impl Default for AuthSection {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            ca_id: default_ca_id(),
            ca_pubkey_b64: String::new(),
        }
    }
}

/// Restricted auditor log view (`GET /logs`, Bearer CA permit).
/// Same CA identity as `[auth]`; the permit's subject must equal
/// `auditor_sub` (e.g. minted via
/// `sagex-certauth issue-permit --identity ledger-auditor`).
/// Unlike intake permits, auditor permits are multi-use (no JTI burn) —
/// security comes from short TTL + the restricted subject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsSection {
    /// Serve GET /logs at all. Default true (still permit-gated).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// In-memory ring capacity for this process. Default 2000.
    #[serde(default = "default_log_buffer")]
    pub buffer: usize,
    /// Required permit subject for log access. Default `ledger-auditor`.
    #[serde(default = "default_auditor_sub")]
    pub auditor_sub: String,
}

impl Default for LogsSection {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            buffer: default_log_buffer(),
            auditor_sub: default_auditor_sub(),
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerSection::default(),
            ledger: LedgerSection::default(),
            outbox: OutboxSection::default(),
            auth: AuthSection::default(),
            logs: LogsSection::default(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_log_buffer() -> usize {
    2000
}
fn default_auditor_sub() -> String {
    "ledger-auditor".into()
}
fn default_ca_id() -> String {
    "sagex-ca".into()
}

fn default_listen() -> String {
    "127.0.0.1:8081".into()
}
fn default_nodes() -> Vec<String> {
    vec![
        "127.0.0.1:7000".into(),
        "127.0.0.1:7001".into(),
        "127.0.0.1:7002".into(),
        "127.0.0.1:7003".into(),
    ]
}
fn default_submit_timeout() -> u64 {
    15000
}
fn default_query_timeout() -> u64 {
    5000
}
fn default_db() -> String {
    "gateway.db".into()
}
fn default_retry_interval() -> u64 {
    2000
}
fn default_batch() -> usize {
    32
}

impl GatewayConfig {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&text)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.ledger.nodes.is_empty() {
            anyhow::bail!("ledger.nodes must list at least one node address");
        }
        if self.server.listen.trim().is_empty() {
            anyhow::bail!("server.listen is empty");
        }
        if self.auth.enabled
            && (self.auth.ca_id.trim().is_empty() || self.auth.ca_pubkey_b64.trim().is_empty())
        {
            anyhow::bail!(
                "auth.enabled but [auth].ca_id/ca_pubkey_b64 is not configured \
                 (paste the CA ML-DSA-65 public key, or set auth.enabled=false for open demos)"
            );
        }
        Ok(())
    }

    pub fn template() -> String {
        let cfg = Self::default();
        toml::to_string_pretty(&cfg).expect("template")
    }
}
