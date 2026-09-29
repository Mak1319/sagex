use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use sagex_ledger::model::DecryptionRecord;

use crate::ledger_client::{ClientError, LedgerClient};
use crate::outbox::{Outbox, OutboxEntry, STATUS_DONE};

#[derive(Clone)]
pub struct AppState {
    pub outbox: Outbox,
    pub ledger: Arc<LedgerClient>,
    pub retry_batch: usize,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub watermark: String,
    pub status: String, // "committed" | "queued"
    pub block_index: Option<u64>,
    pub block_hash: Option<String>,
    pub duplicate: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

async fn attempt_submit(
    outbox: &Outbox,
    ledger: &LedgerClient,
    record: &DecryptionRecord,
) -> RegisterResponse {
    let wm = record.watermark.clone();
    match ledger.submit(record).await {
        Ok(r) => {
            let _ = outbox.mark_done(&wm, r.block_index, &r.block_hash);
            info!("gateway committed {wm} idx={} dup={}", r.block_index, r.duplicate);
            RegisterResponse {
                watermark: wm,
                status: "committed".into(),
                block_index: Some(r.block_index),
                block_hash: Some(r.block_hash),
                duplicate: r.duplicate,
                detail: None,
            }
        }
        Err(ClientError::Rejected(e)) => {
            let _ = outbox.mark_failed(&wm, &e);
            warn!("gateway ledger rejected {wm}: {e}");
            RegisterResponse {
                watermark: wm,
                status: "failed".into(),
                block_index: None,
                block_hash: None,
                duplicate: false,
                detail: Some(e),
            }
        }
        Err(e) => {
            let _ = outbox.mark_retryable(&wm, &e.to_string());
            warn!("gateway queued {wm} for retry: {e}");
            RegisterResponse {
                watermark: wm,
                status: "queued".into(),
                block_index: None,
                block_hash: None,
                duplicate: false,
                detail: Some(e.to_string()),
            }
        }
    }
}

/// POST /register — structural validation only (pass-through: no signature
/// verification, no key lookup). Durable: persisted before the first attempt.
async fn post_register(
    State(st): State<Arc<AppState>>,
    Json(record): Json<DecryptionRecord>,
) -> impl IntoResponse {
    if let Err(e) = record.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorBody { error: format!("invalid record: {e:#}") })),
        )
            .into_response();
    }
    let wm = record.watermark.clone();
    match st.outbox.upsert_pending(&record) {
        Ok((existing, _)) if existing.status == STATUS_DONE => {
            return (
                StatusCode::OK,
                Json(serde_json::json!(RegisterResponse {
                    watermark: wm,
                    status: "committed".into(),
                    block_index: existing.block_index,
                    block_hash: existing.block_hash,
                    duplicate: true,
                    detail: Some("already registered".into()),
                })),
            )
                .into_response();
        }
        Ok(_) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!(ErrorBody { error: e.to_string() })),
            )
                .into_response();
        }
    }
    let _ = st.outbox.set_inflight(&wm);
    let resp = attempt_submit(&st.outbox, &st.ledger, &record).await;
    let code = match resp.status.as_str() {
        "committed" => StatusCode::CREATED,
        "queued" => StatusCode::ACCEPTED,
        _ => StatusCode::BAD_GATEWAY,
    };
    (code, Json(serde_json::json!(resp))).into_response()
}

async fn get_record(
    State(st): State<Arc<AppState>>,
    Path(watermark): Path<String>,
) -> impl IntoResponse {
    match st.ledger.query_watermark(&watermark).await {
        Ok(blocks) if !blocks.is_empty() => (StatusCode::OK, Json(serde_json::json!({ "blocks": blocks, "error": null }))).into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "blocks": [], "error": "not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "blocks": [], "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct UserQuery {
    user_id: String,
}

async fn get_records_by_user(
    State(st): State<Arc<AppState>>,
    Query(q): Query<UserQuery>,
) -> impl IntoResponse {
    match st.ledger.query_user(&q.user_id).await {
        Ok(blocks) => (StatusCode::OK, Json(serde_json::json!({ "blocks": blocks, "error": null }))).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "blocks": [], "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_block(State(st): State<Arc<AppState>>, Path(index): Path<u64>) -> impl IntoResponse {
    match st.ledger.get_block(index).await {
        Ok(blocks) if !blocks.is_empty() => (StatusCode::OK, Json(serde_json::json!({ "blocks": blocks, "error": null }))).into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "blocks": [], "error": "not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "blocks": [], "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_status(State(st): State<Arc<AppState>>) -> impl IntoResponse {
    let nodes = st.ledger.status_all().await;
    let pending = st.outbox.pending_count().unwrap_or(-1);
    (
        StatusCode::OK,
        Json(serde_json::json!({ "outbox_pending": pending, "ledger": nodes })),
    )
        .into_response()
}

async fn get_outbox(State(st): State<Arc<AppState>>) -> impl IntoResponse {
    match st.outbox.list(100) {
        Ok(entries) => (StatusCode::OK, Json(serde_json::json!(entries))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!(ErrorBody { error: e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_health() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response()
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/register", post(post_register))
        .route("/record/:watermark", get(get_record))
        .route("/records", get(get_records_by_user))
        .route("/block/:index", get(get_block))
        .route("/status", get(get_status))
        .route("/outbox", get(get_outbox))
        .route("/health", get(get_health))
        .with_state(state)
}

/// Background worker: drain PENDING outbox rows with at-least-once delivery
/// (ledger dedupes by watermark, so redelivery is safe).
pub async fn outbox_worker(state: Arc<AppState>, interval: std::time::Duration) {
    loop {
        tokio::time::sleep(interval).await;
        let batch = match state.outbox.claim_pending(state.retry_batch.max(1)) {
            Ok(b) => b,
            Err(e) => {
                warn!("outbox claim failed: {e}");
                continue;
            }
        };
        if batch.is_empty() {
            continue;
        }
        info!("outbox worker: retrying {} rows", batch.len());
        for entry in batch {
            let e: OutboxEntry = entry;
            let resp = attempt_submit(&state.outbox, &state.ledger, &e.record).await;
            info!("outbox worker: {} -> {}", e.watermark, resp.status);
        }
    }
}
