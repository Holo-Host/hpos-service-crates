use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{app_connection::CoreAppRoleName, hf_agent::HfAgent, holofuel_types::Profile};

pub async fn get() -> Result<()> {
    let mut agent = HfAgent::spawn(None).await?;

    let profile: Profile = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("profile"),
            FunctionName::from("get_my_profile"),
            (),
        )
        .await?;

    println!("===================");
    println!("Your Profile details are: ");
    println!("Agent Pub key: {:?}", profile.agent_address);
    println!("Nickname: {:?}", profile.nickname);
    println!("Avatar: {:?}", profile.avatar_url);
    println!("===================");

    Ok(())
}
