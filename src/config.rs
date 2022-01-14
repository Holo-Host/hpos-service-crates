use serde::Deserialize;
use std::{env, path::PathBuf};
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
    pub dnas: Option<Vec<DnaUrl>>,
    pub properties: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DnaUrl {
    pub id: String,
    pub url: Option<Url>,
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
            format!("{}::{}", name.replace(".happ", "").replace(".", ":"), uid)
        } else {
            name.replace(".happ", "").replace(".", ":")
        }
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
            bundle_path: Some("my/path/to/elemental_chat.1.0001.happ".into()),
            bundle_url: None,
            ui_url: None,
            ui_path: None,
            dnas: None,
            properties: None,
        };
        assert_eq!(cfg.id(), String::from("elemental_chat:1:0001"));
        let cfg = Happ {
            bundle_path: None,
            bundle_url: Some(Url::parse("https://github.com/holochain/elemental-chat/releases/download/v0.1.0-alpha1/elemental_chat.1.0001.happ").unwrap()),
            ui_url: None,
            ui_path: None,
            dnas: None,
            properties: None
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
            properties: None,
        };
        assert_eq!(cfg.ui_name(), String::from("elemental_chat"));
    }
}
