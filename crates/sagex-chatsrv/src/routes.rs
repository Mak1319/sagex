use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use mongodb::{
    bson::{doc, oid::ObjectId, DateTime},
    options::FindOptions,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        deliver_otp, hash_secret, new_otp_code, norm_email, valid_email, valid_password,
        valid_username, verify_secret, AuthUser,
    },
    error::{AppError, AppResult},
    models::{Message, Otp, Room, Session, User},
    state::{AppState, C_MESSAGES, C_OTPS, C_ROOMS, C_SESSIONS, C_USERS},
    ws::ws_handler,
};

// ---------- helpers ----------
fn now_plus(secs: i64) -> DateTime {
    DateTime::from_millis(DateTime::now().timestamp_millis() + secs * 1000)
}

fn ts(dt: &DateTime) -> String {
    dt.try_to_rfc3339_string().unwrap_or_default()
}

fn parse_id(hex: &str) -> AppResult<ObjectId> {
    ObjectId::parse_str(hex).map_err(|_| AppError::BadRequest("bad id".into()))
}

fn id_of(o: Option<ObjectId>, what: &str) -> AppResult<ObjectId> {
    o.ok_or_else(|| AppError::Internal(format!("{what} missing _id")))
}

// ---------- DTOs ----------
#[derive(Serialize)]
struct UserDto {
    id: String,
    email: String,
    username: String,
    display_name: Option<String>,
    is_verified: bool,
    created_at: String,
}

impl UserDto {
    fn of(u: &User) -> AppResult<Self> {
        Ok(Self {
            id: id_of(u.id, "user")?.to_hex(),
            email: u.email.clone(),
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            is_verified: u.is_verified,
            created_at: ts(&u.created_at),
        })
    }
}

#[derive(Serialize)]
struct Tokens {
    access_token: String,
    access_expires_at: i64,
    refresh_token: String,
    refresh_expires_at: i64,
    token_type: String,
}

#[derive(Serialize)]
struct RoomDto {
    id: String,
    name: Option<String>,
    is_dm: bool,
    member_ids: Vec<String>,
    created_by: String,
    created_at: String,
}

impl RoomDto {
    fn of(r: &Room) -> AppResult<Self> {
        Ok(Self {
            id: id_of(r.id, "room")?.to_hex(),
            name: r.name.clone(),
            is_dm: r.is_dm,
            member_ids: r.member_ids.iter().map(|o| o.to_hex()).collect(),
            created_by: r.created_by.to_hex(),
            created_at: ts(&r.created_at),
        })
    }
}

#[derive(Serialize)]
struct MessageDto {
    id: String,
    room_id: String,
    sender_id: String,
    body: String,
    created_at: String,
}

impl MessageDto {
    fn of(m: &Message) -> AppResult<Self> {
        Ok(Self {
            id: id_of(m.id, "message")?.to_hex(),
            room_id: m.room_id.to_hex(),
            sender_id: m.sender_id.to_hex(),
            body: m.body.clone(),
            created_at: ts(&m.created_at),
        })
    }
}

// ---------- health ----------
async fn health(State(state): State<AppState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "db": state.config.db_name })),
    )
}

async fn ready(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    state
        .db
        .run_command(doc! { "ping": 1 }, None)
        .await
        .map_err(|e| AppError::Internal(format!("mongo not ready: {e}")))?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ready" })),
    ))
}

// ---------- auth ----------
#[derive(Deserialize)]
struct SignupBody {
    email: String,
    username: String,
    password: String,
    display_name: Option<String>,
}

async fn signup(
    State(state): State<AppState>,
    Json(b): Json<SignupBody>,
) -> AppResult<impl IntoResponse> {
    let email = norm_email(&b.email);
    let username = b.username.trim().to_string();
    if !valid_email(&email) {
        return Err(AppError::BadRequest("invalid email".into()));
    }
    if !valid_username(&username) {
        return Err(AppError::BadRequest(
            "username must be 3..32 chars [a-zA-Z0-9_.-]".into(),
        ));
    }
    if !valid_password(&b.password) {
        return Err(AppError::BadRequest("password must be 8..128 chars".into()));
    }
    let users = state.db.collection::<User>(C_USERS);
    if users
        .find_one(doc! { "email": &email }, None)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict("email already registered".into()));
    }
    if users
        .find_one(doc! { "username": &username }, None)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict("username taken".into()));
    }
    let now = DateTime::now();
    let res = users
        .insert_one(
            User {
                id: None,
                email: email.clone(),
                username: username.into(),
                display_name: b.display_name.filter(|s| !s.trim().is_empty()),
                password_hash: hash_secret(&b.password)?,
                is_verified: false,
                created_at: now,
                updated_at: now,
            },
            None,
        )
        .await?;
    let uid = res
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("insert failed".into()))?
        .to_hex();
    issue_otp(&state, &email, "signup").await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "user_id": uid,
            "message": "signup ok — verify the OTP sent to your email (dev: check server logs)",
        })),
    ))
}

async fn issue_otp(state: &AppState, email: &str, purpose: &str) -> AppResult<()> {
    let otps = state.db.collection::<Otp>(C_OTPS);
    // Cooldown on the latest OTP for this (email, purpose).
    if let Some(prev) = otps
        .find_one(doc! { "email": email, "purpose": purpose }, None)
        .await?
    {
        let elapsed =
            (DateTime::now().timestamp_millis() - prev.updated_at.timestamp_millis()) / 1000;
        if elapsed < state.config.otp_resend_cooldown_secs {
            return Err(AppError::RateLimited(format!(
                "wait {}s before requesting a new code",
                state.config.otp_resend_cooldown_secs - elapsed
            )));
        }
        otps.delete_many(doc! { "email": email, "purpose": purpose }, None)
            .await?;
    }
    let code = new_otp_code();
    let now = DateTime::now();
    otps.insert_one(
        Otp {
            id: None,
            email: email.to_string(),
            purpose: purpose.to_string(),
            code_hash: hash_secret(&code)?,
            attempts: 0,
            verified: false,
            created_at: now,
            updated_at: now,
            expires_at: now_plus(state.config.otp_ttl_secs),
        },
        None,
    )
    .await?;
    deliver_otp(email, purpose, &code);
    Ok(())
}

/// Returns the OTP doc if the code matches (bumps attempts on failure).
async fn check_otp(state: &AppState, email: &str, purpose: &str, code: &str) -> AppResult<Otp> {
    let otps = state.db.collection::<Otp>(C_OTPS);
    let Some(otp) = otps
        .find_one(doc! { "email": email, "purpose": purpose }, None)
        .await?
    else {
        return Err(AppError::NotFound(
            "no OTP found — request one first".into(),
        ));
    };
    if otp.expires_at.timestamp_millis() <= DateTime::now().timestamp_millis() {
        otps.delete_many(doc! { "email": email, "purpose": purpose }, None)
            .await?;
        return Err(AppError::BadRequest(
            "OTP expired — request a new one".into(),
        ));
    }
    if otp.attempts >= state.config.otp_max_attempts {
        return Err(AppError::RateLimited(
            "too many attempts — request a new OTP".into(),
        ));
    }
    if !verify_secret(code.trim(), &otp.code_hash) {
        otps.update_one(
            doc! { "_id": id_of(otp.id, "otp")? },
            doc! { "$inc": { "attempts": 1 } },
            None,
        )
        .await?;
        return Err(AppError::Unauthorized("wrong code".into()));
    }
    Ok(otp)
}

async fn issue_tokens(state: &AppState, user_id: &ObjectId) -> AppResult<Tokens> {
    let sub = user_id.to_hex();
    let (access_token, _, access_exp) = state.jose.issue(&sub, "access")?;
    let (refresh_token, refresh_jti, refresh_exp) = state.jose.issue(&sub, "refresh")?;
    state
        .db
        .collection::<Session>(C_SESSIONS)
        .insert_one(
            Session {
                id: None,
                user_id: *user_id,
                jti: refresh_jti,
                revoked: false,
                created_at: DateTime::now(),
                expires_at: DateTime::from_millis(refresh_exp * 1000),
            },
            None,
        )
        .await?;
    Ok(Tokens {
        access_token,
        access_expires_at: access_exp,
        refresh_token,
        refresh_expires_at: refresh_exp,
        token_type: "Bearer".into(),
    })
}

#[derive(Deserialize)]
struct VerifyOtpBody {
    email: String,
    code: String,
    purpose: String, // "signup" | "reset" | "login"
}

async fn verify_otp(
    State(state): State<AppState>,
    Json(b): Json<VerifyOtpBody>,
) -> AppResult<impl IntoResponse> {
    let email = norm_email(&b.email);
    if !matches!(b.purpose.as_str(), "signup" | "reset" | "login") {
        return Err(AppError::BadRequest(
            "purpose must be signup|reset|login".into(),
        ));
    }
    let otp = check_otp(&state, &email, &b.purpose, &b.code).await?;
    let users = state.db.collection::<User>(C_USERS);
    let Some(user) = users.find_one(doc! { "email": &email }, None).await? else {
        return Err(AppError::NotFound("user not found".into()));
    };
    let uid = id_of(user.id, "user")?;
    let otps = state.db.collection::<Otp>(C_OTPS);
    match b.purpose.as_str() {
        "signup" => {
            users
                .update_one(
                    doc! { "_id": uid },
                    doc! { "$set": { "is_verified": true, "updated_at": DateTime::now() } },
                    None,
                )
                .await?;
            otps.delete_many(doc! { "email": &email, "purpose": "signup" }, None)
                .await?;
            let tokens = issue_tokens(&state, &uid).await?;
            Ok((StatusCode::OK, Json(serde_json::to_value(tokens).unwrap())))
        }
        "login" => {
            if !user.is_verified {
                return Err(AppError::Forbidden("verify your email first".into()));
            }
            otps.delete_many(doc! { "email": &email, "purpose": "login" }, None)
                .await?;
            let tokens = issue_tokens(&state, &uid).await?;
            Ok((StatusCode::OK, Json(serde_json::to_value(tokens).unwrap())))
        }
        _ => {
            // reset: mark verified; reset-password consumes it.
            otps.update_one(
                doc! { "_id": id_of(otp.id, "otp")? },
                doc! { "$set": { "verified": true, "updated_at": DateTime::now() } },
                None,
            )
            .await?;
            Ok((
                StatusCode::OK,
                Json(serde_json::json!({ "ok": true, "reset_allowed": true })),
            ))
        }
    }
}

#[derive(Deserialize)]
struct RequestOtpBody {
    email: String,
    purpose: String,
}

async fn request_otp(
    State(state): State<AppState>,
    Json(b): Json<RequestOtpBody>,
) -> AppResult<impl IntoResponse> {
    let email = norm_email(&b.email);
    if !matches!(b.purpose.as_str(), "signup" | "reset" | "login") {
        return Err(AppError::BadRequest(
            "purpose must be signup|reset|login".into(),
        ));
    }
    // Anti-enumeration for reset: always generic 200.
    if b.purpose == "reset" {
        let users = state.db.collection::<User>(C_USERS);
        if users
            .find_one(doc! { "email": &email }, None)
            .await?
            .is_some()
        {
            let _ = issue_otp(&state, &email, "reset").await;
        }
        return Ok((
            StatusCode::OK,
            Json(
                serde_json::json!({ "ok": true, "message": "if the email exists, an OTP was sent" }),
            ),
        ));
    }
    issue_otp(&state, &email, &b.purpose).await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true, "message": "OTP sent (dev: check server logs)" })),
    ))
}

#[derive(Deserialize)]
struct LoginBody {
    email: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    Json(b): Json<LoginBody>,
) -> AppResult<impl IntoResponse> {
    let email = norm_email(&b.email);
    let users = state.db.collection::<User>(C_USERS);
    let Some(user) = users.find_one(doc! { "email": &email }, None).await? else {
        return Err(AppError::Unauthorized("invalid credentials".into()));
    };
    if !verify_secret(&b.password, &user.password_hash) {
        return Err(AppError::Unauthorized("invalid credentials".into()));
    }
    if !user.is_verified {
        return Err(AppError::Forbidden(
            "email not verified — complete OTP verification".into(),
        ));
    }
    let tokens = issue_tokens(&state, &id_of(user.id, "user")?).await?;
    Ok((StatusCode::OK, Json(serde_json::to_value(tokens).unwrap())))
}

#[derive(Deserialize)]
struct RefreshBody {
    refresh_token: String,
}

async fn refresh(
    State(state): State<AppState>,
    Json(b): Json<RefreshBody>,
) -> AppResult<impl IntoResponse> {
    let claims = state.jose.verify(&b.refresh_token, "refresh")?;
    let sessions = state.db.collection::<Session>(C_SESSIONS);
    let Some(sess) = sessions.find_one(doc! { "jti": &claims.jti }, None).await? else {
        return Err(AppError::Unauthorized("unknown session".into()));
    };
    if sess.revoked {
        return Err(AppError::Unauthorized("session revoked".into()));
    }
    // Rotate: revoke old, issue new pair.
    sessions
        .update_one(
            doc! { "jti": &claims.jti },
            doc! { "$set": { "revoked": true } },
            None,
        )
        .await?;
    let uid = ObjectId::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("bad subject".into()))?;
    let tokens = issue_tokens(&state, &uid).await?;
    Ok((StatusCode::OK, Json(serde_json::to_value(tokens).unwrap())))
}

async fn logout(
    user: AuthUser,
    State(state): State<AppState>,
    Json(b): Json<RefreshBody>,
) -> AppResult<impl IntoResponse> {
    let claims = state.jose.verify(&b.refresh_token, "refresh")?;
    if claims.sub != user.user_id.to_hex() {
        return Err(AppError::Forbidden("not your session".into()));
    }
    state
        .db
        .collection::<Session>(C_SESSIONS)
        .update_one(
            doc! { "jti": &claims.jti },
            doc! { "$set": { "revoked": true } },
            None,
        )
        .await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

async fn logout_all(user: AuthUser, State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let res = state
        .db
        .collection::<Session>(C_SESSIONS)
        .update_many(
            doc! { "user_id": user.user_id },
            doc! { "$set": { "revoked": true } },
            None,
        )
        .await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true, "revoked": res.modified_count })),
    ))
}

#[derive(Deserialize)]
struct ForgotBody {
    email: String,
}

async fn forgot_password(
    State(state): State<AppState>,
    Json(b): Json<ForgotBody>,
) -> AppResult<impl IntoResponse> {
    request_otp(
        State(state),
        Json(RequestOtpBody {
            email: b.email,
            purpose: "reset".into(),
        }),
    )
    .await
}

#[derive(Deserialize)]
struct ResetBody {
    email: String,
    code: String,
    new_password: String,
}

async fn reset_password(
    State(state): State<AppState>,
    Json(b): Json<ResetBody>,
) -> AppResult<impl IntoResponse> {
    if !valid_password(&b.new_password) {
        return Err(AppError::BadRequest("password must be 8..128 chars".into()));
    }
    let email = norm_email(&b.email);
    check_otp(&state, &email, "reset", &b.code).await?;
    let users = state.db.collection::<User>(C_USERS);
    let res = users
        .update_one(
            doc! { "email": &email },
            doc! { "$set": {
                "password_hash": hash_secret(&b.new_password)?,
                "is_verified": true,
                "updated_at": DateTime::now(),
            } },
            None,
        )
        .await?;
    if res.matched_count == 0 {
        return Err(AppError::NotFound("user not found".into()));
    }
    // Consume OTPs + revoke all sessions (all devices logged out).
    state
        .db
        .collection::<Otp>(C_OTPS)
        .delete_many(doc! { "email": &email }, None)
        .await?;
    let user = users
        .find_one(doc! { "email": &email }, None)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;
    state
        .db
        .collection::<Session>(C_SESSIONS)
        .update_many(
            doc! { "user_id": id_of(user.id, "user")? },
            doc! { "$set": { "revoked": true } },
            None,
        )
        .await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true, "message": "password reset — log in again" })),
    ))
}

async fn jwks(State(state): State<AppState>) -> impl IntoResponse {
    (StatusCode::OK, Json(state.jose.jwks()))
}

// ---------- users ----------
async fn me(user: AuthUser, State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let users = state.db.collection::<User>(C_USERS);
    let u = users
        .find_one(doc! { "_id": user.user_id }, None)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;
    Ok((
        StatusCode::OK,
        Json(serde_json::to_value(UserDto::of(&u)?).unwrap()),
    ))
}

#[derive(Deserialize)]
struct UpdateMe {
    username: Option<String>,
    display_name: Option<String>,
}

async fn update_me(
    user: AuthUser,
    State(state): State<AppState>,
    Json(b): Json<UpdateMe>,
) -> AppResult<impl IntoResponse> {
    let users = state.db.collection::<User>(C_USERS);
    let mut set = doc! { "updated_at": DateTime::now() };
    if let Some(username) = b.username {
        let username = username.trim().to_string();
        if !valid_username(&username) {
            return Err(AppError::BadRequest("bad username".into()));
        }
        if let Some(other) = users.find_one(doc! { "username": &username }, None).await? {
            if id_of(other.id, "user")? != user.user_id {
                return Err(AppError::Conflict("username taken".into()));
            }
        }
        set.insert("username", username);
    }
    if let Some(dn) = b.display_name {
        set.insert("display_name", dn.trim().to_string());
    }
    users
        .update_one(doc! { "_id": user.user_id }, doc! { "$set": set }, None)
        .await?;
    me(user, State(state)).await
}

#[derive(Deserialize)]
struct ListQuery {
    q: Option<String>,
    page: Option<i64>,
    limit: Option<i64>,
}

fn regex_escape(s: &str) -> String {
    // tiny escape without the regex crate: prefix meta chars with backslash
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if ".+*?^${}()|[]\\".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

async fn list_users(
    _user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<impl IntoResponse> {
    let limit = q.limit.unwrap_or(20).clamp(1, 50);
    let page = q.page.unwrap_or(0).max(0);
    let mut filter = doc! {};
    if let Some(term) = q.q.filter(|s| !s.trim().is_empty()) {
        let rx = doc! { "$regex": regex_escape(term.trim()), "$options": "i" };
        filter = doc! { "$or": [ { "username": rx.clone() }, { "email": rx } ] };
    }
    let users = state.db.collection::<User>(C_USERS);
    let opts = FindOptions::builder()
        .sort(doc! { "_id": -1 })
        .limit(limit)
        .skip((page * limit) as u64)
        .build();
    let mut cur = users.find(filter, opts).await?;
    let mut out = Vec::new();
    {
        use futures::StreamExt;
        while let Some(u) = cur.next().await {
            out.push(UserDto::of(&u?)?);
        }
    }
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "users": out, "page": page, "limit": limit })),
    ))
}

// ---------- rooms ----------
#[derive(Deserialize)]
struct CreateRoom {
    name: Option<String>,
    is_dm: Option<bool>,
    member_ids: Option<Vec<String>>,
}

async fn create_room(
    user: AuthUser,
    State(state): State<AppState>,
    Json(b): Json<CreateRoom>,
) -> AppResult<impl IntoResponse> {
    let is_dm = b.is_dm.unwrap_or(false);
    let mut members: Vec<ObjectId> = vec![user.user_id];
    for hex in b.member_ids.unwrap_or_default() {
        let oid = parse_id(&hex)?;
        if !members.contains(&oid) {
            members.push(oid);
        }
    }
    // Validate members exist.
    let users = state.db.collection::<User>(C_USERS);
    for m in &members {
        if users.find_one(doc! { "_id": m }, None).await?.is_none() {
            return Err(AppError::BadRequest(format!(
                "unknown member {}",
                m.to_hex()
            )));
        }
    }
    if is_dm && members.len() != 2 {
        return Err(AppError::BadRequest(
            "DM rooms need exactly 2 members".into(),
        ));
    }
    let name = b
        .name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if !is_dm && name.is_none() {
        return Err(AppError::BadRequest("group rooms need a name".into()));
    }
    let rooms = state.db.collection::<Room>(C_ROOMS);
    let res = rooms
        .insert_one(
            Room {
                id: None,
                name,
                is_dm,
                member_ids: members,
                created_by: user.user_id,
                created_at: DateTime::now(),
            },
            None,
        )
        .await?;
    let room = rooms
        .find_one(
            doc! { "_id": res.inserted_id.as_object_id().unwrap() },
            None,
        )
        .await?
        .ok_or_else(|| AppError::Internal("insert failed".into()))?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(RoomDto::of(&room)?).unwrap()),
    ))
}

async fn list_rooms(user: AuthUser, State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let rooms = state.db.collection::<Room>(C_ROOMS);
    let opts = FindOptions::builder().sort(doc! { "_id": -1 }).build();
    let mut cur = rooms
        .find(doc! { "member_ids": user.user_id }, opts)
        .await?;
    let mut out = Vec::new();
    {
        use futures::StreamExt;
        while let Some(r) = cur.next().await {
            out.push(RoomDto::of(&r?)?);
        }
    }
    Ok((StatusCode::OK, Json(serde_json::json!({ "rooms": out }))))
}

async fn get_room(state: &AppState, room_hex: &str) -> AppResult<Room> {
    let rid = parse_id(room_hex)?;
    state
        .db
        .collection::<Room>(C_ROOMS)
        .find_one(doc! { "_id": rid }, None)
        .await?
        .ok_or_else(|| AppError::NotFound("room not found".into()))
}

fn must_be_member(room: &Room, user: &ObjectId) -> AppResult<()> {
    if room.member_ids.contains(user) {
        Ok(())
    } else {
        Err(AppError::Forbidden("not a room member".into()))
    }
}

#[derive(Deserialize)]
struct JoinBody {
    user_id: Option<String>,
}

async fn join_room(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<JoinBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    let rid = id_of(room.id, "room")?;
    let target = match b.user_id {
        Some(hex) => {
            // Only members can add others.
            must_be_member(&room, &user.user_id)?;
            parse_id(&hex)?
        }
        None => user.user_id,
    };
    state
        .db
        .collection::<Room>(C_ROOMS)
        .update_one(
            doc! { "_id": rid },
            doc! { "$addToSet": { "member_ids": target } },
            None,
        )
        .await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

async fn leave_room(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    must_be_member(&room, &user.user_id)?;
    state
        .db
        .collection::<Room>(C_ROOMS)
        .update_one(
            doc! { "_id": id_of(room.id, "room")? },
            doc! { "$pull": { "member_ids": user.user_id } },
            None,
        )
        .await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

// ---------- messages ----------
#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<i64>,
    before: Option<String>, // paginate: message id upper bound (exclusive)
}

async fn history(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<HistoryQuery>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    must_be_member(&room, &user.user_id)?;
    let rid = id_of(room.id, "room")?;
    let limit = q.limit.unwrap_or(30).clamp(1, 100);
    let mut filter = doc! { "room_id": rid };
    if let Some(before) = q.before {
        filter.insert("_id", doc! { "$lt": parse_id(&before)? });
    }
    let opts = FindOptions::builder()
        .sort(doc! { "_id": -1 })
        .limit(limit)
        .build();
    let mut cur = state
        .db
        .collection::<Message>(C_MESSAGES)
        .find(filter, opts)
        .await?;
    let mut out = Vec::new();
    {
        use futures::StreamExt;
        while let Some(m) = cur.next().await {
            out.push(MessageDto::of(&m?)?);
        }
    }
    out.reverse(); // ascending for clients
    Ok((StatusCode::OK, Json(serde_json::json!({ "messages": out }))))
}

#[derive(Deserialize)]
struct PostBody {
    body: String,
}

async fn post_message(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<PostBody>,
) -> AppResult<impl IntoResponse> {
    let room = get_room(&state, &id).await?;
    must_be_member(&room, &user.user_id)?;
    let body = b.body.trim().to_string();
    if body.is_empty() || body.len() > 4000 {
        return Err(AppError::BadRequest("body must be 1..4000 chars".into()));
    }
    let rid = id_of(room.id, "room")?;
    let msg = Message {
        id: None,
        room_id: rid,
        sender_id: user.user_id,
        body,
        created_at: DateTime::now(),
    };
    let res = state
        .db
        .collection::<Message>(C_MESSAGES)
        .insert_one(msg, None)
        .await?;
    let mid = res.inserted_id.as_object_id().unwrap().to_hex();
    // Fan out to WS subscribers of this room.
    let dto = MessageDto {
        id: mid,
        room_id: rid.to_hex(),
        sender_id: user.user_id.to_hex(),
        body: b.body.trim().to_string(),
        created_at: ts(&DateTime::now()),
    };
    state.hub.broadcast_text(
        &rid.to_hex(),
        serde_json::json!({
            "type": "message",
            "room_id": dto.room_id,
            "message_id": dto.id,
            "sender_id": dto.sender_id,
            "body": dto.body,
        })
        .to_string(),
    );
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(dto).unwrap()),
    ))
}

// ---------- router ----------
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/v1/auth/signup", post(signup))
        .route("/api/v1/auth/verify-otp", post(verify_otp))
        .route("/api/v1/auth/request-otp", post(request_otp))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/refresh", post(refresh))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/logout-all", post(logout_all))
        .route("/api/v1/auth/forgot-password", post(forgot_password))
        .route("/api/v1/auth/reset-password", post(reset_password))
        .route("/api/v1/auth/jwks.json", get(jwks))
        .route("/api/v1/users/me", get(me).put(update_me))
        .route("/api/v1/users", get(list_users))
        .route("/api/v1/rooms", post(create_room).get(list_rooms))
        .route("/api/v1/rooms/:id/join", post(join_room))
        .route("/api/v1/rooms/:id/leave", post(leave_room))
        .route(
            "/api/v1/rooms/:id/messages",
            get(history).post(post_message),
        )
        .route("/ws/chat", get(ws_handler))
        .with_state(state)
}
