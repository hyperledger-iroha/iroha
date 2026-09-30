//! CLI argument and presentation adapters for the shared native wallet account service.
use super::*;
use iroha_wallet::{
    onboarding::{FaucetRequest, OnboardingRequest, OnboardingService, PreparationOptions},
    operations::{OperationReport, OperationStatus},
};
/// Ordinary account bootstrap operations, independent of the public-reset coordinator.
#[derive(Debug, clap::Subcommand)]
pub(crate) enum AccountCommand {
    /// Register the configured account and bind an operator-authorized alias.
    #[command(subcommand)]
    Onboard(OnboardCommand),
    /// Fund the configured account using an independently trusted faucet policy.
    #[command(subcommand)]
    Faucet(FaucetCommand),
}

/// Explicit phases of account onboarding.
#[derive(Debug, clap::Subcommand)]
pub(crate) enum OnboardCommand {
    /// Verify a sponsored receipt and save the exact transaction without submitting it.
    Prepare(OnboardPrepare),
    /// Submit only the exact saved transaction, or report its existing outcome.
    Submit(OnboardSubmit),
    /// Read-only reconciliation of the saved transaction or account-and-alias proof.
    Resume(JournalArgs),
}

/// Explicit phases of a faucet claim.
#[derive(Debug, clap::Subcommand)]
pub(crate) enum FaucetCommand {
    /// Solve the native proof of work and save the verified transaction without submitting it.
    Prepare(FaucetPrepare),
    /// Submit only the exact saved transaction, or report its existing outcome.
    Submit(JournalArgs),
    /// Read-only reconciliation of the exact saved faucet transaction.
    Resume(JournalArgs),
}

/// Shared immutable-journal selection and bounded request duration.
#[derive(Debug, clap::Args)]
pub(crate) struct JournalArgs {
    /// Operation directory; preparation creates it privately and refuses to replace it.
    #[arg(long, value_name = "DIRECTORY")]
    journal: PathBuf,
    /// Maximum duration of each HTTP request in seconds.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=300))]
    timeout_secs: u64,
}

#[derive(Debug, clap::Args)]
struct PrepareArgs {
    #[command(flatten)]
    journal: JournalArgs,
    /// Optional exact operation ID; otherwise a fresh random 32-byte ID is retained in the journal.
    #[arg(long, value_parser = validate_request_id)]
    request_id: Option<String>,
    /// Maximum envelope lifetime; onboarding is additionally bounded by the signed receipt.
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=3600))]
    expires_in_secs: u64,
}

/// Private runtime-only onboarding credential selection.
#[derive(Debug, clap::Args)]
struct TokenArgs {
    /// Owner-private token file supplied by the deployment operator; never copied into the journal.
    #[arg(
        long,
        required_unless_present = "token_fd",
        conflicts_with = "token_fd"
    )]
    token_file: Option<PathBuf>,
    /// Explicit inherited owner-private token descriptor instead of a path.
    #[arg(
        long,
        required_unless_present = "token_file",
        conflicts_with = "token_file"
    )]
    token_fd: Option<u32>,
}

/// Trusted input for one sponsored account-and-alias preparation.
#[derive(Debug, clap::Args)]
pub(crate) struct OnboardPrepare {
    #[command(flatten)]
    prepare: PrepareArgs,
    #[command(flatten)]
    token: TokenArgs,
    /// Canonical alias to bind to the account in the client configuration.
    #[arg(long)]
    alias: String,
    /// Independently trusted onboarding issuer from the deployment operator.
    #[arg(long)]
    issuer: String,
    /// Exact additional unscoped permission authorized by the operator; repeat as needed.
    #[arg(long = "permission")]
    permissions: Vec<String>,
}

/// Explicit submission of an already retained onboarding transaction.
#[derive(Debug, clap::Args)]
pub(crate) struct OnboardSubmit {
    #[command(flatten)]
    journal: JournalArgs,
    #[command(flatten)]
    token: TokenArgs,
}

/// Trusted input for one faucet preparation.
#[derive(Debug, clap::Args)]
pub(crate) struct FaucetPrepare {
    #[command(flatten)]
    prepare: PrepareArgs,
    /// Independently trusted faucet issuer from the deployment operator.
    #[arg(long)]
    issuer: String,
    /// Exact canonical asset definition issued by the faucet.
    #[arg(long)]
    asset_definition: String,
    /// Exact positive quantity issued by one claim.
    #[arg(long)]
    amount: String,
}

impl TokenArgs {
    fn read(&self) -> Result<Zeroizing<String>> {
        match (&self.token_file, self.token_fd) {
            (Some(path), None) => read_onboarding_token_file(path),
            (None, Some(fd)) => read_onboarding_token_fd(fd),
            _ => eyre::bail!("onboarding requires exactly one private runtime token input"),
        }
    }
}

use iroha_wallet::onboarding::{canonical_issuer, validate_request_id};
impl PrepareArgs {
    fn options(&self) -> PreparationOptions {
        PreparationOptions {
            request_id: self.request_id.clone(),
            expires_in_secs: self.expires_in_secs,
            timeout_secs: self.journal.timeout_secs,
        }
    }
}
impl Run for AccountCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let config = context.config().clone();
        if config.chain.to_string() != DEFAULT_CHAIN_ID
            || config.account_chain_discriminant != DEFAULT_CHAIN_DISCRIMINANT
        {
            eyre::bail!(
                "Taira account operations require the canonical Taira chain and profile {DEFAULT_CHAIN_DISCRIMINANT}"
            );
        }
        let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
        let service = OnboardingService::new(config)?;
        let (report, preparing) = match self {
            Self::Onboard(OnboardCommand::Prepare(args)) => {
                let token = args.token.read()?;
                let request = OnboardingRequest {
                    alias: args.alias.clone(),
                    issuer: canonical_issuer(&args.issuer)?,
                    permissions: args.permissions.clone(),
                    fee_payment: context.transaction_fee_payment()?,
                };
                (
                    service.prepare_onboarding(
                        &request,
                        &token,
                        &args.prepare.options(),
                        &args.prepare.journal.journal,
                    )?,
                    true,
                )
            }
            Self::Faucet(FaucetCommand::Prepare(args)) => {
                let asset_definition: AssetDefinitionId = args.asset_definition.parse()?;
                let amount: Quantity = args.amount.parse()?;
                if asset_definition.to_string() != args.asset_definition
                    || amount.to_string() != args.amount
                {
                    eyre::bail!(
                        "faucet asset definition and amount must use exact canonical spellings"
                    );
                }
                let request = FaucetRequest {
                    issuer: canonical_issuer(&args.issuer)?,
                    asset_definition,
                    amount,
                    fee_payment: context.transaction_fee_payment()?,
                };
                (
                    service.prepare_faucet(
                        &request,
                        &args.prepare.options(),
                        &args.prepare.journal.journal,
                    )?,
                    true,
                )
            }
            Self::Onboard(OnboardCommand::Submit(args)) => {
                let token = args.token.read()?;
                (
                    service.submit_onboarding(
                        &args.journal.journal,
                        &token,
                        args.journal.timeout_secs,
                    )?,
                    false,
                )
            }
            Self::Onboard(OnboardCommand::Resume(args)) => (
                service.resume_onboarding(&args.journal, args.timeout_secs)?,
                false,
            ),
            Self::Faucet(FaucetCommand::Submit(args)) => (
                service.submit_faucet(&args.journal, args.timeout_secs)?,
                false,
            ),
            Self::Faucet(FaucetCommand::Resume(args)) => (
                service.resume_faucet(&args.journal, args.timeout_secs)?,
                false,
            ),
        };
        render_report(context, &report)?;
        if preparing {
            Ok(())
        } else {
            report.require_complete()
        }
    }
}
fn render_report<C: RunContext>(context: &mut C, report: &OperationReport) -> Result<()> {
    let value = &report.data;
    if context.output_format() == CliOutputFormat::Json {
        return context.print_data(value);
    }
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or("");
    let kind = text("operation");
    let status = report.status.as_str();
    context.println(format!("{status}: {kind} for {}", text("account_id")))?;
    context.println(format!(
        "Network: {}\nAddress profile: {}\nExpires at (Unix ms): {}\nFee intent: {}",
        text("network_id"),
        json::to_string(&value["chain_discriminant"])?,
        json::to_string(&value["expires_at_unix_ms"])?,
        json::to_string(&value["fee_payment"])?
    ))?;
    if !text("transaction_hash").is_empty() {
        context.println(format!("Transaction: {}", text("transaction_hash")))?;
    }
    if kind == "onboarding" {
        context.println(format!(
            "Alias: {}\nIssuer: {}",
            text("alias"),
            text("issuer")
        ))?;
        if let Some(permissions) = value["permissions_requested"].as_array() {
            if !permissions.is_empty() {
                context.println(format!(
                    "Permissions requested: {}",
                    permissions
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))?;
            }
        }
        if value["owner_auto_renew_follow_up"].as_bool() == Some(true) {
            context.println("Alias auto-renew requires the separately signed owner follow-up recorded in the receipt.")?;
        }
    } else {
        context.println(format!(
            "Funding: {} of {}\nIssuer: {}",
            text("amount"),
            text("asset_definition"),
            text("issuer")
        ))?;
    }
    context.println(format!("Journal: {}", text("journal")))?;
    if !matches!(
        report.status,
        OperationStatus::Applied
            | OperationStatus::AlreadyPresent
            | OperationStatus::Rejected
            | OperationStatus::AliasConflict
            | OperationStatus::Expired
    ) {
        let command = if kind == "onboarding" {
            "onboard"
        } else {
            "faucet"
        };
        let next = if report.status == OperationStatus::Prepared {
            "submit"
        } else {
            "resume"
        };
        context.println(format!(
            "Next: iroha taira account {command} {next} --journal <same directory>{}",
            if command == "onboard" && next == "submit" {
                " --token-file <private token>"
            } else {
                ""
            }
        ))?;
    }
    Ok(())
}
#[cfg(test)]
#[path = "taira_onboarding_tests.rs"]
mod tests;
