use std::{env, fs};

use anyhow::{Context, Result};
use hpos_hc_connect::hha_agent::CoreAppAgent;
pub use hpos_hc_connect::{
    hha_types::HappInput,
    holo_config::{Config, Happ, HappsFile},
};
use tracing::{debug, info};

pub async fn run(config: &Config) -> Result<()> {
    info!("Running happ manager");

    let mut hha = CoreAppAgent::spawn(Some(config)).await?;

    let apps = happ_to_be_published()?;

    println!("Happs to be published {:?}", apps);

    let list_of_published_happs = hha.get_my_happs().await?;

    println!(
        "Happs that are already published {:?}",
        list_of_published_happs
    );
    for mut app in apps {
        if !list_of_published_happs
            .iter()
            .any(|a| a.bundle_url == app.bundle_url)
        {
            // Check if the name is "cloud console"
            // if it does set a special_installed_app_id as the installed_app_id of the core_app
            // This special_installed_app_id is designed for Cloud Console or Holofuel
            if app.name.contains("Cloud") || app.name.contains("Holofuel") {
                app.special_installed_app_id = Some(hha.id())
            }
            hha.publish_happ(app).await?;
        } else {
            debug!("already published")
        }
    }

    Ok(())
}

pub fn happ_to_be_published() -> Result<Vec<HappInput>> {
    let apps_path = env::var("HOLO_PUBLISHED_HAPPS")
        .context("Failed to read HOLO_PUBLISHED_HAPPS. Is it set in env?")?;
    let app_json = fs::read(apps_path)?;
    let apps = serde_json::from_slice(&app_json)?;
    Ok(apps)
}
