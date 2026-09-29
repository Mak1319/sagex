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
    pub status: Option<String>,
    /// MinIO object key of the avatar image (fetched via presigned GET).
    pub avatar_key: Option<String>,
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
///
/// `kind` is one of: text|image|video|voice|document|contact|poll|event|sticker
/// (default "text" for pre-kind documents). `metadata` carries kind-specific
/// payload: caption, duration/bars, file name+size+object_key, contact
/// fields, poll question/options, event title/when, sticker glyph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub room_id: ObjectId,
    pub sender_id: ObjectId,
    pub body: String,
    #[serde(default = "default_message_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<DateTime>,
    /// WhatsApp-style tombstone: the record stays, content is hidden.
    #[serde(default)]
    pub deleted: bool,
    pub created_at: DateTime,
}

fn default_message_kind() -> String {
    "text".to_string()
}

// ---------- Poll votes ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollVote {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub message_id: ObjectId,
    pub user_id: ObjectId,
    pub option_idx: u32,
    pub created_at: DateTime,
}

// ---------- Reactions ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub message_id: ObjectId,
    pub user_id: ObjectId,
    pub emoji: String,
    pub created_at: DateTime,
}

// ---------- Pins (per room) ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub room_id: ObjectId,
    pub message_id: ObjectId,
    pub pinned_by: ObjectId,
    pub created_at: DateTime,
}

// ---------- Stars (per user) ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Star {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub user_id: ObjectId,
    pub message_id: ObjectId,
    pub created_at: DateTime,
}

// ---------- Read watermarks (per user per room) ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadMarker {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub room_id: ObjectId,
    pub user_id: ObjectId,
    pub last_read_id: ObjectId,
    pub updated_at: DateTime,
}

// ---------- Per-user per-room settings (mute/fav/archive/pin/disappearing) ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSettings {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub room_id: ObjectId,
    pub user_id: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted_until: Option<DateTime>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub fav: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disappearing_secs: Option<u64>,
    pub updated_at: DateTime,
}

// ---------- Reports ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub reporter_id: ObjectId,
    pub reported_id: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub created_at: DateTime,
}

// ---------- Blocks ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub blocker_id: ObjectId,
    pub blocked_id: ObjectId,
    pub created_at: DateTime,
}
