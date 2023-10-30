use anyhow::Result;
use holochain_types::prelude::{ActionHash, ActionHashB64, ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::{hha_types::HappPreferences, CoreAppAgent, CoreAppRoleName};

pub async fn get(pref_hash: String) -> Result<()> {
    let mut agent = CoreAppAgent::connect().await?;
    let pref_holo_hash = ActionHashB64::from_b64_str(&pref_hash)?;
    let hash = ActionHash::from(pref_holo_hash);

    let result = agent
        .zome_call(
            CoreAppRoleName::HHA,
            ZomeName::from("hha"),
            FunctionName::from("get_specific_happ_preferences"),
            ExternIO::encode(hash)?,
        )
        .await?;

    let prefs: HappPreferences = rmp_serde::from_slice(result.as_bytes())?;

    println!("===================");
    println!("All Hosts for Preference Hash {} are: ", pref_hash);
    println!("{:#?}", prefs);
    println!("===================");

    Ok(())
}
