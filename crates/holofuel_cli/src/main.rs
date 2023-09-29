use anyhow::Result;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum Opt {
    /// Gets your balance, fees, promised and available Fuel
    #[structopt(name = "b")]
    Ledger,
    /// Gets the list of your pending transactions
    #[structopt(name = "p")]
    Pending,
    /// Gets the list of your actionable transactions
    #[structopt(name = "a")]
    Actionable,
    /// Gets the list of your completed transactions
    #[structopt(name = "c")]
    Completed,
    /// Gets profile details
    #[structopt(name = "pr")]
    Profile,
    /// Get All Reserves Accounts Settings
    #[structopt(name = "rs")]
    ReserveSetting,
    /// Get this reserve accounts sales price
    #[structopt(name = "rsp")]
    ReserveSalePrice,
}
impl Opt {
    /// Run this command
    pub async fn run(self) -> Result<()> {
        match self {
            Opt::Ledger => hf::actions::ledger::get().await?,
            Opt::Pending => hf::actions::pending::get().await?,
            Opt::Actionable => hf::actions::actionable::get().await?,
            Opt::Completed => hf::actions::completed::get().await?,
            Opt::Profile => hf::actions::profile::get().await?,
            Opt::ReserveSetting => hf::actions::reserve::get_setting().await?,
            Opt::ReserveSalePrice => hf::actions::reserve::get_sale_price().await?,
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    opt.run().await
}
