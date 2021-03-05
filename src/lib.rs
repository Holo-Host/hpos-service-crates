// TODO: https://github.com/tokio-rs/tracing/issues/843
#![allow(clippy::unit_arg)]

mod config;
pub use config::{Config, Happ, HappFile};

mod entries;
pub use entries::{Body, DnaResource, HappBundle, HappBundleDetails, Preferences};

mod websocket;
pub use websocket::{AdminWebsocket, AppWebsocket};

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use tempfile::TempDir;
use tracing::{debug, info, instrument, warn};
use url::Url;

use hc_utils::WrappedHeaderHash;
use holochain::conductor::api::AppResponse;
use holochain::conductor::api::ZomeCall;
use holochain_zome_types::{
    zome::{FunctionName, ZomeName},
    zome_io::ExternInput,
    SerializedBytes,
};
type HappIds = Vec<String>;

pub async fn activate_holo_hosted_happs(core_happ: Happ) -> Result<()> {
    let list_of_happs: Vec<WrappedHeaderHash> = get_enabled_hosted_happs(core_happ).await?;
    info!("got back list_of_happs {:?}", list_of_happs);
    install_holo_hosted_happs(list_of_happs).await?;
    Ok(())
}

pub async fn install_holo_hosted_happs(happs: Vec<WrappedHeaderHash>) -> Result<()> {
    info!("Starting to install....");
    // iterate through the vec and
    // Call http://localhost/hpos-holochain-api/install_hosted_happ
    // for each WrappedHeaderHash to install the hosted_happ
    let client = reqwest::Client::new();
    // Note: Tmp preferences
    let preferences = Preferences {
        max_fuel_before_invoice: 1.0,
        max_time_before_invoice: vec![86400, 0],
        price_compute: 1.0,
        price_storage: 1.0,
        price_bandwidth: 1.0,
    };
    for happ_id in happs {
        info!("Installing happ-id {:?}", happ_id);
        let body = Body {
            happ_id: happ_id.0.to_string(),
            preferences: preferences.clone(),
        };
        let response = client
            .post("http://localhost/hpos-holochain-api/install_hosted_happ")
            .json(&body)
            .send()
            .await?;
        info!("Installed happ-id {:?}", happ_id);
        info!("Response {:?}", response);
    }
    Ok(())
}

pub async fn get_enabled_hosted_happs(core_happ: Happ) -> Result<Vec<WrappedHeaderHash>> {
    let mut app_websocket = AppWebsocket::connect(42233)
        .await
        .context("failed to connect to holochain's app interface")?;
    match app_websocket.get_app_info(core_happ.id_from_config()).await {
        Some(app_info) => {
            let zome_call_payload = ZomeCall {
                cell_id: app_info.cell_data[0].clone().into_id(), // This works on the assumption that the core happs has HHA in the first position of the vec
                zome_name: ZomeName::from("hha"),
                fn_name: FunctionName::from("get_happs"),
                payload: ExternInput::new(SerializedBytes::default()),
                cap: None,
                provenance: app_info.cell_data[0]
                    .clone()
                    .into_id()
                    .agent_pubkey()
                    .to_owned(),
            };
            let response = app_websocket.zome_call(zome_call_payload).await?;
            match response {
                // This is the happs list that is returned from the hha DNA
                // https://github.com/Holo-Host/holo-hosting-app-rsm/blob/develop/zomes/hha/src/lib.rs#L54
                // return Vec of happ_list.happ_id
                AppResponse::ZomeCall(r) => {
                    info!("ZomeCall Response - Hosted happs List {:?}", r);
                    let happ_bundles: Vec<HappBundleDetails> =
                        rmp_serde::from_read_ref(r.into_inner().bytes())?;
                    let happ_bundle_ids: Vec<WrappedHeaderHash> =
                        happ_bundles.into_iter().map(|happ| happ.happ_id).collect();
                    info!("List of happ_ids {:?}", happ_bundle_ids);
                    return Ok(happ_bundle_ids);
                }
                _ => return Err(anyhow!("unexpected response: {:?}", response)),
            }
        }
        None => {
            return Err(anyhow!("HHA is not installed"));
        }
    }
}

#[instrument(err, fields(path = %path.as_ref().display()))]
pub fn load_happ_file(path: impl AsRef<Path>) -> Result<HappFile> {
    use std::fs::File;

    let file = File::open(path).context("failed to open file")?;
    let happ_file =
        serde_yaml::from_reader(&file).context("failed to deserialize YAML as HappFile")?;
    debug!(?happ_file);
    Ok(happ_file)
}

pub async fn install_happs(happ_file: &HappFile, config: &Config) -> Result<()> {
    let mut admin_websocket = AdminWebsocket::connect(config.admin_port)
        .await
        .context("failed to connect to holochain's admin interface")?;

    if let Err(error) = admin_websocket.attach_app_interface(config.happ_port).await {
        warn!(port = ?config.happ_port, ?error, "failed to start app interface, maybe it's already up?");
    }

    let active_happs = Arc::new(
        admin_websocket
            .list_active_happs()
            .await
            .context("failed to get installed hApps")?,
    );

    let happs_to_install: Vec<&Happ> = happ_file
        .core_happs
        .iter()
        .chain(happ_file.self_hosted_happs.iter())
        .collect();

    // This line makes sure agent key gets created and stored before all the async stuff starts
    let mut agent_websocket = admin_websocket.clone();
    let _ = agent_websocket.get_agent_key().await?;

    for happ in &happs_to_install {
        let full_happ_id = &happ.id_from_config();
        if active_happs.contains(full_happ_id) {
            info!(
                "App {} already installed, just downloading UI",
                full_happ_id
            );
            install_ui(happ, config).await?
        } else {
            info!("Installing app {}", full_happ_id);
            if let Err(err) = admin_websocket.install_happ(happ).await {
                if err.to_string().contains("AppAlreadyInstalled") {
                    info!(
                        "app {} was previously installed, re-activating",
                        full_happ_id
                    );
                    admin_websocket.activate_happ(happ).await?;
                } else {
                    return Err(err);
                }
            }
            install_ui(happ, config).await?;
        }
    }

    // Clean-up part of the script
    let mut app_websocket = AppWebsocket::connect(config.happ_port)
        .await
        .context("failed to connect to holochain's app interface")?;

    let happs_to_keep: HappIds = happs_to_install
        .iter()
        .map(|happ| happ.id_from_config())
        .collect();

    for app in &*active_happs {
        if let Some(app_info) = app_websocket.get_app_info(app.to_string()).await {
            if !keep_app_active(&app_info.installed_app_id, happs_to_keep.clone()) {
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

#[instrument(
    err,
    skip(happ, config),
    fields(?happ.app_id)
)]
async fn install_ui(happ: &Happ, config: &Config) -> Result<()> {
    let source_path = match happ.ui_path.clone() {
        Some(path) => path,
        None => {
            if happ.ui_url.is_none() {
                debug!(?happ.app_id, "ui_url == None, skipping UI installation");
                return Ok(());
            }
            download_file(happ.ui_url.as_ref().unwrap())
                .await
                .context("failed to download UI archive")?
        }
    };

    let unpack_path = config.ui_store_folder.join(&happ.app_id);
    extract_zip(&source_path, &unpack_path).context("failed to extract UI archive")?;
    info!(?happ.app_id, "installed UI");
    Ok(())
}

#[instrument(err, skip(url))]
pub(crate) async fn download_file(url: &Url) -> Result<PathBuf> {
    use isahc::config::RedirectPolicy;
    use isahc::prelude::*;

    debug!("downloading");
    let mut url = Url::clone(&url);
    url.set_scheme("https")
        .map_err(|_| anyhow!("failed to set scheme to https"))?;
    let client = HttpClient::builder()
        .redirect_policy(RedirectPolicy::Follow)
        .build()
        .context("failed to initiate download request")?;
    let mut response = client
        .get(url.as_str())
        .context("failed to send GET request")?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "response status code {} indicated failure",
            response.status().as_str()
        ));
    }
    let dir = TempDir::new().context("failed to create tempdir")?;
    let url_path = PathBuf::from(url.path());
    let basename = url_path
        .file_name()
        .context("failed to get basename from url")?;
    let path = dir.into_path().join(basename);
    let mut file = fs::File::create(&path).context("failed to create target file")?;
    response
        .copy_to(&mut file)
        .context("failed to write response to file")?;
    debug!("download successful");
    Ok(path)
}

#[instrument(
    err,
    fields(
        source_path = %source_path.as_ref().display(),
        unpack_path = %unpack_path.as_ref().display(),
    ),
)]
pub(crate) fn extract_zip<P: AsRef<Path>>(source_path: P, unpack_path: P) -> Result<()> {
    let _ = fs::remove_dir_all(unpack_path.as_ref());
    fs::create_dir(unpack_path.as_ref()).context("failed to create empty unpack_path")?;

    debug!("unziping file");

    let output = process::Command::new("unzip")
        .arg(source_path.as_ref().as_os_str())
        .arg("-d")
        .arg(unpack_path.as_ref().as_os_str())
        .stdout(process::Stdio::piped())
        .output()
        .context("failed to spawn unzip command")?;

    debug!("{}", String::from_utf8_lossy(&output.stdout));

    Ok(())
}

// Returns true if app should be kept active in holochain
fn keep_app_active(installed_app_id: &str, happs_to_keep: HappIds) -> bool {
    happs_to_keep.contains(&installed_app_id.to_string()) || installed_app_id.contains("uhCkk")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_keep_app_active() {
        let happs_to_keep = vec!["elemental-chat:2".to_string(), "hha:1".to_string()];
        let app_1 = "elemental-chat:1";
        let app_2 = "elemental-chat:2";
        let app_3 = "uhCkkcF0X1dpwHFeIPI6-7rzM6ma9IgyiqD-othxgENSkL1So1Slt::servicelogger";
        let app_4 = "other-app";

        assert_eq!(keep_app_active(app_1, happs_to_keep.clone()), false);
        assert_eq!(keep_app_active(app_2, happs_to_keep.clone()), true); // because it is in config
        assert_eq!(keep_app_active(app_3, happs_to_keep.clone()), true); // because it is hosted
        assert_eq!(keep_app_active(app_4, happs_to_keep.clone()), false);
    }
}
