use anyhow::Result;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::holofuel_types::Actionable;
use hpos_hc_connect::HolofuelAgent;

pub async fn get() -> Result<()> {
    let mut agent = HolofuelAgent::connect().await?;
    let result = agent
        .zome_call(
            ZomeName::from("transactor"),
            FunctionName::from("get_actionable_transactions"),
            ExternIO::encode(())?,
        )
        .await?;

    let txs: Actionable = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("Your Actionable List is: ");
    println!("Invoices: {:?}", txs.invoice_actionable);
    println!("Promises: {:?}", txs.promise_actionable);
    println!(
        "Number of actionable: {:?}",
        txs.invoice_actionable.len() + txs.promise_actionable.len()
    );
    println!("===================");

    Ok(())
}
