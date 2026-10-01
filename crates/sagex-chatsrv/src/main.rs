use std::sync::Arc;

use sagex_chatsrv::{
    config::Config,
    jose_mldsa::JoseSigner,
    routes,
    state::{connect_db, ensure_indexes, AppState},
    storage::{FakeStorage, S3Storage, SharedStorage, Storage},
    ws::ChatHub,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::from_env();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sagex_chatsrv=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db = connect_db(&cfg).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    ensure_indexes(&db)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    tracing::info!("connected to mongodb db={}", cfg.db_name);

    let (signer, ephemeral) = JoseSigner::from_env_or_ephemeral(
        cfg.mldsa65_sk_b64.as_deref(),
        cfg.mldsa65_pk_b64.as_deref(),
        cfg.mldsa65_kid.clone(),
        cfg.access_token_ttl_secs,
        cfg.refresh_token_ttl_secs,
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    if ephemeral {
        tracing::warn!(
            "ML-DSA-65 keypair is EPHEMERAL (dev only). Pin it via env:\nMLDSA65_PK_B64={}\n(all tokens invalidate on restart; persist keys for production)",
            signer.public_key_b64()
        );
    } else {
        tracing::info!("ML-DSA-65 signer loaded (kid={})", signer.kid());
    }

    // Object storage: real MinIO when configured, in-memory fake otherwise.
    // A configured-but-unreachable MinIO is fatal (media URLs must resolve);
    // an unconfigured one only warns (text chat is unaffected).
    let storage: SharedStorage = if cfg.minio_endpoint.trim().is_empty() {
        tracing::warn!(
            "MINIO_ENDPOINT unset — media endpoints use an in-memory fake (dev only)"
        );
        Arc::new(FakeStorage::default())
    } else {
        let s3 = S3Storage::new(
            &cfg.minio_endpoint,
            &cfg.minio_bucket,
            &cfg.minio_access_key,
            &cfg.minio_secret_key,
        )
        .map_err(|e| anyhow::anyhow!("minio config: {e}"))?;
        s3.ensure_bucket()
            .await
            .map_err(|e| anyhow::anyhow!("minio bootstrap: {e}"))?;
        tracing::info!(
            "minio ready: endpoint={} bucket={}",
            cfg.minio_endpoint,
            cfg.minio_bucket
        );
        Arc::new(s3)
    };

    let state = AppState {
        db,
        jose: Arc::new(signer),
        hub: Arc::new(ChatHub::new()),
        config: Arc::new(cfg.clone()),
        storage,
    };
    let app = routes::router(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("bind {}: {e}", cfg.bind_addr))?;
    tracing::info!("sagex-chatsrv listening on {}", cfg.bind_addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutdown signal received");
        })
        .await
        .map_err(|e| anyhow::anyhow!("serve: {e}"))?;
    Ok(())
}
