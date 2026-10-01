//! Room/user management beyond the basics: message search, per-user room
//! settings, member removal, room deletion, export, user lookup by id,
//! DM lookup-or-create, reports, and blocks.
//!
//! Blocks are enforced at DM creation (either direction); reports are an
//! operator queue with no automatic action.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use futures::StreamExt;
use mongodb::{
    bson::{doc, DateTime},
    options::FindOptions,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthUser,
    common::{get_room, id_of, must_be_member, parse_id, ts},
    error::{AppError, AppResult},
    models::{Block, Message, Report, Room, RoomSettings, User},
    state::{
        AppState, C_BLOCKS, C_MESSAGES, C_ROOMS, C_ROOM_SETTINGS, C_REPORTS, C_USERS,
    },
};

// ---------- message search ----------
#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<i64>,
}

async fn search_messages(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<SearchQuery>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    must_be_member(&room, &user.user_id)?;
    let term = q.q.trim().to_string();
    if term.is_empty() || term.len() > 200 {
        return Err(AppError::BadRequest("q must be 1..200 chars".into()));
    }
    let rid = id_of(room.id, "room")?;
    let rx = doc! { "$regex": crate::routes::regex_escape(&term), "$options": "i" };
    let opts = FindOptions::builder()
        .sort(doc! { "_id": -1 })
        .limit(q.limit.unwrap_or(30).clamp(1, 100))
        .build();
    let mut cur = state
        .db
        .collection::<Message>(C_MESSAGES)
        .find(doc! { "room_id": rid, "body": rx }, opts)
        .await?;
    let mut msgs = Vec::new();
    while let Some(m) = cur.next().await {
        msgs.push(m?);
    }
    let out = crate::engage::enrich(&state, &user.user_id, msgs).await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "messages": out }))))
}

// ---------- per-user room settings ----------
#[derive(Debug, Serialize, Deserialize)]
struct SettingsBody {
    muted_until: Option<String>, // RFC3339, null clears
    archived: Option<bool>,
    fav: Option<bool>,
    pinned: Option<bool>,
    disappearing_secs: Option<u64>,
}

#[derive(Serialize)]
struct SettingsDto {
    muted_until: Option<String>,
    archived: bool,
    fav: bool,
    pinned: bool,
    disappearing_secs: Option<u64>,
    updated_at: Option<String>,
}

impl SettingsDto {
    fn of(s: Option<RoomSettings>) -> Self {
        match s {
            Some(v) => Self {
                muted_until: v.muted_until.as_ref().map(ts),
                archived: v.archived,
                fav: v.fav,
                pinned: v.pinned,
                disappearing_secs: v.disappearing_secs,
                updated_at: Some(ts(&v.updated_at)),
            },
            None => Self {
                muted_until: None,
                archived: false,
                fav: false,
                pinned: false,
                disappearing_secs: None,
                updated_at: None,
            },
        }
    }
}

async fn get_settings(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    must_be_member(&room, &user.user_id)?;
    let s = state
        .db
        .collection::<RoomSettings>(C_ROOM_SETTINGS)
        .find_one(
            doc! { "room_id": id_of(room.id, "room")?, "user_id": user.user_id },
            None,
        )
        .await?;
    Ok((StatusCode::OK, Json(SettingsDto::of(s))))
}

async fn put_settings(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<SettingsBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    must_be_member(&room, &user.user_id)?;
    let rid = id_of(room.id, "room")?;
    let mut set = doc! { "updated_at": DateTime::now() };
    if let Some(mu) = b.muted_until {
        if mu.trim().is_empty() {
            set.insert("muted_until", mongodb::bson::Bson::Null);
        } else {
            let dt = chrono::DateTime::parse_from_rfc3339(mu.trim())
                .map_err(|_| AppError::BadRequest("muted_until must be RFC3339".into()))?;
            set.insert(
                "muted_until",
                DateTime::from_millis(dt.timestamp_millis()),
            );
        }
    }
    for (k, v) in [
        ("archived", b.archived),
        ("fav", b.fav),
        ("pinned", b.pinned),
    ] {
        if let Some(v) = v {
            set.insert(k, v);
        }
    }
    if let Some(d) = b.disappearing_secs {
        if d > 0 && d <= 31_536_000 {
            set.insert("disappearing_secs", d as i64);
        } else {
            return Err(AppError::BadRequest(
                "disappearing_secs must be 1..31536000".into(),
            ));
        }
    }
    state
        .db
        .collection::<RoomSettings>(C_ROOM_SETTINGS)
        .update_one(
            doc! { "room_id": rid, "user_id": user.user_id },
            doc! { "$set": set },
            mongodb::options::UpdateOptions::builder()
                .upsert(true)
                .build(),
        )
        .await?;
    get_settings(user, State(state), Path(id)).await
}

// ---------- member removal ----------
#[derive(Deserialize)]
struct RemoveBody {
    user_id: String,
}

async fn remove_member(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<RemoveBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    must_be_member(&room, &user.user_id)?;
    let target = parse_id(&b.user_id)?;
    if target == user.user_id {
        return Err(AppError::BadRequest("use leave to remove yourself".into()));
    }
    if !room.member_ids.contains(&target) {
        return Err(AppError::NotFound("not a member".into()));
    }
    state
        .db
        .collection::<Room>(C_ROOMS)
        .update_one(
            doc! { "_id": id_of(room.id, "room")? },
            doc! { "$pull": { "member_ids": target } },
            None,
        )
        .await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

// ---------- room deletion + export ----------
async fn delete_room(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    must_be_member(&room, &user.user_id)?;
    let rid = id_of(room.id, "room")?;
    if room.is_dm {
        // Either DM party may delete the shared room.
    } else if room.created_by != user.user_id {
        return Err(AppError::Forbidden("only the group creator can delete".into()));
    }
    state
        .db
        .collection::<Room>(C_ROOMS)
        .delete_one(doc! { "_id": rid }, None)
        .await?;
    // History is audit data: messages stay, the room (membership) is gone.
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

async fn export_room(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    must_be_member(&room, &user.user_id)?;
    let rid = id_of(room.id, "room")?;
    let mut cur = state
        .db
        .collection::<Message>(C_MESSAGES)
        .find(
            doc! { "room_id": rid },
            FindOptions::builder()
                .sort(doc! { "_id": 1 })
                .limit(5000)
                .build(),
        )
        .await?;
    let mut msgs = Vec::new();
    while let Some(m) = cur.next().await {
        msgs.push(m?);
    }
    let out = crate::engage::enrich(&state, &user.user_id, msgs).await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "room_id": rid.to_hex(),
            "exported_at": ts(&DateTime::now()),
            "message_count": out.len(),
            "messages": out,
        })),
    ))
}

// ---------- user lookup by id ----------
async fn get_user(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let uid = parse_id(&id)?;
    let u = state
        .db
        .collection::<User>(C_USERS)
        .find_one(doc! { "_id": uid }, None)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "id": id_of(u.id, "user")?.to_hex(),
            "username": u.username,
            "display_name": u.display_name,
            "is_verified": u.is_verified,
            "created_at": ts(&u.created_at),
        })),
    ))
}

// ---------- DM lookup-or-create ----------
#[derive(Deserialize)]
struct DmLookupBody {
    user_id: String,
}

async fn dm_lookup(
    user: AuthUser,
    State(state): State<AppState>,
    Json(b): Json<DmLookupBody>,
) -> AppResult<impl IntoResponse> {
    let target = parse_id(&b.user_id)?;
    if target == user.user_id {
        return Err(AppError::BadRequest("cannot DM yourself".into()));
    }
    let users = state.db.collection::<User>(C_USERS);
    users
        .find_one(doc! { "_id": target }, None)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;
    // Blocked in either direction => no new DM.
    let blocks = state.db.collection::<Block>(C_BLOCKS);
    if blocks
        .find_one(
            doc! { "$or": [
                { "blocker_id": user.user_id, "blocked_id": target },
                { "blocker_id": target, "blocked_id": user.user_id },
            ] },
            None,
        )
        .await?
        .is_some()
    {
        return Err(AppError::Forbidden("cannot DM this user".into()));
    }
    let rooms = state.db.collection::<Room>(C_ROOMS);
    if let Some(r) = rooms
        .find_one(
            doc! { "is_dm": true, "member_ids": { "$all": [user.user_id, target] } },
            None,
        )
        .await?
    {
        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "room_id": id_of(r.id, "room")?.to_hex(),
                "created": false,
            })),
        ));
    }
    let res = rooms
        .insert_one(
            Room {
                id: None,
                name: None,
                is_dm: true,
                member_ids: vec![user.user_id, target],
                created_by: user.user_id,
                created_at: DateTime::now(),
            },
            None,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "room_id": res.inserted_id.as_object_id().unwrap().to_hex(),
            "created": true,
        })),
    ))
}

// ---------- reports + blocks ----------
#[derive(Deserialize)]
struct ReportBody {
    reason: Option<String>,
}

async fn report_user(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<ReportBody>,
) -> AppResult<impl IntoResponse> {
    let target = parse_id(&id)?;
    if target == user.user_id {
        return Err(AppError::BadRequest("cannot report yourself".into()));
    }
    state
        .db
        .collection::<User>(C_USERS)
        .find_one(doc! { "_id": target }, None)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;
    if let Some(r) = b.reason.clone() {
        let r = r.trim().to_string();
        if r.len() > 500 {
            return Err(AppError::BadRequest("reason must be <= 500 chars".into()));
        }
    }
    let r = state
        .db
        .collection::<Report>(C_REPORTS)
        .insert_one(
            Report {
                id: None,
                reporter_id: user.user_id,
                reported_id: target,
                reason: b.reason.map(|s| s.trim().to_string()),
                created_at: DateTime::now(),
            },
            None,
        )
        .await;
    match r {
        Ok(_) => Ok((StatusCode::CREATED, Json(serde_json::json!({ "ok": true })))),
        Err(e) if e.to_string().contains("E11000") => {
            Err(AppError::Conflict("already reported".into()))
        }
        Err(e) => Err(AppError::Internal(format!("db: {e}"))),
    }
}

async fn block_user(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let target = parse_id(&id)?;
    if target == user.user_id {
        return Err(AppError::BadRequest("cannot block yourself".into()));
    }
    state
        .db
        .collection::<User>(C_USERS)
        .find_one(doc! { "_id": target }, None)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;
    let r = state
        .db
        .collection::<Block>(C_BLOCKS)
        .insert_one(
            Block {
                id: None,
                blocker_id: user.user_id,
                blocked_id: target,
                created_at: DateTime::now(),
            },
            None,
        )
        .await;
    match r {
        Ok(_) => Ok((StatusCode::CREATED, Json(serde_json::json!({ "ok": true })))),
        Err(e) if e.to_string().contains("E11000") => {
            Err(AppError::Conflict("already blocked".into()))
        }
        Err(e) => Err(AppError::Internal(format!("db: {e}"))),
    }
}

async fn unblock_user(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let target = parse_id(&id)?;
    let r = state
        .db
        .collection::<Block>(C_BLOCKS)
        .delete_one(
            doc! { "blocker_id": user.user_id, "blocked_id": target },
            None,
        )
        .await?;
    if r.deleted_count == 0 {
        return Err(AppError::NotFound("not blocked".into()));
    }
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/users/:id", get(get_user))
        .route("/api/v1/dms/lookup", post(dm_lookup))
        .route(
            "/api/v1/rooms/:id/messages/search",
            get(search_messages),
        )
        .route(
            "/api/v1/users/me/rooms/:id/settings",
            get(get_settings).put(put_settings),
        )
        .route("/api/v1/rooms/:id/remove", post(remove_member))
        .route("/api/v1/rooms/:id", delete(delete_room))
        .route("/api/v1/rooms/:id/export", get(export_room))
        .route("/api/v1/users/:id/report", post(report_user))
        .route(
            "/api/v1/users/:id/blocks",
            post(block_user).delete(unblock_user),
        )
}
