//! Username/password auth with hand-rolled HS256 JWT (no extra deps:
//! hmac + sha2 + base64 are already in the tree) and argon2 password hashes.

use argon2::{Argon2, PasswordVerifier};
use password_hash::{PasswordHasher, phc::PasswordHash};
use axum::{
    Json,
    extract::{FromRequestParts, Request, State},
    http::{StatusCode, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    models::User,
    state::{AppState, AuthUser},
};

pub type ApiResult<T> = Result<T, (StatusCode, Json<Value>)>;

pub fn api_err(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": msg.into() })))
}

// ---- JWT (HS256, hand-rolled) ----

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    username: String,
    typ: String,
    iat: i64,
    exp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    jti: Option<String>,
}

fn b64u(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

fn hs256(secret: &[u8], input: &str) -> Vec<u8> {
    let mut mac =
        Hmac::<sha2::Sha256>::new_from_slice(secret).expect("hmac takes any key size");
    mac.update(input.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

pub fn mint(
    secret: &[u8],
    sub: &str,
    username: &str,
    typ: &str,
    ttl_secs: i64,
    jti: Option<String>,
) -> String {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: sub.to_string(),
        username: username.to_string(),
        typ: typ.to_string(),
        iat: now,
        exp: now + ttl_secs,
        jti,
    };
    let header = b64u(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = b64u(serde_json::to_string(&claims).unwrap().as_bytes());
    let input = format!("{header}.{payload}");
    let sig = b64u(&hs256(secret, &input));
    format!("{input}.{sig}")
}

fn verify(secret: &[u8], token: &str, expect_typ: &str) -> Result<Claims, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("malformed token".to_string());
    }
    let input = format!("{}.{}", parts[0], parts[1]);
    let mut mac =
        Hmac::<sha2::Sha256>::new_from_slice(secret).map_err(|_| "bad secret".to_string())?;
    mac.update(input.as_bytes());
    let sig = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| "bad signature encoding".to_string())?;
    mac.verify_slice(&sig)
        .map_err(|_| "bad signature".to_string())?;
    let payload = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| "bad payload encoding".to_string())?;
    let claims: Claims =
        serde_json::from_slice(&payload).map_err(|_| "bad payload".to_string())?;
    if claims.typ != expect_typ {
        return Err("wrong token type".to_string());
    }
    if claims.exp <= Utc::now().timestamp() {
        return Err("token expired".to_string());
    }
    Ok(claims)
}

// ---- password hashing ----

fn hash_password(password: &str) -> Result<String, String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

fn check_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

// ---- handlers ----

#[derive(Deserialize)]
pub struct RegisterBody {
    pub username: String,
    pub password: String,
    pub kem_pub: Option<String>,
    pub dsa_pub: Option<String>,
}

pub async fn register(
    State(st): State<AppState>,
    Json(body): Json<RegisterBody>,
) -> ApiResult<impl IntoResponse> {
    let username = body.username.trim().to_string();
    if username.is_empty() || username.len() > 64 {
        return Err(api_err(StatusCode::BAD_REQUEST, "bad username"));
    }
    if body.password.len() < 8 {
        return Err(api_err(StatusCode::BAD_REQUEST, "password too short"));
    }
    let object_key = Uuid::now_v7().to_string();
    let pw_hash = hash_password(&body.password).map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let user = User {
        object_key: object_key.clone(),
        username: username.clone(),
        pw_hash,
        kem_pub: body.kem_pub,
        dsa_pub: body.dsa_pub,
        refresh_jti: None,
        created_ms: Utc::now().timestamp_millis(),
    };
    match st.db.users.insert_one(user).await {
        Ok(_) => Ok((StatusCode::CREATED, Json(json!({ "object_key": object_key, "username": username })))),
        Err(e) if e.to_string().contains("11000") => {
            Err(api_err(StatusCode::CONFLICT, "username taken"))
        }
        Err(e) => Err(api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[derive(Deserialize)]
pub struct LoginBody {
    pub username: String,
    pub password: String,
}

fn issue_pair(st: &AppState, user: &User, jti: &str) -> (String, String) {
    let access = mint(
        &st.jwt_secret,
        &user.object_key,
        &user.username,
        "access",
        st.cfg.auth.access_ttl_mins * 60,
        None,
    );
    let refresh = mint(
        &st.jwt_secret,
        &user.object_key,
        &user.username,
        "refresh",
        st.cfg.auth.refresh_ttl_days * 86400,
        Some(jti.to_string()),
    );
    (access, refresh)
}

pub async fn login(
    State(st): State<AppState>,
    Json(body): Json<LoginBody>,
) -> ApiResult<impl IntoResponse> {
    let user: User = st
        .db
        .users
        .find_one(doc! { "username": body.username.trim() })
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| api_err(StatusCode::UNAUTHORIZED, "bad credentials"))?;
    if !check_password(&body.password, &user.pw_hash) {
        return Err(api_err(StatusCode::UNAUTHORIZED, "bad credentials"));
    }
    let jti = Uuid::new_v4().to_string();
    st.db
        .users
        .update_one(
            doc! { "_id": &user.object_key },
            doc! { "$set": { "refresh_jti": &jti } },
        )
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (access, refresh) = issue_pair(&st, &user, &jti);
    Ok(Json(json!({
        "access_token": access,
        "refresh_token": refresh,
        "object_key": user.object_key,
        "username": user.username,
    })))
}

#[derive(Deserialize)]
pub struct RefreshBody {
    pub refresh_token: String,
}

pub async fn refresh(
    State(st): State<AppState>,
    Json(body): Json<RefreshBody>,
) -> ApiResult<impl IntoResponse> {
    let claims = verify(&st.jwt_secret, &body.refresh_token, "refresh")
        .map_err(|e| api_err(StatusCode::UNAUTHORIZED, e))?;
    let jti = claims.jti.ok_or_else(|| api_err(StatusCode::UNAUTHORIZED, "no jti"))?;
    let user: User = st
        .db
        .users
        .find_one(doc! { "_id": &claims.sub })
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| api_err(StatusCode::UNAUTHORIZED, "unknown user"))?;
    if user.refresh_jti.as_deref() != Some(jti.as_str()) {
        return Err(api_err(StatusCode::UNAUTHORIZED, "refresh rotated"));
    }
    let new_jti = Uuid::new_v4().to_string();
    st.db
        .users
        .update_one(
            doc! { "_id": &user.object_key },
            doc! { "$set": { "refresh_jti": &new_jti } },
        )
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (access, refresh) = issue_pair(&st, &user, &new_jti);
    Ok(Json(json!({ "access_token": access, "refresh_token": refresh })))
}

// ---- middleware ----

pub async fn auth_middleware(
    State(st): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    match verify(&st.jwt_secret, token, "access") {
        Ok(c) => {
            req.extensions_mut().insert(AuthUser {
                object_key: c.sub,
                username: c.username,
            });
            next.run(req).await
        }
        Err(_) => api_err(StatusCode::UNAUTHORIZED, "unauthorized").into_response(),
    }
}

/// Lets handlers take `auth: AuthUser` directly. Runs after the
/// middleware, which is what inserts the value into extensions.
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or_else(|| api_err(StatusCode::UNAUTHORIZED, "unauthorized"))
    }
}
