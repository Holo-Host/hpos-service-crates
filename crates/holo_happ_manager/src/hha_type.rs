use holochain_types::prelude::{holochain_serial, SerializedBytes};
use serde::{Deserialize, Serialize};

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
    pub categories: Vec<String>, //
    #[serde(default)]
    pub jurisdictions: Vec<String>, //
    #[serde(default)]
    pub exclude_jurisdictions: bool, //
    #[serde(default)]
    pub hosting_prices: HostingPrices,
    #[serde(default)]
    pub login_config: LoginConfig,
    #[serde(default)] // default Option is None
    pub uid: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone, Default)]
pub struct LoginConfig {
    pub display_publisher_name: bool,
    pub registration_info_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone, Default)]
pub struct HostingPrices {
    pub cpu: String,
    pub storage: String,
    pub bandwidth: String,
}

#[derive(Debug, Serialize, Deserialize, SerializedBytes, Clone)]
pub struct DnaResource {
    pub hash: String, // hash of the dna, not a stored dht address
    pub src_url: String,
    pub nick: String,
}
