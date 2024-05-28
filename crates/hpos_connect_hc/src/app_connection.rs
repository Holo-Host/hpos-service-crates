use crate::{
    admin_ws::AdminWebsocket,
    utils::{fresh_nonce, WsPollRecv},
};
use anyhow::{anyhow, Context, Result};
use holochain_conductor_api::{
    AppAuthenticationRequest, AppInfo, AppRequest, AppResponse, CellInfo, ZomeCall,
};
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::{
    CellId, ExternIO, FunctionName, RoleName, ZomeCallUnsigned, ZomeName,
};
use holochain_websocket::{connect, ConnectRequest, WebsocketConfig, WebsocketSender};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, net::ToSocketAddrs, sync::Arc};
use tracing::{instrument, trace};

type CellInfoMap = HashMap<RoleName, Vec<CellInfo>>;

#[allow(dead_code)]
#[derive(Clone)]
pub struct AppWebsocket {
    tx: WebsocketSender,
    rx: Arc<WsPollRecv>,
}

#[derive(Clone)]
pub struct AppConnection {
    ws: AppWebsocket,
    pub keystore: MetaLairClient,
    cell_info: Option<CellInfoMap>,
    app_id: String,
}

impl AppConnection {
    pub async fn connect(
        admin_ws: &mut AdminWebsocket,
        keystore: MetaLairClient,
        app_id: String,
    ) -> Result<Self> {
        let app_port = admin_ws
            .attach_app_interface(None)
            .await
            .context("failed to start app interface for core app")?;

        let token = admin_ws.issue_app_auth_token(app_id.clone()).await?;

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

        Ok(Self {
            ws: AppWebsocket { tx, rx },
            keystore,
            cell_info: None, // cell info is populated lazily
            app_id,
        })
    }

    /// Return app info for a connected app
    /// Returns an error if there is no app info
    #[instrument(skip(self))]
    pub async fn app_info(&mut self) -> Result<AppInfo> {
        let msg = AppRequest::AppInfo;
        let response = self.send(msg).await?;
        trace!(
            "app_info response for app_id{}: {:?}",
            self.app_id,
            response
        );
        match response {
            AppResponse::AppInfo(Some(app_info)) => Ok(app_info),
            _ => Err(anyhow!("No AppInfo available for {}", self.app_id)),
        }
    }

    /// Return cell_info for a connected app
    /// Cell_info is evaluated lazily
    pub async fn cell_info(&mut self) -> Result<CellInfoMap> {
        if let Some(c) = self.cell_info.clone() {
            return Ok(c);
        }

        self.cell_info = Some(self.app_info().await?.cell_info);
        Ok(self.cell_info.clone().unwrap())
    }

    /// Returns a cell for a given RoleName in a connected app
    pub async fn cell(&mut self, role_name: RoleName) -> Result<CellId> {
        match &self
            .cell_info()
            .await?
            .get(&role_name)
            .ok_or(anyhow!("unable to find cell for RoleName {}!", &role_name))?[0]
        {
            CellInfo::Provisioned(c) => Ok(c.cell_id.clone()),
            _ => Err(anyhow!("unable to find cell for RoleName {}", &role_name)),
        }
    }

    /// Raw zome call function taking holochain_conductor_api::app_interface::ZomeCall as an argument
    /// and returning AppResponse without checking an outcomeor deserializing
    #[instrument(skip(self))]
    pub async fn zome_call(&mut self, msg: ZomeCall) -> Result<AppResponse> {
        let app_request = AppRequest::CallZome(Box::new(msg));
        self.send(app_request).await
    }

    /// Return app id
    #[instrument(skip(self))]
    pub fn id(&self) -> String {
        self.app_id.clone()
    }

    /// Make a zome call to holochain's cell defined by `role_name`.
    /// Returns typed deserialized response.
    pub async fn zome_call_typed<T, R>(
        &mut self,
        role_name: RoleName,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: T,
    ) -> Result<R>
    where
        T: Serialize + std::fmt::Debug,
        R: DeserializeOwned,
    {
        let (nonce, expires_at) = fresh_nonce()?;
        let cell = self.cell(role_name).await?;

        let zome_call_unsigned = ZomeCallUnsigned {
            cell_id: cell.clone(),
            zome_name,
            fn_name,
            payload: ExternIO::encode(payload)?,
            cap_secret: None,
            provenance: cell.agent_pubkey().clone(),
            nonce,
            expires_at,
        };
        let signed_zome_call =
            ZomeCall::try_from_unsigned_zome_call(&self.keystore, zome_call_unsigned).await?;

        let response = self.zome_call(signed_zome_call).await?;

        match response {
            AppResponse::ZomeCalled(r) => {
                let response: R = rmp_serde::from_slice(r.as_bytes())?;
                Ok(response)
            }
            _ => Err(anyhow!("unexpected ZomeCallresponse: {:?}", response)),
        }
    }

    /// Sign byte payload with holofuel agent's private key
    /// Currently it is commented out, because I do not know what agent key shall i use
    // pub async fn sign_raw(&mut self, data: Arc<[u8]>) -> Result<Signature> {
    //     let (_, agent_pubkey) = self.cell().await?;
    //     Ok(agent_pubkey.sign_raw(&self.keystore, data).await?)
    // }

    /// Low level internal websocket function
    #[instrument(skip(self))]
    async fn send(&mut self, msg: AppRequest) -> Result<AppResponse> {
        let response = self
            .ws
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

// TODO: move to consts
pub enum CoreAppRoleName {
    HHA,
    Holofuel,
}

impl From<CoreAppRoleName> for RoleName {
    fn from(val: CoreAppRoleName) -> Self {
        match val {
            CoreAppRoleName::HHA => "core-app".into(),
            CoreAppRoleName::Holofuel => "holofuel".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use holochain_types::prelude::RoleName;

    use super::CoreAppRoleName;

    #[test]
    fn core_app_role_name_to_role_name() {
        let hha: RoleName = CoreAppRoleName::HHA.into();
        let hf: RoleName = CoreAppRoleName::Holofuel.into();

        assert_eq!(hha, "core-app");
        assert_eq!(hf, "holofuel");
    }
}
