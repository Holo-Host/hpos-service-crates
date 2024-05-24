use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{app_connection::CoreAppRoleName, hf_agent::HfAgent, holofuel_types::{Reserve, ReserveSalePrice}};

pub async fn get_setting() -> Result<()> {
    let mut agent = HfAgent::spawn(None).await?;

    let reserve: Vec<Reserve> = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("reserves"),
            FunctionName::from("get_all_reserve_accounts_details"),
            (),
        )
        .await?;

    println!("===================");
    println!("All Reserve details: ");
    println!("Balance: {:?}", reserve);
    println!("===================");

    Ok(())
}

pub async fn get_sale_price() -> Result<()> {
    let mut agent = HfAgent::spawn(None).await?;

    let reserve: ReserveSalePrice = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("reserves"),
            FunctionName::from("get_my_sale_price"),
            (),
        )
        .await?;

    println!("===================");
    println!("Reserve Sale Price: ");
    println!("Balance: {:?}", reserve);
    println!("===================");

    Ok(())
}
