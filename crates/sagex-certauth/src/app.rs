//! HTTP layer: shared state, routes, and the permit-gated CSR handler.
//!
//! Verification order in `post_csr` is load-bearing (fail fast, cheapest
//! first, no state mutations until all checks pass):
//! permit -> kind(user-only) -> binding(sub==identity) ->
//! triple-bind(chatsrv token, optional unless strict) -> CSR/PoP ->
//! replay(jti) -> duplicate identity -> issue -> persist.

use axum::{Json, Router, extract::{Path, State}, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post}};
use mongodb::bson::{doc, DateTime};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};

use crate::{
    cert,
    csr::{self, CsrRequest},
    mldsa::MlDsa65Signer,
    permit,
};

#[derive(Clone)]
pub struct AppState {
    pub client: mongodb::Client,
    pub db_name: String,
    pub ca_identity: String,
    pub ca_dsa_public: Vec<u8>,
    pub signer: Arc<MlDsa65Signer>,
    pub validity_days: u64,
    /// Pinned BLS (chatsrv) ML-DSA-65 public key + expected kid, for
    /// verifying chatsrv JOSE tokens locally. No syncing, no callbacks.
    pub chatsrv_dsa_public: Vec<u8>,
    pub chatsrv_kid: String,
    /// Pinned RG (gateway) ML-DSA-65 public key + id, for authenticating
    /// key-directory lookups. No syncing.
    pub rg_dsa_public: Vec<u8>,
    pub rg_id: String,
    /// Strict mode: CSR requires a chatsrv token (no legacy permit-only path).
    pub require_chatsrv_token: bool,
    /// TTL for HTTP-minted user permits (seconds; mirrors PERMIT_TTL_HOURS).
    pub permit_ttl_secs: u64,
}

impl AppState {
    pub fn certs(&self) -> mongodb::Collection<mongodb::bson::Document> {
        self.client.database(&self.db_name).collection("certificates")
    }
    pub fn used_permits(&self) -> mongodb::Collection<mongodb::bson::Document> {
        self.client.database(&self.db_name).collection("used_permits")
    }
}

/// Ensure unique indexes: one certificate per identity, one serial each,
/// single-use permit JTIs. Idempotent restarts.
pub async fn ensure_indexes(state: &AppState) -> mongodb::error::Result<()> {
    use mongodb::{IndexModel, options::IndexOptions};
    let uniq = || {
        IndexOptions::builder()
            .unique(true)
            .build()
    };
    let identity_idx = IndexModel::builder()
        .keys(doc! { "identity": 1 })
        .options(uniq())
        .build();
    let serial_idx = IndexModel::builder()
        .keys(doc! { "serial": 1 })
        .options(uniq())
        .build();
    state.certs().create_index(identity_idx).await?;
    state.certs().create_index(serial_idx).await?;
    let jti_idx = IndexModel::builder()
        .keys(doc! { "jti": 1 })
        .options(uniq())
        .build();
    state.used_permits().create_index(jti_idx).await?;
    Ok(())
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    service: &'static str,
    db: &'static str,
    db_name: String,
    ca_identity: String,
}

#[derive(Serialize)]
struct DbPing {
    db_name: String,
    ok: bool,
}

async fn root() -> &'static str {
    "Hello from sagex-certauth!"
}

async fn health(State(state): State<AppState>) -> Json<Health> {
    let db = if ping_mongo(&state).await {
        "connected"
    } else {
        "disconnected"
    };
    Json(Health {
        status: "ok",
        service: "sagex-certauth",
        db,
        db_name: state.db_name.clone(),
        ca_identity: state.ca_identity.clone(),
    })
}

async fn db_ping(State(state): State<AppState>) -> Json<DbPing> {
    Json(DbPing {
        db_name: state.db_name.clone(),
        ok: ping_mongo(&state).await,
    })
}

pub async fn ping_mongo(state: &AppState) -> bool {
    state
        .client
        .database(&state.db_name)
        .run_command(doc! { "ping": 1 })
        .await
        .is_ok()
}

/// Public API error: safe message for the client; details go to stderr.
pub struct ApiError {
    status: StatusCode,
    message: &'static str,
}

impl ApiError {
    fn new(status: StatusCode, message: &'static str) -> Self {
        Self { status, message }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorBody { error: self.message })).into_response()
    }
}

#[derive(Serialize)]
struct CsrResponse {
    identity: String,
    serial: u64,
    certificate_pem: String,
}

#[derive(Serialize)]
struct PermitResponse {
    permit: String,
    sub: String,
    expires_at: u64,
}

/// POST /v1/permits — self-service user permits, gated by a chatsrv JOSE
/// access token. Verifies the token against the pinned BLS key and mints
/// kind=user with sub=<verified username>; callers can never choose another
/// subject. Missing/invalid token -> 401. (The offline CLI remains the only
/// path for server-kind permits.)
async fn post_permits(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<(StatusCode, Json<PermitResponse>), ApiError> {
    let token = match bearer_token(&headers) {
        Some(t) => t,
        None => {
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "chatsrv token required",
            ))
        }
    };
    let (_sub, username) = sagex_auth::chatsrv_token::verify(
        token,
        &state.chatsrv_dsa_public,
        &state.chatsrv_kid,
    )
    .map_err(|e| {
        eprintln!("WARN: /v1/permits chatsrv token rejected: {e}");
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid chatsrv token")
    })?;
    let permit = permit::mint_for(
        &state.ca_identity,
        &state.signer,
        &username,
        sagex_auth::PERMIT_KIND_USER,
        state.permit_ttl_secs,
    )
    .map_err(|e| {
        eprintln!("ERROR: /v1/permits minting failed: {e}");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "issuance failed")
    })?;
    println!("issued user permit for {username:?} via chatsrv token");
    Ok((
        StatusCode::CREATED,
        Json(PermitResponse {
            permit,
            sub: username,
            expires_at: sagex_auth::now_secs().saturating_add(state.permit_ttl_secs),
        }),
    ))
}

/// Extract a Bearer token from request headers.
fn bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Key-directory entry: a customer's enrolled public keys + certificate
/// pointer, served to the authenticated gateway (RG) only.
#[derive(Serialize)]
struct KeyEntry {
    identity: String,
    key_kem_b64: String,
    key_dsa_b64: String,
    cert_serial: i64,
    fingerprint: String,
}

#[derive(Serialize)]
struct KeyError {
    identity: String,
    error: String,
}

/// SHA-256 hex fingerprint of the stored certificate PEM.
fn cert_fingerprint(pem: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(pem.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

async fn lookup_keys(
    state: &AppState,
    identities: &[String],
) -> Result<(Vec<KeyEntry>, Vec<KeyError>), ApiError> {
    let mut keys = Vec::new();
    let mut errors = Vec::new();
    for identity in identities {
        let doc = state
            .certs()
            .find_one(doc! { "identity": identity })
            .await
            .map_err(|e| {
                eprintln!("ERROR: /v1/keys store lookup failed: {e}");
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "store unavailable")
            })?;
        match doc {
            Some(d) => {
                let get = |k: &str| {
                    d.get_str(k)
                        .map(str::to_string)
                        .map_err(|_| {
                            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "bad record")
                        })
                };
                let pem = get("certificate_pem")?;
                keys.push(KeyEntry {
                    identity: identity.clone(),
                    key_kem_b64: get("key_kem_b64")?,
                    key_dsa_b64: get("key_dsa_b64")?,
                    cert_serial: d.get_i64("serial").map_err(|_| {
                        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "bad record")
                    })?,
                    fingerprint: cert_fingerprint(&pem),
                });
            }
            None => errors.push(KeyError {
                identity: identity.clone(),
                error: "not enrolled".to_string(),
            }),
        }
    }
    Ok((keys, errors))
}

/// GET /v1/keys + GET /v1/keys/:identity — customer public-key directory
/// for the gateway. Authenticated with a short-lived RG JWS (Bearer) minted
/// with the gateway's key and verified against the pinned RG key/identity.
/// Unknown identities yield per-entry errors, never whole-request failure.
async fn keys_auth(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), ApiError> {
    let token = match bearer_token(headers) {
        Some(t) => t,
        None => {
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "RG token required",
            ))
        }
    };
    sagex_auth::rg_token::verify(token, &state.rg_id, &state.rg_dsa_public).map_err(|e| {
        eprintln!("WARN: /v1/keys RG token rejected: {e}");
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid RG token")
    })?;
    Ok(())
}

#[derive(Deserialize)]
struct KeysQuery {
    #[serde(default)]
    identity: String,
}

async fn get_keys(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(q): axum::extract::Query<KeysQuery>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    keys_auth(&state, &headers).await?;
    let identities: Vec<String> = q
        .identity
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(32)
        .map(str::to_string)
        .collect();
    if identities.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "identity query required (max 32)",
        ));
    }
    let (keys, errors) = lookup_keys(&state, &identities).await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "keys": keys, "errors": errors })),
    ))
}

async fn get_key(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(identity): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    keys_auth(&state, &headers).await?;
    let (keys, errors) = lookup_keys(&state, std::slice::from_ref(&identity)).await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "keys": keys, "errors": errors })),
    ))
}

async fn post_csr(
    State(state): State<AppState>,
    Json(req): Json<CsrRequest>,
) -> Result<(StatusCode, Json<CsrResponse>), ApiError> {
    // 1. Permit: proves the caller is a customer of THIS server.
    let claims = permit::verify(&req.permit, &state.ca_identity, &state.ca_dsa_public)
        .map_err(|e| {
            eprintln!("WARN: /v1/csr permit rejected: {e}");
            ApiError::new(StatusCode::UNAUTHORIZED, "invalid permit")
        })?;
    // 1b. Jurisdiction: enrollment accepts user permits only. A server
    // permit (e.g. auditor) is cryptographically valid yet useless here —
    // operator tooling can never enroll user certificates, by construction.
    if claims.kind != sagex_auth::PERMIT_KIND_USER {
        eprintln!("WARN: /v1/csr non-user permit kind {:?}", claims.kind);
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "server permits cannot enroll user certificates",
        ));
    }
    // 2. Binding: the permit's subject must equal the CSR identity.
    //    Without this, a permit for alice could enroll bob's keys.
    if claims.sub != req.csr.identity {
        eprintln!(
            "WARN: /v1/csr permit sub {:?} != csr identity {:?}",
            claims.sub, req.csr.identity
        );
        return Err(ApiError::new(StatusCode::FORBIDDEN, "permit does not cover identity"));
    }
    // 2b. Triple bind (optional unless strict): a chatsrv JOSE token proves
    // the requester owns the chatsrv account, and the CSR identity must equal
    // that username — which must equal the permit subject above.
    let chatsrv_user: Option<String> = match &req.chatsrv_token {
        None if state.require_chatsrv_token => {
            return Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "chatsrv token required",
            ));
        }
        None => None,
        Some(token) => {
            let (_sub, username) = sagex_auth::chatsrv_token::verify(
                token,
                &state.chatsrv_dsa_public,
                &state.chatsrv_kid,
            )
            .map_err(|e| {
                eprintln!("WARN: /v1/csr chatsrv token rejected: {e}");
                ApiError::new(StatusCode::UNAUTHORIZED, "invalid chatsrv token")
            })?;
            if username != req.csr.identity || username != claims.sub {
                eprintln!(
                    "WARN: /v1/csr triple-bind mismatch token={username:?} csr={:?} permit={:?}",
                    req.csr.identity, claims.sub
                );
                return Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "chatsrv identity does not match",
                ));
            }
            Some(username)
        }
    };
    if let Some(u) = &chatsrv_user {
        println!("CSR triple-bound to chatsrv user {u:?}");
    }
    // 3. CSR shape + proof-of-possession (client key self-signature).
    let valid = csr::validate(&req.csr).map_err(|e| {
        eprintln!("WARN: /v1/csr validation failed: {e}");
        ApiError::new(StatusCode::BAD_REQUEST, "invalid csr")
    })?;
    // 4. Replay: single-use permits.
    if state
        .used_permits()
        .find_one(doc! { "jti": &claims.jti })
        .await
        .map_err(|e| {
            eprintln!("ERROR: /v1/csr used_permits lookup failed: {e}");
            ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "store unavailable")
        })?
        .is_some()
    {
        eprintln!("WARN: /v1/csr replayed permit jti {}", claims.jti);
        return Err(ApiError::new(StatusCode::CONFLICT, "permit already used"));
    }
    // 5. Duplicate identity: one certificate per identity.
    if state
        .certs()
        .find_one(doc! { "identity": &valid.identity })
        .await
        .map_err(|e| {
            eprintln!("ERROR: /v1/csr certificates lookup failed: {e}");
            ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "store unavailable")
        })?
        .is_some()
    {
        return Err(ApiError::new(StatusCode::CONFLICT, "identity already enrolled"));
    }
    // 6. Issue: X.509 signed with the CA's own ML-DSA key.
    let issued = cert::issue(
        &state.ca_identity,
        &state.ca_dsa_public,
        &state.signer,
        &valid.identity,
        &valid.key_dsa,
        state.validity_days,
    )
    .map_err(|e| {
        eprintln!("ERROR: /v1/csr issuance failed: {e}");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "issuance failed")
    })?;
    // 7. Persist enrollment + burn the permit JTI.
    let now = DateTime::now();
    let cert_doc = doc! {
        "serial": issued.serial as i64,
        "identity": &valid.identity,
        "key_kem_b64": &req.csr.key_kem_b64,
        "key_dsa_b64": &req.csr.key_dsa_b64,
        "certificate_pem": &issued.pem,
        "permit_jti": &claims.jti,
        "issued_at": now,
    };
    state.certs().insert_one(cert_doc).await.map_err(|e| {
        eprintln!("ERROR: /v1/csr certificate store failed: {e}");
        ApiError::new(StatusCode::CONFLICT, "identity already enrolled")
    })?;
    let used = doc! { "jti": &claims.jti, "sub": &claims.sub, "used_at": now };
    if let Err(e) = state.used_permits().insert_one(used).await {
        eprintln!("WARN: /v1/csr used-permit store failed (cert already stored): {e}");
    }
    println!(
        "enrolled {:?} serial {} (permit jti {})",
        valid.identity, issued.serial, claims.jti
    );
    Ok((
        StatusCode::CREATED,
        Json(CsrResponse {
            identity: valid.identity,
            serial: issued.serial,
            certificate_pem: issued.pem,
        }),
    ))
}

#[derive(Serialize)]
struct CertRecord {
    serial: u64,
    identity: String,
    certificate_pem: String,
    issued_at: DateTime,
}

async fn get_cert(
    State(state): State<AppState>,
    Path(serial): Path<u64>,
) -> Result<Json<CertRecord>, ApiError> {
    let doc = state
        .certs()
        .find_one(doc! { "serial": serial as i64 })
        .await
        .map_err(|_| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "store unavailable"))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "certificate not found"))?;
    Ok(Json(CertRecord {
        serial,
        identity: doc
            .get_str("identity")
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "bad record"))?
            .to_string(),
        certificate_pem: doc
            .get_str("certificate_pem")
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "bad record"))?
            .to_string(),
        issued_at: *doc
            .get_datetime("issued_at")
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "bad record"))?,
    }))
}

#[derive(Serialize)]
struct VerifyResponse {
    serial: u64,
    identity: String,
    valid: bool,
}

async fn verify_cert(
    State(state): State<AppState>,
    Path(serial): Path<u64>,
) -> Result<Json<VerifyResponse>, ApiError> {
    let doc = state
        .certs()
        .find_one(doc! { "serial": serial as i64 })
        .await
        .map_err(|_| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "store unavailable"))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "certificate not found"))?;
    let identity = doc
        .get_str("identity")
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "bad record"))?
        .to_string();
    let pem = doc
        .get_str("certificate_pem")
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "bad record"))?;
    match cert::verify_pem(pem, &state.ca_identity, &state.ca_dsa_public, &identity) {
        Ok(s) if s == serial => Ok(Json(VerifyResponse {
            serial,
            identity,
            valid: true,
        })),
        Ok(_) => Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "serial mismatch",
        )),
        Err(e) => {
            eprintln!("WARN: /v1/certs/{serial}/verify failed: {e}");
            Ok(Json(VerifyResponse {
                serial,
                identity,
                valid: false,
            }))
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/db/ping", get(db_ping))
        .route("/v1/csr", post(post_csr))
        .route("/v1/permits", post(post_permits))
        .route("/v1/keys", get(get_keys))
        .route("/v1/keys/{identity}", get(get_key))
        .route("/v1/certs/{serial}", get(get_cert))
        .route("/v1/certs/{serial}/verify", get(verify_cert))
        .with_state(state)
}

pub async fn serve(listener: tokio::net::TcpListener, state: AppState) -> std::io::Result<()> {
    let addr: SocketAddr = listener.local_addr()?;
    println!("sagex-certauth listening on http://{addr}");
    axum::serve(listener, router(state)).await
}
