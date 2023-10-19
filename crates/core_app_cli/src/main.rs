use anyhow::Result;
use structopt::StructOpt;

fn parse_tuple(arg: &str) -> Result<(String, String)> {
    let tuple_as_vec: Vec<&str> = arg.trim().split(",").collect();
    Ok((tuple_as_vec[0].to_string(), tuple_as_vec[1].to_string()))
}

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
    /// List all happs published by me
    #[structopt(name = "happs")]
    Happs,
    /// List all hosts for a happ by `happ_id``
    #[structopt(name = "hosts")]
    Hosts { happ_id: String },
    /// Fetch the happ preferences associated with a `pref_hash`
    #[structopt(name = "prefs")]
    GetPreferenceByHash {
        #[structopt(name = "hash")]
        pref_hash: String,
    },
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
        #[structopt(name = "max-time", parse(try_from_str = parse_tuple))]
        max_time_before_invoice: (String, String),
    },
}
impl Opt {
    /// Run this command
    pub async fn run(self) -> Result<()> {
        match self {
            Opt::Profile => core_app_cli::profile::get().await?,
            Opt::Ledger => core_app_cli::ledger::get().await?,
            Opt::Transactions => core_app_cli::list_all_tx::get().await?,
            Opt::PayInvoice => core_app_cli::pay_invoices::get().await?,
            Opt::Happs => core_app_cli::list_all_my_happs::get().await?,
            Opt::Hosts { happ_id } => core_app_cli::get_happ_hosts::get(happ_id).await?,
            Opt::GetPreferenceByHash { pref_hash } => {
                core_app_cli::get_specific_happ_prefs::get(pref_hash).await?
            }
            Opt::SetHappPreferences {
                happ_id,
                price_compute,
                price_bandwidth,
                price_storage,
                max_fuel_before_invoice,
                max_time_before_invoice,
            } => {
                core_app_cli::set_happ_prefs::get(
                    happ_id,
                    price_compute,
                    price_bandwidth,
                    price_storage,
                    max_fuel_before_invoice,
                    max_time_before_invoice,
                )
                .await?
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
