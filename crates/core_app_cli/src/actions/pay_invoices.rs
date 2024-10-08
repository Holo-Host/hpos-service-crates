use anyhow::Result;
use holochain_types::prelude::Timestamp;
use holochain_types::prelude::{
    holochain_serial, EntryHashB64, FunctionName, SerializedBytes, ZomeName,
};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::hha_agent::CoreAppAgent;
use hpos_hc_connect::holofuel_types::{CounterSigningResponse, Pending};
use serde::{Deserialize, Serialize};

pub async fn get() -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;

    let txs: Pending = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("transactor"),
            FunctionName::from("get_pending_transactions"),
            (),
        )
        .await?;

    #[derive(Serialize, Deserialize, Debug, SerializedBytes)]
    struct AcceptTx {
        address: EntryHashB64,
        expiration_date: Option<Timestamp>,
    }
    if !txs.invoice_pending.is_empty() {
        println!("===================");
        println!("Going to accept first transaction");
        println!("Invoices Pending: {:?}", txs.invoice_pending[0]);
        println!("===================");

        let hash: EntryHashB64 = agent
            .app
            .zome_call_typed(
                CoreAppRoleName::Holofuel.into(),
                ZomeName::from("transactor"),
                FunctionName::from("accept_transaction"),
                AcceptTx {
                    address: txs.invoice_pending[0].id.clone(),
                    expiration_date: None,
                },
            )
            .await?;

        println!("Accepted tx: {:?}", hash);
        println!("Trying to complete, if this fails it will be completed by your schedular");

        let countersigning_response: CounterSigningResponse = agent
            .app
            .zome_call_typed(
                CoreAppRoleName::Holofuel.into(),
                ZomeName::from("transactor"),
                FunctionName::from("complete_transactions"),
                hash,
            )
            .await?;

        println!("CounterSigningResponse {:?}", countersigning_response);
    } else {
        println!("===================");
        println!("No pending invoices");
        println!("===================");
    }

    Ok(())
}
