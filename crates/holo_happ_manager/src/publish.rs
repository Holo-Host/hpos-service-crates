use super::hha::HHAAgent;
use anyhow::{anyhow, Result};
use holochain_conductor_api::AppResponse;
use holochain_types::prelude::ActionHashB64;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::{
    hha_types::HappInput,
    holo_config::{Config, Happ},
};
use serde::Deserialize;
use tracing::debug;

#[derive(Deserialize, Debug, Clone)]
pub struct PresentedHappBundle {
    pub id: ActionHashB64,
    pub bundle_url: String,
}

pub async fn publish_happ(
    core_happ: &Happ,
    config: &Config,
    happ: HappInput,
) -> Result<PresentedHappBundle> {
    let mut agent = HHAAgent::spawn(core_happ, config).await?;
    let response = agent
        .zome_call(
            agent.cells.core_app.clone(),
            ZomeName::from("hha"),
            FunctionName::from("register_happ"),
            ExternIO::encode(happ)?,
        )
        .await?;

    match response {
        // This is the happs list that is returned from the hha DNA
        // https://github.com/Holo-Host/holo-hosting-app-rsm/blob/develop/zomes/hha/src/lib.rs#L54
        // return Vec of happ_list.happ_id
        AppResponse::ZomeCalled(r) => {
            let happ_bundles: PresentedHappBundle = rmp_serde::from_slice(r.as_bytes())?;
            debug!("Published happ bundles {:?}", happ_bundles);
            Ok(happ_bundles)
        }
        _ => Err(anyhow!("unexpected response: {:?}", response)),
    }
}
