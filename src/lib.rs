// TODO: https://github.com/tokio-rs/tracing/issues/843
#![allow(clippy::unit_arg)]

mod config;
pub use config::{Config, Happ, HappFile};

mod websocket;
use holochain_types::app::InstalledCell;
use holochain_types::dna::AgentPubKey;
pub use websocket::{AdminWebsocket, AppWebsocket};

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use futures::future;
use tempfile::TempDir;
use tracing::{debug, info, instrument, warn};
use url::Url;

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

    let happs_to_install = happ_file
        .core_happs
        .iter()
        .chain(happ_file.self_hosted_happs.iter());

    // This line makes sure agent key gets created and stored before all the async stuff starts
    let mut agent_websocket = admin_websocket.clone();
    let agent_key = agent_websocket.get_agent_key().await?;

    let futures: Vec<_> = happs_to_install.clone()
        .map(|happ| {
            let mut admin_websocket = admin_websocket.clone();
            let active_happs = Arc::clone(&active_happs);
            async move {
                let install_ui = install_ui(happ, config);
                if active_happs.contains(&happ.id_with_version()) {
                    info!(?happ.app_id, "already installed, just downloading UI");
                    install_ui.await
                } else {
                    let install_happ = admin_websocket.install_happ(happ);
                    futures::try_join!(install_happ, install_ui).map(|_| ())
                }
            }
        })
        .collect();
    let _: Vec<_> = future::join_all(futures).await;

    // Clean-up part of the script
    let mut app_websocket = AppWebsocket::connect(config.happ_port)
        .await
        .context("failed to connect to holochain's app interface")?;

    let happs_to_keep = happs_to_install.map(|happ| {
        happ.id_with_version()
    }).collect();

    for app in &*active_happs {
        if let Some(app_info) = app_websocket.get_app_info(app.to_string()).await {
            if is_agent_owner(app_info.cell_data, agent_key.clone())
                && !is_listed_in_config(&app_info.installed_app_id, &happs_to_keep)
            {
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
    if happ.ui_url.is_none() {
        debug!(?happ.app_id, "ui_url == None, skipping UI installation");
        return Ok(());
    }

    let source_path = download_file(happ.ui_url.as_ref().unwrap())
        .await
        .context("failed to download UI archive")?;
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

// Returns true if cell was installed by agent
fn is_agent_owner(cell_data: Vec<InstalledCell>, key: AgentPubKey) -> bool {
    let mut result = false;
    for cell in cell_data {
        result = result || cell.into_id().agent_pubkey() == &key;
    }
    result
}

// Returns true if app is listed in happs_to_keep
fn is_listed_in_config(installed_app_id: &str, happs_to_keep: &Vec<String>) -> bool {
    happs_to_keep.contains(&installed_app_id.to_string())
}
