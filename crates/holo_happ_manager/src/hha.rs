use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AppInfo, AppResponse, ProvisionedCell};
use holochain_conductor_api::{CellInfo, ZomeCall};
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::{AgentPubKey, ExternIO, FunctionName, ZomeName};
use holochain_types::prelude::{Nonce256Bits, Timestamp, ZomeCallUnsigned};
use hpos_hc_connect::holo_config::{Config, Happ};
use hpos_hc_connect::AppWebsocket;
use std::time::Duration;
use tracing::debug;

pub struct HHAAgent {
    app_ws: AppWebsocket,
    keystore: MetaLairClient,
    cell: ProvisionedCell,
}

impl HHAAgent {
    pub async fn spawn(core_happ: &Happ, config: &Config) -> Result<Self> {
        debug!("get_all_enabled_hosted_happs");
        let mut app_ws = AppWebsocket::connect(42233)
            .await
            .context("failed to connect to holochain's app interface")?;
        debug!("get app info for {}", core_happ.id());
        let cell = match app_ws.get_app_info(core_happ.id()).await {
            Some(AppInfo {
                // This works on the assumption that the core happs has HHA in the first position of the vec
                cell_info,
                ..
            }) => {
                debug!("got app info");
                let cell = match &cell_info.get("core-app").unwrap()[0] {
                    CellInfo::Provisioned(c) => c.clone(),
                    _ => return Err(anyhow!("core-app cell not found")),
                };
                debug!("got cell {:?}", cell);
                cell
            }
            None => return Err(anyhow!("HHA is not installed")),
        };
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

        Ok(Self {
            app_ws,
            keystore,
            cell,
        })
    }
    pub async fn zome_call(
        &mut self,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: ExternIO,
    ) -> Result<AppResponse> {
        let (nonce, expires_at) = fresh_nonce()?;
        let zome_call_unsigned = ZomeCallUnsigned {
            cell_id: self.cell.cell_id.clone(),
            zome_name,
            fn_name,
            payload,
            cap_secret: None,
            provenance: self.cell.cell_id.agent_pubkey().clone(),
            nonce,
            expires_at,
        };
        let signed_zome_call =
            ZomeCall::try_from_unsigned_zome_call(&self.keystore, zome_call_unsigned).await?;

        self.app_ws.zome_call(signed_zome_call).await
    }
    pub fn pubkey(&self) -> AgentPubKey {
        self.cell.cell_id.agent_pubkey().to_owned()
    }
}

/// generates nonce for zome calls
pub fn fresh_nonce() -> Result<(Nonce256Bits, Timestamp)> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes)?;
    let nonce = Nonce256Bits::from(bytes);
    // Rather arbitrary but we expire nonces after 5 mins.
    let expires = (Timestamp::now() + Duration::from_secs(60 * 5))?;
    Ok((nonce, expires))
}
