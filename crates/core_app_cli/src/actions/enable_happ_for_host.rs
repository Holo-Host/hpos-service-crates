use anyhow::Result;
use holochain_types::prelude::{ActionHash, ActionHashB64};
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::{hha_types::HappAndHost, CoreAppAgent, CoreAppRoleName};

pub async fn get(happ_id: String, host_id: String) -> Result<()> {
    let mut agent = CoreAppAgent::connect().await?;

    let holo_hash = ActionHashB64::from_b64_str(&happ_id.clone())
        .expect("Failed to serialize string into ActionHashB4");

    let payload = HappAndHost {
        happ_id: holo_hash,
        holoport_id: host_id.clone(),
    };

    let result = agent
        .zome_call(
            CoreAppRoleName::HHA,
            ZomeName::from("hha"),
            FunctionName::from("enable_happ"),
            ExternIO::encode(payload)?,
        )
        .await;

    if result.is_ok() {
        println!("===================");
        println!("Enabled Happ ID {} for Host {}: ", happ_id, host_id);
        println!("Fetching happ preference hash...");

        crate::get_happ_pref_for_host::get(happ_id, host_id).await?;

        println!("===================");
    }

    Ok(())
}
