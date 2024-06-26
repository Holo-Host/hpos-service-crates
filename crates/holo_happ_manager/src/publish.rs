use super::hha::HHAAgent;
use anyhow::Result;
use holochain_types::prelude::ActionHashB64;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    hha_types::HappInput,
    holo_config::{Config, Happ},
};
use serde::Deserialize;

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
    agent
        .zome_call(
            agent.cells.core_app.clone(),
            ZomeName::from("hha"),
            FunctionName::from("register_happ"),
            happ,
        )
        .await
}
