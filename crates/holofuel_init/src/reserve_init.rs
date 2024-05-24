use anyhow::Result;
use holochain_types::dna::hash_type::Agent;
use holochain_types::dna::{ActionHash, HoloHash};
use holochain_types::prelude::{FunctionName, ZomeName};
use hpos_hc_connect::app_connection::CoreAppRoleName;
use hpos_hc_connect::hf_agent::HfAgent;
use hpos_hc_connect::holofuel_types::{Reserve, ReserveSalePrice, ReserveSettingFile};
use tracing::{info, instrument, trace, warn};

#[instrument(err, skip(agent))]
pub async fn set_up_reserve(mut agent: HfAgent, agent_pub_key: HoloHash<Agent>) -> Result<()> {
    trace!("Setting up reserve settings...");
    match ReserveSettingFile::load_happ_file() {
        Ok(reserve_settings_file) => {
            let agent_pub_key_byte_arr: [u8; 32] =
                <[u8; 32]>::try_from(agent_pub_key.get_raw_32())?;

            let reserve_settings =
                reserve_settings_file.into_reserve_settings(agent_pub_key_byte_arr.into());

            trace!("Getting all reserve account details");

            let reserve: Vec<Reserve> = agent
                .app
                .zome_call_typed(
                    CoreAppRoleName::Holofuel.into(),
                    ZomeName::from("reserves"),
                    FunctionName::from("get_all_reserve_accounts_details"),
                    (),
                )
                .await?;

            if reserve.is_empty() {
                trace!("Setting reserve details");
                // Setting initial reserve account details
                let _: Reserve = agent
                    .app
                    .zome_call_typed(
                        CoreAppRoleName::Holofuel.into(),
                        ZomeName::from("reserves"),
                        FunctionName::from("register_reserve_account"),
                        reserve_settings,
                    )
                    .await?;

                // Setting reserve sales price to 1
                // Current expectation is a 1 to 1 conversion
                // 1HF = 1HOT
                let _: ActionHash = agent
                    .app
                    .zome_call_typed(
                        CoreAppRoleName::Holofuel.into(),
                        ZomeName::from("reserves"),
                        FunctionName::from("set_sale_price"),
                        ReserveSalePrice {
                            latest_unit_price: "1".to_string(),
                            inputs_used: vec![],
                        },
                    )
                    .await?;
            } else {
                info!("Reserve settings: {:?}", reserve);
            }
        }
        Err(e) => warn!("{}", e),
    }

    Ok(())
}
