use std::net::IpAddr;

use axum::{Router, routing::get};
use mongodb::Client;

use error::AppError;

mod db;
mod error;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3000;
const DEFAULT_MONGODB_URI: &str = "mongodb://127.0.0.1:27017";
const DEFAULT_MONGODB_DB: &str = "certificate-authority";

async fn health() -> &'static str {
    "ok"
}

async fn root() -> &'static str {
    "hello user"
}

fn app(client: Client) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .with_state(client)
}

fn load_config() -> Result<(IpAddr, u16, String, String), AppError> {
    let _ = dotenvy::dotenv();
    let host: IpAddr = match dotenvy::var("HOST") {
        Ok(s) => s.parse().map_err(AppError::BadHost)?,
        Err(_) => DEFAULT_HOST.parse().map_err(AppError::BadHost)?,
    };
    let port: u16 = match dotenvy::var("PORT") {
        Ok(s) => s.parse().map_err(AppError::BadPort)?,
        Err(_) => DEFAULT_PORT,
    };
    let mongo_uri = dotenvy::var("MONGODB_URI").unwrap_or_else(|_| DEFAULT_MONGODB_URI.to_string());
    let mongo_db = dotenvy::var("MONGODB_DB").unwrap_or_else(|_| DEFAULT_MONGODB_DB.to_string());
    Ok((host, port, mongo_uri, mongo_db))
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let (host, port, mongo_uri, _mongo_db) = load_config()?;
    let client = db::connect(&mongo_uri).await?;
    let listener = tokio::net::TcpListener::bind((host, port))
        .await
        .map_err(AppError::Bind)?;

    println!("Listening: http://{host}:{port}");
    axum::serve(listener, app(client))
        .await
        .map_err(AppError::Serve)
}
