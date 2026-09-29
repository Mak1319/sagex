use std::sync::Arc;
use std::time::Duration;

use mongodb::{
    bson::doc,
    options::{ClientOptions, IndexOptions},
    Client, Database, IndexModel,
};

use crate::{
    config::Config,
    error::AppResult,
    jose_mldsa::JoseSigner,
    storage::SharedStorage,
    ws::ChatHub,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub jose: Arc<JoseSigner>,
    pub hub: Arc<ChatHub>,
    pub config: Arc<Config>,
    pub storage: SharedStorage,
}

pub const C_USERS: &str = "users";
pub const C_OTPS: &str = "otps";
pub const C_SESSIONS: &str = "sessions";
pub const C_ROOMS: &str = "rooms";
pub const C_MESSAGES: &str = "messages";
pub const C_POLL_VOTES: &str = "poll_votes";
pub const C_REACTIONS: &str = "reactions";
pub const C_PINS: &str = "pins";
pub const C_STARS: &str = "stars";
pub const C_READ_MARKERS: &str = "read_markers";
pub const C_ROOM_SETTINGS: &str = "room_settings";
pub const C_REPORTS: &str = "reports";
pub const C_BLOCKS: &str = "blocks";

pub async fn connect_db(cfg: &Config) -> AppResult<Database> {
    let mut opts = ClientOptions::parse(&cfg.mongodb_uri)
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("bad MONGODB_URI: {e}")))?;
    opts.app_name = Some("sagex-chatsrv".to_string());
    let client = Client::with_options(opts)
        .map_err(|e| crate::error::AppError::Internal(format!("mongo client: {e}")))?;
    Ok(client.database(&cfg.db_name))
}

/// Unique + TTL indexes. Idempotent — safe to run on every boot.
pub async fn ensure_indexes(db: &Database) -> AppResult<()> {
    use crate::models::{Block, Message, Otp, Pin, PollVote, Reaction, ReadMarker, Report, Room, RoomSettings, Session, Star, User};
    let users = db.collection::<User>(C_USERS);
    users
        .create_index(
            IndexModel::builder()
                .keys(doc! { "email": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .unique(true)
                        .build(),
                )
                .build(),
            None,
        )
        .await?;
    users
        .create_index(
            IndexModel::builder()
                .keys(doc! { "username": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .unique(true)
                        .build(),
                )
                .build(),
            None,
        )
        .await?;

    // OTPs expire automatically via expires_at.
    db.collection::<Otp>(C_OTPS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "expires_at": 1 })
                .options(
                    IndexOptions::builder()
                        .expire_after(Duration::from_secs(0))
                        .build(),
                )
                .build(),
            None,
        )
        .await?;
    db.collection::<Otp>(C_OTPS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "email": 1, "purpose": 1 })
                .build(),
            None,
        )
        .await?;

    db.collection::<Session>(C_SESSIONS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "expires_at": 1 })
                .options(
                    IndexOptions::builder()
                        .expire_after(Duration::from_secs(0))
                        .build(),
                )
                .build(),
            None,
        )
        .await?;
    db.collection::<Session>(C_SESSIONS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "jti": 1 })
                .options(
                    mongodb::options::IndexOptions::builder()
                        .unique(true)
                        .build(),
                )
                .build(),
            None,
        )
        .await?;

    db.collection::<Room>(C_ROOMS)
        .create_index(
            IndexModel::builder().keys(doc! { "member_ids": 1 }).build(),
            None,
        )
        .await?;
    db.collection::<Message>(C_MESSAGES)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "room_id": 1, "_id": -1 })
                .build(),
            None,
        )
        .await?;
    // Full-text-ish search support (substring fallback via regex).
    db.collection::<Message>(C_MESSAGES)
        .create_index(
            IndexModel::builder().keys(doc! { "body": "text" }).build(),
            None,
        )
        .await?;

    let unique = || {
        IndexOptions::builder()
            .unique(true)
            .build()
    };
    // One vote per user per message (change = delete + re-vote).
    db.collection::<PollVote>(C_POLL_VOTES)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "message_id": 1, "user_id": 1 })
                .options(unique())
                .build(),
            None,
        )
        .await?;
    // One reaction-emoji per user per message (toggle semantics).
    db.collection::<Reaction>(C_REACTIONS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "message_id": 1, "user_id": 1, "emoji": 1 })
                .options(unique())
                .build(),
            None,
        )
        .await?;
    db.collection::<Reaction>(C_REACTIONS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "message_id": 1 })
                .build(),
            None,
        )
        .await?;
    db.collection::<Pin>(C_PINS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "room_id": 1, "message_id": 1 })
                .options(unique())
                .build(),
            None,
        )
        .await?;
    db.collection::<Star>(C_STARS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "user_id": 1, "message_id": 1 })
                .options(unique())
                .build(),
            None,
        )
        .await?;
    db.collection::<ReadMarker>(C_READ_MARKERS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "room_id": 1, "user_id": 1 })
                .options(unique())
                .build(),
            None,
        )
        .await?;
    db.collection::<RoomSettings>(C_ROOM_SETTINGS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "room_id": 1, "user_id": 1 })
                .options(unique())
                .build(),
            None,
        )
        .await?;
    // One report per reporter/target pair; one block row per pair.
    db.collection::<Report>(C_REPORTS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "reporter_id": 1, "reported_id": 1 })
                .options(unique())
                .build(),
            None,
        )
        .await?;
    db.collection::<Block>(C_BLOCKS)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "blocker_id": 1, "blocked_id": 1 })
                .options(unique())
                .build(),
            None,
        )
        .await?;
    Ok(())
}
