use anyhow::{Context, Result};
use holochain_types::prelude::ActionHash;
use holochain_types::prelude::AgentPubKey;
use holochain_types::prelude::AnyLinkableHash;
use holochain_types::prelude::EntryHash;
use holochain_types::prelude::Signature;
use holochain_types::prelude::{
    holochain_serial, ActionHashB64, AgentPubKeyB64, AnyDhtHashB64, CapSecret, EntryHashB64,
    SerializedBytes, Timestamp, X25519PubKey,
};
use holofuel_types::fuel::Fuel;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use tracing::debug;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct PendingTransaction {
    pub invoice_pending: Vec<Transaction>,
    pub promise_pending: Vec<Transaction>,
    pub invoice_declined: Vec<Transaction>,
    pub promise_declined: Vec<Transaction>,
    pub accepted: Vec<Transaction>,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct Ledger {
    pub balance: String,
    pub promised: String,
    pub fees: String,
    pub available: String,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct Pending {
    pub invoice_pending: Vec<Transaction>,
    pub promise_pending: Vec<Transaction>,
    pub invoice_declined: Vec<Transaction>,
    pub promise_declined: Vec<Transaction>,
    pub accepted: Vec<Transaction>,
}
#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct Actionable {
    pub invoice_actionable: Vec<Transaction>,
    pub promise_actionable: Vec<Transaction>,
}

#[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone, PartialEq)]
pub enum CounterSigningResponse {
    Successful(EntryHashB64),
    UnableToReachCounterparty(String),
    FeeDropOff(String),
    TimeDelayWait(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum POS {
    Hosting(CapSecret),
    Redemption(String), // Contains wallet address
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct Transaction {
    pub id: EntryHashB64,
    pub amount: String,
    pub fee: String,
    pub created_date: Timestamp,
    pub completed_date: Option<Timestamp>,
    pub transaction_type: TransactionType,
    pub counterparty: AgentPubKeyB64,
    pub direction: TransactionDirection,
    pub status: TransactionStatus,
    pub note: Option<String>,
    pub proof_of_service: Option<POS>,
    pub url: Option<String>,
    pub expiration_date: Option<Timestamp>,
}
#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub enum TransactionType {
    Request, //Invoice
    Offer,   //Promise
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub enum TransactionDirection {
    Outgoing, // To(Address),
    Incoming, // From(Address),
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub enum TransactionStatus {
    Actionable, // tx that was created by 1st instance and is awaiting acceptance by the counterparty to complete the tx
    Pending, // tx that was created by 1st instance and second instance (reciprocal state is either "actionable" or "awaiting countersigning")
    Accepted(AcceptedBy), // tx that was accepted by the counterparty, but has yet to complete countersigning.
    Completed,
    Declined,
    Expired,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AcceptedBy {
    ByMe,           // In this scenario, my agent is the counteryparty of the original tx
    ByCounterParty, // In this scenario, my agent is the author of the original tx
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct Profile {
    pub agent_address: AgentPubKeyB64,
    pub nickname: Option<String>,
    pub avatar_url: Option<String>,
    pub uniqueness: AnyDhtHashB64,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct Reserve {
    pub reserve_id: ActionHashB64,
    pub pub_key: AgentPubKeyB64,
    pub info: ReserveSetting,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct ReserveSetting {
    pub external_reserve_currency: String,
    pub external_account_number: String,
    pub external_signing_key: X25519PubKey,
    pub default_promise_expiry: Duration,
    pub min_external_currency_tx_size: String,
    pub max_external_currency_tx_size: String,
    note: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct ReserveSettingFile {
    pub external_reserve_currency: String,
    pub external_account_number: String,
    pub default_promise_expiry: Duration,
    pub min_external_currency_tx_size: String,
    pub max_external_currency_tx_size: String,
    note: Option<String>,
}
impl ReserveSettingFile {
    pub fn load_happ_file() -> Result<Self> {
        debug!("loading happ file");
        let path = std::env::var("REGISTER_RESERVE")
            .context("Failed to read REGISTER_RESERVE. Is it set in env?")?;
        debug!("got path {}", path);
        // let file = File::open(path).context("failed to open file")?;
        let file = std::fs::read(path)?;
        debug!("got file: {:?}", file);
        let happ_file =
            serde_json::from_slice(&file).context("failed to deserialize YAML as HappsFile")?;
        debug!("happ file {:?}", happ_file);
        Ok(happ_file)
    }

    pub fn into_reserve_settings(self, agent_pub_key: X25519PubKey) -> ReserveSetting {
        ReserveSetting {
            external_reserve_currency: self.external_reserve_currency,
            external_account_number: self.external_account_number,
            external_signing_key: agent_pub_key,
            default_promise_expiry: self.default_promise_expiry,
            min_external_currency_tx_size: self.min_external_currency_tx_size,
            max_external_currency_tx_size: self.max_external_currency_tx_size,
            note: self.note,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct ReserveSalePrice {
    pub latest_unit_price: String, // Number of HF units one external currency unit purchases, as determined by periodic (scheduled) runs of the pricing algorithm
    pub inputs_used: Vec<String>,
}

/// Summary
///

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct MigrationCloseStateV1Handler {
    pub opening_balance: Fuel,
    pub closing_balance: Fuel,
    pub number_of_declined: usize,
    pub multi_sig_authorizer: Option<MultiSigAuthorizers>,
    pub reserve_setting: Option<ReserveSetting>,
    pub reserve_sale_price: Option<ReserveSalePrice>,
    pub cs_txs: Vec<CounterSignedTxBundle>,
    pub tx_parked_links: Vec<TxParkedLink>,
    pub incomplete_invoice_txs: Vec<InvoiceBundle>,
    pub incomplete_promise_txs: Vec<PromiseBundle>,
}

pub type CounterSignedTxBundle = (CounterSignedTx, ActionHash, Timestamp);
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, SerializedBytes)]
pub struct TxParkedLink {
    pub id: ActionHash,
    pub parking_spot_hash: AnyLinkableHash,
    pub timestamp: Timestamp,
    pub fees: Fuel,
}

pub type InvoiceBundle = (Invoice, EntryHash, Timestamp);
pub type PromiseBundle = (Promise, EntryHash, Timestamp);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, SerializedBytes)]
pub struct Promise {
    pub timestamp: Timestamp,
    pub promise_details: TxDetails,
    pub fee: Fuel,
    pub expiration_date: Timestamp,
    pub invoice_hash: Option<EntryHash>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, SerializedBytes)]
pub struct Invoice {
    pub timestamp: Timestamp,
    pub invoice_details: TxDetails,
    pub expiration_date: Option<Timestamp>,
    pub promise_hash: Option<EntryHash>,
}
#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes, PartialEq, Eq)]
pub struct TxDetails {
    pub spender: AgentPubKey,
    pub receiver: AgentPubKey,
    pub amount: Fuel,
    pub payload: Payload,
}
#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct MultiSigAuthorizers(Vec<AuthorizerRules>);
#[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone, PartialEq, Eq)]
pub struct AuthorizerRules {
    condition: MultiSigCondition, // MultiSig expected when conditions are met
    m_of_n: i8,                   // is the M of N of the signers whose signatures are required
    signer_keys: Vec<X25519PubKey>, // are a vector of keys used for authorization of the action
}
#[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone, PartialEq, Eq)]
pub struct MultiSigCondition {
    above_hf_amt: Fuel, // MultiSig will be expected on values above this amt
}

// Countersigning tx
#[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone, PartialEq, Eq)]
pub struct CounterSignedTx {
    pub tx_body: CounterSignedTxBody,
    pub spender_state: FuelState,
    pub receiver_state: FuelState,
}
#[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone, PartialEq, Eq)]
pub struct FuelState {
    pub new_bal: Fuel,
    pub new_promise: Fuel,
    pub tx_fees_owed: Fuel,
    pub tx_body_signature: Signature,
}
#[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone, PartialEq, Eq)]
pub struct CounterSignedTxBody {
    pub tx_amt: Fuel,
    pub tx_fee: Fuel,
    pub spender_payload: Payload,
    pub receiver_payload: Payload,
    pub spender_chain_info: ChainInfo,
    pub receiver_chain_info: ChainInfo,
}
#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes, Default, PartialEq, Eq)]
pub struct Payload {
    pub note: Option<NoteTypes>,
    pub proof_of_service: Option<POS>,
    pub url: Option<String>,
}
#[derive(Serialize, Deserialize, SerializedBytes, Debug, Clone, PartialEq, Eq)]
pub struct ChainInfo {
    pub agent_address: AgentPubKey,
    pub pre_auth: EntryHash,
    pub prior_action: ActionHash,
    pub tx_seq_num: u32,
}
#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes, PartialEq, Eq)]
pub enum NoteTypes {
    ReserveNote(ReserveNote),
    MultiSigPayload(MultiSigNote),
    SimpleNote(String),
}
#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes, PartialEq, Eq)]
pub struct ReserveNote {
    pub details: ReserveProof,
    pub signature: Signature,
    pub extra_note: Option<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone, SerializedBytes, PartialEq, Eq)]
pub struct ReserveProof {
    pub ext_amt_transferred: Fuel, // Amount of external currency
    pub nonce: String,
    pub reserve_sales_price: Fuel, // Number of HF units one external currency unit purchases, at the time of promise or request based off the ReserveSalePrice
    pub ext_tx_id: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MultiSigNote {
    pub details: MultiSigPayload,
    pub list_of_sig: Vec<Signature>,
    pub extra_note: Option<String>,
}
#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, Clone, PartialEq, Eq)]
pub struct MultiSigPayload {
    pub role: String,
    pub amt: Fuel,
    pub counterparty: String, // using AgentPubKeyB64 has serde deserialization issues
    pub auth_date: Timestamp,
}
#[cfg(test)]
pub mod tests {
    use crate::holofuel_types::ReserveSettingFile;

    #[test]
    fn read_file() {
        use std::env::set_var;
        set_var("REGISTER_RESERVE", "./test/reserve_details.json");
        ReserveSettingFile::load_happ_file().unwrap();
    }
}
