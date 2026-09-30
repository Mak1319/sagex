//! ML-DSA-signed service tokens. The service signs a JSON payload with
//! its own keypair (from config `.prv`); verifiers (e.g. the future CA)
//! check it against the service `.pub`.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    auth::{ApiResult, api_err},
    state::{AppState, AuthUser},
};

#[derive(Serialize, Deserialize)]
struct ServicePayload {
    iss: String,
    sub: String,
    username: String,
    aud: String,
    iat: i64,
    exp: i64,
}

#[derive(Serialize)]
struct TokenOut {
    token_type: String,
    /// Compact ML-DSA JWT: b64u(header).b64u(payload).b64u(sig).
    /// This is the contract with sagex-authsrv (see docs/ca-chatsrv-contract.md).
    token: String,
}

fn mint_service_token(st: &AppState, auth: &AuthUser, aud: &str) -> ApiResult<TokenOut> {
    let now = Utc::now().timestamp();
    let payload = ServicePayload {
        iss: "sagex-chatsrv".to_string(),
        sub: auth.object_key.clone(),
        username: auth.username.clone(),
        aud: aud.to_string(),
        iat: now,
        exp: now + st.cfg.ca.token_ttl_mins * 60,
    };
    let payload_str =
        serde_json::to_string(&payload).map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Compact JWT: the ML-DSA signature covers the ASCII "header.payload" input,
    // exactly as sagex-authsrv verifies it.
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"ML-DSA-65","typ":"JWT"}"#);
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload_str.as_bytes());
    let signing_input = format!("{header}.{payload_b64}");
    let sig = sagex_crypto::pqc::dsa::Implement::sign_from_password(
        &st.svc.enc_dsa,
        &st.svc.password,
        signing_input.as_bytes(),
    )
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("sign failed: {e:?}")))?;
    Ok(TokenOut {
        token_type: "mldsa65".to_string(),
        token: format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(&sig)),
    })
}

#[derive(Deserialize)]
pub struct ServiceTokenBody {
    pub aud: Option<String>,
}

/// Token for an arbitrary other service (`aud` names it).
pub async fn service_token(
    State(st): State<AppState>,
    auth: AuthUser,
    Json(body): Json<ServiceTokenBody>,
) -> ApiResult<impl IntoResponse> {
    let aud = body.aud.unwrap_or_else(|| st.cfg.ca.audience.clone());
    Ok(Json(json!(mint_service_token(&st, &auth, &aud)?)))
}

/// Token scoped for the Certificate Authority.
pub async fn ca_token(
    State(st): State<AppState>,
    auth: AuthUser,
) -> ApiResult<impl IntoResponse> {
    let aud = st.cfg.ca.audience.clone();
    Ok(Json(json!(mint_service_token(&st, &auth, &aud)?)))
}

/// Service DSA verifying key (base64 STANDARD) so verifiers can pin it.
pub async fn service_pubkey(State(st): State<AppState>) -> ApiResult<impl IntoResponse> {
    Ok(Json(json!({ "key_dsa": STANDARD.encode(&st.svc.vk_pub) })))
}
