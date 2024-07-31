use anyhow::Result;
use holochain_types::prelude::{ActionHash, ActionHashB64, FunctionName, ZomeName};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, hha_types::HappPreferences,
};

pub async fn get(pref_hash: String) -> Result<()> {
    let mut agent = CoreAppAgent::spawn(None).await?;
    let pref_holo_hash = ActionHashB64::from_b64_str(&pref_hash)
        .expect("Failed to serialize string into ActionHashB4");
    let hash = ActionHash::from(pref_holo_hash);

    let prefs: HappPreferences = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("get_specific_happ_preferences"),
            hash,
        )
        .await?;

    println!("===================");
    println!(
        "Host Preference Details for Preference Hash {} are: ",
        pref_hash
    );
    println!("{:#?}", prefs);
    println!("===================");

    Ok(())
}
