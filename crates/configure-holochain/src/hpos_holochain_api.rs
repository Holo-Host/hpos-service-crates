use anyhow::Result;
use serde::Deserialize;
use std::process::{Command, Output};
use tracing::debug;

#[derive(Debug, Deserialize)]
struct HostingCriteria {
    id: String,
    jurisdiction: String,
    kyc: String,
}

pub async fn get_jurisdiction() -> Result<String> {
    debug!("in get_jurisdiction");

    let output: Output = Command::new("/run/current-system/sw/bin/hpos-holochain-client")
        .args(&["--url=http://localhost/holochain-api/", "hosting-criteria"])
        .output()?;

    debug!("called hpos-holochain-client");

    let output_str = String::from_utf8_lossy(&output.stdout).to_string();        

    debug!("output_str: {}", output_str);

    let hosting_criteria: HostingCriteria = serde_json::from_str(&output_str)?;

    debug!("hosting_criteria.jurisdiction: {}", &hosting_criteria.jurisdiction);

    Ok(hosting_criteria.jurisdiction)
}
