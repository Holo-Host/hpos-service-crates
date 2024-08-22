use super::admin_ws::AdminWebsocket;
use super::hpos_membrane_proof::{delete_mem_proof_file, get_mem_proof};
use anyhow::{Context, Result};
use ed25519_dalek::*;
use holochain_types::dna::AgentPubKey;
use holochain_types::prelude::MembraneProof;
use hpos_config_core::Config;
use hpos_config_seed_bundle_explorer::unlock;
use std::{env, fs, fs::File, io::prelude::*};
use tracing::{info, instrument};

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

pub async fn get_signing_admin() -> Result<(SigningKey, String)> {
    let password = bundle_default_password()?;
    let config_path = env::var("HPOS_CONFIG_PATH")
        .context("Failed to read HPOS_CONFIG_PATH. Is it set in env?")?;
    match get_hpos_config()? {
        Config::V2 {
            device_bundle,
            settings,
            ..
        } => {
            // take in password
            let signing_key = unlock(&device_bundle, Some(password))
                .await
                .context(format!(
                    "unable to unlock the device bundle from {}",
                    &config_path
                ))?;
            Ok((signing_key, settings.admin.email))
        }
        _ => Err(AuthError::ConfigVersionError.into()),
    }
}

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Error: Invalid config version used. please upgrade to hpos-config v2")]
    ConfigVersionError,
    #[error("Registration Error: {}", _0)]
    RegistrationError(String),
}

/// Populates Admin struct with agent's pub_key and admin details
/// extracted from hpos_config file
#[instrument(skip(admin_websocket), err)]
async fn populate_admin(admin_websocket: AdminWebsocket) -> Result<Admin> {
    let config = get_hpos_config()?;
    let key = get_agent_key(admin_websocket, &config).await?;

    match config {
        Config::V3 {
            registration_code,
            settings,
            ..
        } => Ok(Admin {
            key,
            registration_code,
            email: settings.admin.email,
        }),
        Config::V2 {
            registration_code,
            settings,
            ..
        } => Ok(Admin {
            key,
            registration_code,
            email: settings.admin.email,
        }),
        Config::V1 { .. } => Err(AuthError::ConfigVersionError.into()),
    }
}

/// Makes sure that the right agent key is in use based on the value
/// of env var FORCE_RANDOM_AGENT_KEY. Once selected agent key is saved to
/// a file under HOST_PUBKEY_PATH.
/// For example on devNet FORCE_RANDOM_AGENT_KEY=1 in which case
/// random agent key is used
#[instrument(skip(admin_websocket, config), err)]
async fn get_agent_key(
    mut admin_websocket: AdminWebsocket,
    config: &Config,
) -> Result<AgentPubKey> {
    let pubkey_path = env::var("HOST_PUBKEY_PATH")
        .context("Failed to read HOST_PUBKEY_PATH. Is it set in env?")?;

    let key_result: Result<AgentPubKey> = if &env::var("FORCE_RANDOM_AGENT_KEY")
        .context("Failed to read FORCE_RANDOM_AGENT_KEY. Is it set in env?")?
        == "1"
    {
        // Try agent key from disc
        if let Ok(key_vec) = fs::read(&pubkey_path) {
            if let Ok(key) = AgentPubKey::from_raw_39(key_vec) {
                info!("returning random agent key from file");
                return Ok(key);
            }
        }
        // Create agent key in Lair and save it in file
        let key: AgentPubKey = admin_websocket.generate_agent_pub_key().await?;

        // Creating new random agent makes memproof file invalid,
        // because each memproof is valid only for a particular agent
        // If we delete memproof file now it will be regenerated
        // in next step for newly created agent
        info!("deleting memproof file for previous agent");
        delete_mem_proof_file()?;

        info!("returning newly created random agent key");
        Ok(key)
    } else {
        info!("Using agent key from hpos-config file");

        let pub_key = hpos_config_seed_bundle_explorer::holoport_public_key(
            config,
            Some(bundle_default_password()?),
        )
        .await
        .unwrap();

        Ok(AgentPubKey::from_raw_32(pub_key.to_bytes().to_vec()))
    };

    save_pubkey(key_result?, &pubkey_path).await
}

pub fn bundle_default_password() -> Result<String> {
    env::var("DEVICE_SEED_DEFAULT_PASSWORD")
        .context("Failed to read DEVICE_SEED_DEFAULT_PASSWORD. Is it set in env?")
}

/// Saves host's pub key to the file under pubkey_path
/// so that other apps in the system can access it
#[instrument(skip(pub_key), err)]
async fn save_pubkey(pub_key: AgentPubKey, pubkey_path: &str) -> Result<AgentPubKey> {
    let mut file = File::create(pubkey_path)?;
    file.write_all(pub_key.clone().get_raw_39())
        .context(format!("Failed writing to pubkey file {}", &pubkey_path))?;
    Ok(pub_key)
}

/// Reads hpos-config into a struct
pub fn get_hpos_config() -> Result<Config> {
    let config_path = env::var("HPOS_CONFIG_PATH")
        .context("Failed to read HPOS_CONFIG_PATH. Is it set in env?")?;
    read_hpos_config(&config_path)
}

pub fn read_hpos_config(path: &String) -> Result<Config> {
    let config_json = fs::read(path)?;
    let config: Config = serde_json::from_slice(&config_json)?;
    Ok(config)
}
