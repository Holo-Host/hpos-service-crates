use std::path::PathBuf;

use holochain_types::app::InstalledAppId;
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
    pub happ_list_path: PathBuf,
}

impl Config {
    /// Create Config from CLI arguments with logging
    pub fn load() -> Self {
        let config = Config::from_args();
        debug!(?config, "loaded");
        config
    }
}

/// Configuration of a single hApp from config.yaml
#[derive(Debug, Deserialize)]
pub struct Happ {
    #[serde(alias = "app_id")]
    pub app_id: InstalledAppId,
    pub version: String,
    pub ui_url: Option<Url>,
    pub dna_url: Option<Url>,
}

impl Happ {
    pub fn id_with_version(&self) -> String {
        format!("{}:{}", self.app_id, self.version)
    }
}
