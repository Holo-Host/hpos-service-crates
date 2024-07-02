//! This crate can be used to connect to holofuel running on
//! a hpos profile that is installed by configure-holochain
//!
//! It provides HolofuelAgent that connects to the holofuel instance
//! and provide wrapper to signed zome calls.
//!
//! ### Expected Environment vars
//! ```
//! // HOLOCHAIN_DEFAULT_PASSWORD=<password to unlock holochain conductor>
//! // CORE_HAPP_FILE=<path to a config.json file used for the configure-holochain service>
//! // DEV_UID_OVERRIDE=<network-seed that is used to create new hash spaces with different holo-nixpkgs builds>
//! // LAIR_CONNECTION_URL=<string uri to lcoation of lair keystore> *OPTIONAL*
//! // HOLOCHAIN_WORKING_DIR=<path to holochains working dir> *OPTIONAL is LAIR_CONNECTION_URL is not provided*
//! ```
//! ### Example:
//!
//! ```rust
//! use hpos_hc_connect::{app_connection::CoreAppRoleName, hf_agent::HfAgent, holofuel_types::Ledger};
//! use holochain_types::prelude::{FunctionName, ZomeName};
//! pub async fn test() {
//!     let mut agent = HfAgent::spawn(None).await.unwrap();
//!
//!    let ledger: Ledger = agent
//!    .app
//!    .zome_call_typed(
//!        CoreAppRoleName::Holofuel.into(),
//!        ZomeName::from("transactor"),
//!        FunctionName::from("get_ledger"),
//!        (),
//!    )
//!    .await
//!    .unwrap();
//! }
//! ```

pub mod admin_ws;
pub mod app_connection;
pub mod hf_agent;
pub mod hha_agent;
pub mod hha_types;
pub mod holo_config;
pub mod holofuel_types;
pub mod hpos_agent;
pub mod hpos_membrane_proof;
pub mod sl_utils;
pub mod utils;
pub use admin_ws::AdminWebsocket;
pub use app_connection::AppConnection;
