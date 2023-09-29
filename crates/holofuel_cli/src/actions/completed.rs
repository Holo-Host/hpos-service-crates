use anyhow::Result;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::holofuel_types::Transaction;
use hpos_hc_connect::HolofuelAgent;

pub async fn get() -> Result<()> {
    let mut agent = HolofuelAgent::connect().await?;
    let result = agent
        .zome_call(
            ZomeName::from("transactor"),
            FunctionName::from("get_completed_transactions"),
            ExternIO::encode(())?,
        )
        .await?;

    let txs: Vec<Transaction> = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("Your Completed List is: ");
    for tx in &txs {
        println!("{:?}", tx);
    }
    println!("Number of completed tx: {:?}", txs.len());
    println!("===================");

    Ok(())
}
