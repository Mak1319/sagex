use anyhow::{Context, Result};
use sagex_nodeblock::{config::Config, consensus::Consensus, http, store::Store};
use std::{env, path::PathBuf, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let first = args.next();
    if first.as_deref() == Some("--public-key-hex") {
        let seed = args.next().context("usage: sagex-nodeblock --public-key-hex <seed-hex>")?;
        println!("{}", Consensus::public_key_for_seed(&seed)?);
        return Ok(());
    }
    let path = match first {
        Some(arg) if arg == "--config" => PathBuf::from(args.next().context("--config requires a path")?),
        Some(arg) => PathBuf::from(arg),
        None => PathBuf::from("config.toml"),
    };
    let config = Config::load(&path).context("load nodeblock TOML config")?;
    let store = Arc::new(Store::open(&config.database.path)?);
    http::serve(config, store).await
}
