use super::holo_config::Happ;
use super::hpos_agent::Agent;
use super::hpos_membrane_proof::MembraneProofs;
use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AdminRequest, AdminResponse, AppStatusFilter};
use holochain_types::app::{AppBundleSource, InstallAppPayload, InstalledAppId};
use holochain_websocket::{connect, WebsocketConfig, WebsocketSender};
use std::{env, sync::Arc};
use tracing::{debug, info, instrument, trace};
use url::Url;

#[derive(Clone)]
pub struct AdminWebsocket {
    tx: WebsocketSender,
}

impl AdminWebsocket {
    /// Initializes websocket connection to holochain's admin interface
    #[instrument(err)]
    pub async fn connect(admin_port: u16) -> Result<Self> {
        let url = format!("ws://localhost:{}/", admin_port);
        let url = Url::parse(&url).context("invalid ws:// URL")?;
        let websocket_config = Arc::new(WebsocketConfig::default());
        let (tx, _rx) = again::retry(|| {
            let websocket_config = Arc::clone(&websocket_config);
            connect(url.clone().into(), websocket_config)
        })
        .await?;

        Ok(Self { tx })
    }

    pub async fn attach_app_interface(&mut self, happ_port: u16) -> Result<AdminResponse> {
        info!(port = ?happ_port, "starting app interface");
        let msg = AdminRequest::AttachAppInterface {
            port: Some(happ_port),
        };
        self.send(msg).await
    }

    pub async fn list_app(
        &mut self,
        status_filter: Option<AppStatusFilter>,
    ) -> Result<Vec<InstalledAppId>> {
        let response = self.send(AdminRequest::ListApps { status_filter }).await?;
        match response {
            AdminResponse::AppsListed(info) => {
                Ok(info.iter().map(|i| i.installed_app_id.to_owned()).collect())
            }
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    }

    pub async fn list_running_app(&mut self) -> Result<Vec<InstalledAppId>> {
        let mut running = self.list_app(Some(AppStatusFilter::Running)).await?;
        let mut enabled = self.list_app(Some(AppStatusFilter::Enabled)).await?;
        running.append(&mut enabled);
        Ok(running)
    }

    #[instrument(skip(self, happ, membrane_proofs, agent))]
    pub async fn install_and_activate_happ(
        &mut self,
        happ: &Happ,
        membrane_proofs: MembraneProofs,
        agent: Agent,
    ) -> Result<()> {
        let source = happ.source().await?;
        self.install_happ(happ, source, membrane_proofs, agent)
            .await?;
        self.activate_app(happ).await?;
        debug!("installed & activated hApp: {}", happ.id());
        Ok(())
    }

    #[instrument(err, skip(self, happ, source, membrane_proofs, agent))]
    async fn install_happ(
        &mut self,
        happ: &Happ,
        source: AppBundleSource,
        membrane_proofs: MembraneProofs,
        agent: Agent,
    ) -> Result<()> {
        let mut agent_key = agent.admin.key.clone();

        if let Some(admin) = &happ.agent_override_details().await? {
            agent_key = admin.key.clone();
        };

        let payload = if let Ok(id) = env::var("DEV_UID_OVERRIDE") {
            debug!("using network_seed to install: {}", id);
            InstallAppPayload {
                agent_key,
                installed_app_id: Some(happ.id()),
                source,
                membrane_proofs,
                network_seed: Some(id),
            }
        } else {
            debug!("using default network_seed to install");
            InstallAppPayload {
                agent_key,
                installed_app_id: Some(happ.id()),
                source,
                membrane_proofs,
                network_seed: None,
            }
        };

        let msg = AdminRequest::InstallApp(Box::new(payload));
        match self.send(msg).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.to_string().contains("AppAlreadyInstalled") {
                    return Ok(());
                }
                Err(e)
            }
        }
    }

    #[instrument(skip(self, happ))]
    pub async fn activate_happ(&mut self, happ: &Happ) -> Result<()> {
        self.activate_app(happ).await?;
        debug!("activated hApp: {}", happ.id());
        Ok(())
    }

    #[instrument(skip(self), err)]
    async fn activate_app(&mut self, happ: &Happ) -> Result<AdminResponse> {
        let msg = AdminRequest::EnableApp {
            installed_app_id: happ.id(),
        };
        self.send(msg).await
    }

    #[instrument(skip(self), err)]
    pub async fn uninstall_app(&mut self, installed_app_id: &str) -> Result<AdminResponse> {
        let msg = AdminRequest::UninstallApp {
            installed_app_id: installed_app_id.to_string(),
        };
        self.send(msg).await
    }

    #[instrument(skip(self))]
    pub async fn send(&mut self, msg: AdminRequest) -> Result<AdminResponse> {
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
