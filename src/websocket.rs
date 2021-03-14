use std::fs::File;
use std::io::prelude::*;
use std::sync::Arc;
use std::{env, fs};

use anyhow::{anyhow, Context, Result};
use holochain::conductor::api::{AdminRequest, AdminResponse, AppRequest, AppResponse};
use holochain_types::{
    app::{
        DnaSource, InstallAppDnaPayload, InstallAppPayload, InstalledApp, InstalledAppId,
        RegisterDnaPayload,
    },
    dna::AgentPubKey,
};
use holochain_websocket::{websocket_connect, WebsocketConfig, WebsocketSender};
use tracing::{debug, info, instrument, trace};
use url::Url;

use crate::Happ;

#[derive(Clone)]
pub struct AdminWebsocket {
    tx: WebsocketSender,
    agent_key: Option<AgentPubKey>,
}

impl AdminWebsocket {
    #[instrument(err)]
    pub async fn connect(admin_port: u16) -> Result<Self> {
        let url = format!("ws://localhost:{}/", admin_port);
        let url = Url::parse(&url).context("invalid ws:// URL")?;
        let websocket_config = Arc::new(WebsocketConfig::default());
        let (tx, _rx) = again::retry(|| {
            let websocket_config = Arc::clone(&websocket_config);
            websocket_connect(url.clone().into(), websocket_config)
        })
        .await?;
        Ok(Self {
            tx,
            agent_key: None,
        })
    }

    #[instrument(skip(self), err)]
    pub async fn get_agent_key(&mut self) -> Result<AgentPubKey> {
        // Try agent key from memory
        if let Some(key) = self.agent_key.clone() {
            info!("returning agent key from memory");
            return Ok(key);
        }
        // Try agent key from disc
        if let Ok(pubkey_path) = env::var("PUBKEY_PATH") {
            if let Ok(key_vec) = fs::read(&pubkey_path) {
                if let Ok(key) = AgentPubKey::from_raw_39(key_vec) {
                    info!("returning agent key from file");
                    self.agent_key = Some(key.clone());
                    return Ok(key);
                }
            }
        }

        // Create agent key in Lair and save it in file
        let response = self.send(AdminRequest::GenerateAgentPubKey).await?;
        match response {
            AdminResponse::AgentPubKeyGenerated(key) => {
                let key_vec = key.get_raw_39();
                if let Ok(pubkey_path) = env::var("PUBKEY_PATH") {
                    let mut file = File::create(pubkey_path)?;
                    file.write_all(&key_vec)?;
                }
                info!("returning newly created agent key");
                self.agent_key = Some(key.clone());
                Ok(key)
            }
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    }

    #[instrument(skip(self))]
    pub async fn attach_app_interface(&mut self, happ_port: u16) -> Result<AdminResponse> {
        info!(port = ?happ_port, "starting app interface");
        let msg = AdminRequest::AttachAppInterface {
            port: Some(happ_port),
        };
        self.send(msg).await
    }

    #[instrument(skip(self), err)]
    pub async fn list_active_happs(&mut self) -> Result<Vec<InstalledAppId>> {
        let response = self.send(AdminRequest::ListActiveApps).await?;
        match response {
            AdminResponse::ActiveAppsListed(app_ids) => Ok(app_ids),
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    }

    #[instrument(
        skip(self, happ),
        fields(?happ.app_id),
    )]
    pub async fn install_happ(&mut self, happ: &Happ) -> Result<()> {
        if happ.dna_url.is_some() || happ.dna_path.is_some() {
            self.install_dna(happ).await?;
        } else {
            debug!(?happ.app_id, "dna_url == None || dna_path == None, skipping DNA installation")
        }
        self.activate_app(happ).await?;
        info!(?happ.app_id, "installed & activated hApp");
        Ok(())
    }

    #[instrument(
        skip(self, happ),
        fields(?happ.app_id),
    )]
    pub async fn activate_happ(&mut self, happ: &Happ) -> Result<()> {
        self.activate_app(happ).await?;
        info!(?happ.app_id, "activated hApp");
        Ok(())
    }

    #[instrument(
        err,
        skip(self, happ),
        fields(?happ.app_id)
    )]
    async fn install_dna(&mut self, happ: &Happ) -> Result<AdminResponse> {
        let agent_key = self
            .get_agent_key()
            .await
            .context("failed to generate agent key")?;
        let path = match happ.dna_path.clone() {
            Some(path) => path,
            None => crate::download_file(happ.dna_url.as_ref().context("dna_url is None")?)
                .await
                .context("failed to download DNA archive")?,
        };

        // register the DNA so we can pass in a uuid
        let dna = RegisterDnaPayload {
            uuid: happ.uuid.clone(),
            source: DnaSource::Path(path),
            properties: None,
        };

        let msg = AdminRequest::RegisterDna(Box::new(dna));
        let response = self.send(msg).await?;
        if let AdminResponse::DnaRegistered(hash) = response {
            // install the happ using the registered DNA
            let dna = InstallAppDnaPayload {
                nick: happ.id_from_config(),
                path: None,
                hash: Some(hash),
                properties: None,
                membrane_proof: happ.membrane_proof,
            };
            let payload = InstallAppPayload {
                installed_app_id: happ.id_from_config(),
                agent_key,
                dnas: vec![dna],
            };
            let msg = AdminRequest::InstallApp(Box::new(payload));
            let response = self.send(msg).await?;
            Ok(response)
        } else {
            unreachable!()
        }
    }

    #[instrument(skip(self), err)]
    async fn activate_app(&mut self, happ: &Happ) -> Result<AdminResponse> {
        let msg = AdminRequest::ActivateApp {
            installed_app_id: happ.id_from_config(),
        };
        self.send(msg).await
    }

    #[instrument(skip(self), err)]
    pub async fn deactivate_app(&mut self, installed_app_id: &str) -> Result<AdminResponse> {
        let msg = AdminRequest::DeactivateApp {
            installed_app_id: installed_app_id.to_string(),
        };
        self.send(msg).await
    }

    #[instrument(skip(self))]
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
            websocket_connect(url.clone().into(), websocket_config)
        })
        .await?;
        Ok(Self { tx })
    }

    #[instrument(skip(self))]
    pub async fn get_app_info(&mut self, app_id: InstalledAppId) -> Option<InstalledApp> {
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
