// TODO: https://github.com/tokio-rs/tracing/issues/843
#![allow(clippy::unit_arg)]

mod config;
pub use config::{Config, Happ};

mod websocket;
pub use websocket::AdminWebsocket;

use std::fs;
use std::io;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use futures::{future::try_join_all, prelude::*};
use tempfile::NamedTempFile;
use tracing::{debug, info, instrument, trace};
use url::Url;
use zip::ZipArchive;

#[instrument(err, fields(path = %path.as_ref().display()))]
pub fn load_happs_yaml(path: impl AsRef<Path>) -> Result<Vec<Happ>> {
    use std::fs::File;

    let file = File::open(path).context("failed to open file")?;
    let happ_list =
        serde_yaml::from_reader(&file).context("failed to deserialize YAML as Vec<Happ>")?;
    debug!(?happ_list);
    Ok(happ_list)
}

pub async fn install_happs(happ_list: &[Happ], config: &Config) -> Result<()> {
    let admin_websocket = AdminWebsocket::connect(config.admin_port)
        .await
        .context("failed to connect to holochain")?;
    let futures: Vec<_> = happ_list
        .iter()
        .map(|happ| {
            let mut admin_websocket = admin_websocket.clone();
            async move {
                let mut agent_websocket = admin_websocket.clone();
                let install_happ = agent_websocket
                    .generate_agent_pubkey()
                    .and_then(|agent_key| {
                        admin_websocket.install_happ(happ, agent_key, config.happ_port)
                    });
                let install_ui = install_ui(happ, config);
                futures::try_join!(install_happ, install_ui)
            }
        })
        .collect();
    let _: Vec<_> = try_join_all(futures)
        .await
        .context("failed to install some hApps")?;

    info!("all hApps installed");
    Ok(())
}

#[instrument(
    err,
    skip(happ, config),
    fields(?happ.app_id)
)]
async fn install_ui(happ: &Happ, config: &Config) -> Result<()> {
    let mut ui_archive = download_file(&happ.ui_url)
        .await
        .context("failed to download UI archive")?;
    let unpack_path = config.ui_store_folder.join(&happ.app_id);
    extract_zip(ui_archive.as_file_mut(), unpack_path).context("failed to extract UI archive")?;
    info!(?happ.app_id, "installed UI");
    Ok(())
}

#[instrument]
pub(crate) async fn download_file(url: &Url) -> Result<NamedTempFile> {
    use isahc::prelude::*;

    debug!("downloading");
    let mut url = Url::clone(&url);
    url.set_scheme("https")
        .map_err(|_| anyhow!("failed to set scheme to https"))?;
    let mut response = isahc::get_async(url.as_str())
        .await
        .context("failed to send GET request")?;
    let mut file = NamedTempFile::new().context("failed to create tempfile")?;
    response
        .copy_to(&mut file)
        .context("failed to write response to file")?;
    debug!("download successful");
    Ok(file)
}

#[instrument(
    err,
    skip(archive),
    fields(unpack_path = %unpack_path.as_ref().display()),
)]
// HACK: This has no place in this crate. Well, at least we are cross-platform...
pub(crate) fn extract_zip(archive: &mut fs::File, unpack_path: impl AsRef<Path>) -> Result<()> {
    fs::remove_dir_all(unpack_path.as_ref()).context("failed to remove unpack_path")?;
    fs::create_dir(unpack_path.as_ref()).context("failed to create empty unpack_path")?;

    let mut archive = ZipArchive::new(archive).context("failed to interpret file as archive")?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        trace!(name = %file.name());
        if !file.is_file() {
            trace!("not a regular file");
            continue;
        }
        let outpath = unpack_path.as_ref().join(file.name());
        if let Some(parent) = outpath.parent() {
            if !parent.exists() {
                trace!(path = %parent.display(), "ensuring parent directory exists");
                fs::create_dir_all(parent).context("failed to create parent directory")?;
            }
        }
        let mut outfile =
            fs::File::create(outpath.as_path()).context("failed to create destination file")?;
        io::copy(&mut file, &mut outfile).context("failed to copy file to destination")?;
    }
    Ok(())
}
