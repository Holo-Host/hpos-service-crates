use anyhow::{anyhow, Context, Result};
use ed25519_dalek::*;
use holochain::conductor::api::{
    AdminRequest, AdminResponse, AppRequest, AppResponse, InstalledAppInfo, ZomeCall,
};
use holochain_types::prelude::MembraneProof;
use holochain_types::{
    app::{
        AppBundleSource, DnaSource, InstallAppBundlePayload, InstallAppDnaPayload,
        InstallAppPayload, InstalledAppId, RegisterDnaPayload,
    },
    dna::AgentPubKey,
    prelude::YamlProperties,
};
use holochain_websocket::{connect, WebsocketConfig, WebsocketSender};
use hpos_config_core::{Config};
use std::{collections::HashMap, env, fs, fs::File, io::prelude::*, sync::Arc};
use tracing::{info, instrument, trace};
use url::Url;
mod membrane_proof;

use crate::config::Happ;

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
            connect(url.clone().into(), websocket_config)
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

        // Based on the holo-network choose what agent key is to be used
        // For mainNet,flexNet and alphaNet: use the holoport ID as the holochain key
        // For devNet: create a random agent key
        if let Ok(holo_network) = env::var("HOLO_NETWORK") {
            // For mainNet and alphaNet
            if holo_network != "devNet" {
                // Use agent key from from the config file in main net
                if let Ok(config_path) = env::var("HPOS_CONFIG_PATH") {
                    if let Ok(config_json) = fs::read(&config_path) {
                        let config: Config = serde_json::from_slice(&config_json)?;
                        let pub_key = hpos_config_seed_bundle_explorer::holoport_public_key(
                            &config,
                            Some("pass".to_string()),
                        )
                        .await
                        .unwrap();
                        let key = AgentPubKey::from_raw_32(pub_key.to_bytes().to_vec());
                        // Copy to the `agent-key.pub` files for other apps that use it as reference
                        if let Ok(pubkey_path) = env::var("PUBKEY_PATH") {
                            let mut file = File::create(pubkey_path)?;
                            file.write_all(key.get_raw_39())?;
                        }
                        self.agent_key = Some(key.clone());
                        return Ok(key);
                    }
                }
            }
        }
        // For devNet or flexNet
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
                    file.write_all(key_vec)?;
                }
                info!("returning newly created agent key");
                self.agent_key = Some(key.clone());
                // if using devNet,
                // enable membrane proof using generated key
                let agent_pub_key = PublicKey::from_bytes(key.get_raw_32())?;
                if let Err(e) = membrane_proof::enable_memproof_dev_net(agent_pub_key).await {
                    info!("membrane proof error {}", e);
                }
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
        let response = self.send(AdminRequest::ListEnabledApps).await?;
        match response {
            AdminResponse::EnabledAppsListed(app_ids) => Ok(app_ids),
            _ => Err(anyhow!("unexpected response: {:?}", response)),
        }
    }

    #[instrument(skip(self, happ, membrane_proofs))]
    pub async fn install_and_activate_happ(
        &mut self,
        happ: &Happ,
        membrane_proofs: HashMap<String, MembraneProof>,
        properties: Option<YamlProperties>,
    ) -> Result<()> {
        if happ.dnas.is_some() {
            self.register_and_install_happ(happ, membrane_proofs, properties)
                .await?;
        } else {
            self.install_happ(happ, membrane_proofs).await?;
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

    #[instrument(err, skip(self, happ, membrane_proofs))]
    async fn install_happ(
        &mut self,
        happ: &Happ,
        membrane_proofs: HashMap<String, MembraneProof>,
    ) -> Result<()> {
        let agent_key = self
            .get_agent_key()
            .await
            .context("failed to generate agent key")?;
        let path = match happ.bundle_path.clone() {
            Some(path) => path,
            None => crate::download_file(happ.bundle_url.as_ref().context("dna_url is None")?)
                .await
                .context("failed to download DNA archive")?,
        };

        let payload = if let Ok(id) = env::var("DEV_UID_OVERRIDE") {
            info!("using uid to install: {}", id);
            InstallAppBundlePayload {
                agent_key,
                installed_app_id: Some(happ.id()),
                source: AppBundleSource::Path(path),
                membrane_proofs,
                uid: Some(id),
            }
        } else {
            info!("using default uid to install");
            InstallAppBundlePayload {
                agent_key,
                installed_app_id: Some(happ.id()),
                source: AppBundleSource::Path(path),
                membrane_proofs,
                uid: None,
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

    #[instrument(err, skip(self, happ, membrane_proofs))]
    async fn register_and_install_happ(
        &mut self,
        happ: &Happ,
        membrane_proofs: HashMap<String, MembraneProof>,
        properties: Option<YamlProperties>,
    ) -> Result<()> {
        let agent_key = self
            .get_agent_key()
            .await
            .context("failed to generate agent key")?;
        let mut dna_payload: Vec<InstallAppDnaPayload> = Vec::new();
        match &happ.dnas {
            Some(dnas) => {
                for dna in dnas.iter() {
                    let path = crate::download_file(dna.url.as_ref().context("dna_url is None")?)
                        .await
                        .context("failed to download DNA archive")?;
                    let register_dna_payload = if let Ok(id) = env::var("DEV_UID_OVERRIDE") {
                        info!("using uid to install: {}", id);
                        RegisterDnaPayload {
                            uid: Some(id),
                            properties: properties.clone(),
                            source: DnaSource::Path(path),
                        }
                    } else {
                        info!("using default uid to install");
                        RegisterDnaPayload {
                            uid: None,
                            properties: properties.clone(),
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
                                membrane_proof: membrane_proofs.get(&dna.id).cloned(),
                            });
                        }
                        _ => return Err(anyhow!("unexpected response: {:?}", response)),
                    };
                }
            }
            None => {
                self.install_happ(happ, membrane_proofs).await?;
            }
        };

        let payload = InstallAppPayload {
            agent_key,
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
