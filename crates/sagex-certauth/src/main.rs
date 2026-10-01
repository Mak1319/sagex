//! sagex-certauth: permit-gated X.509 (ML-DSA-65) certificate service.
//!
//! Modes:
//!   serve (default) — run the Axum API (`GET /`, `/health`, `/db/ping`,
//!     `POST /v1/permits`, `POST /v1/csr`, `GET /v1/certs/:serial[/verify]`,
//!     `GET /v1/keys[/:identity]`).
//!   issue-permit --identity NAME [--ttl-hours N] [--out PATH] [--kind server|user]
//!     — offline JOSE permit minting with the CA key (load-only, never
//!     generates; run on the CA host or inside the isolated test-env).
//!     Mints kind=server permits (service access, e.g. auditor); --kind user
//!     is a dev-only escape hatch behind SAGEX_DEV_PERMITS=1.
//!
//! Peer keys (BLS + RG) are pinned statically at startup via env
//! (`CHATSRV_PK_B64`/`CHATSRV_KID`, `RG_PK_B64`/`RG_ID`): verified locally,
//! never synced or fetched.

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

/// Load a pinned ML-DSA-65 public key (STANDARD base64) or exit. Pins are
/// static by design (prototype: no rotation, no syncing).
fn pinned_key(key: &str) -> Vec<u8> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let raw = std::env::var(key).unwrap_or_else(|_| {
        eprintln!("ERROR: {key} is not set (pin the peer key; see .env.example)");
        std::process::exit(1);
    });
    let bytes = STANDARD.decode(raw.trim()).unwrap_or_else(|_| {
        eprintln!("ERROR: {key} is not valid base64");
        std::process::exit(1);
    });
    if bytes.len() != sagex_auth::DSA_PUBLIC_KEY_BYTES {
        eprintln!(
            "ERROR: {key} has {} bytes (want {})",
            bytes.len(),
            sagex_auth::DSA_PUBLIC_KEY_BYTES
        );
        std::process::exit(1);
    }
    bytes
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

    // Pinned peer keys (BLS + RG): static config, verified locally, never
    // synced or fetched. Missing/malformed pins are fatal — a CA that
    // cannot verify its peers must not serve.
    let chatsrv_dsa_public = pinned_key("CHATSRV_PK_B64");
    let chatsrv_kid = env_or("CHATSRV_KID", "");
    if chatsrv_kid.trim().is_empty() {
        eprintln!("ERROR: CHATSRV_KID is not set (pin the BLS signer identity)");
        std::process::exit(1);
    }
    let rg_dsa_public = pinned_key("RG_PK_B64");
    let rg_id = env_or("RG_ID", "");
    if rg_id.trim().is_empty() {
        eprintln!("ERROR: RG_ID is not set (pin the gateway identity)");
        std::process::exit(1);
    }
    let require_chatsrv_token = env_or("REQUIRE_CHATSRV_TOKEN", "false")
        .parse::<bool>()
        .unwrap_or(false);
    let permit_ttl_secs = env_or("PERMIT_TTL_HOURS", "24")
        .parse::<u64>()
        .unwrap_or(24)
        .saturating_mul(3600);
    println!("pinned BLS key (kid {chatsrv_kid:?}) and RG key (id {rg_id:?})");
    if require_chatsrv_token {
        println!("strict mode: CSR requires a chatsrv token");
    }

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
        chatsrv_dsa_public,
        chatsrv_kid: chatsrv_kid.trim().to_string(),
        rg_dsa_public,
        rg_id: rg_id.trim().to_string(),
        require_chatsrv_token,
        permit_ttl_secs,
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
    let mut kind_flag: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--identity" => identity = args.next(),
            "--ttl-hours" => {
                ttl_hours = args.next().and_then(|v| v.parse().ok());
            }
            "--out" => out = args.next(),
            // Dev-only escape hatch (see below): mint a user-kind permit.
            "--kind" => kind_flag = args.next(),
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
    // The offline CLI authorizes SERVICES, never user enrollment: it mints
    // kind=server permits only. A user kind is available solely behind the
    // dev-only escape hatch SAGEX_DEV_PERMITS=1 (never default-on), for
    // throwaway e2e runs — production user permits come from POST /v1/permits.
    let kind = match kind_flag.as_deref() {
        None => sagex_certauth::permit::PERMIT_KIND_SERVER,
        Some("server") => sagex_certauth::permit::PERMIT_KIND_SERVER,
        Some("user") => {
            if std::env::var("SAGEX_DEV_PERMITS").as_deref() != Ok("1") {
                eprintln!(
                    "ERROR: --kind user is a dev-only escape hatch (set SAGEX_DEV_PERMITS=1); \
                     the CLI mints server permits. User permits come from POST /v1/permits."
                );
                std::process::exit(2);
            }
            eprintln!("WARN: dev-only user permit mint (SAGEX_DEV_PERMITS=1) — never in production");
            sagex_certauth::permit::PERMIT_KIND_USER
        }
        Some(other) => {
            eprintln!("ERROR: bad --kind {other:?} (want \"server\" or dev-only \"user\")");
            std::process::exit(2);
        }
    };
    let token =
        permit::mint(&ca, &signer, &identity, kind, ttl_secs).unwrap_or_else(|e| {
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
        "permit sub={:?} iss={:?} kind={:?} ttl={}s",
        identity, ca.identity, kind, ttl_secs
    );
    std::process::exit(0);
}
