use anyhow::Result;
use holo_happ_manager::{self, Config};
use tracing::instrument;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::from_default_env().add_directive("again=trace".parse().unwrap());
    tracing_subscriber::fmt().with_env_filter(filter).init();
    spawn().await
}

#[instrument(err)]
async fn spawn() -> Result<()> {
    let config = Config::load();
    holo_happ_manager::run(&config).await
}
