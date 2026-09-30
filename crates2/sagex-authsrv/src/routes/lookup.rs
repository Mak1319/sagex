use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::Serialize;

use crate::{ca, db};

#[derive(Debug, Serialize)]
pub struct LookupResponse {
    pub user_name: String,
    pub key_kem: Vec<u8>,
    pub key_dsa: Vec<u8>,
    pub cert_pem: String,
}

fn err(status: StatusCode, msg: impl std::fmt::Display) -> (StatusCode, String) {
    (status, msg.to_string())
}

fn presenter_pem(headers: &HeaderMap) -> Result<String, (StatusCode, String)> {
    for key in ["x-client-cert-pem", "x-client-cert"] {
        if let Some(v) = headers.get(key) {
            return v
                .to_str()
                .map(|s| s.to_string())
                .map_err(|_| err(StatusCode::BAD_REQUEST, "bad cert header encoding"));
        }
    }
    Err(err(StatusCode::UNAUTHORIZED, "missing client certificate"))
}

pub async fn lookup(
    State(state): State<crate::config::AppState>,
    Path(user_name): Path<String>,
    headers: HeaderMap,
) -> Result<Json<LookupResponse>, (StatusCode, String)> {
    // 1. presenter cert required.
    let pem_raw = presenter_pem(&headers)?;
    // Headers can't carry newlines raw; clients send `\n`-escaped PEM.
    // NOTE: unescape first, THEN look for markers — a blanket
    // space->newline pass would corrupt "BEGIN CERTIFICATE" itself.
    // Only fall back to space->newline when no markers are found
    // (some proxies turn newlines into spaces).
    let pem_text = pem_raw.replace("\\n", "\n");
    let mut repaired = repair_pem(&pem_text);
    if !repaired.1 {
        repaired = repair_pem(&pem_text.replace(' ', "\n"));
    }
    let der = ca::pem_to_der(&repaired.0).map_err(|e| err(StatusCode::UNAUTHORIZED, e))?;

    // 2. verify chains to our CA.
    let ca = ca::CaSigner::load(
        &state.secrets.ca_privkey_path,
        &state.secrets.ca_privkey_password,
    )
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    ca::verify_presenter_cert(&der, &ca.vk_bytes)
        .map_err(|e| err(StatusCode::UNAUTHORIZED, e))?;

    // 3. return stored keys.
    let coll = state
        .mongo
        .as_ref()
        .ok_or_else(|| err(StatusCode::SERVICE_UNAVAILABLE, "no mongodb"))?;
    let rec = db::find_user(coll, &user_name)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "unknown user"))?;
    Ok(Json(LookupResponse {
        user_name: rec.user_name,
        key_kem: rec.key_kem.bytes,
        key_dsa: rec.key_dsa.bytes,
        cert_pem: rec.cert_pem,
    }))
}

/// Tolerate header-mangled PEM: rebuild proper 64-col body.
fn repair_pem(text: &str) -> (String, bool) {
    if text.contains("-----BEGIN CERTIFICATE-----") && text.contains("-----END CERTIFICATE-----") {
        // normalize body lines
        let body: String = text
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect::<Vec<_>>()
            .join("");
        let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
        for chunk in body.as_bytes().chunks(64) {
            out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
            out.push('\n');
        }
        out.push_str("-----END CERTIFICATE-----\n");
        return (out, true);
    }
    (text.to_string(), false)
}
