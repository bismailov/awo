#![allow(unused_crate_dependencies)]

use anyhow::Result;
use awo_core::{AppCore, DaemonOptions, DaemonServer};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    initialize_tracing()?;

    let core = AppCore::bootstrap()?;
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
        .with_target(false)
        .compact()
        .try_init()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    Ok(())
}
