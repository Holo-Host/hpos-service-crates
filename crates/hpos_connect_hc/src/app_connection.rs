use crate::{
    admin_ws::AdminWebsocket,
    utils::{fresh_nonce, WsPollRecv},
};
use anyhow::{anyhow, Context, Result};
use core::fmt::Debug;
use holochain_conductor_api::{
    AppAuthenticationRequest, AppInfo, AppRequest, AppResponse, CellInfo, ZomeCall,
};
use holochain_keystore::MetaLairClient;
use holochain_types::{
    app::{CreateCloneCellPayload, DisableCloneCellPayload, EnableCloneCellPayload},
    prelude::{CellId, ClonedCell, ExternIO, FunctionName, RoleName, ZomeCallUnsigned, ZomeName},
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
        let info = &self.cell_info().await?;
        match &info
            .get(&role_name)
            .ok_or(anyhow!("unable to find cell for RoleName {}!", &role_name))?[0]
        {
            CellInfo::Provisioned(c) => Ok(c.cell_id.clone()),
            _ => Err(anyhow!("unable to find cell for RoleName {}", &role_name)),
        }
    }

    /// Returns a all cloned cells for a given RoleName in a connected app
    pub async fn clone_cells(&mut self, role_name: RoleName) -> Result<Vec<ClonedCell>> {
        let info = &self.cell_info().await?;
        let app_cells = info
            .get(&role_name)
            .ok_or(anyhow!("unable to find cells for RoleName {}", &role_name))?;
        let cells = app_cells
            .into_iter()
            .filter_map(|cell_info| match cell_info {
                CellInfo::Cloned(cloned_cell) => Some(cloned_cell.clone()),
                _ => None,
            })
            .collect();
        Ok(cells)
    }

    /// Returns a cell for a given RoleName and CloneName in a connected app
    pub async fn clone_cell(&mut self, role_name: RoleName, clone_name: String) -> Result<CellId> {
        let clone_cells = self.clone_cells(role_name.clone()).await?;
        let cell = clone_cells
            .into_iter()
            .find_map(|cell| {
                if cell.name == clone_name {
                    Some(cell)
                } else {
                    None
                }
            })
            .ok_or(anyhow!(
                "unable to find clone cell for RoleName {} with name {}",
                &role_name,
                &clone_name
            ))?;
        Ok(cell.cell_id.clone())
    }

    /// Creates a clone cell in a connected app
    pub async fn create_clone(&mut self, payload: CreateCloneCellPayload) -> Result<ClonedCell> {
        let app_request = AppRequest::CreateCloneCell(Box::new(payload.clone()));
        let response = self.send(app_request).await?;
        match response {
            AppResponse::CloneCellCreated(cell) => Ok(cell),
            _ => Err(anyhow!("Error creating clone {:?}", payload)),
        }
    }

    /// Disables a clone cell
    pub async fn disable_clone(&mut self, payload: DisableCloneCellPayload) -> Result<()> {
        let app_request = AppRequest::DisableCloneCell(Box::new(payload.clone()));
        let response = self.send(app_request).await?;
        match response {
            AppResponse::CloneCellDisabled => Ok(()),
            _ => Err(anyhow!("Error disabling clone {:?}", payload)),
        }
    }

    /// Enable a clone cell
    pub async fn enable_clone(&mut self, payload: EnableCloneCellPayload) -> Result<ClonedCell> {
        let app_request = AppRequest::EnableCloneCell(Box::new(payload.clone()));
        let response = self.send(app_request).await?;
        match response {
            AppResponse::CloneCellEnabled(cloned_cell) => Ok(cloned_cell),
            _ => Err(anyhow!("Error enabling clone {:?}", payload)),
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
    /// Returns raw response in a form of ExternIo encoded bytes
    pub async fn zome_call_raw<T: Debug + Serialize>(
        &mut self,
        role_name: RoleName,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: T,
    ) -> Result<ExternIO> {
        let cell_id = self.cell(role_name).await?;
        self.zome_call_raw_cell_id(cell_id, zome_name, fn_name, payload)
            .await
    }

    /// Make a zome call to holochain's cell defined by `cell_id``.
    /// Returns raw response in a form of ExternIo encoded bytes
    pub async fn zome_call_raw_cell_id<T: Debug + Serialize>(
        &mut self,
        cell_id: CellId,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: T,
    ) -> Result<ExternIO> {
        let (nonce, expires_at) = fresh_nonce()?;

        let zome_call_unsigned = ZomeCallUnsigned {
            cell_id: cell_id.clone(),
            zome_name,
            fn_name,
            payload: ExternIO::encode(payload)?,
            cap_secret: None,
            provenance: cell_id.agent_pubkey().clone(),
            nonce,
            expires_at,
        };
        let signed_zome_call =
            ZomeCall::try_from_unsigned_zome_call(&self.keystore, zome_call_unsigned).await?;

        match self.zome_call(signed_zome_call).await {
            Ok(AppResponse::ZomeCalled(r)) => Ok(*r),
            Ok(r) => Err(anyhow!("unexpected ZomeCall response: {:?}", r)),
            Err(e) => Err(e),
        }
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
        T: Serialize + Debug,
        R: DeserializeOwned,
    {
        rmp_serde::from_slice(
            self.zome_call_raw(role_name, zome_name, fn_name, payload)
                .await?
                .as_bytes(),
        )
        .context("Error while deserializing zome call response")
    }

    /// Make a zome call to holochain's cell defined by `role_id` and `clone_name`.
    /// Returns typed deserialized response.
    pub async fn clone_zome_call_typed<T, R>(
        &mut self,
        role_name: RoleName,
        clone_name: String,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: T,
    ) -> Result<R>
    where
        T: Serialize + Debug,
        R: DeserializeOwned,
    {
        let cell_id = self.clone_cell(role_name, clone_name).await?;

        rmp_serde::from_slice(
            self.zome_call_raw_cell_id(cell_id, zome_name, fn_name, payload)
                .await?
                .as_bytes(),
        )
        .context("Error while deserializing zome call response")
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
