use crate::config::Happ;
use crate::membrane_proof::{self, get_hpos_config};
use anyhow::{anyhow, Context, Result};
use ed25519_dalek::*;
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppRequest, AppResponse, AppStatusFilter, InstalledAppInfo,
    ZomeCall,
};
use holochain_types::prelude::MembraneProof;
use holochain_types::{
    app::{
        AppBundleSource, DnaSource, InstallAppBundlePayload, InstallAppDnaPayload,
        InstallAppPayload, InstalledAppId, RegisterDnaPayload,
    },
    dna::AgentPubKey,
    prelude::{DnaModifiersOpt, YamlProperties},
};
use holochain_websocket::{connect, WebsocketConfig, WebsocketSender};
use holochain_zome_types::Timestamp;
use std::{collections::HashMap, env, fs, fs::File, io::prelude::*, sync::Arc};
use tracing::{info, instrument, trace};
use url::Url;

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
        let force = match env::var("FORCE_RANDOM_AGENT_KEY") {
            Ok(f) => !f.is_empty(),
            // The default is set to true since its only used while running `cargo test`.
            // In all other instances in holo-nixpkgs we have set a value based on the enviroment
            Err(_) => true,
        };
        // Based on the holo-network choose what agent key is to be used
        // For mainNet,flexNet and alphaNet: use the holoport ID as the holochain key
        // For devNet: create a random agent key
        // For mainNet and alphaNet
        if !force {
            info!("Using agent key from hpos-config file");
            // Use agent key from from the config file in main net
            let config = get_hpos_config()?;
            let pub_key = hpos_config_seed_bundle_explorer::holoport_public_key(
                &config,
                Some(crate::config::DEFAULT_PASSWORD.to_string()),
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
        // For devNet
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
                    crate::utils::overwrite(pubkey_path, key_vec)?;
                }
                info!("returning newly created agent key");
                self.agent_key = Some(key.clone());
                // if a new agent was created, we expect to get a new mem-proof
                let agent_pub_key = PublicKey::from_bytes(key.get_raw_32())?;
                if let Err(e) =
                    membrane_proof::try_mem_proof_server_inner(Some(agent_pub_key)).await
                {
                    println!("membrane proof error {}", e);
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

    #[instrument(skip(self, happ, membrane_proofs))]
    pub async fn install_and_activate_happ(
        &mut self,
        happ: &Happ,
        membrane_proofs: HashMap<String, MembraneProof>,
    ) -> Result<()> {
        if happ.dnas.is_some() {
            self.register_and_install_happ(happ, membrane_proofs)
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
            .context("failed to generate agent key while installing")?;
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
                agent_key,
                installed_app_id: Some(happ.id()),
                source: AppBundleSource::Path(path),
                membrane_proofs,
                network_seed: Some(id),
            }
        } else {
            info!("using default network_seed to install");
            InstallAppBundlePayload {
                agent_key,
                installed_app_id: Some(happ.id()),
                source: AppBundleSource::Path(path),
                membrane_proofs,
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

    #[instrument(err, skip(self, happ, membrane_proofs))]
    async fn register_and_install_happ(
        &mut self,
        happ: &Happ,
        membrane_proofs: HashMap<String, MembraneProof>,
    ) -> Result<()> {
        let agent_key = self
            .get_agent_key()
            .await
            .context("failed to generate agent key while registering")?;
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
