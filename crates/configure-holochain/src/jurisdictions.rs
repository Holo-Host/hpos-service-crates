use anyhow::{anyhow, Context, Result};
use holochain_types::{
    dna::AgentPubKey,
    prelude::{ExternIO, FunctionName, Timestamp, ZomeName},
};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, holo_config::Config,
    host_keys::HostKeys,
};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

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

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationRecord {
    pub id: String,
    email: String,
    pub access_token: String,
    permissions: Vec<String>,
    pub kyc: String,
    pub jurisdiction: String,
    public_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HoloClientPayload {
    pub email: String,
    pub timestamp: u64,
    pub pub_key: String,
}

#[derive(Debug, Clone)]
pub struct HbsClient {
    hbs_url: String,
    keys: HostKeys,
}
impl HbsClient {
    pub async fn connect() -> Result<Self> {
        let hbs_url =
            std::env::var("HBS_URL").context("Failed to read HBS_URL. Is it set in env?")?;
        // Creates a keypair that contains email from config, pubkey to_holochain_encoded_agent_key and signing_key
        let keys = HostKeys::new().await?;
        Ok(Self { hbs_url, keys })
    }

    /// Handles post request to HBS server under /auth/api/v1/holo-client path
    /// Creates signature from agent's key that is verified by HBS
    /// Returns the host's registration record
    pub async fn get_host_registration(&self) -> Result<RegistrationRecord> {
        // Extracts email
        let email = self.keys.email.clone();

        // Extracts host pub key
        let pub_key = self.keys.pubkey_base36.clone();

        // Formats timestamp to the one with milisecs
        let now = Timestamp::now().as_seconds_and_nanos();
        let timestamp: u64 = <i64 as TryInto<u64>>::try_into(now.0 * 1000).unwrap()
            + <u32 as Into<u64>>::into(now.1 / 1_000_000);

        let payload = HoloClientPayload {
            email,
            timestamp,
            pub_key,
        };
        trace!("HBS `holo-client` payload: {:?}", payload);

        // Msgpack encodes payload
        let encoded_payload = ExternIO::encode(&payload)?;

        // Signs encoded bytes
        let signature = self.keys.sign(encoded_payload).await?;

        let mut response = self
            .call_holo_client(payload.clone(), signature.clone())
            .await?;
        debug!("HBS Response: {:?}", response);
        response = response.error_for_status()?;
        let mut body = response.text().await?;

        // 504 Gateway Timeout
        // here we either need to retry once more or end the script
        if body.contains("error code: 504") {
            tracing::warn!(
                "Gateway Timeout during `holo-client` call to HBS. Retrying once more..."
            );
            response = self.call_holo_client(payload, signature).await?;
            body = response.text().await?;
            if body.contains("error code: 504") {
                tracing::warn!("Gateway Timeout during `holo-client` call to HBS. Exiting...");
                return Err(anyhow!(
                    "Failed to call holo-client and fetch host jurisdiction."
                ));
            }
        }

        let result: serde_json::Value = serde_json::from_str(&body)?;
        let record: RegistrationRecord =
            serde_json::from_value(result).context("Failed to parse response body")?;
        Ok(record)
    }

    async fn call_holo_client(
        &self,
        payload: HoloClientPayload,
        signature: String,
    ) -> Result<Response> {
        let client = Client::new();
        Ok(client
            .post(format!("{}/auth/api/v1/holo-client", self.hbs_url))
            .json(&payload)
            .header("X-Signature", signature)
            .send()
            .await?)
    }
}

#[tokio::test]
async fn get_host_registration_details_call() {
    env_logger::init();
    use dotenv::dotenv;
    dotenv().ok();
    // Point HPOS_CONFIG_PATH to test config file
    std::env::set_var(
        "HPOS_CONFIG_PATH",
        "../holochain_env_setup/config/hp-primary-bzywj.json",
    );
    std::env::set_var("DEVICE_SEED_DEFAULT_PASSWORD", "pass");
    std::env::set_var("HBS_URL", "https://hbs.dev.holotest.net".to_string());
    let hbs = HbsClient::connect().await.unwrap();
    hbs.get_host_registration().await.unwrap();
}
