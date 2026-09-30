use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::{auth, ca, config::UserRecord, db};

#[derive(Debug, Deserialize)]
pub struct IssueRequest {
    pub jwt: String,
    pub csr: sagex_format::format::PublicFileFormatExternal,
    pub pop_signature: String,
}

#[derive(Debug, Serialize)]
pub struct IssueResponse {
    pub certificate_pem: String,
}

fn err(status: StatusCode, msg: impl std::fmt::Display) -> (StatusCode, String) {
    (status, msg.to_string())
}

pub async fn issue(
    State(state): State<crate::config::AppState>,
    Json(req): Json<IssueRequest>,
) -> Result<Json<IssueResponse>, (StatusCode, String)> {
    // 1. chatsrv JWT (anti-junk hint). Key pinned via .pub file or hex.
    let chatsrv_vk = crate::config::chatsrv_verify_key(&state.settings)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let claims = auth::verify_jwt(
        &req.jwt,
        &chatsrv_vk,
        &state.settings.chatsrv_iss,
        &state.settings.audience,
    )
    .map_err(|e| err(StatusCode::UNAUTHORIZED, format!("jwt: {e}")))?;

    // 2. user PoP over canonical CSR.
    let pop_sig =
        auth::decode_sig(&req.pop_signature).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    auth::verify_pop(&req.csr, &pop_sig).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;

    // 3. username binding: jwt.username authoritative; sub logged, must match if non-empty and different?
    let csr_name = req.csr.internal.user_name.clone();
    if csr_name != claims.username {
        return Err(err(
            StatusCode::FORBIDDEN,
            "csr user_name != jwt username",
        ));
    }

    // 4. load CA signer, issue cert.
    let ca = ca::CaSigner::load(
        &state.secrets.ca_privkey_path,
        &state.secrets.ca_privkey_password,
    )
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let der = ca::issue_cert_der(
        &ca,
        &state.settings.issuer_cn,
        &csr_name,
        &req.csr.internal.key_kem,
        &req.csr.internal.key_dsa,
        state.settings.cert_ttl_days,
    )
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let pem = ca::der_to_pem(&der);

    // 5. persist minimal record.
    let coll = state
        .mongo
        .as_ref()
        .ok_or_else(|| err(StatusCode::SERVICE_UNAVAILABLE, "no mongodb"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    db::upsert_user(
        coll,
        UserRecord {
            user_name: csr_name,
            key_kem: mongodb::bson::Binary {
                subtype: mongodb::bson::spec::BinarySubtype::Generic,
                bytes: req.csr.internal.key_kem.clone(),
            },
            key_dsa: mongodb::bson::Binary {
                subtype: mongodb::bson::spec::BinarySubtype::Generic,
                bytes: req.csr.internal.key_dsa.clone(),
            },
            cert_pem: pem.clone(),
            issued_at: now,
        },
    )
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(IssueResponse {
        certificate_pem: pem,
    }))
}
