use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Clone, Deserialize)]
pub struct Config {
    pub node_id: String,
    pub api_listen: String,
    pub peer_listen: String,
    pub database: DatabaseConfig,
    pub tls: TlsConfig,
    pub validator: ValidatorConfig,
    pub peers: Vec<PeerConfig>,
}

#[derive(Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Clone, Deserialize)]
pub struct TlsConfig {
    pub server_certificate: String,
    pub server_private_key: String,
    pub gateway_ca: String,
    pub validator_ca: String,
    pub peer_client_certificate: String,
    pub peer_client_private_key: String,
}

#[derive(Clone, Deserialize)]
pub struct ValidatorConfig {
    pub signing_seed_hex: String,
}

#[derive(Clone, Deserialize)]
pub struct PeerConfig {
    pub id: String,
    pub address: String,
    pub verifying_key_hex: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("read config {}", path.display()))?;
        let config: Self = toml::from_str(&source).context("parse nodeblock TOML")?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.node_id.trim().is_empty() {
            bail!("node_id must not be empty");
        }
        if self.peers.len() < 3 {
            bail!("at least four validators are required, including this node");
        }
        if self.database.path.trim().is_empty() {
            bail!("database.path must not be empty");
        }
        for peer in &self.peers {
            if !peer.address.starts_with("https://") {
                bail!("validator peer addresses must use HTTPS");
            }
        }
        if self.peers.iter().any(|peer| peer.id == self.node_id) {
            bail!("peers must list other validators only");
        }
        let unique_ids: std::collections::HashSet<_> =
            self.peers.iter().map(|peer| peer.id.as_str()).collect();
        if unique_ids.len() != self.peers.len() {
            bail!("validator IDs must be unique");
        }
        hex::decode(&self.validator.signing_seed_hex)
            .context("validator signing_seed_hex must be hex")?;
        for peer in &self.peers {
            hex::decode(&peer.verifying_key_hex)
                .with_context(|| format!("validator {} key must be hex", peer.id))?;
        }
        Ok(())
    }
}
