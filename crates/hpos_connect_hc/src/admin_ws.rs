use crate::utils::WsPollRecv;

use super::holo_config::Happ;
use super::hpos_agent::Agent;
use super::hpos_membrane_proof::MembraneProofs;
use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppAuthenticationToken, AppAuthenticationTokenIssued, AppInfo,
    AppStatusFilter, IssueAppAuthenticationTokenPayload,
};
use holochain_types::{
    app::{InstallAppPayload, InstalledAppId},
    dna::AgentPubKey,
    websocket::AllowedOrigins,
};
use holochain_websocket::{connect, ConnectRequest, WebsocketConfig, WebsocketSender};
use std::{env, net::ToSocketAddrs, sync::Arc};
use tracing::{debug, info, instrument, trace};

#[allow(dead_code)]
#[derive(Clone)]
pub struct AdminWebsocket {
    tx: WebsocketSender,
    rx: Arc<WsPollRecv>,
}

impl AdminWebsocket {
    /// Initializes websocket connection to holochain's admin interface
    #[instrument(err)]
    pub async fn connect(admin_port: u16) -> Result<Self> {
        let socket_addr = format!("localhost:{admin_port}");
        let addr = socket_addr
            .to_socket_addrs()?
            .next()
            .expect("invalid websocket address");
        let websocket_config = Arc::new(WebsocketConfig::CLIENT_DEFAULT);
        let (tx, rx) = again::retry(|| {
            let websocket_config = Arc::clone(&websocket_config);
            connect(websocket_config, ConnectRequest::new(addr))
        })
        .await?;

        let rx = WsPollRecv::new::<AdminResponse>(rx).into();

        Ok(Self { tx, rx })
    }

    /// Attach an interface for app calls. If a port numer is None conductor will choose an available port
    /// Returns attached port number
    pub async fn attach_app_interface(&mut self, happ_port: Option<u16>) -> Result<u16> {
        info!(port = ?happ_port, "starting app interface");
        let msg = AdminRequest::AttachAppInterface {
            port: happ_port,
            allowed_origins: AllowedOrigins::Any,
            installed_app_id: None,
        };
        match self.send(msg).await? {
            AdminResponse::AppInterfaceAttached { port } => Ok(port),
            _ => Err(anyhow!("Failed to attach app interface")),
        }
    }

    pub async fn issue_app_auth_token(&mut self, app_id: String) -> Result<AppAuthenticationToken> {
        debug!("issuing app authentication token for app {:?}", app_id);
        let msg = AdminRequest::IssueAppAuthenticationToken(IssueAppAuthenticationTokenPayload {
            installed_app_id: app_id,
            expiry_seconds: 30,
            single_use: true,
        });
        let response = self.send(msg).await?;

        match response {
            AdminResponse::AppAuthenticationTokenIssued(AppAuthenticationTokenIssued {
                token,
                expires_at: _,
            }) => Ok(token),
            _ => Err(anyhow!("Error while issuing authentication token")),
        }
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

    #[instrument(skip(self, app, membrane_proofs, agent))]
    pub async fn install_and_activate_app(
        &mut self,
        app: &Happ,
        membrane_proofs: MembraneProofs,
        agent: Agent,
    ) -> Result<()> {
        let source = app.source().await?;

        let agent_key = if let Some(admin) = &app.agent_override_details().await? {
            admin.key.clone()
        } else {
            agent.admin.key.clone()
        };

        let payload = if let Ok(id) = env::var("DEV_UID_OVERRIDE") {
            debug!("using network_seed to install: {}", id);
            InstallAppPayload {
                agent_key,
                installed_app_id: Some(app.id()),
                source,
                membrane_proofs,
                network_seed: Some(id),
                ignore_genesis_failure: false,
            }
        } else {
            debug!("using default network_seed to install");
            InstallAppPayload {
                agent_key,
                installed_app_id: Some(app.id()),
                source,
                membrane_proofs,
                network_seed: None,
                ignore_genesis_failure: false,
            }
        };

        if let Err(e) = self.install_app(payload).await {
            if !e.to_string().contains("AppAlreadyInstalled") {
                return Err(e);
            }
        }

        self.activate_app(app).await?;
        debug!("installed & activated hApp: {}", app.id());
        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn install_app(&mut self, payload: InstallAppPayload) -> Result<AdminResponse> {
        let msg = AdminRequest::InstallApp(Box::new(payload));
        self.send(msg).await
    }

    #[instrument(skip(self), err)]
    pub async fn activate_app(&mut self, happ: &Happ) -> Result<AdminResponse> {
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

    #[instrument(skip(self), err)]
    pub async fn enable_app(&mut self, installed_app_id: &str) -> Result<AdminResponse> {
        let msg = AdminRequest::EnableApp {
            installed_app_id: installed_app_id.to_string(),
        };
        self.send(msg).await
    }

    #[instrument(skip(self), err)]
    pub async fn disable_app(&mut self, installed_app_id: &str) -> Result<AdminResponse> {
        let msg = AdminRequest::DisableApp {
            installed_app_id: installed_app_id.to_string(),
        };
        self.send(msg).await
    }

    #[instrument(skip(self), err)]
    pub async fn list_apps(
        &mut self,
        status_filter: Option<AppStatusFilter>,
    ) -> Result<Vec<AppInfo>> {
        let response = self.send(AdminRequest::ListApps { status_filter }).await?;
        match response {
            AdminResponse::AppsListed(apps_infos) => Ok(apps_infos),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn generate_agent_pub_key(&mut self) -> Result<AgentPubKey> {
        // Create agent key in Lair and save it in file
        let response = self.send(AdminRequest::GenerateAgentPubKey).await?;
        match response {
            AdminResponse::AgentPubKeyGenerated(key) => Ok(key),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    #[instrument(skip(self))]
    pub async fn send(&mut self, msg: AdminRequest) -> Result<AdminResponse> {
        let response = self
            .tx
            .request_timeout(msg, std::time::Duration::from_secs(600))
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
