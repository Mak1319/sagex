use serde::{Deserialize, Serialize};

/// Chat user. `_id` is the object key (UUIDv7 string) — the primary key
/// used across the whole system. `username` is unique but NOT the key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id")]
    pub object_key: String,
    pub username: String,
    pub pw_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kem_pub: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsa_pub: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_jti: Option<String>,
    pub created_ms: i64,
}

/// Group. Members/admins reference user object keys, never usernames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    #[serde(rename = "_id")]
    pub object_key: String,
    pub name: String,
    pub members: Vec<String>,
    pub admins: Vec<String>,
    pub created_by: String,
    pub created_ms: i64,
}

/// Message body is an opaque client-encrypted blob. The server never
/// decrypts it; `rustfs_ref` optionally points at a bigger encrypted
/// object in RustFS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "_id")]
    pub object_key: String,
    pub group_id: String,
    pub sender_key: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rustfs_ref: Option<String>,
    pub created_ms: i64,
}
