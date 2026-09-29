//! Engagement: poll votes, reactions, pins, stars, read watermarks,
//! message edit/delete. All member-gated; state changes fan out over the
//! existing WS hub as pre-serialized JSON (no ws.rs protocol change needed
//! for emission — the envelope is free-form text).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Json, Router,
};
use futures::StreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId, DateTime, Document},
    options::UpdateOptions,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthUser,
    common::{id_of, must_be_member, parse_id, ts, get_room},
    error::{AppError, AppResult},
    models::{Message, Pin, PollVote, Reaction, ReadMarker, Star},
    state::{
        AppState, C_MESSAGES, C_PINS, C_POLL_VOTES, C_REACTIONS, C_READ_MARKERS, C_STARS,
    },
};

fn bson_err(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("db decode: {e}"))
}

fn dup_err(e: mongodb::error::Error) -> AppError {
    if e.to_string().contains("E11000") {
        AppError::Conflict("already exists".into())
    } else {
        AppError::Internal(format!("db: {e}"))
    }
}

async fn get_message(state: &AppState, mid_hex: &str) -> AppResult<Message> {
    let mid = parse_id(mid_hex)?;
    state
        .db
        .collection::<Message>(C_MESSAGES)
        .find_one(doc! { "_id": mid }, None)
        .await?
        .ok_or_else(|| AppError::NotFound("message not found".into()))
}

/// Load a message and assert it belongs to `room` (prevents cross-room id confusion).
async fn get_room_message(
    state: &AppState,
    room_hex: &str,
    mid_hex: &str,
) -> AppResult<(Message, ObjectId)> {
    let room = get_room(state, room_hex).await?;
    let rid = id_of(room.id, "room")?;
    let msg = get_message(state, mid_hex).await?;
    if msg.room_id != rid {
        return Err(AppError::NotFound("message not found".into()));
    }
    Ok((msg, rid))
}

fn fan_out(state: &AppState, room_hex: &str, payload: serde_json::Value) {
    state.hub.broadcast_text(room_hex, payload.to_string());
}

// ---------- poll votes ----------
#[derive(Deserialize)]
struct VoteBody {
    option_idx: u32,
}

fn poll_option_count(msg: &Message) -> AppResult<usize> {
    if msg.kind != "poll" {
        return Err(AppError::BadRequest("not a poll message".into()));
    }
    let n = msg
        .metadata
        .as_ref()
        .and_then(|m| m.get("options"))
        .and_then(|o| o.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if n == 0 {
        return Err(AppError::BadRequest("poll has no options".into()));
    }
    Ok(n)
}

async fn poll_tallies(state: &AppState, mid: ObjectId, n_options: usize) -> AppResult<Vec<u32>> {
    use mongodb::bson::Bson;
    let mut tallies = vec![0u32; n_options];
    let mut cur = state
        .db
        .collection::<PollVote>(C_POLL_VOTES)
        .aggregate(
            vec![
                doc! { "$match": { "message_id": mid } },
                doc! { "$group": { "_id": "$option_idx", "n": { "$sum": 1 } } },
            ],
            None,
        )
        .await?;
    while let Some(d) = cur.next().await {
        let d: Document = d?;
        let idx = match d.get("_id") {
            Some(Bson::Int32(i)) => *i as usize,
            Some(Bson::Int64(i)) => *i as usize,
            _ => continue,
        };
        let n = d.get_i32("n").unwrap_or(0).max(0) as u32;
        if idx < tallies.len() {
            tallies[idx] = n;
        }
    }
    Ok(tallies)
}

async fn vote(
    user: AuthUser,
    State(state): State<AppState>,
    Path((room_hex, mid_hex)): Path<(String, String)>,
    Json(b): Json<VoteBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let (msg, rid) = get_room_message(&state, &room_hex, &mid_hex).await?;
    if msg.deleted {
        return Err(AppError::BadRequest("message deleted".into()));
    }
    let n = poll_option_count(&msg)?;
    if (b.option_idx as usize) >= n {
        return Err(AppError::BadRequest("option out of range".into()));
    }
    let mid = id_of(msg.id, "message")?;
    let coll = state.db.collection::<PollVote>(C_POLL_VOTES);
    // One vote per user: replace any previous choice.
    coll.delete_many(doc! { "message_id": mid, "user_id": user.user_id }, None)
        .await?;
    coll.insert_one(
        PollVote {
            id: None,
            message_id: mid,
            user_id: user.user_id,
            option_idx: b.option_idx,
            created_at: DateTime::now(),
        },
        None,
    )
    .await
    .map_err(dup_err)?;
    let tallies = poll_tallies(&state, mid, n).await?;
    fan_out(
        &state,
        &rid.to_hex(),
        serde_json::json!({
            "type": "poll.result",
            "room_id": rid.to_hex(),
            "message_id": mid.to_hex(),
            "sender_id": user.user_id.to_hex(),
            "data": { "tallies": tallies },
        }),
    );
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true, "tallies": tallies })),
    ))
}

async fn unvote(
    user: AuthUser,
    State(state): State<AppState>,
    Path((room_hex, mid_hex)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let (msg, rid) = get_room_message(&state, &room_hex, &mid_hex).await?;
    let mid = id_of(msg.id, "message")?;
    let r = state
        .db
        .collection::<PollVote>(C_POLL_VOTES)
        .delete_one(doc! { "message_id": mid, "user_id": user.user_id }, None)
        .await?;
    if r.deleted_count == 0 {
        return Err(AppError::NotFound("no vote to retract".into()));
    }
    fan_out(
        &state,
        &rid.to_hex(),
        serde_json::json!({
            "type": "poll.result",
            "room_id": rid.to_hex(),
            "message_id": mid.to_hex(),
            "sender_id": user.user_id.to_hex(),
            "data": { "retracted_by": user.user_id.to_hex() },
        }),
    );
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

// ---------- reactions ----------
#[derive(Deserialize)]
struct ReactBody {
    emoji: String,
}

fn valid_emoji(e: &str) -> bool {
    let t = e.trim();
    !t.is_empty() && t.len() <= 12
}

async fn react(
    user: AuthUser,
    State(state): State<AppState>,
    Path((room_hex, mid_hex)): Path<(String, String)>,
    Json(b): Json<ReactBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let (msg, rid) = get_room_message(&state, &room_hex, &mid_hex).await?;
    if msg.deleted {
        return Err(AppError::BadRequest("message deleted".into()));
    }
    let emoji = b.emoji.trim().to_string();
    if !valid_emoji(&emoji) {
        return Err(AppError::BadRequest("bad emoji".into()));
    }
    let mid = id_of(msg.id, "message")?;
    state
        .db
        .collection::<Reaction>(C_REACTIONS)
        .insert_one(
            Reaction {
                id: None,
                message_id: mid,
                user_id: user.user_id,
                emoji: emoji.clone(),
                created_at: DateTime::now(),
            },
            None,
        )
        .await
        .map_err(|e| {
            if e.to_string().contains("E11000") {
                AppError::Conflict("already reacted".into())
            } else {
                AppError::Internal(format!("db: {e}"))
            }
        })?;
    fan_out(
        &state,
        &rid.to_hex(),
        serde_json::json!({
            "type": "reaction",
            "room_id": rid.to_hex(),
            "message_id": mid.to_hex(),
            "sender_id": user.user_id.to_hex(),
            "data": { "emoji": emoji, "action": "added" },
        }),
    );
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "ok": true }))))
}

async fn unreact(
    user: AuthUser,
    State(state): State<AppState>,
    Path((room_hex, mid_hex)): Path<(String, String)>,
    Query(q): Query<ReactQuery>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let (msg, rid) = get_room_message(&state, &room_hex, &mid_hex).await?;
    let mid = id_of(msg.id, "message")?;
    let emoji = q.emoji.trim().to_string();
    let r = state
        .db
        .collection::<Reaction>(C_REACTIONS)
        .delete_one(
            doc! { "message_id": mid, "user_id": user.user_id, "emoji": &emoji },
            None,
        )
        .await?;
    if r.deleted_count == 0 {
        return Err(AppError::NotFound("no such reaction".into()));
    }
    fan_out(
        &state,
        &rid.to_hex(),
        serde_json::json!({
            "type": "reaction",
            "room_id": rid.to_hex(),
            "message_id": mid.to_hex(),
            "sender_id": user.user_id.to_hex(),
            "data": { "emoji": emoji, "action": "removed" },
        }),
    );
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

#[derive(Deserialize)]
struct ReactQuery {
    emoji: String,
}

// ---------- pins ----------
#[derive(Deserialize)]
struct PinBody {
    message_id: String,
}

async fn pin(
    user: AuthUser,
    State(state): State<AppState>,
    Path(room_hex): Path<String>,
    Json(b): Json<PinBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let (msg, rid) = get_room_message(&state, &room_hex, &b.message_id).await?;
    if msg.deleted {
        return Err(AppError::BadRequest("message deleted".into()));
    }
    let mid = id_of(msg.id, "message")?;
    state
        .db
        .collection::<Pin>(C_PINS)
        .insert_one(
            Pin {
                id: None,
                room_id: rid,
                message_id: mid,
                pinned_by: user.user_id,
                created_at: DateTime::now(),
            },
            None,
        )
        .await
        .map_err(dup_err)?;
    fan_out(
        &state,
        &rid.to_hex(),
        serde_json::json!({
            "type": "pin.updated",
            "room_id": rid.to_hex(),
            "message_id": mid.to_hex(),
            "sender_id": user.user_id.to_hex(),
            "data": { "pinned": true },
        }),
    );
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "ok": true }))))
}

async fn unpin(
    user: AuthUser,
    State(state): State<AppState>,
    Path((room_hex, mid_hex)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let (_, rid) = get_room_message(&state, &room_hex, &mid_hex).await?;
    let mid = parse_id(&mid_hex)?;
    let r = state
        .db
        .collection::<Pin>(C_PINS)
        .delete_one(doc! { "room_id": rid, "message_id": mid }, None)
        .await?;
    if r.deleted_count == 0 {
        return Err(AppError::NotFound("not pinned".into()));
    }
    fan_out(
        &state,
        &rid.to_hex(),
        serde_json::json!({
            "type": "pin.updated",
            "room_id": rid.to_hex(),
            "message_id": mid.to_hex(),
            "sender_id": user.user_id.to_hex(),
            "data": { "pinned": false },
        }),
    );
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

async fn list_pins(
    user: AuthUser,
    State(state): State<AppState>,
    Path(room_hex): Path<String>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let rid = id_of(room.id, "room")?;
    let mut cur = state
        .db
        .collection::<Pin>(C_PINS)
        .find(doc! { "room_id": rid }, None)
        .await?;
    let mut metas = Vec::new();
    let mut msgs = Vec::new();
    while let Some(p) = cur.next().await {
        let p: Pin = p?;
        let mid = p.message_id;
        if let Some(m) = state
            .db
            .collection::<Message>(C_MESSAGES)
            .find_one(doc! { "_id": mid }, None)
            .await?
        {
            metas.push((mid, p.pinned_by, p.created_at));
            msgs.push(m);
        }
    }
    let dtos = enrich(&state, &user.user_id, msgs).await?;
    let out: Vec<serde_json::Value> = metas
        .into_iter()
        .zip(dtos.into_iter())
        .map(|((mid, pinned_by, created_at), dto)| {
            serde_json::json!({
                "message_id": mid.to_hex(),
                "pinned_by": pinned_by.to_hex(),
                "created_at": ts(&created_at),
                "message": dto,
            })
        })
        .collect();
    Ok((StatusCode::OK, Json(serde_json::json!({ "pins": out }))))
}

// ---------- stars ----------
#[derive(Deserialize)]
struct StarBody {
    message_id: String,
}

async fn visible_message(
    state: &AppState,
    user_id: &ObjectId,
    mid_hex: &str,
) -> AppResult<Message> {
    let msg = get_message(state, mid_hex).await?;
    let room_hex = msg.room_id.to_hex();
    let room = get_room(state, &room_hex).await?;
    must_be_member(&room, user_id)?;
    Ok(msg)
}

async fn star(
    user: AuthUser,
    State(state): State<AppState>,
    Json(b): Json<StarBody>,
) -> AppResult<impl IntoResponse> {
    let msg = visible_message(&state, &user.user_id, &b.message_id).await?;
    let mid = id_of(msg.id, "message")?;
    state
        .db
        .collection::<Star>(C_STARS)
        .insert_one(
            Star {
                id: None,
                user_id: user.user_id,
                message_id: mid,
                created_at: DateTime::now(),
            },
            None,
        )
        .await
        .map_err(dup_err)?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "ok": true }))))
}

async fn unstar(
    user: AuthUser,
    State(state): State<AppState>,
    Path(mid_hex): Path<String>,
) -> AppResult<impl IntoResponse> {
    let mid = parse_id(&mid_hex)?;
    let r = state
        .db
        .collection::<Star>(C_STARS)
        .delete_one(doc! { "user_id": user.user_id, "message_id": mid }, None)
        .await?;
    if r.deleted_count == 0 {
        return Err(AppError::NotFound("not starred".into()));
    }
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

async fn list_stars(user: AuthUser, State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let mut cur = state
        .db
        .collection::<Star>(C_STARS)
        .find(
            doc! { "user_id": user.user_id },
            mongodb::options::FindOptions::builder()
                .sort(doc! { "_id": -1 })
                .limit(50)
                .build(),
        )
        .await?;
    let mut msgs = Vec::new();
    while let Some(s) = cur.next().await {
        let s: Star = s?;
        // Skip messages the user can no longer see (left room, deleted room).
        if let Ok(msg) = visible_message(&state, &user.user_id, &s.message_id.to_hex()).await
        {
            msgs.push(msg);
        }
    }
    let out = enrich(&state, &user.user_id, msgs).await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "messages": out }))))
}

// ---------- read watermarks ----------
#[derive(Deserialize)]
struct ReadBody {
    message_id: String,
}

async fn mark_read(
    user: AuthUser,
    State(state): State<AppState>,
    Path(room_hex): Path<String>,
    Json(b): Json<ReadBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let rid = id_of(room.id, "room")?;
    let mid = parse_id(&b.message_id)?;
    // The marker must point at a real message in this room.
    let msg = get_message(&state, &b.message_id).await?;
    if msg.room_id != rid {
        return Err(AppError::NotFound("message not found".into()));
    }
    let now = DateTime::now();
    state
        .db
        .collection::<ReadMarker>(C_READ_MARKERS)
        .update_one(
            doc! { "room_id": rid, "user_id": user.user_id },
            doc! { "$set": { "last_read_id": mid, "updated_at": now } },
            UpdateOptions::builder().upsert(true).build(),
        )
        .await?;
    fan_out(
        &state,
        &rid.to_hex(),
        serde_json::json!({
            "type": "read",
            "room_id": rid.to_hex(),
            "message_id": mid.to_hex(),
            "sender_id": user.user_id.to_hex(),
        }),
    );
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

// ---------- edit / delete ----------
#[derive(Deserialize)]
struct EditBody {
    body: String,
}

async fn edit_message(
    user: AuthUser,
    State(state): State<AppState>,
    Path((room_hex, mid_hex)): Path<(String, String)>,
    Json(b): Json<EditBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let (msg, rid) = get_room_message(&state, &room_hex, &mid_hex).await?;
    if msg.sender_id != user.user_id {
        return Err(AppError::Forbidden("only the author can edit".into()));
    }
    if msg.deleted {
        return Err(AppError::BadRequest("message deleted".into()));
    }
    let body = b.body.trim().to_string();
    if body.is_empty() || body.len() > 4000 {
        return Err(AppError::BadRequest("body must be 1..4000 chars".into()));
    }
    let mid = id_of(msg.id, "message")?;
    let now = DateTime::now();
    state
        .db
        .collection::<Message>(C_MESSAGES)
        .update_one(
            doc! { "_id": mid },
            doc! { "$set": { "body": &body, "edited_at": now } },
            None,
        )
        .await?;
    fan_out(
        &state,
        &rid.to_hex(),
        serde_json::json!({
            "type": "message.edited",
            "room_id": rid.to_hex(),
            "message_id": mid.to_hex(),
            "sender_id": user.user_id.to_hex(),
            "body": body,
        }),
    );
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

async fn delete_message(
    user: AuthUser,
    State(state): State<AppState>,
    Path((room_hex, mid_hex)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &room_hex).await?;
    must_be_member(&room, &user.user_id)?;
    let (msg, rid) = get_room_message(&state, &room_hex, &mid_hex).await?;
    if msg.sender_id != user.user_id {
        return Err(AppError::Forbidden("only the author can delete".into()));
    }
    if msg.deleted {
        return Err(AppError::BadRequest("message deleted".into()));
    }
    let mid = id_of(msg.id, "message")?;
    state
        .db
        .collection::<Message>(C_MESSAGES)
        .update_one(doc! { "_id": mid }, doc! { "$set": { "deleted": true } }, None)
        .await?;
    fan_out(
        &state,
        &rid.to_hex(),
        serde_json::json!({
            "type": "message.deleted",
            "room_id": rid.to_hex(),
            "message_id": mid.to_hex(),
            "sender_id": user.user_id.to_hex(),
        }),
    );
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

// ---------- enrichment (reactions / polls / pins folded into DTOs) ----------
#[derive(Serialize, Clone)]
pub struct ReactionTally {
    pub emoji: String,
    pub count: u32,
    pub mine: bool,
}

#[derive(Serialize, Clone)]
pub struct PollTally {
    pub options: Vec<PollOption>,
    pub total: u32,
    pub my_vote: Option<u32>,
}

#[derive(Serialize, Clone)]
pub struct PollOption {
    pub text: String,
    pub votes: u32,
}

/// Batch-load reactions for a set of messages: one aggregation.
async fn reaction_map(
    state: &AppState,
    mids: &[ObjectId],
    mine: &ObjectId,
) -> AppResult<std::collections::HashMap<String, Vec<ReactionTally>>> {
    use std::collections::HashMap;
    let mut map: HashMap<String, Vec<ReactionTally>> = HashMap::new();
    if mids.is_empty() {
        return Ok(map);
    }
    let mut cur = state
        .db
        .collection::<Reaction>(C_REACTIONS)
        .aggregate(
            vec![
                doc! { "$match": { "message_id": { "$in": mids } } },
                doc! { "$group": {
                    "_id": { "message_id": "$message_id", "emoji": "$emoji" },
                    "count": { "$sum": 1 },
                    "users": { "$addToSet": "$user_id" },
                } },
            ],
            None,
        )
        .await?;
    while let Some(d) = cur.next().await {
        let d: Document = d.map_err(bson_err)?;
        let id = d.get_document("_id").map_err(bson_err)?;
        let mid = id.get_object_id("message_id").map_err(bson_err)?.to_hex();
        let emoji = id.get_str("emoji").map_err(bson_err)?.to_string();
        let count = d.get_i32("count").unwrap_or(0).max(0) as u32;
        let users = d.get_array("users").map(|a| a.to_vec()).unwrap_or_default();
        let mine = users.iter().any(|u| u.as_object_id() == Some(*mine));
        map.entry(mid).or_default().push(ReactionTally { emoji, count, mine });
    }
    Ok(map)
}

/// Which of these messages are pinned in `room`.
async fn pinned_set(state: &AppState, rid: ObjectId, mids: &[ObjectId]) -> AppResult<std::collections::HashSet<String>> {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    if mids.is_empty() {
        return Ok(set);
    }
    let mut cur = state
        .db
        .collection::<Pin>(C_PINS)
        .find(doc! { "room_id": rid, "message_id": { "$in": mids } }, None)
        .await?;
    while let Some(p) = cur.next().await {
        let p: Pin = p?;
        set.insert(p.message_id.to_hex());
    }
    Ok(set)
}

/// Poll tallies + my vote for poll-kind messages.
async fn poll_map(
    state: &AppState,
    msgs: &[Message],
    mine: &ObjectId,
) -> AppResult<std::collections::HashMap<String, PollTally>> {    use std::collections::HashMap;
    let mut map = HashMap::new();
    for m in msgs.iter().filter(|m| m.kind == "poll") {
        let mid = match m.id {
            Some(id) => id,
            None => continue,
        };
        let options: Vec<String> = m
            .metadata
            .as_ref()
            .and_then(|v| v.get("options"))
            .and_then(|o| o.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let tallies = poll_tallies(state, mid, options.len()).await?;
        let total: u32 = tallies.iter().sum();
        let my_vote = state
            .db
            .collection::<PollVote>(C_POLL_VOTES)
            .find_one(doc! { "message_id": mid, "user_id": *mine }, None)
            .await?
            .map(|v| v.option_idx);
        map.insert(
            mid.to_hex(),
            PollTally {
                options: options
                    .into_iter()
                    .enumerate()
                    .map(|(i, text)| PollOption {
                        text,
                        votes: tallies.get(i).copied().unwrap_or(0),
                    })
                    .collect(),
                total,
                my_vote,
            },
        );
    }
    Ok(map)
}

/// Enrich messages with reactions, poll tallies, and pin flags for `mine`.
/// Every endpoint serving MessageDtos goes through here so the shape is
/// uniform (reactions/poll/pinned always present, possibly empty).
pub(crate) async fn enrich(
    state: &AppState,
    mine: &ObjectId,
    msgs: Vec<Message>,
) -> AppResult<Vec<crate::routes::MessageDto>> {
    let mids: Vec<ObjectId> = msgs.iter().filter_map(|m| m.id).collect();
    let reactions = reaction_map(state, &mids, mine).await?;
    let polls = poll_map(state, &msgs, mine).await?;
    // Pins are per-room; union across every room present (stars span rooms).
    let mut by_room: std::collections::HashMap<ObjectId, Vec<ObjectId>> =
        std::collections::HashMap::new();
    for m in &msgs {
        if let Some(id) = m.id {
            by_room.entry(m.room_id).or_default().push(id);
        }
    }
    let mut pinned: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (rid, ids) in by_room {
        pinned.extend(pinned_set(state, rid, &ids).await?);
    }
    let mut out = Vec::with_capacity(msgs.len());
    for m in &msgs {
        let mut dto = crate::routes::MessageDto::of(m)?;
        if let Some(id) = m.id {
            let hex = id.to_hex();
            if let Some(r) = reactions.get(&hex) {
                dto.reactions = r.clone();
            }
            dto.poll = polls.get(&hex).cloned();
            dto.pinned = pinned.contains(&hex);
        }
        out.push(dto);
    }
    Ok(out)
}

pub fn router() -> Router<AppState> {    Router::new()
        .route(
            "/api/v1/rooms/:id/messages/:mid/vote",
            post(vote).delete(unvote),
        )
        .route(
            "/api/v1/rooms/:id/messages/:mid/reactions",
            post(react).delete(unreact),
        )
        .route("/api/v1/rooms/:id/pins", get(list_pins).post(pin))
        .route("/api/v1/rooms/:id/pins/:mid", delete(unpin))
        .route("/api/v1/users/me/starred", get(list_stars).post(star))
        .route("/api/v1/users/me/starred/:mid", delete(unstar))
        .route("/api/v1/rooms/:id/read", post(mark_read))
        .route(
            "/api/v1/rooms/:id/messages/:mid",
            patch(edit_message).delete(delete_message),
        )
}
