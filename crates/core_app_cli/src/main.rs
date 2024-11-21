use anyhow::Result;
use holochain_types::dna::AgentPubKeyB64;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "core-app-cli", about = "An example of StructOpt usage.")]
pub enum Opt {
    /// Gets profile details
    #[structopt(name = "pr")]
    Profile,
    /// Gets your balance, fees, promised and available Fuel
    #[structopt(name = "b")]
    Ledger,
    /// Gets the list of all your transactions
    #[structopt(name = "tx")]
    Transactions,
    /// Pay your first pending invoice
    #[structopt(name = "pay")]
    PayInvoice,
    /// List all happs registered in hha
    #[structopt(name = "all-happs")]
    AllHapps,
    /// List happ setting details
    #[structopt(name = "happ-details")]
    HappDetails { happ_id: String },
    /// Register happ
    #[structopt(name = "register")]
    RegisterHapp {
        #[structopt(name = "hosted-urls")]
        hosted_urls: Vec<String>,
        bundle_url: String,
        name: String,
        #[structopt(name = "special-uid")]
        uid: Option<String>,
        special_id: Option<String>,
    },
    /// List all happs published by me
    #[structopt(name = "my-happs")]
    HappsByMe,
    /// List all happs by provided publisher
    #[structopt(name = "publisher-happs")]
    GetHappsByPublisher { publisher_pubkey: String },
    /// List the jurisdiction for the provided agent
    #[structopt(name = "jurisdiction")]
    GetAgentsJurisdiction { agent_pubkey: String },
    /// List all hosts for a happ by `happ_id``
    #[structopt(name = "hosts")]
    Hosts { happ_id: String },
    /// Enable hosting for a specific happ
    #[structopt(name = "enable-happ")]
    EnableHappForHost { happ_id: String, host_id: String },
    /// Fetch the happ preferences associated with a happ preference hash
    #[structopt(name = "pref-details")]
    GetPreferenceByHash { pref_hash: String },
    /// Fetch the happ preference hash for a specific host for a specific happ
    #[structopt(name = "host-prefs")]
    GetHappPrefHashForHost { happ_id: String, host_id: String },
    /// Set new happ preferences
    #[structopt(name = "set-prefs")]
    SetHappPreferences {
        happ_id: String,
        #[structopt(name = "compute")]
        price_compute: String,
        #[structopt(name = "storage")]
        price_storage: String,
        #[structopt(name = "bandwidth")]
        price_bandwidth: String,
        #[structopt(name = "max-fuel")]
        max_fuel_before_invoice: String,
        #[structopt(name = "max-time-s")]
        max_time_before_invoice_sec: String,
        #[structopt(name = "max-time-ms")]
        max_time_before_invoice_ms: String,
    },
    /// Get My Summary
    #[structopt(name = "gms")]
    GetMySummary,
    /// Get Summary by providing an agent public key
    #[structopt(name = "gas")]
    GetAgentSummary { pub_key: String },
}
impl Opt {
    /// Run this command
    pub async fn run(self) -> Result<()> {
        match self {
            Opt::Profile => core_app_cli::profile::get().await?,
            Opt::Ledger => core_app_cli::ledger::get().await?,
            Opt::Transactions => core_app_cli::list_all_tx::get().await?,
            Opt::PayInvoice => core_app_cli::pay_invoices::get().await?,
            Opt::AllHapps => core_app_cli::list_all_happs::get().await?,
            Opt::HappDetails { happ_id } => core_app_cli::get_happ_details::get(happ_id).await?,
            Opt::RegisterHapp {
                hosted_urls,
                bundle_url,
                name,
                uid,
                special_id,
            } => {
                core_app_cli::register_happ::get(hosted_urls, bundle_url, name, uid, special_id)
                    .await?
            }
            Opt::HappsByMe => core_app_cli::list_all_my_happs::get().await?,
            Opt::Hosts { happ_id } => core_app_cli::get_happ_hosts::get(happ_id).await?,
            Opt::GetPreferenceByHash { pref_hash } => {
                core_app_cli::get_specific_happ_prefs::get(pref_hash).await?
            }
            Opt::GetHappsByPublisher { publisher_pubkey } => {
                core_app_cli::get_all_happs_by::get(publisher_pubkey).await?
            }
            Opt::GetAgentsJurisdiction { agent_pubkey } => {
                core_app_cli::get_agents_jurisdiction::get(agent_pubkey).await?
            }
            Opt::EnableHappForHost { happ_id, host_id } => {
                core_app_cli::enable_happ_for_host::get(happ_id, host_id).await?
            }
            Opt::GetHappPrefHashForHost { happ_id, host_id } => {
                core_app_cli::get_happ_pref_hash_for_host::get(happ_id, host_id).await?
            }
            Opt::SetHappPreferences {
                happ_id,
                price_compute,
                price_bandwidth,
                price_storage,
                max_fuel_before_invoice,
                max_time_before_invoice_sec,
                max_time_before_invoice_ms,
            } => {
                core_app_cli::set_host_happ_prefs::get(
                    happ_id,
                    price_compute,
                    price_bandwidth,
                    price_storage,
                    max_fuel_before_invoice,
                    max_time_before_invoice_sec,
                    max_time_before_invoice_ms,
                )
                .await?
            }
            Opt::GetMySummary => core_app_cli::summary::get_my_summary().await?,
            Opt::GetAgentSummary { pub_key } => {
                let pub_key = AgentPubKeyB64::from_b64_str(&pub_key)
                    .expect("Failed to serialize string into AgentPubKey");
                core_app_cli::summary::get_agent_summary(pub_key.into()).await?
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    opt.run().await
}
