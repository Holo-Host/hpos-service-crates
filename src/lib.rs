mod config;
use arbitrary::Arbitrary;
pub use config::{Config, Happ, HappsFile, MembraneProofFile, ProofPayload};
pub mod agent;
mod websocket;
use agent::{default_password, get_hpos_config, Agent};
use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AdminRequest, AppInfo, CellInfo};
use holochain_types::prelude::AgentPubKey;
use holochain_zome_types::{
    CapAccess, CapSecret, GrantZomeCallCapabilityPayload, GrantedFunctions, ZomeCallCapGrant,
};
use std::{collections::BTreeSet, env, sync::Arc};
use tracing::{debug, info, instrument, warn};
pub use websocket::{AdminWebsocket, AppWebsocket};
pub mod membrane_proof;
mod utils;

#[instrument(err, skip(config))]
pub async fn run(config: Config) -> Result<()> {
    debug!("Starting...");
    let happ_file = HappsFile::load_happ_file(&config.happs_file_path)
        .context("failed to load hApps YAML config")?;
    install_happs(&happ_file, &config).await?;
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
            let mem_proof_vec = crate::membrane_proof::create_vec_for_happ(
                &happ.id(),
                agent.membrane_proof.clone(),
            )
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
            // A forced agent key will require a UI pub key with an appropriate cap-secret for core-app
            if &env::var("FORCE_RANDOM_AGENT_KEY")
                .context("Failed to read FORCE_RANDOM_AGENT_KEY. Is it set in env?")?
                == "1"
                && happ.id().contains("core-app")
            {
                generate_ui_cap_secret(&admin_websocket, happ.id(), config.happ_port).await?;
            };
        }
        install_ui(happ, config).await?
    }

    // Clean-up part of the script
    let mut app_websocket = AppWebsocket::connect(config.happ_port)
        .await
        .context("failed to connect to holochain's app interface")?;

    let happs_to_keep: utils::HappIds = happs_to_install.iter().map(|happ| happ.id()).collect();

    for app in &*active_happs {
        if let Some(app_info) = app_websocket.get_app_info(app.to_string()).await {
            if !utils::keep_app_active(&app_info.installed_app_id, happs_to_keep.clone()) {
                info!("deactivating app {}", app_info.installed_app_id);
                admin_websocket
                    .deactivate_app(&app_info.installed_app_id)
                    .await?;
            }
        }
    }

    // Here all websocket connections should be closed but ATM holochain_websocket does not provide this functionality

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
            utils::download_file(happ.ui_url.as_ref().unwrap())
                .await
                .context("failed to download UI archive")?
        }
    };

    let unpack_path = config.ui_store_folder.join(&happ.ui_name());
    utils::extract_zip(&source_path, &unpack_path).context("failed to extract UI archive")?;
    debug!("installed UI: {}", happ.id());
    Ok(())
}

/// generates a UI_CAP_SECRET for the holoport to uses in develop
pub async fn generate_ui_cap_secret(
    admin_websocket: &AdminWebsocket,
    app_id: String,
    app_port: u16,
) -> Result<()> {
    let config = get_hpos_config()?;
    let pub_key =
        hpos_config_seed_bundle_explorer::holoport_public_key(&config, Some(default_password()?))
            .await?;
    let mut assignees = BTreeSet::new();
    assignees.insert(AgentPubKey::from_raw_32(pub_key.to_bytes().to_vec()));
    let mut app_ws = AppWebsocket::connect(app_port).await?;
    // This is an arbitrary secret
    let mut buf = arbitrary::Unstructured::new(&[0, 1, 6, 14, 26, 0]);
    let cap_secret = CapSecret::arbitrary(&mut buf).unwrap();

    let grant = match app_ws.get_app_info(app_id).await {
        Some(AppInfo { cell_info, .. }) => {
            let cell = match &cell_info.get("core-app").unwrap()[0] {
                CellInfo::Provisioned(c) => c.clone(),
                _ => return Err(anyhow!("core-app cell not found")),
            };

            GrantZomeCallCapabilityPayload {
                cell_id: cell.cell_id,
                cap_grant: ZomeCallCapGrant {
                    tag: "ui-grant".to_string(),
                    access: CapAccess::Assigned {
                        secret: cap_secret,
                        assignees,
                    },
                    functions: GrantedFunctions::All,
                },
            }
        }
        None => return Err(anyhow!("HHA is not installed")),
    };

    admin_websocket
        .clone()
        .send(AdminRequest::GrantZomeCallCapability(Box::new(grant)))
        .await?;

    Ok(())
}
