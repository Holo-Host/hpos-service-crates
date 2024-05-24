use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{app_connection::CoreAppRoleName, hha::HHAAgent, hha_types::HoloportDetails};

pub async fn get(happ_id: String) -> Result<()> {
    let mut agent = HHAAgent::spawn(None).await?;

    let hosts: Vec<HoloportDetails> = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("get_hosts"),
            happ_id.clone(),
        )
        .await?;

    println!("===================");
    println!("All Hosts for Happ ID {} are: ", happ_id);
    println!("{:#?}", hosts);
    println!("===================");

    Ok(())
}
