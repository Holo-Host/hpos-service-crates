use anyhow::{Context, Result};
use holochain_conductor_api::CellInfo;
pub use holochain_types::prelude::CellId;
pub use hpos_hc_connect::AdminWebsocket;
pub use hpos_hc_connect::{
    holo_config::{Config, Happ, HappsFile, MembraneProofFile, ProofPayload},
    utils::{download_file, extract_zip},
};
use hpos_hc_connect::{hpos_agent::Agent, hpos_membrane_proof};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument, trace, warn};

mod utils;

pub mod jurisdictions;
use jurisdictions::HbsClient;

#[instrument(err, skip(config))]
pub async fn run(config: Config) -> Result<()> {
    debug!("Starting configure holochain...");

    let happ_file = HappsFile::load_happ_file(&config.happs_file_path)
        .context("failed to load hApps YAML config")?;
    install_happs(&happ_file, &config).await?;

    if let Err(e) = update_host_jurisdiction_if_changed(&config).await {
        warn!(
            "Note: This is only needed for holoports. Failed to update jurisdiction.  Error: {}",
            e
        );
    }

    Ok(())
}

/// based on the config file provided this installs the core apps on the holoport
/// It manages getting the mem-proofs and properties that are expected to be used on the holoport
pub async fn install_happs(happ_file: &HappsFile, config: &Config) -> Result<()> {
    let mut admin_websocket = AdminWebsocket::connect(config.admin_port)
        .await
        .context("failed to connect to holochain's admin interface")?;

    if let Err(error) = admin_websocket
        .attach_app_interface(Some(config.happ_port), None)
        .await
    {
        warn!(port = ?config.happ_port, ?error, "failed to start app interface for hosted happs, maybe it's already up?");
    }

    let agent = Agent::init(admin_websocket.clone()).await?;

    debug!("Agent key for all core happs {:?}", agent.admin.key);

    debug!("Getting a list of active happ");
    let active_happs = Arc::new(
        admin_websocket
            .list_enabled_apps()
            .await
            .context("failed to get active hApps")?,
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
                .install_and_activate_app(
                    happ,
                    Some(mem_proof_vec.clone()),
                    agent.clone(),
                    HashMap::new(),
                )
                .await
            {
                if err.to_string().contains("AppAlreadyInstalled") {
                    info!("app {} was previously installed, re-activating", &happ.id());
                    admin_websocket.activate_app(happ).await?;
                } else if err.to_string().contains("CellAlreadyExists")
                    && (happ.id().contains("core-app")
                        || happ.id().contains("holofuel")
                        || happ.id().contains("servicelogger"))
                {
                    // TODO: Revisit this exception with team - are there any blindspots of making this exception?
                    // Note: We currently will only encounter the case of a happ's `installed-app-id` being updated alongside changes that *do not* cause a DNA integrity zome change
                    // for our core-app, holofuel, and root servicelogger instance as they are the only apps to use the version number of the happ in their id;
                    // whereas all other hosted happs are forced to have their installed app id be their hha happ id
                    // ...and currently there is no way to update ONLY the coordinator zome without creating a new happ.
                    // ^^^^ TODO: Make it possible to update ONLY the coordinator for hosted happs too / add to cloud console flow.
                    warn!("cells for app {} already exist", &happ.id());

                    // // TEMPORARY HACK:
                    // // Until we have an integrated holochain solution to only upating the conductor zome,
                    // // we simply will uninstall the app that shares the cells and re-attempt installation.
                    // // This is NOT a permenant solution as it does not allow for data to persist.
                    // info!(
                    //     "de-activating prior app {} that uses shared cells",
                    //     &happ.id()
                    // );
                    // admin_websocket
                    //     .uninstall_app(<installed_app_id>, true)
                    //     .await?;

                    debug!("Getting a list of all installed happs");
                    let installed_apps = Arc::new(
                        admin_websocket
                            .list_apps(None)
                            .await
                            .context("failed to get installed hApps")?,
                    );

                    let get_existing_cells = move |id: &str| -> HashMap<String, CellId> {
                        let mut cells = HashMap::new();
                        let core_installed_app = installed_apps
                            .iter()
                            .find(|a| a.installed_app_id.contains(id));

                        if let Some(app_info) = core_installed_app {
                            for cell in &app_info.cell_info {
                                let cell_role_name = cell.0.clone();
                                let maybe_cell_info = cell
                                    .1
                                    .iter()
                                    .find(|i| matches!(i, CellInfo::Provisioned(_)));

                                if let Some(CellInfo::Provisioned(cell)) = maybe_cell_info {
                                    cells.insert(cell_role_name, cell.cell_id.clone());
                                }
                            }
                        };
                        cells
                    };

                    let existing_cells = if happ.id().contains("core-app") {
                        get_existing_cells("core-app")
                    } else if happ.id().contains("holofuel") {
                        get_existing_cells("holofuel")
                    } else {
                        get_existing_cells("servicelogger")
                    };

                    trace!("App has existing_cells : {:#?}", existing_cells);

                    return admin_websocket
                        .install_and_activate_app(
                            happ,
                            Some(mem_proof_vec),
                            agent.clone(),
                            existing_cells,
                        )
                        .await;
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
    let happs_to_keep: Vec<String> = happs_to_install.iter().map(|happ| happ.id()).collect();

    for app in &*active_happs {
        let installed_app_id = app.to_string();
        if !utils::keep_app_active(&installed_app_id, happs_to_keep.clone()) {
            info!("deactivating app {}", &installed_app_id);
            admin_websocket
                .uninstall_app(&installed_app_id, false)
                .await?;
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
    if let Ok(is_integration_test) = std::env::var("IS_INTEGRATION_TEST") {
        if is_integration_test == "TRUE" {
            // set in ../tests/integration.rs and ../../holo_happ_manager/tests/integration.ts
            return Ok(());
        }
    }

    // get current jurisdiction in hbs
    let hbs = HbsClient::connect().await?;
    let hbs_jurisdiction = hbs.get_host_registration().await?.jurisdiction;

    jurisdictions::update_jurisdiction_if_changed(config, hbs_jurisdiction).await
}
