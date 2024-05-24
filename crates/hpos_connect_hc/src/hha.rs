use anyhow::{anyhow, Context, Result};
use holochain_types::dna::{ActionHashB64, AgentPubKey};
use holochain_types::prelude::{FunctionName, ZomeName};
use crate::app_connection::CoreAppRoleName;
use crate::hha_types::HappInput;
use crate::holo_config::{default_password, get_lair_url, Config, HappsFile, ADMIN_PORT};
use crate::{AdminWebsocket, AppConnection};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct PresentedHappBundle {
    pub id: ActionHashB64,
    pub bundle_url: String,
}

/// Struct giving access to local instance of HHA on HPOS
/// `config` of type `holo_config::Config` represents CLI params and can be passed
/// to describe local running environment
pub struct HHAAgent {
    pub app: AppConnection,
}

impl HHAAgent {
    pub async fn spawn(config: Option<&Config>) -> Result<Self> {
        let mut admin_ws = AdminWebsocket::connect(ADMIN_PORT)
            .await
            .context("failed to connect to holochain's app interface")?;

        let app_file = HappsFile::load_happ_file_from_env(config)?;
        let core_app = app_file
            .core_app()
            .ok_or(anyhow!("There's no core-app defined in a happs file"))?;

        // connect to lair
        let passphrase = sodoken::BufRead::from(
            default_password()?
                .as_bytes()
                .to_vec(),
        );

        let keystore = holochain_keystore::lair_keystore::spawn_lair_keystore(
            url2::url2!("{}", get_lair_url(config)?),
            passphrase,
        )
        .await?;

        let app = AppConnection::connect(&mut admin_ws, keystore, core_app.id())
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

    pub fn id(&self) -> String {
        self.app.id()
    }
}
