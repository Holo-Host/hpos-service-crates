use anyhow::Result;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::{hha_types::HoloportDetails, CoreAppAgent, CoreAppRoleName};

pub async fn get(happ_id: String) -> Result<()> {
    let mut agent = CoreAppAgent::connect().await?;

    let result = agent
        .zome_call(
            CoreAppRoleName::HHA,
            ZomeName::from("hha"),
            FunctionName::from("get_hosts"),
            ExternIO::encode(happ_id.clone())?,
        )
        .await?;

    let hosts: Vec<HoloportDetails> = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("All Hosts for Happ ID {} are: ", happ_id);
    println!("{:#?}", hosts);
    println!("===================");

    Ok(())
}
