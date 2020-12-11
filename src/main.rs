// TODO: https://github.com/tokio-rs/tracing/issues/843
#![allow(clippy::unit_arg)]

use anyhow::{Context, Result};

use configure_holochain::{install_happs, load_happs_yaml, Config};
use tracing::instrument;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::from_default_env().add_directive("again=trace".parse().unwrap());
    tracing_subscriber::fmt().with_env_filter(filter).init();
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
