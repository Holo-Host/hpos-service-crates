use crate::agent::Admin;
use crate::utils::AuthError;
use anyhow::{Context, Result};
use ed25519_dalek::*;
use holochain_types::prelude::{MembraneProof, UnsafeBytes};
use holochain_zome_types::SerializedBytes;
use hpos_config_core::public_key;
use lazy_static::*;
use reqwest::Client;
use serde::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::{env, fmt, fs, io::Write, path::Path};
use tracing::{debug, error, instrument};

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

pub type MembraneProofs = HashMap<String, Arc<SerializedBytes>>;

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

/// Some Holo servers (like mem-proof-server and match-server) set READ_ONLY_MEM_PROOF=true because
/// they need read only access to core app. In that case function returns "empty" memproof.
/// In other cases returns memproof from a file at MEM_PROOF_PATH
/// If a file does not exist then function downloads existing mem-proof for given agent
/// from HBS server and saves it to the file
/// Returns error if no memproof obtained, because memproof is mandatory
/// for core-app installation
#[instrument(skip(admin), err)]
pub async fn get_mem_proof(admin: Admin) -> Result<MembraneProof> {
    if &env::var("READ_ONLY_MEM_PROOF")
        .context("Failed to read READ_ONLY_MEM_PROOF. Is it set in env?")?
        == "true"
    {
        debug!("Using read-only memproof");
        return Ok(Arc::new(SerializedBytes::from(UnsafeBytes::from(vec![0]))));
    }

    let memproof_path =
        env::var("MEM_PROOF_PATH").context("Failed to read MEM_PROOF_PATH. Is it set in env?")?;
    if let Ok(m) = load_mem_proof_from_file(&memproof_path) {
        debug!("Using memproof from file");
        return Ok(m);
    }

    let mem_proof_str = download_memproof(admin).await?;
    save_mem_proof_to_file(&mem_proof_str, &memproof_path)?;

    let mem_proof_bytes = base64::decode(mem_proof_str)?;
    let mem_proof_serialized = Arc::new(SerializedBytes::from(UnsafeBytes::from(mem_proof_bytes)));

    debug!("Using memproof downloaded from HBS server");
    Ok(mem_proof_serialized)
}

/// Creates HashMap of memproofs for dnas based on happ id
/// which is later consumed during happ installation
/// Currently creates memproofs only for core-app
/// otherwise returns empty HashMap
/// Returns HashMap<dna_name, memproof_bytes>
pub async fn create_vec_for_happ(
    happ_id: &str,
    mem_proof: MembraneProof,
) -> Result<MembraneProofs> {
    let mut mem_proofs_vec = HashMap::new();
    if happ_id.contains("core-app") {
        mem_proofs_vec = crate::membrane_proof::add_core_app(mem_proof)?;
    } else if happ_id.contains("holofuel") {
        mem_proofs_vec = crate::membrane_proof::add_holofuel(mem_proof)?;
    }
    Ok(mem_proofs_vec)
}

/// returns core-app specic vec of memproofs for each core-app DNA
fn add_core_app(mem_proof: MembraneProof) -> Result<MembraneProofs> {
    let mut vec = HashMap::new();
    vec.insert("core-app".to_string(), mem_proof.clone());
    vec.insert("holofuel".to_string(), mem_proof);
    Ok(vec)
}

/// returns holofuel specic vec of memproofs for each holofuel DNA
fn add_holofuel(mem_proof: MembraneProof) -> Result<MembraneProofs> {
    let mut vec = HashMap::new();
    vec.insert("holofuel".to_string(), mem_proof);
    Ok(vec)
}

/// Reads mem-proof from a file under MEM_PROOF_PATH
fn load_mem_proof_from_file(path: &str) -> Result<MembraneProof> {
    use std::str;
    let file = fs::read(path).context("failed to open file")?;
    let mem_proof_str = str::from_utf8(&file)?;
    debug!("Loaded Proof {:?}", mem_proof_str);
    let mem_proof_bytes = base64::decode(mem_proof_str)?;
    let mem_proof_serialized = Arc::new(SerializedBytes::from(UnsafeBytes::from(mem_proof_bytes)));
    Ok(mem_proof_serialized)
}

/// Saves mem-proof to a file under MEM_PROOF_PATH
fn save_mem_proof_to_file(mem_proof: &str, path: &str) -> Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(mem_proof.as_bytes())
        .context(format!("Failed writing memproof to file {}", path))?;
    Ok(())
}

/// Deletes mem-proof file located at MEM_PROOF_PATH
/// if it does exist
pub fn delete_mem_proof_file() -> Result<()> {
    if let Ok(path) = env::var("MEM_PROOF_PATH") {
        if Path::new(&path).exists() {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// Add's a pub key to an existing registration and generates a membrane proof.
/// If a membrane proof is already generated downloads that membrane proof.
/// from HBS server and returns as a string
async fn download_memproof(admin: Admin) -> Result<String> {
    let role = env::var("HOLOFUEL_INSTANCE_ROLE")
        .context("Failed to read MEM_PROOF_SERVER_URL. Is it set in env?")?;
    let payload = Registration {
        registration_code: admin.registration_code,
        agent_pub_key: PublicKey::from_bytes(admin.key.get_raw_32())?,
        email: admin.email,
        payload: RegistrationPayload { role },
    };
    let url = format!(
        "{}/registration/api/v1/membrane-proof",
        env::var("MEM_PROOF_SERVER_URL")
            .context("Failed to read MEM_PROOF_SERVER_URL. Is it set in env?")?
    );
    let resp = CLIENT.post(url).json(&payload).send().await?;
    match resp.error_for_status_ref() {
        Ok(_) => {
            let reg: RegistrationRequest = resp.json().await?;
            debug!("Registration completed message: {:?}", reg);
            Ok(reg.mem_proof)
        }
        Err(e) => {
            error!("Error: {:?}", e);
            let err: RegistrationError = resp.json().await?;
            Err(AuthError::RegistrationError(err.to_string()).into())
        }
    }
}
