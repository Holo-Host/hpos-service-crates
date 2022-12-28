use anyhow::{Context, Result};
use holochain_types::prelude::AppBundleSource;
use holochain_types::{app::AppManifest, prelude::YamlProperties};
use serde::Deserialize;
use std::env;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tracing::{debug, instrument};
use url::Url;

pub const DEFAULT_PASSWORD: &str = "pass";

#[derive(Debug, Clone, StructOpt)]
pub struct Config {
    /// Holochain conductor port
    #[structopt(long, env, default_value = "4444")]
    pub admin_port: u16,
    /// hApp listening port
    #[structopt(long, env, default_value = "42233")]
    pub happ_port: u16,
    /// Path to the folder where hApp UIs will be extracted
    #[structopt(long, env)]
    pub ui_store_folder: PathBuf,
    /// Path to a YAML file containing the lists of hApps to install
    pub happs_file_path: PathBuf,
}

impl Config {
    /// Create Config from CLI arguments with logging
    pub fn load() -> Self {
        let config = Config::from_args();
        debug!(?config, "loaded");
        config
    }
}

/// MembraneProof payload contaiing cell_nick
#[derive(Debug, Deserialize)]
pub struct ProofPayload {
    pub cell_nick: String,
    /// Base64-encoded MembraneProof.
    pub proof: String,
}
/// payload vec of all the mem_proof for one happ
/// current implementation is implemented to contain mem_proof for elemental_chat
#[derive(Debug, Deserialize)]
pub struct MembraneProofFile {
    pub payload: Vec<ProofPayload>,
}

/// Configuration of a single hApp from config.yaml
/// ui_path and dna_path takes precedence over ui_url and dna_url respectively
/// and is meant for running tests
#[derive(Debug, Deserialize, Clone)]
pub struct Happ {
    pub ui_url: Option<Url>,
    pub ui_path: Option<PathBuf>,
    pub bundle_url: Option<Url>,
    pub bundle_path: Option<PathBuf>,
    pub dnas: Option<Vec<Dna>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Dna {
    pub role_name: String,
    pub properties: Option<String>,
}

impl Happ {
    /// returns the name that will be used to access the ui
    pub fn ui_name(&self) -> String {
        let mut name = self.id();
        if let Some(idx) = name.find(':') {
            name.truncate(idx);
        }
        name
    }
    /// generates the installed app id that should be used
    /// based on the path or url of the bundle.
    /// Assumes file name ends in .happ, and converts periods -> colons
    pub fn id(&self) -> String {
        let name = if let Some(ref bundle) = self.bundle_path {
            bundle
                .file_name()
                .unwrap()
                .to_os_string()
                .to_string_lossy()
                .to_string()
        } else if let Some(ref bundle) = self.bundle_url {
            bundle.path_segments().unwrap().last().unwrap().to_string()
        } else {
            //TODO fix
            "unreabable".to_string()
        };
        if let Ok(uid) = env::var("DEV_UID_OVERRIDE") {
            format!("{}::{}", name.replace(".happ", "").replace('.', ":"), uid)
        } else {
            name.replace(".happ", "").replace('.', ":")
        }
    }
    /// Downloads the happ bundle and returns its path
    pub async fn download(&self) -> Result<PathBuf> {
        match self.bundle_path.clone() {
            Some(path) => Ok(path),
            None => {
                let path = crate::utils::download_file(
                    self.bundle_url
                        .as_ref()
                        .context("bundle_url in happ is None")?,
                )
                .await?;
                Ok(path)
            }
        }
    }
    // get the source of the happ by retrieving the happ and updating the properties if any
    pub async fn source(&self) -> Result<AppBundleSource> {
        let path = self.download().await?;
        let mut source = AppBundleSource::Path(path);
        if self.dnas.is_some() {
            for dna in self.dnas.clone().unwrap().iter() {
                use mr_bundle::Bundle;
                let bundle = match source {
                    AppBundleSource::Bundle(bundle) => bundle.into_inner(),
                    AppBundleSource::Path(path) => Bundle::read_from_file(&path).await.unwrap(),
                };
                let AppManifest::V1(mut manifest) = bundle.manifest().clone();
                for role_manifest in &mut manifest.roles {
                    if &role_manifest.name == &dna.role_name {
                        // check for provided properties in the config file and apply if it exists
                        let mut properties: Option<YamlProperties> = None;
                        if let Some(p) = dna.properties.clone() {
                            let prop = p.to_string();
                            debug!("Core app Properties: {}", prop);
                            properties =
                                Some(YamlProperties::new(serde_yaml::from_str(&prop).unwrap()));
                        }
                        role_manifest.dna.modifiers.properties = properties
                    }
                }
                source = AppBundleSource::Bundle(
                    bundle
                        .update_manifest(AppManifest::V1(manifest))
                        .unwrap()
                        .into(),
                )
            }
        }
        Ok(source)
    }
}

/// hApps
#[derive(Debug, Deserialize)]
pub struct HappsFile {
    pub self_hosted_happs: Vec<Happ>,
    pub core_happs: Vec<Happ>,
}
impl HappsFile {
    #[instrument(err, fields(path = %path.as_ref().display()))]
    pub fn load_happ_file(path: impl AsRef<Path>) -> Result<HappsFile> {
        use std::fs::File;
        let file = File::open(path).context("failed to open file")?;
        let happ_file =
            serde_yaml::from_reader(&file).context("failed to deserialize YAML as HappsFile")?;
        debug!(?happ_file);
        Ok(happ_file)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_install_app_id_format() {
        let cfg = Happ {
            bundle_path: Some("my/path/to/elemental_chat.1.0001.happ".into()),
            bundle_url: None,
            ui_url: None,
            ui_path: None,
            dnas: None,
        };
        assert_eq!(cfg.id(), String::from("elemental_chat:1:0001"));
        let cfg = Happ {
            bundle_path: None,
            bundle_url: Some(Url::parse("https://github.com/holochain/elemental-chat/releases/download/v0.1.0-alpha1/elemental_chat.1.0001.happ").unwrap()),
            ui_url: None,
            ui_path: None,
            dnas: None,
        };
        assert_eq!(cfg.id(), String::from("elemental_chat:1:0001"));
    }

    #[test]
    fn verify_ui_name() {
        let cfg = Happ {
            bundle_path: Some("my/path/to/elemental_chat.1.0001.happ".into()),
            bundle_url: None,
            ui_url: None,
            ui_path: None,
            dnas: None,
        };
        assert_eq!(cfg.ui_name(), String::from("elemental_chat"));
    }
}
