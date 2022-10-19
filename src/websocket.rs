use crate::agent::Agent;
use crate::config::Happ;
use crate::membrane_proof::MembraneProofsVec;
use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppRequest, AppResponse, AppStatusFilter, InstalledAppInfo,
    ZomeCall,
};
use holochain_types::{
    app::{
        AppBundleSource, DnaSource, InstallAppBundlePayload, InstallAppDnaPayload,
        InstallAppPayload, InstalledAppId, RegisterDnaPayload,
    },
    prelude::{DnaModifiersOpt, YamlProperties},
};
use holochain_websocket::{connect, WebsocketConfig, WebsocketSender};
use std::{env, sync::Arc};
use tracing::{info, instrument, trace};
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

    #[instrument(skip(self))]
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

    #[instrument(skip(self, happ, mem_proof_vec, agent))]
    pub async fn install_and_activate_happ(
        &mut self,
        happ: &Happ,
        mem_proof_vec: MembraneProofsVec,
        agent: Agent,
    ) -> Result<()> {
        if happ.dnas.is_some() {
            self.register_and_install_happ(happ, mem_proof_vec, agent)
                .await?;
        } else {
            self.install_happ(happ, mem_proof_vec, agent).await?;
        }
        self.activate_app(happ).await?;
        info!("installed & activated hApp: {}", happ.id());
        Ok(())
    }

    #[instrument(skip(self, happ))]
    pub async fn activate_happ(&mut self, happ: &Happ) -> Result<()> {
        self.activate_app(happ).await?;
        info!("activated hApp: {}", happ.id());
        Ok(())
    }

    #[instrument(err, skip(self, happ, mem_proof_vec, agent))]
    async fn install_happ(
        &mut self,
        happ: &Happ,
        mem_proof_vec: MembraneProofsVec,
        agent: Agent,
    ) -> Result<()> {
        let path = match happ.bundle_path.clone() {
            Some(path) => path,
            None => {
                crate::utils::download_file(happ.bundle_url.as_ref().context("dna_url is None")?)
                    .await
                    .context("failed to download DNA archive")?
            }
        };

        let payload = if let Ok(id) = env::var("DEV_UID_OVERRIDE") {
            info!("using network_seed to install: {}", id);
            InstallAppBundlePayload {
                agent_key: agent.key,
                installed_app_id: Some(happ.id()),
                source: AppBundleSource::Path(path),
                membrane_proofs: mem_proof_vec,
                network_seed: Some(id),
            }
        } else {
            info!("using default network_seed to install");
            InstallAppBundlePayload {
                agent_key: agent.key,
                installed_app_id: Some(happ.id()),
                source: AppBundleSource::Path(path),
                membrane_proofs: mem_proof_vec,
                network_seed: None,
            }
        };

        let msg = AdminRequest::InstallAppBundle(Box::new(payload));
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

    #[instrument(err, skip(self, happ, mem_proof_vec, agent))]
    async fn register_and_install_happ(
        &mut self,
        happ: &Happ,
        mem_proof_vec: MembraneProofsVec,
        agent: Agent,
    ) -> Result<()> {
        let mut dna_payload: Vec<InstallAppDnaPayload> = Vec::new();
        match &happ.dnas {
            Some(dnas) => {
                for dna in dnas.iter() {
                    let path =
                        crate::utils::download_file(dna.url.as_ref().context("dna_url is None")?)
                            .await
                            .context("failed to download DNA archive")?;
                    // check for provided properties in the config file and apply if it exists
                    let mut properties: Option<YamlProperties> = None;
                    if let Some(p) = dna.properties.clone() {
                        let prop = p.to_string();
                        info!("Core app Properties: {}", prop);
                        properties =
                            Some(YamlProperties::new(serde_yaml::from_str(&prop).unwrap()));
                    }
                    let register_dna_payload = if let Ok(id) = env::var("DEV_UID_OVERRIDE") {
                        info!("using network_seed to install: {}", id);
                        RegisterDnaPayload {
                            modifiers: DnaModifiersOpt {
                                network_seed: Some(id),
                                properties: properties.clone(),
                                origin_time: None,
                            },
                            source: DnaSource::Path(path),
                        }
                    } else {
                        info!("using default network_seed to install");
                        RegisterDnaPayload {
                            modifiers: DnaModifiersOpt {
                                network_seed: None,
                                properties: properties.clone(),
                                origin_time: None,
                            },
                            source: DnaSource::Path(path),
                        }
                    };

                    let msg = AdminRequest::RegisterDna(Box::new(register_dna_payload));
                    let response = self.send(msg).await?;
                    match response {
                        AdminResponse::DnaRegistered(hash) => {
                            dna_payload.push(InstallAppDnaPayload {
                                hash,
                                role_id: dna.id.clone(),
                                membrane_proof: mem_proof_vec.get(&dna.id).cloned(),
                            });
                        }
                        _ => return Err(anyhow!("unexpected response: {:?}", response)),
                    };
                }
            }
            None => {
                self.install_happ(happ, mem_proof_vec, agent.clone())
                    .await?;
            }
        };

        let payload = InstallAppPayload {
            agent_key: agent.key,
            installed_app_id: happ.id(),
            dnas: dna_payload,
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

    #[instrument(skip(self), err)]
    async fn activate_app(&mut self, happ: &Happ) -> Result<AdminResponse> {
        let msg = AdminRequest::EnableApp {
            installed_app_id: happ.id(),
        };
        self.send(msg).await
    }

    #[instrument(skip(self), err)]
    pub async fn deactivate_app(&mut self, installed_app_id: &str) -> Result<AdminResponse> {
        let msg = AdminRequest::DisableApp {
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

#[derive(Clone)]
pub struct AppWebsocket {
    tx: WebsocketSender,
}

impl AppWebsocket {
    #[instrument(err)]
    pub async fn connect(app_port: u16) -> Result<Self> {
        let url = format!("ws://localhost:{}/", app_port);
        let url = Url::parse(&url).context("invalid ws:// URL")?;
        let websocket_config = Arc::new(WebsocketConfig::default());
        let (tx, _rx) = again::retry(|| {
            let websocket_config = Arc::clone(&websocket_config);
            connect(url.clone().into(), websocket_config)
        })
        .await?;
        Ok(Self { tx })
    }

    #[instrument(skip(self))]
    pub async fn get_app_info(&mut self, app_id: InstalledAppId) -> Option<InstalledAppInfo> {
        let msg = AppRequest::AppInfo {
            installed_app_id: app_id,
        };
        let response = self.send(msg).await.ok()?;
        match response {
            AppResponse::AppInfo(app_info) => app_info,
            _ => None,
        }
    }

    #[instrument(skip(self))]
    pub async fn zome_call(&mut self, msg: ZomeCall) -> Result<AppResponse> {
        let app_request = AppRequest::ZomeCall(Box::new(msg));
        let response = self.send(app_request).await;
        response
    }

    #[instrument(skip(self))]
    async fn send(&mut self, msg: AppRequest) -> Result<AppResponse> {
        let response = self
            .tx
            .request(msg)
            .await
            .context("failed to send message")?;
        match response {
            AppResponse::Error(error) => Err(anyhow!("error: {:?}", error)),
            _ => {
                trace!("send successful");
                Ok(response)
            }
        }
    }
}
