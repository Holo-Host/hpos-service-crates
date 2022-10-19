use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AdminRequest, AdminResponse};
use holochain_types::dna::AgentPubKey;
use holochain_types::prelude::MembraneProof;
use hpos_config_core::Config;
use std::{env, fs, fs::File, io::prelude::*};
use tracing::{info, instrument};

use crate::membrane_proof::get_mem_proof;
use crate::utils::AuthError;
use crate::websocket::AdminWebsocket;

#[derive(Clone)]
pub struct Admin {
    pub key: AgentPubKey,
    pub email: String,
    pub registration_code: String,
}

#[derive(Clone)]
pub struct Agent {
    pub admin: Admin,
    pub membrane_proof: MembraneProof,
}

impl Agent {
    /// Loads agent_key and memproof into memory
    #[instrument(err, skip(admin_websocket))]
    pub async fn init(admin_websocket: AdminWebsocket) -> Result<Self> {
        let admin = populate_admin(admin_websocket).await?;
        let membrane_proof = get_mem_proof(admin.clone()).await?;

        Ok(Self {
            admin,
            membrane_proof,
        })
    }
}

/// Populates Admin struct with agent's pub_key and admin details
/// extracted from hpos_config file
#[instrument(skip(admin_websocket), err)]
async fn populate_admin(admin_websocket: AdminWebsocket) -> Result<Admin> {
    let config = get_hpos_config()?;
    let key = get_agent_key(admin_websocket, &config).await?;

    save_pubkey(key.clone().get_raw_39()).await?;

    match config {
        Config::V2 {
            registration_code,
            settings,
            ..
        } => {
            Ok(Admin {
                key,
                registration_code,
                email: settings.admin.email,
            })
        }
        Config::V1 { .. } => {
            Err(AuthError::ConfigVersionError.into())
        }
    }
}

/// Makes sure that the right agent key is in use based on the value
/// of env var FORCE_RANDOM_AGENT_KEY. Once selected agent key is saved to
/// a file under PUBKEY_PATH.
/// For example on devNet FORCE_RANDOM_AGENT_KEY=true in which case
/// random agent key is used
#[instrument(skip(admin_websocket), err)]
async fn get_agent_key(
    mut admin_websocket: AdminWebsocket,
    config: &Config,
) -> Result<AgentPubKey> {
    if force_random_agent_key() {
        // Try agent key from disc
        if let Ok(pubkey_path) = env::var("PUBKEY_PATH") {
            if let Ok(key_vec) = fs::read(&pubkey_path) {
                if let Ok(key) = AgentPubKey::from_raw_39(key_vec) {
                    info!("returning random agent key from file");
                    return Ok(key);
                }
            }
        }
        // Create agent key in Lair and save it in file
        let response = admin_websocket
            .send(AdminRequest::GenerateAgentPubKey)
            .await?;

        match response {
            AdminResponse::AgentPubKeyGenerated(key) => {
                info!("returning newly created random agent key");
                Ok(key)
            }
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    } else {
        info!("Using agent key from hpos-config file");

        let pub_key = hpos_config_seed_bundle_explorer::holoport_public_key(
            config,
            Some(crate::config::DEFAULT_PASSWORD.to_string()),
        )
        .await
        .unwrap();

        Ok(AgentPubKey::from_raw_32(pub_key.to_bytes().to_vec()))
    }
}

/// Saves host's pub key to the file `agent-key.pub`
/// so that other apps in the system can access it
#[instrument(skip(buf), err)]
async fn save_pubkey(buf: &[u8]) -> Result<()> {
    if let Ok(pubkey_path) = env::var("PUBKEY_PATH") {
        let mut file = File::create(&pubkey_path)?;
        file.write_all(buf)
            .context(format!("Failed writing to pubkey file {}", &pubkey_path))
    } else {
        Err(anyhow!("PUBKEY_PATH is not set, cannot save pubkey"))
    }
}

/// Calcultes whether random agent key should be used
/// based on a value of FORCE_RANDOM_AGENT_KEY
/// Defaults to true for `cargo test`
fn force_random_agent_key() -> bool {
    if let Ok(f) = env::var("FORCE_RANDOM_AGENT_KEY") {
        return !f.is_empty();
    }
    true
}

/// Reads hpos-config into a struct
pub fn get_hpos_config() -> Result<Config> {
    let config_path = env::var("HPOS_CONFIG_PATH")?;
    let config_json = fs::read(config_path)?;
    let config: Config = serde_json::from_slice(&config_json)?;
    Ok(config)
}
