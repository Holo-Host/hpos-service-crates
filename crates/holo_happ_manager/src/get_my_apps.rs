use super::hha::HHAAgent;
use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    hha_types::PresentedHappBundle,
    holo_config::{Config, Happ},
};

pub async fn published(core_happ: &Happ, config: &Config) -> Result<Vec<PresentedHappBundle>> {
    let mut agent = HHAAgent::spawn(core_happ, config).await?;
    agent
        .zome_call(
            agent.cells.core_app.clone(),
            ZomeName::from("hha"),
            FunctionName::from("get_my_happs"),
            (),
        )
        .await
}
