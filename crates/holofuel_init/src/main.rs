use anyhow::{anyhow, Context, Result};
use holochain_types::{
    dna::EntryHashB64,
    prelude::{
        hash_type::Agent, holochain_serial, FunctionName, HoloHashB64, SerializedBytes, ZomeName,
    },
};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName,
    hf_agent::HfAgent,
    holofuel_types::{Profile, ReserveSettingFile},
};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{debug, info, Level};
use tracing_subscriber::FmtSubscriber;
mod reserve_init;

/// Initialize the holofuel app on a holochain instance server
/// Holochain app require one zome call to initialize the init function
/// Holofuel has a fee_collection function scheduled on init
/// This is why we will be setting a profile name for holofuel the holofuel instance
#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Start initializing the holofuel instance");

    let mut agent = HfAgent::spawn(None).await?;

    #[derive(Serialize, Deserialize, Debug, SerializedBytes)]
    pub struct ProfileInput {
        pub nickname: Option<String>,
        pub avatar_url: Option<String>,
    }

    let apk = agent.pubkey().await?;
    if let Some(ek) = expect_pubkey() {
        if ek != apk.clone().into() {
            return Err(anyhow!(
                "Unexpected agent {:?} found on this server, expected: {:?} ",
                apk,
                ek
            ));
        }
    }

    let fpk = fee_collector_pubkey()?;
    let mut nickname = Some("Holo Account".to_string());
    if fpk == apk.clone().into() {
        nickname = Some("Holo Fee Collector".to_string());
    }
    if ReserveSettingFile::load_happ_file().is_ok() {
        nickname = Some("HOT Reserve".to_string());
    }
    let profile: Profile = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::Holofuel.into(),
            ZomeName::from("profile"),
            FunctionName::from("get_my_profile"),
            (),
        )
        .await?;
    // is a profile name already set you are not allowed to update it
    if profile.nickname.is_none() {
        debug!("Setting nickname as {:?}", nickname);
        let _: EntryHashB64 = agent
            .app
            .zome_call_typed(
                CoreAppRoleName::Holofuel.into(),
                ZomeName::from("profile"),
                FunctionName::from("update_my_profile"),
                ProfileInput {
                    nickname,
                    avatar_url: None,
                },
            )
            .await?;
        info!("Profile name set successfully");
    } else {
        info!("Profile name already set as {:?}", profile.nickname);
    }
    // initialize reserve details
    reserve_init::set_up_reserve(agent, apk).await?;
    info!("Completed initializing the holofuel instance");
    Ok(())
}

pub fn fee_collector_pubkey() -> Result<HoloHashB64<Agent>> {
    let key = env::var("FEE_COLLECTOR_PUBKEY")
        .context("Failed to read FEE_COLLECTOR_PUBKEY. Is it set in env?")?;
    Ok(HoloHashB64::from_b64_str(&key)?)
}

pub fn expect_pubkey() -> Option<HoloHashB64<Agent>> {
    if let Ok(key) = env::var("EXPECT_PUBKEY") {
        if let Ok(k) = HoloHashB64::from_b64_str(&key) {
            return Some(k);
        }
    }
    None
}
