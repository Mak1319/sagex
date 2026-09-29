use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sagex-ledger-node", about = "PBFT blockchain node (TCP JSON-lines, SQLite)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate node keys and print a config template.
    Init {
        #[arg(long)]
        id: u64,
        #[arg(long, default_value = "127.0.0.1:7000")]
        listen: String,
        #[arg(long, default_value = "ledger.db")]
        db: String,
    },
    /// Run a node from a TOML config file.
    Run {
        #[arg(long)]
        config: String,
    },
    /// Verify the SQLite hash chain of a db file.
    Verify {
        #[arg(long)]
        db: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init { id, listen, db } => {
            let ident = sagex_ledger::identity::Identity::generate(id);
            println!("# sagex-ledger node config — save as config/nodes/node{id}.toml");
            println!("[node]");
            println!("id = {id}");
            println!("listen = \"{listen}\"");
            println!("db = \"{db}\"");
            println!("secret_key = \"{}\"", ident.secret_b64());
            println!("pubkey = \"{}\"", ident.public_b64);
            println!("\n[consensus]\nf = 1\nview_timeout_ms = 8000");
            println!("\n# peers: fill with the OTHER nodes' id/addr/pubkey");
            println!("[[peers]]\nid = 99\naddr = \"127.0.0.1:7999\"\npubkey = \"...\"");
        }
        Cmd::Run { config } => {
            sagex_ledger::node::run_node(&config).await?;
        }
        Cmd::Verify { db } => {
            let s = sagex_ledger::Store::open(&db)?;
            let n = s.verify_chain()?;
            println!("chain OK: {n} blocks in {db}");
        }
    }
    Ok(())
}
