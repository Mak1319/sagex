//! Password hashing (argon2), OTP helpers, and the access-token extractor.

use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use mongodb::bson::oid::ObjectId;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::{
    error::{AppError, AppResult},
    jose_mldsa::Claims,
    state::AppState,
};

// ---------- Passwords ----------
pub fn hash_secret(secret: &str) -> AppResult<String> {
    Argon2::default()
        .hash_password(secret.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("hash failed: {e}")))
}

pub fn verify_secret(secret: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(secret.as_bytes(), &parsed)
        .is_ok()
}

// ---------- OTP ----------
/// 6-digit numeric OTP, generated with a seeded RNG (os entropy).
pub fn new_otp_code() -> String {
    let mut rng = StdRng::from_entropy();
    format!("{:06}", rng.gen_range(0..1_000_000u32))
}

/// Mock delivery: log the code. Swap with SMTP/SMS via this single fn.
pub fn deliver_otp(email: &str, purpose: &str, code: &str) {
    tracing::info!(target: "sagex_chatsrv::otp", "[mock-otp] to={email} purpose={purpose} code={code}");
}

// ---------- Email / input hygiene ----------
pub fn norm_email(raw: &str) -> String {
    raw.trim().to_lowercase()
}

pub fn valid_email(email: &str) -> bool {
    let e = email.trim();
    e.len() >= 5 && e.len() <= 254 && e.contains('@') && e.contains('.') && !e.contains(' ')
}

pub fn valid_password(pw: &str) -> bool {
    pw.len() >= 8 && pw.len() <= 128
}

pub fn valid_username(u: &str) -> bool {
    let u = u.trim();
    (3..=32).contains(&u.len())
        && u.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
}

// ---------- Auth extractor ----------
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: ObjectId,
    #[allow(dead_code)]
    pub claims: Claims,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;
        let token = auth
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("expected Bearer token".into()))?;
        let claims = state.jose.verify(token, "access")?;
        let user_id = ObjectId::parse_str(&claims.sub)
            .map_err(|_| AppError::Unauthorized("bad subject".into()))?;
        Ok(Self { user_id, claims })
    }
}
