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
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub avatar_key: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_key: Option<String>,
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
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub edited_at: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub reactions: Vec<ReactionTally>,
    #[serde(default)]
    pub poll: Option<PollTally>,
    #[serde(default)]
    pub pinned: bool,
}

fn default_kind() -> String {
    "text".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionTally {
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub mine: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PollOption {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub votes: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PollTally {
    #[serde(default)]
    pub options: Vec<PollOption>,
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub my_vote: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageList {
    pub messages: Vec<MessageDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PostMessageReq {
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
}

// ---------- errors ----------
#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorBody {
    pub error: Option<String>,
    pub code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full server shape (chatsrv MessageDto with enrichment) must parse.
    #[test]
    fn message_dto_full_shape() {
        let v: MessageDto = serde_json::from_value(serde_json::json!({
            "id": "abc", "room_id": "r1", "sender_id": "u1",
            "body": "hi", "created_at": "2026-01-01T00:00:00Z",
            "kind": "poll",
            "metadata": {"question": "q?"},
            "reply_to": "def",
            "edited_at": null,
            "deleted": false,
            "reactions": [{"emoji": "👍", "count": 2, "mine": true}],
            "poll": {"options": [{"text": "a", "votes": 2}], "total": 2, "my_vote": 0},
            "pinned": true,
            "future_field": "ignored",
        }))
        .unwrap();
        assert_eq!(v.kind, "poll");
        assert_eq!(v.reply_to.as_deref(), Some("def"));
        assert!(!v.deleted);
        assert_eq!(v.reactions.len(), 1);
        assert_eq!(v.reactions[0].emoji, "👍");
        assert!(v.reactions[0].mine);
        let poll = v.poll.unwrap();
        assert_eq!(poll.total, 2);
        assert_eq!(poll.my_vote, Some(0));
        assert!(v.pinned);
    }

    /// Legacy/pre-enrichment payloads (only the original five fields) must
    /// still parse via defaults.
    #[test]
    fn message_dto_legacy_shape() {
        let v: MessageDto = serde_json::from_value(serde_json::json!({
            "id": "abc", "room_id": "r1", "sender_id": "u1",
            "body": "hi", "created_at": "2026-01-01T00:00:00Z",
        }))
        .unwrap();
        assert_eq!(v.kind, "text");
        assert!(v.metadata.is_none());
        assert!(v.reply_to.is_none());
        assert!(!v.deleted);
        assert!(v.reactions.is_empty());
        assert!(v.poll.is_none());
        assert!(!v.pinned);
    }

    /// Nones stay off the wire so old servers keep working.
    #[test]
    fn post_message_req_omits_nones() {
        let v = serde_json::to_value(&PostMessageReq {
            body: "hi".into(),
            kind: None,
            metadata: None,
            reply_to: None,
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"body": "hi"}));
        let v = serde_json::to_value(&PostMessageReq {
            body: "hi".into(),
            kind: Some("image".into()),
            metadata: Some(serde_json::json!({"object_key": "media/k"})),
            reply_to: Some("def".into()),
        })
        .unwrap();
        assert_eq!(v["kind"], "image");
        assert_eq!(v["reply_to"], "def");
    }
}
