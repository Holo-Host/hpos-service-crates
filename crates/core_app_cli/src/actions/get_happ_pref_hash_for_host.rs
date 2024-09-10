// NB: This endpoint is used by the nightly tests.  Any change to it's input or output should also be updated there.

use anyhow::Result;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, hha_types::HoloportDetails,
};

pub async fn get(happ_id: String, host_id: String) -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;

    let hosts: Vec<HoloportDetails> = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("get_hosts"),
            happ_id.clone(),
        )
        .await?;

    let found = hosts.into_iter().find(|h| h.holoport_id.0 == host_id);

    if let Some(d) = found {
        if let Some(p) = d.preferences_hash {
            // Please do not change this print and if you do see that the nightly tests that depend on this print are updated as well
            println!("===================");
            println!("Happ Preference Hash: {:#?}", p);
            println!("===================");
        }
    } else {
        println!("Error: No preferences found for host {:?}", host_id)
    }

    Ok(())
}
