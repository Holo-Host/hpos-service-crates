use holochain_client::{AdminWebsocket, AppWebsocket};
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::{ActionHash, ActionHashB64, Nonce256Bits, Signature, Timestamp};

// CHC types
#[derive(Debug, Snafu)]
pub enum ChcError {
    #[snafu(display("Failed to connect to chc at addr {}: {}", addr, source))]
    FailedRequest {
        source: reqwest::Error,
        addr: Box<Url>,
    },
    #[snafu(display("Error status returned from chc {}:", status))]
    FailedResponse { status: u16 },
    #[snafu(display("Unexpected response from chc {}:", source))]
    UnexpectedResponse { source: reqwest::Error },
    #[snafu(display("Failed to deserialize response from chc"))]
    SerdeError,
    #[snafu(display("Failed to generate nonce"))]
    NonceError,
    #[snafu(display("Failed to sign payload"))]
    SigningError,
}

pub struct ChcCredentials {
    pub app_websocket: &AppWebsocket,
    pub keystore: &MetaLairClient,
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
/// NB: This same fn exists in envoy with just a slight modification to the fn input. TODO: Consider exporting this in a utility crate that can be used by all required services.
fn find_chc_head_moved_error_since_hashes(
    response: &crate::holochain::RequestError,
) -> Result<Vec<ActionHash>, Error> {
    if let Ok(error) = response {
        if let Err(
            _err @ holochain_conductor_api::ExternalApiWireError::InternalError(err_string),
        ) = error
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
    }

    Ok(vec![])
}

/// NB: This same fn exists in envoy -- TODO: Consider exporting this in a utility crate that can be used by all required services.
pub async fn handle_out_of_sync_install(
    keystore: &MetaLairClient,
    admin_websocket: &AdminWebsocket,
    app_websocket: &AppWebsocket,
    chc_url: &String,
    since_hash: Option<ActionHash>,
    app_id: String,
) -> usize {
    let app_info_after_out_of_sync_install_res = app_websocket.app_info(app_id.clone()).await;

    let mut total_entries_restored = 0;

    dbg!("^&* handle_out_of_sync_install", &since_hash, &app_id);

    match app_info_after_out_of_sync_install_res {
        Ok(app_info_after_out_of_sync_install) => match app_info_after_out_of_sync_install {
            Ok(inner_app_info) => {
                if let Some(app_info) = inner_app_info {
                    for cell_id in all_cell_ids(app_info) {
                        total_entries_restored += restore_chain_from_chc(
                            keystore,
                            admin_websocket,
                            chc_url,
                            cell_id,
                            since_hash.clone(),
                        )
                        .await
                        .expect("Failed to restore cell");
                    }
                } else {
                    dbg!("inner_app_info app_info was None");
                }
            }
            Err(e) => {
                dbg!("failed in layer two of app_info {:}", e);
            }
        },
        Err(e) => {
            dbg!("failed in layer one of app_info {:}", e);
        }
    };

    total_entries_restored
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

pub async fn restore_chain_from_chc(
    keystore: &MetaLairClient,
    admin_websocket: AdminWebsocket,
    chc_url: &String,
    cell_id: CellId,
    since_hash: Option<ActionHash>,
) -> Result<usize, RequestError> {
    dbg!("^&* restore_chain_from_chc", &cell_id, &since_hash);

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
        dbg!("^&* records not empty, grafting", &records);

        let _graft_result =
            graft_records(admin_websocket, cell_id, false, records.to_vec()).await?;

        dbg!("^&*", records.len());

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

    let (nonce, timestamp) =
        utils::fresh_nonce(Timestamp::now()).map_err(|_| ChcError::NonceError)?;

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

pub async fn graft_records(
    admin_ws: AdminWebsocket,
    cell_id: CellId,
    validate: bool,
    records: Vec<Record>,
) -> Result<Response<()>, RequestError> {
    let response = admin_ws.graft_records(cell_id, validate, records).await;

    match response {
        Ok(inner) => Ok(Ok(inner)),
        Err(conductor_api_error) => match conductor_api_error {
            holochain_client::ConductorApiError::WebsocketError(_) => Err(RequestError::Protocol {
                source: ProtocolError::WebsocketError,
                connection_type: "admin",
            }),
            holochain_client::ConductorApiError::ExternalApiWireError(error) => Ok(Err(error)),
        },
    }
}
