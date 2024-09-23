use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, hha_types::PresentedHappBundle,
};

pub async fn get() -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;

    let happs: Vec<PresentedHappBundle> = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("get_happs"),
            (),
        )
        .await?;

    println!("===================");
    println!("All Holo Happs: ");
    println!("{:?}", happs);
    println!("===================");

    Ok(())
}