use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, hha_types::PresentedHappBundle,
};

pub async fn get(publisher_pubkey: String) -> Result<()> {
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

    let publisher_happs: Vec<&PresentedHappBundle> = happs
        .iter()
        .filter(|h| h.provider_pubkey.to_string() == publisher_pubkey)
        .collect();

    println!("===================");
    println!("All Published Happs by {} are: ", publisher_pubkey);
    println!("{:?}", publisher_happs);
    println!("===================");

    Ok(())
}
