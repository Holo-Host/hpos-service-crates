use super::hha::HHAAgent;
use anyhow::{anyhow, Result};
use holochain_conductor_api::AppResponse;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::{
    hha_types::PresentedHappBundle,
    holo_config::{Config, Happ},
};
use tracing::debug;

pub async fn published(core_happ: &Happ, config: &Config) -> Result<Vec<PresentedHappBundle>> {
    let mut agent = HHAAgent::spawn(core_happ, config).await?;
    let response = agent
        .zome_call(
            agent.cells.core_app.clone(),
            ZomeName::from("hha"),
            FunctionName::from("get_my_happs"),
            ExternIO::encode(())?,
        )
        .await?;

    match response {
        // This is the happs list that is returned from the hha DNA
        // https://github.com/Holo-Host/holo-hosting-app-rsm/blob/develop/zomes/hha/src/lib.rs#L54
        // return Vec of happ_list.happ_id
        AppResponse::ZomeCalled(r) => {
            let happ_bundles: Vec<PresentedHappBundle> = rmp_serde::from_slice(r.as_bytes())?;
            debug!("got happ bundles {:?}", happ_bundles);
            Ok(happ_bundles)
        }
        _ => Err(anyhow!("unexpected response: {:?}", response)),
    }
}
