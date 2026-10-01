use mongodb::{
    Client, Collection, Database, IndexModel,
    bson::doc,
    options::IndexOptions,
};

use crate::models::{Group, Message, User};

#[derive(Clone)]
pub struct Db {
    pub users: Collection<User>,
    pub groups: Collection<Group>,
    pub messages: Collection<Message>,
}

pub async fn connect(uri: &str, db_name: &str) -> Result<Db, mongodb::error::Error> {
    let client = Client::with_uri_str(uri).await?;
    let db: Database = client.database(db_name);
    let db = Db {
        users: db.collection("users"),
        groups: db.collection("groups"),
        messages: db.collection("messages"),
    };
    db.ensure_indexes().await?;
    Ok(db)
}

impl Db {
    async fn ensure_indexes(&self) -> Result<(), mongodb::error::Error> {
        let username_unique = IndexModel::builder()
            .keys(doc! { "username": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();
        self.users.create_index(username_unique).await?;

        let group_members = IndexModel::builder()
            .keys(doc! { "members": 1 })
            .build();
        self.groups.create_index(group_members).await?;

        let msg_lookup = IndexModel::builder()
            .keys(doc! { "group_id": 1, "created_ms": -1 })
            .build();
        self.messages.create_index(msg_lookup).await?;
        Ok(())
    }
}
