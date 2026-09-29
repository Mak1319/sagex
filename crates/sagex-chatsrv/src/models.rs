use mongodb::bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

// ---------- Users ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub email: String, // normalized lowercase
    pub username: String,
    pub display_name: Option<String>,
    pub password_hash: String, // argon2 PHC string
    pub is_verified: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

// ---------- OTPs ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Otp {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub email: String,
    /// "signup" | "reset" | "login"
    pub purpose: String,
    /// argon2 hash of the 6-digit code
    pub code_hash: String,
    pub attempts: u32,
    pub verified: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub expires_at: DateTime,
}

// ---------- Sessions (refresh-token revocation) ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub user_id: ObjectId,
    /// JWT ID of the refresh token
    pub jti: String,
    pub revoked: bool,
    pub created_at: DateTime,
    pub expires_at: DateTime,
}

// ---------- Rooms ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: Option<String>,
    pub is_dm: bool,
    pub member_ids: Vec<ObjectId>,
    pub created_by: ObjectId,
    pub created_at: DateTime,
}

// ---------- Messages ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub room_id: ObjectId,
    pub sender_id: ObjectId,
    pub body: String,
    pub created_at: DateTime,
}
