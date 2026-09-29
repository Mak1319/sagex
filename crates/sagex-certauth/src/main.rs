//! sagex-certauth: permit-gated X.509 (ML-DSA-65) certificate service.
//!
//! Modes:
//!   serve (default) — run the Axum API (`GET /`, `/health`, `/db/ping`,
//!     `POST /v1/csr`, `GET /v1/certs/:serial[/verify]`).
//!   issue-permit --identity NAME [--ttl-hours N] [--out PATH]
//!     — offline JOSE permit minting with the CA key (load-only, never
//!     generates; run on the CA host or inside the isolated test-env).

use sagex_certauth::{app, ca::CaMaterial, permit};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use zeroize::Zeroizing;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn require_env(key: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    std::env::var(key)
        .map(|v| Zeroizing::new(v.into_bytes()))
        .map_err(|_| format!("{key} is not set (see .env.example)"))
}

fn load_dotenvs() {
    // Single root `.env` is the source of truth; crate-local files are legacy fallback.
    // Real environment variables always win over any file.
    let _ = dotenvy::dotenv();
    let _ = dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
    let _ = dotenvy::from_path("crates/sagex-certauth/.env");
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  sagex-certauth [serve]\n  sagex-certauth issue-permit --identity NAME [--ttl-hours N] [--out PATH]"
    );
    std::process::exit(2);
}

#[tokio::main]
async fn main() {
    load_dotenvs();
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("serve") => serve().await,
        Some("issue-permit") => issue_permit(args),
        _ => usage(),
    }
}

fn ca_config() -> Result<(PathBuf, String), String> {
    let dir = PathBuf::from(env_or("CA_KEY_DIR", "./ca-data"));
    let identity = env_or("CA_IDENTITY", "sagex-ca");
    if !sagex_certauth::ca::valid_identity(&identity) {
        return Err(format!("bad CA_IDENTITY: {identity}"));
    }
    Ok((dir, identity))
}

async fn serve() {
    let host = env_or("HOST", "127.0.0.1");
    let port: u16 = env_or("PORT", "8080").parse().unwrap_or_else(|_| {
        eprintln!("WARN: invalid PORT value, falling back to 8080");
        8080
    });
    let mongo_uri = env_or("MONGODB_URI", "mongodb://127.0.0.1:27017");
    let db_name = env_or("MONGO_DB", "sagexcadb");
    let validity_days: u64 = env_or("CA_VALIDITY_DAYS", "365").parse().unwrap_or_else(|_| {
        eprintln!("WARN: invalid CA_VALIDITY_DAYS, falling back to 365");
        365
    });
    let password = require_env("CA_PASSWORD").unwrap_or_else(|e| {
        eprintln!("ERROR: {e}");
        std::process::exit(1);
    });
    let (ca_dir, ca_identity) = ca_config().unwrap_or_else(|e| {
        eprintln!("ERROR: {e}");
        std::process::exit(1);
    });

    let (ca, generated) =
        CaMaterial::load_or_generate(&ca_dir, &password, &ca_identity).unwrap_or_else(|e| {
            eprintln!("ERROR: CA setup failed: {e}");
            std::process::exit(1);
        });
    if generated {
        println!("generated fresh CA keys for {:?} in {}", ca.identity, ca_dir.display());
    } else {
        println!("loaded CA keys for {:?} from {}", ca.identity, ca_dir.display());
    }
    // Decrypt once; keep the signer in memory, drop the password.
    let signer = ca.signer(&password).unwrap_or_else(|e| {
        eprintln!("ERROR: CA unlock failed (wrong CA_PASSWORD?): {e}");
        std::process::exit(1);
    });
    drop(password);

    let client = mongodb::Client::with_uri_str(&mongo_uri)
        .await
        .expect("failed to create MongoDB client");
    let state = app::AppState {
        client,
        db_name: db_name.clone(),
        ca_identity: ca.identity.clone(),
        ca_dsa_public: ca.dsa_public.clone(),
        signer: Arc::new(signer),
        validity_days,
    };
    if app::ping_mongo(&state).await {
        println!("sagex-certauth connected to MongoDB database `{db_name}`");
        if let Err(e) = app::ensure_indexes(&state).await {
            eprintln!("ERROR: index setup failed: {e}");
            std::process::exit(1);
        }
    } else {
        eprintln!("WARN: could not ping MongoDB database `{db_name}` at startup (server still running)");
    }

    let addr: SocketAddr = format!("{host}:{port}").parse().unwrap_or_else(|_| {
        eprintln!("WARN: invalid HOST/PORT, falling back to 127.0.0.1:8080");
        "127.0.0.1:8080".parse().unwrap()
    });
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    app::serve(listener, state).await.expect("server error");
}

fn issue_permit(mut args: std::iter::Skip<std::env::Args>) -> ! {
    let mut identity: Option<String> = None;
    let mut ttl_hours: Option<u64> = None;
    let mut out: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--identity" => identity = args.next(),
            "--ttl-hours" => {
                ttl_hours = args.next().and_then(|v| v.parse().ok());
            }
            "--out" => out = args.next(),
            _ => usage(),
        }
    }
    let identity = identity.unwrap_or_else(|| {
        eprintln!("ERROR: --identity is required");
        std::process::exit(2);
    });
    if !sagex_certauth::ca::valid_identity(&identity) {
        eprintln!("ERROR: bad --identity: {identity}");
        std::process::exit(2);
    }
    let ttl_secs = ttl_hours
        .or_else(|| std::env::var("PERMIT_TTL_HOURS").ok().and_then(|v| v.parse().ok()))
        .unwrap_or(24)
        .saturating_mul(3600);
    let password = require_env("CA_PASSWORD").unwrap_or_else(|e| {
        eprintln!("ERROR: {e}");
        std::process::exit(1);
    });
    let (ca_dir, _) = ca_config().unwrap_or_else(|e| {
        eprintln!("ERROR: {e}");
        std::process::exit(1);
    });
    // Load-only: never generate here — a permit from a fresh throwaway CA
    // would be worthless (and mask a misconfigured CA_KEY_DIR).
    let ca = CaMaterial::load(&ca_dir).unwrap_or_else(|e| {
        eprintln!("ERROR: {e}");
        std::process::exit(1);
    });
    let signer = ca.signer(&password).unwrap_or_else(|e| {
        eprintln!("ERROR: CA unlock failed (wrong CA_PASSWORD?): {e}");
        std::process::exit(1);
    });
    drop(password);
    let token = permit::mint(&ca, &signer, &identity, ttl_secs).unwrap_or_else(|e| {
        eprintln!("ERROR: permit minting failed: {e}");
        std::process::exit(1);
    });
    match out {
        Some(path) => {
            std::fs::write(&path, &token).unwrap_or_else(|e| {
                eprintln!("ERROR: cannot write {path}: {e}");
                std::process::exit(1);
            });
            eprintln!("permit for {:?} written to {path}", identity);
        }
        None => println!("{token}"),
    }
    eprintln!(
        "permit sub={:?} iss={:?} ttl={}s",
        identity, ca.identity, ttl_secs
    );
    std::process::exit(0);
}
