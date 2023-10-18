pub mod get_my_apps;
use anyhow::{anyhow, Context, Result};
use hha::HHAAgent;
use holochain_conductor_api::AppResponse;
use holochain_types::prelude::{AgentPubKey, ExternIO, FunctionName, ZomeName};
pub use hpos_hc_connect::holo_config::{Config, Happ, HappsFile};
use serde::Serialize;
use tracing::{debug, info};
mod hha_type;
use hha_type::HappInput;
mod publish;
use std::{env, fs, path::PathBuf};
pub mod hha;

pub async fn run(config: &Config) -> Result<()> {
    info!("Running happ manager");

    let core_happ: Happ = get_core_happ(&config.happs_file_path)?;

    let apps = happ_to_published()?;

    println!("Happs to be published {:?}", apps);

    let list_of_published_happs = get_my_apps::published(&core_happ, config).await?;
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
            publish::publish_happ(&core_happ, config, app).await?;
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

pub async fn update_jurisdiction_if_changed(
    config: &Config,
    hbs_jurisdiction: String,
) -> Result<()> {
    let core_happ: Happ = get_core_happ(&config.happs_file_path)?;

    let mut agent = HHAAgent::spawn(&core_happ, config).await?;

    let host_pubkey = agent.pubkey();

    let response = agent
        .zome_call(
            ZomeName::from("hha"),
            FunctionName::from("get_host_jurisdiction"),
            ExternIO::encode(host_pubkey.clone())?,
        )
        .await?;

    let hha_jurisdiction: Option<String> = match response {
        AppResponse::ZomeCalled(r) => rmp_serde::from_slice(r.as_bytes())?,
        _ => {
            return Err(anyhow!(
                "unexpected response from get_host_jurisdiction {:?}",
                response
            ))
        }
    };

    if hha_jurisdiction.is_none() || hha_jurisdiction.as_ref() != Some(&hbs_jurisdiction) {
        #[derive(Debug, Serialize)]
        pub struct SetHostJurisdictionInput {
            pub host_pubkey: AgentPubKey,
            pub jurisdiction: String,
        }

        agent
            .zome_call(
                ZomeName::from("hha"),
                FunctionName::from("set_host_jurisdiction"),
                ExternIO::encode(SetHostJurisdictionInput {
                    host_pubkey,
                    jurisdiction: hbs_jurisdiction,
                })?,
            )
            .await?;
    }

    Ok(())
}
