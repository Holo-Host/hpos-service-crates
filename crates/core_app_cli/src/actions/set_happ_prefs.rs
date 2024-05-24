use anyhow::Result;
use holochain_types::prelude::{ActionHashB64, FunctionName, ZomeName};
use holofuel_types::fuel::Fuel;
use hpos_hc_connect::{
    app_connection::CoreAppRoleName,
    hha::HHAAgent,
    hha_types::{HappPreferences, SetHappPreferencesInput},
};
use std::{str::FromStr, time::Duration};

pub async fn get(
    happ_id: String,
    price_compute: String,
    price_storage: String,
    price_bandwidth: String,
    max_fuel_before_invoice: String,
    max_time_before_invoice_sec: String,
    max_time_before_invoice_ms: String,
) -> Result<()> {
    let mut agent = HHAAgent::spawn(None).await?;

    let max_time_sec = max_time_before_invoice_sec
        .parse::<u64>()
        .expect("Failed to convert `max_time_before_invoice` seconds to U64.");

    let max_time_ms = max_time_before_invoice_ms
        .parse::<u32>()
        .expect("Failed to convert `max_time_before_invoice` milliseconds to U32.");

    let host_pricing_prefs = SetHappPreferencesInput {
        happ_id: ActionHashB64::from_b64_str(&happ_id)?,
        max_fuel_before_invoice: Fuel::from_str(&max_fuel_before_invoice)?,
        price_compute: Fuel::from_str(&price_compute)?,
        price_storage: Fuel::from_str(&price_storage)?,
        price_bandwidth: Fuel::from_str(&price_bandwidth)?,
        max_time_before_invoice: Duration::new(max_time_sec, max_time_ms),
    };

    let happ_prefs: HappPreferences = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("set_happ_preferences"),
            host_pricing_prefs,
        )
        .await?;

    println!("===================");
    println!("Your Published Happ Preferences are: ");
    println!("{:?}", happ_prefs);
    println!("===================");

    Ok(())
}
