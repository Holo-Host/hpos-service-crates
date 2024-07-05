use crate::utils::WsPollRecv;

use super::holo_config::Happ;
use super::hpos_agent::Agent;
use super::hpos_membrane_proof::MembraneProofs;
use anyhow::{anyhow, Context, Result};
use chrono::Duration;
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppAuthenticationToken, AppAuthenticationTokenIssued, AppInfo,
    AppStatusFilter, IssueAppAuthenticationTokenPayload,
};
use holochain_types::{
    app::{DeleteCloneCellPayload, InstallAppPayload, InstalledAppId},
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
        match self.send(msg, None).await? {
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
        let response = self.send(msg, None).await?;

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
        let response = self
            .send(AdminRequest::ListApps { status_filter }, None)
            .await?;
        match response {
            AdminResponse::AppsListed(info) => {
                Ok(info.iter().map(|i| i.installed_app_id.to_owned()).collect())
            }
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    }

    pub async fn list_enabled_apps(&mut self) -> Result<Vec<InstalledAppId>> {
        let enabled = self.list_app(Some(AppStatusFilter::Enabled)).await?;
        Ok(enabled)
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
        self.send(msg, Some(300)).await // First install takes a while due to compile to WASM step
    }

    #[instrument(skip(self), err)]
    pub async fn activate_app(&mut self, happ: &Happ) -> Result<AdminResponse> {
        let msg = AdminRequest::EnableApp {
            installed_app_id: happ.id(),
        };
        self.send(msg, None).await
    }

    #[instrument(skip(self), err)]
    pub async fn uninstall_app(&mut self, installed_app_id: &str) -> Result<AdminResponse> {
        let msg = AdminRequest::UninstallApp {
            installed_app_id: installed_app_id.to_string(),
        };
        self.send(msg, None).await
    }

    #[instrument(skip(self), err)]
    pub async fn enable_app(&mut self, installed_app_id: &str) -> Result<AdminResponse> {
        let msg = AdminRequest::EnableApp {
            installed_app_id: installed_app_id.to_string(),
        };
        self.send(msg, None).await
    }

    #[instrument(skip(self), err)]
    pub async fn disable_app(&mut self, installed_app_id: &str) -> Result<AdminResponse> {
        let msg = AdminRequest::DisableApp {
            installed_app_id: installed_app_id.to_string(),
        };
        self.send(msg, None).await
    }

    #[instrument(skip(self), err)]
    pub async fn list_apps(
        &mut self,
        status_filter: Option<AppStatusFilter>,
    ) -> Result<Vec<AppInfo>> {
        let response = self
            .send(AdminRequest::ListApps { status_filter }, None)
            .await?;
        match response {
            AdminResponse::AppsListed(apps_infos) => Ok(apps_infos),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    pub async fn generate_agent_pub_key(&mut self) -> Result<AgentPubKey> {
        // Create agent key in Lair and save it in file
        let response = self.send(AdminRequest::GenerateAgentPubKey, None).await?;
        match response {
            AdminResponse::AgentPubKeyGenerated(key) => Ok(key),
            _ => unreachable!("Unexpected response {:?}", response),
        }
    }

    /// Deletes a clone cell
    pub async fn delete_clone(&mut self, payload: DeleteCloneCellPayload) -> Result<()> {
        let admin_request = AdminRequest::DeleteCloneCell(Box::new(payload.clone()));
        let response = self.send(admin_request, None).await?;
        match response {
            AdminResponse::CloneCellDeleted => Ok(()),
            _ => Err(anyhow!("Error creating clone {:?}", payload)),
        }
    }

    #[instrument(skip(self))]
    pub async fn send(
        &mut self,
        msg: AdminRequest,
        duration: Option<u64>,
    ) -> Result<AdminResponse> {
        // default timeout is 60 seconds
        let timeout_duration = std::time::Duration::from_secs(duration.unwrap_or(60));

        let response = self
            .tx
            .request_timeout(msg, timeout_duration)
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
