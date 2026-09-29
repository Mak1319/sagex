use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub server: ServerSection,
    #[serde(default)]
    pub ledger: LedgerSection,
    #[serde(default)]
    pub outbox: OutboxSection,
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

#[allow(clippy::derivable_impls)]
impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerSection::default(),
            ledger: LedgerSection::default(),
            outbox: OutboxSection::default(),
        }
    }
}

fn default_listen() -> String {
    "127.0.0.1:8080".into()
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
        Ok(())
    }

    pub fn template() -> String {
        let cfg = Self::default();
        toml::to_string_pretty(&cfg).expect("template")
    }
}
