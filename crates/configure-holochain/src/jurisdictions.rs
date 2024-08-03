use anyhow::Result;
use holochain_types::{
    dna::AgentPubKey,
    prelude::{FunctionName, ZomeName},
};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, holo_config::Config,
};
use serde::{Deserialize, Serialize};
use std::process::{Command, Output};
use tracing::debug;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HostingCriteria {
    id: String,
    jurisdiction: String,
    kyc: String,
}

pub async fn get_jurisdiction() -> Result<String> {
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost/api/v2/host/hosting_criteria")
        .send()
        .await?;
    let text = response.text().await.unwrap();
    println!(" >>> text {:?}", text);
    let hosting_criteria: HostingCriteria = serde_json::from_str(&text)?;
    debug!("hosting_criteria result {:?}", hosting_criteria);

    let output: Output = Command::new("hpos-holochain-client")
        .args(["--url=http://localhost/api/v2/", "hosting-criteria"])
        .output()?;

    let output_str = String::from_utf8_lossy(&output.stdout).to_string();
    debug!("called hpos api and got result {}", output_str);

    let hosting_criteria: HostingCriteria = serde_json::from_str(&output_str)?;
    debug!("hosting_criteria result {:?}", hosting_criteria);

    Ok(hosting_criteria.jurisdiction)
}

pub async fn update_jurisdiction_if_changed(
    config: &Config,
    hbs_jurisdiction: String,
) -> Result<()> {
    let mut agent = CoreAppAgent::spawn(Some(config)).await?;

    let host_pubkey = agent.pubkey().await?;

    let hha_jurisdiction: Option<String> = agent
        .app
        .zome_call_typed(
            CoreAppRoleName::HHA.into(),
            ZomeName::from("hha"),
            FunctionName::from("get_host_jurisdiction"),
            host_pubkey.clone(),
        )
        .await?;

    if hha_jurisdiction.is_none() || hha_jurisdiction.as_ref() != Some(&hbs_jurisdiction) {
        #[derive(Debug, Serialize)]
        pub struct SetHostJurisdictionInput {
            pub pubkey: AgentPubKey,
            pub jurisdiction: String,
        }

        let _: () = agent
            .app
            .zome_call_typed(
                CoreAppRoleName::HHA.into(),
                ZomeName::from("hha"),
                FunctionName::from("set_host_jurisdiction"),
                SetHostJurisdictionInput {
                    pubkey: host_pubkey,
                    jurisdiction: hbs_jurisdiction,
                },
            )
            .await?;
    }

    Ok(())
}
