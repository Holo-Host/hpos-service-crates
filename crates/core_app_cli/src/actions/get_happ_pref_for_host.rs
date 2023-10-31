use anyhow::Result;
use holochain_types::prelude::{ExternIO, FunctionName, ZomeName};
use hpos_hc_connect::{hha_types::HoloportDetails, CoreAppAgent, CoreAppRoleName};

pub async fn get(happ_id: String, host_id: String) -> Result<()> {
    let mut agent = CoreAppAgent::connect().await?;

    let result = agent
        .zome_call(
            CoreAppRoleName::HHA,
            ZomeName::from("hha"),
            FunctionName::from("get_hosts"),
            ExternIO::encode(happ_id.clone())?,
        )
        .await?;

    let hosts: Vec<HoloportDetails> = rmp_serde::from_slice(result.as_bytes())?;

    let found = hosts.into_iter().find(|h| h.holoport_id.0 == host_id);

    if let Some(d) = found {
        if let Some(p) = d.preferences_hash {
            println!("{:#?}", p)
        }
    }

    Ok(())
}
