use anyhow::{anyhow, Context, Result};
use ed25519_dalek::*;
use holochain_types::prelude::{MembraneProof, UnsafeBytes};
use holochain_zome_types::SerializedBytes;
use hpos_config_core::{public_key, Config};
use lazy_static::*;
use reqwest::Client;
use serde::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::{env, fmt, fs};
use tracing::{debug, instrument};

pub fn mem_proof_path() -> String {
    match env::var("MEM_PROOF_PATH") {
        Ok(path) => path,
        _ => "./tests/config/mem-proof".to_string(),
    }
}

pub fn force_use_read_only_mem_proof() -> bool {
    match env::var("READ_ONLY_MEM_PROOF") {
        Ok(path) => {
            if path == "true" {
                return true;
            } else {
                return false;
            }
        }
        _ => false,
    }
}

#[derive(thiserror::Error, Debug)]
enum AuthError {
    #[error("Error: Invalid config version used. please upgrade to hpos-config v2")]
    ConfigVersionError,
    #[error("Registration Error: {}", _0)]
    RegistrationError(String),
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
struct RegistrationError {
    error: String,
    isDisplayedToUser: bool,
    info: String,
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error: {}, More Info: {}", self.error, self.info)
    }
}

#[derive(Debug, Serialize)]
struct Registration {
    registration_code: String,
    #[serde(serialize_with = "serialize_holochain_agent_pub_key")]
    agent_pub_key: PublicKey,
    email: String,
    payload: RegistrationPayload,
}

#[derive(Debug, Serialize)]
struct RegistrationPayload {
    role: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistrationRequest {
    mem_proof: String,
}

lazy_static! {
    static ref CLIENT: Client = Client::new();
}

fn serialize_holochain_agent_pub_key<S>(
    public_key: &PublicKey,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&public_key::to_holochain_encoded_agent_key(public_key))
}

fn mem_proof_server_url() -> String {
    match env::var("MEM_PROOF_SERVER_URL") {
        Ok(url) => url,
        _ => "https://hbs.dev.holotest.net".to_string(),
    }
}

pub fn get_hpos_config() -> Result<Config> {
    let config_path = env::var("HPOS_CONFIG_PATH")?;
    let config_json = fs::read(config_path)?;
    let config: Config = serde_json::from_slice(&config_json)?;
    Ok(config)
}

/// get the mem-proof needed to be used in this setup
/// for the holo servers we will want to pass a read-only mem-proof
/// for the holoports we should always expect a mem-proof, else return an error that will stop the core-app from getting installed
pub async fn get_mem_proof() -> Result<HashMap<String, Arc<SerializedBytes>>> {
    let mut mem_proof = HashMap::new();
    if force_use_read_only_mem_proof() {
        // This setting is mostly going to be used by the holo servers like mem-proof-server and match-server
        let read_only_mem_proof = Arc::new(SerializedBytes::from(UnsafeBytes::from(vec![0])));
        mem_proof.insert("core-app".to_string(), read_only_mem_proof.clone());
        mem_proof.insert("holofuel".to_string(), read_only_mem_proof);
    } else {
        if let Ok(proof) = load_mem_proof_file() {
            mem_proof.insert("core-app".to_string(), proof.clone());
            mem_proof.insert("holofuel".to_string(), proof);
        } else {
            // Try again to get a mem-proof
            match crate::membrane_proof::try_mem_proof_server_inner(None).await {
                Ok(_) => {
                    let proof = load_mem_proof_file()?;
                    mem_proof.insert("core-app".to_string(), proof.clone());
                    mem_proof.insert("holofuel".to_string(), proof);
                }
                Err(e) => {
                    return Err(anyhow!(format!(
                        "Unable to fetch a required mem-proof. {:?}",
                        e
                    )))
                }
            }
        }
    }
    Ok(mem_proof)
}

/// reads the mem-proof that is stored on the holoport
/// this proof is used for the core-app i.e. hha and holofuel
#[instrument(err)]
pub fn load_mem_proof_file() -> Result<MembraneProof> {
    use std::str;
    let path = mem_proof_path();
    let file = fs::read(&path).context("failed to open file")?;
    let mem_proof_str = str::from_utf8(&file)?;
    debug!("Loaded Proof {:?}", mem_proof_str);
    let mem_proof_bytes = base64::decode(mem_proof_str)?;
    let mem_proof_serialized = Arc::new(SerializedBytes::from(UnsafeBytes::from(mem_proof_bytes)));
    Ok(mem_proof_serialized)
}

#[instrument(err, skip(holochain_public_key))]
pub async fn try_mem_proof_server_inner(holochain_public_key: Option<PublicKey>) -> Result<()> {
    let config = crate::membrane_proof::get_hpos_config()?;
    let agent_pub_key = if holochain_public_key.is_some() {
        holochain_public_key.unwrap()
    } else {
        hpos_config_seed_bundle_explorer::holoport_public_key(
            &config,
            Some(crate::config::DEFAULT_PASSWORD.to_string()),
        )
        .await?
    };

    match config {
        Config::V2 {
            registration_code,
            settings,
            ..
        } => {
            let email = settings.admin.email.clone();
            let payload = Registration {
                registration_code: registration_code.clone(),
                agent_pub_key,
                email: email.clone(),
                payload: RegistrationPayload {
                    role: "host".to_string(),
                },
            };
            let mem_proof_server_url = format!(
                "{}/registration/api/v1/membrane-proof",
                mem_proof_server_url()
            );
            let resp = CLIENT
                .post(mem_proof_server_url)
                .json(&payload)
                .send()
                .await?;
            match resp.error_for_status_ref() {
                Ok(_) => {
                    let reg: RegistrationRequest = resp.json().await?;
                    println!("Registration completed message ID: {:?}", reg);
                    // save mem-proofs into a file on the hpos
                    crate::utils::write(mem_proof_path(), reg.mem_proof.as_bytes())?;
                }
                Err(_) => {
                    let err: RegistrationError = resp.json().await?;
                    return Err(AuthError::RegistrationError(err.to_string()).into());
                }
            }
        }
        Config::V1 { .. } => {
            return Err(AuthError::ConfigVersionError.into());
        }
    }
    Ok(())
}
