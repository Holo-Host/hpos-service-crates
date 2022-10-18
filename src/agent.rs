use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AdminRequest, AdminResponse};
use holochain_types::dna::AgentPubKey;
use holochain_types::prelude::MembraneProof;
use std::{collections::HashMap, env, fs, fs::File, io::prelude::*};

use crate::membrane_proof::get_hpos_config;
use crate::websocket::AdminWebsocket;
use tracing::{info, instrument, trace};

#[derive(Clone)]
pub struct Agent {
    pub key: AgentPubKey,
    pub membrane_proofs: HashMap<String, MembraneProof>,
}

impl Agent {
    /// Loads agent_key and memproof into memory
    #[instrument(err, skip(admin_websocket))]
    pub async fn init(admin_websocket: AdminWebsocket) -> Result<Self> {
        let key = get_agent_key(admin_websocket).await?;
        // let membrane_proofs = self.get_memproof().await?;

        // if a new agent was created, we expect to get a new mem-proof
        // let agent_pub_key = PublicKey::from_bytes(key.get_raw_32())?;
        // if let Err(e) =
        //             membrane_proof::try_mem_proof_server_inner(Some(agent_pub_key)).await
        //         {
        //             println!("membrane proof error {}", e);
        //         }

        Ok(Self {
            key,
            membrane_proofs: HashMap::new(),
        })
    }
}

#[instrument(skip(admin_websocket), err)]
async fn get_agent_key(mut admin_websocket: AdminWebsocket) -> Result<AgentPubKey> {
    let force = match env::var("FORCE_RANDOM_AGENT_KEY") {
        Ok(f) => !f.is_empty(),
        // The default is set to true since its only used while running `cargo test`.
        // In all other instances in holo-nixpkgs we have set a value based on the enviroment
        Err(_) => true,
    };
    // Based on the holo-network choose what agent key is to be used
    // For mainNet,flexNet and alphaNet: use the holoport ID as the holochain key
    // For devNet: create a random agent key
    // For mainNet and alphaNet
    if !force {
        info!("Using agent key from hpos-config file");
        // Use agent key from from the config file in main net
        let config = get_hpos_config()?;
        let pub_key = hpos_config_seed_bundle_explorer::holoport_public_key(
            &config,
            Some(crate::config::DEFAULT_PASSWORD.to_string()),
        )
        .await
        .unwrap();
        let key = AgentPubKey::from_raw_32(pub_key.to_bytes().to_vec());
        // Copy to the `agent-key.pub` files for other apps that use it as reference
        if let Ok(pubkey_path) = env::var("PUBKEY_PATH") {
            let mut file = File::create(pubkey_path)?;
            file.write_all(key.get_raw_39())?;
        }
        return Ok(key);
    }
    // For devNet
    // Try agent key from disc
    if let Ok(pubkey_path) = env::var("PUBKEY_PATH") {
        if let Ok(key_vec) = fs::read(&pubkey_path) {
            if let Ok(key) = AgentPubKey::from_raw_39(key_vec) {
                info!("returning agent key from file");
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
            let key_vec = key.get_raw_39();
            if let Ok(pubkey_path) = env::var("PUBKEY_PATH") {
                crate::utils::overwrite(pubkey_path, key_vec)?;
            }
            info!("returning newly created agent key");
            Ok(key)
        }
        _ => Err(anyhow!("unexpected response: {:?}", response)),
    }
}

// Returns HashMap(happ_id, memproof)
// get_memproof() {
// let mut mem_proof = HashMap::new();
//       // Special properties and mem-proofs for core-app
//       if full_happ_id.contains("core-app") {
//           mem_proof = crate::membrane_proof::get_mem_proof().await?;
//       }
// }
