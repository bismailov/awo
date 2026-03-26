#![allow(unused_crate_dependencies)]

use anyhow::Result;
use awo_core::{AppCore, DaemonOptions, DaemonServer};
use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "awod", about = "Agent workspace orchestrator daemon")]
struct Args {
    /// Explicit path to the configuration directory.
    #[arg(long, env = "AWO_CONFIG_DIR")]
    config_dir: Option<PathBuf>,

    /// Explicit path to the data directory.
    #[arg(long, env = "AWO_DATA_DIR")]
    data_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    initialize_tracing()?;

    let args = Args::parse();

    let core = if let (Some(config_dir), Some(data_dir)) = (args.config_dir, args.data_dir) {
        AppCore::with_dirs(config_dir, data_dir)?
    } else {
        AppCore::bootstrap()?
    };

    let options = DaemonOptions::from_paths(core.paths());
    let server = DaemonServer::acquire(options)?;

    tracing::info!("awod starting");
    let mut core = core;
    server.run(&mut core)?;

    Ok(())
}

fn initialize_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .map_err(anyhow::Error::from)?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .try_init()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    Ok(())
}
