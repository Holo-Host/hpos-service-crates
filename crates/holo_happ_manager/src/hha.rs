use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AppInfo, AppResponse, ProvisionedCell};
use holochain_conductor_api::{CellInfo, ZomeCall};
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::{AgentPubKey, ExternIO, FunctionName, ZomeName};
use holochain_types::prelude::{Nonce256Bits, Timestamp, ZomeCallUnsigned};
use hpos_hc_connect::holo_config::{Config, Happ, ADMIN_PORT};
use hpos_hc_connect::{AdminWebsocket, AppWebsocket};
use std::time::Duration;
use tracing::{debug, trace};
use serde::{Serialize, de::DeserializeOwned};

pub struct Cells {
    pub core_app: ProvisionedCell,
    pub holofuel: ProvisionedCell,
}

pub struct HHAAgent {
    app_ws: AppWebsocket,
    keystore: MetaLairClient,
    pub cells: Cells,
}

impl HHAAgent {
    pub async fn spawn(core_happ: &Happ, config: &Config) -> Result<Self> {
        debug!("get_all_enabled_hosted_happs");

        let mut admin_websocket = AdminWebsocket::connect(ADMIN_PORT)
            .await
            .context("failed to connect to holochain's app interface")?;

        let port = admin_websocket.attach_app_interface(None).await?;

        let token = admin_websocket.issue_app_auth_token(core_happ.id()).await?;

        let mut app_ws = AppWebsocket::connect(port, token)
            .await
            .context("failed to connect to holochain's app interface")?;
        debug!("get app info for {}", core_happ.id());
        let cells = match app_ws.get_app_info().await {
            Some(AppInfo {
                // This works on the assumption that the core happs has HHA in the first position of the vec
                cell_info,
                ..
            }) => {
                trace!("got app info");

                let core_app: holochain_conductor_api::ProvisionedCell =
                    match &cell_info.get("core-app").unwrap()[0] {
                        CellInfo::Provisioned(c) => c.clone(),
                        _ => return Err(anyhow!("core-app cell not found")),
                    };
                trace!("got core happ cell {:?}", core_app);
                let holofuel: holochain_conductor_api::ProvisionedCell =
                    match &cell_info.get("holofuel").unwrap()[0] {
                        CellInfo::Provisioned(c) => c.clone(),
                        _ => return Err(anyhow!("holofuel cell not found")),
                    };
                trace!("got holofuel cell {:?}", holofuel);
                Cells{
                    core_app,
                    holofuel
                }
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
            cells,
        })
    }
    pub async fn zome_call<T, R>(
        &mut self,
        cell: ProvisionedCell,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: T,
    ) -> Result<R>
    where
        T: Serialize + std::fmt::Debug,
        R: DeserializeOwned,
    {
        let (nonce, expires_at) = fresh_nonce()?;
        let zome_call_unsigned = ZomeCallUnsigned {
            cell_id: cell.cell_id.clone(),
            zome_name,
            fn_name,
            payload: ExternIO::encode(payload)?,
            cap_secret: None,
            provenance: cell.cell_id.agent_pubkey().clone(),
            nonce,
            expires_at,
        };
        let signed_zome_call =
            ZomeCall::try_from_unsigned_zome_call(&self.keystore, zome_call_unsigned).await?;

            let response = self.app_ws.zome_call(signed_zome_call).await?;

            match response {
                // This is the happs list that is returned from the hha DNA
                // https://github.com/Holo-Host/holo-hosting-app-rsm/blob/develop/zomes/hha/src/lib.rs#L54
                // return Vec of happ_list.happ_id
                AppResponse::ZomeCalled(r) => {
                    let response: R = rmp_serde::from_slice(r.as_bytes())?;
                    Ok(response)
                }
                _ => Err(anyhow!("unexpected response: {:?}", response)),
            }
    }
    pub fn pubkey(&self) -> AgentPubKey {
        self.cells.core_app.cell_id.agent_pubkey().to_owned()
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
