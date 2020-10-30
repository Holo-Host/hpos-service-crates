use std::path::PathBuf;

use holochain_types::app::AppId;
use serde::Deserialize;
use structopt::StructOpt;
use tracing::info;
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
    #[structopt(long, env, default_value = "/var/lib/self-hosted-happs/uis")]
    pub ui_store_folder: PathBuf,
    /// Path to a YAML file containing the list of hApps to install
    pub happ_list_path: PathBuf,
}

impl Config {
    /// Create Config from CLI arguments with logging
    pub fn load() -> Self {
        let config = Config::from_args();
        info!(?config, "loaded");
        config
    }
}

/// Configuration of a single hApp from config.yaml
#[derive(Debug, Deserialize)]
pub struct Happ {
    pub app_id: AppId,
    pub ui_url: Url,
    pub dna_url: Url,
}
