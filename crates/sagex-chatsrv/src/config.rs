use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub mongodb_uri: String,
    pub db_name: String,
    pub bind_addr: String,
    pub access_token_ttl_secs: i64,
    pub refresh_token_ttl_secs: i64,
    pub otp_ttl_secs: i64,
    pub otp_resend_cooldown_secs: i64,
    pub otp_max_attempts: u32,
    pub mldsa65_sk_b64: Option<String>,
    pub mldsa65_pk_b64: Option<String>,
    pub mldsa65_kid: String,
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_i64(key: &str, default: i64) -> i64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_opt(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.trim().is_empty())
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        Self {
            mongodb_uri: env_or(
                "MONGODB_URI",
                "mongodb://root:example@localhost:27017/chatdbx?authSource=admin",
            ),
            db_name: env_or("DB_NAME", "chatdbx"),
            bind_addr: env_or("BIND_ADDR", "0.0.0.0:8080"),
            access_token_ttl_secs: env_i64("ACCESS_TOKEN_TTL_SECS", 900),
            refresh_token_ttl_secs: env_i64("REFRESH_TOKEN_TTL_SECS", 2_592_000),
            otp_ttl_secs: env_i64("OTP_TTL_SECS", 600),
            otp_resend_cooldown_secs: env_i64("OTP_RESEND_COOLDOWN_SECS", 60),
            otp_max_attempts: env_i64("OTP_MAX_ATTEMPTS", 5) as u32,
            mldsa65_sk_b64: env_opt("MLDSA65_SK_B64"),
            mldsa65_pk_b64: env_opt("MLDSA65_PK_B64"),
            mldsa65_kid: env_or("MLDSA65_KID", "sagex-chatsrv-mldsa65-01"),
        }
    }
}
