use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::HHAAgent, hha_types::PresentedHappBundle,
};

pub async fn get() -> Result<()> {
    let mut agent = HHAAgent::spawn(None).await?;

    let happs: Vec<PresentedHappBundle> = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("get_my_happs"),
            (),
        )
        .await?;

    println!("===================");
    println!("Your Published Happs is: ");
    println!("{:?}", happs);
    println!("===================");

    Ok(())
}
