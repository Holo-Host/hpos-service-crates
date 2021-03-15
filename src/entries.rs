use hc_utils::{WrappedAgentPubKey, WrappedHeaderHash};
use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize, Debug, Clone)]
pub struct DnaResource {
    pub hash: String, // hash of the dna, not a stored dht address
    pub src_url: String,
    pub nick: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct HappBundle {
    pub hosted_url: String,
    pub happ_alias: String,
    pub ui_src_url: String,
    pub name: String,
    pub dnas: Vec<DnaResource>,
}
#[derive(Deserialize, Debug)]
pub struct HappBundleDetails {
    pub happ_id: WrappedHeaderHash,
    pub happ_bundle: HappBundle,
    pub provider_pubkey: WrappedAgentPubKey,
}

#[derive(Serialize, Debug, Clone)]
pub struct Preferences {
    pub max_fuel_before_invoice: f64,
    pub max_time_before_invoice: Vec<u64>,
    pub price_compute: f64,
    pub price_storage: f64,
    pub price_bandwidth: f64,
}

#[derive(Serialize, Debug, Clone)]
pub struct InstallHappBody {
    pub happ_id: String,
    pub preferences: Preferences,
}

#[derive(Serialize, Debug, Clone)]
pub struct AddHostBody {
    pub happ_ids: Vec<WrappedHeaderHash>,
    pub host_id: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct RemoveHostBody {
    pub host_id: String,
}