use anyhow::Result;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::{hha_types::PresentedHappBundle, CoreAppAgent, CoreAppRoleName};

pub async fn get() -> Result<()> {
    let mut agent = CoreAppAgent::connect().await?;
    let result = agent
        .zome_call(
            CoreAppRoleName::HHA,
            ZomeName::from("hha"),
            FunctionName::from("get_my_happs"),
            ExternIO::encode(())?,
        )
        .await?;

    let happs: Vec<PresentedHappBundle> = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("Your Published Happs is: ");
    println!("{:?}", happs);
    println!("===================");

    Ok(())
}
