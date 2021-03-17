// TODO: https://github.com/tokio-rs/tracing/issues/843
#![allow(clippy::unit_arg)]

mod config;
pub use config::{Config, Happ, HappsFile};

mod websocket;
pub use websocket::{AdminWebsocket, AppWebsocket};

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use tempfile::TempDir;
use tracing::{debug, info, instrument, warn};
use url::Url;

type HappIds = Vec<String>;

#[instrument(err, fields(path = %path.as_ref().display()))]
pub fn load_happ_file(path: impl AsRef<Path>) -> Result<HappsFile> {
    use std::fs::File;

    let file = File::open(path).context("failed to open file")?;
    let happ_file =
        serde_yaml::from_reader(&file).context("failed to deserialize YAML as HappsFile")?;
    debug!(?happ_file);
    Ok(happ_file)
}

pub async fn install_happs(happ_file: &HappsFile, config: &Config) -> Result<()> {
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
        let full_happ_id = happ.id();
        if active_happs.contains(&full_happ_id) {
            info!(
                "App {} already installed, just downloading UI",
                full_happ_id
            );
            install_ui(happ, config).await?
        } else {
            info!("Installing app {}", full_happ_id);
            if let Err(err) = admin_websocket
                .install_and_activate_happ(happ, HashMap::new())
                .await
            {
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

    let happs_to_keep: HappIds = happs_to_install.iter().map(|happ| happ.id()).collect();

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

    let unpack_path = config.ui_store_folder.join(&happ.id());
    extract_zip(&source_path, &unpack_path).context("failed to extract UI archive")?;
    info!("installed UI: {}", happ.id());
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
