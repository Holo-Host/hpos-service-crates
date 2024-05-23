use anyhow::{anyhow, Context, Result};
use hha::HHAAgent;
pub use hpos_hc_connect::{
    hha_types::HappInput,
    holo_config::{Config, Happ, HappsFile},
};
use hpos_hc_connect::{holo_config::ADMIN_PORT, AdminWebsocket};
use std::{env, fs, path::PathBuf};
use tracing::{debug, info};

pub mod hha;

pub async fn run(config: &Config) -> Result<()> {
    info!("Running happ manager");

    let core_happ: Happ = get_core_happ(&config.happs_file_path)?;

    let mut admin_ws = AdminWebsocket::connect(ADMIN_PORT)
        .await
        .context("failed to connect to holochain's app interface")?;

    let mut agent = HHAAgent::spawn(&core_happ, config, &mut admin_ws).await?;

    let apps = happ_to_published()?;

    println!("Happs to be published {:?}", apps);

    let list_of_published_happs = agent.get_my_happs().await?;

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
            // This special_installed_app_id is designed for Cloud Console(formally know as Publisher Portal)
            if app.name.contains("Cloud") {
                app.special_installed_app_id = Some(core_happ.id())
            }
            agent.publish_happ(app).await?;
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

fn get_core_happ(happs_file_path: &PathBuf) -> Result<Happ> {
    let happ_file =
        HappsFile::load_happ_file(happs_file_path).context("failed to load hApps YAML config")?;
    let core_happ = happ_file.core_app().ok_or_else(|| {
        anyhow!(
        "Please check that the happ config file is present. No Core apps found in configuration"
    )
    })?;
    Ok(core_happ)
}
