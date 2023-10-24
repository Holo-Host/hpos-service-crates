use anyhow::Result;
use holochain_types::prelude::{ActionHashB64, AgentPubKeyB64, ExternIO, FunctionName, ZomeName};
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

#[derive(Debug, Clone, Deserialize)]
pub struct HoloportId(pub String);

#[derive(Debug, Clone, Deserialize)]
pub struct HoloportDetails {
    pub host_pub_key: AgentPubKeyB64,
    pub holoport_id: HoloportId,
    pub preferences: Option<HappPreferences>,
    pub preferences_hash: Option<ActionHashB64>,
}

pub async fn get(happ_id: String) -> Result<()> {
    println!(" >>>>>>>>>>>>> happ_id {:?} ", happ_id);

    let mut agent = CoreAppAgent::connect().await?;

    let result = agent
        .zome_call(
            CoreAppRoleName::HHA,
            ZomeName::from("hha"),
            FunctionName::from("get_hosts"),
            ExternIO::encode(happ_id.clone())?,
        )
        .await?;
    println!(" >>>>>>>>>>>>> result {:?} ", result);

    let hosts: Vec<HoloportDetails> = rmp_serde::from_slice(result.as_bytes())?;
    println!(" >>>>>>>>>>>>> hosts {:?} ", hosts);

    println!("===================");
    println!("All Hosts for Happ ID {} are: ", happ_id);
    println!("{:#?}", hosts);
    println!("===================");

    Ok(())
}
