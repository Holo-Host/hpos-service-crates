use anyhow::Result;
use holochain_types::prelude::{ActionHash, ActionHashB64, ExternIO, FunctionName, ZomeName};
use holofuel_types::fuel::Fuel;
use hpos_hc_connect::{CoreAppAgent, CoreAppRoleName};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HappPreferences {
    pub max_fuel_before_invoice: Fuel,
    pub price_compute: Fuel,
    pub price_storage: Fuel,
    pub price_bandwidth: Fuel,
    pub max_time_before_invoice: Duration,
}

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
