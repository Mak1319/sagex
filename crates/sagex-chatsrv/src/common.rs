//! Shared request helpers: id parsing, room lookup, membership gate.
//! Moved out of `routes.rs` so the media/engagement/room modules reuse them.

use mongodb::bson::{doc, oid::ObjectId, DateTime};

use crate::{
    error::{AppError, AppResult},
    models::{Message, Room},
    state::{AppState, C_MESSAGES, C_ROOMS},
};

pub(crate) fn ts(dt: &DateTime) -> String {
    dt.try_to_rfc3339_string().unwrap_or_default()
}

pub(crate) fn parse_id(hex: &str) -> AppResult<ObjectId> {
    ObjectId::parse_str(hex).map_err(|_| AppError::BadRequest("bad id".into()))
}

pub(crate) fn id_of(o: Option<ObjectId>, what: &str) -> AppResult<ObjectId> {
    o.ok_or_else(|| AppError::Internal(format!("{what} missing _id")))
}

pub(crate) async fn get_room(state: &AppState, room_hex: &str) -> AppResult<Room> {
    let rid = parse_id(room_hex)?;
    state
        .db
        .collection::<Room>(C_ROOMS)
        .find_one(doc! { "_id": rid }, None)
        .await?
        .ok_or_else(|| AppError::NotFound("room not found".into()))
}

pub(crate) fn must_be_member(room: &Room, user: &ObjectId) -> AppResult<()> {
    if room.member_ids.contains(user) {
        Ok(())
    } else {
        Err(AppError::Forbidden("not a room member".into()))
    }
}

/// Message kinds the server accepts. Anything else is rejected — clients
/// must not invent kinds the UI cannot render.
pub(crate) fn valid_kind(kind: &str) -> bool {
    matches!(
        kind,
        "text" | "image" | "video" | "voice" | "document" | "contact" | "poll" | "event"
            | "sticker"
    )
}

/// Validated post content shared by REST and WS paths (they must not drift).
/// Returns (body, kind, metadata, reply_to).
pub(crate) async fn resolve_post(
    state: &AppState,
    rid: ObjectId,
    body_raw: &str,
    kind_opt: Option<String>,
    metadata: Option<serde_json::Value>,
    reply_to_hex: Option<String>,
) -> AppResult<(String, String, Option<serde_json::Value>, Option<ObjectId>)> {
    let body = body_raw.trim().to_string();
    if body.is_empty() || body.len() > 4000 {
        return Err(AppError::BadRequest("body must be 1..4000 chars".into()));
    }
    let kind = kind_opt.unwrap_or_else(|| "text".to_string());
    if !valid_kind(&kind) {
        return Err(AppError::BadRequest("unknown message kind".into()));
    }
    if kind != "text" {
        let key = metadata
            .as_ref()
            .and_then(|m| m.get("object_key"))
            .and_then(|k| k.as_str())
            .filter(|k| !k.is_empty() && k.starts_with("media/") && !k.contains(".."));
        if matches!(kind.as_str(), "image" | "video" | "voice" | "document") {
            let key = key.ok_or_else(|| AppError::BadRequest("missing object_key".into()))?;
            if !state.storage.exists(key).await {
                return Err(AppError::BadRequest("media object not found".into()));
            }
        }
        if matches!(kind.as_str(), "poll" | "contact" | "event") && metadata.is_none() {
            return Err(AppError::BadRequest("missing metadata".into()));
        }
    }
    let reply_to = match reply_to_hex {
        Some(hex) => {
            let mid = parse_id(&hex)?;
            let target = state
                .db
                .collection::<Message>(C_MESSAGES)
                .find_one(doc! { "_id": mid }, None)
                .await?
                .ok_or_else(|| AppError::NotFound("reply target not found".into()))?;
            if target.room_id != rid {
                return Err(AppError::BadRequest("reply target is in another room".into()));
            }
            Some(mid)
        }
        None => None,
    };
    Ok((body, kind, metadata, reply_to))
}
