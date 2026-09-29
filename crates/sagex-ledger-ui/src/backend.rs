//! Read-only HTTP client for the register gateway.
//!
//! The explorer talks **only** to the gateway (never TCP-direct to ledger
//! nodes). Record/status/outbox endpoints are open; `GET /logs` additionally
//! requires the CA-issued auditor permit, sent as
//! `Authorization: Bearer <permit>`.
//!
//! Permit source (first non-empty wins): in-app settings field, then the
//! `SAGEX_AUDIT_PERMIT` env var. The permit is never written to disk by
//! this app.

use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Context};
use serde::Deserialize;

use sagex_ledger::model::Block;

#[derive(Debug, Clone, Deserialize)]
pub struct BlocksBody {
    #[serde(default)]
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatusBody {
    #[serde(default)]
    pub outbox_pending: i64,
    #[serde(default)]
    pub ledger: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogEntryView {
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeLogsView {
    #[serde(default)]
    pub node_addr: String,
    #[serde(default)]
    pub entries: Option<Vec<LogEntryView>>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogsBody {
    #[serde(default)]
    pub gateway: Option<Vec<LogEntryView>>,
    #[serde(default)]
    pub nodes: Option<Vec<NodeLogsView>>,
}

#[derive(Debug, Clone)]
pub struct GatewayClient {
    http: reqwest::Client,
    base: String,
    permit: Arc<RwLock<String>>,
}

impl GatewayClient {
    pub fn new(base_url: &str) -> Self {
        let permit = std::env::var("SAGEX_AUDIT_PERMIT").unwrap_or_default();
        Self {
            http: reqwest::Client::new(),
            base: base_url.trim_end_matches('/').to_string(),
            permit: Arc::new(RwLock::new(permit)),
        }
    }

    pub fn set_permit(&self, permit: &str) {
        *self.permit.write().unwrap() = permit.trim().to_string();
    }

    pub fn has_permit(&self) -> bool {
        !self.permit.read().unwrap().is_empty()
    }

    fn bearer(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let p = self.permit.read().unwrap().clone();
        if p.is_empty() { req } else { req.bearer_auth(p) }
    }

    async fn get_json(&self, path: &str, authed: bool) -> anyhow::Result<serde_json::Value> {
        let req = self.http.get(format!("{}{path}", self.base));
        let req = if authed { self.bearer(req) } else { req };
        let resp = req.send().await.context("gateway unreachable")?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!("401: auditor permit missing or invalid (see Settings)");
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            anyhow::bail!("403: permit is not an auditor credential");
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("404: not found (logs endpoint may be disabled)");
        }
        if !status.is_success() {
            anyhow::bail!("gateway HTTP {status}");
        }
        resp.json().await.context("bad gateway JSON")
    }

    fn blocks_of(v: serde_json::Value) -> anyhow::Result<Vec<Block>> {
        let body: BlocksBody = serde_json::from_value(v)?;
        if body.blocks.is_empty()
            && let Some(e) = body.error.filter(|e| !e.is_empty())
        {
            return Err(anyhow!("{e}"));
        }
        Ok(body.blocks)
    }

    pub async fn health(&self) -> anyhow::Result<()> {
        self.get_json("/health", false).await.map(|_| ())
    }

    pub async fn status(&self) -> anyhow::Result<StatusBody> {
        Ok(serde_json::from_value(self.get_json("/status", false).await?)?)
    }

    pub async fn block(&self, index: u64) -> anyhow::Result<Vec<Block>> {
        Self::blocks_of(self.get_json(&format!("/block/{index}"), false).await?)
    }

    pub async fn by_watermark(&self, wm: &str) -> anyhow::Result<Vec<Block>> {
        Self::blocks_of(self.get_json(&format!("/record/{wm}"), false).await?)
    }

    pub async fn by_user(&self, user: &str) -> anyhow::Result<Vec<Block>> {
        Self::blocks_of(self.get_json(&format!("/records?user_id={user}"), false).await?)
    }

    pub async fn outbox(&self) -> anyhow::Result<serde_json::Value> {
        self.get_json("/outbox", false).await
    }

    pub async fn logs(
        &self,
        level: Option<&str>,
        limit: u64,
        source: Option<&str>,
    ) -> anyhow::Result<LogsBody> {
        let mut q = format!("/logs?limit={limit}");
        if let Some(l) = level.filter(|s| !s.is_empty()) {
            q.push_str(&format!("&level={l}"));
        }
        if let Some(s) = source.filter(|s| !s.is_empty()) {
            q.push_str(&format!("&source={s}"));
        }
        Ok(serde_json::from_value(self.get_json(&q, true).await?)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_body_matches_gateway_shape() {
        let v = serde_json::json!({
            "blocks": [{
                "header": {
                    "index": 0, "prev_hash": "GENESIS", "hash": "ab",
                    "timestamp": 1, "view": 0, "seq": 1, "proposer": 0
                },
                "record": {
                    "watermark": "wm-1", "session_id": "s", "timestamp": 1,
                    "user_id": "u", "file_hash": "a", "payload_hash": "b",
                    "auth_server": "auth", "signature": "sig"
                }
            }],
            "error": null
        });
        let body: BlocksBody = serde_json::from_value(v).unwrap();
        assert_eq!(body.blocks.len(), 1);
        assert_eq!(body.blocks[0].record.watermark, "wm-1");
        // Proof badge logic: recomputed hash is deterministic.
        let h1 = sagex_ledger::model::Block::compute_hash(
            0, "GENESIS", 1, 0, 1, 0, &body.blocks[0].record,
        );
        let h2 = sagex_ledger::model::Block::compute_hash(
            0, "GENESIS", 1, 0, 1, 0, &body.blocks[0].record,
        );
        assert_eq!(h1, h2);
    }

    #[test]
    fn logs_body_matches_gateway_shape() {
        let v = serde_json::json!({
            "gateway": [
                {"ts": "t", "level": "INFO", "target": "gw", "message": "m"}
            ],
            "nodes": [
                {"node_addr": "127.0.0.1:7000",
                 "entries": [
                    {"ts": "t", "level": "WARN", "target": "n", "message": "w"}
                 ],
                 "error": null}
            ]
        });
        let body: LogsBody = serde_json::from_value(v).unwrap();
        assert_eq!(body.gateway.unwrap().len(), 1);
        let nodes = body.nodes.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].entries.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn status_body_matches_gateway_shape() {
        let v = serde_json::json!({
            "outbox_pending": 3,
            "ledger": [{"type": "StatusResult", "node_id": 0, "height": 9}]
        });
        let body: StatusBody = serde_json::from_value(v).unwrap();
        assert_eq!(body.outbox_pending, 3);
        assert_eq!(body.ledger.len(), 1);
    }
}
