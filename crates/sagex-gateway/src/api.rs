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

use crate::auth::AuthVerifier;
use crate::ledger_client::{ClientError, LedgerClient};
use crate::outbox::{Outbox, OutboxEntry, STATUS_DONE};

#[derive(Clone)]
pub struct AppState {
    pub outbox: Outbox,
    pub ledger: Arc<LedgerClient>,
    pub retry_batch: usize,
    pub auth: AuthVerifier,
    /// Required permit subject for `GET /logs` (from `[logs].auditor_sub`).
    pub auditor_sub: String,
    /// Serve `GET /logs` at all (from `[logs].enabled`).
    pub logs_enabled: bool,
    /// SHA-256 fingerprint of this gateway's RG public key (for ops +
    /// CA pin comparison). Served on `GET /status`.
    pub rg_fingerprint: String,
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

/// POST /register — intake for decryption records.
///
/// Two modes, selected by `[auth].enabled` (default true):
/// - enabled: body must be `{permit, record}`. The permit is verified with
///   the CA-auth strategy (same code as sagex-certauth): signature, `kid`/
///   `iss` pin, expiry, `sub == record.user_id` binding, single-use JTI.
///   Failures are 401/403/409 and nothing is stored.
/// - disabled (explicit opt-in for open demos): body is the bare record, as
///   before — no identity is established.
///
/// In both modes the record itself is structurally validated and flows
/// through the durable outbox exactly as before.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RegisterBody {
    /// Authenticated intake: server-issued JOSE permit + record.
    Envelope { permit: String, record: DecryptionRecord },
    /// Legacy open intake: bare record (only when auth is disabled).
    Bare(DecryptionRecord),
}

async fn post_register(
    State(st): State<Arc<AppState>>,
    Json(body): Json<RegisterBody>,
) -> impl IntoResponse {
    let (permit, record) = match body {
        RegisterBody::Envelope { permit, record } => (Some(permit), record),
        RegisterBody::Bare(record) => (None, record),
    };
    if let Err(e) = record.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!(ErrorBody { error: format!("invalid record: {e:#}") })),
        )
            .into_response();
    }
    let wm = record.watermark.clone();
    // Idempotent retry: an already-committed watermark short-circuits here,
    // before any permit is consumed.
    match st.outbox.get(&wm) {
        Ok(Some(existing)) if existing.status == STATUS_DONE => {
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
    if st.auth.enabled {
        let permit = match permit {
            Some(p) => p,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!(ErrorBody { error: "permit required".into() })),
                )
                    .into_response();
            }
        };
        let claims = match st.auth.verify_permit(&permit) {
            Ok(c) => c,
            Err(e) => {
                warn!("gateway permit rejected for {wm}: {e}");
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!(ErrorBody { error: "invalid permit".into() })),
                )
                    .into_response();
            }
        };
        // Jurisdiction: intake accepts user permits only. A server permit
        // (e.g. auditor) is cryptographically valid yet useless here.
        if claims.kind != sagex_auth::PERMIT_KIND_USER {
            warn!("gateway non-user permit kind {:?} on intake ({wm})", claims.kind);
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!(ErrorBody {
                    error: "permit kind not accepted for intake".into()
                })),
            )
                .into_response();
        }
        // Binding: the permit's subject must equal the record's user_id.
        // `user_id` stays a free-form string — plain equality, no charset rules.
        if claims.sub != record.user_id {
            warn!(
                "gateway permit sub {:?} != record user_id {:?} ({wm})",
                claims.sub, record.user_id
            );
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!(ErrorBody {
                    error: "permit does not cover user_id".into()
                })),
            )
                .into_response();
        }
        match st.outbox.jti_spent(&claims.jti) {
            Ok(true) => {
                warn!("gateway replayed permit jti {} ({wm})", claims.jti);
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!(ErrorBody {
                        error: "permit already used".into()
                    })),
                )
                    .into_response();
            }
            Ok(false) => {}
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!(ErrorBody { error: e.to_string() })),
                )
                    .into_response();
            }
        }
        // Burn before the first submit attempt (same strictness as
        // sagex-certauth): a failed/rejected submit consumes its permit.
        if let Err(e) = st.outbox.burn_jti(&claims.jti, &claims.sub) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!(ErrorBody { error: e.to_string() })),
            )
                .into_response();
        }
    }
    // DONE rows short-circuit above; anything left here is fresh or retryable.
    if let Err(e) = st.outbox.upsert_pending(&record) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!(ErrorBody { error: e.to_string() })),
        )
            .into_response();
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
        Json(serde_json::json!({
            "outbox_pending": pending,
            "ledger": nodes,
            "rg_fingerprint": st.rg_fingerprint,
        })),
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

#[derive(Debug, Deserialize)]
struct LogsQuery {
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    limit: Option<u64>,
    /// `gateway`, a node addr (as listed in config), or omitted = everything.
    #[serde(default)]
    source: Option<String>,
}

/// Restricted auditor log view: this process's ring buffer plus a fan-out
/// tail from every ledger node.
///
/// Access is gated by a CA-issued **auditor permit** (`Authorization: Bearer
/// <token>`, e.g. `sagex-certauth issue-permit --identity ledger-auditor`):
/// same `sagex-auth` verification as intake, but the permit's subject must
/// equal `[logs].auditor_sub` — and unlike intake permits the JTI is *not*
/// burned, so one permit serves a whole UI session (expiry still enforced).
/// Missing/malformed/invalid → 401, wrong subject → 403.
async fn get_logs(
    State(st): State<Arc<AppState>>,
    Query(q): Query<LogsQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if !st.logs_enabled {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!(ErrorBody { error: "logs disabled".into() })),
        )
            .into_response();
    }
    if st.auth.enabled {
        let token = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let token = match token {
            Some(t) => t,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!(ErrorBody {
                        error: "auditor permit required".into()
                    })),
                )
                    .into_response();
            }
        };
        let claims = match st.auth.verify_permit(token) {
            Ok(c) => c,
            Err(_) => {
                warn!("gateway /logs permit rejected");
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!(ErrorBody { error: "invalid permit".into() })),
                )
                    .into_response();
            }
        };
        if claims.sub != st.auditor_sub {
            warn!("gateway /logs permit sub {:?} is not the auditor", claims.sub);
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!(ErrorBody {
                    error: "permit is not an auditor credential".into()
                })),
            )
                .into_response();
        }
        // Jurisdiction: auditor access requires a server-kind permit. A user
        // permit for the same subject name is rejected all the same.
        if claims.kind != sagex_auth::PERMIT_KIND_SERVER {
            warn!("gateway /logs non-server permit kind {:?}", claims.kind);
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!(ErrorBody {
                    error: "permit is not an auditor credential".into()
                })),
            )
                .into_response();
        }
    }
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let want_gateway = q.source.as_deref().is_none_or(|s| s == "gateway");
    let gateway = if want_gateway {
        Some(sagex_ledger::LogBuffer::global().snapshot(q.level.as_deref(), limit as usize))
    } else {
        None
    };
    let nodes = match q.source.as_deref() {
        None | Some("all") | Some("nodes") => {
            Some(st.ledger.node_logs_all(limit, q.level.clone()).await)
        }
        Some("gateway") => Some(vec![]),
        Some(addr) => Some(vec![st.ledger.node_logs_one(addr, limit, q.level.clone()).await]),
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({ "gateway": gateway, "nodes": nodes })),
    )
        .into_response()
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/register", post(post_register))
        .route("/record/:watermark", get(get_record))
        .route("/records", get(get_records_by_user))
        .route("/block/:index", get(get_block))
        .route("/status", get(get_status))
        .route("/outbox", get(get_outbox))
        .route("/logs", get(get_logs))
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
