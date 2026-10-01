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

#[derive(Parser, Debug)]
#[command(name = "sagex-register-gateway", about = "SAGEX register gateway compiled-service")]
struct Args {
    #[arg(long, default_value = "config.toml")]
    config: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    bind_addr: String,
    ledger_nodes: Vec<String>,
    #[serde(default = "default_pubs")]
    trusted_pubs_dir: String,
    #[serde(default = "default_ca")]
    ca_cert_pem_path: String,
    #[serde(default = "default_true")]
    read_open: bool,
    #[serde(default)]
    bearer_token_env: String,
    /// Optional file (one bearer token per line, `#` comments) with extra
    /// accepted login keys — e.g. desktop telemetry app keys. Lines are
    /// trimmed; empty lines ignored. Never commit; lives in the env dir.
    #[serde(default)]
    bearer_tokens_file: String,
}
fn default_pubs() -> String {
    "trusted-pubs".into()
}
fn default_ca() -> String {
    "ca-cert.pem".into()
}
fn default_true() -> bool {
    true
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

fn check_ca(cfg: &Config, h: &HeaderMap) -> bool {
    // Proper CA certificate must be provisioned pre-start; bearer proves possession.
    if !std::path::Path::new(&cfg.ca_cert_pem_path).exists() {
        return false;
    }
    let got = h.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("");
    if !cfg.bearer_token_env.is_empty() {
        let Ok(want) = std::env::var(&cfg.bearer_token_env) else {
            return false;
        };
        if got == format!("Bearer {want}") {
            return true;
        }
    } else {
        // No single-token gate configured: the env-var token (if set) or the
        // tokens file decides. Fall through to the file check.
    }
    if cfg.bearer_tokens_file.is_empty() {
        // No file gate either: open only when no env gate was configured.
        return cfg.bearer_token_env.is_empty();
    }
    let Ok(text) = std::fs::read_to_string(&cfg.bearer_tokens_file) else {
        return false;
    };
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).any(
        |tok| got == format!("Bearer {tok}"),
    )
}

fn check_pub(cfg: &Config, rec: &Record) -> bool {
    // recipient_id must match trusted-pubs/<recipient_id>.pub filename (keygen --username),
    // and file content must contain the claimed recipient_dsa_pub hex.
    if rec.recipient_id.is_empty() || rec.recipient_id.contains('/') || rec.recipient_id.contains('.') {
        return false;
    }
    let p = PathBuf::from(&cfg.trusted_pubs_dir).join(format!("{}.pub", rec.recipient_id));
    let Ok(content) = std::fs::read_to_string(&p) else {
        return false;
    };
    !rec.recipient_dsa_pub.is_empty() && content.contains(&rec.recipient_dsa_pub)
}

async fn health() -> &'static str {
    "ok"
}

async fn register(
    State(cfg): State<Config>,
    headers: HeaderMap,
    Json(rec): Json<Record>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !check_ca(&cfg, &headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if !check_pub(&cfg, &rec) {
        return Err(StatusCode::FORBIDDEN);
    }
    if rec.watermark_id.is_empty() || rec.signature.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let client = reqwest::Client::new();
    let mut ok = 0;
    let mut last: Option<serde_json::Value> = None;
    // Auth to ledger nodes with our own node bearer (never forward the caller's).
    let node_auth = std::env::var("LEDGER_BEARER").map(|t| format!("Bearer {t}")).ok();
    for node in &cfg.ledger_nodes {
        let mut req = client.post(format!("{node}/internal/append")).json(&rec);
        if let Some(a) = node_auth.clone() {
            req = req.header("authorization", a);
        }
        if let Ok(r) = req.send().await
            && let Ok(v) = r.json::<serde_json::Value>().await
        {
            ok += 1;
            last = Some(v);
        }
    }
    if ok == 0 {
        return Err(StatusCode::BAD_GATEWAY);
    }
    Ok(Json(serde_json::json!({"acks": ok, "block": last})))
}

async fn proxy_get(cfg: Config, path: &str) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = reqwest::Client::new();
    for node in &cfg.ledger_nodes {
        if let Ok(r) = client.get(format!("{node}{path}")).send().await
            && let Ok(v) = r.json::<serde_json::Value>().await
        {
            return Ok(Json(v));
        }
    }
    Err(StatusCode::BAD_GATEWAY)
}

async fn records(State(cfg): State<Config>) -> Result<Json<serde_json::Value>, StatusCode> {
    if !cfg.read_open {
        return Err(StatusCode::FORBIDDEN);
    }
    proxy_get(cfg, "/chain").await
}

async fn record_one(
    State(cfg): State<Config>,
    axum::extract::Path(wm): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !cfg.read_open {
        return Err(StatusCode::FORBIDDEN);
    }
    proxy_get(cfg, &format!("/record/{wm}")).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let text = std::fs::read_to_string(&args.config)?;
    let cfg: Config = toml::from_str(&text)?;
    if std::path::Path::new(".env").exists() {
        let _ = dotenvy::from_path(".env");
    }
    let app = Router::new()
        .route("/health", get(health))
        .route("/register", post(register))
        .route("/records", get(records))
        .route("/records/{watermark_id}", get(record_one))
        .route("/verify/{watermark_id}", get(record_one))
        .with_state(cfg.clone());
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    println!("sagex-register-gateway on {}", cfg.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
