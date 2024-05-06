use anyhow::{anyhow, Context, Result};
use holochain_types::prelude::{SerializedBytes, SerializedBytesError};
use holochain_types::prelude::{Nonce256Bits, Timestamp};
use holochain_websocket::WebsocketReceiver;
use lair_keystore_api::dependencies::tokio;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tracing::{debug, instrument};
use url::Url;

/// generates nonce for zome calls
pub fn fresh_nonce() -> Result<(Nonce256Bits, Timestamp)> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes)?;
    let nonce = Nonce256Bits::from(bytes);
    // Rather arbitrary but we expire nonces after 5 mins.
    let expires: Timestamp = (Timestamp::now() + Duration::from_secs(60 * 5))?;
    Ok((nonce, expires))
}

#[instrument(
    err,
    fields(
        source_path = %source_path.as_ref().display(),
        unpack_path = %unpack_path.as_ref().display(),
    ),
)]
pub fn extract_zip<P: AsRef<Path>>(source_path: P, unpack_path: P) -> Result<()> {
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

#[instrument(err, skip(url))]
pub async fn download_file(url: &Url) -> Result<PathBuf> {
    use isahc::config::Configurable;
    use isahc::config::RedirectPolicy;
    use isahc::prelude::*;
    use isahc::HttpClient;

    let path = if url.scheme() == "file" {
        let p = PathBuf::from(url.path());
        debug!("Using: {:?}", p);
        p
    } else {
        debug!("downloading");
        let mut url = Url::clone(url);
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
        path
    };
    Ok(path)
}

/// You do not need to do anything with this type. While it is held it will keep polling a websocket
/// receiver.
pub struct WsPollRecv(tokio::task::JoinHandle<()>);

impl Drop for WsPollRecv {
    fn drop(&mut self) {
        tracing::info!("Poll dropping");
        self.0.abort();
    }
}

impl WsPollRecv {
    /// Create a new [WsPollRecv] that will poll the given [WebsocketReceiver] for messages.
    /// The type of the messages being received must be specified. For example
    ///
    /// ```no_run
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()>
    /// # {
    ///
    /// use holochain::sweettest::{websocket_client_by_port, WsPollRecv};
    /// use holochain_conductor_api::AdminResponse;
    ///
    /// let (tx, rx) = websocket_client_by_port(3000).await?;
    /// let _rx = WsPollRecv::new::<AdminResponse>(rx);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<D>(mut rx: WebsocketReceiver) -> Self
    where
        D: std::fmt::Debug,
        SerializedBytes: TryInto<D, Error = SerializedBytesError>,
    {
        Self(tokio::task::spawn(async move {
            while rx.recv::<D>().await.is_ok() {}
        }).into())
    }
}

