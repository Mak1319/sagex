use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{info, warn};

use sagex_ledger::model::{Block, DecryptionRecord};
use sagex_ledger::proto::{Envelope, Message, Response, ResponseEnvelope};

#[derive(Debug, Clone)]
pub struct SubmitResult {
    pub block_index: u64,
    pub block_hash: String,
    pub duplicate: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("all {0} ledger nodes unreachable: {1}")]
    Unreachable(usize, String),
    #[error("ledger rejected record: {0}")]
    Rejected(String),
    #[error("timeout after {0}ms")]
    Timeout(u64),
    #[error("protocol: {0}")]
    Protocol(String),
}

/// Thin TCP client for the ledger JSON-lines protocol.
/// Connects per request, rotates nodes for failover / load spreading.
#[derive(Debug)]
pub struct LedgerClient {
    pub addrs: Vec<String>,
    pub submit_timeout: Duration,
    pub query_timeout: Duration,
    next_req: AtomicU64,
    next_node: AtomicUsize,
}

impl LedgerClient {
    pub fn new(addrs: Vec<String>, submit_timeout_ms: u64, query_timeout_ms: u64) -> Self {
        Self {
            addrs,
            submit_timeout: Duration::from_millis(submit_timeout_ms.max(1000)),
            query_timeout: Duration::from_millis(query_timeout_ms.max(1000)),
            next_req: AtomicU64::new(1),
            next_node: AtomicUsize::new(0),
        }
    }

    fn req_id(&self) -> u64 {
        self.next_req.fetch_add(1, Ordering::Relaxed)
    }

    /// Node order starting at a rotating offset.
    fn order(&self) -> Vec<usize> {
        let n = self.addrs.len();
        let start = self.next_node.fetch_add(1, Ordering::Relaxed) % n.max(1);
        (0..n).map(|i| (start + i) % n).collect()
    }

    async fn roundtrip(
        &self,
        addr: &str,
        env: Envelope,
        timeout: Duration,
    ) -> anyhow::Result<ResponseEnvelope> {
        let ms = timeout.as_millis() as u64;
        let work = async {
            let sock = TcpStream::connect(addr)
                .await
                .map_err(|e| anyhow::anyhow!("connect {addr}: {e}"))?;
            let (rh, mut wh) = sock.into_split();
            let mut line = serde_json::to_string(&env)?;
            line.push('\n');
            wh.write_all(line.as_bytes()).await?;
            let mut lines = BufReader::new(rh).lines();
            let resp = lines
                .next_line()
                .await?
                .ok_or_else(|| anyhow::anyhow!("eof from {addr}"))?;
            let out: ResponseEnvelope = serde_json::from_str(&resp)?;
            Ok(out)
        };
        match tokio::time::timeout(timeout, work).await {
            Ok(r) => r,
            Err(_) => Err(anyhow::anyhow!(ClientError::Timeout(ms))),
        }
    }

    /// Submit a record; tries every node until one commits it.
    /// A `duplicate: already committed` reply counts as success.
    pub async fn submit(&self, record: &DecryptionRecord) -> Result<SubmitResult, ClientError> {
        if self.addrs.is_empty() {
            return Err(ClientError::Unreachable(0, "no ledger nodes configured".into()));
        }
        let mut last_err = String::new();
        for idx in self.order() {
            let addr = &self.addrs[idx];
            let env = Envelope {
                req_id: Some(self.req_id()),
                from: None,
                msg: Message::Submit { record: record.clone() },
            };
            match self.roundtrip(addr, env, self.submit_timeout).await {
                Ok(resp) => match resp.resp {
                    Response::Reply { ok: true, block_index, block_hash, error } => {
                        let duplicate = error
                            .as_deref()
                            .map(|e| e.contains("duplicate"))
                            .unwrap_or(false);
                        if duplicate {
                            info!("ledger {addr}: duplicate {wm} already committed", wm = record.watermark);
                        }
                        return Ok(SubmitResult {
                            block_index: block_index.unwrap_or(0),
                            block_hash: block_hash.unwrap_or_default(),
                            duplicate,
                        });
                    }
                    Response::Reply { ok: false, error, .. } => {
                        let e = error.unwrap_or_else(|| "unknown".into());
                        warn!("ledger {addr} rejected {wm}: {e}", wm = record.watermark);
                        return Err(ClientError::Rejected(e));
                    }
                    other => {
                        last_err = format!("unexpected response from {addr}: {other:?}");
                    }
                },
                Err(e) => {
                    last_err = format!("{addr}: {e:#}");
                    warn!("ledger submit via {addr} failed: {e:#}");
                }
            }
        }
        Err(ClientError::Unreachable(self.addrs.len(), last_err))
    }

    async fn query_first(&self, msg: Message) -> anyhow::Result<Vec<Block>> {
        let mut last: Option<Response> = None;
        for idx in self.order() {
            let addr = &self.addrs[idx];
            let env = Envelope { req_id: Some(self.req_id()), from: None, msg: msg.clone() };
            match self.roundtrip(addr, env, self.query_timeout).await {
                Ok(resp) => match resp.resp {
                    Response::QueryResult { blocks, error } => {
                        if !blocks.is_empty() {
                            return Ok(blocks);
                        }
                        // Empty: keep looking on other nodes (replication lag),
                        // remember the last answer as fallback.
                        last = Some(Response::QueryResult { blocks, error });
                    }
                    other => {
                        last = Some(other);
                    }
                },
                Err(e) => warn!("ledger query via {addr} failed: {e:#}"),
            }
        }
        match last {
            Some(Response::QueryResult { blocks, .. }) => Ok(blocks),
            Some(other) => Err(anyhow::anyhow!("unexpected: {other:?}")),
            None => Err(anyhow::anyhow!(ClientError::Unreachable(
                self.addrs.len(),
                "no ledger node answered".into()
            ))),
        }
    }

    pub async fn query_watermark(&self, watermark: &str) -> anyhow::Result<Vec<Block>> {
        self.query_first(Message::QueryWatermark { watermark: watermark.into() }).await
    }

    pub async fn query_user(&self, user_id: &str) -> anyhow::Result<Vec<Block>> {
        self.query_first(Message::QueryUser { user_id: user_id.into() }).await
    }

    pub async fn get_block(&self, index: u64) -> anyhow::Result<Vec<Block>> {
        self.query_first(Message::GetBlock { index }).await
    }

    /// Fan-out Status to every reachable node (forensic / ops overview).
    pub async fn status_all(&self) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        for addr in &self.addrs {
            let env = Envelope { req_id: Some(self.req_id()), from: None, msg: Message::Status {} };
            match self.roundtrip(addr, env.clone(), self.query_timeout).await {
                Ok(resp) => {
                    let mut v = serde_json::to_value(&resp.resp).unwrap_or(serde_json::Value::Null);
                    v["node_addr"] = serde_json::Value::String(addr.clone());
                    out.push(v);
                }
                Err(e) => {
                    out.push(serde_json::json!({
                        "type": "Unreachable", "node_addr": addr, "error": format!("{e:#}")
                    }));
                }
            }
        }
        out
    }
}
