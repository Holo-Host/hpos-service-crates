use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use holochain::conductor::api::{AdminRequest, AdminResponse};
use holochain_types::{
    app::{InstallAppDnaPayload, InstallAppPayload, InstalledAppId},
    dna::AgentPubKey,
};
use holochain_websocket::{websocket_connect, WebsocketConfig, WebsocketSender};
use tracing::{debug, info, instrument, trace};
use url::Url;

use crate::Happ;

#[derive(Clone)]
pub struct AdminWebsocket {
    tx: WebsocketSender,
}

impl AdminWebsocket {
    #[instrument(err)]
    pub async fn connect(admin_port: u16) -> Result<Self> {
        let url = format!("ws://localhost:{}/", admin_port);
        let url = Url::parse(&url).context("invalid ws:// URL")?;
        let websocket_config = Arc::new(WebsocketConfig::default());
        let (tx, _rx) = websocket_connect(url.clone().into(), websocket_config).await?;
        Ok(Self { tx })
    }

    #[instrument(skip(self), err)]
    pub async fn generate_agent_pubkey(&mut self) -> Result<AgentPubKey> {
        let response = self.send(AdminRequest::GenerateAgentPubKey).await?;
        match response {
            AdminResponse::AgentPubKeyGenerated(agent_pubkey) => Ok(agent_pubkey),
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    }

    #[instrument(skip(self), err)]
    pub async fn attach_app_interface(&mut self, happ_port: u16) -> Result<AdminResponse> {
        info!(port = ?happ_port, "starting app interface");
        let msg = AdminRequest::AttachAppInterface {
            port: Some(happ_port),
        };
        self.send(msg).await
    }

    // TODO: use list_installed_happs
    #[instrument(skip(self), err)]
    pub async fn list_installed_happs(&mut self) -> Result<Vec<InstalledAppId>> {
        let response = self.send(AdminRequest::ListActiveApps).await?;
        match response {
            AdminResponse::ActiveAppsListed(app_ids) => Ok(app_ids),
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    }

    #[instrument(
        skip(self, happ, agent_key),
        fields(?happ.installed_app_id),
    )]
    pub async fn install_happ(&mut self, happ: &Happ, agent_key: AgentPubKey) -> Result<()> {
        debug!(?agent_key);
        self.instance_dna_for_agent(happ, agent_key).await?;
        self.activate_app(happ).await?;
        info!(?happ.installed_app_id, "installed hApp");
        Ok(())
    }

    #[instrument(
        err,
        skip(self, happ, agent_key),
        fields(?happ.installed_app_id)
    )]
    async fn instance_dna_for_agent(
        &mut self,
        happ: &Happ,
        agent_key: AgentPubKey,
    ) -> Result<AdminResponse> {
        let file = crate::download_file(&happ.dna_url)
            .await
            .context("failed to download DNA archive")?;
        let dna = InstallAppDnaPayload {
            nick: happ.id_with_version(),
            path: file.path().to_path_buf(),
            properties: None,
            membrane_proof: None,
        };
        let payload = InstallAppPayload {
            installed_app_id: happ.id_with_version(),
            agent_key,
            dnas: vec![dna],
        };
        let msg = AdminRequest::InstallApp(Box::new(payload));
        let response = self.send(msg).await?;
        Ok(response)
    }

    #[instrument(skip(self), err)]
    async fn activate_app(&mut self, happ: &Happ) -> Result<AdminResponse> {
        let msg = AdminRequest::ActivateApp {
            installed_app_id: happ.id_with_version(),
        };
        self.send(msg).await
    }

    #[instrument(skip(self), err)]
    async fn send(&mut self, msg: AdminRequest) -> Result<AdminResponse> {
        let response = self
            .tx
            .request(msg)
            .await
            .context("failed to send message")?;
        match response {
            AdminResponse::Error(error) => Err(anyhow!("error: {:?}", error)),
            _ => {
                trace!("send successful");
                Ok(response)
            }
        }
    }
}
