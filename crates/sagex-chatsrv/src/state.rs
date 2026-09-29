use std::sync::Arc;
use std::time::Duration;

use mongodb::{
    bson::doc,
    options::{ClientOptions, IndexOptions},
    Client, Database, IndexModel,
};

use crate::{config::Config, error::AppResult, jose_mldsa::JoseSigner, ws::ChatHub};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub jose: Arc<JoseSigner>,
    pub hub: Arc<ChatHub>,
    pub config: Arc<Config>,
}

pub const C_USERS: &str = "users";
pub const C_OTPS: &str = "otps";
pub const C_SESSIONS: &str = "sessions";
pub const C_ROOMS: &str = "rooms";
pub const C_MESSAGES: &str = "messages";

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
    use crate::models::{Message, Otp, Room, Session, User};
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
    Ok(())
}
