use anyhow::Result;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::holofuel_types::Pending;
use hpos_hc_connect::HolofuelAgent;

pub async fn get() -> Result<()> {
    let mut agent = HolofuelAgent::connect().await?;
    let result = agent
        .zome_call(
            ZomeName::from("transactor"),
            FunctionName::from("get_pending_transactions"),
            ExternIO::encode(())?,
        )
        .await?;

    let txs: Pending = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("Your Pending List is: ");
    println!("Invoices Pending: {:?}", txs.invoice_pending);
    println!("Invoices Declined: {:?}", txs.invoice_declined);
    println!("Promises Pending: {:?}", txs.promise_pending);
    println!("Promises Declined: {:?}", txs.promise_declined);
    println!("Accepted but now completed: {:?}", txs.accepted);
    println!(
        "Number of pending: {:?}",
        txs.invoice_pending.len() + txs.promise_pending.len()
    );
    println!(
        "Number of declined: {:?}",
        txs.invoice_declined.len() + txs.promise_declined.len()
    );
    println!(
        "Number of accepted (waiting for scheduler to complete it): {:?}",
        txs.accepted.len()
    );
    println!("===================");

    Ok(())
}
