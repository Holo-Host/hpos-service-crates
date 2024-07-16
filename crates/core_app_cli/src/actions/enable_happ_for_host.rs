use anyhow::Result;
use holochain_types::prelude::ActionHashB64;
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::hha_agent::CoreAppAgent;
use hpos_hc_connect::hha_types::HappAndHost;

pub async fn get(happ_id: String, host_id: String) -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;

    let holo_hash = ActionHashB64::from_b64_str(&happ_id.clone())
        .expect("Failed to serialize string into ActionHashB4");

    let payload = HappAndHost {
        happ_id: holo_hash,
        holoport_id: host_id.clone(),
    };

    let _: () = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("enable_happ"),
            payload,
        )
        .await?;

    println!("===================");
    println!("Enabled Happ ID {} for Host {}: ", happ_id, host_id);
    println!("Fetching happ preference hash...");

    crate::get_happ_pref_for_host::get(happ_id, host_id).await?;

    println!("===================");

    Ok(())
}
