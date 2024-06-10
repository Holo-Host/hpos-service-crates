use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hf_agent::HfAgent, holofuel_types::Transaction,
};

pub async fn get() -> Result<()> {
    let mut agent = HfAgent::spawn(None).await?;

    let txs: Vec<Transaction> = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("transactor"),
            FunctionName::from("get_completed_transactions"),
            (),
        )
        .await?;

    println!("===================");
    println!("Your Completed List is: ");
    for tx in &txs {
        println!("{:?}", tx);
    }
    println!("Number of completed tx: {:?}", txs.len());
    println!("===================");

    Ok(())
}
