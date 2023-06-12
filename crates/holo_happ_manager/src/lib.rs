mod get_my_apps;
use anyhow::{Context, Result};
pub use hpos_hc_connect::holo_config::{Config, Happ, HappsFile};
use tracing::{debug, info};
mod hha_type;
use hha_type::HappInput;
mod publish;
use std::{env, fs};

pub async fn run(core_happ: &Happ, config: &Config) -> Result<()> {
    info!("Running happ manager");
    let apps = happ_to_published()?;
    let list_of_published_happs = get_my_apps::published(core_happ, config).await?;
    for app in apps {
        if !list_of_published_happs
            .iter()
            .any(|a| a.bundle_url == app.bundle_url)
        {
            publish::publish_happ(core_happ, config, app).await?;
        } else {
            debug!("already published")
        }
    }

    Ok(())
}

pub fn happ_to_published() -> Result<Vec<HappInput>> {
    let apps_path = env::var("HOLO_PUBLISHED_HAPPS")
        .context("Failed to read HOLO_PUBLISHED_HAPPS. Is it set in env?")?;
    let app_json = fs::read(apps_path)?;
    let apps = serde_json::from_slice(&app_json)?;
    Ok(apps)
}
