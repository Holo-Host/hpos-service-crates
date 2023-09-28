use anyhow::Result;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::holofuel_types::{Reserve, ReserveSalePrice};
use hpos_hc_connect::HolofuelAgent;

pub async fn get_setting() -> Result<()> {
    let mut agent = HolofuelAgent::connect().await?;
    let result = agent
        .zome_call(
            ZomeName::from("reserves"),
            FunctionName::from("get_all_reserve_accounts_details"),
            ExternIO::encode(())?,
        )
        .await?;

    let reserve: Vec<Reserve> = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("All Reserve details: ");
    println!("Balance: {:?}", reserve);
    println!("===================");

    Ok(())
}

pub async fn get_sale_price() -> Result<()> {
    let mut agent = HolofuelAgent::connect().await?;
    let result = agent
        .zome_call(
            ZomeName::from("reserves"),
            FunctionName::from("get_my_sale_price"),
            ExternIO::encode(())?,
        )
        .await?;

    let reserve: ReserveSalePrice = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("Reserve Sale Price: ");
    println!("Balance: {:?}", reserve);
    println!("===================");

    Ok(())
}
