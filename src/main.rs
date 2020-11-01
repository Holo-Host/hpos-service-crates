// TODO: https://github.com/tokio-rs/tracing/issues/843
#![allow(clippy::unit_arg)]

use anyhow::Result;

use self_hosted_happs::{install_happs, load_happs_yaml, Config};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    run().await
}

async fn run() -> Result<()> {
    let config = Config::load();
    let happ_list = load_happs_yaml(&config.happ_list_path)?;
    install_happs(&happ_list, &config).await?;
    Ok(())
}
