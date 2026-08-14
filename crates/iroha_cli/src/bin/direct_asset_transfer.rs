//! Direct asset quantity transfer helper for live operator workflows.
use clap::{Parser, ValueEnum};
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
        transaction::FeePaymentIntent,
    },
};
use std::{path::PathBuf, str::FromStr};
#[derive(Clone, Copy, Debug, ValueEnum)]
enum FeePayer {
    Authority,
    Sponsor,
}
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
    #[arg(long, value_enum)]
    fee_payer: FeePayer,
    #[arg(long, value_name = "PROGRAM_ID")]
    fee_program: Option<String>,
    #[arg(long, value_name = "NONZERO_U64")]
    fee_program_revision: Option<u64>,
}
fn parse_account(raw: &str) -> Result<AccountId> {
    parse_account_address(raw, None)
        .wrap_err("failed to parse account address")?
        .address
        .to_account_id()
        .map_err(|err| eyre!(err.to_string()))
}
fn fee_payment(args: &Args) -> Result<FeePaymentIntent> {
    match args.fee_payer {
        FeePayer::Authority => {
            if args.fee_program.is_some() || args.fee_program_revision.is_some() {
                eyre::bail!("--fee-program and --fee-program-revision require --fee-payer sponsor");
            }
            Ok(FeePaymentIntent::authority(Vec::new(), None))
        }
        FeePayer::Sponsor => {
            let program_id = args
                .fee_program
                .as_deref()
                .ok_or_else(|| eyre!("--fee-payer sponsor requires --fee-program"))?
                .parse()
                .wrap_err("parse --fee-program")?;
            let revision = args
                .fee_program_revision
                .ok_or_else(|| eyre!("--fee-payer sponsor requires --fee-program-revision"))?;
            if revision == 0 {
                eyre::bail!("--fee-program-revision must be greater than zero");
            }
            Ok(FeePaymentIntent::sponsor(
                program_id,
                revision,
                Vec::new(),
                None,
            ))
        }
    }
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
    let quantity: Quantity = args.quantity.parse().wrap_err("parse --quantity")?;
    let fee_payment = fee_payment(&args)?;
    let tx_hash = client.submit_blocking(
        Transfer::asset_quantity(AssetId::new(definition, from), quantity, to),
        fee_payment,
    )?;
    println!("{tx_hash}");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args_with(extra: &[&str]) -> Args {
        let mut argv = vec![
            "direct_asset_transfer",
            "--config",
            "client.toml",
            "--from",
            "from",
            "--to",
            "to",
            "--asset-definition",
            "asset",
            "--quantity",
            "1",
        ];
        argv.extend_from_slice(extra);
        Args::try_parse_from(argv).expect("parse fee selection fixture")
    }
    #[test]
    fn fee_payment_requires_explicit_consistent_payer_selection() {
        assert!(matches!(
            fee_payment(&args_with(&["--fee-payer", "authority"])),
            Ok(FeePaymentIntent::Authority(_))
        ));
        let sponsor = args_with(&["--fee-payer", "sponsor"]);
        assert!(
            fee_payment(&sponsor)
                .expect_err("sponsor must identify an exact program")
                .to_string()
                .contains("--fee-program")
        );
    }
}
