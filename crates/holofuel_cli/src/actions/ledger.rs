use anyhow::Result;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::holofuel_types::Ledger;
use hpos_hc_connect::HolofuelAgent;

pub async fn get() -> Result<()> {
    let mut agent = HolofuelAgent::connect().await?;
    let result = agent
        .zome_call(
            ZomeName::from("transactor"),
            FunctionName::from("get_ledger"),
            ExternIO::encode(())?,
        )
        .await?;

    let ledger: Ledger = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("Your Ledger is: ");
    println!("Balance: {:?}", ledger.balance);
    println!("Promised amt: {:?}", ledger.promised);
    println!("Fees to be paid: {:?}", ledger.fees);
    println!("Available Bal: {:?}", ledger.available);
    println!("===================");

    Ok(())
}
