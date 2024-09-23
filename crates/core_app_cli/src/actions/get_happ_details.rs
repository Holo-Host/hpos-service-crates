use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, hha_types::PresentedHappBundle,
};

pub async fn get(happ_id: String) -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;

    let happ: PresentedHappBundle = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("get_happ"),
            happ_id.clone(),
        )
        .await?;

    println!("===================");
    println!("Happ Details {:?}", happ);
    println!("===================");

    Ok(())
}
