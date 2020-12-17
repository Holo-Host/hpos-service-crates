// TODO: https://github.com/tokio-rs/tracing/issues/843
#![allow(clippy::unit_arg)]

mod config;
pub use config::{Config, Happ, HappFile};

mod websocket;
pub use websocket::AdminWebsocket;

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
        serde_yaml::from_reader(&file).context("failed to deserialize YAML as Vec<Happ>")?;
    debug!(?happ_file);
    Ok(happ_file)
}

pub async fn install_happs(happ_file: &HappFile, config: &Config) -> Result<()> {
    let mut admin_websocket = AdminWebsocket::connect(config.admin_port)
        .await
        .context("failed to connect to holochain")?;

    if let Err(error) = admin_websocket.attach_app_interface(config.happ_port).await {
        warn!(port = ?config.happ_port, ?error, "failed to start app interface, maybe it's already up?");
    }

    let installed_happs = Arc::new(
        admin_websocket
            .list_installed_happs()
            .await
            .context("failed to get installed hApps")?,
    );

    let happs_to_install = happ_file
        .core_happs
        .iter()
        .chain(happ_file.self_hosted_happs.iter());

    let futures: Vec<_> = happs_to_install
        .map(|happ| {
            let mut admin_websocket = admin_websocket.clone();
            let installed_happs = Arc::clone(&installed_happs);
            async move {
                let install_ui = install_ui(happ, config);
                if installed_happs.contains(&happ.id_with_version()) {
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

    // Here websocket connection should be closed but ATM holochain_websocket does not provide this functionality

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
