use anyhow::Result;
use holochain_types::dna::AgentPubKey;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::hha_agent::CoreAppAgent;
use hpos_hc_connect::holofuel_types::MigrationCloseStateV1Handler;

pub async fn get_my_summary() -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;

    let summary: MigrationCloseStateV1Handler = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("transactor"),
            FunctionName::from("get_my_summary"),
            (),
        )
        .await?;

    display(summary);
    Ok(())
}

pub async fn get_agent_summary(pub_key: AgentPubKey) -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;

    let summary: MigrationCloseStateV1Handler = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("transactor"),
            FunctionName::from("get_agent_summary"),
            pub_key,
        )
        .await?;
    display(summary);
    Ok(())
}

fn display(summary: MigrationCloseStateV1Handler) {
    println!("===================");
    println!("Your Summary: ");
    println!("multi_sig_authorizer: {:?}", summary.multi_sig_authorizer);
    println!("reserve_setting: {:?}", summary.reserve_setting);
    println!("reserve_sale_price: {:?}", summary.reserve_sale_price);
    println!("tx_parked_links: {:?}", summary.tx_parked_links);
    println!("cs_txs: {:?}", summary.cs_txs);
    println!(
        "incomplete_invoice_txs: {:?}",
        summary.incomplete_invoice_txs
    );
    println!(
        "incomplete_promise_txs: {:?}",
        summary.incomplete_promise_txs
    );
    println!("Number of cs tx: {:?}", summary.cs_txs.len());
    println!(
        "Number of incomplete invoice: {:?}",
        summary.incomplete_invoice_txs.len()
    );
    println!(
        "Number of incomplete promises: {:?}",
        summary.incomplete_promise_txs.len()
    );
    println!("Number of declined: {:?}", summary.number_of_declined);
    println!("opening_balance: {:?}", summary.opening_balance);
    println!("closing_balance: {:?}", summary.closing_balance);
    println!("===================");
}
