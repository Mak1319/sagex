mod auth;
mod config;
mod db;
mod documents;
mod groups;
mod messages;
mod models;
mod s3;
mod seed;
mod state;
mod tokens;

use std::path::PathBuf;

use axum::{
    Router,
    middleware,
    routing::{delete, get, post},
};
use clap::Parser;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use zeroize::Zeroizing;

use state::{AppState, ServiceKeys};

#[derive(Parser, Debug)]
#[command(name = "sagex-chatsrv", about = "SAGEX chat compiled-service")]
struct Args {
    /// Path to config.toml (env dir = its parent)
    #[arg(long, default_value = "environments/chatsrv/config.toml")]
    config: PathBuf,

    /// One-shot: seed Mongo from an example-test-set dir, then exit
    #[arg(long)]
    seed: Option<PathBuf>,
}

type DynError = Box<dyn std::error::Error>;

fn load_service_keys(cfg: &config::Config) -> Result<ServiceKeys, DynError> {
    let prv_bytes = std::fs::read(&cfg.keys.prv_path)
        .map_err(|e| format!("read {}: {e}", cfg.keys.prv_path.display()))?;
    let pub_bytes = std::fs::read(&cfg.keys.pub_path)
        .map_err(|e| format!("read {}: {e}", cfg.keys.pub_path.display()))?;
    let prv = sagex_format::format::PrivateExternal::from_bytes(&prv_bytes)
        .map_err(|e| format!("parse prv: {e}"))?;
    let publ = sagex_format::format::PublicFileFormatExternal::from_bytes(&pub_bytes)
        .map_err(|e| format!("parse pub: {e}"))?;
    let password: Zeroizing<Vec<u8>> = Zeroizing::new(
        std::env::var(&cfg.keys.password_env)
            .map_err(|_| format!("env {} not set", cfg.keys.password_env))?
            .into_bytes(),
    );
    if password.is_empty() {
        return Err(format!("env {} is empty", cfg.keys.password_env).into());
    }
    // Fail fast: the password must actually unwrap the DSA seed.
    sagex_crypto::aes::AESHandler::decrypt_private_key(
        &password,
        &prv.internal.key_encapsulation_dsa,
    )
    .map_err(|_| "service .prv password does not unwrap the DSA seed".to_string())?;
    Ok(ServiceKeys {
        enc_dsa: prv.internal.key_encapsulation_dsa,
        vk_pub: publ.internal.key_dsa,
        password,
    })
}

async fn health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let args = Args::parse();
    let env_dir: PathBuf = args
        .config
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let cfg = config::load(&env_dir, &args.config)?;

    let jwt_secret: Zeroizing<Vec<u8>> = Zeroizing::new(
        std::env::var("JWT_SECRET")
            .map_err(|_| "env JWT_SECRET not set (put it in <env>/.env)")?
            .into_bytes(),
    );
    if jwt_secret.is_empty() {
        return Err("env JWT_SECRET is empty".into());
    }

    let svc = load_service_keys(&cfg)?;
    let db = db::connect(&cfg.mongo.uri, &cfg.mongo.db).await?;

    if let Some(dir) = args.seed {
        let summary = seed::seed_from_dir(&db, &dir)
            .await
            .map_err(|e| format!("seed failed: {e}"))?;
        println!("{summary}");
        return Ok(());
    }

    // Best-effort bucket ensure (dev http endpoints); never fatal.
    match s3::S3::from_config(
        &cfg.rustfs.endpoint,
        &cfg.rustfs.bucket,
        &cfg.rustfs.region,
        &cfg.rustfs.access_key,
        &cfg.rustfs.secret_key,
    ) {
        Ok(s3) => {
            if let Err(e) = s3::ensure_bucket(&s3).await {
                eprintln!("warn: rustfs bucket ensure skipped: {e}");
            }
        }
        Err(e) => eprintln!("warn: bad rustfs config: {e}"),
    }

    let state = AppState {
        cfg: cfg.clone(),
        db,
        jwt_secret,
        svc,
    };

    let protected = Router::new()
        .route("/groups", post(groups::create_group).get(groups::list_groups))
        .route("/groups/{id}", get(groups::get_group))
        .route("/groups/{id}/members", post(groups::add_members))
        .route(
            "/groups/{id}/members/{user}",
            delete(groups::remove_member),
        )
        .route(
            "/groups/{id}/messages",
            post(messages::post_message).get(messages::list_messages),
        )
        .route("/documents/upload-url", post(documents::upload_url))
        .route("/documents/download-url", get(documents::download_url))
        .route("/service/token", post(tokens::service_token))
        .route("/service/pubkey", get(tokens::service_pubkey))
        .route("/ca/token", post(tokens::ca_token))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    let app = Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .merge(protected)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("sagex-chatsrv on {addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
