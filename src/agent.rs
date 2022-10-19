use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AdminRequest, AdminResponse};
use holochain_types::dna::AgentPubKey;
use holochain_types::prelude::MembraneProof;
use std::{env, fs, fs::File, io::prelude::*};

use crate::membrane_proof::{get_hpos_config, get_mem_proof};
use crate::websocket::AdminWebsocket;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct Agent {
    pub key: AgentPubKey,
    pub membrane_proof: MembraneProof,
}

impl Agent {
    /// Loads agent_key and memproof into memory
    #[instrument(err, skip(admin_websocket))]
    pub async fn init(admin_websocket: AdminWebsocket) -> Result<Self> {
        let key = get_agent_key(admin_websocket).await?;
        let membrane_proof = get_mem_proof(key.clone()).await?;

        // if a new agent was created, we expect to get a new mem-proof
        // let agent_pub_key = PublicKey::from_bytes(key.get_raw_32())?;
        // if let Err(e) =
        //             membrane_proof::try_mem_proof_server_inner(Some(agent_pub_key)).await
        //         {
        //             println!("membrane proof error {}", e);
        //         }

        Ok(Self {
            key,
            membrane_proof,
        })
    }
}

/// Makes sure that the right agent key is in use based on the value
/// of env var FORCE_RANDOM_AGENT_KEY. Also agent key is stored in
/// a file under PUBKEY_PATH.
/// For example on devNet FORCE_RANDOM_AGENT_KEY=true in which case
/// random agent key is used
#[instrument(skip(admin_websocket), err)]
async fn get_agent_key(mut admin_websocket: AdminWebsocket) -> Result<AgentPubKey> {
    if !force_random_agent_key() {
        info!("Using agent key from hpos-config file");

        let config = get_hpos_config()?;
        let pub_key = hpos_config_seed_bundle_explorer::holoport_public_key(
            &config,
            Some(crate::config::DEFAULT_PASSWORD.to_string()),
        )
        .await
        .unwrap();

        let key = AgentPubKey::from_raw_32(pub_key.to_bytes().to_vec());

        // Copy to the `agent-key.pub` files for other apps that use it as reference
        save_pubkey(key.get_raw_39()).await?;

        return Ok(key);
    } else {
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
                save_pubkey(key.get_raw_39()).await?;
                info!("returning newly created random agent key");
                Ok(key)
            }
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    }
}

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
