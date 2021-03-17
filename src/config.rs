use serde::Deserialize;
use std::path::PathBuf;
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
    /// Path to a YAML file containing the lists of hApps to install
    pub happs_file_path: PathBuf,
    /// Path to a YAML file containing hApp membrane proofs
    pub membrane_proofs_file_path: PathBuf,
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
/// ui_path and dna_path takes precedence over ui_url and dna_url respectively
/// and is meant for running tests
#[derive(Debug, Deserialize)]
pub struct Happ {
    pub ui_url: Option<Url>,
    pub ui_path: Option<PathBuf>,
    pub bundle: PathBuf,
}

impl Happ {
    /// generates the installed app id that should be used
    /// based on the path of the bundle.
    /// Assumes file name ends in .happ, and converts periods -> colons
    pub fn id(&self) -> String {
        self.bundle
            .clone()
            .into_os_string()
            .into_string()
            .unwrap()
            .replace(".happ", "")
            .replace(".", ":")
    }
}

/// hApps
#[derive(Debug, Deserialize)]
pub struct HappsFile {
    pub self_hosted_happs: Vec<Happ>,
    pub core_happs: Vec<Happ>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_install_app_id_format() {
        let cfg = Happ {
            bundle: "elemental_chat.1.0001.happ".into(),
            ui_url: None,
            ui_path: None,
        };
        assert_eq!(cfg.id(), String::from("elemental_chat:1:0001"));
    }
}
