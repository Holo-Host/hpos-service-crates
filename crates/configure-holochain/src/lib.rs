use anyhow::{Context, Result};
use holo_happ_manager;
pub use hpos_hc_connect::{
    holo_config::{Config, Happ, HappsFile, MembraneProofFile, ProofPayload},
    utils::{download_file, extract_zip},
};
use hpos_hc_connect::{hpos_agent::Agent, hpos_membrane_proof};
pub use hpos_hc_connect::{AdminWebsocket, AppWebsocket};
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};
pub mod hpos_holochain_api;
mod utils;

#[instrument(err, skip(config))]
pub async fn run(config: Config) -> Result<()> {
    debug!("Starting configure holochain...");
    let happ_file = HappsFile::load_happ_file(&config.happs_file_path)
        .context("failed to load hApps YAML config")?;
    install_happs(&happ_file, &config).await?;

    update_host_jurisdiction_if_changed(&config).await?;
    Ok(())
}

/// based on the config file provided this installs the core apps on the holoport
/// It manages getting the mem-proofs and properties that are expected to be used on the holoport
pub async fn install_happs(happ_file: &HappsFile, config: &Config) -> Result<()> {
    let mut admin_websocket = AdminWebsocket::connect(config.admin_port)
        .await
        .context("failed to connect to holochain's admin interface")?;

    if let Err(error) = admin_websocket.attach_app_interface(config.happ_port).await {
        warn!(port = ?config.happ_port, ?error, "failed to start app interface, maybe it's already up?");
    }

    let agent = Agent::init(admin_websocket.clone()).await?;

    debug!("Agent key for all core happs {:?}", agent.admin.key);

    debug!("Getting a list of active happ");
    let active_happs = Arc::new(
        admin_websocket
            .list_running_app()
            .await
            .context("failed to get installed hApps")?,
    );

    let happs_to_install: Vec<&Happ> = happ_file
        .core_happs
        .iter()
        .chain(happ_file.self_hosted_happs.iter())
        .collect();

    for happ in &happs_to_install {
        if active_happs.contains(&happ.id()) {
            info!("App {} already installed, just downloading UI", &happ.id());
        } else {
            info!("Installing app {}", &happ.id());
            let mem_proof_vec =
                hpos_membrane_proof::create_vec_for_happ(happ, agent.membrane_proof.clone())
                    .await?;

            if let Err(err) = admin_websocket
                .install_and_activate_happ(happ, mem_proof_vec, agent.clone())
                .await
            {
                if err.to_string().contains("AppAlreadyInstalled") {
                    info!("app {} was previously installed, re-activating", &happ.id());
                    admin_websocket.activate_happ(happ).await?;
                } else {
                    return Err(err);
                }
            }
        }
        install_ui(happ, config).await?
    }

    // Clean-up part of the script
    // This clean up will remove any old app that were installed by the old config file
    // This will also include removing happs that were installed with the old UID
    // This will leave old servicelogger instances and old hosted happs. (That should be cleaned by the holo-auto-installer service)
    let mut app_websocket = AppWebsocket::connect(config.happ_port)
        .await
        .context("failed to connect to holochain's app interface")?;

    let happs_to_keep: Vec<String> = happs_to_install.iter().map(|happ| happ.id()).collect();

    for app in &*active_happs {
        if let Some(app_info) = app_websocket.get_app_info(app.to_string()).await {
            if !utils::keep_app_active(&app_info.installed_app_id, happs_to_keep.clone()) {
                info!("deactivating app {}", app_info.installed_app_id);
                admin_websocket
                    .uninstall_app(&app_info.installed_app_id)
                    .await?;
            }
        }
    }

    info!("finished installing hApps");
    Ok(())
}

/// Install the UI based on the zip files that are provided in the config
#[instrument(err, skip(happ, config))]
async fn install_ui(happ: &Happ, config: &Config) -> Result<()> {
    let source_path = match happ.ui_path.clone() {
        Some(path) => path,
        None => {
            if happ.ui_url.is_none() {
                debug!("ui_url == None, skipping UI installation for {}", happ.id());
                return Ok(());
            }
            download_file(happ.ui_url.as_ref().unwrap())
                .await
                .context("failed to download UI archive")?
        }
    };
    if let Some(ui_home) = config.ui_store_folder.clone() {
        let unpack_path = ui_home.join(happ.ui_name());
        extract_zip(&source_path, &unpack_path).context("failed to extract UI archive")?;
        debug!("installed UI: {}", happ.id());
    }
    Ok(())
}

pub async fn update_host_jurisdiction_if_changed(config: &Config) -> Result<()> {
    if std::env::var("IS_INTEGRATION_TEST")? == "TRUE" { // set in ../tests/integration.rs and ../../holo_happ_manager/tests/integration.ts
        return Ok(())
    }

    // get current jurisdiction in hbs
    let hbs_jurisdiction = hpos_holochain_api::get_jurisdiction()
        .await
        .context("failed to get jurisdiction from hbs")?;

    holo_happ_manager::update_jurisdiction_if_changed(config, hbs_jurisdiction).await
}
