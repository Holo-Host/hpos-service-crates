use anyhow::Result;
use holochain_types::{
    dna::{AgentPubKey, AgentPubKeyB64},
    prelude::{FunctionName, ZomeName},
};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, hha_types::PresentedHappBundle,
};

pub async fn get(agent_pubkey: String) -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;
    let pubkey_bytes: AgentPubKey = AgentPubKeyB64::from_b64_str(&agent_pubkey.clone())?.into();

    let agent_jurisdiction: Vec<(AgentPubKey, Option<String>)> = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("get_agents_jurisdiction"),
            vec![pubkey_bytes],
        )
        .await?[0];

    if let Some(d) = agent_jurisdiction.1 {
        // Please do not change this print and if you do see that the nightly tests that depend on this print are updated as well
        println!("===================");
        println!(
            "Hosting Preference for agent {:?}: {}",
            agent_pubkey, jurisdiction
        );
        println!("===================");
    } else {
        println!("Error: No jurisdiction found for agent {:?}", agent_pubkey)
    }

    Ok(())
}
