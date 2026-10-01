use std::path::PathBuf;

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(name = "sagex-ledger-node", about = "SAGEX ledger node compiled-service")]
struct Args {
    #[arg(long, default_value = "config.toml")]
    config: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    node_id: String,
    bind_addr: String,
    #[serde(default = "default_data_file")]
    data_file: String,
    #[serde(default)]
    peers: Vec<String>,
    #[serde(default = "default_quorum")]
    quorum: usize,
    #[serde(default)]
    ca_cert_pem_path: String,
    #[serde(default)]
    bearer_token_env: String,
}
fn default_data_file() -> String {
    "chain.jsonl".into()
}
fn default_quorum() -> usize {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Record {
    recipient_id: String,
    watermark_id: String,
    doc_hash: String,
    recipient_dsa_pub: String,
    session_id: String,
    timestamp_ms: i64,
    signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Block {
    height: u64,
    prev_hash: String,
    record: Record,
    block_hash: String,
}

type Chain = Arc<Mutex<Vec<Block>>>;

fn hash_block(height: u64, prev: &str, r: &Record) -> String {
    let mut h = Sha256::new();
    h.update(height.to_le_bytes());
    h.update(prev.as_bytes());
    h.update(serde_json::to_vec(r).unwrap_or_default());
    hex::encode(h.finalize())
}

async fn load_chain(path: &str) -> Vec<Block> {
    let Ok(t) = tokio::fs::read_to_string(path).await else {
        return vec![];
    };
    t.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

async fn persist(path: &str, b: &Block) {
    if let Ok(mut f) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        use tokio::io::AsyncWriteExt;
        let mut s = serde_json::to_string(b).unwrap_or_default();
        s.push('\n');
        let _ = f.write_all(s.as_bytes()).await;
    }
}

fn check_auth(cfg: &Config, h: &HeaderMap) -> bool {
    // CA cert must exist (provisioned pre-start); bearer optional.
    if !cfg.ca_cert_pem_path.is_empty()
        && !std::path::Path::new(&cfg.ca_cert_pem_path).exists()
    {
        return false;
    }
    if cfg.bearer_token_env.is_empty() {
        return true;
    }
    let Ok(want) = std::env::var(&cfg.bearer_token_env) else {
        return true; // open if env not set (telemetry-friendly)
    };
    let got = h
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    got == format!("Bearer {want}")
}

async fn health() -> &'static str {
    "ok"
}

async fn get_chain(State(c): State<(Chain, Config)>) -> Json<Vec<Block>> {
    Json(c.0.lock().await.clone())
}

async fn get_record(
    State(c): State<(Chain, Config)>,
    axum::extract::Path(wm): axum::extract::Path<String>,
) -> Result<Json<Block>, StatusCode> {
    let chain = c.0.lock().await;
    chain
        .iter()
        .find(|b| b.record.watermark_id == wm)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn append(
    State(st): State<(Chain, Config)>,
    headers: HeaderMap,
    Json(rec): Json<Record>,
) -> Result<Json<Block>, StatusCode> {
    if !check_auth(&st.1, &headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if rec.recipient_id.is_empty() || rec.watermark_id.is_empty() || rec.signature.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut chain = st.0.lock().await;
    if chain.iter().any(|b| b.record.watermark_id == rec.watermark_id) {
        return Err(StatusCode::CONFLICT);
    }
    let height = chain.len() as u64;
    let prev = chain.last().map(|b| b.block_hash.clone()).unwrap_or_else(|| "GENESIS".into());
    let block_hash = hash_block(height, &prev, &rec);
    let block = Block { height, prev_hash: prev, record: rec, block_hash };
    persist(&st.1.data_file, &block).await;
    // Best-effort fan-out to peers (quorum reached when peers ack; fire-and-forget here).
    let peers = st.1.peers.clone();
    let payload = block.clone();
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        for p in peers {
            let _ = client.post(format!("{p}/internal/append-replica")).json(&payload).send().await;
        }
    });
    chain.push(block.clone());
    Ok(Json(block))
}

async fn append_replica(State(st): State<(Chain, Config)>, Json(b): Json<Block>) -> StatusCode {
    let mut chain = st.0.lock().await;
    if chain.iter().any(|x| x.block_hash == b.block_hash) {
        return StatusCode::OK;
    }
    if chain.len() as u64 != b.height {
        return StatusCode::CONFLICT;
    }
    let expect = hash_block(b.height, &b.prev_hash, &b.record);
    if expect != b.block_hash {
        return StatusCode::BAD_REQUEST;
    }
    persist(&st.1.data_file, &b).await;
    chain.push(b);
    StatusCode::OK
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    // Resolve config relative to CWD (= env dir under bwrap).
    let text = std::fs::read_to_string(&args.config)?;
    let cfg: Config = toml::from_str(&text)?;
    let dotenv = std::path::Path::new(".env");
    if dotenv.exists() {
        let _ = dotenvy::from_path(dotenv);
    }
    let chain: Chain = Arc::new(Mutex::new(load_chain(&cfg.data_file).await));
    let state = (chain, cfg.clone());
    let app = Router::new()
        .route("/health", get(health))
        .route("/chain", get(get_chain))
        .route("/record/{watermark_id}", get(get_record))
        .route("/internal/append", post(append))
        .route("/internal/append-replica", post(append_replica))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    println!("sagex-ledger-node {} on {}", cfg.node_id, cfg.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
