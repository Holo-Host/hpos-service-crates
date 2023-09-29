use anyhow::Result;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
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
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    opt.run().await
}
