mod config;
pub use config::{Config, Happ};

mod websocket;
pub use websocket::AdminWebsocket;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use futures::future::try_join_all;
use tracing::{debug, info, instrument};
use url::Url;
use zip::ZipArchive;

#[instrument(err, fields(path = %path.as_ref().display()))]
pub fn load_happs_yaml(path: impl AsRef<Path>) -> Result<Vec<Happ>> {
    use std::fs::File;

    let file = File::open(path)?;
    let happ_list = serde_yaml::from_reader(&file).context("failed to read happ list")?;
    debug!(?happ_list);
    Ok(happ_list)
}

pub async fn install_happs(happ_list: &[Happ], config: &Config) -> Result<()> {
    let mut admin_websocket = AdminWebsocket::connect(config.admin_port).await?;
    let agent_key = admin_websocket.generate_agent_pubkey().await?;
    debug!(?agent_key);

    let futures: Vec<_> = happ_list
        .iter()
        .map(|happ| {
            let mut admin_websocket = admin_websocket.clone();
            let agent_key = agent_key.clone();
            let future = async move {
                let install_happ = admin_websocket.install_happ(happ, agent_key, config.happ_port);
                let install_ui = install_ui(happ, config);
                futures::try_join!(install_happ, install_ui)
            };
            future
        })
        .collect();
    let _: Vec<_> = try_join_all(futures).await?;

    info!("all hApps installed");
    Ok(())
}

#[instrument(skip(config), err)]
async fn install_ui(happ: &Happ, config: &Config) -> Result<()> {
    let ui_archive_path = download_file(&happ.ui_url).await?;
    let unpack_path = config.ui_store_folder.join(&happ.app_id);
    extract_zip(ui_archive_path, unpack_path)?;
    info!(?happ.app_id, "installed UI");
    Ok(())
}

#[instrument]
pub(crate) async fn download_file(url: &Url) -> Result<PathBuf> {
    use isahc::prelude::*;
    use tempfile::NamedTempFile;

    debug!("downloading");
    let mut url = Url::clone(&url);
    url.set_scheme("https")
        .map_err(|_| anyhow!("failed to set scheme to https"))?;
    let mut response = isahc::get_async(url.as_str()).await?;
    let mut file = NamedTempFile::new()?;
    response.copy_to(&mut file)?;

    let path = file.path().to_path_buf();
    debug!(path = %path.display(), "download successful");
    Ok(path)
}

#[instrument(err, fields(
    archive_path = %archive_path.as_ref().display(),
    unpack_path = %unpack_path.as_ref().display(),
))]
// HACK: This has no place in this crate. Well, at least we are cross-platform...
pub(crate) fn extract_zip<P>(archive_path: P, unpack_path: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let archive = fs::File::open(archive_path.as_ref()).context("failed to open archive file")?;
    let mut archive = ZipArchive::new(archive).context("failed to interpret file as archive")?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        debug!(name = %file.name());
        if !file.is_file() {
            debug!("not a regular file");
            continue;
        }
        let outpath = unpack_path.as_ref().join(file.name());
        if let Some(parent) = outpath.parent() {
            if !parent.exists() {
                debug!(path = %parent.display(), "ensuring parent directory exists");
                fs::create_dir_all(parent).context("failed to create parent directory")?;
            }
        }
        let mut outfile =
            fs::File::create(outpath.as_path()).context("failed to create destination file")?;
        io::copy(&mut file, &mut outfile).context("failed to copy file to destination")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(outpath.as_path(), fs::Permissions::from_mode(mode))
                    .context("failed to set Unix permissions")?;
            }
        }
    }
    Ok(())
}
