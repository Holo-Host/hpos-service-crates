use std::sync::Arc;

use crate::app_connection::CoreAppRoleName;
use crate::hha_types::{
    HappAndHost, HappInput, HappPreferences, HoloportDetails, PresentedHappBundle,
    ServiceloggerHappPreferences,
};
use crate::holo_config::{default_password, get_lair_url, Config, HappsFile, ADMIN_PORT};
use crate::holofuel_types::PendingTransaction;
use crate::{AdminWebsocket, AppConnection};
use anyhow::{anyhow, Context, Result};
use holochain_keystore::AgentPubKeyExt;
use holochain_types::dna::{ActionHashB64, AgentPubKey};
use holochain_types::prelude::{ExternIO, FunctionName, Signature, ZomeName};

// NOTE: This should really be renamed CORE_APP_AGENT, as it related to the core app and therfore connects to BOTH hha and hf
/// Struct giving access to local instance of HHA on HPOS
/// `config` of type `holo_config::Config` represents CLI params and can be passed
/// to describe local running environment
pub struct CoreAppAgent {
    pub app: AppConnection,
}

impl CoreAppAgent {
    pub async fn spawn(config: Option<&Config>) -> Result<Self> {
        let mut admin_ws = AdminWebsocket::connect(ADMIN_PORT)
            .await
            .context("failed to connect to holochain's app interface")?;

        let app_file = HappsFile::load_happ_file_from_env(config)?;
        let core_app = app_file
            .core_app()
            .ok_or(anyhow!("There's no core-app defined in a happs file"))?;

        // connect to lair
        let passphrase = sodoken::BufRead::from(default_password()?.as_bytes().to_vec());

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

    // CORE_APP/HHA ZOME CALLS:
    pub async fn pubkey(&self) -> Result<AgentPubKey> {
        Ok(self
            .app
            .clone()
            .cell(CoreAppRoleName::HHA.into())
            .await?
            .agent_pubkey()
            .to_owned())
    }

    pub async fn get_happs(&mut self) -> Result<Vec<PresentedHappBundle>> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("get_happs"),
                (),
            )
            .await
    }

    pub async fn get_hosts(&mut self, happ_id: ActionHashB64) -> Result<Vec<HoloportDetails>> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("get_hosts"),
                happ_id,
            )
            .await
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

    pub async fn get_host_preferences(&mut self) -> Result<HappPreferences> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("get_default_happ_preferences"),
                (),
            )
            .await
    }

    pub async fn get_happ_preferences(
        &mut self,
        happ_id: ActionHashB64,
    ) -> Result<ServiceloggerHappPreferences> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("get_happ_preferences"),
                happ_id,
            )
            .await
    }

    pub async fn get_publisher_jurisdiction(
        &mut self,
        pubkey: AgentPubKey,
    ) -> Result<Option<String>> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("get_publisher_jurisdiction"),
                pubkey,
            )
            .await
    }

    pub async fn holo_enable_happ(
        &mut self,
        happ_id: &ActionHashB64,
        holoport_id: &String,
    ) -> Result<()> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("enable_happ"),
                HappAndHost {
                    happ_id: happ_id.to_owned(),
                    holoport_id: holoport_id.to_owned(),
                },
            )
            .await
    }

    pub async fn holo_disable_happ(
        &mut self,
        happ_id: &ActionHashB64,
        holoport_id: &String,
    ) -> Result<()> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("disable_happ"),
                HappAndHost {
                    happ_id: happ_id.to_owned(),
                    holoport_id: holoport_id.to_owned(),
                },
            )
            .await
    }

    // CORE_APP/HF ZOME CALLS:
    pub async fn get_pending_transactions(&mut self) -> Result<PendingTransaction> {
        self.app
            .zome_call_typed(
                CoreAppRoleName::Holofuel.into(),
                ZomeName::from("transactor"),
                FunctionName::from("get_pending_transactions"),
                (),
            )
            .await
    }

    /// Sign byte payload with holofuel agent's private key
    /// Currently it is commented out, because I do not know what agent key shall i use
    pub async fn sign_raw(&mut self, data: Arc<[u8]>) -> Result<Signature> {
        let pubkey = self.pubkey().await?;
        Ok(pubkey.sign_raw(&self.app.keystore, data).await?)
    }

    pub fn id(&self) -> String {
        self.app.id()
    }
}
