use std::sync::Arc;

use clap::{Parser, Subcommand};

use sagex_gateway::{api, config::GatewayConfig, AuthVerifier, LedgerClient, Outbox};

#[derive(Parser)]
#[command(name = "sagex-gateway", about = "Register gateway: HTTP intake + outbox + ledger proxy (sole writer)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print a gateway.toml template.
    Init,
    /// Run the gateway from a TOML config file.
    Run {
        #[arg(long)]
        config: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // Size the auditor ring buffer from config before any event is emitted.
    // (Unparseable config falls back to the default here; run() re-reads
    // strictly and fails fast, so nothing is masked.)
    let buf_cap = match &cli.cmd {
        Cmd::Run { config } => GatewayConfig::from_file(config)
            .map(|c| c.logs.buffer)
            .unwrap_or(2000),
        Cmd::Init => 2000,
    };
    // Same auditor ring buffer as the ledger nodes (served at GET /logs).
    let log_buffer = sagex_ledger::LogBuffer::init_global(buf_cap).clone();
    {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .with(tracing_subscriber::fmt::layer())
            .with(sagex_ledger::LogLayer::new(log_buffer))
            .init();
    }
    match cli.cmd {
        Cmd::Init => {
            println!("# sagex-gateway config — save as gateway.toml");
            println!("{}", GatewayConfig::template());
        }
        Cmd::Run { config } => {
            run(&config).await?;
        }
    }
    Ok(())
}

pub async fn run(config_path: &str) -> anyhow::Result<()> {
    let cfg = GatewayConfig::from_file(config_path)?;
    let auth = AuthVerifier::new(
        cfg.auth.enabled,
        &cfg.auth.ca_id,
        &cfg.auth.ca_pubkey_b64,
    )
    .map_err(|e| anyhow::anyhow!("invalid [auth] config: {e}"))?;
    if auth.enabled {
        tracing::info!("gateway intake permits pinned to CA {:?}", auth.ca_id);
    } else {
        tracing::warn!("gateway [auth].enabled=false: /register is OPEN (demo mode)");
    }
    let outbox = Outbox::open(&cfg.outbox.db)?;
    let ledger = Arc::new(LedgerClient::new(
        cfg.ledger.nodes.clone(),
        cfg.ledger.submit_timeout_ms,
        cfg.ledger.query_timeout_ms,
    ));
    let state = Arc::new(api::AppState {
        outbox,
        ledger,
        retry_batch: cfg.outbox.retry_batch,
        auth,
        auditor_sub: cfg.logs.auditor_sub.clone(),
        logs_enabled: cfg.logs.enabled,
    });
    if cfg.logs.enabled {
        tracing::info!(
            "gateway auditor logs at GET /logs (subject {:?})",
            cfg.logs.auditor_sub
        );
    } else {
        tracing::warn!("gateway [logs].enabled=false: GET /logs is disabled");
    }
    tokio::spawn(api::outbox_worker(
        state.clone(),
        std::time::Duration::from_millis(cfg.outbox.retry_interval_ms.max(250)),
    ));
    let app = api::router(state);
    let listener = tokio::net::TcpListener::bind(&cfg.server.listen).await?;
    tracing::info!("sagex-gateway listening on {}", cfg.server.listen);
    axum::serve(listener, app).await?;
    Ok(())
}
