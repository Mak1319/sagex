use crate::config::UserRecord;
use mongodb::{Collection, IndexModel, bson::doc, options::IndexOptions};

pub async fn connect(uri: &str, db: &str, coll: &str) -> anyhow::Result<Collection<UserRecord>> {
    let client = mongodb::Client::with_uri_str(uri).await?;
    let collection: Collection<UserRecord> = client.database(db).collection(coll);
    let opts = IndexOptions::builder().unique(true).build();
    let model = IndexModel::builder()
        .keys(doc! { "user_name": 1 })
        .options(opts)
        .build();
    let _ = collection.create_index(model).await;
    Ok(collection)
}

pub async fn upsert_user(
    coll: &Collection<UserRecord>,
    rec: UserRecord,
) -> anyhow::Result<()> {
    coll.update_one(
        doc! { "user_name": rec.user_name.clone() },
        doc! { "$set": {
            "user_name": rec.user_name.clone(),
            "key_kem": rec.key_kem.clone(),
            "key_dsa": rec.key_dsa.clone(),
            "cert_pem": rec.cert_pem.clone(),
            "issued_at": rec.issued_at,
        }},
    )
    .upsert(true)
    .await?;
    Ok(())
}

pub async fn find_user(
    coll: &Collection<UserRecord>,
    user_name: &str,
) -> anyhow::Result<Option<UserRecord>> {
    Ok(coll
        .find_one(doc! { "user_name": user_name })
        .await?)
}
