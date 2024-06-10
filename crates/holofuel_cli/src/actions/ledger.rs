use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{app_connection::CoreAppRoleName, hf_agent::HfAgent, holofuel_types::Ledger};

pub async fn get() -> Result<()> {
    let mut agent = HfAgent::spawn(None).await?;

    let ledger: Ledger = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("transactor"),
            FunctionName::from("get_ledger"),
            (),
        )
        .await?;

    println!("===================");
    println!("Your Ledger is: ");
    println!("Balance: {:?}", ledger.balance);
    println!("Promised amt: {:?}", ledger.promised);
    println!("Fees to be paid: {:?}", ledger.fees);
    println!("Available Bal: {:?}", ledger.available);
    println!("===================");

    Ok(())
}
