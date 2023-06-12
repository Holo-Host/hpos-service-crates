use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AppInfo, AppResponse};
use holochain_conductor_api::{CellInfo, ZomeCall};
use holochain_types::prelude::ActionHashB64;
use holochain_types::prelude::{zome_io::ExternIO, FunctionName, ZomeName};
use holochain_types::prelude::{Nonce256Bits, Timestamp, ZomeCallUnsigned};
use hpos_hc_connect::holo_config::{Config, Happ};
use hpos_hc_connect::AppWebsocket;
use serde::Deserialize;
use std::time::Duration;
use tracing::trace;

use super::hha_type::HappInput;

#[derive(Deserialize, Debug, Clone)]
pub struct PresentedHappBundle {
    pub id: ActionHashB64,
    pub bundle_url: String,
}

pub async fn publish_happ(
    core_happ: &Happ,
    config: &Config,
    happ: HappInput,
) -> Result<Vec<PresentedHappBundle>> {
    trace!("get_all_enabled_hosted_happs");
    let mut app_websocket = AppWebsocket::connect(42233)
        .await
        .context("failed to connect to holochain's app interface")?;
    trace!("get app info for {}", core_happ.id());
    match app_websocket.get_app_info(core_happ.id()).await {
        Some(AppInfo {
            // This works on the assumption that the core happs has HHA in the first position of the vec
            cell_info,
            ..
        }) => {
            trace!("got app info");

            let cell = match &cell_info.get("core-app").unwrap()[0] {
                CellInfo::Provisioned(c) => c.clone(),
                _ => return Err(anyhow!("core-app cell not found")),
            };
            trace!("got cell {:?}", cell);

            // connect to lair
            let passphrase = sodoken::BufRead::from(
                hpos_hc_connect::hpos_agent::default_password()?
                    .as_bytes()
                    .to_vec(),
            );

            let lair_url = config
                .lair_url
                .clone()
                .ok_or(anyhow!("Does not have lair url, please provide --lair-url"))?;

            let keystore = holochain_keystore::lair_keystore::spawn_lair_keystore(
                url2::url2!("{}", lair_url),
                passphrase,
            )
            .await?;

            let (nonce, expires_at) = fresh_nonce()?;
            let zome_call_unsigned = ZomeCallUnsigned {
                cell_id: cell.cell_id.clone(),
                zome_name: ZomeName::from("hha"),
                fn_name: FunctionName::from("register_happ"),
                payload: ExternIO::encode(happ)?,
                cap_secret: None,
                provenance: cell.cell_id.agent_pubkey().clone(),
                nonce,
                expires_at,
            };
            let signed_zome_call =
                ZomeCall::try_from_unsigned_zome_call(&keystore, zome_call_unsigned).await?;

            let response = app_websocket.zome_call(signed_zome_call).await?;

            match response {
                // This is the happs list that is returned from the hha DNA
                // https://github.com/Holo-Host/holo-hosting-app-rsm/blob/develop/zomes/hha/src/lib.rs#L54
                // return Vec of happ_list.happ_id
                AppResponse::ZomeCalled(r) => {
                    let happ_bundles: Vec<PresentedHappBundle> =
                        rmp_serde::from_slice(r.as_bytes())?;
                    trace!("got happ bundles {:?}", happ_bundles);
                    Ok(happ_bundles)
                }
                _ => Err(anyhow!("unexpected response: {:?}", response)),
            }
        }
        None => Err(anyhow!("HHA is not installed")),
    }
}

/// generates nonce for zome calls
pub fn fresh_nonce() -> Result<(Nonce256Bits, Timestamp)> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes)?;
    let nonce = Nonce256Bits::from(bytes);
    // Rather arbitrary but we expire nonces after 5 mins.
    let expires: Timestamp = (Timestamp::now() + Duration::from_secs(60 * 5))?;
    Ok((nonce, expires))
}
