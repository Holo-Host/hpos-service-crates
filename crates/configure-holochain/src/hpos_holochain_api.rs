use anyhow::Result;
use serde::Deserialize;
use std::process::{Command, Output};

#[derive(Debug, Deserialize)]
struct HostingCriteria {
    id: String,
    jurisdiction: String,
    kyc: String,
}

pub async fn get_jurisdiction() -> Result<String> {
    let output: Output = Command::new("/run/current-system/sw/bin/hpos-holochain-client")
        .args(&["--url=http://localhost/holochain-api/", "hosting-criteria"])
        .output()?;

    let output_str = String::from_utf8_lossy(&output.stdout).to_string();

    let hosting_criteria: HostingCriteria = serde_json::from_str(&output_str)?;

    Ok(hosting_criteria.jurisdiction)
}
