use std::sync::Arc;

use clap::{Parser, Subcommand};

use sagex_gateway::{api, config::GatewayConfig, LedgerClient, Outbox};

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
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
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
    });
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
