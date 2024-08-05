use crate::app_connection::CoreAppRoleName;
use crate::holo_config::{default_password, get_lair_url, Config, HappsFile, ADMIN_PORT};
use crate::{AdminWebsocket, AppConnection};
use anyhow::{anyhow, Context, Result};
use holochain_keystore::AgentPubKeyExt;
use holochain_types::dna::AgentPubKey;
use holochain_types::prelude::Signature;
use std::sync::Arc;

/// Struct giving access to local instance of HHA on HPOS
/// `config` of type `holo_config::Config` represents CLI params and can be passed
/// to describe local running environment
pub struct HfAgent {
    pub app: AppConnection,
}

impl HfAgent {
    pub async fn spawn(config: Option<&Config>) -> Result<Self> {
        let mut admin_ws = AdminWebsocket::connect(ADMIN_PORT)
            .await
            .context("failed to connect to holochain's app interface")?;

        let app_file = HappsFile::load_happ_file_from_env(config)?;
        let holofuel = app_file
            .holofuel()
            .ok_or(anyhow!("There's no core-app defined in a happs file"))?;

        // connect to lair
        let passphrase = sodoken::BufRead::from(default_password()?.as_bytes().to_vec());

        let keystore = holochain_keystore::lair_keystore::spawn_lair_keystore(
            url2::url2!("{}", get_lair_url(config)?),
            passphrase,
        )
        .await?;

        let holofuel_id = if let Ok(id) = std::env::var("TEST_HOLOFUEL_ID") {
            id
        } else {
            holofuel.id()
        };

        let app = AppConnection::connect(&mut admin_ws, keystore, holofuel_id)
            .await
            .context("failed to connect to holochain's app interface")?;

        Ok(Self { app })
    }

    pub async fn pubkey(&self) -> Result<AgentPubKey> {
        Ok(self
            .app
            .clone()
            .cell(CoreAppRoleName::Holofuel.into())
            .await?
            .agent_pubkey()
            .to_owned())
    }

    pub fn id(&self) -> String {
        self.app.id()
    }

    // /// Sign byte payload with holofuel agent's private key
    pub async fn sign_raw(&mut self, data: Arc<[u8]>) -> Result<Signature> {
        let pubkey = self.pubkey().await?;
        Ok(pubkey.sign_raw(&self.app.keystore, data).await?)
    }
}
