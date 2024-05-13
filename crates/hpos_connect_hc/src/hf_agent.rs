use super::holo_config::{self, HappsFile, APP_PORT};
use crate::{holo_config::ADMIN_PORT, utils::fresh_nonce};
use crate::{AdminWebsocket, AppWebsocket};
use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AppInfo, AppResponse, CellInfo, ProvisionedCell, ZomeCall};
use holochain_keystore::{AgentPubKeyExt, MetaLairClient};
use holochain_types::prelude::{
    AgentPubKey, ExternIO, FunctionName, Signature, ZomeCallUnsigned, ZomeName,
};
use std::sync::Arc;

pub struct HolofuelAgent {
    app_websocket: AppWebsocket,
    // admin_websocket: AdminWebsocket,
    keystore: MetaLairClient,
    holofuel_id: String,
}

impl HolofuelAgent {
    /// Connect to a holofuel instance identified by an app_id. If app_id is passed as None, then
    /// crate will read app_id from data passed in environmental variables CORE_HAPP_FILE and DEV_UID_OVERRIDE
    /// so that it connects to a default holofuel instance on HPOS
    pub async fn connect() -> Result<Self> {
        let mut admin_websocket = AdminWebsocket::connect(ADMIN_PORT)
            .await
            .context("failed to connect to holochain's app interface")?;
        let passphrase =
            sodoken::BufRead::from(holo_config::default_password()?.as_bytes().to_vec());
        let keystore = holochain_keystore::lair_keystore::spawn_lair_keystore(
            url2::url2!("{}", holo_config::get_lair_url()?),
            passphrase,
        )
        .await?;
        let holofuel_id: String;
        if let Ok(id) = std::env::var("TEST_HOLOFUEL_ID") {
            holofuel_id = id;
        } else {
            let app_file = HappsFile::load_happ_file_from_env()?;
            let holofuel = app_file
                .holofuel()
                .ok_or(anyhow!("Could not find a holofuel in HPOS file"))?;
            holofuel_id = holofuel.id();
        }

        let token = admin_websocket
            .issue_app_auth_token(holofuel_id.clone())
            .await?;

        let app_websocket = AppWebsocket::connect(APP_PORT, token)
            .await
            .context("failed to connect to holochain's app interface")?;

        Ok(Self {
            app_websocket,
            // admin_websocket,
            keystore,
            holofuel_id,
        })
    }

    /// get cell details of the holofuel agent
    pub async fn get_cell(&mut self) -> Result<(ProvisionedCell, AgentPubKey)> {
        match self.app_websocket.get_app_info().await {
            Some(AppInfo {
                // This works on the assumption that the core apps has HHA in the first position of the vec
                cell_info,
                agent_pub_key,
                ..
            }) => {
                let cell = match &cell_info
                    .get("holofuel")
                    .ok_or(anyhow!("there's no cell named holofuel!"))?[0]
                {
                    CellInfo::Provisioned(c) => c.clone(),
                    _ => return Err(anyhow!("unable to find holofuel")),
                };
                Ok((cell, agent_pub_key))
            }
            _ => Err(anyhow!("holofuel is not installed")),
        }
    }

    /// Sign byte payload with holofuel agent's private key
    pub async fn sign_raw(&mut self, data: Arc<[u8]>) -> Result<Signature> {
        let (_, agent_pubkey) = self.get_cell().await?;
        Ok(agent_pubkey.sign_raw(&self.keystore, data).await?)
    }

    /// make a zome call to the holofuel agent that is running on a hpos server
    pub async fn zome_call(
        &mut self,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: ExternIO,
    ) -> Result<ExternIO> {
        let (cell, agent_pubkey) = self.get_cell().await?;
        let (nonce, expires_at) = fresh_nonce()?;
        let zome_call_unsigned = ZomeCallUnsigned {
            cell_id: cell.cell_id,
            zome_name,
            fn_name,
            payload,
            cap_secret: None,
            provenance: agent_pubkey,
            nonce,
            expires_at,
        };
        let signed_zome_call =
            ZomeCall::try_from_unsigned_zome_call(&self.keystore, zome_call_unsigned).await?;

        match self
            .app_websocket
            .zome_call(signed_zome_call)
            .await
            .map_err(|err| anyhow!("{:?}", err))?
        {
            AppResponse::ZomeCalled(bytes) => Ok(*bytes),
            _ => Err(anyhow!("")),
        }
    }
}
