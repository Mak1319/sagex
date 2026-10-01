use serde::{Deserialize, Serialize};

use crate::logbuf::LogEntry;
use crate::model::{Block, DecryptionRecord};

/// JSON-lines wire protocol. One JSON object per line over TCP.
/// Node<->node consensus messages + register-gate ingress + forensic queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    // ---- ingress (register gate -> any node) ----
    #[serde(rename = "Submit")]
    Submit { record: DecryptionRecord },
    /// Internal forward to leader when a replica receives Submit.
    #[serde(rename = "ForwardSubmit")]
    ForwardSubmit {
        record: DecryptionRecord,
        origin: u64,
    },

    // ---- PBFT ----
    #[serde(rename = "PrePrepare")]
    PrePrepare {
        view: u64,
        seq: u64,
        block: Block,
        sig: String,
    },
    #[serde(rename = "Prepare")]
    Prepare {
        view: u64,
        seq: u64,
        digest: String,
        node_id: u64,
        sig: String,
    },
    #[serde(rename = "Commit")]
    Commit {
        view: u64,
        seq: u64,
        digest: String,
        node_id: u64,
        sig: String,
    },
    #[serde(rename = "ViewChange")]
    ViewChange { new_view: u64, node_id: u64 },

    // ---- queries (forensic tooling -> any node, TCP only) ----
    #[serde(rename = "QueryWatermark")]
    QueryWatermark { watermark: String },
    #[serde(rename = "QueryUser")]
    QueryUser { user_id: String },
    #[serde(rename = "GetBlock")]
    GetBlock { index: u64 },
    #[serde(rename = "Status")]
    Status {},
    /// Auditor log tail served from the node's in-memory ring buffer.
    /// Answered directly (no consensus); the gateway fans this out.
    #[serde(rename = "GetLogs")]
    GetLogs {
        limit: u64,
        #[serde(default)]
        level: Option<String>,
    },
    /// Peer handshake: first line on a peer link in both directions.
    #[serde(rename = "Hello")]
    Hello { node_id: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    #[serde(rename = "Reply")]
    Reply {
        ok: bool,
        block_index: Option<u64>,
        block_hash: Option<String>,
        error: Option<String>,
    },
    #[serde(rename = "QueryResult")]
    QueryResult {
        blocks: Vec<Block>,
        error: Option<String>,
    },
    #[serde(rename = "StatusResult")]
    StatusResult {
        node_id: u64,
        height: u64,
        tip_hash: String,
        view: u64,
        seq: u64,
    },
    #[serde(rename = "LogsResult")]
    LogsResult {
        entries: Vec<LogEntry>,
        error: Option<String>,
    },
    #[serde(rename = "Error")]
    Error { error: String },
}

/// Envelope used on peer links so receivers can route replies.
/// `req_id` correlates Submit->Reply for the originating client connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub req_id: Option<u64>,
    pub from: Option<u64>,
    #[serde(flatten)]
    pub msg: Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub req_id: Option<u64>,
    #[serde(flatten)]
    pub resp: Response,
}
