use std::path::PathBuf;

use holochain_types::app::{InstalledAppId, MembraneProof};
use serde::Deserialize;
use structopt::StructOpt;
use tracing::debug;
use url::Url;

#[derive(Debug, StructOpt)]
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
    /// Path to a YAML file containing the list of hApps to install
    pub happ_file_path: PathBuf,
}

impl Config {
    /// Create Config from CLI arguments with logging
    pub fn load() -> Self {
        let config = Config::from_args();
        debug!(?config, "loaded");
        config
    }
}

#[derive(Debug, Deserialize)]
pub struct HappFile {
    pub self_hosted_happs: Vec<Happ>,
    pub core_happs: Vec<Happ>,
}

/// Configuration of a single hApp from config.yaml
/// ui_path and dna_path takes precedence over ui_url and dna_url respectively
/// and is meant for running tests
#[derive(Debug, Deserialize)]
pub struct Happ {
    #[serde(alias = "app_id")]
    pub app_id: InstalledAppId,
    pub version: String,
    pub ui_url: Option<Url>,
    pub dna_url: Option<Url>,
    pub ui_path: Option<PathBuf>,
    pub dna_path: Option<PathBuf>,
    pub uuid: Option<String>,
    pub membrane_proof: Option<MembraneProof>
}

impl Happ {
    /// generates the installed app id that should be used
    /// based on the version and the uuid provided in the config fiel
    pub fn id_from_config(&self) -> String {
        if let Some(ref uuid) = self.uuid {
            format!("{}:{}:{}", self.app_id, self.version, uuid)
        } else {
            format!("{}:{}", self.app_id, self.version)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_install_app_id_format() {
        let mut cfg = Happ {
            app_id: "x".into(),
            version: String::from("1"),
            ui_url: None,
            dna_url: None,
            dna_path: None,
            ui_path: None,
            uuid: None,
        };
        assert_eq!(cfg.id_from_config(), String::from("x:1"));
        cfg.uuid = Some(String::from("001"));
        assert_eq!(cfg.id_from_config(), String::from("x:1:001"));
    }
}
