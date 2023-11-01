use holochain_types::prelude::{holochain_serial, ActionHashB64, AgentPubKeyB64, SerializedBytes};
use holofuel_types::fuel::Fuel;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HappAndHost {
    pub happ_id: ActionHashB64,
    pub holoport_id: String,
}

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

#[derive(Debug, Clone, Deserialize)]
pub struct HoloportId(pub String);

#[derive(Debug, Clone, Deserialize)]
pub struct HoloportDetails {
    pub host_pub_key: AgentPubKeyB64,
    pub holoport_id: HoloportId,
    pub preferences: Option<HappPreferences>,
    pub preferences_hash: Option<ActionHashB64>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes)]
pub struct PresentedHappBundle {
    pub id: ActionHashB64,
    pub provider_pubkey: AgentPubKeyB64,
    pub is_draft: bool,
    pub is_paused: bool,
    pub uid: Option<String>,
    pub bundle_url: String,
    pub ui_src_url: Option<String>,
    // pub dnas: Vec<DnaResource>,
    pub hosted_urls: Vec<String>,
    pub name: String,
    pub logo_url: Option<String>,
    pub description: String,
    pub categories: Vec<String>,
    pub jurisdictions: Vec<String>,
    pub exclude_jurisdictions: bool,
    pub publisher_pricing_pref: PublisherPricingPref,
    pub login_config: LoginConfig,
    pub special_installed_app_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct PublisherPricingPref {
    pub cpu: Fuel,
    pub storage: Fuel,
    pub bandwidth: Fuel,
}
impl Default for PublisherPricingPref {
    fn default() -> Self {
        Self {
            cpu: Fuel::new(0),
            storage: Fuel::new(0),
            bandwidth: Fuel::new(0),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone, Default)]
pub struct LoginConfig {
    pub display_publisher_name: bool,
    pub registration_info_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct HappInput {
    #[serde(default)]
    pub hosted_urls: Vec<String>,
    pub bundle_url: String,
    #[serde(default)]
    pub ui_src_url: Option<String>,
    #[serde(default)]
    pub special_installed_app_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub logo_url: Option<String>,
    #[serde(default)]
    pub dnas: Vec<DnaResource>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub jurisdictions: Vec<String>,
    #[serde(default)]
    pub exclude_jurisdictions: bool,
    #[serde(default)]
    pub publisher_pricing_pref: PublisherPricingPref,
    #[serde(default)]
    pub login_config: LoginConfig,
    #[serde(default)] // default Option is None
    pub uid: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct DnaResource {
    pub hash: String, // hash of the dna, not a stored dht address
    pub src_url: String,
    pub nick: String,
}
