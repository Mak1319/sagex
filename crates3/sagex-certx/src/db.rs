use error::AppError;

use crate::error;

pub async fn connect(uri: &str) -> Result<mongodb::Client, AppError> {
    let options = mongodb::options::ClientOptions::parse(uri)
        .await
        .map_err(AppError::BadMongoUri)?;
    let client = mongodb::Client::with_options(options).map_err(AppError::Mongo)?;
    client
        .database("admin")
        .run_command(mongodb::bson::doc! { "ping": 1 })
        .await
        .map_err(AppError::Mongo)?;
    Ok(client)
}
