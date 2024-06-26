use crate::utils::WsPollRecv;
use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{
    AppAuthenticationRequest, AppAuthenticationToken, AppInfo, AppRequest, AppResponse, ZomeCall,
};
use holochain_websocket::{connect, ConnectRequest, WebsocketConfig, WebsocketSender};
use std::{net::ToSocketAddrs, sync::Arc};
use tracing::{instrument, trace};

#[allow(dead_code)]
#[derive(Clone)]
pub struct AppWebsocket {
    tx: WebsocketSender,
    rx: Arc<WsPollRecv>,
}

impl AppWebsocket {
    #[instrument(err)]
    pub async fn connect(app_port: u16, token: AppAuthenticationToken) -> Result<Self> {
        let socket_addr = format!("localhost:{app_port}");
        let addr = socket_addr
            .to_socket_addrs()?
            .next()
            .context("invalid websocket address")?;
        let websocket_config = Arc::new(WebsocketConfig::CLIENT_DEFAULT);
        let (tx, rx) = again::retry(|| {
            let websocket_config = Arc::clone(&websocket_config);
            connect(websocket_config, ConnectRequest::new(addr))
        })
        .await?;
        let rx = WsPollRecv::new::<AppResponse>(rx).into();

        // Websocket connection needs authentication via token previously obtained from Admin Interface
        tx.authenticate(AppAuthenticationRequest { token })
            .await
            .map_err(|err| anyhow!("Failed to send authentication: {err:?}"))?;

        Ok(Self { tx, rx })
    }

    #[instrument(skip(self))]
    pub async fn get_app_info(&mut self) -> Option<AppInfo> {
        let msg = AppRequest::AppInfo;
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
