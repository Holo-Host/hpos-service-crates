// NB: These types and functions already exist in envoy (most with just a slight modification to the fn input).
// TODO: Consider using a single utility crate that across all required services.

use crate::error_types::{ChcError, RequestError};
use crate::utils;
use crate::AdminWebsocket;
use crate::AppWebsocket;

use anyhow::{anyhow, Error};
use futures::StreamExt;
use holochain_conductor_api::{AdminResponse, AppInfo, CellInfo};
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::{
    ActionHash, ActionHashB64, CellId, Nonce256Bits, Signature, Timestamp,
};
use holochain_types::prelude::{Record, SignedActionHashed};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use url::Url;

// CHC types
#[derive(Debug)]
pub struct ChcCredentials {
    pub app_websocket: AppWebsocket,
    pub keystore: MetaLairClient,
    pub chc_url: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GetRecordsDataPayload {
    since_hash: Option<ActionHash>,
    nonce: Nonce256Bits,
    timestamp: Timestamp,
}

#[derive(Serialize, Deserialize, Debug)]
struct GetRecordsDataInput {
    payload: GetRecordsDataPayload,
    signature: Signature,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, derive_more::From)]
pub struct EncryptedEntry(#[serde(with = "serde_bytes")] pub Vec<u8>);
type GetRecordsDataRow = (SignedActionHashed, Option<(Arc<EncryptedEntry>, Signature)>);
type GetRecordsDataResponse = Vec<GetRecordsDataRow>;

// CHC functions
/// Determines if the there is a chc error, returning the most recent hash stored in the CHC if an chc error exists and an empty vec if not.
pub fn find_chc_head_moved_error_since_hashes(
    response: &Result<AdminResponse, holochain_conductor_api::ExternalApiWireError>,
) -> Result<Vec<ActionHash>, Error> {
    if let Err(_err @ holochain_conductor_api::ExternalApiWireError::InternalError(err_string)) =
        response
    {
        let re = regex::Regex::new(
                r#"ChcHeadMoved\(\\"([^"]+)\\", InvalidChain\(([0-9]+), ActionHash\(([a-zA-Z0-9-_]+)\)\)\)"#,
            ).expect("Failed to construct regex");

        let result: Result<Vec<ActionHash>, Error> = re
            .captures_iter(&format!("{:?}", err_string))
            .map(|capture| {
                Ok(
                    ActionHashB64::from_b64_str(capture.get(3).unwrap().as_str())
                        .map_err(|source| Error::BadActionHashChc { source })?
                        .into(),
                ) // unwrap is safe here because if there's a match at all, then group 3 will exist
            })
            .collect();

        return result;
    }

    Ok(vec![])
}

/// NB: This same fn exists in envoy -- TODO: Consider exporting this in a utility crate that can be used by all required services.
async fn handle_out_of_sync_install(
    admin_websocket: &mut AdminWebsocket,
    keystore: &MetaLairClient,
    app_websocket: &AppWebsocket,
    chc_url: &String,
    since_hash: Option<ActionHash>,
) -> Result<usize, RequestError> {
    let mut total_entries_restored = 0;

    match app_websocket.app_info().await.map_err(|error_response| {
        Err(anyhow!(
            "Error getting app_info for out of sync install: {:?}",
            error_response
        ))
    })? {
        Ok(app_info) => match app_info {
            Some(inner_app_info) => {
                for cell_id in all_cell_ids(inner_app_info) {
                    total_entries_restored += restore_chain_from_chc(
                        keystore,
                        admin_websocket,
                        cell_id,
                        chc_url,
                        since_hash.clone(),
                    )
                    .await
                    .expect("Failed to restore cell");
                }
            }
            None => {
                log::debug!("inner_app_info app_info was None");
            }
        },
        Err(e) => {
            log::debug!("{:?}", e);
        }
    };

    Ok(total_entries_restored)
}

fn all_cell_ids(app_info: AppInfo) -> Vec<CellId> {
    app_info
        .cell_info
        .into_iter()
        .flat_map(|(_dna_role, cell_infos)| {
            cell_infos
                .into_iter()
                .filter_map(cell_id_from_cell_info)
                .collect::<Vec<CellId>>()
        })
        .collect()
}

fn cell_id_from_cell_info(cell_info: CellInfo) -> Option<CellId> {
    match cell_info {
        CellInfo::Provisioned(provisioned_cell) => Some(provisioned_cell.cell_id),
        CellInfo::Cloned(cloned_cell) => Some(cloned_cell.cell_id),
        CellInfo::Stem(_) => None,
    }
}

pub async fn ensure_restored_chain(
    admin_websocket: &mut AdminWebsocket,
    keystore: &MetaLairClient,
    app_websocket: &mut AppWebsocket,
    chc_url: &String,
    maybe_since_hash: Option<ActionHash>,
) -> Result<(), (RequestError, usize)> {
    let total_added_entries = handle_out_of_sync_install(
        admin_websocket,
        &keystore,
        &app_websocket,
        &chc_url,
        maybe_since_hash,
    )
    .await
    .map_err(|e| {
        Err((
            RequestError::Chc {
                source: Err(anyhow!(
                    "Failed call to `handle_out_of_sync_install`. Err: {:?}",
                    e,
                )),
            },
            1,
        ))
    })?;

    if total_added_entries <= 0 && maybe_since_hash.is_none() {
        // No entries were added, so sync was failed
        return Err((
            RequestError::Chc {
                source: Err(anyhow!(
                    "CHC sync failed, no entries were available to be added."
                )),
            },
            total_added_entries,
        ));
    };

    // Entries were added, so sync was successful
    // Ensure all cells are restored then continue on to activing the app id
    ensure_restored_cells(
        admin_websocket,
        &keystore,
        &app_websocket,
        &chc_url,
        maybe_since_hash,
    )
    .await
    .map_err(|e| {
        Err((
            RequestError::Chc {
                source: Err(anyhow!(
                    "Failed call to `ensure_restored_cells`. Err: {:?}",
                    e,
                )),
            },
            ensure_restored_cells,
        ))
    })?;

    Ok(())
}

async fn restore_chain_from_chc(
    keystore: &MetaLairClient,
    admin_websocket: &mut AdminWebsocket,
    cell_id: CellId,
    chc_url: &String,
    since_hash: Option<ActionHash>,
) -> Result<usize, RequestError> {
    log::debug!(
        "restore_chain_from_chc with cell_id ({:?}) and since_hash ({:?})",
        &cell_id,
        &since_hash
    );

    let records_data = get_records_data(keystore, chc_url, cell_id.clone(), since_hash)
        .await
        .map_err(|err| RequestError::Chc { source: err })?;

    let records: Vec<Record> = records_data
        .into_iter()
        .map(|(a, me)| {
            Record::new(
                a,
                me.map(|(e, _s)| {
                    holochain_serialized_bytes::decode(&e.0).expect("Failed to decode entry")
                }),
            )
        })
        .collect();

    if !records.is_empty() {
        log::debug!("records not empty, grafting {:#?}", &records);

        let validate = false;
        let _graft_result = admin_websocket
            .graft_records(cell_id, validate, records.to_vec())
            .await;

        log::debug!("grafted {:?} records", records.len());

        return Ok(records.len());
    }

    Ok(0)
}

async fn get_records_data(
    keystore: &MetaLairClient,
    chc_url: &Url,
    cell_id: CellId,
    since_hash: Option<ActionHash>,
) -> Result<GetRecordsDataResponse, ChcError> {
    let mut full_url = chc_url.clone();
    let (dna_hash, agent_hash) = cell_id.into_dna_and_agent();
    full_url
        .path_segments_mut()
        .expect("Failed to get mutable path segments")
        .push(&dna_hash.to_string())
        .push(&agent_hash.to_string())
        .push("get_record_data");

    let (nonce, timestamp) = utils::fresh_nonce().map_err(|_| ChcError::NonceError)?;

    let payload = GetRecordsDataPayload {
        since_hash,
        nonce,
        timestamp,
    };

    let payload_to_sign = rmp_serde::to_vec_named(&payload).map_err(|_| ChcError::SerdeError)?;

    let signature = agent_hash
        .sign_raw(keystore, payload_to_sign.into())
        .await
        .map_err(|_| ChcError::SigningError)?;

    let get_records_data_input = GetRecordsDataInput { payload, signature };

    let body = holochain_serialized_bytes::encode(&get_records_data_input)
        .map_err(|_| ChcError::SerdeError)?;

    let client = reqwest::Client::new();

    let response =
        client
            .post(full_url)
            .body(body)
            .send()
            .await
            .map_err(|e| ChcError::FailedRequest {
                source: e,
                addr: Box::new(chc_url.to_owned()),
            })?;

    let status = response.status().as_u16();
    let bytes = response
        .bytes()
        .await
        .map_err(|source| ChcError::UnexpectedResponse { source })?;

    match status {
        200 => {
            Ok(holochain_serialized_bytes::decode(&bytes)
                .expect("holochain_serialized_bytes::decode"))
        }
        498 => {
            // The since_hash was not found in the CHC,
            // so we can interpret this as an empty list of records.
            Ok(vec![])
        }
        _ => Err(ChcError::FailedResponse { status }),
    }
}

async fn find_empty_cells(
    admin_ws: &mut AdminWebsocket,
    app_ws: &AppWebsocket,
) -> Result<Vec<CellId>, Error> {
    if let Some(app_info) = app_ws.app_info().await.map_err(|error_response| {
        Err(anyhow!(
            "Error getting app_info for empty cells: {:?}",
            error_response
        ))
    })? {
        let cell_ids = all_cell_ids(app_info);

        // async future that takes a `cell_id` and returns `Some(cell_id)` if that cell has an empty source chain, otherwise returns `None`
        let cell_id_if_empty = |cell_id: CellId| async move {
            let maybe_cell_id = if let AdminResponse::StateDumped(cell_state) =
                admin_ws.dump_state(cell_id.clone()).await.ok()?
            {
                let data: (serde_json::Value, String) =
                    serde_json::from_str(&cell_state).expect("failed to deserialize");

                // ATTN: this relies on specific format of the state dump summary string and so is brittle to changes
                if let Some(captures) = regex::Regex::new(r"Records authored: (\d+)")
                    .unwrap()
                    .captures(&data.1)
                {
                    // Extract the captured integer
                    let records_authored: i32 = captures[1].parse().unwrap();

                    if records_authored > 0 {
                        // There are authored records, so the cell is not empty and we return None
                        None
                    } else {
                        // There are no authored records, so the cell is empty and we return it
                        Some(cell_id)
                    }
                } else {
                    // There are no records found, so we return the cell to be safe
                    Some(cell_id)
                }
            } else {
                log::error!("Error when calling cell state dump during CHC restoration.");
                // Something went wrong, so we return the cell to be safe
                Some(cell_id)
            };
            maybe_cell_id
        };

        let maybe_cell_ids = futures::stream::iter(cell_ids)
            .then(|cell_id| async move { cell_id_if_empty(cell_id).await })
            .collect::<Vec<Option<CellId>>>()
            .await;

        let cell_ids = maybe_cell_ids
            .into_iter()
            .filter_map(|maybe_cell_id| maybe_cell_id)
            .collect();

        Ok(cell_ids)
    } else {
        log::error!("Found empty app info during CHC restoration.");
        Ok(vec![])
    }
}

async fn ensure_restored_cells(
    admin_websocket: &mut AdminWebsocket,
    keystore: &MetaLairClient,
    app_websocket: &AppWebsocket,
    chc_url: &Url,
    since_hash: Option<ActionHash>,
) -> Result<(), Error> {
    let empty_cells = find_empty_cells(admin_websocket, app_websocket)
        .await
        .expect("Failed to get empty_cells");

    log::debug!("empty_cells : {:#?}", &empty_cells);

    for empty_cell_id in empty_cells {
        match restore_chain_from_chc(
            keystore,
            admin_websocket,
            empty_cell_id.clone(),
            chc_url,
            since_hash.clone(),
        )
        .await
        {
            Ok(_) => {
                log::debug!("Succesfully restored cell {}", empty_cell_id);
            }
            Err(_) => {
                log::debug!("Failed to restore cell {}", empty_cell_id);
            }
        }
    }

    match app_websocket.app_info().await {
        Ok(Some(app_info)) => {
            log::debug!("Post graft app status: {:?} ", &app_info.status);
            Ok(())
        }
        Ok(None) => Err(anyhow!("Failed to find app after CHC graft.")),
        Err(error) => {
            log::debug!(
                "Failed when getting app status after CHC graft: {:?}",
                &error
            );
            Err(anyhow!(
                "Failed when getting app status after CHC graft: {:?}",
                error
            ))
        }
    }
}
