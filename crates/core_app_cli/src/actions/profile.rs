use anyhow::Result;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::holofuel_types::Profile;
use hpos_hc_connect::{CoreAppAgent, CoreAppRoleName};

pub async fn get() -> Result<()> {
    let mut agent = CoreAppAgent::connect().await?;
    let result = agent
        .zome_call(
            CoreAppRoleName::Holofuel,
            ZomeName::from("profile"),
            FunctionName::from("get_my_profile"),
            ExternIO::encode(())?,
        )
        .await?;

    let profile: Profile = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("Your Profile details are: ");
    println!("Agent Pub key: {:?}", profile.agent_address);
    println!("Nickname: {:?}", profile.nickname);
    println!("Avatar: {:?}", profile.avatar_url);
    println!("===================");

    Ok(())
}
