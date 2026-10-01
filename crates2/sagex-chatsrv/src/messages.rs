//! Message routes. Bodies are opaque client-encrypted blobs — the
//! server stores and returns them verbatim, never decrypting.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    auth::{ApiResult, api_err},
    models::Message,
    state::{AppState, AuthUser},
};

#[derive(Deserialize)]
pub struct PostMessageBody {
    /// Opaque E2EE ciphertext (e.g. base64). Stored verbatim.
    pub body: String,
    pub content_type: Option<String>,
    /// Optional RustFS object key for a bigger encrypted attachment.
    pub rustfs_ref: Option<String>,
}

#[derive(Serialize)]
struct MessageOut {
    object_key: String,
    sender_key: String,
    body: String,
    content_type: Option<String>,
    rustfs_ref: Option<String>,
    created_ms: i64,
}

async fn member_of(st: &AppState, group_id: &str, user: &str) -> ApiResult<()> {
    let g = st
        .db
        .groups
        .find_one(doc! { "_id": group_id })
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match g {
        Some(g) if g.members.contains(&user.to_string()) => Ok(()),
        Some(_) => Err(api_err(StatusCode::FORBIDDEN, "not a member")),
        None => Err(api_err(StatusCode::NOT_FOUND, "group not found")),
    }
}

pub async fn post_message(
    State(st): State<AppState>,
    auth: AuthUser,
    Path(group_id): Path<String>,
    Json(body): Json<PostMessageBody>,
) -> ApiResult<impl IntoResponse> {
    member_of(&st, &group_id, &auth.object_key).await?;
    if body.body.is_empty() || body.body.len() > 1_000_000 {
        return Err(api_err(StatusCode::BAD_REQUEST, "bad body"));
    }
    let msg = Message {
        object_key: Uuid::now_v7().to_string(),
        group_id,
        sender_key: auth.object_key,
        body: body.body,
        content_type: body.content_type,
        rustfs_ref: body.rustfs_ref,
        created_ms: Utc::now().timestamp_millis(),
    };
    st.db
        .messages
        .insert_one(msg.clone())
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((
        StatusCode::CREATED,
        Json(json!(MessageOut {
            object_key: msg.object_key,
            sender_key: msg.sender_key,
            body: msg.body,
            content_type: msg.content_type,
            rustfs_ref: msg.rustfs_ref,
            created_ms: msg.created_ms,
        })),
    ))
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub before: Option<i64>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

pub async fn list_messages(
    State(st): State<AppState>,
    auth: AuthUser,
    Path(group_id): Path<String>,
    Query(q): Query<ListQuery>,
) -> ApiResult<impl IntoResponse> {
    member_of(&st, &group_id, &auth.object_key).await?;
    let limit = q.limit.clamp(1, 200);
    let mut filter = doc! { "group_id": &group_id };
    if let Some(before) = q.before {
        filter.insert("created_ms", doc! { "$lt": before });
    }
    let mut cursor = st
        .db
        .messages
        .find(filter)
        .sort(doc! { "created_ms": -1 })
        .limit(limit)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut out = Vec::new();
    while cursor
        .advance()
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        let m: Message = cursor
            .deserialize_current()
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        out.push(MessageOut {
            object_key: m.object_key,
            sender_key: m.sender_key,
            body: m.body,
            content_type: m.content_type,
            rustfs_ref: m.rustfs_ref,
            created_ms: m.created_ms,
        });
    }
    out.reverse();
    Ok(Json(json!(out)))
}
