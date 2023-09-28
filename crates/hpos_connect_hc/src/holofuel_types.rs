use anyhow::{Context, Result};
use holochain_types::prelude::{
    holochain_serial, ActionHashB64, AgentPubKeyB64, AnyDhtHashB64, CapSecret, EntryHashB64,
    SerializedBytes, Timestamp, X25519PubKey,
};
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use tracing::debug;

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
    pub proof_of_service_token: Option<CapSecret>,
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
    Actionable, // tx that is create by 1st instance and waiting for counterparty to complete the tx
    Pending,    // tx that was created by 1st instance and second instance
    Accepted,   // tx that was accepted by counterparty but has yet to complete countersigning.
    Completed,
    Declined,
    Expired,
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
