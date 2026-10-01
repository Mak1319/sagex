//! Media upload credentials + downloads (MinIO presigned URLs).
//!
//! The server never handles file bytes: `POST /api/v1/media/presign`
//! validates type/size and mints a time-boxed PUT URL, the client uploads
//! straight to MinIO, then references `object_key` in a message.
//! `GET /api/v1/media/*key` mints a time-boxed GET URL after a room
//! membership check — there are no public URLs.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{
    auth::AuthUser,
    common::{get_room, must_be_member},
    error::{AppError, AppResult},
    state::AppState,
};

/// Allowed content types: images, video, audio, common documents.
fn allowed_content_type(ct: &str) -> bool {
    const PREFIXES: &[&str] = &["image/", "video/", "audio/"];
    const EXACT: &[&str] = &[
        "application/pdf",
        "application/msword",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.ms-excel",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "application/vnd.ms-powerpoint",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "application/vnd.oasis.opendocument.text",
        "application/vnd.oasis.opendocument.spreadsheet",
        "application/vnd.oasis.opendocument.presentation",
        "text/plain",
        "text/csv",
        "application/zip",
    ];
    let ct = ct.split(';').next().unwrap_or("").trim().to_lowercase();
    PREFIXES.iter().any(|p| ct.starts_with(p)) || EXACT.contains(&ct.as_str())
}

fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let clean: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let clean = clean.trim_matches(['.', '_']).to_string();
    let short: String = clean.chars().take(100).collect();
    if short.is_empty() {
        "file".to_string()
    } else {
        short
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Deserialize)]
pub struct PresignBody {
    pub room_id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: u64,
}

#[derive(Serialize)]
struct PresignResponse {
    object_key: String,
    put_url: String,
    expires_at: i64,
}

/// Mint a time-boxed PUT URL. The object key embeds the room so downloads
/// can enforce membership without extra bookkeeping. The client MUST send
/// the exact `content_type` on the PUT (it is SigV4-signed into the URL).
pub async fn presign(
    user: AuthUser,
    State(state): State<AppState>,
    Json(b): Json<PresignBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &b.room_id).await?;
    must_be_member(&room, &user.user_id)?;
    let ct = b.content_type.split(';').next().unwrap_or("").trim().to_string();
    if !allowed_content_type(&ct) {
        return Err(AppError::BadRequest("unsupported content type".into()));
    }
    let max = state.config.media_max_bytes.max(1);
    if b.size_bytes == 0 || b.size_bytes > max {
        return Err(AppError::BadRequest(format!(
            "size_bytes must be 1..{max}"
        )));
    }
    let key = format!(
        "media/{}/{}/{}",
        b.room_id,
        uuid::Uuid::new_v4(),
        sanitize_filename(&b.filename)
    );
    let ttl = Duration::from_secs(state.config.presign_ttl_secs.max(60));
    let url = state.storage.presign_put(&key, &ct, ttl).await.map_err(|e| {
        AppError::Internal(format!("presign failed: {e}"))
    })?;
    Ok((
        StatusCode::OK,
        Json(PresignResponse {
            object_key: key,
            put_url: url,
            expires_at: now_unix() + ttl.as_secs() as i64,
        }),
    ))
}

/// Membership-checked download URL. The key must live under `media/` and
/// name its room as the first segment (`media/{room_hex}/...`).
pub async fn download(
    user: AuthUser,
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> AppResult<impl IntoResponse> {
    let mut parts = key.split('/');
    match (parts.next(), parts.next()) {
        (Some("media"), Some(room_hex)) if !room_hex.is_empty() => {
            let room = get_room(&state, room_hex).await?;
            must_be_member(&room, &user.user_id)?;
        }
        _ => return Err(AppError::NotFound("object not found".into())),
    }
    if key.contains("..") {
        return Err(AppError::BadRequest("bad object key".into()));
    }
    if !state.storage.exists(&key).await {
        return Err(AppError::NotFound("object not found".into()));
    }
    let ttl = Duration::from_secs(state.config.presign_ttl_secs.max(60));
    let url = state.storage.presign_get(&key, ttl).await.map_err(|e| {
        AppError::Internal(format!("presign failed: {e}"))
    })?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "object_key": key,
            "get_url": url,
            "expires_at": now_unix() + ttl.as_secs() as i64,
        })),
    ))
}

pub fn router() -> axum::Router<crate::state::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/api/v1/media/presign", post(presign))
        .route("/api/v1/media/*key", get(download))
}
