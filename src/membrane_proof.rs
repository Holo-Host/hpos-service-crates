use anyhow::{Context, Result};
use ed25519_dalek::*;
use failure::{Fail, Fallible};
use holochain_types::prelude::{MembraneProof, UnsafeBytes};
use holochain_zome_types::SerializedBytes;
use hpos_config_core::{public_key, Config};
use lazy_static::*;
use reqwest::Client;
use serde::*;
use std::path::Path;
use std::sync::Arc;
use std::{env, fmt, fs, fs::File, io::prelude::*};
use tracing::instrument;

pub fn mem_proof_path() -> String {
    match env::var("MEM_PROOF_PATH") {
        Ok(path) => path,
        _ => "./tests/config/mem-proof".to_string(),
    }
}

/// reads the mem-proof that is stored on the holoport
/// this proof is used for the core-app i.e. hha and holofuel
#[instrument(err, fields(path = %path.as_ref().display()))]
pub fn load_mem_proof_file(path: impl AsRef<Path>) -> Result<MembraneProof> {
    use std::str;
    let file = fs::read(&path).context("failed to open file")?;
    let mem_proof_str = str::from_utf8(&file)?;
    let mem_proof_bytes = base64::decode(mem_proof_str)?;
    let mem_proof_serialized = Arc::new(SerializedBytes::from(UnsafeBytes::from(mem_proof_bytes)));
    Ok(MembraneProof::from(mem_proof_serialized))
}

#[derive(Debug, Fail)]
enum AuthError {
    #[fail(display = "Error: Invalid config version used. please upgrade to hpos-config v2")]
    ConfigVersionError,
    #[fail(display = "Registration Error: {}", _0)]
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
        _ => "https://test-membrane-proof-service.holo.host".to_string(),
    }
}

fn get_hpos_config() -> Fallible<Config> {
    let config_path = env::var("HPOS_CONFIG_PATH")?;
    let config_json = fs::read(config_path)?;
    let config: Config = serde_json::from_slice(&config_json)?;
    Ok(config)
}

async fn try_registration_auth(config: &Config, holochain_public_key: PublicKey) -> Fallible<()> {
    match config {
        Config::V2 {
            registration_code,
            settings,
            ..
        } => {
            let email = settings.admin.email.clone();
            let payload = Registration {
                registration_code: registration_code.clone(),
                agent_pub_key: holochain_public_key,
                email: email.clone(),
                payload: RegistrationPayload {
                    role: "host".to_string(),
                },
            };
            let mem_proof_server_url = format!("{}/register-user/", mem_proof_server_url());
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
                    let mut file = File::create(mem_proof_path())?;
                    file.write_all(reg.mem_proof.as_bytes())?;
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

pub async fn enable_memproof_dev_net(agent_key: PublicKey) -> Fallible<()> {
    let config = get_hpos_config()?;
    try_registration_auth(&config, agent_key).await
}
