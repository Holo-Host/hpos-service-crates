use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::hf_agent::HfAgent;
use hpos_hc_connect::holofuel_types::Actionable;

pub async fn get() -> Result<()> {
    let mut agent = HfAgent::spawn(None).await?;

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

    Ok(())
}
