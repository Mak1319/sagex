//! Short-lived RustFS URLs for client-encrypted blobs. The server
//! mints capability URLs; object keys are unguessable (`u/<user>/<uuid>/…`).

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    auth::{ApiResult, api_err},
    s3::{S3, presign},
    state::{AppState, AuthUser},
};

fn s3_of(st: &AppState) -> ApiResult<S3> {
    S3::from_config(
        &st.cfg.rustfs.endpoint,
        &st.cfg.rustfs.bucket,
        &st.cfg.rustfs.region,
        &st.cfg.rustfs.access_key,
        &st.cfg.rustfs.secret_key,
    )
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

#[derive(Deserialize)]
pub struct UploadUrlBody {
    pub filename: String,
    pub content_type: Option<String>,
}

#[derive(Serialize)]
struct UploadUrlOut {
    url: String,
    key: String,
    expires_in: u64,
}

pub async fn upload_url(
    State(st): State<AppState>,
    auth: AuthUser,
    Json(body): Json<UploadUrlBody>,
) -> ApiResult<impl IntoResponse> {
    let filename = body.filename.trim().replace(['/', '\\'], "_");
    if filename.is_empty() || filename.len() > 256 {
        return Err(api_err(StatusCode::BAD_REQUEST, "bad filename"));
    }
    let s3 = s3_of(&st)?;
    let key = format!("u/{}/{}/{}", auth.object_key, Uuid::now_v7(), filename);
    let ttl = st.cfg.rustfs.put_url_ttl_secs;
    let p = presign(&s3, "PUT", &key, ttl);
    let _ = body.content_type;
    Ok(Json(json!(UploadUrlOut {
        url: p.url,
        key,
        expires_in: p.expires_in,
    })))
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub key: String,
}

pub async fn download_url(
    State(st): State<AppState>,
    _auth: AuthUser,
    Query(q): Query<DownloadQuery>,
) -> ApiResult<impl IntoResponse> {
    if q.key.is_empty() || q.key.len() > 512 || q.key.contains("..") {
        return Err(api_err(StatusCode::BAD_REQUEST, "bad key"));
    }
    let s3 = s3_of(&st)?;
    let ttl = st.cfg.rustfs.get_url_ttl_secs;
    let p = presign(&s3, "GET", &q.key, ttl);
    Ok(Json(json!({ "url": p.url, "expires_in": p.expires_in })))
}
