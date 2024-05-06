use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{AppInfo, AppRequest, AppResponse, ZomeCall};
use holochain_types::app::InstalledAppId;
use holochain_websocket::{connect, ConnectRequest, WebsocketConfig, WebsocketSender};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tracing::{instrument, trace};
use crate::utils::WsPollRecv;

#[derive(Clone)]
pub struct AppWebsocket {
    tx: WebsocketSender,
    rx: Arc<WsPollRecv>,
}

impl AppWebsocket {
    #[instrument(err)]
    pub async fn connect(app_port: u16) -> Result<Self> {
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), app_port);
        let websocket_config = Arc::new(WebsocketConfig::LISTENER_DEFAULT);
        let (tx, rx) = again::retry(|| {
            let websocket_config = Arc::clone(&websocket_config);
            connect(websocket_config, ConnectRequest::new(socket))
        })
        .await?;
        let rx = WsPollRecv::new::<AppResponse>(rx).into();
        Ok(Self { tx, rx })
    }

    #[instrument(skip(self))]
    pub async fn get_app_info(&mut self, app_id: InstalledAppId) -> Option<AppInfo> {
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
        let app_request = AppRequest::CallZome(Box::new(msg));
        self.send(app_request).await
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
