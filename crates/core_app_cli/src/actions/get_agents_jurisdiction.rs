use anyhow::Result;
use holochain_types::{
    dna::{AgentPubKey, AgentPubKeyB64},
    prelude::{FunctionName, ZomeName},
};
use hpos_hc_connect::{app_connection::CoreAppRoleName, hha_agent::CoreAppAgent};

pub async fn get(agent_pubkey: String) -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;
    let pubkey_bytes: AgentPubKey = AgentPubKeyB64::from_b64_str(&agent_pubkey.clone())?.into();

    let agent_jurisdictions: Vec<(AgentPubKey, Option<String>)> = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("get_agents_jurisdiction"),
            vec![pubkey_bytes],
        )
        .await?;

    if let Some(j) = &agent_jurisdictions[0].1 {
        println!("===================");
        println!("Jurisdiction for agent {:?}: {:?}", agent_pubkey, j);
        println!("===================");
    } else {
        println!("Error: No jurisdiction found for agent {:?}", agent_pubkey)
    }

    Ok(())
}
