use anyhow::Result;
use holochain_types::prelude::Timestamp;
use holochain_types::prelude::{
    holochain_serial, EntryHashB64, ExternIO, FunctionName, SerializedBytes, ZomeName,
};
use hpos_hc_connect::holofuel_types::Pending;
use hpos_hc_connect::{CoreAppAgent, CoreAppRoleName};
use serde::{Deserialize, Serialize};

pub async fn get() -> Result<()> {
    let mut agent = CoreAppAgent::connect().await?;
    let result = agent
        .zome_call(
            CoreAppRoleName::Holofuel,
            ZomeName::from("transactor"),
            FunctionName::from("get_pending_transactions"),
            ExternIO::encode(())?,
        )
        .await?;

    let txs: Pending = rmp_serde::from_slice(result.as_bytes())?;

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
        let result = agent
            .zome_call(
                CoreAppRoleName::Holofuel,
                ZomeName::from("transactor"),
                FunctionName::from("accept_transaction"),
                ExternIO::encode(AcceptTx {
                    address: txs.invoice_pending[0].id.clone(),
                    expiration_date: None,
                })?,
            )
            .await?;
        let hash: EntryHashB64 = rmp_serde::from_slice(result.as_bytes())?;
        println!("Accepted tx: {:?}", hash);
        println!("Trying to complete, if this fails it will be completed by your schedular");
        agent
            .zome_call(
                CoreAppRoleName::Holofuel,
                ZomeName::from("transactor"),
                FunctionName::from("complete_transactions"),
                ExternIO::encode(hash)?,
            )
            .await?;
    } else {
        println!("===================");
        println!("No pending invoices");
        println!("===================");
    }

    Ok(())
}
