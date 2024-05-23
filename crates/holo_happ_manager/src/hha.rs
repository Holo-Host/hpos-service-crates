use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AppInfo, AppResponse, ProvisionedCell};
use holochain_conductor_api::{CellInfo, ZomeCall};
use holochain_keystore::MetaLairClient;
use holochain_types::dna::ActionHashB64;
use holochain_types::prelude::{AgentPubKey, ExternIO, FunctionName, ZomeName};
use holochain_types::prelude::{Nonce256Bits, Timestamp, ZomeCallUnsigned};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::hha_types::HappInput;
use hpos_hc_connect::holo_config::{Config, Happ, ADMIN_PORT};
use hpos_hc_connect::{AdminWebsocket, AppConnection};
use serde::Deserialize;
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;
use tracing::{debug, trace};

#[derive(Deserialize, Debug, Clone)]
pub struct PresentedHappBundle {
    pub id: ActionHashB64,
    pub bundle_url: String,
}

pub struct HHAAgent {
    pub app: AppConnection,
}

impl HHAAgent {
    pub async fn spawn(
        core_happ: &Happ,
        config: &Config,
        admin_ws: &mut AdminWebsocket,
    ) -> Result<Self> {
        // connect to lair
        let passphrase = sodoken::BufRead::from(
            hpos_hc_connect::holo_config::default_password()?
                .as_bytes()
                .to_vec(),
        );

        let lair_url = config
            .lair_url
            .clone()
            .ok_or_else(|| anyhow!("Does not have lair url, please provide --lair-url"))?;

        let keystore = holochain_keystore::lair_keystore::spawn_lair_keystore(
            url2::url2!("{}", lair_url),
            passphrase,
        )
        .await?;

        let app = AppConnection::connect(admin_ws, keystore, core_happ.id())
            .await
            .context("failed to connect to holochain's app interface")?;

        Ok(Self { app })
    }

    pub async fn pubkey(&self) -> Result<AgentPubKey> {
        Ok(self
            .app
            .clone()
            .cell(CoreAppRoleName::HHA.into())
            .await?
            .agent_pubkey()
            .to_owned())
    }

    pub async fn get_my_happs(&mut self) -> Result<Vec<PresentedHappBundle>> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("get_my_happs"),
                (),
            )
            .await
    }

    pub async fn publish_happ(&mut self, happ: HappInput) -> Result<PresentedHappBundle> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("register_happ"),
                happ,
            )
            .await
    }
}
