mod auth;
mod ca;
mod config;
mod db;
mod routes;

use axum::{Router, routing::{get, post}};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg_path = config::config_path_arg();
    if let Some(dotenv) = find_dotenv(&cfg_path) {
        let _ = dotenvy::from_path(&dotenv);
    }

    let settings = config::Settings::load(&cfg_path)?;
    let secrets = config::Secrets::load();
    if secrets.ca_privkey_path.is_empty() {
        anyhow::bail!("CA_PRIVKEY_PATH not set (see .env)");
    }

    // Mongo optional at boot; routes return 503 if absent.
    let mongo = if secrets.mongodb_uri.is_empty() {
        eprintln!("sagex-authsrv: MONGODB_URI unset, running without DB");
        None
    } else {
        match db::connect(
            &secrets.mongodb_uri,
            &settings.mongodb_db,
            &settings.mongodb_collection,
        )
        .await
        {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("sagex-authsrv: mongodb connect failed ({e}), running without DB");
                None
            }
        }
    };

    let state = config::AppState {
        settings: settings.clone(),
        secrets,
        mongo,
    };

    let app = Router::new()
        .route("/healthz", get(routes::healthz))
        .route("/csr/issue", post(routes::issue::issue))
        .route("/keys/{user_name}", get(routes::lookup::lookup))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&settings.bind_addr).await?;
    println!("sagex-authsrv listening on {}", settings.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

/// Look for `.env` next to the config file (env dir), else CWD `.env`.
fn find_dotenv(cfg_path: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(cfg_path);
    if let Some(dir) = p.parent() {
        let cand = dir.join(".env");
        if cand.exists() {
            return Some(cand);
        }
    }
    if std::path::Path::new(".env").exists() {
        return Some(".env".into());
    }
    None
}
