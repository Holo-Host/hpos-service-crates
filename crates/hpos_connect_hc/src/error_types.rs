use holochain_client::{AdminWebsocket, AppWebsocket};
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::{ActionHash, ActionHashB64, Nonce256Bits, Signature, Timestamp};
use snafu::{ResultExt, Snafu};

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

#[derive(Debug, Snafu)]
pub enum ProtocolError {
    #[snafu(display(
        "Could not deserialize bytes {:?} as response: {}
    Note: probably caused by Holochain making a breaking change to its admin API",
        bytes,
        source
    ))]
    DeserializeResponse {
        bytes: Vec<u8>,
        source: rmp_serde::decode::Error,
    },
    #[snafu(display("Connection ended without having received awaited response"))]
    NoResponse { source: Canceled },
    #[snafu(display(
        "Unexpected response type. Expected type {}, got response: {}",
        expected,
        got
    ))]
    UnexpectedResponseType { expected: &'static str, got: String },
    #[snafu(display("Failed to deserialize action hash from chc {}", source))]
    BadActionHashChc { source: HoloHashError },
    #[snafu(display("Websocket error while using conductor api"))]
    WebsocketError,
}

#[derive(Debug, Snafu)]
pub enum RequestError {
    #[snafu(display(
        "Encountered error with connection to Holochain {} WebSocket: {}",
        connection_type,
        source
    ))]
    Connection {
        source: ConnectionError,
        connection_type: &'static str,
    },
    #[snafu(display(
        "Holochain {} WebSocket violated expected RPC protocol: {}",
        connection_type,
        source
    ))]
    Protocol {
        source: ProtocolError,
        connection_type: &'static str,
    },
    #[snafu(display(
        "Encountered error constructing AppBundle while connecting to Holochain {} WebSocket: {}",
        connection_type,
        source
    ))]
    AppBundle {
        source: AppBundleError,
        connection_type: &'static str,
    },
    #[snafu(display("Failed to sign host zome call: {}", source))]
    SignHostZomeCall { source: one_err::OneErr },
    #[snafu(display("Error connecting to chc: {}", source))]
    Chc { source: ChcError },
}
