// TODO: https://github.com/tokio-rs/tracing/issues/843
#![allow(clippy::unit_arg)]

use anyhow::{Context, Result};

use hpos_configure_holochain::{install_happs, load_happs_yaml, Config};
use tracing::instrument;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    run().await
}

#[instrument(err)]
async fn run() -> Result<()> {
    let config = Config::load();
    let happ_list =
        load_happs_yaml(&config.happ_list_path).context("failed to load hApps YAML config")?;
    install_happs(&happ_list, &config).await?;
    Ok(())
}
