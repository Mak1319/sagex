use serde::{Deserialize, Serialize};

// ---------- auth ----------
#[derive(Debug, Clone, Serialize)]
pub struct SignupReq {
    pub email: String,
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignupRes {
    pub user_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyOtpReq {
    pub email: String,
    pub code: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub access_expires_at: i64,
    pub refresh_token: String,
    pub refresh_expires_at: i64,
    pub token_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResetAllowed {
    pub ok: bool,
    pub reset_allowed: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestOtpReq {
    pub email: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshReq {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForgotReq {
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResetReq {
    pub email: String,
    pub code: String,
    pub new_password: String,
}

// ---------- users ----------
#[derive(Debug, Clone, Deserialize)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub username: String,
    pub display_name: Option<String>,
    pub is_verified: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserList {
    pub users: Vec<UserDto>,
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateMeReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

// ---------- rooms / messages ----------
#[derive(Debug, Clone, Serialize)]
pub struct CreateRoomReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dm: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoomDto {
    pub id: String,
    pub name: Option<String>,
    pub is_dm: bool,
    pub member_ids: Vec<String>,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoomList {
    pub rooms: Vec<RoomDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JoinReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageDto {
    pub id: String,
    pub room_id: String,
    pub sender_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageList {
    pub messages: Vec<MessageDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PostMessageReq {
    pub body: String,
}

// ---------- errors ----------
#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorBody {
    pub error: Option<String>,
    pub code: Option<String>,
}
