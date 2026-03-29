//! Direct numeric asset transfer helper for live operator workflows.

use std::{path::PathBuf, str::FromStr};

use clap::Parser;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    client::Client,
    config::{Config, LoadPath},
    data_model::{
        account::address::ChainDiscriminantGuard,
        asset::{AssetDefinitionId, AssetId},
        isi::Transfer,
        prelude::*,
    },
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    from: String,
    #[arg(long)]
    to: String,
    #[arg(long)]
    asset_definition: String,
    #[arg(long)]
    quantity: String,
    #[arg(long, default_value_t = 753)]
    chain_discriminant: u16,
}

fn parse_account(raw: &str) -> Result<AccountId> {
    parse_account_address(raw, None)
        .wrap_err("failed to parse account address")?
        .address
        .to_account_id()
        .map_err(|err| eyre!(err.to_string()))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::load(LoadPath::Explicit(&args.config))
        .map_err(|err| eyre!(err.to_string()))
        .wrap_err_with(|| format!("load config {}", args.config.display()))?;
    let client = Client::new(config);
    let _chain_guard = ChainDiscriminantGuard::enter(args.chain_discriminant);
    let from = parse_account(&args.from).wrap_err("decode --from")?;
    let to = parse_account(&args.to).wrap_err("decode --to")?;
    let definition =
        AssetDefinitionId::from_str(&args.asset_definition).wrap_err("parse --asset-definition")?;
    let quantity: Numeric = args.quantity.parse().wrap_err("parse --quantity")?;
    let tx_hash = client.submit_blocking(Transfer::asset_numeric(
        AssetId::new(definition, from),
        quantity,
        to,
    ))?;
    println!("{tx_hash}");
    Ok(())
}
