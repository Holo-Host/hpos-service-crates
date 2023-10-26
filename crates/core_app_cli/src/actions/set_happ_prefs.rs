use anyhow::Result;
use holochain_types::prelude::{ActionHashB64, ExternIO, FunctionName, ZomeName};
use holofuel_types::fuel::Fuel;
use hpos_hc_connect::{CoreAppAgent, CoreAppRoleName};
use serde::{Deserialize, Serialize};
use std::{str::FromStr, time::Duration};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HappPreferences {
    pub max_fuel_before_invoice: Fuel,
    pub price_compute: Fuel,
    pub price_storage: Fuel,
    pub price_bandwidth: Fuel,
    pub max_time_before_invoice: Duration,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetHappPreferencesInput {
    pub happ_id: ActionHashB64,
    pub max_fuel_before_invoice: Fuel, // how much holofuel to accumulate before sending invoice
    pub price_compute: Fuel,
    pub price_storage: Fuel,
    pub price_bandwidth: Fuel,
    pub max_time_before_invoice: Duration, // how much time to allow to pass before sending invoice even if fuel trigger not reached.
}

pub async fn get(
    happ_id: String,
    price_compute: String,
    price_storage: String,
    price_bandwidth: String,
    max_fuel_before_invoice: String,
    max_time_before_invoice: (String, String),
) -> Result<()> {
    println!(
        " >>>>>>>>>>>>> in `set_happ_prefs` helper:
        happ_id ({:?}),
        price_compute ({:?}),
        price_storage ({:?}),
        price_bandwidth ({:?}),
        max_fuel_before_invoice ({:?}),
        max_time_before_invoice ({:?})",
        happ_id,
        price_compute,
        price_storage,
        price_bandwidth,
        max_fuel_before_invoice,
        max_time_before_invoice
    );

    let mut agent = CoreAppAgent::connect().await?;

    max_time_before_invoice = max_time_before_invoice
        .parse::<(String, String)>()
        .expect("Failed to convert `max_time_before_invoice` param to string tuple.");
    println!(
        " >>>>>>>>>>>>> max_time_before_invoice {:?} ",
        max_time_before_invoice
    );

    let max_time_sec = max_time_before_invoice
        .0
        .parse::<u64>()
        .expect("Failed to convert `max_time_before_invoice` seconds to U64.");
    println!(" >>>>>>>>>>>>> max_time_sec {:?} ", max_time_sec);

    let max_time_ms = max_time_before_invoice
        .1
        .parse::<u32>()
        .expect("Failed to convert `max_time_before_invoice` milliseconds to U32.");
    println!(" >>>>>>>>>>>>> max_time_ms {:?} ", max_time_ms);

    let host_pricing_prefs = SetHappPreferencesInput {
        happ_id: ActionHashB64::from_b64_str(&happ_id)?,
        max_fuel_before_invoice: Fuel::from_str(&max_fuel_before_invoice)?,
        price_compute: Fuel::from_str(&price_compute)?,
        price_storage: Fuel::from_str(&price_storage)?,
        price_bandwidth: Fuel::from_str(&price_bandwidth)?,
        max_time_before_invoice: Duration::new(max_time_sec, max_time_ms),
    };
    println!(
        " >>>>>>>>>>>>> host_pricing_prefs {:?} ",
        host_pricing_prefs
    );

    let result = agent
        .zome_call(
            CoreAppRoleName::HHA,
            ZomeName::from("hha"),
            FunctionName::from("set_happ_preferences"),
            ExternIO::encode(host_pricing_prefs)?,
        )
        .await?;
    println!(" >>>>>>>>>>>>> result {:?} ", result);

    let happ_prefs: HappPreferences = rmp_serde::from_slice(result.as_bytes())?;
    println!(" >>>>>>>>>>>>> happ_prefs {:?} ", happ_prefs);

    println!("===================");
    println!("Your Published Happ Preferences are: ");
    println!("{:?}", happ_prefs);
    println!("===================");

    Ok(())
}
