use anyhow::{Context, Result};
use holochain_types::{
    dna::AgentPubKey,
    prelude::{FunctionName, ZomeName},
};
use hpos_hc_connect::{
    app_connection::CoreAppRoleName, hha_agent::CoreAppAgent, holo_config::Config,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, trace, warn};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JurisdictionRecord {
    _id: String,
    email: String,
    pub jurisdiction: String,
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

#[derive(Debug, Clone)]
pub struct HbsClient {
    pub client: reqwest::Client,
    jwt: String,
    url: String,
}
impl HbsClient {
    pub async fn connect() -> Result<Self> {
        let client = reqwest::Client::builder().build()?;
        let hbs_id: String =
            std::env::var("HBS_AUTH_ID").expect("HBS_AUTH_ID must be set in the env");
        let hbs_secret: String =
            std::env::var("HBS_AUTH_SECRET").expect("HBS_AUTH_SECRET must be set in the env");
        let url = std::env::var("HBS_URL").context("Failed to read HBS_URL. Is it set in env?")?;
        let jwt = match Self::get_auth_token(&url, &client).await {
            Ok(jwt) => jwt,
            Err(err) => {
                error!("HbsClient::Failed to fetch JWT token from HBS. Using HBS_AUTH_ID: {:?} and HBS_AUTH_SECRET: {:?}.",  hbs_id, hbs_secret);
                return Err(err);
            }
        };
        Ok(Self { client, jwt, url })
    }

    pub async fn get_auth_token(hbs_url: &str, client: &reqwest::Client) -> Result<String> {
        let params = [
            ("id", std::env::var("HBS_AUTH_ID")?),
            ("secret", std::env::var("HBS_AUTH_SECRET")?),
        ];

        let request = client
            .request(
                reqwest::Method::GET,
                format!("{}/auth/api/v1/service-account", hbs_url),
            )
            .query(&params);

        match request.send().await {
            Ok(res) => {
                debug!(
                    "HbsClient::Received `service-account` response status: {}",
                    res.status()
                );
                let res = res.error_for_status()?;
                let jwt = res.text().await?;
                Ok(jwt)
            }
            Err(err) => {
                warn!(
                    "HbsClient::Call to `service-account` failed. Error: {:?}",
                    err
                );
                Err(err.into())
            }
        }
    }

    /// Handles get request to HBS server under `/registration/api/v3/my-registration` path
    /// Returns only the host's jurisdiction
    pub async fn get_registration_details() -> Result<String> {
        trace!("HbsClient::Getting registration details for Host");
        let connection = Self::connect().await?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse()?);
        let request = connection
            .client
            .request(
                reqwest::Method::GET,
                format!("{}/registration/api/v3/my-registration", connection.url),
            )
            .bearer_auth(connection.jwt.clone())
            .headers(headers);

        let response = request.send().await?;
        let response = response.error_for_status()?;

        let record: JurisdictionRecord = response.json().await?;
        trace!("HbsClient::Registration record results: {:?}", record);

        Ok(record.jurisdiction)
    }
}

#[tokio::test]
async fn get_host_registration_details_call() {
    env_logger::init();
    use dotenv::dotenv;
    dotenv().ok();

    std::env::set_var("HBS_URL", "https://hbs.dev.holotest.net".to_string());
    HbsClient::get_registration_details().await.unwrap();
}
