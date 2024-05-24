use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName,
    hha::HHAAgent,
    holofuel_types::{Actionable, Pending, Transaction},
};

pub async fn get() -> Result<()> {
    let mut agent = HHAAgent::spawn(None).await?;

    let txs: Pending = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("transactor"),
            FunctionName::from("get_pending_transactions"),
            (),
        )
        .await?;

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
    let txs: Actionable = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("transactor"),
            FunctionName::from("get_actionable_transactions"),
            (),
        )
        .await?;

    println!("===================");
    println!("Your Actionable List is: ");
    println!("Invoices: {:?}", txs.invoice_actionable);
    println!("Promises: {:?}", txs.promise_actionable);
    println!(
        "Number of actionable: {:?}",
        txs.invoice_actionable.len() + txs.promise_actionable.len()
    );
    println!("===================");
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
