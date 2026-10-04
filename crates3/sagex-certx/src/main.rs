use std::net::IpAddr;
use std::path::PathBuf;

use axum::{Router, routing::get};
use clap::{Parser, Subcommand};
use mongodb::Client;
use zeroize::Zeroizing;

use error::AppError;

mod db;
mod error;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3000;
const DEFAULT_MONGODB_URI: &str = "mongodb://127.0.0.1:27017";
const DEFAULT_MONGODB_DB: &str = "certificate-authority";
const DEFAULT_KEYS_DIR: &str = "./keys";

#[derive(Parser)]
#[command(name = "sagex-certx", version, about = "sagex certificate server")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Make the server's own key pair and prepare the trusted keys dir.
    Init,
    /// Serve the certificate API.
    Listen,
}

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

fn load_dotenv() {
    let _ = dotenvy::dotenv();
}

fn server_config() -> Result<(IpAddr, u16, String, String), AppError> {
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

fn trusted_keys_dir() -> Result<PathBuf, AppError> {
    match dotenvy::var("TRUSTED_KEYS_DIR") {
        Ok(dir) => Ok(PathBuf::from(dir)),
        Err(_) => std::env::var("HOME")
            .map(|home| PathBuf::from(home).join(".sagex-keys"))
            .map_err(|_| AppError::MissingEnv("HOME")),
    }
}

async fn run_init() -> Result<(), AppError> {
    let user = dotenvy::var("KEY_USER").map_err(|_| AppError::MissingEnv("KEY_USER"))?;
    let password: Zeroizing<String> =
        Zeroizing::new(dotenvy::var("PASSWORD").map_err(|_| AppError::MissingEnv("PASSWORD"))?);
    let keys_dir = dotenvy::var("KEYS_DIR").unwrap_or_else(|_| DEFAULT_KEYS_DIR.to_string());
    let trust_dir = trusted_keys_dir()?;
    tokio::fs::create_dir_all(&trust_dir)
        .await
        .map_err(|e| AppError::ReadDir(trust_dir.clone(), e))?;
    let (_private_path, public_path) = sagex_archive::create_user_keys(
        &user,
        password.as_bytes(),
        std::path::Path::new(&keys_dir),
    )
    .map_err(AppError::KeyGen)?;
    println!("wrote private key {}", _private_path.display());
    println!("wrote public key  {}", public_path.display());
    let trust_path = trust_dir.join(format!("{user}.pub"));
    tokio::fs::copy(&public_path, &trust_path)
        .await
        .map_err(|e| AppError::CopyKey(trust_path.clone(), e))?;
    println!("trusted own key {}", trust_path.display());
    Ok(())
}

async fn run_listen() -> Result<(), AppError> {
    let (host, port, mongo_uri, _mongo_db) = server_config()?;
    let client = db::connect(&mongo_uri).await?;
    let listener = tokio::net::TcpListener::bind((host, port))
        .await
        .map_err(AppError::Bind)?;

    println!("Listening: http://{host}:{port}");
    axum::serve(listener, app(client))
        .await
        .map_err(AppError::Serve)
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    load_dotenv();
    match Cli::parse().command {
        Commands::Init => run_init().await,
        Commands::Listen => run_listen().await,
    }
}
