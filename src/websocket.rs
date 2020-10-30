use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use holochain::conductor::api::{AdminRequest, AdminResponse};
use holochain_types::{
    app::{AppId, InstallAppDnaPayload, InstallAppPayload},
    dna::AgentPubKey,
};
use holochain_websocket::{websocket_connect, WebsocketConfig, WebsocketSender};
use tracing::{debug, info, instrument};
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
            AdminResponse::GenerateAgentPubKey(agent_pubkey) => Ok(agent_pubkey),
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    }

    // TODO: write and use get_installed_happs
    #[instrument(skip(self), err)]
    pub async fn get_installed_happs(&mut self) -> Result<AdminResponse> {
        todo!()
    }

    #[instrument(skip(self, agent_key), err)]
    pub async fn install_happ(
        &mut self,
        happ: &Happ,
        agent_key: AgentPubKey,
        happ_port: u16,
    ) -> Result<()> {
        self.instance_dna_for_agent(happ, agent_key).await?;
        self.activate_app(&happ.app_id).await?;
        self.attach_app_interface(happ_port).await?;
        info!(?happ.app_id, "installed hApp");
        Ok(())
    }

    #[instrument(skip(self, agent_key), err)]
    async fn instance_dna_for_agent(
        &mut self,
        happ: &Happ,
        agent_key: AgentPubKey,
    ) -> Result<AdminResponse> {
        let path = crate::download_file(&happ.dna_url).await?;
        let dna = InstallAppDnaPayload {
            nick: happ.app_id.clone(),
            path: path.to_path_buf(),
            properties: None,
            membrane_proof: None,
        };
        let payload = InstallAppPayload {
            app_id: happ.app_id.clone(),
            agent_key,
            dnas: vec![dna],
        };
        let msg = AdminRequest::InstallApp(Box::new(payload));
        let response = self.send(msg).await?;
        Ok(response)
    }

    #[instrument(skip(self), err)]
    async fn activate_app(&mut self, app_id: &AppId) -> Result<AdminResponse> {
        let msg = AdminRequest::ActivateApp {
            app_id: app_id.clone(),
        };
        self.send(msg).await
    }

    #[instrument(skip(self), err)]
    async fn attach_app_interface(&mut self, happ_port: u16) -> Result<AdminResponse> {
        let msg = AdminRequest::AttachAppInterface {
            port: Some(happ_port),
        };
        self.send(msg).await
    }

    async fn send(&mut self, msg: AdminRequest) -> Result<AdminResponse> {
        let response = self
            .tx
            .request(msg)
            .await
            .context("failed to send message")?;
        match response {
            AdminResponse::Error(error) => Err(anyhow!("error: {:?}", error)),
            _ => {
                debug!("send successful");
                Ok(response)
            }
        }
    }
}
