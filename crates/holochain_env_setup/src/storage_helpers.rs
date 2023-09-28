use anyhow::{anyhow, Context, Result};
use lazy_static::*;
use reqwest::Client;
use std::path::PathBuf;
use std::{fs, io::prelude::*};
use tempfile::TempDir;
use tracing::debug;
use url::Url;

pub async fn download_file(url: &Url) -> Result<PathBuf> {
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

lazy_static! {
    static ref CLIENT: Client = Client::new();
}

/// Saves mem-proof to a file under MEM_PROOF_PATH
pub fn save_mem_proof_to_file(mem_proof: &str, path: &str) -> Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(mem_proof.as_bytes())
        .context(format!("Failed writing memproof to file {}", path))?;
    Ok(())
}
