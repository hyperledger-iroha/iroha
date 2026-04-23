//! Re-encode canonical account literals across chain discriminants or derive contract subjects.

use clap::Parser;
use eyre::{Result, WrapErr as _, eyre};
use iroha::account_address::parse_account_address;
use iroha::data_model::smart_contract::ContractAddress;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, conflicts_with = "contract_address")]
    account: Option<String>,
    #[arg(long, conflicts_with = "account")]
    contract_address: Option<String>,
    #[arg(long)]
    from_chain_discriminant: Option<u16>,
    #[arg(long)]
    to_chain_discriminant: u16,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let account = if let Some(account) = args.account.as_deref() {
        let from_chain_discriminant = args
            .from_chain_discriminant
            .ok_or_else(|| eyre!("--from-chain-discriminant is required with --account"))?;
        parse_account_address(account, Some(from_chain_discriminant))
            .wrap_err("failed to parse --account as canonical account address")?
            .address
            .to_account_id()
            .map_err(|err| eyre!(err.to_string()))
            .wrap_err("failed to decode --account")?
    } else if let Some(contract_address) = args.contract_address.as_deref() {
        contract_address
            .parse::<ContractAddress>()
            .map_err(|err| eyre!(err.to_string()))
            .wrap_err("failed to parse --contract-address")?
            .subject_id()
    } else {
        return Err(eyre!("either --account or --contract-address is required"));
    };
    println!(
        "{}",
        account
            .to_i105_for_discriminant(args.to_chain_discriminant)
            .map_err(|err| eyre!(err.to_string()))
            .wrap_err("failed to encode re-mapped account address")?
    );
    Ok(())
}
