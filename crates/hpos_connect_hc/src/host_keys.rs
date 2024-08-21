use super::hpos_agent::get_signing_admin;
use anyhow::{Context, Result};
use base64::encode_config;
use ed25519_dalek::*;
use holochain_types::prelude::ExternIO;
use hpos_config_core::public_key;

#[derive(Clone, Debug)]
pub struct HostKeys {
    pub email: String,
    keypair: SigningKey,
    pub pubkey_base36: String,
    pub holoport_id: String,
}

impl HostKeys {
    pub async fn new() -> Result<Self> {
        let (keypair, email) = get_signing_admin().await?;
        let pubkey_base36 = public_key::to_holochain_encoded_agent_key(&keypair.verifying_key());
        let holoport_id = public_key::to_base36_id(&keypair.verifying_key());

        Ok(Self {
            email,
            keypair,
            pubkey_base36,
            holoport_id,
        })
    }

    pub async fn sign(&self, payload: ExternIO) -> Result<String> {
        let signature = self
            .keypair
            .try_sign(payload.as_bytes())
            .context("Failed to sign payload")?;

        Ok(encode_config(
            &signature.to_bytes()[..],
            base64::STANDARD_NO_PAD,
        ))
    }
}
