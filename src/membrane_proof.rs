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
use tracing::{debug, instrument};

fn mem_proof_path() -> String {
    match env::var("MEM_PROOF_PATH") {
        Ok(path) => path,
        _ => "./tests/config/mem-proof".to_string(),
    }
}

fn force_use_read_only_mem_proof() -> bool {
    match env::var("READ_ONLY_MEM_PROOF") {
        Ok(path) => path == "true",
        _ => false,
    }
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

pub type MembraneProofsVec = HashMap<String, Arc<SerializedBytes>>;

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

/// Returns memproof from a file at MEM_PROOF_PATH
/// If a file does not exist then function downloads existing mem-proof for given agent
/// from HBS server and saves it to the file
/// Returns error if no memproof obtained, because memproof is mandatory
/// for core-app installation
#[instrument(skip(admin), err)]
pub async fn get_mem_proof(admin: Admin) -> Result<MembraneProof> {
    if let Ok(m) = load_mem_proof_from_file() {
        return Ok(m);
    }

    let mem_proof_str = download_memproof(admin).await?;
    save_mem_proof_to_file(&mem_proof_str)?;

    let mem_proof_bytes = base64::decode(mem_proof_str)?;
    let mem_proof_serialized = Arc::new(SerializedBytes::from(UnsafeBytes::from(mem_proof_bytes)));
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
) -> Result<MembraneProofsVec> {
    let mut mem_proofs_vec = HashMap::new();

    if happ_id.contains("core-app") {
        mem_proofs_vec = crate::membrane_proof::add_core_app(mem_proof);
    }
    Ok(mem_proofs_vec)
}

/// returns core-app specic vec of memproofs for each core-app DNA
/// Holo servers need read only access to core app, therefore
/// on those servers READ_ONLY_MEM_PROOF=true
fn add_core_app(mem_proof: MembraneProof) -> MembraneProofsVec {
    let mut vec = HashMap::new();
    if force_use_read_only_mem_proof() {
        // This setting is mostly going to be used by the holo servers like mem-proof-server and match-server
        let read_only_mem_proof = Arc::new(SerializedBytes::from(UnsafeBytes::from(vec![0])));
        vec.insert("core-app".to_string(), read_only_mem_proof.clone());
        vec.insert("holofuel".to_string(), read_only_mem_proof);
    } else {
        vec.insert("core-app".to_string(), mem_proof.clone());
        vec.insert("holofuel".to_string(), mem_proof);
    }
    vec
}

/// Reads mem-proof from a file under MEM_PROOF_PATH
fn load_mem_proof_from_file() -> Result<MembraneProof> {
    use std::str;
    let path = mem_proof_path();
    let file = fs::read(&path).context("failed to open file")?;
    let mem_proof_str = str::from_utf8(&file)?;
    debug!("Loaded Proof {:?}", mem_proof_str);
    let mem_proof_bytes = base64::decode(mem_proof_str)?;
    let mem_proof_serialized = Arc::new(SerializedBytes::from(UnsafeBytes::from(mem_proof_bytes)));
    Ok(mem_proof_serialized)
}

/// Saves mem-proof to a file under MEM_PROOF_PATH
fn save_mem_proof_to_file(mem_proof: &str) -> Result<()> {
    let mut file = fs::File::create(&mem_proof_path())?;
    file.write_all(mem_proof.as_bytes()).context(format!(
        "Failed writing memproof to file {}",
        &mem_proof_path()
    ))?;
    Ok(())
}

/// Deletes mem-proof file located at MEM_PROOF_PATH
/// if it does exist
pub fn delete_mem_proof_file() -> Result<()> {
    let path = mem_proof_path();

    if Path::new(&path).exists() {
        fs::remove_file(&path)?;
    }

    Ok(())
}

/// Downloads existing mem-proof for a given agent
/// from HBS server and returns as a string
async fn download_memproof(admin: Admin) -> Result<String> {
    let payload = Registration {
        registration_code: admin.registration_code,
        agent_pub_key: PublicKey::from_bytes(admin.key.get_raw_32())?,
        email: admin.email,
        payload: RegistrationPayload {
            role: "host".to_string(),
        },
    };
    let url = format!(
        "{}/registration/api/v1/membrane-proof",
        mem_proof_server_url()
    );
    let resp = CLIENT.post(url).json(&payload).send().await?;
    match resp.error_for_status_ref() {
        Ok(_) => {
            let reg: RegistrationRequest = resp.json().await?;
            println!("Registration completed message ID: {:?}", reg);
            Ok(reg.mem_proof)
        }
        Err(e) => {
            println!("Error: {:?}", e);
            let err: RegistrationError = resp.json().await?;
            Err(AuthError::RegistrationError(err.to_string()).into())
        }
    }
}
