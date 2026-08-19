mod address;
mod audit;
#[cfg(feature = "bridge")]
mod bridge;
mod cli_output;
mod commands;
mod compute;
mod confidential;
#[cfg(test)]
mod config_utils;
mod content;
mod contracts;
mod crypto;
mod endorsement;
mod gov;
mod ivm_cli;
mod json_utils;
mod jurisdiction;
mod list_support;
mod nexus;
mod offline;
mod operator_key;
mod runtime;
mod soracloud;
mod space_directory;
mod staking;
mod subscriptions;
mod sumeragi;
mod taira;
mod zk; // ZK helpers (app API convenience) // IVM/ABI helpers
use clap::{ArgAction, CommandFactory, FromArgMatches, error::ErrorKind};
use iroha_i18n::{Bundle, Localizer, detect_language};
use std::{
    fmt::Display,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::LazyLock,
    time::Duration,
};
use error_stack::{IntoReportCompat, Report, ResultExt, fmt::ColorMode};
use eyre::{Result, WrapErr, eyre};
use futures::{TryStreamExt, stream::TryStream};
use iroha::data_model::account::address::ChainDiscriminantGuard;
use iroha::{
    client::Client,
    config::{Config, LoadPath},
    data_model::{prelude::*, transaction::IvmBytecode},
};
use iroha_config::parameters::{actual::SorafsRolloutPhase, defaults};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_torii_shared::{ErrorEnvelope, FeeQuoteResponse};
use std::num::NonZeroU64;
use thiserror::Error;
use tokio::runtime::Runtime;
// For base64 Engine trait (decode)
use base64::Engine as _;
use norito::json::{self, JsonDeserialize, JsonSerialize};
use sorafs_manifest::alias_cache::AliasCachePolicy;
use sorafs_orchestrator::AnonymityPolicy;
use url::Url;
const VERGEN_GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {
    Some(value) => value,
    None => "unknown",
};
// The first-release CLI accepts instruction JSON and bytecode through stdin. Sixty-four MiB is
// above the default 10 MiB transaction wire limit (including JSON/base64 expansion) and matches
// the largest configured signed-transaction corridor, while keeping a pipe from consuming
// unbounded resident memory before semantic admission runs.
const MAX_CLI_STDIN_BYTES_V1: usize = 64 * 1024 * 1024;
// The transaction model admits at most 100,000 instructions. The aggregate allowance leaves ten
// lexical values per instruction, while the allocation budget bounds typed decoding after the
// allocation-free lexical preflight has accepted the document.
const MAX_CLI_JSON_SEQUENCE_ELEMENTS_V1: usize = 100_000;
const MAX_CLI_JSON_FIELD_BYTES_V1: usize = 16 * 1024 * 1024;
const MAX_CLI_JSON_TOTAL_ELEMENTS_V1: usize = 1_000_000;
const MAX_CLI_JSON_DECODE_ALLOCATION_BYTES_V1: usize = 128 * 1024 * 1024;
const MAX_CLI_JSON_NESTING_DEPTH_V1: usize = 64;
const CLI_JSON_DECODE_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    MAX_CLI_JSON_SEQUENCE_ELEMENTS_V1,
    MAX_CLI_JSON_FIELD_BYTES_V1,
    MAX_CLI_JSON_TOTAL_ELEMENTS_V1,
    MAX_CLI_JSON_DECODE_ALLOCATION_BYTES_V1,
    MAX_CLI_JSON_NESTING_DEPTH_V1,
);
fn validate_executable_fee_payment(
    executable: &Executable,
    fee_payment: &FeePaymentIntent,
) -> Result<()> {
    if !executable.requires_transaction_gas_limit() {
        return Ok(());
    }
    iroha::data_model::transaction::require_transaction_gas_limit(fee_payment)
        .map(|_| ())
        .map_err(|err| eyre!(format_gas_limit_validation_error(executable, err)))
}
fn format_gas_limit_validation_error(
    executable: &Executable,
    err: iroha::data_model::transaction::TransactionGasLimitError,
) -> String {
    let executable_label = match executable {
        Executable::Instructions(_) => "instruction transactions",
        Executable::ContractCall(_) => "contract-call transactions",
        Executable::Batch(_) => "mixed executable batches",
        Executable::Ivm(_) | Executable::IvmProved(_) => "IVM transactions",
    };
    match err {
        iroha::data_model::transaction::TransactionGasLimitError::Missing => {
            if matches!(executable, Executable::Ivm(_) | Executable::IvmProved(_)) {
                format!(
                    "{executable_label} require a signature-bound gas limit; pass `--gas-limit <u64>`"
                )
            } else {
                format!(
                    "{executable_label} require a signature-bound gas limit in the selected fee payment intent"
                )
            }
        }
    }
}
pub(crate) fn apply_cli_gas_limit_override(
    fee_payment: FeePaymentIntent,
    gas_limit: Option<u64>,
) -> Result<FeePaymentIntent> {
    let Some(gas_limit) = gas_limit else {
        return Ok(fee_payment);
    };
    let gas_limit =
        NonZeroU64::new(gas_limit).ok_or_else(|| eyre!("--gas-limit must be greater than zero"))?;
    Ok(match fee_payment {
        FeePaymentIntent::Authority(payment) => {
            FeePaymentIntent::authority(payment.charge_limits, Some(gas_limit))
        }
        FeePaymentIntent::Sponsor(payment) => FeePaymentIntent::sponsor(
            payment.program_id,
            payment.program_revision,
            payment.charge_limits,
            Some(gas_limit),
        ),
    })
}
fn fee_quote_rejection_message(status: reqwest::StatusCode, body: &[u8]) -> String {
    let Ok(envelope) = norito::json::from_slice::<ErrorEnvelope>(body) else {
        let fallback = String::from_utf8_lossy(body);
        return format!(
            "fee quote request failed with HTTP {status}: {}",
            fallback.trim()
        );
    };
    let mut message = format!(
        "fee quote rejected with HTTP {status} [{}]: {}",
        envelope.code, envelope.message
    );
    if let Some(fee) = envelope
        .details
        .as_ref()
        .and_then(|details| details.fee.as_ref())
    {
        use std::fmt::Write as _;
        let _ = write!(
            message,
            "; fee_code={}; retryable={}",
            fee.code, fee.retryable
        );
        if let Some(program_id) = &fee.program_id {
            let _ = write!(message, "; program={program_id}");
        }
        if let Some(revision) = fee.program_revision {
            let _ = write!(message, "; revision={revision}");
        }
        if let Some(asset_definition_id) = &fee.asset_definition_id {
            let _ = write!(message, "; asset={asset_definition_id}");
        }
        if let Some(required) = &fee.required {
            let _ = write!(message, "; required={required}");
        }
        if let Some(available) = &fee.available {
            let _ = write!(message, "; available={available}");
        }
        if let Some(rule_id) = &fee.rule_id {
            let _ = write!(message, "; rule={rule_id}");
        }
        if let Some(height) = fee.observation_height {
            let _ = write!(message, "; observation_height={height}");
        }
        if let Some(remediation) = &fee.remediation {
            let _ = write!(message, "; remediation: {remediation}");
        }
    }
    message
}
pub(crate) fn quote_and_sign_transaction(
    client: &Client,
    executable: Executable,
    requested_fee_payment: FeePaymentIntent,
    metadata: Metadata,
) -> Result<(SignedTransaction, FeeQuoteResponse)> {
    validate_executable_fee_payment(&executable, &requested_fee_payment)?;
    let mut payload = client
        .try_build_transaction_payload(executable.clone(), requested_fee_payment.clone(), metadata)
        .wrap_err("Failed to build exact unsigned transaction payload for fee quoting")?;
    let response = client
        .post_fee_quote_response(&payload)
        .wrap_err("Failed to request an exact transaction fee quote")?;
    if !response.status().is_success() {
        return Err(eyre!(fee_quote_rejection_message(
            response.status(),
            response.body(),
        )));
    }
    let quote: FeeQuoteResponse = norito::json::from_slice(response.body())
        .wrap_err("Failed to decode the exact transaction fee quote")?;
    if !requested_fee_payment.has_same_payer_and_gas_bound(&quote.intent) {
        eyre::bail!(
            "fee quote changed the selected payer, sponsor revision, or gas bound; refusing to sign"
        );
    }
    quote
        .intent
        .validate()
        .wrap_err("Fee quote returned an invalid signature-bound payment intent")?;
    validate_executable_fee_payment(&executable, &quote.intent)?;
    payload.fee_payment = quote.intent.clone();
    let transaction = client
        .try_sign_transaction_payload(payload)
        .wrap_err("Failed to sign the exact quoted transaction payload")?;
    Ok((transaction, quote))
}
fn print_fee_quote_text<C: RunContext + ?Sized>(
    context: &mut C,
    quote: &FeeQuoteResponse,
) -> Result<()> {
    context.println(format_args!(
        "fee quote accepted: observation_height={}, ledger_time_ms={}",
        quote.observation.next_block_height, quote.observation.ledger_time_ms
    ))?;
    for component in &quote.components {
        context.println(format_args!(
            "  maximum {:?}: {} {}",
            component.kind, component.max_amount, component.asset_definition_id
        ))?;
    }
    for capacity in &quote.capacities {
        context.println(format_args!(
            "  sponsor capacity {}: vault={}, reserve={}, block_remaining={}, program_epoch_remaining={}, beneficiary_epoch_remaining={}",
            capacity.asset_definition_id,
            capacity.vault_balance,
            capacity.reserve_floor,
            capacity.block_remaining,
            capacity.program_epoch_remaining,
            capacity.beneficiary_epoch_remaining,
        ))?;
    }
    Ok(())
}
/// Norito JSON derive macros exported for CLI data definitions.
pub mod json_macros {
    pub use norito::derive::{FastJsonWrite, JsonDeserialize, JsonSerialize};
}
/// Output format for CLI responses.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliOutputFormat {
    /// Emit JSON only.
    Json,
    /// Emit human-readable text when available.
    Text,
}
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransactionWaitTerminalStatusArg {
    Queued,
    Approved,
    Committed,
    Applied,
    Rejected,
    Expired,
}
/// Signature-bound source selected for transaction fees.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum FeePayerArg {
    /// Charge the transaction authority directly.
    Authority,
    /// Charge one exact immutable sponsor-program revision.
    Sponsor,
}
#[derive(clap::Args, Clone, Debug, Default)]
struct FeePaymentArgs {
    /// Required fee source for every submitted transaction.
    #[arg(long, value_enum, global = true)]
    fee_payer: Option<FeePayerArg>,
    /// Exact sponsor program (`<canonical-I105>/<name>`); valid only with `--fee-payer sponsor`.
    #[arg(long, global = true, value_name = "PROGRAM_ID")]
    fee_program: Option<String>,
    /// Exact immutable sponsor-program revision; valid only with `--fee-payer sponsor`.
    #[arg(long, global = true, value_name = "NONZERO_U64")]
    fee_program_revision: Option<u64>,
}
impl FeePaymentArgs {
    fn selection(&self) -> Result<FeePaymentIntent> {
        match self.fee_payer {
            Some(FeePayerArg::Authority) => {
                if self.fee_program.is_some() || self.fee_program_revision.is_some() {
                    eyre::bail!(
                        "--fee-program and --fee-program-revision require `--fee-payer sponsor`"
                    );
                }
                Ok(FeePaymentIntent::authority(Vec::new(), None))
            }
            Some(FeePayerArg::Sponsor) => {
                let program_id = self
                    .fee_program
                    .as_deref()
                    .ok_or_else(|| eyre!("--fee-payer sponsor requires --fee-program"))?
                    .parse()
                    .wrap_err("invalid --fee-program")?;
                let revision = self
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
            None => eyre::bail!(
                "transaction submission requires an explicit `--fee-payer authority` or `--fee-payer sponsor --fee-program <id> --fee-program-revision <revision>`"
            ),
        }
    }
}
impl From<TransactionWaitTerminalStatusArg> for iroha::client::TransactionWaitTerminalStatus {
    fn from(value: TransactionWaitTerminalStatusArg) -> Self {
        match value {
            TransactionWaitTerminalStatusArg::Queued => Self::Queued,
            TransactionWaitTerminalStatusArg::Approved => Self::Approved,
            TransactionWaitTerminalStatusArg::Committed => Self::Committed,
            TransactionWaitTerminalStatusArg::Applied => Self::Applied,
            TransactionWaitTerminalStatusArg::Rejected => Self::Rejected,
            TransactionWaitTerminalStatusArg::Expired => Self::Expired,
        }
    }
}
#[derive(clap::Args, Debug, Clone)]
pub(crate) struct TransactionWaitArgs {
    /// Poll `/v1/pipeline/transactions/status` until the transaction reaches Applied finality.
    #[arg(long)]
    pub wait: bool,
    /// Submit the transaction without waiting for finality.
    #[arg(long, conflicts_with = "wait")]
    pub submit_only: bool,
    /// Maximum time to wait before failing.
    #[arg(long, default_value_t = 30_000)]
    pub timeout_ms: u64,
    /// Poll interval used while waiting.
    #[arg(long, default_value_t = 500)]
    pub poll_interval_ms: u64,
    /// Stop when the pipeline reaches any of these statuses instead of the default Applied finality.
    #[arg(
        long = "terminal-status",
        value_enum,
        action = ArgAction::Append
    )]
    pub terminal_statuses: Vec<TransactionWaitTerminalStatusArg>,
}
impl TransactionWaitArgs {
    pub(crate) fn is_enabled(&self) -> bool {
        !self.submit_only
    }
    pub(crate) fn to_options(&self) -> Result<iroha::client::TransactionWaitOptions> {
        if self.poll_interval_ms == 0 {
            eyre::bail!("--poll-interval-ms must be greater than 0");
        }
        Ok(iroha::client::TransactionWaitOptions {
            timeout: Duration::from_millis(self.timeout_ms),
            poll_interval: Duration::from_millis(self.poll_interval_ms),
            terminal_statuses: self
                .terminal_statuses
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
        })
    }
}
pub(crate) fn wait_for_transaction_status(
    client: &Client,
    hash: HashOf<iroha::data_model::transaction::SignedTransaction>,
    wait: &TransactionWaitArgs,
) -> Result<iroha::client::TransactionWaitOutcome> {
    client.wait_for_transaction_terminal_status(hash, wait.to_options()?)
}
/// Iroha Client CLI provides a simple way to interact with the Iroha Web API.
#[derive(clap::Parser, Debug)]
#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]
struct Args {
    /// Path to the configuration file.
    ///
    /// By default, `iroha` reads `client.toml`; runtime commands require it to be present and readable.
    #[arg(short, long, value_name("PATH"))]
    config: Option<PathBuf>,
    /// Absolute path to an owner-only operator private-key file for operator reads.
    ///
    /// This runtime-only credential is never inferred from the account key, environment, or
    /// client TOML. The selected node must allowlist its public key for the configured exact
    /// NetworkId.
    #[arg(long, value_name("ABSOLUTE_PATH"))]
    operator_private_key_file: Option<PathBuf>,
    /// Print configuration details to stderr
    #[arg(short, long)]
    verbose: bool,
    /// Path to a JSON file for attaching transaction metadata (optional)
    #[arg(short, long, value_name("PATH"))]
    metadata: Option<PathBuf>,
    /// Reads instructions from stdin and appends new ones.
    ///
    /// Example usage:
    ///
    /// `echo "[]" | iroha -io asset definition register --id "66owaQmAQMuHxPzxUN3bqZ6FJfDa" --name "USD" --scale 0`
    #[arg(short, long)]
    input: bool,
    /// Outputs instructions to stdout without submitting them.
    ///
    /// Example usage:
    ///
    /// `iroha -o asset definition register --id "66owaQmAQMuHxPzxUN3bqZ6FJfDa" --name "USD" --scale 0 | iroha transaction stdin`
    #[arg(short, long)]
    output: bool,
    /// Output format for command responses.
    #[arg(long = "output-format", value_enum, default_value_t = CliOutputFormat::Json)]
    output_format: CliOutputFormat,
    /// Language code for messages, overrides system language
    #[arg(long, value_name("LANG"))]
    language: Option<String>,
    /// Enable deterministic machine mode (no startup chatter, strict config loading).
    #[arg(long)]
    machine: bool,
    /// Required signature-bound fee source for transaction submissions.
    #[command(flatten)]
    fee_payment: FeePaymentArgs,
    /// Commands
    #[command(subcommand)]
    command: Command,
}
#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Canonical account reads and account mutations
    #[command(subcommand)]
    Account(account::Command),
    /// Typed transaction status and transaction helpers
    #[command(subcommand, alias = "transaction")]
    Tx(transaction::Command),
    /// Ledger data and transaction helpers
    #[command(subcommand)]
    Ledger(ledger::Command),
    /// Read, write, and execute triggers
    #[command(subcommand)]
    Trigger(crate::trigger::Command),
    /// Node and operator helpers
    #[command(subcommand)]
    Ops(ops::Command),
    /// App API helpers and product tooling
    #[command(subcommand)]
    App(app::Command),
    /// Contract app bundles, deploys, calls, and alias tooling
    #[command(subcommand)]
    Contract(crate::contracts::Command),
    /// Developer utilities and diagnostics
    #[command(subcommand)]
    Tools(tools::Command),
    /// SORA Taira public testnet diagnostics and canaries
    #[command(subcommand)]
    Taira(taira::Command),
    /// Offline encoders, reports, and diagnostics
    #[command(subcommand)]
    Offline(offline::Command),
    /// Soracloud app platform helpers
    #[command(subcommand)]
    Soracloud(crate::soracloud::Command),
}
/// Context inside which commands run
trait RunContext {
    fn config(&self) -> &Config;
    fn transaction_metadata(&self) -> Option<&Metadata>;
    fn transaction_fee_payment(&self) -> Result<FeePaymentIntent> {
        eyre::bail!("this command context has no explicit fee payment selection")
    }
    fn input_instructions(&self) -> bool;
    fn output_instructions(&self) -> bool;
    fn i18n(&self) -> &Localizer;
    fn output_format(&self) -> CliOutputFormat {
        CliOutputFormat::Json
    }
    fn print_data<T>(&mut self, data: &T) -> Result<()>
    where
        T: JsonSerialize + ?Sized;
    fn println_data(&mut self, data: impl Display) -> Result<()> {
        self.println(data)
    }
    fn println(&mut self, data: impl Display) -> Result<()>;
    fn client_from_config(&self) -> Client {
        let mut client = Client::new(self.config().clone());
        if let Some(operator_key_pair) = self.operator_key_pair() {
            client.set_operator_key_pair(operator_key_pair.clone());
        }
        client
    }
    fn operator_key_pair(&self) -> Option<&KeyPair> {
        None
    }
    fn server_version(&self) -> Result<String> {
        self.client_from_config().get_server_version()
    }
    /// Submit instructions or dump them to stdout depending on the flag
    fn finish(&mut self, instructions: impl Into<Executable>) -> Result<()> {
        self.finish_with_mode(instructions, true)
    }
    /// Submit instructions without waiting for confirmation.
    fn finish_unconfirmed(&mut self, instructions: impl Into<Executable>) -> Result<()> {
        self.finish_with_mode(instructions, false)
    }
    fn finish_with_mode(
        &mut self,
        instructions: impl Into<Executable>,
        wait_for_confirmation: bool,
    ) -> Result<()> {
        self.submit_with_mode(instructions, wait_for_confirmation)
    }
    /// Combine instructions into a single transaction and submit it
    ///
    /// # Errors
    ///
    /// Fails if submitting over network fails
    #[allow(dead_code)]
    fn submit(&mut self, instructions: impl Into<Executable>) -> Result<()> {
        self.submit_with_mode(instructions, true)
    }
    /// Submit instructions without waiting for confirmation.
    ///
    /// Useful when the transaction can legitimately restart the node (e.g., executor upgrade)
    /// and break the event stream used for confirmations.
    #[allow(dead_code)]
    fn submit_without_confirmation(&mut self, instructions: impl Into<Executable>) -> Result<()> {
        self.submit_with_mode(instructions, false)
    }
    fn submit_with_mode(
        &mut self,
        instructions: impl Into<Executable>,
        wait_for_confirmation: bool,
    ) -> Result<()> {
        let metadata = self.transaction_metadata().cloned().unwrap_or_default();
        self.submit_with_metadata(instructions, metadata, wait_for_confirmation)
    }
    fn submit_with_metadata(
        &mut self,
        instructions: impl Into<Executable>,
        metadata: Metadata,
        wait_for_confirmation: bool,
    ) -> Result<()> {
        self.submit_with_metadata_and_gas(instructions, metadata, wait_for_confirmation, None)
    }
    fn submit_with_metadata_and_gas(
        &mut self,
        instructions: impl Into<Executable>,
        metadata: Metadata,
        wait_for_confirmation: bool,
        gas_limit: Option<u64>,
    ) -> Result<()> {
        let executable = instructions.into();
        let executable = match executable {
            Executable::ContractCall(invocation) => {
                if self.input_instructions() || self.output_instructions() {
                    eyre::bail!(
                        "Incompatible `--input` `--output` flags with contract-call executables"
                    )
                }
                Executable::ContractCall(invocation)
            }
            Executable::Ivm(bytecode) => {
                if self.input_instructions() || self.output_instructions() {
                    eyre::bail!(
                        "Incompatible `--input` `--output` flags with `iroha transaction ivm`"
                    )
                }
                Executable::Ivm(bytecode)
            }
            Executable::IvmProved(proved) => {
                if self.input_instructions() || self.output_instructions() {
                    eyre::bail!(
                        "Incompatible `--input` `--output` flags with `iroha transaction ivm`"
                    )
                }
                Executable::IvmProved(proved)
            }
            Executable::Batch(items) => {
                if self.input_instructions() || self.output_instructions() {
                    eyre::bail!(
                        "Incompatible `--input` `--output` flags with mixed executable batches"
                    )
                }
                Executable::Batch(items)
            }
            Executable::Instructions(instructions) => {
                let mut out = instructions.into_vec();
                if self.input_instructions() {
                    let mut acc: Vec<InstructionBox> = parse_json_stdin_unchecked()?;
                    acc.append(&mut out);
                    out = acc;
                }
                if self.output_instructions() {
                    dump_json_stdout(&out)?;
                    return Ok(());
                }
                Executable::Instructions(out.into())
            }
        };
        let fee_payment = apply_cli_gas_limit_override(self.transaction_fee_payment()?, gas_limit)?;
        let client = self.client_from_config();
        let (transaction, fee_quote) =
            quote_and_sign_transaction(&client, executable, fee_payment, metadata)?;
        let i18n = self.i18n().clone();
        let err_msg = if cfg!(debug_assertions) {
            let tx = format!("{transaction:?}");
            i18n.t_with(
                "error.submit_transaction_debug",
                &[("transaction", tx.as_str())],
            )
        } else {
            i18n.t("error.submit_transaction")
        };
        let (hash, confirmation_msg) = if wait_for_confirmation {
            let hash = client
                .submit_transaction_blocking(&transaction)
                .map_err(|err| map_account_admission_error(err, &i18n))
                .wrap_err(err_msg.clone())?;
            (hash, i18n.t("info.tx_submitted"))
        } else {
            let hash = transaction.hash();
            client
                .submit_transaction(&transaction)
                .map_err(|err| map_account_admission_error(err, &i18n))
                .wrap_err(err_msg.clone())?;
            (hash, i18n.t("info.tx_submitted_no_confirmation"))
        };
        match self.output_format() {
            CliOutputFormat::Json => {
                let result = json_utils::json_object(vec![
                    ("hash", json_utils::json_value(&hash)?),
                    ("transaction", json_utils::json_value(&transaction)?),
                    ("fee_quote", json_utils::json_value(&fee_quote)?),
                ])?;
                self.print_data(&result)
            }
            CliOutputFormat::Text => {
                print_fee_quote_text(self, &fee_quote)?;
                self.println(confirmation_msg)?;
                self.println(format!("{}: {}", i18n.t("label.hash"), hash))?;
                Ok(())
            }
        }
    }
}
include!("print_json_context.rs");
/// Runs command
trait Run {
    /// Runs command
    ///
    /// # Errors
    /// if inner command errors
    fn run<C: RunContext>(self, context: &mut C) -> Result<()>;
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        use Command::*;
        match self {
            Account(variant) => Run::run(variant, context),
            Tx(variant) => Run::run(variant, context),
            Ledger(variant) => Run::run(variant, context),
            Trigger(variant) => Run::run(variant, context),
            Ops(variant) => Run::run(variant, context),
            App(variant) => Run::run(variant, context),
            Contract(variant) => Run::run(variant, context),
            Tools(variant) => Run::run(variant, context),
            Taira(variant) => Run::run(variant, context),
            Offline(variant) => Run::run(variant, context),
            Soracloud(variant) => Run::run(variant, context),
        }
    }
}
impl Command {
    fn allows_fallback_config(&self) -> bool {
        match self {
            Self::App(command) => command.allows_fallback_config(),
            Self::Tools(command) => command.allows_fallback_config(),
            Self::Account(_)
            | Self::Tx(_)
            | Self::Ledger(_)
            | Self::Trigger(_)
            | Self::Ops(_)
            | Self::Taira(_) => false,
            Self::Contract(command) => command.allows_fallback_config(),
            Self::Offline(command) => command.allows_fallback_config(),
            Self::Soracloud(command) => command.allows_fallback_config(),
        }
    }
    fn allows_fallback_config_in_machine_mode(&self) -> bool {
        match self {
            Self::Offline(command) => command.allows_fallback_config(),
            Self::Contract(command) => command.allows_fallback_config(),
            _ => false,
        }
    }
}
mod ledger {
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Read and write domains
        #[command(subcommand)]
        Domain(crate::domain::Command),
        /// Read and write accounts
        #[command(subcommand)]
        Account(crate::account::Command),
        /// Read and write assets
        #[command(subcommand)]
        Asset(crate::asset::Command),
        /// Read and write NFTs
        #[command(subcommand)]
        Nft(crate::nft::Command),
        /// Read and write RWA lots
        #[command(subcommand)]
        Rwa(crate::rwa::Command),
        /// Read and write peers
        #[command(subcommand)]
        Peer(crate::peer::Command),
        /// Read and write roles
        #[command(subcommand)]
        Role(crate::role::Command),
        /// Read and write system parameters
        #[command(subcommand)]
        Parameter(crate::parameter::Command),
        /// Read and write triggers
        #[command(subcommand)]
        Trigger(crate::trigger::Command),
        /// Read various data
        #[command(subcommand)]
        Query(crate::query::Command),
        /// Read transactions and write various data
        #[command(subcommand)]
        Transaction(crate::transaction::Command),
        /// Read and write multi-signature accounts and transactions
        #[command(subcommand)]
        Multisig(crate::multisig::Command),
        /// Subscribe to events: state changes, transaction/block/trigger progress
        Events(crate::events::Args),
        /// Subscribe to blocks
        Blocks(crate::blocks::Args),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Domain(variant) => Run::run(variant, context),
                Account(variant) => Run::run(variant, context),
                Asset(variant) => Run::run(variant, context),
                Nft(variant) => Run::run(variant, context),
                Rwa(variant) => Run::run(variant, context),
                Peer(variant) => Run::run(variant, context),
                Role(variant) => Run::run(variant, context),
                Parameter(variant) => Run::run(variant, context),
                Trigger(variant) => Run::run(variant, context),
                Query(variant) => Run::run(variant, context),
                Transaction(variant) => Run::run(variant, context),
                Multisig(variant) => Run::run(variant, context),
                Events(variant) => Run::run(variant, context),
                Blocks(variant) => Run::run(variant, context),
            }
        }
    }
}
mod ops {
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Read and write the executor
        #[command(subcommand)]
        Executor(crate::executor::Command),
        /// Runtime ABI/upgrades
        #[command(subcommand)]
        Runtime(crate::runtime::Command),
        /// Sumeragi helpers (status)
        #[command(subcommand)]
        Sumeragi(crate::sumeragi::Command),
        /// Audit helpers (debug endpoints)
        #[command(subcommand)]
        Audit(crate::audit::Command),
        /// Connect diagnostics helpers (queue inspection, evidence export)
        #[command(subcommand)]
        Connect(crate::commands::connect::Command),
        /// Bridge tools (feature: bridge)
        #[cfg(feature = "bridge")]
        #[command(subcommand)]
        Bridge(crate::bridge::Command),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Executor(variant) => Run::run(variant, context),
                Runtime(variant) => Run::run(variant, context),
                Sumeragi(variant) => Run::run(variant, context),
                Audit(variant) => Run::run(variant, context),
                Connect(variant) => Run::run(variant, context),
                #[cfg(feature = "bridge")]
                Bridge(variant) => Run::run(variant, context),
            }
        }
    }
}
mod app {
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Governance helpers (app API convenience)
        #[command(subcommand)]
        Gov(crate::gov::Command),
        /// Zero-knowledge helpers (roots, etc.)
        #[command(subcommand)]
        Zk(crate::zk::Command),
        /// Confidential asset tooling helpers
        #[command(subcommand)]
        Confidential(crate::confidential::Command),
        /// Taikai publisher tooling (CAR bundler, envelopes)
        #[command(subcommand)]
        Taikai(crate::commands::taikai::Command),
        /// Content hosting helpers
        #[command(subcommand)]
        Content(crate::content::Command),
        /// Data availability helpers (ingest tooling)
        #[command(subcommand)]
        Da(crate::commands::da::Command),
        /// Streaming helpers (HPKE fingerprints, suite listings)
        #[command(subcommand)]
        Streaming(crate::commands::streaming::Command),
        /// Nexus helpers (lanes, governance)
        #[command(subcommand)]
        Nexus(crate::nexus::Command),
        /// Public-lane staking helpers (register/activate/exit)
        #[command(subcommand)]
        Staking(crate::staking::Command),
        /// Subscription plan and billing helpers
        #[command(subcommand)]
        Subscriptions(crate::subscriptions::Command),
        /// Domain endorsement helpers (committees, policies, submissions)
        #[command(subcommand)]
        Endorsement(crate::endorsement::Command),
        /// Jurisdiction Data Guardian helpers (attestations and SDN registries)
        #[command(subcommand)]
        Jurisdiction(crate::jurisdiction::Command),
        /// Compute lane simulation helpers
        #[command(subcommand)]
        Compute(crate::compute::Command),
        /// Social incentive helpers (viral follow rewards and escrows)
        #[command(subcommand)]
        Social(crate::commands::social::Command),
        /// Space Directory helpers (UAID capability manifests)
        #[command(subcommand)]
        SpaceDirectory(crate::space_directory::Command),
        /// Kaigi session helpers
        #[command(subcommand)]
        Kaigi(crate::commands::kaigi::Command),
        /// SoraFS helpers (pin registry, aliases, replication orders, storage)
        #[command(subcommand)]
        Sorafs(crate::commands::sorafs::Command),
        /// Soracles helpers (evidence bundling)
        #[command(subcommand)]
        Soracles(crate::commands::soracles::Command),
        /// Sora Name Service helpers (registrar + policy tooling)
        #[command(subcommand)]
        Sns(crate::commands::sns::Command),
        /// Alias resolution and declarative setup helpers
        #[command(subcommand)]
        Alias(crate::commands::alias::Command),
        /// Repo settlement helpers
        #[command(subcommand)]
        Repo(crate::repo::Command),
        /// Delivery-versus-payment and payment-versus-payment helpers
        #[command(subcommand)]
        Settlement(crate::settlement::Command),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Gov(variant) => Run::run(variant, context),
                Zk(variant) => Run::run(variant, context),
                Confidential(variant) => Run::run(variant, context),
                Taikai(variant) => Run::run(variant, context),
                Content(variant) => Run::run(variant, context),
                Da(variant) => Run::run(variant, context),
                Streaming(variant) => Run::run(variant, context),
                Nexus(variant) => Run::run(variant, context),
                Staking(variant) => Run::run(variant, context),
                Subscriptions(variant) => Run::run(variant, context),
                Endorsement(variant) => Run::run(variant, context),
                Jurisdiction(variant) => Run::run(variant, context),
                Compute(variant) => Run::run(variant, context),
                Social(variant) => Run::run(variant, context),
                SpaceDirectory(variant) => Run::run(variant, context),
                Kaigi(variant) => Run::run(variant, context),
                Sorafs(variant) => Run::run(variant, context),
                Soracles(variant) => Run::run(variant, context),
                Sns(variant) => Run::run(variant, context),
                Alias(variant) => Run::run(variant, context),
                Repo(variant) => Run::run(variant, context),
                Settlement(variant) => Run::run(variant, context),
            }
        }
    }
    impl Command {
        pub(super) fn allows_fallback_config(&self) -> bool {
            match self {
                Self::Da(
                    crate::commands::da::Command::RentQuote(_)
                    | crate::commands::da::Command::RentLedger(_),
                ) => true,
                Self::Zk(command) => command.allows_fallback_config(),
                Self::Taikai(command) => taikai_allows_fallback_config(command),
                Self::Sorafs(command) => sorafs_allows_fallback_config(command),
                Self::SpaceDirectory(crate::space_directory::Command::Manifest(
                    crate::space_directory::ManifestCommand::Encode(_)
                    | crate::space_directory::ManifestCommand::AuditBundle(_)
                    | crate::space_directory::ManifestCommand::Scaffold(_),
                )) => true,
                Self::Gov(_)
                | Self::Confidential(_)
                | Self::Content(_)
                | Self::Da(_)
                | Self::Streaming(_)
                | Self::Nexus(_)
                | Self::Staking(_)
                | Self::Subscriptions(_)
                | Self::Endorsement(_)
                | Self::Jurisdiction(_)
                | Self::Compute(_)
                | Self::Social(_)
                | Self::SpaceDirectory(_)
                | Self::Kaigi(_)
                | Self::Soracles(_)
                | Self::Sns(_)
                | Self::Alias(_)
                | Self::Repo(_)
                | Self::Settlement(_) => false,
            }
        }
    }
    fn taikai_allows_fallback_config(command: &crate::commands::taikai::Command) -> bool {
        use crate::commands::taikai::{Command as TaikaiCommand, IngestCommand};
        match command {
            TaikaiCommand::Bundle(_)
            | TaikaiCommand::CekRotate(_)
            | TaikaiCommand::RptAttest(_)
            | TaikaiCommand::Ingest(IngestCommand::Edge(_)) => true,
            TaikaiCommand::Ingest(IngestCommand::Watch(args)) => !args.publish_da,
        }
    }
    fn sorafs_allows_fallback_config(command: &crate::commands::sorafs::Command) -> bool {
        use crate::commands::sorafs::{
            Command as SorafsCommand, IncentivesCommand, IncentivesServiceCommand, ReserveCommand,
        };
        match command {
            SorafsCommand::Reserve(
                ReserveCommand::Quote(_) | ReserveCommand::Ledger(_) | ReserveCommand::Lifecycle(_),
            ) => true,
            SorafsCommand::Incentives(
                IncentivesCommand::Compute(_)
                | IncentivesCommand::OpenDispute(_)
                | IncentivesCommand::Dashboard(_),
            ) => true,
            SorafsCommand::Incentives(IncentivesCommand::Service(command)) => match command {
                IncentivesServiceCommand::Process(args) => !args.submit_transfer,
                IncentivesServiceCommand::Init(_)
                | IncentivesServiceCommand::Record(_)
                | IncentivesServiceCommand::Dispute(_)
                | IncentivesServiceCommand::Dashboard(_)
                | IncentivesServiceCommand::Audit(_)
                | IncentivesServiceCommand::ShadowRun(_)
                | IncentivesServiceCommand::Reconcile(_)
                | IncentivesServiceCommand::Daemon(_) => true,
            },
            SorafsCommand::Pin(_)
            | SorafsCommand::Alias(_)
            | SorafsCommand::Replication(_)
            | SorafsCommand::Storage(_)
            | SorafsCommand::Gateway(_)
            | SorafsCommand::Handshake(_)
            | SorafsCommand::Toolkit(_)
            | SorafsCommand::GuardDirectory(_)
            | SorafsCommand::Appeals(_)
            | SorafsCommand::Gar(_)
            | SorafsCommand::Transparency(_)
            | SorafsCommand::Moderation(_)
            | SorafsCommand::Repair(_)
            | SorafsCommand::Billing(_)
            | SorafsCommand::Hedging(_)
            | SorafsCommand::Gc(_)
            | SorafsCommand::Fetch(_) => false,
        }
    }
}
mod tools {
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Account address helpers (canonical I105 conversions)
        #[command(subcommand)]
        Address(crate::address::Command),
        /// Cryptography helpers (SM2/SM3/SM4)
        #[command(subcommand)]
        Crypto(crate::crypto::Command),
        /// IVM/ABI helpers (e.g., compute ABI hash)
        #[command(subcommand)]
        Ivm(crate::ivm_cli::Command),
        /// Output CLI documentation in Markdown format
        MarkdownHelp(crate::MarkdownHelp),
        /// Show versions and git SHA of client and server
        Version(crate::Version),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Address(variant) => Run::run(variant, context),
                Crypto(variant) => Run::run(variant, context),
                Ivm(variant) => Run::run(variant, context),
                MarkdownHelp(variant) => Run::run(variant, context),
                Version(variant) => Run::run(variant, context),
            }
        }
    }
    impl Command {
        pub(super) fn allows_fallback_config(&self) -> bool {
            matches!(self, Self::Address(_))
        }
    }
}
#[derive(Error, Debug)]
enum MainError {
    #[error("Failed to parse command-line arguments: {0}")]
    CliArgs(String),
    #[error("Failed to load config")]
    Config,
    #[error("Failed to serialize config")]
    SerializeConfig,
    #[error("Failed to get transaction metadata from file")]
    TransactionMetadata,
    #[error("Failed to run the command: {0}")]
    Command(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliErrorKind {
    Config,
    Input,
    Command,
    Internal,
}
impl CliErrorKind {
    fn label(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Input => "input",
            Self::Command => "command",
            Self::Internal => "internal",
        }
    }
    fn exit_code(self) -> i32 {
        match self {
            Self::Config => 3,
            Self::Input => 4,
            Self::Command => 1,
            Self::Internal => 7,
        }
    }
}
#[derive(clap::Args, Debug)]
struct MarkdownHelp;
impl Run for MarkdownHelp {
    fn run<C: RunContext>(self, _context: &mut C) -> Result<()> {
        Ok(())
    }
}
#[derive(clap::Args, Debug)]
struct Version;
impl Run for Version {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client_version = env!("CARGO_PKG_VERSION");
        let response = context.server_version()?;
        match context.output_format() {
            CliOutputFormat::Text => {
                let (client_git_sha, client_version_msg, server_version_msg) = {
                    let i18n = context.i18n();
                    (
                        i18n.t_with("info.client_git_sha", &[("sha", VERGEN_GIT_SHA)]),
                        i18n.t_with("info.client_version", &[("version", client_version)]),
                        i18n.t_with("info.server_version", &[("version", response.as_str())]),
                    )
                };
                context.println(client_git_sha)?;
                context.println(client_version_msg)?;
                context.println(server_version_msg)?;
                Ok(())
            }
            CliOutputFormat::Json => {
                let value = json_utils::json_object(vec![
                    ("client_git_sha", json_utils::json_value(&VERGEN_GIT_SHA)?),
                    ("client_version", json_utils::json_value(&client_version)?),
                    ("server_version", json_utils::json_value(&response)?),
                ])?;
                context.print_data(&value)
            }
        }
    }
}
fn main() {
    let raw_args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let output_format = output_format_override_from_args(
        raw_args
            .iter()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned()),
    )
    .unwrap_or(CliOutputFormat::Json);
    if let Err(report) = run() {
        let rendered = render_cli_error(&report, output_format);
        eprint!("{}", rendered.output);
        std::process::exit(rendered.kind.exit_code());
    }
}
#[allow(clippy::too_many_lines)]
fn run() -> ReportResult<(), MainError> {
    let raw_args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let language_override = language_override_from_args(
        raw_args
            .iter()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned()),
    );
    let help_language = detect_language(language_override.as_deref());
    let help_i18n = Localizer::new(Bundle::Cli, help_language);
    let cmd = Args::command();
    let matches = match cmd.try_get_matches_from(&raw_args) {
        Ok(matches) => matches,
        Err(err) => match err.kind() {
            ErrorKind::DisplayHelp => {
                let rendered = err.render().to_string();
                let localized = localize_help_text(&rendered, &help_i18n);
                print!("{localized}");
                return Ok(());
            }
            ErrorKind::DisplayVersion => {
                print!("{}", err.render());
                return Ok(());
            }
            _ => {
                return Err(Report::new(MainError::CliArgs(err.to_string())));
            }
        },
    };
    let args = Args::from_arg_matches(&matches)
        .map_err(|err| Report::new(MainError::CliArgs(err.to_string())))?;
    let language = detect_language(args.language.as_deref());
    let i18n = Localizer::new(Bundle::Cli, language);
    if !args.machine {
        eprintln!("{}", i18n.t("info.started"));
    }
    if let Command::Tools(tools::Command::MarkdownHelp(_md)) = &args.command {
        clap_markdown::print_help_markdown::<Args>();
        return Ok(());
    }
    error_stack::Report::set_color_mode(color_mode());
    let (load_path, config_was_explicit) = args.config.as_ref().map_or_else(
        || (LoadPath::Default(PathBuf::from("client.toml")), false),
        |path| (LoadPath::Explicit(resolve_config_path(path)), true),
    );
    let config_path = match &load_path {
        LoadPath::Explicit(path) | LoadPath::Default(path) => Some(path.clone()),
    };
    let mut config = match Config::load(load_path) {
        Ok(cfg) => cfg,
        Err(_)
            if !config_was_explicit
                && args.command.allows_fallback_config()
                && (!args.machine || args.command.allows_fallback_config_in_machine_mode()) =>
        {
            try_fallback_config().map_err(|err| {
                Report::new(MainError::Config)
                    .attach("failed to derive offline fallback signing key")
                    .attach(err.to_string())
            })?
        }
        Err(report) => {
            let mut report = report
                .change_context(MainError::Config)
                .attach(i18n.t("error.config_path"));
            if !config_was_explicit {
                report = report.attach(
                    "runtime commands require a readable `client.toml`; use command-specific offline tooling explicitly",
                );
            }
            return Err(report);
        }
    };
    if let Some(path) = config_path
        && let Ok(raw) = read_cli_text_file_bounded(&path, "client configuration")
        && let Ok(value) = toml::from_str::<toml::Value>(&raw)
    {
        apply_transaction_overrides(&mut config, &value);
    }
    if args.verbose {
        let config_json = config_to_json(&config).into_report().map_err(|report| {
            report
                .change_context(MainError::SerializeConfig)
                .attach("caused by `--verbose` argument")
        })?;
        let rendered = norito::json::to_json_pretty(&config_json)
            .change_context(MainError::SerializeConfig)
            .attach("caused by `--verbose` argument")?;
        eprintln!(
            "{}",
            i18n.t_with("info.configuration_dump", &[("config", rendered.as_str())])
        );
    }
    let operator_key_pair = args
        .operator_private_key_file
        .as_deref()
        .map(operator_key::load_operator_key_pair)
        .transpose()
        .map_err(|error| {
            Report::new(MainError::Config)
                .attach("failed to load runtime operator signing key")
                .attach(error.to_string())
        })?;
    let output_format = effective_output_format(&args);
    let mut context = PrintJsonContext {
        write: io::stdout(),
        err_write: io::stderr(),
        config,
        operator_key_pair,
        transaction_metadata: None,
        fee_payment: args.fee_payment,
        input_instructions: args.input,
        output_instructions: args.output,
        output_format,
        i18n: i18n.clone(),
    };
    if let Some(path) = args.metadata {
        let str = read_cli_text_file_bounded(&path, "transaction metadata")
            .into_report()
            .change_context(MainError::TransactionMetadata)
            .attach("failed to read to string")?;
        let metadata: Metadata = parse_json(&str)
            .wrap_err("failed to deserialize metadata from JSON")
            .into_report()
            .map_err(|report| report.change_context(MainError::TransactionMetadata))?;
        context.transaction_metadata = Some(metadata);
    }
    let _account_chain_discriminant =
        ChainDiscriminantGuard::enter(context.config.account_chain_discriminant);
    args.command
        .run(&mut context)
        .into_report()
        .map_err(|report| {
            let message = format!("{:#}", report.current_context());
            report.change_context(MainError::Command(message))
        })?;
    Ok(())
}
const HELP_REPLACEMENTS: &[(&str, &str)] = &[
    ("Usage:", "help.heading.usage"),
    ("Commands:", "help.heading.commands"),
    ("Options:", "help.heading.options"),
    ("Arguments:", "help.heading.arguments"),
    ("Subcommands:", "help.heading.subcommands"),
    ("Flags:", "help.heading.flags"),
    ("Aliases:", "help.heading.aliases"),
    ("Possible values:", "help.label.possible_values"),
    ("Default value:", "help.label.default_value"),
    ("Default:", "help.label.default"),
    ("Environment:", "help.label.env"),
];
fn language_override_from_args<I, S>(args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let arg = arg.as_ref();
        if arg == "--language" {
            if let Some(value) = iter.next() {
                let value = value.as_ref();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        } else if let Some(value) = arg.strip_prefix("--language=")
            && !value.is_empty()
        {
            return Some(value.to_string());
        }
    }
    None
}
fn localize_help_text(help: &str, i18n: &Localizer) -> String {
    let mut localized = help.to_string();
    for (needle, key) in HELP_REPLACEMENTS {
        let replacement = i18n.t(key);
        if replacement != *needle {
            localized = localized.replace(needle, replacement.as_str());
        }
    }
    localized
}
fn output_format_override_from_args<I, S>(args: I) -> Option<CliOutputFormat>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let arg = arg.as_ref();
        if arg == "--output-format" {
            if let Some(value) = iter.next() {
                if let Some(format) = parse_output_format(value.as_ref()) {
                    return Some(format);
                }
            }
        } else if let Some(value) = arg.strip_prefix("--output-format=") {
            if let Some(format) = parse_output_format(value) {
                return Some(format);
            }
        }
    }
    None
}
fn parse_output_format(value: &str) -> Option<CliOutputFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "json" => Some(CliOutputFormat::Json),
        "text" => Some(CliOutputFormat::Text),
        _ => None,
    }
}
fn effective_output_format(args: &Args) -> CliOutputFormat {
    args.output_format
}
fn color_mode() -> ColorMode {
    if supports_color::on(supports_color::Stream::Stdout).is_some()
        && supports_color::on(supports_color::Stream::Stderr).is_some()
    {
        ColorMode::Color
    } else {
        ColorMode::None
    }
}
fn resolve_config_path(path: &Path) -> PathBuf {
    if path.is_absolute() || path.exists() {
        return path.to_path_buf();
    }
    let candidate = WORKSPACE_ROOT.join(path);
    if candidate.exists() {
        return candidate;
    }
    path.to_path_buf()
}
fn parse_duration_value(raw: &toml::Value) -> Option<Duration> {
    match raw {
        toml::Value::Integer(ms) if *ms >= 0 => u64::try_from(*ms).ok().map(Duration::from_millis),
        toml::Value::String(s) => humantime::parse_duration(s).ok(),
        _ => None,
    }
}
fn apply_transaction_overrides(config: &mut Config, raw: &toml::Value) {
    if let Some(transaction) = raw.get("transaction").and_then(|v| v.as_table()) {
        if let Some(ttl) = transaction
            .get("time_to_live_ms")
            .and_then(parse_duration_value)
        {
            config.transaction_ttl = ttl;
        }
        if let Some(status) = transaction
            .get("status_timeout_ms")
            .and_then(parse_duration_value)
        {
            config.transaction_status_timeout = status;
        }
    }
}
fn try_fallback_config() -> Result<Config> {
    let chain = ChainId::from("offline-cli");
    // Offline-only commands still share the complete client configuration type.
    // Use an explicit sentinel that cannot authenticate a request to a deployed
    // network; commands allowed to use this fallback never perform network I/O.
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"iroha:offline-cli:no-network:v1"),
    ));
    let seed = b"iroha-cli-offline-fallback-ed25519-v1".to_vec();
    let key_pair = KeyPair::try_from_seed(seed, Algorithm::Ed25519)
        .wrap_err("failed to derive offline fallback Ed25519 key pair")?;
    let account = AccountId::new(key_pair.public_key().clone());
    let alias_cache = AliasCachePolicy::new(
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS),
    );
    Ok(Config {
        chain,
        network_id,
        account,
        account_chain_discriminant: defaults::common::chain_discriminant(),
        key_pair,
        basic_auth: None,
        torii_api_url: Url::parse("http://127.0.0.1:8080/").expect("fallback url"),
        torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
        transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
        transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
        transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
        connect_queue_root: iroha::config::default_connect_queue_root(),
        soracloud_http_witness_file: None,
        sorafs_alias_cache: alias_cache,
        sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
        sorafs_rollout_phase: SorafsRolloutPhase::Default,
    })
}
#[cfg(test)]
pub(crate) fn fallback_config() -> Config {
    try_fallback_config().expect("offline fallback config should derive a deterministic key pair")
}
static WORKSPACE_ROOT: LazyLock<PathBuf> = LazyLock::new(|| {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(&manifest_dir)
        .to_path_buf()
});
fn config_to_json(config: &Config) -> Result<norito::json::Value> {
    json_utils::json_object(vec![
        ("chain", json_utils::json_value(&config.chain)?),
        ("network_id", json_utils::json_value(&config.network_id)?),
        ("account", json_utils::json_value(&config.account)?),
        (
            "account_chain_discriminant",
            json_utils::json_value(&config.account_chain_discriminant)?,
        ),
        ("key_pair", json_utils::json_value(&config.key_pair)?),
        ("basic_auth", json_utils::json_value(&config.basic_auth)?),
        (
            "torii_api_url",
            json_utils::json_value(&config.torii_api_url)?,
        ),
        (
            "torii_request_timeout",
            json_utils::json_value(&config.torii_request_timeout)?,
        ),
        (
            "transaction_ttl",
            json_utils::json_value(&config.transaction_ttl)?,
        ),
        (
            "transaction_status_timeout",
            json_utils::json_value(&config.transaction_status_timeout)?,
        ),
        (
            "transaction_add_nonce",
            json_utils::json_value(&config.transaction_add_nonce)?,
        ),
        (
            "soracloud_http_witness_file",
            json_utils::json_value(&config.soracloud_http_witness_file)?,
        ),
    ])
}
fn account_admission_hint(err: &(dyn std::error::Error + 'static)) -> Option<String> {
    use iroha::data_model::isi::error::{AccountAdmissionError, AccountAdmissionQuotaScope};
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(cause) = current {
        if let Some(admission) = cause.downcast_ref::<AccountAdmissionError>() {
            return Some(match admission {
                AccountAdmissionError::ImplicitAccountCreationDisabled => format!(
                    "Implicit account creation is disabled; register the destination or pass `--ensure-destination` to add an explicit registration."
                ),
                AccountAdmissionError::InvalidPolicy(invalid) => format!(
                    "Account admission policy is invalid: {}",
                    invalid.reason
                ),
                AccountAdmissionError::DefaultRoleError(default_role_error) => format!(
                    "Default role `{}` could not be assigned during implicit account creation: {}",
                    default_role_error.role, default_role_error.reason
                ),
                AccountAdmissionError::QuotaExceeded(quota) => {
                    let scope = match quota.scope {
                        AccountAdmissionQuotaScope::Transaction => "transaction",
                        AccountAdmissionQuotaScope::Block => "block",
                    };
                    format!(
                        "Implicit account creation quota exceeded ({}/{} {})",
                        quota.created, quota.cap, scope
                    )
                }
                AccountAdmissionError::AlgorithmNotAllowed(algorithm) => format!(
                    "Signing algorithm `{algorithm}` is not permitted for implicit account creation; register the account explicitly or use an allowed key."
                ),
                AccountAdmissionError::GenesisDomainForbidden => {
                    "Implicit account creation in the genesis domain is forbidden; register the destination explicitly."
                        .to_string()
                }
                AccountAdmissionError::FeeUnsatisfied(fee) => format!(
                    "Implicit account creation fee {} {} could not be paid (available {}).",
                    fee.required, fee.asset_definition, fee.available
                ),
                AccountAdmissionError::MinInitialAmountUnsatisfied(minimum) => format!(
                    "First receipt below the minimum for `{}` (required {}, provided {}).",
                    minimum.asset_definition, minimum.required, minimum.provided
                ),
            });
        }
        current = cause.source();
    }
    None
}
fn map_account_admission_error(err: eyre::Report, i18n: &Localizer) -> eyre::Report {
    if let Some(hint) = account_admission_hint(err.as_ref()) {
        eprintln!(
            "{}",
            account_admission_rejected_message(hint.as_str(), i18n)
        );
    }
    err
}
fn account_admission_rejected_message(hint: &str, i18n: &Localizer) -> String {
    i18n.t_with("error.account_admission_rejected", &[("hint", hint)])
}
mod filter {
    use iroha::data_model::query::dsl::CompoundPredicate;
    use super::*;
    use crate::list_support::{CommonArgs, FilterArgs};
    #[derive(clap::Args, Debug)]
    pub struct DomainFilter {
        /// Filtering condition specified as a JSON string
        #[arg(value_parser = parse_json::<CompoundPredicate<Domain>>)]
        predicate: CompoundPredicate<Domain>,
        #[command(flatten)]
        options: CommonArgs,
    }
    impl DomainFilter {
        pub fn into_list_args(self) -> FilterArgs<CompoundPredicate<Domain>> {
            FilterArgs::new(self.predicate, self.options)
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct AccountFilter {
        /// Filtering condition specified as a JSON string
        #[arg(value_parser = parse_json::<CompoundPredicate<Account>>)]
        predicate: CompoundPredicate<Account>,
        #[command(flatten)]
        options: CommonArgs,
    }
    impl AccountFilter {
        pub fn into_list_args(self) -> FilterArgs<CompoundPredicate<Account>> {
            FilterArgs::new(self.predicate, self.options)
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct AssetFilter {
        /// Filtering condition specified as a JSON string
        #[arg(value_parser = parse_json::<CompoundPredicate<Asset>>)]
        predicate: CompoundPredicate<Asset>,
        #[command(flatten)]
        options: CommonArgs,
    }
    impl AssetFilter {
        pub fn into_list_args(self) -> FilterArgs<CompoundPredicate<Asset>> {
            FilterArgs::new(self.predicate, self.options)
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct AssetDefinitionFilter {
        /// Filtering condition specified as a JSON string
        #[arg(value_parser = parse_json::<CompoundPredicate<AssetDefinition>>)]
        predicate: CompoundPredicate<AssetDefinition>,
        #[command(flatten)]
        options: CommonArgs,
    }
    impl AssetDefinitionFilter {
        pub fn into_list_args(self) -> FilterArgs<CompoundPredicate<AssetDefinition>> {
            FilterArgs::new(self.predicate, self.options)
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct NftFilter {
        /// Filtering condition specified as a JSON string
        #[arg(value_parser = parse_json::<CompoundPredicate<Nft>>)]
        predicate: CompoundPredicate<Nft>,
        #[command(flatten)]
        options: CommonArgs,
    }
    impl NftFilter {
        pub fn into_list_args(self) -> FilterArgs<CompoundPredicate<Nft>> {
            FilterArgs::new(self.predicate, self.options)
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct RwaFilter {
        /// Filtering condition specified as a JSON string
        #[arg(value_parser = parse_json::<CompoundPredicate<Rwa>>)]
        predicate: CompoundPredicate<Rwa>,
        #[command(flatten)]
        options: CommonArgs,
    }
    impl RwaFilter {
        pub fn into_list_args(self) -> FilterArgs<CompoundPredicate<Rwa>> {
            FilterArgs::new(self.predicate, self.options)
        }
    }
}
async fn drive_try_stream_until_timeout<S, F>(
    stream: &mut S,
    mut on_item: F,
    timeout: Duration,
    timeout_message: &str,
) -> Result<()>
where
    S: TryStream + Unpin,
    S::Error: std::fmt::Display + Send + Sync + 'static,
    F: FnMut(S::Ok) -> Result<()>,
{
    while let Ok(item) = tokio::time::timeout(timeout, stream.try_next()).await {
        match item.map_err(|err| eyre!("Torii event stream error: {err}"))? {
            Some(value) => on_item(value)?,
            None => break,
        }
    }
    eprintln!("{timeout_message}");
    Ok(())
}
fn listen_events_message(
    filter: &EventFilterBox,
    timeout: Option<Duration>,
    i18n: &Localizer,
) -> String {
    let filter_text = format!("{filter:?}");
    timeout.map_or_else(
        || i18n.t_with("info.listen_events", &[("filter", filter_text.as_str())]),
        |timeout| {
            let timeout_text = format!("{timeout:?}");
            i18n.t_with(
                "info.listen_events_with_timeout",
                &[
                    ("filter", filter_text.as_str()),
                    ("timeout", timeout_text.as_str()),
                ],
            )
        },
    )
}
fn listen_blocks_message(
    height: NonZeroU64,
    timeout: Option<Duration>,
    i18n: &Localizer,
) -> String {
    let height_text = height.to_string();
    timeout.map_or_else(
        || i18n.t_with("info.listen_blocks", &[("height", height_text.as_str())]),
        |timeout| {
            let timeout_text = format!("{timeout:?}");
            i18n.t_with(
                "info.listen_blocks_with_timeout",
                &[
                    ("height", height_text.as_str()),
                    ("timeout", timeout_text.as_str()),
                ],
            )
        },
    )
}
mod events {
    use iroha::data_model::events::pipeline::{BlockEventFilter, TransactionEventFilter};
    use super::*;
    #[derive(clap::Args, Debug)]
    pub struct Args {
        /// Duration to listen for events.
        /// Example: "1y 6M 2w 3d 12h 30m 30s"
        #[arg(short, long, global = true)]
        timeout: Option<humantime::Duration>,
        #[command(subcommand)]
        command: Command,
    }
    #[derive(clap::Subcommand, Debug)]
    enum Command {
        /// Notify when the world state undergoes certain changes
        State,
        /// Notify governance lifecycle events
        Governance(GovernanceArgs),
        /// Notify when a transaction reaches specific stages
        Transaction,
        /// Notify when a block reaches specific stages
        Block,
        /// Notify when a trigger execution is ordered
        TriggerExecute,
        /// Notify when a trigger execution is completed
        TriggerComplete,
    }
    impl Run for Args {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            let timeout: Option<Duration> = self.timeout.map(Into::into);
            match self.command {
                State => listen(DataEventFilter::Any, context, timeout),
                Governance(args) => listen(args.into_filter(), context, timeout),
                Transaction => listen(TransactionEventFilter::default(), context, timeout),
                Block => listen(BlockEventFilter::default(), context, timeout),
                TriggerExecute => listen(ExecuteTriggerEventFilter::new(), context, timeout),
                TriggerComplete => listen(TriggerCompletedEventFilter::new(), context, timeout),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct GovernanceArgs {
        /// Filter by proposal id (hex)
        #[arg(long, value_name = "ID_HEX")]
        proposal_id: Option<String>,
        /// Filter by referendum id
        #[arg(long, value_name = "RID")]
        referendum_id: Option<String>,
    }
    impl GovernanceArgs {
        fn into_filter(self) -> DataEventFilter {
            let mut f = GovernanceEventFilter::new();
            if let Some(h) = self.proposal_id
                && let Ok(bytes) = hex::decode(h.trim_start_matches("0x"))
                && bytes.len() == 32
            {
                let mut id = [0u8; 32];
                id.copy_from_slice(&bytes);
                f = f.for_proposal(id);
            }
            if let Some(rid) = self.referendum_id {
                f = f.for_referendum(rid);
            }
            DataEventFilter::Governance(f)
        }
    }
    fn listen(
        filter: impl Into<EventFilterBox>,
        context: &mut impl RunContext,
        timeout: Option<Duration>,
    ) -> Result<()> {
        let filter = filter.into();
        let client = context.client_from_config();
        let i18n = context.i18n();
        eprintln!("{}", listen_events_message(&filter, timeout, i18n));
        if let Some(timeout) = timeout {
            let timeout_message = i18n.t("warning.timeout_expired");
            let rt = Runtime::new().wrap_err("Failed to create runtime")?;
            rt.block_on(async {
                let mut stream = client
                    .listen_for_events_async([filter])
                    .await
                    .wrap_err("Failed to listen for events")?;
                drive_try_stream_until_timeout(
                    &mut stream,
                    |event| context.print_data(&event),
                    timeout,
                    timeout_message.as_str(),
                )
                .await
            })?;
        } else {
            client
                .listen_for_events([filter])
                .wrap_err("Failed to listen for events")?
                .try_for_each(|event| context.print_data(&event?))?;
        }
        Ok(())
    }
}
mod blocks {
    use std::num::NonZeroU64;
    use super::*;
    #[derive(clap::Args, Debug)]
    pub struct Args {
        /// Block height from which to start streaming blocks
        height: NonZeroU64,
        /// Duration to listen for events.
        /// Example: "1y 6M 2w 3d 12h 30m 30s"
        #[arg(short, long)]
        timeout: Option<humantime::Duration>,
    }
    impl Run for Args {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let Args { height, timeout } = self;
            let timeout: Option<Duration> = timeout.map(Into::into);
            listen(height, context, timeout)
        }
    }
    fn listen(
        height: NonZeroU64,
        context: &mut impl RunContext,
        timeout: Option<Duration>,
    ) -> Result<()> {
        let client = context.client_from_config();
        let i18n = context.i18n();
        eprintln!("{}", listen_blocks_message(height, timeout, i18n));
        if let Some(timeout) = timeout {
            let timeout_message = i18n.t("warning.timeout_expired");
            let rt = Runtime::new().wrap_err("Failed to create runtime")?;
            rt.block_on(async {
                let mut stream = client
                    .listen_for_blocks_async(height)
                    .await
                    .wrap_err("Failed to listen for blocks")?;
                drive_try_stream_until_timeout(
                    &mut stream,
                    |event| context.print_data(&event),
                    timeout,
                    timeout_message.as_str(),
                )
                .await
            })?;
        } else {
            client
                .listen_for_blocks(height)
                .wrap_err("Failed to listen for blocks")?
                .try_for_each(|event| context.print_data(&event?))?;
        }
        Ok(())
    }
}
mod domain {
    use super::*;
    #[allow(clippy::large_enum_variant)]
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List domains
        #[command(subcommand)]
        List(List),
        /// Retrieve details of a specific domain
        Get(Id),
        /// Unregister a domain
        Unregister(Id),
        /// Transfer ownership of a domain
        Transfer(Transfer),
        /// Read and write metadata
        #[command(subcommand)]
        Meta(metadata::domain::Command),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                List(cmd) => cmd.run(context),
                Get(args) => {
                    let client = context.client_from_config();
                    let entries = client
                        .query(FindDomains)
                        .execute_all()
                        .wrap_err("Failed to get domain")?;
                    let entry = entries
                        .into_iter()
                        .find(|e| e.id() == &args.id)
                        .ok_or_else(|| eyre!("Domain not found"))?;
                    context.print_data(&entry)
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::domain(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister domain")
                }
                Transfer(args) => {
                    let from = resolve_account_id(context, &args.from)
                        .wrap_err("failed to resolve --from account")?;
                    let to = resolve_account_id(context, &args.to)
                        .wrap_err("failed to resolve --to account")?;
                    let instruction = iroha::data_model::isi::Transfer::domain(from, args.id, to);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to transfer domain")
                }
                Meta(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Transfer {
        /// Domain name
        #[arg(short, long, value_parser = parse_domain_id_literal)]
        pub id: DomainId,
        /// Source account identifier (canonical I105 literal)
        #[arg(short, long)]
        pub from: String,
        /// Destination account identifier (canonical I105 literal)
        #[arg(short, long)]
        pub to: String,
    }
    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Domain name
        #[arg(short, long, value_parser = parse_domain_id_literal)]
        pub id: DomainId,
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all IDs, or full entries when `--verbose` is specified
        All(crate::list_support::AllArgs),
        /// Filter by a given predicate
        Filter(filter::DomainFilter),
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            match self {
                List::All(args) => list_all(context, client.query(FindDomains), &args),
                List::Filter(filter) => {
                    let (predicate, common) = filter.into_list_args().decompose();
                    let builder = client.query(FindDomains).filter(predicate);
                    let builder = apply_common_args(builder, &common)?;
                    let entries = builder.execute_all()?;
                    context.print_data(&entries)
                }
            }
        }
    }
    fn list_all<C: RunContext>(
        context: &mut C,
        builder: iroha::data_model::query::builder::QueryBuilder<'_, Client, FindDomains, Domain>,
        args: &crate::list_support::AllArgs,
    ) -> Result<()> {
        let builder = apply_common_args(builder, &args.common)?;
        let entries = builder.execute_all()?;
        if args.verbose {
            context.print_data(&entries)
        } else {
            let ids: Vec<_> = entries.into_iter().map(|e| e.id().clone()).collect();
            context.print_data(&ids)
        }
    }
    fn apply_common_args<'a>(
        builder: iroha::data_model::query::builder::QueryBuilder<'a, Client, FindDomains, Domain>,
        common: &'a crate::list_support::CommonArgs,
    ) -> Result<iroha::data_model::query::builder::QueryBuilder<'a, Client, FindDomains, Domain>>
    {
        use iroha::data_model::query::parameters::{FetchSize, Pagination, Sorting};
        use std::num::NonZeroU64;
        let mut builder = builder;
        if let Some(key) = common.sort_by_metadata_key.clone() {
            let sorting = Sorting::new(Some(key), common.order.map(Into::into));
            builder = builder.with_sorting(sorting);
        }
        if common.limit.is_some() || common.offset > 0 {
            let pagination = Pagination::new(common.limit.and_then(NonZeroU64::new), common.offset);
            builder = builder.with_pagination(pagination);
        }
        if let Some(n) = common.fetch_size.and_then(NonZeroU64::new) {
            let fs = FetchSize::new(Some(n));
            builder = builder.with_fetch_size(fs);
        }
        if let Some(sel) = common.select.as_deref() {
            let tuple = crate::list_support::parse_selector_tuple::<Domain>(sel)?;
            builder = builder.with_selector_tuple(tuple);
        }
        Ok(builder)
    }
}
mod account {
    use std::fmt::Debug;
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Read and write account roles
        #[command(subcommand)]
        Role(RoleCommand),
        /// Read and write account permissions
        #[command(subcommand)]
        Permission(PermissionCommand),
        /// List accounts
        #[command(subcommand)]
        List(List),
        /// Retrieve details of a specific account
        Get(Id),
        /// Register an account
        Register(RegisterId),
        /// Unregister an account
        Unregister(Id),
        /// Read and write metadata
        #[command(subcommand)]
        Meta(metadata::account::Command),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Role(cmd) => cmd.run(context),
                Permission(cmd) => cmd.run(context),
                List(cmd) => cmd.run(context),
                Get(args) => {
                    let account_id = resolve_account_id(context, &args.id)
                        .wrap_err("failed to resolve --id account")?;
                    let client = context.client_from_config();
                    let entry = client
                        .get_account_read(&account_id)
                        .wrap_err("Failed to get account")?;
                    context.print_data(&entry)
                }
                Register(args) => {
                    let account_id = parse_register_account_id(&args.id)?;
                    let instruction =
                        iroha::data_model::isi::Register::account(Account::new(account_id));
                    let submit = if args.no_wait {
                        context.finish_unconfirmed([instruction])
                    } else {
                        context.finish([instruction])
                    };
                    submit.wrap_err("Failed to register account")
                }
                Unregister(args) => {
                    let account_id = resolve_account_id(context, &args.id)
                        .wrap_err("failed to resolve --id account")?;
                    let instruction = iroha::data_model::isi::Unregister::account(account_id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister account")
                }
                Meta(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum RoleCommand {
        /// List account role IDs
        List(RoleList),
        /// Grant a role to an account
        Grant(IdRole),
        /// Revoke a role from an account
        Revoke(IdRole),
    }
    impl Run for RoleCommand {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::RoleCommand::*;
            match self {
                List(args) => {
                    let account_id = resolve_account_id(context, &args.id)
                        .wrap_err("failed to resolve --id account")?;
                    let client = context.client_from_config();
                    let mut builder = client.query(FindRolesByAccountId::new(account_id));
                    if args.limit.is_some() || args.offset > 0 {
                        let pagination = iroha::data_model::query::parameters::Pagination::new(
                            args.limit.and_then(NonZeroU64::new),
                            args.offset,
                        );
                        builder = builder.with_pagination(pagination);
                    }
                    if let Some(n) = args.fetch_size.and_then(NonZeroU64::new) {
                        let fs = iroha::data_model::query::parameters::FetchSize::new(Some(n));
                        builder = builder.with_fetch_size(fs);
                    }
                    let roles = builder.execute_all()?;
                    context.print_data(&roles)
                }
                Grant(args) => {
                    let account_id = resolve_account_id(context, &args.id)
                        .wrap_err("failed to resolve --id account")?;
                    let instruction =
                        iroha::data_model::isi::Grant::account_role(args.role, account_id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to grant the role to the account")
                }
                Revoke(args) => {
                    let account_id = resolve_account_id(context, &args.id)
                        .wrap_err("failed to resolve --id account")?;
                    let instruction =
                        iroha::data_model::isi::Revoke::account_role(args.role, account_id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to revoke the role from the account")
                }
            }
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum PermissionCommand {
        /// List account permissions
        List(PermissionList),
        /// Grant an account permission using JSON input from stdin
        Grant(PermissionId),
        /// Revoke an account permission using JSON input from stdin
        Revoke(PermissionId),
    }
    impl Run for PermissionCommand {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::PermissionCommand::*;
            match self {
                List(args) => {
                    let account_id = resolve_account_id(context, &args.id)
                        .wrap_err("failed to resolve --id account")?;
                    let client = context.client_from_config();
                    let mut builder = client.query(FindPermissionsByAccountId::new(account_id));
                    if args.limit.is_some() || args.offset > 0 {
                        let pagination = iroha::data_model::query::parameters::Pagination::new(
                            args.limit.and_then(NonZeroU64::new),
                            args.offset,
                        );
                        builder = builder.with_pagination(pagination);
                    }
                    if let Some(n) = args.fetch_size.and_then(NonZeroU64::new) {
                        let fs = iroha::data_model::query::parameters::FetchSize::new(Some(n));
                        builder = builder.with_fetch_size(fs);
                    }
                    let permissions = builder.execute_all()?;
                    context.print_data(&permissions)
                }
                Grant(args) => {
                    let permission: Permission = parse_json_stdin(context)?;
                    let account_id = resolve_account_id(context, &args.id)
                        .wrap_err("failed to resolve --id account")?;
                    let instruction =
                        iroha::data_model::isi::Grant::account_permission(permission, account_id);
                    let submit = if args.no_wait {
                        context.finish_unconfirmed([instruction])
                    } else {
                        context.finish([instruction])
                    };
                    submit.wrap_err("Failed to grant the permission to the account")
                }
                Revoke(args) => {
                    let permission: Permission = parse_json_stdin(context)?;
                    let account_id = resolve_account_id(context, &args.id)
                        .wrap_err("failed to resolve --id account")?;
                    let instruction =
                        iroha::data_model::isi::Revoke::account_permission(permission, account_id);
                    let submit = if args.no_wait {
                        context.finish_unconfirmed([instruction])
                    } else {
                        context.finish([instruction])
                    };
                    submit.wrap_err("Failed to revoke the permission from the account")
                }
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Account identifier (canonical I105 literal)
        #[arg(short, long)]
        id: String,
    }
    #[derive(clap::Args, Debug)]
    pub struct PermissionId {
        /// Account identifier (canonical I105 literal)
        #[arg(short, long)]
        id: String,
        /// Submit without waiting for confirmation
        #[arg(long)]
        no_wait: bool,
    }
    #[derive(clap::Args, Debug)]
    pub struct RegisterId {
        /// Canonical global account identifier for registration (canonical I105 literal)
        #[arg(short, long)]
        id: String,
        /// Submit without waiting for confirmation.
        #[arg(long)]
        no_wait: bool,
    }
    #[derive(clap::Args, Debug)]
    pub struct RoleList {
        /// Account identifier (canonical I105 literal)
        #[arg(short, long)]
        id: String,
        /// Maximum number of items to return (server-side limit)
        #[arg(long)]
        limit: Option<u64>,
        /// Offset into the result set (server-side offset)
        #[arg(long, default_value_t = 0)]
        offset: u64,
        /// Batch fetch size for iterable queries
        #[arg(long)]
        fetch_size: Option<u64>,
    }
    #[derive(clap::Args, Debug)]
    pub struct PermissionList {
        /// Account identifier (canonical I105 literal)
        #[arg(short, long)]
        id: String,
        /// Maximum number of items to return (server-side limit)
        #[arg(long)]
        limit: Option<u64>,
        /// Offset into the result set (server-side offset)
        #[arg(long, default_value_t = 0)]
        offset: u64,
        /// Batch fetch size for iterable queries
        #[arg(long)]
        fetch_size: Option<u64>,
    }
    #[derive(clap::Args, Debug)]
    pub struct IdRole {
        /// Account identifier (canonical I105 literal)
        #[arg(short, long)]
        pub id: String,
        /// Role name
        #[arg(short, long)]
        pub role: RoleId,
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all IDs, or full entries when `--verbose` is specified
        All(crate::list_support::AllArgs),
        /// Filter by a given predicate
        Filter(filter::AccountFilter),
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            match self {
                List::All(args) => list_all(context, &client, &args),
                List::Filter(filter) => {
                    let (predicate, common) = filter.into_list_args().decompose();
                    let builder = client.query(FindAccounts).filter(predicate);
                    let builder = apply_common_args(builder, &common)?;
                    let entries = builder.execute_all()?;
                    context.print_data(&entries)
                }
            }
        }
    }
    fn list_all<C: RunContext>(
        context: &mut C,
        client: &Client,
        args: &crate::list_support::AllArgs,
    ) -> Result<()> {
        if args.verbose
            || args.common.select.is_some()
            || args.common.sort_by_metadata_key.is_some()
        {
            let builder = apply_common_args(client.query(FindAccounts), &args.common)?;
            let entries = builder.execute_all()?;
            if args.verbose {
                context.print_data(&entries)
            } else {
                let ids: Vec<_> = entries.into_iter().map(|e| e.id().clone()).collect();
                context.print_data(&ids)
            }
        } else {
            let builder = apply_common_id_args(client.query(FindAccountIds), &args.common)?;
            let ids = builder.execute_all()?;
            context.print_data(&ids)
        }
    }
    fn apply_common_args<'a>(
        builder: iroha::data_model::query::builder::QueryBuilder<'a, Client, FindAccounts, Account>,
        common: &'a crate::list_support::CommonArgs,
    ) -> Result<iroha::data_model::query::builder::QueryBuilder<'a, Client, FindAccounts, Account>>
    {
        use iroha::data_model::query::parameters::{FetchSize, Pagination, Sorting};
        use std::num::NonZeroU64;
        let mut builder = builder;
        if let Some(key) = common.sort_by_metadata_key.clone() {
            let sorting = Sorting::new(Some(key), common.order.map(Into::into));
            builder = builder.with_sorting(sorting);
        }
        if common.limit.is_some() || common.offset > 0 {
            let pagination = Pagination::new(common.limit.and_then(NonZeroU64::new), common.offset);
            builder = builder.with_pagination(pagination);
        }
        if let Some(n) = common.fetch_size.and_then(NonZeroU64::new) {
            let fs = FetchSize::new(Some(n));
            builder = builder.with_fetch_size(fs);
        }
        if let Some(sel) = common.select.as_deref() {
            let tuple = crate::list_support::parse_selector_tuple::<Account>(sel)?;
            builder = builder.with_selector_tuple(tuple);
        }
        Ok(builder)
    }
    fn apply_common_id_args<'a>(
        builder: iroha::data_model::query::builder::QueryBuilder<
            'a,
            Client,
            FindAccountIds,
            AccountId,
        >,
        common: &'a crate::list_support::CommonArgs,
    ) -> Result<
        iroha::data_model::query::builder::QueryBuilder<'a, Client, FindAccountIds, AccountId>,
    > {
        use iroha::data_model::query::parameters::{FetchSize, Pagination};
        use std::num::NonZeroU64;
        let mut builder = builder;
        if common.limit.is_some() || common.offset > 0 {
            let pagination = Pagination::new(common.limit.and_then(NonZeroU64::new), common.offset);
            builder = builder.with_pagination(pagination);
        }
        if let Some(n) = common.fetch_size.and_then(NonZeroU64::new) {
            let fs = FetchSize::new(Some(n));
            builder = builder.with_fetch_size(fs);
        }
        Ok(builder)
    }
}
mod asset {
    use super::*;
    use iroha::data_model::account::AccountAdmissionMode;
    fn admission_policy(
        client: &Client,
    ) -> Result<iroha::data_model::account::AccountAdmissionPolicy> {
        use iroha::data_model::{
            account::AccountAdmissionPolicy, parameter::Parameters, prelude::FindParameters,
        };
        let params: Parameters = client.query_single(FindParameters)?;
        params
            .custom()
            .get(&AccountAdmissionPolicy::parameter_id())
            .map_or_else(
                || Ok(AccountAdmissionPolicy::default()),
                |custom| {
                    custom
                        .payload()
                        .try_into_any_norito::<AccountAdmissionPolicy>()
                        .wrap_err("failed to decode default account admission policy")
                },
            )
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Read and write asset definitions
        #[command(subcommand)]
        Definition(definition::Command),
        /// Retrieve details of a specific asset
        Get(Id),
        /// List assets
        #[command(subcommand)]
        List(List),
        /// Increase the quantity of an asset
        Mint(IdQuantity),
        /// Decrease the quantity of an asset
        Burn(IdQuantity),
        /// Transfer an asset between accounts
        Transfer(Transfer),
    }
    fn asset_transfer_instructions(
        id: AssetId,
        args: &Transfer,
        to: &AccountId,
        policy: Option<&AccountAdmissionPolicy>,
    ) -> Result<Vec<InstructionBox>> {
        let mut instructions: Vec<InstructionBox> = Vec::new();
        if args.ensure_destination
            && matches!(
                policy.map(|p| p.mode),
                Some(AccountAdmissionMode::ExplicitOnly)
            )
        {
            eyre::bail!(
                "`--ensure-destination` no longer infers a registration domain; register the destination account explicitly before transferring when implicit account creation is disabled"
            );
        }
        instructions.push(InstructionBox::from(
            iroha::data_model::isi::Transfer::asset_quantity(id, args.quantity.clone(), to.clone()),
        ));
        Ok(instructions)
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Definition(cmd) => cmd.run(context),
                Get(args) => {
                    let id = args
                        .resolve_asset_id(context)
                        .wrap_err("failed to resolve asset identifier")?;
                    let client = context.client_from_config();
                    let entries = client
                        .query(FindAssets)
                        .execute_all()
                        .wrap_err("Failed to get asset")?;
                    let entry = entries
                        .into_iter()
                        .find(|e| e.id() == &id)
                        .ok_or_else(|| eyre!("Asset not found"))?;
                    context.print_data(&entry)
                }
                List(cmd) => cmd.run(context),
                Mint(args) => {
                    let id = args
                        .resolve_asset_id(context)
                        .wrap_err("failed to resolve asset identifier")?;
                    let instruction =
                        iroha::data_model::isi::Mint::asset_quantity(args.quantity, id);
                    let submit = if args.no_wait {
                        context.finish_unconfirmed([instruction])
                    } else {
                        context.finish([instruction])
                    };
                    submit.wrap_err("Failed to mint asset quantity")
                }
                Burn(args) => {
                    let id = args
                        .resolve_asset_id(context)
                        .wrap_err("failed to resolve asset identifier")?;
                    let instruction =
                        iroha::data_model::isi::Burn::asset_quantity(args.quantity, id);
                    let submit = if args.no_wait {
                        context.finish_unconfirmed([instruction])
                    } else {
                        context.finish([instruction])
                    };
                    submit.wrap_err("Failed to burn asset quantity")
                }
                Transfer(args) => {
                    let id = args
                        .resolve_asset_id(context)
                        .wrap_err("failed to resolve asset identifier")?;
                    let to = resolve_account_id(context, &args.to)
                        .wrap_err("failed to resolve --to account")?;
                    let policy = if args.ensure_destination {
                        let client = context.client_from_config();
                        Some(admission_policy(&client)?)
                    } else {
                        None
                    };
                    let instructions =
                        asset_transfer_instructions(id, &args, &to, policy.as_ref())?;
                    let submit = if args.no_wait {
                        context.finish_unconfirmed(instructions)
                    } else {
                        context.finish(instructions)
                    };
                    submit.wrap_err("Failed to transfer numeric asset")
                }
            }
        }
    }
    fn resolve_asset_id_components<C: RunContext>(
        context: &C,
        definition: Option<AssetDefinitionId>,
        definition_alias: Option<AssetDefinitionAlias>,
        account: Option<String>,
        scope: Option<iroha::data_model::asset::AssetBalanceScope>,
    ) -> Result<AssetId> {
        let account =
            account.ok_or_else(|| eyre!("`--account` must be provided with asset selectors"))?;
        let account =
            resolve_account_id(context, &account).wrap_err("failed to resolve --account")?;
        let definition = match (definition, definition_alias) {
            (Some(definition), None) => definition,
            (None, Some(alias)) => {
                let client = context.client_from_config();
                resolve_asset_definition_id_by_alias(&client, &alias)?
            }
            _ => {
                eyre::bail!(
                    "provide either `--definition <base58-asset-definition-id>` or `--definition-alias <name>#<domain>.<dataspace>|<name>#<dataspace>`"
                )
            }
        };
        Ok(AssetId::with_scope(
            definition,
            account,
            scope.unwrap_or(iroha::data_model::asset::AssetBalanceScope::Global),
        ))
    }
    mod definition {
        use iroha::{
            data_model::asset::{AssetDefinition, AssetDefinitionAlias, AssetDefinitionId},
            data_model::sorafs_uri::SorafsUri,
        };
        use iroha_primitives::numeric::MAX_DECIMAL_SCALE;
        use super::*;
        fn numeric_spec_from_scale(scale: Option<u32>) -> Result<NumericSpec> {
            scale.map_or_else(
                || Ok(NumericSpec::unconstrained()),
                |scale| {
                    NumericSpec::try_fractional(scale).wrap_err_with(|| {
                        format!(
                            "invalid --scale {scale}: numeric scale must be between 0 and {MAX_DECIMAL_SCALE}"
                        )
                    })
                },
            )
        }
        #[derive(clap::Subcommand, Debug)]
        pub enum Command {
            /// List asset definitions
            #[command(subcommand)]
            List(List),
            /// Retrieve details of a specific asset definition
            Get(Id),
            /// Register an asset definition
            Register(Register),
            /// Unregister an asset definition
            Unregister(Id),
            /// Transfer ownership of an asset definition
            Transfer(Transfer),
            /// Read and write metadata
            #[command(subcommand)]
            Meta(metadata::asset_definition::Command),
        }
        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    List(cmd) => cmd.run(context),
                    Get(args) => {
                        let id = args
                            .resolve_id(context)
                            .wrap_err("failed to resolve asset definition identifier")?;
                        let client = context.client_from_config();
                        let entries = client
                            .query(FindAssetsDefinitions)
                            .execute_all()
                            .wrap_err("Failed to get asset definition")?;
                        let entry = entries
                            .into_iter()
                            .find(|e| e.id() == &id)
                            .ok_or_else(|| eyre!("Asset definition not found"))?;
                        context.print_data(&entry)
                    }
                    Register(args) => {
                        let alias = register_alias_from_args(&args)?;
                        let spec = numeric_spec_from_scale(args.scale)?;
                        let mut entry = AssetDefinition::new(
                            args.id,
                            args.name,
                            spec,
                            iroha_data_model::asset::AssetBalancePolicy::Global,
                            None,
                        );
                        if let Some(description) = args.description {
                            entry = entry.with_description(Some(description));
                        }
                        if let Some(alias) = alias {
                            entry = entry.with_alias(Some(alias));
                        }
                        if let Some(logo) = args.logo {
                            entry = entry.with_logo(Some(logo));
                        }
                        if args.mint_once {
                            entry = entry.mintable_once();
                        }
                        let instruction = iroha::data_model::isi::Register::asset_definition(entry);
                        context
                            .finish([instruction])
                            .wrap_err("Failed to register asset")
                    }
                    Unregister(args) => {
                        let id = args
                            .resolve_id(context)
                            .wrap_err("failed to resolve asset definition identifier")?;
                        let instruction = iroha::data_model::isi::Unregister::asset_definition(id);
                        context
                            .finish([instruction])
                            .wrap_err("Failed to unregister asset")
                    }
                    Transfer(args) => {
                        let id = args
                            .resolve_id(context)
                            .wrap_err("failed to resolve asset definition identifier")?;
                        let from = resolve_account_id(context, &args.from)
                            .wrap_err("failed to resolve --from account")?;
                        let to = resolve_account_id(context, &args.to)
                            .wrap_err("failed to resolve --to account")?;
                        let instruction =
                            iroha::data_model::isi::Transfer::asset_definition(from, id, to);
                        context
                            .finish([instruction])
                            .wrap_err("Failed to transfer asset definition")
                    }
                    Meta(cmd) => cmd.run(context),
                }
            }
        }
        #[derive(clap::Args, Debug)]
        pub struct Register {
            /// Asset definition identifier (unprefixed Base58 address)
            #[arg(short, long, value_parser = parse_asset_definition_literal)]
            pub id: AssetDefinitionId,
            /// Human-readable asset name.
            #[arg(long)]
            pub name: String,
            /// Optional human-readable description.
            #[arg(long)]
            pub description: Option<String>,
            /// Optional explicit alias literal (`<name>#<domain>.<dataspace>` or
            /// `<name>#<dataspace>`).
            #[arg(long, conflicts_with_all = ["alias_domain", "alias_dataspace"])]
            pub alias: Option<AssetDefinitionAlias>,
            /// Optional alias owner/domain segment used to build `<name>#<domain>.<dataspace>`.
            #[arg(long, requires = "alias_dataspace", conflicts_with = "alias")]
            pub alias_domain: Option<Name>,
            /// Optional alias dataspace segment used to build `<name>#<domain>.<dataspace>` or
            /// `<name>#<dataspace>`.
            #[arg(long, conflicts_with = "alias")]
            pub alias_dataspace: Option<Name>,
            /// Optional logo URI. Must use `sorafs://...`.
            #[arg(long)]
            pub logo: Option<SorafsUri>,
            /// Disables minting after the first instance
            #[arg(short, long)]
            pub mint_once: bool,
            /// Numeric scale of the asset. No value means unconstrained.
            #[arg(short, long)]
            pub scale: Option<u32>,
        }
        #[derive(clap::Args, Debug)]
        pub struct Transfer {
            /// Asset definition identifier (unprefixed Base58 address).
            #[arg(short, long, value_parser = parse_asset_definition_literal, required_unless_present = "alias", conflicts_with = "alias")]
            pub id: Option<AssetDefinitionId>,
            /// Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`).
            #[arg(long, required_unless_present = "id", conflicts_with = "id")]
            pub alias: Option<AssetDefinitionAlias>,
            /// Source account identifier (canonical I105 literal)
            #[arg(short, long)]
            pub from: String,
            /// Destination account identifier (canonical I105 literal)
            #[arg(short, long)]
            pub to: String,
        }
        #[derive(clap::Args, Debug)]
        pub struct Id {
            /// Asset definition identifier (unprefixed Base58 address).
            #[arg(short, long, value_parser = parse_asset_definition_literal, required_unless_present = "alias", conflicts_with = "alias")]
            pub id: Option<AssetDefinitionId>,
            /// Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`).
            #[arg(long, required_unless_present = "id", conflicts_with = "id")]
            pub alias: Option<AssetDefinitionAlias>,
        }
        #[derive(clap::Subcommand, Debug)]
        pub enum List {
            /// List all IDs, or full entries when `--verbose` is specified
            All(crate::list_support::AllArgs),
            /// Filter by a given predicate
            Filter(filter::AssetDefinitionFilter),
        }
        impl Run for List {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                let client = context.client_from_config();
                match self {
                    List::All(args) => {
                        list_all(context, client.query(FindAssetsDefinitions), &args)
                    }
                    List::Filter(filter) => {
                        let (predicate, common) = filter.into_list_args().decompose();
                        let builder = client.query(FindAssetsDefinitions).filter(predicate);
                        let builder = apply_common_args(builder, &common)?;
                        let entries = builder.execute_all()?;
                        context.print_data(&entries)
                    }
                }
            }
        }
        fn list_all<C: RunContext>(
            context: &mut C,
            builder: iroha::data_model::query::builder::QueryBuilder<
                '_,
                Client,
                FindAssetsDefinitions,
                AssetDefinition,
            >,
            args: &crate::list_support::AllArgs,
        ) -> Result<()> {
            let builder = apply_common_args(builder, &args.common)?;
            let entries = builder.execute_all()?;
            if args.verbose {
                context.print_data(&entries)
            } else {
                let ids: Vec<_> = entries.into_iter().map(|e| e.id().clone()).collect();
                context.print_data(&ids)
            }
        }
        fn apply_common_args<'a>(
            builder: iroha::data_model::query::builder::QueryBuilder<
                'a,
                Client,
                FindAssetsDefinitions,
                AssetDefinition,
            >,
            common: &'a crate::list_support::CommonArgs,
        ) -> Result<
            iroha::data_model::query::builder::QueryBuilder<
                'a,
                Client,
                FindAssetsDefinitions,
                AssetDefinition,
            >,
        > {
            use iroha::data_model::query::parameters::{FetchSize, Pagination, Sorting};
            use std::num::NonZeroU64;
            let mut builder = builder;
            if let Some(key) = common.sort_by_metadata_key.clone() {
                let sorting = Sorting::new(Some(key), common.order.map(Into::into));
                builder = builder.with_sorting(sorting);
            }
            if common.limit.is_some() || common.offset > 0 {
                let pagination =
                    Pagination::new(common.limit.and_then(NonZeroU64::new), common.offset);
                builder = builder.with_pagination(pagination);
            }
            if let Some(n) = common.fetch_size.and_then(NonZeroU64::new) {
                let fs = FetchSize::new(Some(n));
                builder = builder.with_fetch_size(fs);
            }
            if let Some(sel) = common.select.as_deref() {
                let tuple = crate::list_support::parse_selector_tuple::<AssetDefinition>(sel)?;
                builder = builder.with_selector_tuple(tuple);
            }
            Ok(builder)
        }
        fn register_alias_from_args(args: &Register) -> Result<Option<AssetDefinitionAlias>> {
            match (&args.alias, &args.alias_domain, &args.alias_dataspace) {
                (Some(alias), None, None) => Ok(Some(alias.clone())),
                (None, Some(domain), Some(dataspace)) => AssetDefinitionAlias::from_components(
                    &args.name,
                    Some(domain.as_ref()),
                    dataspace.as_ref(),
                )
                .map(Some)
                .map_err(|err| eyre!("invalid derived alias: {err}")),
                (None, None, Some(dataspace)) => {
                    AssetDefinitionAlias::from_components(&args.name, None, dataspace.as_ref())
                        .map(Some)
                        .map_err(|err| eyre!("invalid derived alias: {err}"))
                }
                (None, None, None) => Ok(None),
                _ => eyre::bail!(
                    "provide either `--alias`, `--alias-dataspace`, or both `--alias-domain` and `--alias-dataspace`"
                ),
            }
        }
        fn resolve_definition_id<C: RunContext>(
            context: &C,
            id: Option<AssetDefinitionId>,
            alias: Option<AssetDefinitionAlias>,
        ) -> Result<AssetDefinitionId> {
            match (id, alias) {
                (Some(id), None) => Ok(id),
                (None, Some(alias)) => {
                    let client = context.client_from_config();
                    resolve_asset_definition_id_by_alias(&client, &alias)
                }
                _ => eyre::bail!("provide either `--id` or `--alias`"),
            }
        }
        impl Id {
            fn resolve_id<C: RunContext>(&self, context: &C) -> Result<AssetDefinitionId> {
                resolve_definition_id(context, self.id.clone(), self.alias.clone())
            }
        }
        impl Transfer {
            fn resolve_id<C: RunContext>(&self, context: &C) -> Result<AssetDefinitionId> {
                resolve_definition_id(context, self.id.clone(), self.alias.clone())
            }
        }
        #[cfg(test)]
        mod tests {
            use super::*;
            fn base_register_args() -> Register {
                Register {
                    id: AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").expect("domain"),
                        "rose".parse().expect("asset name"),
                    ),
                    name: "Rose".to_owned(),
                    description: None,
                    alias: None,
                    alias_domain: None,
                    alias_dataspace: None,
                    logo: None,
                    mint_once: false,
                    scale: None,
                }
            }
            #[test]
            fn numeric_spec_from_scale_validates_runtime_scale() {
                assert_eq!(
                    numeric_spec_from_scale(None)
                        .expect("unconstrained spec")
                        .scale(),
                    None
                );
                assert_eq!(
                    numeric_spec_from_scale(Some(0))
                        .expect("integer spec")
                        .scale(),
                    Some(0)
                );
                assert_eq!(
                    numeric_spec_from_scale(Some(MAX_DECIMAL_SCALE))
                        .expect("maximum fractional spec")
                        .scale(),
                    Some(MAX_DECIMAL_SCALE)
                );
                let error = numeric_spec_from_scale(Some(MAX_DECIMAL_SCALE + 1))
                    .expect_err("scale above the V1 limit must fail");
                assert!(
                    error
                        .to_string()
                        .contains(&format!("between 0 and {MAX_DECIMAL_SCALE}")),
                    "{error:?}"
                );
            }
            #[test]
            fn register_alias_derives_from_name_domain_and_dataspace() {
                let mut args = base_register_args();
                args.alias_domain = Some("issuer".parse().expect("valid alias domain"));
                args.alias_dataspace = Some("main".parse().expect("valid alias dataspace"));
                let alias = register_alias_from_args(&args)
                    .expect("alias should derive")
                    .expect("alias should be present");
                assert_eq!(alias.as_ref(), "Rose#issuer.main");
            }
            #[test]
            fn register_alias_derives_short_form_from_name_and_dataspace() {
                let mut args = base_register_args();
                args.alias_dataspace = Some("main".parse().expect("valid alias dataspace"));
                let alias = register_alias_from_args(&args)
                    .expect("alias should derive")
                    .expect("alias should be present");
                assert_eq!(alias.as_ref(), "Rose#main");
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Transfer {
        /// Canonical asset definition id (unprefixed Base58 address) used with `--account`.
        #[arg(long, requires = "account", conflicts_with = "definition_alias")]
        pub definition: Option<AssetDefinitionId>,
        /// Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`) used
        /// with `--account`.
        #[arg(long, requires = "account", conflicts_with = "definition")]
        pub definition_alias: Option<AssetDefinitionAlias>,
        /// Source account identifier (canonical I105), required with asset selectors.
        #[arg(long)]
        pub account: Option<String>,
        /// Optional balance scope (`global` or `dataspace:<id>`).
        #[arg(long, value_parser = parse_asset_balance_scope_literal)]
        pub scope: Option<iroha::data_model::asset::AssetBalanceScope>,
        /// Destination account identifier (canonical I105 literal)
        #[arg(short, long)]
        pub to: String,
        /// Transfer amount (integer or decimal)
        #[arg(short, long)]
        pub quantity: Quantity,
        /// Attempt to register the destination when implicit receive is disabled.
        #[arg(long)]
        pub ensure_destination: bool,
        /// Submit without waiting for confirmation.
        #[arg(long)]
        pub no_wait: bool,
    }
    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Canonical asset definition id (unprefixed Base58 address) used with `--account`.
        #[arg(long, requires = "account", conflicts_with = "definition_alias")]
        pub definition: Option<AssetDefinitionId>,
        /// Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`) used
        /// with `--account`.
        #[arg(long, requires = "account", conflicts_with = "definition")]
        pub definition_alias: Option<AssetDefinitionAlias>,
        /// Account identifier (canonical I105), required with asset selectors.
        #[arg(long)]
        pub account: Option<String>,
        /// Optional balance scope (`global` or `dataspace:<id>`).
        #[arg(long, value_parser = parse_asset_balance_scope_literal)]
        pub scope: Option<iroha::data_model::asset::AssetBalanceScope>,
    }
    #[derive(clap::Args, Debug)]
    pub struct IdQuantity {
        /// Canonical asset definition id (unprefixed Base58 address) used with `--account`.
        #[arg(long, requires = "account", conflicts_with = "definition_alias")]
        pub definition: Option<AssetDefinitionId>,
        /// Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`) used
        /// with `--account`.
        #[arg(long, requires = "account", conflicts_with = "definition")]
        pub definition_alias: Option<AssetDefinitionAlias>,
        /// Account identifier (canonical I105), required with asset selectors.
        #[arg(long)]
        pub account: Option<String>,
        /// Optional balance scope (`global` or `dataspace:<id>`).
        #[arg(long, value_parser = parse_asset_balance_scope_literal)]
        pub scope: Option<iroha::data_model::asset::AssetBalanceScope>,
        /// Amount of change (integer or decimal)
        #[arg(short, long)]
        pub quantity: Quantity,
        /// Submit without waiting for confirmation.
        #[arg(long)]
        pub no_wait: bool,
    }
    impl Id {
        fn resolve_asset_id<C: RunContext>(&self, context: &C) -> Result<AssetId> {
            resolve_asset_id_components(
                context,
                self.definition.clone(),
                self.definition_alias.clone(),
                self.account.clone(),
                self.scope,
            )
        }
    }
    impl IdQuantity {
        fn resolve_asset_id<C: RunContext>(&self, context: &C) -> Result<AssetId> {
            resolve_asset_id_components(
                context,
                self.definition.clone(),
                self.definition_alias.clone(),
                self.account.clone(),
                self.scope,
            )
        }
    }
    impl Transfer {
        fn resolve_asset_id<C: RunContext>(&self, context: &C) -> Result<AssetId> {
            resolve_asset_id_components(
                context,
                self.definition.clone(),
                self.definition_alias.clone(),
                self.account.clone(),
                self.scope,
            )
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all IDs, or full entries when `--verbose` is specified
        All(crate::list_support::AllArgs),
        /// Filter by a given predicate
        Filter(filter::AssetFilter),
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            match self {
                List::All(args) => list_all(context, client.query(FindAssets), &args),
                List::Filter(filter) => {
                    let (predicate, common) = filter.into_list_args().decompose();
                    let builder = client.query(FindAssets).filter(predicate);
                    let builder = apply_common_args(builder, &common)?;
                    let entries = builder.execute_all()?;
                    context.print_data(&entries)
                }
            }
        }
    }
    fn list_all<C: RunContext>(
        context: &mut C,
        builder: iroha::data_model::query::builder::QueryBuilder<'_, Client, FindAssets, Asset>,
        args: &crate::list_support::AllArgs,
    ) -> Result<()> {
        let builder = apply_common_args(builder, &args.common)?;
        let entries = builder.execute_all()?;
        if args.verbose {
            context.print_data(&entries)
        } else {
            let ids: Vec<_> = entries.into_iter().map(|e| e.id().clone()).collect();
            context.print_data(&ids)
        }
    }
    fn apply_common_args<'a>(
        builder: iroha::data_model::query::builder::QueryBuilder<'a, Client, FindAssets, Asset>,
        common: &'a crate::list_support::CommonArgs,
    ) -> Result<iroha::data_model::query::builder::QueryBuilder<'a, Client, FindAssets, Asset>>
    {
        use iroha::data_model::query::parameters::{FetchSize, Pagination, Sorting};
        use std::num::NonZeroU64;
        let mut builder = builder;
        if let Some(key) = common.sort_by_metadata_key.clone() {
            let sorting = Sorting::new(Some(key), common.order.map(Into::into));
            builder = builder.with_sorting(sorting);
        }
        if common.limit.is_some() || common.offset > 0 {
            let pagination = Pagination::new(common.limit.and_then(NonZeroU64::new), common.offset);
            builder = builder.with_pagination(pagination);
        }
        if let Some(n) = common.fetch_size.and_then(NonZeroU64::new) {
            let fs = FetchSize::new(Some(n));
            builder = builder.with_fetch_size(fs);
        }
        if let Some(sel) = common.select.as_deref() {
            let tuple = crate::list_support::parse_selector_tuple::<Asset>(sel)?;
            builder = builder.with_selector_tuple(tuple);
        }
        Ok(builder)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use iroha::data_model::isi::{Instruction, TransferBox};
        use iroha_crypto::Algorithm;
        fn fixture_key_pair(seed: u8) -> KeyPair {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture seed must derive a valid keypair")
        }
        fn sample_transfer_args(ensure_destination: bool) -> (Transfer, AccountId, AssetId) {
            let src = fixture_key_pair(1);
            let dest = fixture_key_pair(2);
            let owner = AccountId::new(src.public_key().clone());
            let to = AccountId::new(dest.public_key().clone());
            let asset_def_id = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").expect("domain id"),
                "rose".parse().expect("asset name"),
            );
            let asset_id = AssetId::new(asset_def_id, owner.clone().into());
            let args = Transfer {
                definition: Some(asset_id.definition().clone()),
                definition_alias: None,
                account: Some(asset_id.account().to_string()),
                scope: None,
                to: to.to_string(),
                quantity: Quantity::from(5_u32),
                ensure_destination,
                no_wait: false,
            };
            (args, to, asset_id)
        }
        #[test]
        fn fixture_key_pair_uses_checked_seed_derivation() {
            assert_eq!(fixture_key_pair(1).algorithm(), Algorithm::Ed25519);
            assert!(
                KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
                "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
            );
        }
        fn assert_transfer_destination(instruction: &InstructionBox, to: &AccountId) {
            let any: &dyn Instruction = &**instruction;
            let transfer = any
                .as_any()
                .downcast_ref::<TransferBox>()
                .expect("transfer instruction");
            let TransferBox::Asset(asset_transfer) = transfer else {
                panic!("expected asset transfer");
            };
            assert_eq!(&asset_transfer.destination, to);
        }
        #[test]
        fn explicit_policy_rejects_ensure_destination_without_explicit_scope() {
            let (args, to, asset_id) = sample_transfer_args(true);
            let policy = AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ExplicitOnly,
                ..AccountAdmissionPolicy::default()
            };
            let err = asset_transfer_instructions(asset_id, &args, &to, Some(&policy))
                .expect_err("explicit-only admission should reject inferred destination scope");
            assert!(
                err.to_string()
                    .contains("no longer infers a registration domain"),
                "unexpected error: {err}"
            );
        }
        #[test]
        fn implicit_policy_skips_register_instruction() {
            let (args, to, asset_id) = sample_transfer_args(true);
            let instructions = asset_transfer_instructions(
                asset_id,
                &args,
                &to,
                Some(&AccountAdmissionPolicy::default()),
            )
            .expect("instructions");
            assert_eq!(instructions.len(), 1);
            assert_transfer_destination(&instructions[0], &to);
        }
        #[test]
        fn ensure_flag_off_sends_transfer_only() {
            let (args, to, asset_id) = sample_transfer_args(false);
            let policy = AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ExplicitOnly,
                ..AccountAdmissionPolicy::default()
            };
            let instructions = asset_transfer_instructions(asset_id, &args, &to, Some(&policy))
                .expect("instructions");
            assert_eq!(instructions.len(), 1);
            assert_transfer_destination(&instructions[0], &to);
        }
    }
}
mod nft {
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Retrieve details of a specific NFT
        Get(Id),
        /// List NFTs
        #[clap(subcommand)]
        List(List),
        /// Register NFT with content provided from stdin in JSON format
        Register(Id),
        /// Unregister NFT
        Unregister(Id),
        /// Transfer ownership of NFT
        Transfer(Transfer),
        /// Read and write metadata
        #[command(subcommand)]
        Meta(metadata::nft::Command),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Get(args) => {
                    let client = context.client_from_config();
                    let entries = client
                        .query(FindNfts)
                        .execute_all()
                        .wrap_err("Failed to get NFT")?;
                    let entry = entries
                        .into_iter()
                        .find(|e| e.id() == &args.id)
                        .ok_or_else(|| eyre!("NFT not found"))?;
                    context.print_data(&entry)
                }
                List(cmd) => cmd.run(context),
                Register(args) => {
                    let metadata: Metadata = parse_json_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::Register::nft(Nft::new(args.id, metadata));
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register NFT")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::nft(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister NFT")
                }
                Transfer(args) => {
                    let from = resolve_account_id(context, &args.from)
                        .wrap_err("failed to resolve --from account")?;
                    let to = resolve_account_id(context, &args.to)
                        .wrap_err("failed to resolve --to account")?;
                    let instruction = iroha::data_model::isi::Transfer::nft(from, args.id, to);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to transfer NFT")
                }
                Meta(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Transfer {
        /// NFT in the format "name$domain"
        #[arg(short, long)]
        pub id: NftId,
        /// Source account identifier (canonical I105 literal)
        #[arg(short, long)]
        pub from: String,
        /// Destination account identifier (canonical I105 literal)
        #[arg(short, long)]
        pub to: String,
    }
    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// NFT in the format "name$domain"
        #[arg(short, long)]
        pub id: NftId,
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all IDs, or full entries when `--verbose` is specified
        All(crate::list_support::AllArgs),
        /// Filter by a given predicate
        Filter(filter::NftFilter),
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            match self {
                List::All(args) => list_all(context, client.query(FindNfts), &args),
                List::Filter(filter) => {
                    let (predicate, common) = filter.into_list_args().decompose();
                    let builder = client.query(FindNfts).filter(predicate);
                    let builder = apply_common_args(builder, &common)?;
                    let entries = builder.execute_all()?;
                    context.print_data(&entries)
                }
            }
        }
    }
    fn list_all<C: RunContext>(
        context: &mut C,
        builder: iroha::data_model::query::builder::QueryBuilder<'_, Client, FindNfts, Nft>,
        args: &crate::list_support::AllArgs,
    ) -> Result<()> {
        let builder = apply_common_args(builder, &args.common)?;
        let entries = builder.execute_all()?;
        if args.verbose {
            context.print_data(&entries)
        } else {
            let ids: Vec<_> = entries.into_iter().map(|e| e.id().clone()).collect();
            context.print_data(&ids)
        }
    }
    fn apply_common_args<'a>(
        builder: iroha::data_model::query::builder::QueryBuilder<'a, Client, FindNfts, Nft>,
        common: &'a crate::list_support::CommonArgs,
    ) -> Result<iroha::data_model::query::builder::QueryBuilder<'a, Client, FindNfts, Nft>> {
        use iroha::data_model::query::parameters::{FetchSize, Pagination, Sorting};
        use std::num::NonZeroU64;
        let mut builder = builder;
        if let Some(key) = common.sort_by_metadata_key.clone() {
            let sorting = Sorting::new(Some(key), common.order.map(Into::into));
            builder = builder.with_sorting(sorting);
        }
        if common.limit.is_some() || common.offset > 0 {
            let pagination = Pagination::new(common.limit.and_then(NonZeroU64::new), common.offset);
            builder = builder.with_pagination(pagination);
        }
        if let Some(n) = common.fetch_size.and_then(NonZeroU64::new) {
            let fs = FetchSize::new(Some(n));
            builder = builder.with_fetch_size(fs);
        }
        if let Some(sel) = common.select.as_deref() {
            let tuple = crate::list_support::parse_selector_tuple::<Nft>(sel)?;
            builder = builder.with_selector_tuple(tuple);
        }
        Ok(builder)
    }
}
mod rwa {
    use iroha::data_model::isi::rwa::{
        ForceTransferRwa, FreezeRwa, HoldRwa, MergeRwas, RedeemRwa, RegisterRwa, ReleaseRwa,
        SetRwaControls, TransferRwa, UnfreezeRwa,
    };
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Retrieve details of a specific RWA lot
        Get(Id),
        /// List RWA lots
        #[clap(subcommand)]
        List(List),
        /// Register an RWA lot using `NewRwa` JSON from stdin
        Register,
        /// Transfer quantity from an existing lot
        Transfer(Transfer),
        /// Merge parent lots using `MergeRwas` JSON from stdin
        Merge,
        /// Redeem quantity from an existing lot
        Redeem(Quantity),
        /// Freeze an existing lot
        Freeze(Id),
        /// Unfreeze an existing lot
        Unfreeze(Id),
        /// Hold quantity on an existing lot
        Hold(Quantity),
        /// Release held quantity from an existing lot
        Release(Quantity),
        /// Force-transfer quantity from an existing lot
        ForceTransfer(ForceTransfer),
        /// Replace the lot control policy using `RwaControlPolicy` JSON from stdin
        SetControls(Id),
        /// Read and write metadata
        #[command(subcommand)]
        Meta(metadata::rwa::Command),
    }
    #[derive(crate::json_macros::JsonDeserialize)]
    struct MergeInput {
        parents: Vec<RwaParentRef>,
        primary_reference: String,
        status: Option<Name>,
        metadata: Metadata,
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Get(args) => {
                    let client = context.client_from_config();
                    let entries = client
                        .query(FindRwas)
                        .execute_all()
                        .wrap_err("Failed to get RWA")?;
                    let entry = entries
                        .into_iter()
                        .find(|e| e.id() == &args.id)
                        .ok_or_else(|| eyre!("RWA not found"))?;
                    context.print_data(&entry)
                }
                List(cmd) => cmd.run(context),
                Register => {
                    let rwa: NewRwa = parse_json_stdin(context)?;
                    context
                        .finish([RegisterRwa { rwa }])
                        .wrap_err("Failed to register RWA")
                }
                Transfer(args) => {
                    let from = resolve_account_id(context, &args.from)
                        .wrap_err("failed to resolve --from account")?;
                    let to = resolve_account_id(context, &args.to)
                        .wrap_err("failed to resolve --to account")?;
                    context
                        .finish([TransferRwa {
                            source: from,
                            rwa: args.id,
                            quantity: args.quantity,
                            destination: to,
                        }])
                        .wrap_err("Failed to transfer RWA")
                }
                Merge => {
                    let merge: MergeInput = parse_json_stdin(context)?;
                    context
                        .finish([MergeRwas {
                            parents: merge.parents,
                            primary_reference: merge.primary_reference,
                            status: merge.status,
                            metadata: merge.metadata,
                        }])
                        .wrap_err("Failed to merge RWAs")
                }
                Redeem(args) => context
                    .finish([RedeemRwa {
                        rwa: args.id,
                        quantity: args.quantity,
                    }])
                    .wrap_err("Failed to redeem RWA"),
                Freeze(args) => context
                    .finish([FreezeRwa { rwa: args.id }])
                    .wrap_err("Failed to freeze RWA"),
                Unfreeze(args) => context
                    .finish([UnfreezeRwa { rwa: args.id }])
                    .wrap_err("Failed to unfreeze RWA"),
                Hold(args) => context
                    .finish([HoldRwa {
                        rwa: args.id,
                        quantity: args.quantity,
                    }])
                    .wrap_err("Failed to hold RWA quantity"),
                Release(args) => context
                    .finish([ReleaseRwa {
                        rwa: args.id,
                        quantity: args.quantity,
                    }])
                    .wrap_err("Failed to release RWA hold"),
                ForceTransfer(args) => {
                    let to = resolve_account_id(context, &args.to)
                        .wrap_err("failed to resolve --to account")?;
                    context
                        .finish([ForceTransferRwa {
                            rwa: args.id,
                            quantity: args.quantity,
                            destination: to,
                        }])
                        .wrap_err("Failed to force-transfer RWA")
                }
                SetControls(args) => {
                    let controls: RwaControlPolicy = parse_json_stdin(context)?;
                    context
                        .finish([SetRwaControls {
                            rwa: args.id,
                            controls,
                        }])
                        .wrap_err("Failed to update RWA controls")
                }
                Meta(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// RWA identifier in the format `hash$domain`
        #[arg(short, long)]
        pub id: RwaId,
    }
    #[derive(clap::Args, Debug)]
    pub struct Quantity {
        /// RWA identifier in the format `hash$domain`
        #[arg(short, long)]
        pub id: RwaId,
        /// Quantity for the operation
        #[arg(short, long)]
        pub quantity: iroha_primitives::numeric::Quantity,
    }
    #[derive(clap::Args, Debug)]
    pub struct Transfer {
        /// RWA identifier in the format `hash$domain`
        #[arg(short, long)]
        pub id: RwaId,
        /// Source account identifier (canonical I105 literal)
        #[arg(short, long)]
        pub from: String,
        /// Quantity to transfer
        #[arg(short, long)]
        pub quantity: iroha_primitives::numeric::Quantity,
        /// Destination account identifier (canonical I105 literal)
        #[arg(short, long)]
        pub to: String,
    }
    #[derive(clap::Args, Debug)]
    pub struct ForceTransfer {
        /// RWA identifier in the format `hash$domain`
        #[arg(short, long)]
        pub id: RwaId,
        /// Quantity to transfer
        #[arg(short, long)]
        pub quantity: iroha_primitives::numeric::Quantity,
        /// Destination account identifier (canonical I105 literal)
        #[arg(short, long)]
        pub to: String,
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all IDs, or full entries when `--verbose` is specified
        All(crate::list_support::AllArgs),
        /// Filter by a given predicate
        Filter(filter::RwaFilter),
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            match self {
                List::All(args) => list_all(context, client.query(FindRwas), &args),
                List::Filter(filter) => {
                    let (predicate, common) = filter.into_list_args().decompose();
                    let builder = client.query(FindRwas).filter(predicate);
                    let builder = apply_common_args(builder, &common)?;
                    let entries = builder.execute_all()?;
                    context.print_data(&entries)
                }
            }
        }
    }
    fn list_all<C: RunContext>(
        context: &mut C,
        builder: iroha::data_model::query::builder::QueryBuilder<'_, Client, FindRwas, Rwa>,
        args: &crate::list_support::AllArgs,
    ) -> Result<()> {
        let builder = apply_common_args(builder, &args.common)?;
        let entries = builder.execute_all()?;
        if args.verbose {
            context.print_data(&entries)
        } else {
            let ids: Vec<_> = entries.into_iter().map(|e| e.id().clone()).collect();
            context.print_data(&ids)
        }
    }
    fn apply_common_args<'a>(
        builder: iroha::data_model::query::builder::QueryBuilder<'a, Client, FindRwas, Rwa>,
        common: &'a crate::list_support::CommonArgs,
    ) -> Result<iroha::data_model::query::builder::QueryBuilder<'a, Client, FindRwas, Rwa>> {
        use iroha::data_model::query::parameters::{FetchSize, Pagination, Sorting};
        use std::num::NonZeroU64;
        let mut builder = builder;
        if let Some(key) = common.sort_by_metadata_key.clone() {
            let sorting = Sorting::new(Some(key), common.order.map(Into::into));
            builder = builder.with_sorting(sorting);
        }
        if common.limit.is_some() || common.offset > 0 {
            let pagination = Pagination::new(common.limit.and_then(NonZeroU64::new), common.offset);
            builder = builder.with_pagination(pagination);
        }
        if let Some(n) = common.fetch_size.and_then(NonZeroU64::new) {
            let fs = FetchSize::new(Some(n));
            builder = builder.with_fetch_size(fs);
        }
        if let Some(sel) = common.select.as_deref() {
            let tuple = crate::list_support::parse_selector_tuple::<Rwa>(sel)?;
            builder = builder.with_selector_tuple(tuple);
        }
        Ok(builder)
    }
}
mod peer {
    use super::*;
    use iroha::data_model::isi::register::RegisterPeerWithPop;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List registered peers expected to connect with each other
        #[command(subcommand)]
        List(List),
        /// Register a peer
        Register(RegisterPeer),
        /// Unregister a peer
        Unregister(Id),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                List(cmd) => cmd.run(context),
                Register(args) => {
                    let instruction = args.build_instruction()?;
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register peer")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::peer(args.key.into());
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister peer")
                }
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct RegisterPeer {
        /// Peer's public key in multihash format (must be BLS-normal)
        #[arg(short, long)]
        pub key: PublicKey,
        /// Proof-of-possession bytes as hex (with or without 0x prefix)
        #[arg(long, value_name = "HEX")]
        pub pop: String,
    }
    impl RegisterPeer {
        fn build_instruction(self) -> Result<InstructionBox> {
            let trimmed = self.pop.trim();
            let pop_str = trimmed
                .strip_prefix("0x")
                .or_else(|| trimmed.strip_prefix("0X"))
                .unwrap_or(trimmed);
            let pop_bytes = hex::decode(pop_str).wrap_err("Failed to decode PoP hex")?;
            Ok(RegisterPeerWithPop::new(self.key.into(), pop_bytes).into())
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all registered peers
        All(crate::list_support::AllArgs),
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            match self {
                List::All(args) => list_all(context, client.query(FindPeers), &args),
            }
        }
    }
    fn list_all<C: RunContext>(
        context: &mut C,
        builder: iroha::data_model::query::builder::QueryBuilder<'_, Client, FindPeers, PeerId>,
        args: &crate::list_support::AllArgs,
    ) -> Result<()> {
        let builder = apply_common_args(builder, &args.common)?;
        let entries = builder.execute_all()?;
        if args.verbose {
            context.print_data(&entries)
        } else {
            let ids = peer_ids_only(&entries);
            context.print_data(&ids)
        }
    }
    fn peer_ids_only(entries: &[PeerId]) -> Vec<String> {
        entries.iter().map(ToString::to_string).collect()
    }
    fn apply_common_args<'a>(
        builder: iroha::data_model::query::builder::QueryBuilder<'a, Client, FindPeers, PeerId>,
        common: &'a crate::list_support::CommonArgs,
    ) -> Result<iroha::data_model::query::builder::QueryBuilder<'a, Client, FindPeers, PeerId>>
    {
        use iroha::data_model::query::parameters::{FetchSize, Pagination, Sorting};
        use std::num::NonZeroU64;
        let mut builder = builder;
        if let Some(key) = common.sort_by_metadata_key.clone() {
            let sorting = Sorting::new(Some(key), common.order.map(Into::into));
            builder = builder.with_sorting(sorting);
        }
        if common.limit.is_some() || common.offset > 0 {
            let pagination = Pagination::new(common.limit.and_then(NonZeroU64::new), common.offset);
            builder = builder.with_pagination(pagination);
        }
        if let Some(n) = common.fetch_size.and_then(NonZeroU64::new) {
            let fs = FetchSize::new(Some(n));
            builder = builder.with_fetch_size(fs);
        }
        if let Some(sel) = common.select.as_deref() {
            let tuple = crate::list_support::parse_selector_tuple::<PeerId>(sel)?;
            builder = builder.with_selector_tuple(tuple);
        }
        Ok(builder)
    }
    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Peer's public key in multihash format
        #[arg(short, long)]
        pub key: PublicKey,
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use iroha_test_samples::PEER_KEYPAIR;
        #[test]
        fn peer_ids_only_renders_public_key_strings() {
            let peer_id = PeerId::from(PEER_KEYPAIR.public_key().clone());
            let rendered = peer_ids_only(&[peer_id]);
            assert_eq!(rendered, vec![PEER_KEYPAIR.public_key().to_string()]);
        }
    }
}
mod multisig {
    use core::convert::TryFrom;
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::{NonZeroU16, NonZeroU64},
        time::{Duration, SystemTime},
    };
    use iroha::executor_data_model::isi::multisig::*;
    use super::*;
    type ProposalKey = HashOf<Vec<InstructionBox>>;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List pending multisig proposals for explicitly selected authorities
        #[command(subcommand)]
        List(List),
        /// Register a multisig account
        Register(Box<Register>),
        /// Propose a multisig transaction using JSON input from stdin
        Propose(Propose),
        /// Approve a multisig transaction
        Approve(Approve),
        /// Propose cancellation of an existing multisig transaction
        Cancel(Cancel),
        /// Inspect a multisig account controller and print the CTAP2 payload + digest
        Inspect(Inspect),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                List(cmd) => cmd.run(context),
                Register(cmd) => cmd.run(context),
                Propose(cmd) => cmd.run(context),
                Approve(cmd) => cmd.run(context),
                Cancel(cmd) => cmd.run(context),
                Inspect(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Register {
        /// List of signatories for the multisig account (canonical I105 literal)
        #[arg(short, long, num_args(2..))]
        pub signatories: Vec<String>,
        /// Relative weights of signatories' responsibilities
        #[arg(short, long, num_args(2..))]
        pub weights: Vec<u8>,
        /// Threshold of total weight required for authentication
        #[arg(short, long)]
        pub quorum: u16,
        /// Account id to use for the multisig controller. If omitted, a new
        /// random domainless account id is generated locally, the private key is
        /// discarded, and the registration defaults to a domainless home-domain policy.
        #[arg(long)]
        pub account: Option<String>,
        /// Time-to-live for multisig transactions.
        /// Example: "1y 6M 2w 3d 12h 30m 30s"
        #[arg(short, long, default_value_t = default_transaction_ttl())]
        pub transaction_ttl: humantime::Duration,
    }
    fn default_transaction_ttl() -> humantime::Duration {
        std::time::Duration::from_millis(DEFAULT_MULTISIG_TTL_MS).into()
    }
    impl Run for Register {
        #[allow(clippy::too_many_lines)]
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            if self.signatories.len() != self.weights.len() {
                return Err(eyre!("signatories and weights must be equal in length"));
            }
            let mut signatories = Vec::with_capacity(self.signatories.len());
            for literal in self.signatories {
                let account = resolve_account_id(context, &literal)
                    .wrap_err_with(|| format!("failed to resolve signatory `{literal}`"))?;
                signatories.push(account);
            }
            let mut signatories_with_weights = BTreeMap::new();
            for (account, weight) in signatories.into_iter().zip(self.weights.into_iter()) {
                if weight == 0 {
                    return Err(eyre!("signatory weights must be non-zero"));
                }
                if signatories_with_weights.insert(account, weight).is_some() {
                    return Err(eyre!("duplicate signatory entries are not allowed"));
                }
            }
            let account = if let Some(literal) = self.account {
                resolve_account_id(context, &literal).wrap_err("failed to resolve --account")?
            } else {
                let generated =
                    KeyPair::try_random().wrap_err("failed to generate multisig account id")?;
                AccountId::new(generated.public_key().clone())
            };
            let quorum =
                NonZeroU16::new(self.quorum).ok_or_else(|| eyre!("quorum should not be 0"))?;
            let transaction_ttl_ms = self
                .transaction_ttl
                .as_millis()
                .try_into()
                .ok()
                .and_then(NonZeroU64::new)
                .ok_or_else(|| eyre!("ttl should be between 1 ms and 584942417 years"))?;
            let spec = MultisigSpec::new(signatories_with_weights, quorum, transaction_ttl_ms);
            if !context.output_instructions() {
                context.println(format!("multisig account id: {account}"))?;
            }
            let instruction =
                MultisigRegister::with_account(account.clone(), None::<DomainId>, spec);
            context
                .finish([iroha::data_model::isi::InstructionBox::from(instruction)])
                .wrap_err("Failed to register multisig account")
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Propose {
        /// Multisig authority managing the proposed transaction
        #[arg(short, long)]
        pub account: String,
        /// Overrides the default time-to-live for this transaction.
        /// Example: "1y 6M 2w 3d 12h 30m 30s"
        /// Must not exceed the multisig policy TTL; the CLI will preview the
        /// effective expiry and reject overrides above the policy cap.
        #[arg(short, long)]
        pub transaction_ttl: Option<humantime::Duration>,
    }
    impl Run for Propose {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let instructions: Vec<InstructionBox> = parse_json_stdin(context)?;
            let transaction_ttl_ms = self.transaction_ttl.map(|duration| {
                duration
                    .as_millis()
                    .try_into()
                    .ok()
                    .and_then(NonZeroU64::new)
                    .expect("ttl should be between 1 ms and 584942417 years")
            });
            let account = resolve_account_id(context, &self.account)
                .wrap_err("failed to resolve --account")?;
            if !context.output_instructions() {
                surface_policy_ttl(context, &account, transaction_ttl_ms)?;
            }
            let instructions_hash = HashOf::new(&instructions);
            if matches!(context.output_format(), CliOutputFormat::Text) {
                context.println(format_args!("instructions_hash: {instructions_hash}"))?;
            }
            let propose_multisig_transaction =
                MultisigPropose::new(account, instructions, transaction_ttl_ms);
            context
                .finish([iroha::data_model::isi::InstructionBox::from(
                    propose_multisig_transaction,
                )])
                .wrap_err("Failed to propose transaction")
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Approve {
        /// Multisig authority of the transaction
        #[arg(short, long)]
        pub account: String,
        /// Hash of the instructions to approve
        #[arg(short, long)]
        pub instructions_hash: ProposalKey,
    }
    impl Run for Approve {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let account = resolve_account_id(context, &self.account)
                .wrap_err("failed to resolve --account")?;
            let approve_multisig_transaction =
                MultisigApprove::new(account, self.instructions_hash);
            context
                .finish([iroha::data_model::isi::InstructionBox::from(
                    approve_multisig_transaction,
                )])
                .wrap_err("Failed to approve transaction")
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Cancel {
        /// Multisig authority of the transaction
        #[arg(short, long)]
        pub account: String,
        /// Hash of the target proposal instructions to cancel
        #[arg(short, long)]
        pub instructions_hash: ProposalKey,
        /// Overrides the default time-to-live for the cancel proposal itself.
        #[arg(short, long)]
        pub transaction_ttl: Option<humantime::Duration>,
    }
    impl Run for Cancel {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let transaction_ttl_ms = self.transaction_ttl.map(|duration| {
                duration
                    .as_millis()
                    .try_into()
                    .ok()
                    .and_then(NonZeroU64::new)
                    .expect("ttl should be between 1 ms and 584942417 years")
            });
            let account = resolve_account_id(context, &self.account)
                .wrap_err("failed to resolve --account")?;
            if !context.output_instructions() {
                surface_policy_ttl(context, &account, transaction_ttl_ms)?;
            }
            let cancel_instructions = vec![InstructionBox::from(MultisigCancel::new(
                account.clone(),
                self.instructions_hash,
            ))];
            let cancel_hash = HashOf::new(&cancel_instructions);
            if matches!(context.output_format(), CliOutputFormat::Text) {
                context.println(format_args!("cancel_proposal_hash: {cancel_hash}"))?;
            }
            let propose_cancel =
                MultisigPropose::new(account, cancel_instructions, transaction_ttl_ms);
            context
                .finish([iroha::data_model::isi::InstructionBox::from(propose_cancel)])
                .wrap_err("Failed to propose cancellation")
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Inspect {
        /// Multisig account identifier to inspect
        #[arg(short, long)]
        pub account: String,
        /// Emit JSON instead of human-readable output
        #[arg(long)]
        pub json: bool,
    }
    impl Run for Inspect {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use iroha::data_model::prelude::FindAccountById;
            let account_id = resolve_account_id(context, &self.account)
                .wrap_err("failed to resolve --account")?;
            let client = context.client_from_config();
            let account: Account = client
                .query_single(FindAccountById::new(account_id.clone()))
                .wrap_err_with(|| format!("account `{account_id}` not found"))?;
            let controller = account
                .id()
                .controller()
                .multisig_policy()
                .ok_or_else(|| eyre!("account `{}` is not multisig-controlled", account_id))?;
            let ctap2 = controller.encode_ctap2();
            let digest = controller.digest_blake2b256();
            let ctap2_hex = hex::encode_upper(&ctap2);
            let digest_hex = hex::encode_upper(digest);
            if self.json {
                let members = controller
                    .members()
                    .iter()
                    .map(|member| {
                        let (algorithm, payload) = member
                            .public_key()
                            .try_to_bytes()
                            .wrap_err("account controller member public key is malformed")?;
                        let mut member_map = json::Map::new();
                        member_map.insert(
                            "algorithm".to_string(),
                            json::Value::from(algorithm.as_static_str()),
                        );
                        member_map.insert("weight".to_string(), json::Value::from(member.weight()));
                        member_map.insert(
                            "public_key_hex".to_string(),
                            json::Value::from(hex::encode_upper(payload)),
                        );
                        Ok(json::Value::from(member_map))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let mut doc_map = json::Map::new();
                doc_map.insert(
                    "account_id".to_string(),
                    json::Value::from(account.id().to_string()),
                );
                doc_map.insert(
                    "version".to_string(),
                    json::Value::from(controller.version()),
                );
                doc_map.insert(
                    "threshold".to_string(),
                    json::Value::from(controller.threshold()),
                );
                doc_map.insert(
                    "total_weight".to_string(),
                    json::Value::from(controller.total_weight()),
                );
                doc_map.insert("members".to_string(), json::Value::from(members));
                doc_map.insert(
                    "ctap2_cbor_hex".to_string(),
                    json::Value::from(format!("0x{ctap2_hex}")),
                );
                doc_map.insert(
                    "digest_blake2b256_hex".to_string(),
                    json::Value::from(format!("0x{digest_hex}")),
                );
                let doc = json::Value::from(doc_map);
                context.print_data(&doc)
            } else {
                context.println(format!(
                    "Account: {}\nVersion: {}\nThreshold: {}\nTotal Weight: {}\nCTAP2 CBOR: 0x{}\nDigest (BLAKE2b-256, \"iroha-ms-policy\"): 0x{}",
                    account.id(),
                    controller.version(),
                    controller.threshold(),
                    controller.total_weight(),
                    ctap2_hex,
                    digest_hex,
                ))?;
                context.println("Members (algorithm, weight, public key hex):")?;
                for member in controller.members() {
                    let (algorithm, payload) = member
                        .public_key()
                        .try_to_bytes()
                        .wrap_err("account controller member public key is malformed")?;
                    context.println(format!(
                        "  - {}, {}, {}",
                        algorithm.as_static_str(),
                        member.weight(),
                        hex::encode_upper(payload),
                    ))?;
                }
                Ok(())
            }
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List pending proposals for an explicit finite set of multisig authorities
        All {
            /// Exact multisig account id or canonical alias to query; repeat for each authority
            #[arg(long = "multisig-selector", required = true)]
            multisig_selectors: Vec<String>,
            /// Maximum number of proposals to emit after server ordering (client-side cap)
            #[arg(long)]
            limit: Option<u64>,
            /// Number of ordered proposals to skip after fetching cursor pages
            #[arg(long, default_value_t = 0)]
            offset: u64,
            /// Cursor page size for each remote proposals query request
            #[arg(long)]
            fetch_size: Option<u64>,
        },
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let (multisig_selectors, limit, offset, fetch_size) = match self {
                Self::All {
                    multisig_selectors,
                    limit,
                    offset,
                    fetch_size,
                } => (multisig_selectors, limit, offset, fetch_size),
            };
            let entries = load_multisig_list_all_entries(
                &client,
                &multisig_selectors,
                fetch_size,
                offset,
                limit,
            )?;
            match context.output_format() {
                CliOutputFormat::Json => context.print_data(&entries),
                CliOutputFormat::Text => {
                    let rendered = render_multisig_list_all_text(&entries)?;
                    if rendered.is_empty() {
                        return Ok(());
                    }
                    context.println(rendered)
                }
            }
        }
    }
    fn spec_key() -> Name {
        "multisig/spec".parse().expect("valid multisig spec key")
    }
    const COLLECTING_SIGNATURES_STATUS: &str = "COLLECTING_SIGNATURES";
    fn surface_policy_ttl<C: RunContext>(
        context: &mut C,
        multisig_account: &AccountId,
        override_ttl_ms: Option<NonZeroU64>,
    ) -> Result<()> {
        use iroha::data_model::prelude::FindAccountById;
        let client = context.client_from_config();
        let account = match client.query_single(FindAccountById::new(multisig_account.clone())) {
            Ok(account) => account,
            Err(err) => {
                context.println(format!(
                    "Unable to fetch multisig policy for {multisig_account}: {err}"
                ))?;
                return Ok(());
            }
        };
        let Some(policy) = account
            .metadata()
            .get(&spec_key())
            .map(|value| value.clone().try_into_any_norito::<MultisigSpec>())
            .transpose()
            .wrap_err("Failed to parse multisig spec from account metadata")?
        else {
            context.println(format!(
                "No multisig/spec metadata found for {multisig_account}; TTL overrides above the policy cap will be rejected by the node."
            ))?;
            return Ok(());
        };
        if let Some(ttl_ms) = override_ttl_ms {
            validate_ttl_override(ttl_ms, policy.transaction_ttl_ms)?;
            emit_ttl_hint(
                context,
                ttl_ms,
                policy.transaction_ttl_ms,
                Some(multisig_account),
            )?;
        } else {
            emit_ttl_hint(
                context,
                policy.transaction_ttl_ms,
                policy.transaction_ttl_ms,
                Some(multisig_account),
            )?;
        }
        Ok(())
    }
    fn validate_ttl_override(
        requested_ttl_ms: NonZeroU64,
        policy_ttl_ms: NonZeroU64,
    ) -> Result<()> {
        if requested_ttl_ms > policy_ttl_ms {
            eyre::bail!(
                "Requested multisig TTL {} ms exceeds the policy cap {} ms; retry with a TTL at or below the policy limit.",
                requested_ttl_ms,
                policy_ttl_ms,
            );
        }
        Ok(())
    }
    fn emit_ttl_hint<C: RunContext>(
        context: &mut C,
        effective_ttl_ms: NonZeroU64,
        policy_ttl_ms: NonZeroU64,
        account: Option<&AccountId>,
    ) -> Result<()> {
        let now = SystemTime::now();
        let expiry = now
            .checked_add(Duration::from_millis(effective_ttl_ms.get()))
            .map(humantime::format_rfc3339)
            .map_or_else(
                || "expiry exceeds supported range".to_string(),
                |value| value.to_string(),
            );
        let account_note = account.map_or_else(String::new, |id| format!(" on {id}"));
        context.println(format!(
            "Multisig TTL{account_note}: using {effective_ttl_ms} ms (policy cap {policy_ttl_ms} ms), expires approximately {expiry}"
        ))
    }
    #[derive(Debug, Clone, PartialEq, Eq, crate::json_macros::JsonSerialize)]
    struct MultisigListAllEntry {
        multisig_account_id: AccountId,
        proposal_id: String,
        instructions_hash: String,
        status: String,
        operation_type: String,
        intent: Option<Json>,
        proposed_at_ms: u64,
        terminal_at_ms: Option<u64>,
        proposal: MultisigProposalValue,
    }
    fn proposal_query_request_for_selector(
        selector: &str,
        cursor: Option<String>,
        limit: Option<u64>,
    ) -> Result<iroha::client::MultisigProposalsQueryRequest> {
        if selector.is_empty() || selector.trim() != selector {
            eyre::bail!("multisig selectors must be exact non-empty literals");
        }
        let parsed_account = AccountId::parse_encoded(selector)
            .map(iroha::data_model::account::ParsedAccountId::into_account_id)
            .ok();
        let (multisig_account_id, multisig_account_alias) = match parsed_account {
            Some(account_id) => (Some(account_id), None),
            None if selector.contains('@') => (None, Some(selector.to_owned())),
            None => eyre::bail!(
                "multisig selector `{selector}` must be a canonical I105 account id or account alias"
            ),
        };
        Ok(iroha::client::MultisigProposalsQueryRequest {
            multisig_account_id,
            multisig_account_alias,
            status: vec![COLLECTING_SIGNATURES_STATUS.to_owned()],
            cursor,
            limit,
        })
    }
    fn multisig_list_all_entry_from_proposal(
        multisig_account_id: AccountId,
        proposal: iroha::client::MultisigProposalEntry,
    ) -> MultisigListAllEntry {
        let iroha::client::MultisigProposalEntry {
            proposal_id,
            instructions_hash,
            operation_type,
            intent,
            proposal,
            status,
            terminal_at_ms,
        } = proposal;
        MultisigListAllEntry {
            multisig_account_id,
            proposal_id,
            instructions_hash,
            status,
            operation_type,
            intent,
            proposed_at_ms: proposal.proposed_at_ms,
            terminal_at_ms,
            proposal,
        }
    }
    fn collect_multisig_proposals_with<F>(
        selectors: &[String],
        fetch_size: Option<u64>,
        fetch_page: &mut F,
    ) -> Result<Vec<MultisigListAllEntry>>
    where
        F: FnMut(
            iroha::client::MultisigProposalsQueryRequest,
        ) -> Result<iroha::client::MultisigProposalsQueryResponse>,
    {
        if selectors.is_empty() {
            eyre::bail!("at least one --multisig-selector is required");
        }
        let mut seen_selectors = BTreeSet::new();
        let mut merged = BTreeMap::<(AccountId, String), MultisigListAllEntry>::new();
        for selector in selectors {
            if !seen_selectors.insert(selector.clone()) {
                eyre::bail!("duplicate multisig selector `{selector}`");
            }
            let mut cursor = None;
            let mut seen_cursors = BTreeSet::new();
            let mut resolved_account_id = None;
            loop {
                let request =
                    proposal_query_request_for_selector(selector, cursor.clone(), fetch_size)?;
                let response = fetch_page(request)?;
                if let Some(expected) = resolved_account_id.as_ref() {
                    if expected != &response.resolved_multisig_account_id {
                        eyre::bail!(
                            "multisig selector `{selector}` resolved to different accounts across cursor pages"
                        );
                    }
                } else {
                    resolved_account_id = Some(response.resolved_multisig_account_id.clone());
                }
                for proposal in response.proposals {
                    let entry = multisig_list_all_entry_from_proposal(
                        response.resolved_multisig_account_id.clone(),
                        proposal,
                    );
                    let key = (
                        entry.multisig_account_id.clone(),
                        entry.instructions_hash.clone(),
                    );
                    if let Some(existing) = merged.get(&key) {
                        if existing != &entry {
                            eyre::bail!(
                                "conflicting multisig proposal payload for {} on {}",
                                entry.instructions_hash,
                                entry.multisig_account_id
                            );
                        }
                    } else {
                        merged.insert(key, entry);
                    }
                }
                let Some(next_cursor) = response.next_cursor else {
                    break;
                };
                if next_cursor.is_empty() || !seen_cursors.insert(next_cursor.clone()) {
                    eyre::bail!(
                        "multisig selector `{selector}` returned an invalid or repeated cursor"
                    );
                }
                cursor = Some(next_cursor);
            }
        }
        let mut entries = merged.into_values().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            right
                .proposed_at_ms
                .cmp(&left.proposed_at_ms)
                .then_with(|| left.instructions_hash.cmp(&right.instructions_hash))
                .then_with(|| left.multisig_account_id.cmp(&right.multisig_account_id))
        });
        Ok(entries)
    }
    fn load_multisig_list_all_entries(
        client: &Client,
        selectors: &[String],
        fetch_size: Option<u64>,
        offset: u64,
        limit: Option<u64>,
    ) -> Result<Vec<MultisigListAllEntry>> {
        let mut fetch_page = |request| client.post_multisig_proposals_query(&request);
        let entries = collect_multisig_proposals_with(selectors, fetch_size, &mut fetch_page)?;
        let offset = usize::try_from(offset).wrap_err("multisig offset exceeds usize")?;
        let limit = limit
            .map(|value| usize::try_from(value).wrap_err("multisig limit exceeds usize"))
            .transpose()?
            .unwrap_or(usize::MAX);
        Ok(entries.into_iter().skip(offset).take(limit).collect())
    }
    fn format_multisig_intent(intent: &Option<Json>) -> Result<String> {
        match intent {
            Some(value) => norito::json::to_json(value)
                .map_err(|err| eyre!("failed to render multisig intent: {err}")),
            None => Ok("null".to_owned()),
        }
    }
    fn render_multisig_list_all_text(entries: &[MultisigListAllEntry]) -> Result<String> {
        let mut blocks = Vec::with_capacity(entries.len());
        for entry in entries {
            blocks.push(format!(
                "multisig_account_id: {}\nproposal_id: {}\nstatus: {}\noperation_type: {}\nintent: {}\nproposed_at_ms: {}",
                entry.multisig_account_id,
                entry.proposal_id,
                entry.status,
                entry.operation_type,
                format_multisig_intent(&entry.intent)?,
                entry.proposed_at_ms,
            ));
        }
        Ok(blocks.join("\n\n"))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use iroha::crypto::{Algorithm, KeyPair};
        use std::collections::BTreeSet;
        fn fixture_key_pair(seed: u8) -> KeyPair {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture seed must derive a valid keypair")
        }
        #[test]
        fn fixture_key_pair_uses_checked_seed_derivation() {
            assert_eq!(fixture_key_pair(0x40).algorithm(), Algorithm::Ed25519);
            assert!(
                KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
                "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
            );
        }
        fn sample_proposal_entry(
            suffix: &str,
            proposed_at_ms: u64,
        ) -> iroha::client::MultisigProposalEntry {
            let proposal = MultisigProposalValue::new(
                Vec::new(),
                proposed_at_ms,
                proposed_at_ms + 60_000,
                BTreeSet::new(),
                None,
            );
            let instructions_hash = format!("{suffix:0>64}");
            iroha::client::MultisigProposalEntry {
                proposal_id: instructions_hash.clone(),
                instructions_hash,
                proposal,
                operation_type: "TRANSFER".to_owned(),
                intent: Some(Json::new(norito::json!({ "sequence": suffix }))),
                status: COLLECTING_SIGNATURES_STATUS.to_owned(),
                terminal_at_ms: None,
            }
        }
        #[test]
        fn ttl_override_within_policy_is_allowed() {
            let policy = NonZeroU64::new(5_000).unwrap();
            let requested = NonZeroU64::new(4_000).unwrap();
            assert!(validate_ttl_override(requested, policy).is_ok());
        }
        #[test]
        fn ttl_override_above_policy_is_rejected() {
            let policy = NonZeroU64::new(5_000).unwrap();
            let requested = NonZeroU64::new(6_000).unwrap();
            let err = validate_ttl_override(requested, policy).unwrap_err();
            let message = err.to_string();
            assert!(message.contains("exceeds the policy cap"));
            assert!(message.contains("6000"));
            assert!(message.contains("5000"));
        }
        #[test]
        fn selector_explicit_collection_merges_pages_and_authorities_deterministically() {
            let first_account = AccountId::new(fixture_key_pair(0x51).public_key().clone());
            let second_account = AccountId::new(fixture_key_pair(0x52).public_key().clone());
            let selectors = vec!["first@sbp".to_owned(), "second@sbp".to_owned()];
            let mut requests = Vec::new();
            let mut fetch_page = |request: iroha::client::MultisigProposalsQueryRequest| {
                let selector = request
                    .multisig_account_alias
                    .clone()
                    .expect("alias selector");
                requests.push((selector.clone(), request.cursor.clone(), request.limit));
                let page = match (selector.as_str(), request.cursor.as_deref()) {
                    ("first@sbp", None) => iroha::client::MultisigProposalsQueryResponse {
                        resolved_multisig_account_id: first_account.clone(),
                        proposals: vec![sample_proposal_entry("0", 5)],
                        next_cursor: Some("first-next".to_owned()),
                    },
                    ("first@sbp", Some("first-next")) => {
                        iroha::client::MultisigProposalsQueryResponse {
                            resolved_multisig_account_id: first_account.clone(),
                            proposals: vec![sample_proposal_entry("2", 3)],
                            next_cursor: None,
                        }
                    }
                    ("second@sbp", None) => iroha::client::MultisigProposalsQueryResponse {
                        resolved_multisig_account_id: second_account.clone(),
                        proposals: vec![
                            sample_proposal_entry("1", 4),
                            sample_proposal_entry("3", 2),
                        ],
                        next_cursor: None,
                    },
                    other => panic!("unexpected request {other:?}"),
                };
                Ok(page)
            };
            let actual = collect_multisig_proposals_with(&selectors, Some(2), &mut fetch_page)
                .expect("collect proposals");
            let proposed_at = actual
                .iter()
                .map(|entry| entry.proposed_at_ms)
                .collect::<Vec<_>>();
            assert_eq!(proposed_at, vec![5, 4, 3, 2]);
            assert_eq!(
                requests,
                vec![
                    ("first@sbp".to_owned(), None, Some(2)),
                    (
                        "first@sbp".to_owned(),
                        Some("first-next".to_owned()),
                        Some(2),
                    ),
                    ("second@sbp".to_owned(), None, Some(2)),
                ]
            );
        }
        #[test]
        fn selector_explicit_collection_rejects_empty_duplicate_and_repeated_cursor_inputs() {
            let mut never_fetch =
                |_request| -> Result<iroha::client::MultisigProposalsQueryResponse> {
                    panic!("invalid selector must fail before I/O")
                };
            assert!(collect_multisig_proposals_with(&[], None, &mut never_fetch).is_err());
            assert!(
                collect_multisig_proposals_with(
                    &["same@sbp".to_owned(), "same@sbp".to_owned()],
                    None,
                    &mut |request| Ok(iroha::client::MultisigProposalsQueryResponse {
                        resolved_multisig_account_id: AccountId::new(
                            fixture_key_pair(0x53).public_key().clone(),
                        ),
                        proposals: Vec::new(),
                        next_cursor: request.cursor.is_none().then(|| "next".to_owned()),
                    }),
                )
                .is_err()
            );
            let account = AccountId::new(fixture_key_pair(0x54).public_key().clone());
            let mut fetch = |_request| {
                Ok(iroha::client::MultisigProposalsQueryResponse {
                    resolved_multisig_account_id: account.clone(),
                    proposals: Vec::new(),
                    next_cursor: Some("loop".to_owned()),
                })
            };
            let error = collect_multisig_proposals_with(&["loop@sbp".to_owned()], None, &mut fetch)
                .expect_err("repeated cursor must fail closed");
            assert!(error.to_string().contains("repeated cursor"));
        }
        #[test]
        fn selector_explicit_collection_deduplicates_identical_entries_and_rejects_conflicts() {
            let account = AccountId::new(fixture_key_pair(0x56).public_key().clone());
            let selectors = vec!["first@sbp".to_owned(), "second@sbp".to_owned()];
            let identical = sample_proposal_entry("b", 7);
            let mut fetch_identical = |_request| {
                Ok(iroha::client::MultisigProposalsQueryResponse {
                    resolved_multisig_account_id: account.clone(),
                    proposals: vec![identical.clone()],
                    next_cursor: None,
                })
            };
            let deduplicated =
                collect_multisig_proposals_with(&selectors, None, &mut fetch_identical)
                    .expect("identical selector projections should deduplicate");
            assert_eq!(deduplicated.len(), 1);
            let mut calls = 0_u8;
            let mut fetch_conflict = |_request| {
                calls += 1;
                let mut entry = identical.clone();
                if calls == 2 {
                    entry.operation_type = "MINT".to_owned();
                }
                Ok(iroha::client::MultisigProposalsQueryResponse {
                    resolved_multisig_account_id: account.clone(),
                    proposals: vec![entry],
                    next_cursor: None,
                })
            };
            let error = collect_multisig_proposals_with(&selectors, None, &mut fetch_conflict)
                .expect_err("conflicting duplicate proposal projections must fail closed");
            assert!(
                error
                    .to_string()
                    .contains("conflicting multisig proposal payload")
            );
        }
        #[test]
        fn render_multisig_list_all_text_outputs_human_readable_blocks() {
            let account = AccountId::new(fixture_key_pair(0x55).public_key().clone());
            let entry =
                multisig_list_all_entry_from_proposal(account, sample_proposal_entry("a", 42));
            let rendered =
                render_multisig_list_all_text(std::slice::from_ref(&entry)).expect("render text");
            assert!(rendered.contains("multisig_account_id: "));
            assert!(rendered.contains(&format!("proposal_id: {:0>64}", "a")));
            assert!(rendered.contains("status: COLLECTING_SIGNATURES"));
            assert!(rendered.contains("operation_type: TRANSFER"));
            assert!(rendered.contains("intent: {\"sequence\":\"a\"}"));
            assert!(rendered.contains("proposed_at_ms: 42"));
        }
        #[test]
        fn render_multisig_list_all_text_is_empty_for_empty_results() {
            let rendered = render_multisig_list_all_text(&[]).expect("render empty text");
            assert!(rendered.is_empty());
        }
    }
}
mod query {
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Query using JSON input from stdin
        Stdin(Stdin),
        /// Query using raw `SignedQuery` (base64 or hex) from stdin
        StdinRaw(StdinRaw),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            match self {
                Command::Stdin(cmd) => cmd.run(context),
                Command::StdinRaw(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Stdin;
    impl Run for Stdin {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            // Read a Norito-JSON-like envelope describing a singular/iterable query,
            // sign it with the client's key/authority, submit via /query, and print the response.
            // Accepted JSON shapes:
            // {"singular": {"type": "FindParameters"}}
            // {"singular": {"type": "FindContractManifestByCodeHash", "payload": {"code_hash": "0x.."}}}
            // {"iterable": {"type": "FindPeers", "params": {"limit": 100, "offset": 0, "fetch_size": 128}}}
            use iroha::data_model::query::json::QueryEnvelopeJson;
            let client = context.client_from_config();
            let buf = string_from_stdin()?;
            let envelope: QueryEnvelopeJson = parse_json(&buf).wrap_err("decode query envelope")?;
            let request = envelope
                .into_request()
                .map_err(|err| eyre!(format!("invalid query JSON: {err}")))?;
            let resp = client.execute_query_request(request)?;
            match resp {
                iroha::data_model::query::QueryResponse::Singular(out) => context.print_data(&out),
                iroha::data_model::query::QueryResponse::Iterable(out) => context.print_data(&out),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct ContinueArgs {
        /// `ForwardCursor` encoded as base64 (preferred) or hex (0x...)
        #[arg(long, value_name = "B64_OR_HEX")]
        cursor: String,
    }
    impl Run for ContinueArgs {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use iroha::data_model::query::QueryRequest;
            let client = context.client_from_config();
            let bytes = decode_base64_or_hex(
                &self.cursor,
                "invalid hex length for ForwardCursor",
                "invalid cursor hex",
            )?;
            let cursor: iroha::data_model::query::parameters::ForwardCursor =
                norito::decode_from_bytes(&bytes).wrap_err("decode ForwardCursor")?;
            let request = QueryRequest::Continue(cursor);
            let resp = client.execute_query_request(request)?;
            match resp {
                iroha::data_model::query::QueryResponse::Singular(out) => context.print_data(&out),
                iroha::data_model::query::QueryResponse::Iterable(out) => context.print_data(&out),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct StdinRaw;
    impl Run for StdinRaw {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let s = string_from_stdin()?;
            let s = s.trim();
            let body =
                decode_base64_or_hex(s, "invalid hex length for SignedQuery body", "invalid hex")?;
            let resp = client.execute_signed_query_raw(&body)?;
            match resp {
                iroha::data_model::query::QueryResponse::Singular(out) => context.print_data(&out),
                iroha::data_model::query::QueryResponse::Iterable(out) => context.print_data(&out),
            }
        }
    }
}
mod transaction {
    use iroha::data_model::{Level as LogLevel, isi::Log, metadata::Metadata, name::Name};
    use std::{
        sync::{
            Arc, LazyLock, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Read the typed pipeline status of a submitted transaction
        Status(Status),
        /// Retrieve details of a specific transaction
        Get(Get),
        /// Send an empty transaction that logs a message
        Ping(Ping),
        /// Send a transaction using IVM bytecode
        Ivm(Ivm),
        /// Send a transaction using JSON input from stdin
        Stdin(Stdin),
        /// Build and sign stdin instructions locally, then print their exact framed size without submitting
        SignedSize(SignedSize),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Status(cmd) => cmd.run(context),
                Get(cmd) => cmd.run(context),
                Ping(cmd) => cmd.run(context),
                Ivm(cmd) => cmd.run(context),
                Stdin(cmd) => cmd.run(context),
                SignedSize(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Status {
        /// Hash of the signed transaction to inspect
        #[arg(short('H'), long)]
        pub hash: HashOf<iroha::data_model::transaction::SignedTransaction>,
        /// Explicit status routing scope. Omit with `--wait`, which selects the safe scope for
        /// the requested terminal states.
        #[arg(long, value_enum, conflicts_with = "wait")]
        pub scope: Option<StatusScope>,
        #[command(flatten)]
        pub wait: TransactionWaitArgs,
    }
    #[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
    pub enum StatusScope {
        /// Query only the configured Torii peer.
        Local,
        /// Permit Torii's global/fanout status lookup.
        Global,
    }
    impl Run for Status {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            if self.wait.wait {
                let status = crate::wait_for_transaction_status(&client, self.hash, &self.wait)?;
                context.print_data(&status)
            } else {
                let status = match self.scope.unwrap_or(StatusScope::Local) {
                    StatusScope::Local => {
                        client.get_transaction_status_response_local(self.hash)?
                    }
                    StatusScope::Global => {
                        client.get_transaction_status_response_auto(self.hash)?
                    }
                }
                .ok_or_else(|| eyre!("Transaction status not found"))?;
                context.print_data(&status)
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Get {
        /// Hash of the transaction to retrieve
        #[arg(short('H'), long)]
        pub hash: HashOf<TransactionEntrypoint>,
    }
    impl Run for Get {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let transaction = client
                .query(FindTransactions)
                .execute_all()?
                .into_iter()
                .find(|t| t.entrypoint_hash() == &self.hash)
                .ok_or_else(|| eyre!("Transaction not found"))?;
            context.print_data(&transaction)
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Ping {
        /// Log levels: TRACE, DEBUG, INFO, WARN, ERROR (in increasing order of visibility)
        #[arg(short, long, default_value = "DEBUG")]
        pub log_level: LogLevel,
        /// Log message
        #[arg(short, long)]
        pub msg: String,
        /// Number of ping transactions to send
        #[arg(long, default_value_t = 1)]
        pub count: usize,
        /// Number of parallel workers to use when sending multiple pings
        #[arg(long, default_value_t = 1)]
        pub parallel: usize,
        /// Maximum number of parallel workers (0 disables the cap)
        #[arg(long, default_value_t = DEFAULT_PING_PARALLEL_CAP)]
        pub parallel_cap: usize,
        /// Submit without waiting for confirmation
        #[arg(long)]
        pub no_wait: bool,
        /// Do not suffix message with "-<index>" when count > 1
        #[arg(long)]
        pub no_index: bool,
    }
    struct PingBatchResult {
        attempted: usize,
        failed: usize,
        first_error: Option<eyre::Report>,
    }
    pub const DEFAULT_PING_PARALLEL_CAP: usize = 1024;
    static PING_NONCE_KEY: LazyLock<Name> = LazyLock::new(|| {
        use std::str::FromStr;
        Name::from_str("ping_nonce").expect("ping nonce metadata key must be valid")
    });
    fn ping_message(base: &str, index: usize, count: usize, no_index: bool) -> String {
        if count <= 1 || no_index {
            return base.to_owned();
        }
        format!("{base}-{}", index + 1)
    }
    fn ping_nonce_seed() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }
    fn maybe_add_ping_nonce(metadata: &mut Metadata, seed: u64, index: usize) -> bool {
        if metadata.contains(&*PING_NONCE_KEY) {
            return false;
        }
        let value = seed.wrapping_add(index as u64);
        metadata.insert(PING_NONCE_KEY.clone(), value);
        true
    }
    fn resolve_ping_parallel(count: usize, parallel: usize, parallel_cap: usize) -> (usize, bool) {
        let cap = if parallel_cap == 0 {
            usize::MAX
        } else {
            parallel_cap
        };
        let baseline = parallel.min(count);
        let resolved = baseline.min(cap);
        (resolved, resolved < baseline)
    }
    fn dispatch_ping_work<F, G>(count: usize, parallel: usize, make_worker: F) -> PingBatchResult
    where
        F: Fn() -> G + Sync,
        G: FnMut(usize) -> Result<()> + Send,
    {
        let parallel = parallel.min(count);
        let next = AtomicUsize::new(0);
        let failures = AtomicUsize::new(0);
        let first_error: Mutex<Option<eyre::Report>> = Mutex::new(None);
        thread::scope(|scope| {
            for _ in 0..parallel {
                let make_worker = &make_worker;
                let next = &next;
                let failures = &failures;
                let first_error = &first_error;
                scope.spawn(move || {
                    let mut worker = make_worker();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= count {
                            break;
                        }
                        if let Err(err) = worker(index) {
                            failures.fetch_add(1, Ordering::Relaxed);
                            let mut guard = first_error.lock().expect("lock");
                            if guard.is_none() {
                                *guard = Some(err);
                            }
                        }
                    }
                });
            }
        });
        PingBatchResult {
            attempted: count,
            failed: failures.load(Ordering::Relaxed),
            first_error: first_error.lock().expect("lock").take(),
        }
    }
    impl Run for Ping {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let Ping {
                log_level,
                msg,
                count,
                parallel,
                parallel_cap,
                no_wait,
                no_index,
            } = self;
            if count == 0 {
                eyre::bail!("`--count` must be greater than zero");
            }
            if parallel == 0 {
                eyre::bail!("`--parallel` must be greater than zero");
            }
            let (parallel, clamped) = resolve_ping_parallel(count, parallel, parallel_cap);
            if clamped {
                context.println(format!(
                    "Clamped --parallel to {parallel} (cap {parallel_cap})"
                ))?;
            }
            if count > 1 || parallel > 1 {
                if context.input_instructions() || context.output_instructions() {
                    eyre::bail!(
                        "Incompatible `--input` `--output` flags with batch `iroha transaction ping`"
                    );
                }
                let ping_seed = if no_index && count > 1 {
                    Some(ping_nonce_seed())
                } else {
                    None
                };
                let client = Client::new(context.config().clone());
                let metadata = context.transaction_metadata().cloned().unwrap_or_default();
                let fee_payment = context.transaction_fee_payment()?;
                let i18n = context.i18n().clone();
                let base_msg = msg;
                let first_quote = Arc::new(Mutex::new(None));
                let first_quote_for_workers = Arc::clone(&first_quote);
                let result = dispatch_ping_work(count, parallel, move || {
                    let client = client.clone();
                    let metadata = metadata.clone();
                    let fee_payment = fee_payment.clone();
                    let i18n = i18n.clone();
                    let base_msg = base_msg.clone();
                    let first_quote = Arc::clone(&first_quote_for_workers);
                    move |index| {
                        let message = ping_message(&base_msg, index, count, no_index);
                        let instruction = Log::new(log_level, message);
                        let mut metadata = metadata.clone();
                        if let Some(seed) = ping_seed {
                            let _ = maybe_add_ping_nonce(&mut metadata, seed, index);
                        }
                        let executable = Executable::Instructions(
                            vec![InstructionBox::from(instruction)].into(),
                        );
                        let (transaction, quote) = quote_and_sign_transaction(
                            &client,
                            executable,
                            fee_payment.clone(),
                            metadata,
                        )
                        .wrap_err("Failed to quote and sign ping transaction")?;
                        let mut quote_slot = first_quote.lock().expect("lock");
                        if quote_slot.is_none() {
                            *quote_slot = Some(quote);
                        }
                        drop(quote_slot);
                        let submit = if no_wait {
                            client.submit_transaction(&transaction).map(|_| ())
                        } else {
                            client.submit_transaction_blocking(&transaction).map(|_| ())
                        };
                        submit.map_err(|err| {
                            let err = map_account_admission_error(err, &i18n);
                            let err_msg = if cfg!(debug_assertions) {
                                let tx = format!("{transaction:?}");
                                i18n.t_with(
                                    "error.submit_transaction_debug",
                                    &[("transaction", tx.as_str())],
                                )
                            } else {
                                i18n.t("error.submit_transaction")
                            };
                            err.wrap_err(err_msg)
                        })
                    }
                });
                if let Some(quote) = first_quote.lock().expect("lock").as_ref() {
                    print_fee_quote_text(context, quote)?;
                }
                let submitted = result.attempted.saturating_sub(result.failed);
                if no_wait {
                    context.println(format!(
                        "Submitted {submitted}/{} ping transactions without confirmation",
                        result.attempted
                    ))?;
                } else {
                    context.println(format!(
                        "Submitted {submitted}/{} ping transactions with confirmation",
                        result.attempted
                    ))?;
                }
                if result.failed > 0 {
                    if let Some(err) = result.first_error {
                        return Err(
                            err.wrap_err(format!("{} ping submissions failed", result.failed))
                        );
                    }
                    eyre::bail!("{} ping submissions failed", result.failed);
                }
                return Ok(());
            }
            let instruction = Log::new(log_level, msg);
            if no_wait {
                context.finish_unconfirmed([instruction])
            } else {
                context.finish([instruction])
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn ping_message_appends_index_when_multiple() {
            assert_eq!(ping_message("hello", 0, 3, false), "hello-1");
            assert_eq!(ping_message("hello", 2, 3, false), "hello-3");
        }
        #[test]
        fn ping_message_respects_no_index() {
            assert_eq!(ping_message("hello", 0, 3, true), "hello");
            assert_eq!(ping_message("hello", 0, 1, false), "hello");
        }
        #[test]
        fn dispatch_ping_work_tracks_failures() {
            let result = dispatch_ping_work(5, 3, || {
                move |index| {
                    if index % 2 == 0 {
                        eyre::bail!("boom");
                    }
                    Ok(())
                }
            });
            assert_eq!(result.attempted, 5);
            assert!(result.failed >= 2);
            assert!(result.first_error.is_some());
        }
        #[test]
        fn resolve_ping_parallel_caps_workers() {
            let (parallel, clamped) = resolve_ping_parallel(10, 8, 4);
            assert_eq!(parallel, 4);
            assert!(clamped);
        }
        #[test]
        fn resolve_ping_parallel_allows_cap_disable() {
            let (parallel, clamped) = resolve_ping_parallel(10, 8, 0);
            assert_eq!(parallel, 8);
            assert!(!clamped);
        }
        #[test]
        fn ping_nonce_inserts_when_missing() {
            let mut metadata = Metadata::default();
            let inserted = maybe_add_ping_nonce(&mut metadata, 10, 2);
            assert!(inserted);
            let value = metadata
                .get(&*PING_NONCE_KEY)
                .expect("ping nonce should be set")
                .try_into_any_norito::<u64>()
                .expect("ping nonce should be u64");
            assert_eq!(value, 12);
        }
        #[test]
        fn ping_nonce_skips_when_present() {
            let mut metadata = Metadata::default();
            metadata.insert(PING_NONCE_KEY.clone(), 99_u64);
            let inserted = maybe_add_ping_nonce(&mut metadata, 10, 2);
            assert!(!inserted);
            let value = metadata
                .get(&*PING_NONCE_KEY)
                .expect("ping nonce should remain set")
                .try_into_any_norito::<u64>()
                .expect("ping nonce should be u64");
            assert_eq!(value, 99);
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Ivm {
        /// Path to the IVM bytecode file. If omitted, reads from stdin
        #[arg(short, long)]
        path: Option<PathBuf>,
        /// Signature-bound transaction gas limit for this IVM submit.
        #[arg(long, value_name("U64"))]
        gas_limit: Option<u64>,
    }
    impl Run for Ivm {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let Ivm { path, gas_limit } = self;
            let blob = if let Some(path) = path {
                read_cli_file_bounded(&path, "IVM bytecode")
                    .wrap_err("Failed to read IVM bytecode from the file into the buffer")?
            } else {
                bytes_from_stdin()
                    .wrap_err("Failed to read IVM bytecode from stdin into the buffer")?
            };
            let metadata = context.transaction_metadata().cloned().unwrap_or_default();
            context
                .submit_with_metadata_and_gas(
                    IvmBytecode::from_compiled(blob),
                    metadata,
                    true,
                    gas_limit,
                )
                .wrap_err("Failed to submit an IVM transaction")
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Stdin;
    impl Run for Stdin {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let instructions: Vec<InstructionBox> = parse_json_stdin(context)?;
            context
                .finish(instructions)
                .wrap_err("Failed to submit parsed instructions")
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct SignedSize;
    impl Run for SignedSize {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            if context.output_instructions() {
                eyre::bail!("Incompatible `--output` flag with `iroha tx signed-size`");
            }
            let instructions: Vec<InstructionBox> = parse_json_stdin(context)?;
            let metadata = context.transaction_metadata().cloned().unwrap_or_default();
            let fee_payment = context.transaction_fee_payment()?;
            let client = context.client_from_config();
            let executable = Executable::Instructions(instructions.into());
            let (transaction, fee_quote) =
                quote_and_sign_transaction(&client, executable, fee_payment, metadata)
                    .wrap_err("Failed to quote and sign transaction for exact size measurement")?;
            let signed_transaction_bytes = u64::try_from(
                norito::to_bytes(&transaction)
                    .wrap_err("Failed to encode signed transaction for exact size measurement")?
                    .len(),
            )
            .unwrap_or(u64::MAX);
            match context.output_format() {
                CliOutputFormat::Json => {
                    let result = json_utils::json_object(vec![
                        (
                            "signed_transaction_bytes",
                            json_utils::json_value(&signed_transaction_bytes)?,
                        ),
                        ("fee_quote", json_utils::json_value(&fee_quote)?),
                    ])?;
                    context.print_data(&result)
                }
                CliOutputFormat::Text => {
                    print_fee_quote_text(context, &fee_quote)?;
                    context.println(format_args!(
                        "signed_transaction_bytes: {signed_transaction_bytes}"
                    ))
                }
            }
        }
    }
}
mod role {
    use super::*;
    use crate::json_utils;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Read and write role permissions
        #[command(subcommand)]
        Permission(PermissionCommand),
        /// List role IDs
        #[command(subcommand)]
        List(List),
        /// Register a role and grant it to the registrant
        Register(Id),
        /// Unregister a role
        Unregister(Id),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Permission(cmd) => cmd.run(context),
                List(cmd) => cmd.run(context),
                Register(args) => {
                    let instruction = iroha::data_model::isi::Register::role(Role::new(
                        args.id,
                        context.config().account.clone(),
                    ));
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register role")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::role(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister role")
                }
            }
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum PermissionCommand {
        /// List role permissions
        List(RolePermList),
        /// Grant role permission using JSON input from stdin
        Grant(Id),
        /// Revoke role permission using JSON input from stdin
        Revoke(Id),
    }
    impl Run for PermissionCommand {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::PermissionCommand::*;
            match self {
                List(args) => {
                    let client = context.client_from_config();
                    let role = client
                        .query(FindRoles)
                        .execute_all()?
                        .into_iter()
                        .find(|r| r.id() == &args.id)
                        .ok_or_else(|| eyre!("Role not found"))?;
                    let perms: Vec<_> = role.permissions().cloned().collect();
                    // Apply client-side pagination for consistency (no dedicated query exists)
                    let start = args
                        .offset
                        .try_into()
                        .map_or(perms.len(), |offset: usize| perms.len().min(offset));
                    let end = args
                        .limit
                        .and_then(|n| usize::try_from(n).ok())
                        .map_or(perms.len(), |lim| {
                            perms.len().min(start.saturating_add(lim))
                        });
                    let page = json_utils::json_array(perms[start..end].iter().cloned())?;
                    context.print_data(&page)
                }
                Grant(args) => {
                    let permission: Permission = parse_json_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::Grant::role_permission(permission, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to grant the permission to the role")
                }
                Revoke(args) => {
                    let permission: Permission = parse_json_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::Revoke::role_permission(permission, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to revoke the permission from the role")
                }
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Role name
        #[arg(short, long)]
        id: RoleId,
    }
    #[derive(clap::Args, Debug)]
    pub struct RolePermList {
        /// Role name
        #[arg(short, long)]
        id: RoleId,
        /// Maximum number of items to return (client-side for now)
        #[arg(long)]
        limit: Option<u64>,
        /// Offset into the result set (client-side for now)
        #[arg(long, default_value_t = 0)]
        offset: u64,
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all role IDs
        All {
            /// Maximum number of items to return (server-side limit)
            #[arg(long)]
            limit: Option<u64>,
            /// Offset into the result set (server-side offset)
            #[arg(long, default_value_t = 0)]
            offset: u64,
            /// Batch fetch size for iterable queries
            #[arg(long)]
            fetch_size: Option<u64>,
        },
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            match self {
                List::All {
                    limit,
                    offset,
                    fetch_size,
                } => {
                    let mut builder = client.query(FindRoleIds);
                    if limit.is_some() || offset > 0 {
                        let pagination = iroha::data_model::query::parameters::Pagination::new(
                            limit.and_then(NonZeroU64::new),
                            offset,
                        );
                        builder = builder.with_pagination(pagination);
                    }
                    if let Some(n) = fetch_size.and_then(NonZeroU64::new) {
                        let fs = iroha::data_model::query::parameters::FetchSize::new(Some(n));
                        builder = builder.with_fetch_size(fs);
                    }
                    let ids = builder.execute_all()?;
                    context.print_data(&ids)
                }
            }
        }
    }
}
mod parameter {
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List system parameters
        #[command(subcommand)]
        List(List),
        /// Set a system parameter using JSON input from stdin
        Set(Set),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                List(cmd) => cmd.run(context),
                Set(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all system parameters
        All,
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let params = client.query_single(FindParameters)?;
            context.print_data(&params)
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Set;
    impl Run for Set {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let entry: Parameter = parse_json_stdin(context)?;
            let instruction = SetParameter::new(entry);
            context.finish([instruction])
        }
    }
}
#[allow(clippy::large_enum_variant)]
mod trigger {
    use super::*;
    use clap::ValueEnum;
    use std::collections::BTreeMap;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List trigger IDs
        #[command(subcommand)]
        List(List),
        /// Retrieve details of a specific trigger
        Get(Id),
        /// Register a trigger
        Register(Register),
        /// Unregister a trigger
        Unregister(Id),
        /// Increase the number of trigger executions
        Mint(IdInt),
        /// Decrease the number of trigger executions
        Burn(IdInt),
        /// Enable a trigger by setting metadata key `__enabled=true`
        Enable(TriggerIdArg),
        /// Disable a trigger by setting metadata key `__enabled=false`
        Disable(TriggerIdArg),
        /// Execute a by-call trigger with optional JSON arguments
        Execute(Execute),
        /// Inspect trigger declaration and optional live completion evidence
        Inspect(Inspect),
        /// Collect or watch trigger completion events
        #[command(subcommand)]
        Completed(Completed),
        /// Read and write metadata
        #[command(subcommand)]
        Meta(metadata::trigger::Command),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                List(cmd) => cmd.run(context),
                Get(args) => {
                    let client = context.client_from_config();
                    let entry: Trigger = client
                        .query_single(FindTriggerById::new(args.id))
                        .wrap_err("Failed to get trigger")?;
                    let printable = trigger_pretty_json(&entry)
                        .wrap_err("Failed to serialise trigger for display")?;
                    context.print_data(&printable)
                }
                Register(args) => args.run(context),
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::trigger(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister trigger")
                }
                Mint(args) => {
                    let instruction = iroha::data_model::isi::Mint::trigger_repetitions(
                        args.repetitions,
                        args.id,
                    );
                    context
                        .finish([instruction])
                        .wrap_err("Failed to mint trigger repetitions")
                }
                Burn(args) => {
                    let instruction = iroha::data_model::isi::Burn::trigger_repetitions(
                        args.repetitions,
                        args.id,
                    );
                    context
                        .finish([instruction])
                        .wrap_err("Failed to burn trigger repetitions")
                }
                Enable(args) => set_trigger_enabled(context, args.id, true),
                Disable(args) => set_trigger_enabled(context, args.id, false),
                Execute(args) => args.run(context),
                Inspect(args) => args.run(context),
                Completed(cmd) => cmd.run(context),
                Meta(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List registered trigger IDs
        All {
            /// Only list active trigger IDs
            #[arg(long)]
            active: bool,
            /// Maximum number of items to return (server-side limit)
            #[arg(long)]
            limit: Option<u64>,
            /// Offset into the result set (server-side offset)
            #[arg(long, default_value_t = 0)]
            offset: u64,
            /// Batch fetch size for iterable queries
            #[arg(long)]
            fetch_size: Option<u64>,
        },
    }
    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            match self {
                List::All {
                    active,
                    limit,
                    offset,
                    fetch_size,
                } => {
                    if active {
                        let mut builder = client.query(FindActiveTriggerIds);
                        if limit.is_some() || offset > 0 {
                            let pagination = iroha::data_model::query::parameters::Pagination::new(
                                limit.and_then(NonZeroU64::new),
                                offset,
                            );
                            builder = builder.with_pagination(pagination);
                        }
                        if let Some(n) = fetch_size.and_then(NonZeroU64::new) {
                            let fs = iroha::data_model::query::parameters::FetchSize::new(Some(n));
                            builder = builder.with_fetch_size(fs);
                        }
                        let ids = builder.execute_all()?;
                        context.print_data(&ids)
                    } else {
                        let mut builder = client.query(FindTriggers);
                        if limit.is_some() || offset > 0 {
                            let pagination = iroha::data_model::query::parameters::Pagination::new(
                                limit.and_then(NonZeroU64::new),
                                offset,
                            );
                            builder = builder.with_pagination(pagination);
                        }
                        if let Some(n) = fetch_size.and_then(NonZeroU64::new) {
                            let fs = iroha::data_model::query::parameters::FetchSize::new(Some(n));
                            builder = builder.with_fetch_size(fs);
                        }
                        let triggers = builder.execute_all()?;
                        let ids: Vec<_> = triggers
                            .into_iter()
                            .map(|trigger| trigger.id().clone())
                            .collect();
                        context.print_data(&ids)
                    }
                }
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Trigger name
        #[arg(short, long)]
        pub id: TriggerId,
    }
    #[derive(clap::Args, Debug)]
    pub struct IdInt {
        /// Trigger name
        #[arg(short, long)]
        pub id: TriggerId,
        /// Amount of change (integer)
        #[arg(short, long)]
        pub repetitions: u32,
    }
    #[derive(clap::Args, Debug)]
    pub struct TriggerIdArg {
        /// Trigger name
        pub id: TriggerId,
    }
    fn set_trigger_enabled<C: RunContext>(
        context: &mut C,
        id: TriggerId,
        enabled: bool,
    ) -> Result<()> {
        let key: Name = "__enabled"
            .parse()
            .wrap_err("failed to construct trigger enabled metadata key")?;
        let instruction =
            iroha::data_model::isi::SetKeyValue::trigger(id, key, Json::from(enabled));
        context
            .finish([instruction])
            .wrap_err("Failed to set trigger enabled metadata")
    }
    #[derive(clap::Args, Debug)]
    pub struct Execute {
        /// Trigger name
        pub id: TriggerId,
        /// JSON object passed as trigger execution arguments
        #[arg(long, default_value = "{}")]
        pub args_json: String,
        /// Include runtime completion and pipeline diagnostics from Torii after finality.
        #[arg(long)]
        pub trace: bool,
        #[command(flatten)]
        pub wait: TransactionWaitArgs,
    }
    impl Execute {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let args: norito::json::Value = crate::parse_json(&self.args_json)
                .wrap_err("Failed to parse --args-json as JSON")?;
            let instruction =
                iroha::data_model::isi::ExecuteTrigger::new(self.id.clone()).with_args(args);
            let executable =
                Executable::Instructions(vec![InstructionBox::from(instruction)].into());
            let metadata = context.transaction_metadata().cloned().unwrap_or_default();
            let fee_payment = context.transaction_fee_payment()?;
            let client = context.client_from_config();
            let (transaction, fee_quote) =
                quote_and_sign_transaction(&client, executable, fee_payment, metadata)
                    .wrap_err("Failed to quote and sign trigger execution transaction")?;
            let hash = transaction.hash();
            client
                .submit_transaction(&transaction)
                .wrap_err("Failed to submit trigger execution transaction")?;
            let mut pairs = vec![
                ("hash", json_utils::json_value(&hash)?),
                ("trigger_id", json_utils::json_value(&self.id)?),
                ("transaction", json_utils::json_value(&transaction)?),
                ("fee_quote", json_utils::json_value(&fee_quote)?),
                ("trace_requested", json_utils::json_value(&self.trace)?),
            ];
            if self.wait.is_enabled() {
                let status = wait_for_transaction_status(&client, hash, &self.wait)?;
                pairs.push(("finalized", json_utils::json_value(&true)?));
                pairs.push((
                    "terminal_kind",
                    json_utils::json_value(&status.terminal_kind)?,
                ));
                pairs.push(("attempts", json_utils::json_value(&status.attempts)?));
                pairs.push(("elapsed_ms", json_utils::json_value(&status.elapsed_ms)?));
                pairs.push((
                    "block_height",
                    json_utils::json_value(&status.block_height)?,
                ));
                pairs.push(("scope", json_utils::json_value(&status.scope)?));
                pairs.push((
                    "resolved_from",
                    json_utils::json_value(&status.resolved_from)?,
                ));
                if self.trace {
                    let trace = if let Some(height) = status.block_height {
                        let completions = client.get_trigger_completions(
                            Some(&self.id.to_string()),
                            None,
                            Some("all"),
                            Some(height),
                            Some(height),
                            Some(100),
                            Some(1),
                        )?;
                        norito::json!({
                            "mode": "committed_trigger_completion",
                            "pipeline": (status.r#final.clone()),
                            "completion_query": (completions),
                        })
                    } else {
                        norito::json!({
                            "mode": "pipeline_status",
                            "pipeline": (status.r#final.clone()),
                            "completion_query": null,
                        })
                    };
                    pairs.push(("trace", trace));
                }
                pairs.push(("final", json_utils::json_value(&status.r#final)?));
            } else {
                pairs.push(("finalized", json_utils::json_value(&false)?));
                if self.trace {
                    pairs.push((
                        "trace",
                        norito::json!({
                            "mode": "submit_only",
                            "message": "trace hydration requires waiting for finality",
                        }),
                    ));
                }
            }
            let response = json_utils::json_object(pairs)?;
            context.print_data(&response)
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum Completed {
        /// List matching trigger completions from committed block history.
        List(CompletedList),
        /// Stream matching trigger completion events until interrupted, timed out, or limited.
        Watch(CompletedWatch),
    }
    impl Completed {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            match self {
                Self::List(args) => args.run(context),
                Self::Watch(args) => args.run(context),
            }
        }
    }
    #[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CompletedOutcomeArg {
        All,
        Success,
        Failure,
    }
    impl CompletedOutcomeArg {
        fn as_str(self) -> &'static str {
            match self {
                Self::All => "all",
                Self::Success => "success",
                Self::Failure => "failure",
            }
        }
        fn apply(
            self,
            filter: iroha::data_model::events::trigger_completed::TriggerCompletedEventFilter,
        ) -> iroha::data_model::events::trigger_completed::TriggerCompletedEventFilter {
            use iroha::data_model::events::trigger_completed::TriggerCompletedOutcomeType;
            match self {
                Self::All => filter,
                Self::Success => filter.for_outcome(TriggerCompletedOutcomeType::Success),
                Self::Failure => filter.for_outcome(TriggerCompletedOutcomeType::Failure),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct CompletedList {
        /// Optional trigger ID filter.
        #[arg(long)]
        pub id: Option<TriggerId>,
        /// Optional completion outcome filter.
        #[arg(long, value_enum, default_value_t = CompletedOutcomeArg::All)]
        pub outcome: CompletedOutcomeArg,
        /// Maximum completion records to return.
        #[arg(long, default_value_t = 10)]
        pub limit: u64,
        /// First block height to scan. Defaults to the recent bounded window.
        #[arg(long)]
        pub from_height: Option<u64>,
        /// Last block height to scan. Defaults to the current committed height.
        #[arg(long)]
        pub to_height: Option<u64>,
        /// Hard cap on blocks scanned, including when --from-height is supplied.
        #[arg(long, default_value_t = 1_000)]
        pub scan_limit_blocks: u64,
        /// Deprecated compatibility flag; `list` is historical and does not wait.
        #[arg(long, hide = true)]
        pub timeout_ms: Option<u64>,
    }
    #[derive(clap::Args, Debug)]
    pub struct CompletedWatch {
        /// Optional trigger ID filter.
        #[arg(long)]
        pub id: Option<TriggerId>,
        /// Optional completion outcome filter.
        #[arg(long, value_enum, default_value_t = CompletedOutcomeArg::All)]
        pub outcome: CompletedOutcomeArg,
        /// Optional maximum events to emit before returning.
        #[arg(long)]
        pub limit: Option<u64>,
        /// Optional maximum live-stream watch time.
        #[arg(long)]
        pub timeout_ms: Option<u64>,
    }
    impl CompletedList {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let trigger_id = self.id.as_ref().map(ToString::to_string);
            let response = client.get_trigger_completions(
                trigger_id.as_deref(),
                None,
                Some(self.outcome.as_str()),
                self.from_height,
                self.to_height,
                Some(self.limit),
                Some(self.scan_limit_blocks),
            )?;
            context.print_data(&response)
        }
    }
    impl CompletedWatch {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let filter = completed_filter(self.id, self.outcome);
            let timeout = self.timeout_ms.map(Duration::from_millis);
            if timeout.is_none() && self.limit.is_none() {
                let client = context.client_from_config();
                client
                    .listen_for_events([filter])
                    .wrap_err("Failed to listen for trigger completion events")?
                    .try_for_each(|event| {
                        if let iroha::data_model::events::EventBox::TriggerCompleted(event) = event?
                        {
                            context.print_data(&event)?;
                        }
                        Ok::<(), eyre::Report>(())
                    })?;
                return Ok(());
            }
            let events = collect_completed_events(context, filter, timeout, self.limit)?;
            for event in events {
                context.print_data(&event)?;
            }
            Ok(())
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Inspect {
        /// Trigger name.
        pub id: TriggerId,
        /// Also collect recent live completion evidence for this duration.
        #[arg(long, default_value_t = 0)]
        pub completion_timeout_ms: u64,
        /// Maximum live completion events to include when completion collection is enabled.
        #[arg(long, default_value_t = 5)]
        pub completion_limit: u64,
    }
    impl Inspect {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let entry: Trigger = client
                .query_single(FindTriggerById::new(self.id.clone()))
                .wrap_err("Failed to get trigger")?;
            let trigger =
                trigger_pretty_json(&entry).wrap_err("Failed to serialise trigger for display")?;
            let completions = if self.completion_timeout_ms > 0 {
                collect_completed_events(
                    context,
                    completed_filter(Some(self.id.clone()), CompletedOutcomeArg::All),
                    Some(Duration::from_millis(self.completion_timeout_ms)),
                    Some(self.completion_limit),
                )?
            } else {
                Vec::new()
            };
            let response = json_utils::json_object(vec![
                ("trigger", trigger),
                (
                    "completion_collection",
                    json_utils::json_object(vec![
                        (
                            "enabled",
                            json_utils::json_value(&(self.completion_timeout_ms > 0))?,
                        ),
                        (
                            "timeout_ms",
                            json_utils::json_value(&self.completion_timeout_ms)?,
                        ),
                        ("limit", json_utils::json_value(&self.completion_limit)?),
                    ])?,
                ),
                ("recent_completions", json_utils::json_value(&completions)?),
            ])?;
            context.print_data(&response)
        }
    }
    fn completed_filter(
        id: Option<TriggerId>,
        outcome: CompletedOutcomeArg,
    ) -> iroha::data_model::events::EventFilterBox {
        use iroha::data_model::events::trigger_completed::TriggerCompletedEventFilter;
        let mut filter = TriggerCompletedEventFilter::new();
        if let Some(id) = id {
            filter = filter.for_trigger(id);
        }
        iroha::data_model::events::EventFilterBox::TriggerCompleted(outcome.apply(filter))
    }
    fn collect_completed_events<C: RunContext>(
        context: &mut C,
        filter: iroha::data_model::events::EventFilterBox,
        timeout: Option<Duration>,
        limit: Option<u64>,
    ) -> Result<Vec<iroha::data_model::events::trigger_completed::TriggerCompletedEvent>> {
        let client = context.client_from_config();
        let rt = Runtime::new().wrap_err("Failed to create runtime")?;
        rt.block_on(async move {
            let mut stream = client
                .listen_for_events_async([filter])
                .await
                .wrap_err("Failed to listen for trigger completion events")?;
            let deadline = timeout.map(|duration| tokio::time::Instant::now() + duration);
            let mut events = Vec::new();
            loop {
                if limit.is_some_and(|limit| events.len() as u64 >= limit) {
                    break;
                }
                let next = if let Some(deadline) = deadline {
                    if tokio::time::Instant::now() >= deadline {
                        break;
                    }
                    match tokio::time::timeout_at(deadline, stream.try_next()).await {
                        Ok(result) => result?,
                        Err(_) => break,
                    }
                } else {
                    stream.try_next().await?
                };
                let Some(event) = next else {
                    break;
                };
                if let iroha::data_model::events::EventBox::TriggerCompleted(event) = event {
                    events.push(event);
                }
            }
            Ok::<_, eyre::Report>(events)
        })
    }
    #[derive(clap::Args, Debug)]
    pub struct Register {
        /// Trigger name
        #[arg(short, long)]
        pub id: TriggerId,
        /// Path to the compiled IVM bytecode to execute
        #[arg(short, long, value_name("PATH"))]
        pub path: Option<PathBuf>,
        /// Read JSON array of instructions from stdin instead of bytecode path
        /// Example: echo "[ {\"Log\": {\"level\": \"INFO\", \"message\": \"hi\"}} ]" | iroha trigger register -i `my_trig` --instructions-stdin
        #[arg(long)]
        pub instructions_stdin: bool,
        /// Read JSON array of instructions from a file instead of bytecode path
        #[arg(long, value_name = "PATH")]
        pub instructions: Option<PathBuf>,
        /// Number of permitted executions (default: indefinitely)
        #[arg(short, long)]
        pub repeats: Option<u32>,
        /// Account executing the trigger (canonical I105 literal)
        #[arg(long)]
        pub authority: Option<String>,
        /// Filter type for the trigger
        #[arg(long, value_enum, default_value_t = FilterType::Execute)]
        pub filter: FilterType,
        /// Start time in milliseconds since UNIX epoch for time filter
        #[arg(long)]
        pub time_start_ms: Option<u64>,
        /// Period in milliseconds for time filter (optional)
        #[arg(long)]
        pub time_period_ms: Option<u64>,
        /// JSON for a `DataEventFilter` to use as filter
        #[arg(long, value_name = "JSON")]
        pub data_filter: Option<String>,
        /// Data filter preset: events within a domain
        #[arg(long, value_parser = parse_domain_id_literal)]
        pub data_domain: Option<DomainId>,
        /// Data filter preset: events for an account (canonical I105 literal)
        #[arg(long)]
        pub data_account: Option<String>,
        /// Data filter preset: events for a specific asset definition; use with
        /// `--data-asset-account` for a concrete ownership bucket.
        #[arg(
            long,
            requires = "data_asset_account",
            conflicts_with = "data_asset_definition"
        )]
        pub data_asset: Option<AssetDefinitionId>,
        /// Data filter preset: account owning the selected asset bucket (canonical I105 literal).
        #[arg(long, requires = "data_asset")]
        pub data_asset_account: Option<String>,
        /// Data filter preset: balance scope for the selected asset bucket (`global` or
        /// `dataspace:<id>`).
        #[arg(long, value_parser = parse_asset_balance_scope_literal, requires_all = ["data_asset", "data_asset_account"])]
        pub data_asset_scope: Option<iroha::data_model::asset::AssetBalanceScope>,
        /// Data filter preset: events for an asset definition
        #[arg(long, conflicts_with_all = ["data_asset", "data_asset_account", "data_asset_scope"])]
        pub data_asset_definition: Option<AssetDefinitionId>,
        /// Data filter preset: events for a role
        #[arg(long)]
        pub data_role: Option<RoleId>,
        /// Data filter preset: events for a trigger
        #[arg(long)]
        pub data_trigger: Option<TriggerId>,
        /// Data filter preset: events for a verifying key (format: `<backend>:<name>`)
        #[arg(long, value_name = "BACKEND:NAME")]
        pub data_verifying_key: Option<String>,
        /// Data filter preset: events for a proof (format: `<backend>:<64-hex-proof-hash>`)
        #[arg(long, value_name = "BACKEND:HEX")]
        pub data_proof: Option<String>,
        /// Restrict proof events to a preset when using `--data-proof`.
        /// Presets: `verified`, `rejected`, `all` (default).
        #[arg(long, value_name = "PRESET")]
        pub data_proof_only: Option<ProofEventPreset>,
        /// Restrict verifying key events to a preset when using `--data-verifying-key`.
        /// Presets: `registered`, `updated`, `all` (default).
        #[arg(long, value_name = "PRESET")]
        pub data_vk_only: Option<VkEventPreset>,
        /// Human-readable offset for time start (e.g., "5m", "1h"), added to current time
        #[arg(long, value_name = "DURATION")]
        pub time_start: Option<humantime::Duration>,
        /// RFC3339 timestamp for time filter start (e.g., 2025-01-01T00:00:00Z)
        #[arg(long, value_name = "RFC3339")]
        pub time_start_rfc3339: Option<String>,
    }
    #[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FilterType {
        Execute,
        Time,
        Data,
    }
    #[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum VkEventPreset {
        /// All verifying key events (default)
        All,
        /// Only Registered events
        Registered,
        /// Only Updated events
        Updated,
    }
    pub(super) fn parse_data_verifying_key_id(
        spec: &str,
    ) -> Result<iroha::data_model::proof::VerifyingKeyId> {
        let (backend, name) = spec
            .split_once(':')
            .ok_or_else(|| eyre!("--data-verifying-key requires BACKEND:NAME format"))?;
        if backend.is_empty() {
            eyre::bail!("--data-verifying-key backend must be non-empty");
        }
        if !iroha_core::zk::is_verifier_backend_registry_label_v1(backend) {
            eyre::bail!(
                "--data-verifying-key backend uses unsupported verifier-registry label `{backend}`"
            );
        }
        let normalized_name = name.trim();
        if normalized_name.is_empty() {
            eyre::bail!("--data-verifying-key name must be non-empty");
        }
        if normalized_name.contains(':') {
            eyre::bail!("--data-verifying-key name must not contain ':'");
        }
        Ok(iroha::data_model::proof::VerifyingKeyId::new(
            backend.to_string(),
            normalized_name.to_string(),
        ))
    }
    pub(super) fn parse_data_proof_id(spec: &str) -> Result<iroha::data_model::proof::ProofId> {
        let (backend, hash_hex) = spec
            .split_once(':')
            .ok_or_else(|| eyre!("--data-proof requires BACKEND:HEX format"))?;
        if backend.is_empty() {
            eyre::bail!("--data-proof backend must be non-empty");
        }
        if !iroha_core::zk::is_verifier_backend_registry_label_v1(backend) {
            eyre::bail!(
                "--data-proof backend uses unsupported verifier-registry label `{backend}`"
            );
        }
        let hash_hex = hash_hex.trim();
        let hash_hex = hash_hex.strip_prefix("0x").unwrap_or(hash_hex);
        if hash_hex.len() != 64 {
            eyre::bail!("--data-proof hash must be 32 bytes (64 hex chars)");
        }
        let mut proof_hash = [0u8; 32];
        for (index, chunk) in hash_hex.as_bytes().chunks_exact(2).enumerate() {
            let byte_text =
                std::str::from_utf8(chunk).map_err(|e| eyre!("invalid hex for proof hash: {e}"))?;
            proof_hash[index] = u8::from_str_radix(byte_text, 16)
                .map_err(|e| eyre!("invalid hex for proof hash: {e}"))?;
        }
        Ok(iroha::data_model::proof::ProofId {
            backend: backend.to_string(),
            proof_hash,
        })
    }
    #[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProofEventPreset {
        /// All proof events (default)
        All,
        /// Only Verified events
        Verified,
        /// Only Rejected events
        Rejected,
    }
    #[allow(clippy::too_many_lines)]
    impl Run for Register {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use iroha::data_model::{
                events::{
                    EventFilterBox,
                    data::DataEventFilter,
                    execute_trigger::ExecuteTriggerEventFilter,
                    time::{ExecutionTime, Schedule, TimeEventFilter},
                },
                trigger::action::Repeats,
            };
            // Choose executable: either bytecode from path or instructions from stdin
            let executable = match (
                self.path.as_ref(),
                self.instructions.as_ref(),
                self.instructions_stdin,
            ) {
                (Some(path), None, false) => {
                    let bc = read_cli_file_bounded(path, "trigger IVM bytecode")
                        .map(IvmBytecode::from_compiled)
                        .wrap_err("Failed to read IVM bytecode from the file")?;
                    Executable::from(bc)
                }
                (None, Some(file), false) => {
                    let s = read_cli_text_file_bounded(file, "trigger instruction JSON")
                        .wrap_err("Failed to read instructions file")?;
                    let instrs: Vec<InstructionBox> = crate::parse_json(&s)
                        .wrap_err("Failed to parse JSON instructions from file")?;
                    Executable::from(instrs)
                }
                (None, None, true) => {
                    let instrs: Vec<InstructionBox> = parse_json_stdin(context)?;
                    Executable::from(instrs)
                }
                _ => eyre::bail!(
                    "Provide exactly one of: --path, --instructions, or --instructions-stdin"
                ),
            };
            // Resolve authority and repeat policy
            let authority = match self.authority {
                Some(literal) => resolve_account_id(context, &literal)
                    .wrap_err("failed to resolve --authority")?,
                None => context.config().account.clone(),
            };
            // Build filter according to selection
            let filter_box: EventFilterBox = match self.filter {
                FilterType::Execute => EventFilterBox::ExecuteTrigger(
                    ExecuteTriggerEventFilter::new()
                        .for_trigger(self.id.clone())
                        .under_authority(authority.clone()),
                ),
                FilterType::Time => {
                    // Resolve start time: prefer explicit ms, else human duration from now
                    let start_ms = if let Some(ms) = self.time_start_ms {
                        ms
                    } else if let Some(ts) = &self.time_start_rfc3339 {
                        let st = humantime::parse_rfc3339(ts)
                            .map_err(|e| eyre!("Failed to parse RFC3339: {e}"))?;
                        let millis = st
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis();
                        u64::try_from(millis).unwrap_or(u64::MAX)
                    } else if let Some(human) = self.time_start {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap();
                        let millis = (now + Duration::from(human)).as_millis();
                        u64::try_from(millis).unwrap_or(u64::MAX)
                    } else {
                        eyre::bail!("--time-start-ms or --time-start is required for --filter time")
                    };
                    let mut schedule = Schedule::starting_at(Duration::from_millis(start_ms));
                    if let Some(period_ms) = self.time_period_ms {
                        schedule = schedule.with_period(Duration::from_millis(period_ms));
                    }
                    EventFilterBox::Time(TimeEventFilter::new(ExecutionTime::Schedule(schedule)))
                }
                FilterType::Data => {
                    let df: DataEventFilter = if let Some(json) = self.data_filter {
                        crate::parse_json(&json)
                            .wrap_err("Failed to parse --data-filter JSON into DataEventFilter")?
                    } else if let Some(dom) = self.data_domain.clone() {
                        DataEventFilter::from(
                            iroha::data_model::events::data::prelude::DomainEventFilter::new()
                                .for_domain(dom),
                        )
                    } else if let Some(acc_literal) = self.data_account.as_ref() {
                        let account = resolve_account_id(context, acc_literal)
                            .wrap_err("failed to resolve --data-account")?;
                        DataEventFilter::from(
                            iroha::data_model::events::data::prelude::AccountEventFilter::new()
                                .for_account(account),
                        )
                    } else if let Some(definition) = self.data_asset.clone() {
                        let owner = resolve_account_id(
                            context,
                            self.data_asset_account.as_deref().ok_or_else(|| {
                                eyre!("`--data-asset-account` is required with `--data-asset`")
                            })?,
                        )
                        .wrap_err("failed to resolve --data-asset-account")?;
                        let asset = AssetId::with_scope(
                            definition,
                            owner,
                            self.data_asset_scope
                                .clone()
                                .unwrap_or(iroha::data_model::asset::AssetBalanceScope::Global),
                        );
                        DataEventFilter::from(
                            iroha::data_model::events::data::prelude::AssetEventFilter::new()
                                .for_asset(asset),
                        )
                    } else if let Some(def) = self.data_asset_definition.clone() {
                        DataEventFilter::from(
                            iroha::data_model::events::data::prelude::AssetDefinitionEventFilter::new()
                                .for_asset_definition(def),
                        )
                    } else if let Some(role) = self.data_role.clone() {
                        DataEventFilter::from(
                            iroha::data_model::events::data::prelude::RoleEventFilter::new()
                                .for_role(role),
                        )
                    } else if let Some(trg) = self.data_trigger.clone() {
                        DataEventFilter::from(
                            iroha::data_model::events::data::prelude::TriggerEventFilter::new()
                                .for_trigger(trg),
                        )
                    } else if let Some(vk_spec) = self.data_verifying_key.as_ref() {
                        let id = parse_data_verifying_key_id(vk_spec)?;
                        // Map preset to an event set (default: all)
                        let event_set = match self.data_vk_only.unwrap_or(VkEventPreset::All) {
                            VkEventPreset::Registered => iroha::data_model::events::data::verifying_keys::VerifyingKeyEventSet::only_registered(),
                            VkEventPreset::Updated => iroha::data_model::events::data::verifying_keys::VerifyingKeyEventSet::only_updated(),
                            VkEventPreset::All => iroha::data_model::events::data::verifying_keys::VerifyingKeyEventSet::all(),
                        };
                        DataEventFilter::from(
                            iroha::data_model::events::data::prelude::VerifyingKeyEventFilter::new(
                            )
                            .for_verifying_key(id)
                            .for_events(event_set),
                        )
                    } else if let Some(pf_spec) = self.data_proof.as_ref() {
                        let pid = parse_data_proof_id(pf_spec)?;
                        let event_set = match self.data_proof_only.unwrap_or(ProofEventPreset::All)
                        {
                            ProofEventPreset::Verified => {
                                iroha::data_model::events::data::proof::ProofEventSet::only_verified(
                                )
                            }
                            ProofEventPreset::Rejected => {
                                iroha::data_model::events::data::proof::ProofEventSet::only_rejected(
                                )
                            }
                            ProofEventPreset::All => {
                                iroha::data_model::events::data::proof::ProofEventSet::all()
                            }
                        };
                        DataEventFilter::from(
                            iroha::data_model::events::data::prelude::ProofEventFilter::new()
                                .for_proof(pid)
                                .for_events(event_set),
                        )
                    } else {
                        eyre::bail!(
                            "For --filter data, provide one of: --data-filter, --data-domain, --data-account, --data-asset, --data-asset-definition, --data-role, --data-trigger, --data-verifying-key"
                        )
                    };
                    EventFilterBox::Data(df)
                }
            };
            // Choose repeats; ensure one-shot time triggers use Exactly(1)
            let repeats = if matches!(self.filter, FilterType::Time)
                && self.time_period_ms.is_none()
                && self.repeats.is_none()
            {
                Repeats::Exactly(1)
            } else {
                self.repeats.map_or(Repeats::Indefinitely, Repeats::from)
            };
            if matches!(self.filter, FilterType::Time)
                && self.time_period_ms.is_none()
                && let Some(n) = self.repeats
                && n != 1
            {
                eyre::bail!("Non-periodic time filter requires --repeats=1 (got {n})");
            }
            let action = iroha::data_model::trigger::action::Action::new(
                executable, repeats, authority, filter_box,
            )?;
            let trigger = iroha::data_model::trigger::Trigger::new(self.id, action);
            let instruction = iroha::data_model::isi::Register::trigger(trigger);
            context.finish([instruction])
        }
    }
    fn trigger_pretty_json(
        trigger: &iroha::data_model::trigger::Trigger,
    ) -> Result<norito::json::Value> {
        use norito::json::{self, Value};
        fn to_value<T: JsonSerialize + ?Sized>(value: &T) -> Result<Value> {
            json::to_value(value).map_err(|err| eyre!("Failed to encode JSON value: {err}"))
        }
        let mut map = BTreeMap::<String, Value>::new();
        map.insert("id".into(), to_value(trigger.id())?);
        map.insert("authority".into(), to_value(trigger.action().authority())?);
        map.insert("repeats".into(), to_value(&trigger.action().repeats())?);
        map.insert("filter".into(), to_value(trigger.action().filter())?);
        map.insert("metadata".into(), to_value(trigger.action().metadata())?);
        let executable_value = match trigger.action().executable() {
            Executable::Instructions(instrs) => to_value(instrs)?,
            Executable::ContractCall(invocation) => {
                let mut outer = BTreeMap::<String, Value>::new();
                outer.insert("ContractCall".into(), to_value(invocation)?);
                Value::Object(outer)
            }
            Executable::Ivm(bytecode) => {
                let mut inner = BTreeMap::<String, Value>::new();
                inner.insert("hash".into(), to_value(&HashOf::new(bytecode))?);
                inner.insert("size_bytes".into(), to_value(&bytecode.size_bytes())?);
                let mut outer = BTreeMap::<String, Value>::new();
                outer.insert("Ivm".into(), Value::Object(inner));
                Value::Object(outer)
            }
            Executable::IvmProved(proved) => {
                let mut inner = BTreeMap::<String, Value>::new();
                inner.insert("hash".into(), to_value(&HashOf::new(&proved.bytecode))?);
                inner.insert(
                    "size_bytes".into(),
                    to_value(&proved.bytecode.size_bytes())?,
                );
                inner.insert("overlay_len".into(), to_value(&proved.overlay.len())?);
                let mut outer = BTreeMap::<String, Value>::new();
                outer.insert("IvmProved".into(), Value::Object(inner));
                Value::Object(outer)
            }
            Executable::Batch(items) => {
                let mut outer = BTreeMap::<String, Value>::new();
                outer.insert("Batch".into(), to_value(items)?);
                Value::Object(outer)
            }
        };
        map.insert("executable".into(), executable_value);
        Ok(Value::Object(map))
    }
}
mod executor {
    use super::*;
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Retrieve the executor data model
        DataModel,
        /// Upgrade the executor
        Upgrade(Upgrade),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                DataModel => {
                    let client = context.client_from_config();
                    let model = client.query_single(FindExecutorDataModel)?;
                    context.print_data(&model)
                }
                Upgrade(args) => {
                    let instruction = read_cli_file_bounded(&args.path, "executor IVM bytecode")
                        .map(IvmBytecode::from_compiled)
                        .map(Executor::new)
                        .map(iroha::data_model::isi::Upgrade::new)
                        .wrap_err("Failed to read IVM bytecode from the file")?;
                    context.finish_unconfirmed([instruction])
                }
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Upgrade {
        /// Path to the compiled IVM bytecode file
        #[arg(short, long)]
        path: PathBuf,
    }
}
mod metadata {
    use super::*;
    pub mod domain {
        use super::*;
        #[derive(clap::Subcommand, Debug)]
        pub enum Command {
            /// Retrieve a value from the key-value store
            Get(IdKey),
            /// Create or update an entry in the key-value store using JSON input from stdin
            Set(IdKey),
            /// Delete an entry from the key-value store
            Remove(IdKey),
        }
        #[derive(clap::Args, Debug)]
        pub struct IdKey {
            #[arg(short, long, value_parser = parse_domain_id_literal)]
            pub id: DomainId,
            #[arg(short, long)]
            pub key: Name,
        }
        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    Get(args) => {
                        let client = context.client_from_config();
                        let entries: Vec<Domain> = client
                            .query(FindDomains)
                            .execute_all()
                            .wrap_err("Failed to get value")?;
                        let entry = entries
                            .into_iter()
                            .find(|e| e.id() == &args.id)
                            .ok_or_else(|| eyre!("Domain not found"))?;
                        let value = entry
                            .metadata()
                            .get(&args.key)
                            .cloned()
                            .ok_or_else(|| eyre!("Key not found"))?;
                        context.print_data(&value)
                    }
                    Set(args) => {
                        let value: Json = parse_json_stdin(context)?;
                        let instruction =
                            iroha::data_model::isi::SetKeyValue::domain(args.id, args.key, value);
                        context.finish([instruction])
                    }
                    Remove(args) => {
                        let instruction =
                            iroha::data_model::isi::RemoveKeyValue::domain(args.id, args.key);
                        context.finish([instruction])
                    }
                }
            }
        }
    }
    pub mod account {
        use super::*;
        #[derive(clap::Subcommand, Debug)]
        pub enum Command {
            /// Retrieve a value from the key-value store
            Get(IdKey),
            /// Create or update an entry in the key-value store using JSON input from stdin
            Set(IdKey),
            /// Delete an entry from the key-value store
            Remove(IdKey),
        }
        #[derive(clap::Args, Debug)]
        pub struct IdKey {
            #[arg(short, long)]
            pub id: String,
            #[arg(short, long)]
            pub key: Name,
        }
        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    Get(args) => {
                        let account_id = resolve_account_id(context, &args.id)
                            .wrap_err("failed to resolve --id account")?;
                        let client = context.client_from_config();
                        let entry: Account = client
                            .query_single(FindAccountById::new(account_id))
                            .wrap_err("Failed to get value")?;
                        let value = entry
                            .metadata()
                            .get(&args.key)
                            .cloned()
                            .ok_or_else(|| eyre!("Key not found"))?;
                        context.print_data(&value)
                    }
                    Set(args) => {
                        let value: Json = parse_json_stdin(context)?;
                        let instruction = iroha::data_model::isi::SetKeyValue::account(
                            resolve_account_id(context, &args.id)
                                .wrap_err("failed to resolve --id account")?,
                            args.key,
                            value,
                        );
                        context.finish([instruction])
                    }
                    Remove(args) => {
                        let instruction = iroha::data_model::isi::RemoveKeyValue::account(
                            resolve_account_id(context, &args.id)
                                .wrap_err("failed to resolve --id account")?,
                            args.key,
                        );
                        context.finish([instruction])
                    }
                }
            }
        }
    }
    pub mod asset_definition {
        use super::*;
        #[derive(clap::Subcommand, Debug)]
        pub enum Command {
            /// Retrieve a value from the key-value store
            Get(IdKey),
            /// Create or update an entry in the key-value store using JSON input from stdin
            Set(IdKey),
            /// Delete an entry from the key-value store
            Remove(IdKey),
        }
        #[derive(clap::Args, Debug)]
        pub struct IdKey {
            #[arg(short, long)]
            pub id: AssetDefinitionId,
            #[arg(short, long)]
            pub key: Name,
        }
        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    Get(args) => {
                        let client = context.client_from_config();
                        let entries: Vec<AssetDefinition> = client
                            .query(FindAssetsDefinitions)
                            .execute_all()
                            .wrap_err("Failed to get value")?;
                        let entry = entries
                            .into_iter()
                            .find(|e| e.id() == &args.id)
                            .ok_or_else(|| eyre!("Asset definition not found"))?;
                        let value = entry
                            .metadata()
                            .get(&args.key)
                            .cloned()
                            .ok_or_else(|| eyre!("Key not found"))?;
                        context.print_data(&value)
                    }
                    Set(args) => {
                        let value: Json = parse_json_stdin(context)?;
                        let instruction = iroha::data_model::isi::SetKeyValue::asset_definition(
                            args.id, args.key, value,
                        );
                        context.finish([instruction])
                    }
                    Remove(args) => {
                        let instruction = iroha::data_model::isi::RemoveKeyValue::asset_definition(
                            args.id, args.key,
                        );
                        context.finish([instruction])
                    }
                }
            }
        }
    }
    pub mod nft {
        use super::*;
        #[derive(clap::Subcommand, Debug)]
        pub enum Command {
            /// Retrieve a value from the key-value store
            Get(IdKey),
            /// Create or update an entry in the key-value store using JSON input from stdin
            Set(IdKey),
            /// Delete an entry from the key-value store
            Remove(IdKey),
        }
        #[derive(clap::Args, Debug)]
        pub struct IdKey {
            #[arg(short, long)]
            pub id: NftId,
            #[arg(short, long)]
            pub key: Name,
        }
        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    Get(args) => {
                        let client = context.client_from_config();
                        let entries: Vec<Nft> = client
                            .query(FindNfts)
                            .execute_all()
                            .wrap_err("Failed to get value")?;
                        let entry = entries
                            .into_iter()
                            .find(|e| e.id() == &args.id)
                            .ok_or_else(|| eyre!("NFT not found"))?;
                        let value = entry
                            .content()
                            .get(&args.key)
                            .cloned()
                            .ok_or_else(|| eyre!("Key not found"))?;
                        context.print_data(&value)
                    }
                    Set(args) => {
                        let value: Json = parse_json_stdin(context)?;
                        let instruction =
                            iroha::data_model::isi::SetKeyValue::nft(args.id, args.key, value);
                        context.finish([instruction])
                    }
                    Remove(args) => {
                        let instruction =
                            iroha::data_model::isi::RemoveKeyValue::nft(args.id, args.key);
                        context.finish([instruction])
                    }
                }
            }
        }
    }
    pub mod rwa {
        use super::*;
        #[derive(clap::Subcommand, Debug)]
        pub enum Command {
            /// Retrieve a value from the key-value store
            Get(IdKey),
            /// Create or update an entry in the key-value store using JSON input from stdin
            Set(IdKey),
            /// Delete an entry from the key-value store
            Remove(IdKey),
        }
        #[derive(clap::Args, Debug)]
        pub struct IdKey {
            #[arg(short, long)]
            pub id: RwaId,
            #[arg(short, long)]
            pub key: Name,
        }
        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    Get(args) => {
                        let client = context.client_from_config();
                        let entries: Vec<Rwa> = client
                            .query(FindRwas)
                            .execute_all()
                            .wrap_err("Failed to get value")?;
                        let entry = entries
                            .into_iter()
                            .find(|e| e.id() == &args.id)
                            .ok_or_else(|| eyre!("RWA not found"))?;
                        let value = entry
                            .metadata()
                            .get(&args.key)
                            .cloned()
                            .ok_or_else(|| eyre!("Key not found"))?;
                        context.print_data(&value)
                    }
                    Set(args) => {
                        let value: Json = parse_json_stdin(context)?;
                        let instruction =
                            iroha::data_model::isi::SetKeyValue::rwa(args.id, args.key, value);
                        context.finish([instruction])
                    }
                    Remove(args) => {
                        let instruction =
                            iroha::data_model::isi::RemoveKeyValue::rwa(args.id, args.key);
                        context.finish([instruction])
                    }
                }
            }
        }
    }
    pub mod trigger {
        use super::*;
        #[derive(clap::Subcommand, Debug)]
        pub enum Command {
            /// Retrieve a value from the key-value store
            Get(IdKey),
            /// Create or update an entry in the key-value store using JSON input from stdin
            Set(IdKey),
            /// Delete an entry from the key-value store
            Remove(IdKey),
        }
        #[derive(clap::Args, Debug)]
        pub struct IdKey {
            #[arg(short, long)]
            pub id: TriggerId,
            #[arg(short, long)]
            pub key: Name,
        }
        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    Get(args) => {
                        let client = context.client_from_config();
                        let entry: Trigger = client
                            .query_single(FindTriggerById::new(args.id))
                            .wrap_err("Failed to get value")?;
                        let value = entry
                            .metadata()
                            .get(&args.key)
                            .cloned()
                            .ok_or_else(|| eyre!("Key not found"))?;
                        context.print_data(&value)
                    }
                    Set(args) => {
                        let value: Json = parse_json_stdin(context)?;
                        let instruction =
                            iroha::data_model::isi::SetKeyValue::trigger(args.id, args.key, value);
                        context.finish([instruction])
                    }
                    Remove(args) => {
                        let instruction =
                            iroha::data_model::isi::RemoveKeyValue::trigger(args.id, args.key);
                        context.finish([instruction])
                    }
                }
            }
        }
    }
}
mod repo {
    use super::*;
    use iroha::data_model::{
        isi::{
            InstructionBox,
            repo::{RepoInstructionBox, RepoIsi, RepoMarginCallIsi, ReverseRepoIsi},
        },
        prelude::AssetDefinitionId,
        query::repo::prelude::FindRepoAgreements,
        repo::prelude::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
    };
    use iroha_data_model::metadata::Metadata;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Initiate or roll a repo agreement between two counterparties
        Initiate(Initiate),
        /// Unwind an active repo agreement (reverse repo leg)
        Unwind(Unwind),
        /// Inspect repo agreements stored on-chain
        #[command(subcommand)]
        Query(QueryCommand),
        /// Compute the next margin checkpoint for an agreement
        Margin(Margin),
        /// Record a margin call for an active repo agreement
        MarginCall(MarginCall),
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Initiate(args) => args.run(context),
                Unwind(args) => args.run(context),
                Query(cmd) => cmd.run(context),
                Margin(args) => args.run(context),
                MarginCall(args) => args.run(context),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Initiate {
        /// Stable identifier assigned to the repo agreement lifecycle
        #[arg(long)]
        pub agreement_id: RepoAgreementId,
        /// Initiating account submitting the repo
        #[arg(long)]
        pub initiator: String,
        /// Counterparty receiving the repo cash leg
        #[arg(long)]
        pub counterparty: String,
        /// Optional custodian account holding pledged collateral in tri-party agreements
        #[arg(long)]
        pub custodian: Option<String>,
        /// Cash asset definition identifier
        #[arg(long)]
        pub cash_asset: AssetDefinitionId,
        /// Cash quantity exchanged at initiation (integer or decimal)
        #[arg(long)]
        pub cash_quantity: iroha_primitives::numeric::Quantity,
        /// Collateral asset definition identifier
        #[arg(long)]
        pub collateral_asset: AssetDefinitionId,
        /// Collateral quantity pledged at initiation (integer or decimal)
        #[arg(long)]
        pub collateral_quantity: iroha_primitives::numeric::Quantity,
        /// Fixed interest rate in basis points
        #[arg(long)]
        pub rate_bps: u16,
        /// Unix timestamp (milliseconds) when the repo matures
        #[arg(long)]
        pub maturity_timestamp_ms: u64,
        /// Haircut applied to the collateral leg, in basis points
        #[arg(long)]
        pub haircut_bps: u16,
        /// Cadence between margin checks, in seconds (0 disables margining)
        #[arg(long)]
        pub margin_frequency_secs: u64,
    }
    impl Initiate {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let initiator = resolve_account_id(context, &self.initiator)
                .wrap_err("failed to resolve --initiator account")?;
            let counterparty = resolve_account_id(context, &self.counterparty)
                .wrap_err("failed to resolve --counterparty account")?;
            let custodian = match self.custodian {
                Some(literal) => Some(
                    resolve_account_id(context, &literal)
                        .wrap_err("failed to resolve --custodian account")?,
                ),
                None => None,
            };
            let cash_leg = RepoCashLeg {
                asset_definition_id: self.cash_asset,
                quantity: self.cash_quantity,
            };
            let collateral_leg = RepoCollateralLeg {
                asset_definition_id: self.collateral_asset,
                quantity: self.collateral_quantity,
                metadata: Metadata::default(),
            };
            let governance =
                RepoGovernance::with_defaults(self.haircut_bps, self.margin_frequency_secs);
            let instruction = RepoIsi::new(
                self.agreement_id,
                initiator,
                counterparty,
                custodian,
                cash_leg,
                collateral_leg,
                self.rate_bps,
                self.maturity_timestamp_ms,
                governance,
            );
            let instruction: RepoInstructionBox = instruction.into();
            context
                .finish([InstructionBox::from(instruction)])
                .wrap_err("Failed to initiate repo agreement")
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Unwind {
        /// Stable identifier to settle at maturity as any recorded participant
        #[arg(long)]
        pub agreement_id: RepoAgreementId,
    }
    impl Unwind {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let instruction = ReverseRepoIsi::new(self.agreement_id);
            let instruction: RepoInstructionBox = instruction.into();
            context
                .finish([InstructionBox::from(instruction)])
                .wrap_err("Failed to settle repo agreement at maturity")
        }
    }
    #[derive(clap::Subcommand, Debug)]
    pub enum QueryCommand {
        /// List all repo agreements recorded on-chain
        List,
        /// Fetch a single repo agreement by identifier
        Get(QueryId),
    }
    impl Run for QueryCommand {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::QueryCommand::*;
            match self {
                List => {
                    let client = context.client_from_config();
                    let agreements = client
                        .query(FindRepoAgreements::new())
                        .execute_all()
                        .wrap_err("Failed to list repo agreements")?;
                    context.print_data(&agreements)
                }
                Get(args) => {
                    let client = context.client_from_config();
                    let agreements = client
                        .query(FindRepoAgreements::new())
                        .execute_all()
                        .wrap_err("Failed to fetch repo agreements")?;
                    let Some(entry) = agreements
                        .into_iter()
                        .find(|agreement| agreement.id() == &args.id)
                    else {
                        return Err(eyre!("Repo agreement `{}` not found", args.id));
                    };
                    context.print_data(&entry)
                }
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct QueryId {
        /// Stable identifier assigned to the repo agreement lifecycle
        #[arg(long)]
        pub id: RepoAgreementId,
    }
    #[derive(clap::Args, Debug)]
    pub struct Margin {
        /// Stable identifier assigned to the repo agreement lifecycle
        #[arg(long)]
        pub agreement_id: RepoAgreementId,
        /// Timestamp (ms) used when evaluating margin schedule (defaults to current time)
        #[arg(long)]
        pub at_timestamp_ms: Option<u64>,
    }
    impl Margin {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let agreements = client
                .query(FindRepoAgreements::new())
                .execute_all()
                .wrap_err("Failed to fetch repo agreements")?;
            let Some(agreement) = agreements
                .into_iter()
                .find(|agreement| agreement.id() == &self.agreement_id)
            else {
                return Err(eyre!("Repo agreement `{}` not found", self.agreement_id));
            };
            let timestamp_ms = self
                .at_timestamp_ms
                .unwrap_or_else(current_unix_timestamp_ms);
            let next_due = agreement.next_margin_check_after(timestamp_ms);
            let is_due = agreement.is_margin_check_due(timestamp_ms);
            let result = json_utils::json_object(vec![
                (
                    "agreement_id",
                    json_utils::json_value(&agreement.id().to_string())?,
                ),
                ("input_timestamp_ms", json_utils::json_value(&timestamp_ms)?),
                ("next_margin_check_ms", json_utils::json_value(&next_due)?),
                ("is_due", json_utils::json_value(&is_due)?),
                (
                    "margin_frequency_secs",
                    json_utils::json_value(&agreement.governance().margin_frequency_secs())?,
                ),
                (
                    "initiated_timestamp_ms",
                    json_utils::json_value(&agreement.initiated_timestamp_ms())?,
                ),
            ])?;
            context.print_data(&result)
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct MarginCall {
        /// Stable identifier assigned to the repo agreement lifecycle
        #[arg(long)]
        pub agreement_id: RepoAgreementId,
    }
    impl MarginCall {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let instruction = RepoMarginCallIsi::new(self.agreement_id);
            let instruction: RepoInstructionBox = instruction.into();
            context
                .finish([InstructionBox::from(instruction)])
                .wrap_err("Failed to record repo margin call")
        }
    }
    fn current_unix_timestamp_ms() -> u64 {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        u64::try_from(millis).unwrap_or(u64::MAX)
    }
}
mod settlement {
    use std::collections::BTreeSet;
    use super::*;
    use clap::ValueEnum;
    use iroha::data_model::{
        domain::DomainId,
        isi::{
            InstructionBox,
            settlement::{
                DvpIsi, FundFxCorridorEscrow, FxCorridorOracleEvidence, FxCorridorPolicy, PvpIsi,
                RefundFxCorridorEscrow, SetFxCorridorPolicy, SettleFxCorridor, SettlementAtomicity,
                SettlementExecutionOrder, SettlementId, SettlementInstructionBox, SettlementLeg,
                SettlementPlan,
            },
        },
        metadata::Metadata,
        nexus::DataSpaceId,
        oracle::{FeedConfigVersion, FeedEvent, FeedId},
        prelude::{AssetDefinitionId, Name},
        query::settlement::prelude::{FindFxCorridorPolicyById, FindFxCorridorPolicyRegistry},
    };
    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Create a delivery-versus-payment instruction
        Dvp(DvpArgs),
        /// Create a payment-versus-payment instruction
        Pvp(PvpArgs),
        /// Register or replace a governed native FX corridor policy
        SetFxCorridorPolicy(SetFxCorridorPolicyArgs),
        /// Fund a corridor's isolated reserve from its immutable owner
        FundFxCorridorEscrow(FxCorridorEscrowArgs),
        /// Refund an inactive corridor reserve to its immutable owner
        RefundFxCorridorEscrow(FxCorridorEscrowArgs),
        /// Execute one policy-backed native FX corridor settlement
        SettleFxCorridor(SettleFxCorridorArgs),
        /// Read one governed native FX corridor policy
        GetFxCorridorPolicy(GetFxCorridorPolicyArgs),
        /// Read the complete governed native FX corridor policy registry
        ListFxCorridorPolicies,
    }
    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            match self {
                Command::Dvp(args) => args.run(context),
                Command::Pvp(args) => args.run(context),
                Command::SetFxCorridorPolicy(args) => args.run(context),
                Command::FundFxCorridorEscrow(args) => args.run_fund(context),
                Command::RefundFxCorridorEscrow(args) => args.run_refund(context),
                Command::SettleFxCorridor(args) => args.run(context),
                Command::GetFxCorridorPolicy(args) => args.run(context),
                Command::ListFxCorridorPolicies => {
                    let registry = context
                        .client_from_config()
                        .query_single(FindFxCorridorPolicyRegistry)?;
                    context.print_data(&registry)
                }
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct SetFxCorridorPolicyArgs {
        /// Stable policy identifier
        #[arg(long)]
        pub policy_id: Name,
        /// Monotonic policy revision (first revision is 1)
        #[arg(long)]
        pub revision: u64,
        /// Immutable owner that funds reserve liquidity and receives source currency
        #[arg(long)]
        pub owner: String,
        /// Private dataspace holding the source balance
        #[arg(long)]
        pub source_dataspace: DataSpaceId,
        /// Source-currency asset definition
        #[arg(long)]
        pub source_asset: AssetDefinitionId,
        /// Private dataspace holding the destination reserve
        #[arg(long)]
        pub destination_dataspace: DataSpaceId,
        /// Destination-currency asset definition
        #[arg(long)]
        pub destination_asset: AssetDefinitionId,
        /// Allowed destination account-alias domain (repeat for each FI domain).
        #[arg(
            long = "allowed-destination-alias-domain",
            required = true,
            value_parser = parse_domain_id_literal
        )]
        pub allowed_destination_alias_domains: Vec<DomainId>,
        /// Governed oracle feed supplying the destination/source rate
        #[arg(long)]
        pub oracle_feed_id: FeedId,
        /// Maximum accepted oracle-event age in milliseconds
        #[arg(long)]
        pub max_oracle_age_ms: u64,
        /// Maximum source amount per settlement
        #[arg(long)]
        pub max_source_amount_per_settlement: iroha_primitives::numeric::Quantity,
        /// Maximum destination amount per settlement
        #[arg(long)]
        pub max_destination_amount_per_settlement: iroha_primitives::numeric::Quantity,
        /// Fixed velocity-window length in milliseconds
        #[arg(long)]
        pub velocity_window_ms: u64,
        /// Maximum settlements per velocity window
        #[arg(long)]
        pub max_settlements_per_window: u64,
        /// Maximum source amount per velocity window
        #[arg(long)]
        pub max_source_amount_per_window: iroha_primitives::numeric::Quantity,
        /// Maximum destination amount per velocity window
        #[arg(long)]
        pub max_destination_amount_per_window: iroha_primitives::numeric::Quantity,
        /// Register the policy disabled
        #[arg(long)]
        pub disabled: bool,
    }
    #[derive(clap::Args, Debug)]
    pub struct GetFxCorridorPolicyArgs {
        /// Stable policy identifier
        #[arg(long)]
        pub policy_id: Name,
    }
    impl GetFxCorridorPolicyArgs {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let policy = context
                .client_from_config()
                .query_single(FindFxCorridorPolicyById::new(self.policy_id))?;
            context.print_data(&policy)
        }
    }
    impl SetFxCorridorPolicyArgs {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let allowed_domain_count = self.allowed_destination_alias_domains.len();
            let allowed_destination_alias_domains = self
                .allowed_destination_alias_domains
                .into_iter()
                .collect::<BTreeSet<_>>();
            if allowed_destination_alias_domains.len() != allowed_domain_count {
                return Err(eyre!(
                    "--allowed-destination-alias-domain values must be unique"
                ));
            }
            let policy = FxCorridorPolicy {
                policy_id: self.policy_id,
                revision: self.revision,
                owner: resolve_account_id(context, &self.owner)
                    .wrap_err("failed to resolve --owner")?,
                source_dataspace: self.source_dataspace,
                source_asset_definition_id: self.source_asset,
                destination_dataspace: self.destination_dataspace,
                destination_asset_definition_id: self.destination_asset,
                allowed_destination_alias_domains,
                oracle_feed_id: self.oracle_feed_id,
                max_oracle_age_ms: self.max_oracle_age_ms,
                max_source_amount_per_settlement: self.max_source_amount_per_settlement,
                max_destination_amount_per_settlement: self.max_destination_amount_per_settlement,
                velocity_window_ms: self.velocity_window_ms,
                max_settlements_per_window: self.max_settlements_per_window,
                max_source_amount_per_window: self.max_source_amount_per_window,
                max_destination_amount_per_window: self.max_destination_amount_per_window,
                enabled: !self.disabled,
            };
            if let Some(error) = policy.invariant_error() {
                return Err(eyre!(error));
            }
            let instruction: SettlementInstructionBox = SetFxCorridorPolicy { policy }.into();
            context.finish([InstructionBox::from(instruction)])
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct FxCorridorEscrowArgs {
        /// Stable corridor policy identifier
        #[arg(long)]
        pub policy_id: Name,
        /// Exact active policy revision
        #[arg(long)]
        pub expected_policy_revision: u64,
        /// Exact destination asset from the active policy
        #[arg(long)]
        pub destination_asset: AssetDefinitionId,
        /// Positive reserve quantity
        #[arg(long)]
        pub amount: iroha_primitives::numeric::Quantity,
    }
    impl FxCorridorEscrowArgs {
        fn run_fund<C: RunContext>(self, context: &mut C) -> Result<()> {
            let instruction: SettlementInstructionBox = FundFxCorridorEscrow {
                policy_id: self.policy_id,
                expected_policy_revision: self.expected_policy_revision,
                destination_asset_definition_id: self.destination_asset,
                amount: self.amount,
            }
            .into();
            context.finish([InstructionBox::from(instruction)])
        }
        fn run_refund<C: RunContext>(self, context: &mut C) -> Result<()> {
            let instruction: SettlementInstructionBox = RefundFxCorridorEscrow {
                policy_id: self.policy_id,
                expected_policy_revision: self.expected_policy_revision,
                destination_asset_definition_id: self.destination_asset,
                amount: self.amount,
            }
            .into();
            context.finish([InstructionBox::from(instruction)])
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct SettleFxCorridorArgs {
        /// Stable corridor policy identifier
        #[arg(long)]
        pub policy_id: Name,
        /// Exact active policy revision expected by the signer
        #[arg(long)]
        pub expected_policy_revision: u64,
        /// Expected source asset from the referenced policy
        #[arg(long)]
        pub source_asset: AssetDefinitionId,
        /// Expected destination asset from the referenced policy
        #[arg(long)]
        pub destination_asset: AssetDefinitionId,
        /// Globally unique settlement/replay identifier
        #[arg(long)]
        pub settlement_id: SettlementId,
        /// Destination-currency recipient account or alias
        #[arg(long)]
        pub recipient: String,
        /// Positive source-currency quantity
        #[arg(long)]
        pub source_amount: iroha_primitives::numeric::Quantity,
        /// Exact destination amount expected from the selected oracle event
        #[arg(long)]
        pub expected_destination_amount: iroha_primitives::numeric::Quantity,
        /// Exact oracle feed identifier
        #[arg(long)]
        pub oracle_feed_id: FeedId,
        /// Exact active oracle feed configuration version
        #[arg(long)]
        pub oracle_feed_config_version: u32,
        /// Exact oracle slot
        #[arg(long)]
        pub oracle_slot: u64,
        /// Exact oracle request hash
        #[arg(long)]
        pub oracle_request_hash: Hash,
        /// Typed hash of the complete retained oracle event
        #[arg(long)]
        pub oracle_event_hash: HashOf<FeedEvent>,
    }
    impl SettleFxCorridorArgs {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            if self.expected_policy_revision == 0 {
                return Err(eyre!(
                    "--expected-policy-revision must be greater than zero"
                ));
            }
            if self.source_amount.is_zero() {
                return Err(eyre!("--source-amount must be positive"));
            }
            let instruction = SettleFxCorridor {
                policy_id: self.policy_id,
                expected_policy_revision: self.expected_policy_revision,
                source_asset_definition_id: self.source_asset,
                destination_asset_definition_id: self.destination_asset,
                settlement_id: self.settlement_id,
                recipient: resolve_account_id(context, &self.recipient)
                    .wrap_err("failed to resolve --recipient")?,
                source_amount: self.source_amount,
                expected_destination_amount: self.expected_destination_amount,
                oracle_evidence: FxCorridorOracleEvidence {
                    feed_id: self.oracle_feed_id,
                    feed_config_version: FeedConfigVersion(self.oracle_feed_config_version),
                    slot: self.oracle_slot,
                    request_hash: self.oracle_request_hash,
                    event_hash: self.oracle_event_hash,
                },
            };
            let instruction: SettlementInstructionBox = instruction.into();
            context.finish([InstructionBox::from(instruction)])
        }
    }
    #[derive(ValueEnum, Clone, Copy, Debug)]
    #[value(rename_all = "kebab_case")]
    pub enum OrderArg {
        DeliveryThenPayment,
        PaymentThenDelivery,
    }
    impl From<OrderArg> for SettlementExecutionOrder {
        fn from(value: OrderArg) -> Self {
            match value {
                OrderArg::DeliveryThenPayment => SettlementExecutionOrder::DeliveryThenPayment,
                OrderArg::PaymentThenDelivery => SettlementExecutionOrder::PaymentThenDelivery,
            }
        }
    }
    #[derive(ValueEnum, Clone, Copy, Debug)]
    #[value(rename_all = "kebab_case")]
    pub enum AtomicityArg {
        AllOrNothing,
        CommitFirstLeg,
        CommitSecondLeg,
    }
    impl AtomicityArg {
        fn to_model(self) -> SettlementAtomicity {
            match self {
                AtomicityArg::AllOrNothing => SettlementAtomicity::AllOrNothing,
                AtomicityArg::CommitFirstLeg => SettlementAtomicity::CommitFirstLeg,
                AtomicityArg::CommitSecondLeg => SettlementAtomicity::CommitSecondLeg,
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct DvpArgs {
        /// Stable identifier shared across the settlement lifecycle
        #[arg(long)]
        pub settlement_id: SettlementId,
        /// Asset definition delivered in exchange
        #[arg(long)]
        pub delivery_asset: AssetDefinitionId,
        /// Quantity delivered (integer or decimal)
        #[arg(long)]
        pub delivery_quantity: iroha_primitives::numeric::Quantity,
        /// Account delivering the asset
        #[arg(long)]
        pub delivery_from: String,
        /// Account receiving the delivery leg
        #[arg(long)]
        pub delivery_to: String,
        /// Regulated identifier (ISIN or CUSIP) for the delivery instrument when producing ISO previews
        #[arg(long)]
        pub delivery_instrument_id: Option<String>,
        /// Optional path to an ISIN↔CUSIP crosswalk used to validate `--delivery-instrument-id`
        #[arg(long = "iso-reference-crosswalk")]
        pub iso_reference_crosswalk: Option<std::path::PathBuf>,
        /// Payment asset definition completing the settlement
        #[arg(long)]
        pub payment_asset: AssetDefinitionId,
        /// Payment quantity (integer or decimal)
        #[arg(long)]
        pub payment_quantity: iroha_primitives::numeric::Quantity,
        /// Account sending the payment leg
        #[arg(long)]
        pub payment_from: String,
        /// Account receiving the payment leg
        #[arg(long)]
        pub payment_to: String,
        /// Execution order for the two legs
        #[arg(long, value_enum, default_value = "delivery-then-payment")]
        pub order: OrderArg,
        /// Atomicity policy for partial failures (currently only all-or-nothing)
        #[arg(long, value_enum, default_value = "all-or-nothing")]
        pub atomicity: AtomicityArg,
        /// Optional MIC to emit under PlcOfSttlm/MktId
        #[arg(long)]
        pub place_of_settlement_mic: Option<String>,
        /// Settlement partial indicator for SttlmParams/PrtlSttlmInd (NPAR/PART/PARQ/PARC)
        #[arg(long, value_enum, default_value = "npar")]
        pub partial_indicator: iso_preview::PartialIndicatorArg,
        /// Whether to set SttlmParams/HldInd=true in the generated ISO preview
        #[arg(long)]
        pub hold_indicator: bool,
        /// Optional settlement condition code for SttlmParams/SttlmTxCond/Cd
        #[arg(long)]
        pub settlement_condition: Option<String>,
        /// Optional settlement linkage (TYPE:REFERENCE, TYPE = WITH|BEFO|AFTE). May be repeated.
        #[arg(long, value_parser = iso_preview::parse_linkage_arg)]
        pub linkage: Vec<iso_preview::LinkageArg>,
        /// Explicit ISO settlement date (YYYY-MM-DD) for deterministic sese.023 previews
        #[arg(long = "iso-settlement-date", value_parser = iso_preview::parse_iso_date_arg)]
        pub iso_settlement_date: Option<String>,
        /// Optional path to emit a sese.023 XML preview of the settlement
        #[arg(long = "iso-xml-out")]
        pub iso_xml_out: Option<std::path::PathBuf>,
    }
    impl DvpArgs {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let delivery_from = resolve_account_id(context, &self.delivery_from)
                .wrap_err("failed to resolve --delivery-from account")?;
            let delivery_to = resolve_account_id(context, &self.delivery_to)
                .wrap_err("failed to resolve --delivery-to account")?;
            let payment_from = resolve_account_id(context, &self.payment_from)
                .wrap_err("failed to resolve --payment-from account")?;
            let payment_to = resolve_account_id(context, &self.payment_to)
                .wrap_err("failed to resolve --payment-to account")?;
            let plan = SettlementPlan::new(self.order.into(), self.atomicity.to_model());
            let delivery_leg = SettlementLeg::new(
                self.delivery_asset,
                self.delivery_quantity,
                delivery_from,
                delivery_to,
            );
            let payment_leg = SettlementLeg::new(
                self.payment_asset,
                self.payment_quantity,
                payment_from,
                payment_to,
            );
            let instruction = DvpIsi {
                settlement_id: self.settlement_id,
                delivery_leg,
                payment_leg,
                plan,
                metadata: Metadata::default(),
            };
            let reference_data =
                iso_preview::load_reference_crosswalk(self.iso_reference_crosswalk.as_deref())?;
            if let Some(path) = &self.iso_xml_out {
                let options = iso_preview::SettlementPreviewOptions {
                    hold_indicator: self.hold_indicator,
                    partial_indicator: self.partial_indicator.clone(),
                    settlement_condition: self.settlement_condition.clone(),
                    place_of_settlement_mic: self.place_of_settlement_mic.clone(),
                    linkages: self.linkage.clone(),
                    settlement_date: self.iso_settlement_date.clone(),
                };
                let xml = iso_preview::dvp_to_sese023(
                    &instruction,
                    self.delivery_instrument_id.as_deref(),
                    reference_data.as_ref(),
                    &options,
                )?;
                iso_preview::write_iso_preview(path, &xml)?;
            }
            let instruction: SettlementInstructionBox = instruction.into();
            context.finish([InstructionBox::from(instruction)])
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct PvpArgs {
        /// Stable identifier shared across the settlement lifecycle
        #[arg(long)]
        pub settlement_id: SettlementId,
        /// Primary currency leg asset definition
        #[arg(long)]
        pub primary_asset: AssetDefinitionId,
        /// Quantity of the primary currency (integer or decimal)
        #[arg(long)]
        pub primary_quantity: iroha_primitives::numeric::Quantity,
        /// Account delivering the primary currency
        #[arg(long)]
        pub primary_from: String,
        /// Account receiving the primary currency
        #[arg(long)]
        pub primary_to: String,
        /// Counter currency leg asset definition
        #[arg(long)]
        pub counter_asset: AssetDefinitionId,
        /// Quantity of the counter currency (integer or decimal)
        #[arg(long)]
        pub counter_quantity: iroha_primitives::numeric::Quantity,
        /// Account delivering the counter currency
        #[arg(long)]
        pub counter_from: String,
        /// Account receiving the counter currency
        #[arg(long)]
        pub counter_to: String,
        /// Execution order for the two legs
        #[arg(long, value_enum, default_value = "delivery-then-payment")]
        pub order: OrderArg,
        /// Atomicity policy for partial failures (currently only all-or-nothing)
        #[arg(long, value_enum, default_value = "all-or-nothing")]
        pub atomicity: AtomicityArg,
        /// Optional MIC to emit under PlcOfSttlm/MktId
        #[arg(long)]
        pub place_of_settlement_mic: Option<String>,
        /// Settlement partial indicator for SttlmParams/PrtlSttlmInd (NPAR/PART/PARQ/PARC)
        #[arg(long, value_enum, default_value = "npar")]
        pub partial_indicator: iso_preview::PartialIndicatorArg,
        /// Whether to set SttlmParams/HldInd=true in the generated ISO preview
        #[arg(long)]
        pub hold_indicator: bool,
        /// Optional settlement condition code for SttlmParams/SttlmTxCond/Cd
        #[arg(long)]
        pub settlement_condition: Option<String>,
        /// Explicit ISO settlement date (YYYY-MM-DD) for deterministic sese.025 previews
        #[arg(long = "iso-settlement-date", value_parser = iso_preview::parse_iso_date_arg)]
        pub iso_settlement_date: Option<String>,
        /// Optional path to emit a sese.025 XML preview of the settlement
        #[arg(long = "iso-xml-out")]
        pub iso_xml_out: Option<std::path::PathBuf>,
    }
    impl PvpArgs {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let primary_from = resolve_account_id(context, &self.primary_from)
                .wrap_err("failed to resolve --primary-from account")?;
            let primary_to = resolve_account_id(context, &self.primary_to)
                .wrap_err("failed to resolve --primary-to account")?;
            let counter_from = resolve_account_id(context, &self.counter_from)
                .wrap_err("failed to resolve --counter-from account")?;
            let counter_to = resolve_account_id(context, &self.counter_to)
                .wrap_err("failed to resolve --counter-to account")?;
            let plan = SettlementPlan::new(self.order.into(), self.atomicity.to_model());
            let primary_leg = SettlementLeg::new(
                self.primary_asset,
                self.primary_quantity,
                primary_from,
                primary_to,
            );
            let counter_leg = SettlementLeg::new(
                self.counter_asset,
                self.counter_quantity,
                counter_from,
                counter_to,
            );
            let instruction = PvpIsi {
                settlement_id: self.settlement_id,
                primary_leg,
                counter_leg,
                plan,
                metadata: Metadata::default(),
            };
            if let Some(path) = &self.iso_xml_out {
                let options = iso_preview::SettlementPreviewOptions {
                    hold_indicator: self.hold_indicator,
                    partial_indicator: self.partial_indicator.clone(),
                    settlement_condition: self.settlement_condition.clone(),
                    place_of_settlement_mic: self.place_of_settlement_mic.clone(),
                    linkages: Vec::new(),
                    settlement_date: self.iso_settlement_date.clone(),
                };
                let xml = iso_preview::pvp_to_sese025(&instruction, &options)?;
                iso_preview::write_iso_preview(path, &xml)?;
            }
            let instruction: SettlementInstructionBox = instruction.into();
            context.finish([InstructionBox::from(instruction)])
        }
    }
    mod iso_preview {
        use super::*;
        use eyre::{Context, eyre};
        use iroha_config::parameters::actual::IsoReferenceData;
        use iroha_core::iso_bridge::reference_data::{
            ReferenceDataError, ReferenceDataSnapshots, SnapshotState,
        };
        use ivm::iso20022::{
            MsgError, msg_add, msg_clear, msg_create, msg_serialize, msg_set, msg_validate,
            take_validation_error,
        };
        use std::{fs, path::Path};
        use time::{OffsetDateTime, format_description::well_known::Iso8601};
        #[derive(clap::ValueEnum, Clone, Debug, Default)]
        pub enum PartialIndicatorArg {
            #[default]
            Npar,
            Part,
            Parq,
            Parc,
        }
        impl PartialIndicatorArg {
            fn as_iso(&self) -> &'static str {
                match self {
                    PartialIndicatorArg::Npar => "NPAR",
                    PartialIndicatorArg::Part => "PART",
                    PartialIndicatorArg::Parq => "PARQ",
                    PartialIndicatorArg::Parc => "PARC",
                }
            }
        }
        #[derive(Clone, Debug)]
        pub struct LinkageArg {
            pub relation: String,
            pub reference: String,
        }
        #[derive(Clone, Debug)]
        pub struct SettlementPreviewOptions {
            pub hold_indicator: bool,
            pub partial_indicator: PartialIndicatorArg,
            pub settlement_condition: Option<String>,
            pub place_of_settlement_mic: Option<String>,
            pub linkages: Vec<LinkageArg>,
            pub settlement_date: Option<String>,
        }
        impl Default for SettlementPreviewOptions {
            fn default() -> Self {
                Self {
                    hold_indicator: false,
                    partial_indicator: PartialIndicatorArg::Npar,
                    settlement_condition: None,
                    place_of_settlement_mic: None,
                    linkages: Vec::new(),
                    settlement_date: None,
                }
            }
        }
        pub fn parse_iso_date_arg(input: &str) -> Result<String, String> {
            let trimmed = input.trim();
            let bytes = trimmed.as_bytes();
            let valid = bytes.len() == 10
                && bytes[4] == b'-'
                && bytes[7] == b'-'
                && bytes
                    .iter()
                    .enumerate()
                    .all(|(idx, byte)| idx == 4 || idx == 7 || byte.is_ascii_digit());
            if valid {
                Ok(trimmed.to_owned())
            } else {
                Err("ISO settlement date must use YYYY-MM-DD".to_owned())
            }
        }
        pub fn parse_linkage_arg(input: &str) -> Result<LinkageArg, String> {
            let (relation, reference) = input
                .split_once(':')
                .ok_or_else(|| "expected TYPE:REFERENCE (TYPE = WITH|BEFO|AFTE)".to_owned())?;
            let upper = relation.to_ascii_uppercase();
            match upper.as_str() {
                "WITH" | "BEFO" | "AFTE" => Ok(LinkageArg {
                    relation: upper,
                    reference: reference.to_owned(),
                }),
                _ => Err("linkage TYPE must be WITH, BEFO, or AFTE".to_owned()),
            }
        }
        pub fn write_iso_preview(path: &Path, xml: &str) -> Result<()> {
            fs::write(path, xml).context("failed to write ISO 20022 preview")?;
            Ok(())
        }
        fn validate_instrument_id(id: &str) -> Result<()> {
            if ivm::iso20022::validate_instrument_identifier(id) {
                Ok(())
            } else {
                Err(eyre!(
                    "invalid delivery instrument identifier `{id}` (expect ISIN or CUSIP)"
                ))
            }
        }
        pub fn dvp_to_sese023(
            isi: &DvpIsi,
            instrument_id: Option<&str>,
            reference_data: Option<&ReferenceDataSnapshots>,
            options: &SettlementPreviewOptions,
        ) -> Result<String> {
            iso_scope(|| {
                let fin_instr = instrument_id
                    .ok_or_else(|| eyre!("--delivery-instrument-id is required for ISO preview"))?;
                if let Some(snapshots) = reference_data {
                    match snapshots.validate_isin(fin_instr) {
                        Ok(()) => {}
                        Err(err) => return Err(instrument_reference_error(fin_instr, err)),
                    }
                }
                validate_instrument_id(fin_instr)?;
                msg_create("sese.023");
                msg_set("TxId", isi.settlement_id.to_string().as_bytes());
                msg_set("SttlmTpAndAddtlParams/SctiesMvmntTp", b"DELI");
                msg_set("SttlmTpAndAddtlParams/Pmt", b"APMT");
                msg_set(
                    "SttlmParams/PrtlSttlmInd",
                    options.partial_indicator.as_iso().as_bytes(),
                );
                msg_set("SttlmParams/HldInd", bool_to_bytes(options.hold_indicator));
                if let Some(condition) = &options.settlement_condition {
                    msg_set("SttlmParams/SttlmTxCond/Cd", condition.as_bytes());
                }
                if let Some(mic) = &options.place_of_settlement_mic {
                    msg_set("PlcOfSttlm/MktId", mic.as_bytes());
                }
                for (idx, linkage) in options.linkages.iter().enumerate() {
                    msg_add("Lnkgs/Lnkg");
                    let prefix = format!("Lnkgs/Lnkg[{idx}]");
                    msg_set(
                        format!("{prefix}/Tp/Cd").as_str(),
                        linkage.relation.as_bytes(),
                    );
                    msg_set(
                        format!("{prefix}/Ref/Prtry").as_str(),
                        linkage.reference.as_bytes(),
                    );
                }
                msg_set("SctiesLeg/FinInstrmId", fin_instr.as_bytes());
                msg_set(
                    "SctiesLeg/Qty",
                    isi.delivery_leg.quantity().to_string().as_bytes(),
                );
                msg_set(
                    "CashLeg/Ccy",
                    settlement_currency_code(isi.payment_leg.asset_definition_id()).as_bytes(),
                );
                msg_set(
                    "SttlmDt",
                    settlement_date_string(options.settlement_date.as_deref()).as_bytes(),
                );
                msg_set(
                    "CashLeg/Amt",
                    isi.payment_leg.quantity().to_string().as_bytes(),
                );
                write_party("DlvrgSttlmPties", isi.delivery_leg.from());
                write_party("RcvgSttlmPties", isi.delivery_leg.to());
                msg_set(
                    "Plan/ExecutionOrder",
                    execution_order(isi.plan.order()).as_bytes(),
                );
                msg_set("Plan/Atomicity", atomicity(isi.plan.atomicity()).as_bytes());
                if !msg_validate() {
                    let detail = take_validation_error().map_or_else(
                        || "ISO 20022 validation failed for generated sese.023".to_owned(),
                        |err| format!("ISO 20022 validation failed: {err}"),
                    );
                    return Err(eyre!(detail));
                }
                serialize_xml()
            })
        }
        pub fn load_reference_crosswalk(
            path: Option<&std::path::Path>,
        ) -> Result<Option<ReferenceDataSnapshots>> {
            let Some(path) = path else {
                return Ok(None);
            };
            let config = IsoReferenceData {
                isin_crosswalk_path: Some(path.to_path_buf()),
                ..IsoReferenceData::default()
            };
            let snapshots = ReferenceDataSnapshots::from_config(&config);
            match snapshots.isin_cusip().state() {
                SnapshotState::Loaded => Ok(Some(snapshots)),
                SnapshotState::Missing => Err(eyre!(
                    "ISO reference crosswalk `{}` produced an empty snapshot",
                    path.display()
                )),
                SnapshotState::Failed => {
                    let diagnostics = snapshots
                        .isin_cusip()
                        .diagnostics()
                        .unwrap_or("unknown error");
                    Err(eyre!(
                        "failed to load ISO reference crosswalk `{}`: {diagnostics}",
                        path.display()
                    ))
                }
            }
        }
        fn instrument_reference_error(id: &str, err: ReferenceDataError) -> eyre::Report {
            match err {
                ReferenceDataError::DatasetUnavailable { .. } => {
                    eyre!("cannot validate `{id}` because no ISO reference crosswalk was supplied")
                }
                ReferenceDataError::DatasetFailed { diagnostics, .. } => eyre!(
                    "failed to validate `{id}` against ISO reference crosswalk: {}",
                    diagnostics.unwrap_or_else(|| "unknown error".to_string())
                ),
                ReferenceDataError::NotFound { .. } => eyre!(
                    "`--delivery-instrument-id` `{id}` is not present in the supplied ISO reference crosswalk"
                ),
                ReferenceDataError::MicInactive { .. } => {
                    eyre!("unexpected MIC validation error while checking `{id}`")
                }
                ReferenceDataError::MissingLedgerMapping { value, mapping, .. } => eyre!(
                    "`--delivery-instrument-id` `{id}` is present in the supplied ISO reference crosswalk as `{value}`, but that row lacks required ledger mapping `{mapping}`"
                ),
            }
        }
        fn settlement_date_string(explicit: Option<&str>) -> String {
            explicit.map_or_else(
                || {
                    OffsetDateTime::now_utc()
                        .date()
                        .format(&Iso8601::DATE)
                        .unwrap_or_else(|_| "1970-01-01".to_string())
                },
                ToOwned::to_owned,
            )
        }
        pub fn pvp_to_sese025(isi: &PvpIsi, options: &SettlementPreviewOptions) -> Result<String> {
            iso_scope(|| {
                msg_create("sese.025");
                msg_set("TxId", isi.settlement_id.to_string().as_bytes());
                msg_set("SttlmTpAndAddtlParams/SctiesMvmntTp", b"RECE");
                msg_set("SttlmTpAndAddtlParams/Pmt", b"APMT");
                msg_set(
                    "SttlmDt",
                    settlement_date_string(options.settlement_date.as_deref()).as_bytes(),
                );
                msg_set(
                    "SttlmParams/PrtlSttlmInd",
                    options.partial_indicator.as_iso().as_bytes(),
                );
                msg_set("SttlmParams/HldInd", bool_to_bytes(options.hold_indicator));
                if let Some(condition) = &options.settlement_condition {
                    msg_set("SttlmParams/SttlmTxCond/Cd", condition.as_bytes());
                }
                if let Some(mic) = &options.place_of_settlement_mic {
                    msg_set("PlcOfSttlm/MktId", mic.as_bytes());
                }
                msg_set(
                    "SttlmCcy",
                    settlement_currency_code(isi.primary_leg.asset_definition_id()).as_bytes(),
                );
                msg_set(
                    "SttlmAmt",
                    isi.primary_leg.quantity().to_string().as_bytes(),
                );
                msg_set(
                    "SttlmQty",
                    isi.counter_leg.quantity().to_string().as_bytes(),
                );
                msg_set("ConfSts", b"ACCP");
                msg_set(
                    "Plan/ExecutionOrder",
                    execution_order(isi.plan.order()).as_bytes(),
                );
                msg_set("Plan/Atomicity", atomicity(isi.plan.atomicity()).as_bytes());
                msg_set("AddtlInf", counter_info(isi.counter_leg()).as_bytes());
                if !msg_validate() {
                    let detail = take_validation_error().map_or_else(
                        || "ISO 20022 validation failed for generated sese.025".to_owned(),
                        |err| format!("ISO 20022 validation failed: {err}"),
                    );
                    return Err(eyre!(detail));
                }
                serialize_xml()
            })
        }
        fn serialize_xml() -> Result<String> {
            let xml = msg_serialize("XML").map_err(|err| map_msg_err(&err))?;
            String::from_utf8(xml).wrap_err("ISO 20022 XML is not valid UTF-8")
        }
        fn iso_scope<F, T>(f: F) -> Result<T>
        where
            F: FnOnce() -> Result<T>,
        {
            struct Guard;
            impl Drop for Guard {
                fn drop(&mut self) {
                    msg_clear();
                }
            }
            let _guard = Guard;
            msg_clear();
            f()
        }
        fn bool_to_bytes(value: bool) -> &'static [u8] {
            if value { b"true" } else { b"false" }
        }
        fn write_party(prefix: &str, account: &AccountId) {
            let bic = bic_from_account(account);
            msg_set(format!("{prefix}/Pty/Bic").as_str(), bic.as_bytes());
            msg_set(
                format!("{prefix}/Acct").as_str(),
                account.to_string().as_bytes(),
            );
        }
        fn bic_from_account(account: &AccountId) -> String {
            let country = "XX";
            let mut location: String = account
                .to_string()
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .take(2)
                .map(|c| c.to_ascii_uppercase())
                .collect();
            while location.len() < 2 {
                location.push('0');
            }
            format!("IROA{country}{location}")
        }
        fn settlement_currency_code(_asset: &AssetDefinitionId) -> String {
            // Offline previews only see the canonical asset-definition address. The
            // human currency label belongs to the registered definition, so use a
            // schema-valid placeholder instead of trying to recover it from the id.
            "XXX".to_owned()
        }
        fn counter_info(leg: &SettlementLeg) -> String {
            format!(
                "{{\"counter_currency\":\"{}\",\"amount\":\"{}\"}}",
                settlement_currency_code(leg.asset_definition_id()),
                leg.quantity()
            )
        }
        fn execution_order(order: SettlementExecutionOrder) -> &'static str {
            match order {
                SettlementExecutionOrder::DeliveryThenPayment => "DELIVERY_THEN_PAYMENT",
                SettlementExecutionOrder::PaymentThenDelivery => "PAYMENT_THEN_DELIVERY",
            }
        }
        fn atomicity(atomicity: SettlementAtomicity) -> &'static str {
            match atomicity {
                SettlementAtomicity::AllOrNothing => "ALL_OR_NOTHING",
                SettlementAtomicity::CommitFirstLeg => "COMMIT_FIRST_LEG",
                SettlementAtomicity::CommitSecondLeg => "COMMIT_SECOND_LEG",
            }
        }
        fn map_msg_err(err: &MsgError) -> eyre::Error {
            eyre!("ISO 20022 helper error: {err}")
        }
        #[cfg(test)]
        mod tests {
            use super::*;
            use iroha::crypto::{Algorithm, KeyPair};
            use iroha_core::iso_bridge::reference_data::DatasetKind;
            use iroha_data_model::domain::DomainId;
            use iroha_primitives::numeric::Quantity;
            use std::io::Write;
            use tempfile::NamedTempFile;
            fn fixture_key_pair(seed: u8) -> KeyPair {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("fixture seed must derive a valid keypair")
            }
            fn account_with_seed(domain: &DomainId, seed: u8) -> AccountId {
                let key_pair = fixture_key_pair(seed);
                let _ = domain;
                AccountId::new(key_pair.public_key().clone())
            }
            #[test]
            fn fixture_key_pair_uses_checked_seed_derivation() {
                assert_eq!(fixture_key_pair(0x11).algorithm(), Algorithm::Ed25519);
                assert!(
                    KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
                    "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
                );
            }
            fn sample_dvp() -> DvpIsi {
                let domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
                let seller = account_with_seed(&domain, 0x11);
                let buyer = account_with_seed(&domain, 0x22);
                let payer = account_with_seed(&domain, 0x33);
                let receiver = account_with_seed(&domain, 0x44);
                let delivery_leg = SettlementLeg::new(
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").unwrap(),
                        "bond".parse().unwrap(),
                    ),
                    Quantity::from(100_u32),
                    seller,
                    buyer,
                );
                let payment_leg = SettlementLeg::new(
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").unwrap(),
                        "usd".parse().unwrap(),
                    ),
                    Quantity::from(1_000_u32),
                    payer,
                    receiver,
                );
                let plan = SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                );
                DvpIsi {
                    settlement_id: "dvp".parse().unwrap(),
                    delivery_leg,
                    payment_leg,
                    plan,
                    metadata: Metadata::default(),
                }
            }
            fn sample_pvp() -> PvpIsi {
                let domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
                let payer = account_with_seed(&domain, 0x55);
                let receiver = account_with_seed(&domain, 0x66);
                let counter_payer = account_with_seed(&domain, 0x77);
                let counter_receiver = account_with_seed(&domain, 0x88);
                let primary_leg = SettlementLeg::new(
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").unwrap(),
                        "usd".parse().unwrap(),
                    ),
                    Quantity::from(1_000_u32),
                    payer,
                    receiver,
                );
                let counter_leg = SettlementLeg::new(
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").unwrap(),
                        "eur".parse().unwrap(),
                    ),
                    Quantity::from(900_u32),
                    counter_payer,
                    counter_receiver,
                );
                let plan = SettlementPlan::new(
                    SettlementExecutionOrder::PaymentThenDelivery,
                    SettlementAtomicity::CommitSecondLeg,
                );
                PvpIsi {
                    settlement_id: "pvp".parse().unwrap(),
                    primary_leg,
                    counter_leg,
                    plan,
                    metadata: Metadata::default(),
                }
            }
            fn crosswalk_snapshot(contents: &str) -> ReferenceDataSnapshots {
                let mut file = NamedTempFile::new().expect("snapshot file");
                file.write_all(contents.as_bytes()).expect("write snapshot");
                load_reference_crosswalk(Some(file.path()))
                    .expect("load crosswalk")
                    .expect("snapshot present")
            }
            #[test]
            fn dvp_preview_accepts_instrument_present_in_crosswalk() {
                let snapshots = crosswalk_snapshot(
                    r#"{
                        "version":"2024-05-01",
                        "source":"ANNA",
                        "entries":[{"isin":"US0378331005"}]
                    }"#,
                );
                dvp_to_sese023(
                    &sample_dvp(),
                    Some("US0378331005"),
                    Some(&snapshots),
                    &SettlementPreviewOptions::default(),
                )
                .expect("preview succeeds");
            }
            #[test]
            fn dvp_preview_rejects_unknown_instrument() {
                let snapshots = crosswalk_snapshot(
                    r#"{
                        "version":"2024-05-01",
                        "source":"ANNA",
                        "entries":[{"isin":"US0378331005"}]
                    }"#,
                );
                let err = dvp_to_sese023(
                    &sample_dvp(),
                    Some("US5949181045"),
                    Some(&snapshots),
                    &SettlementPreviewOptions::default(),
                )
                .expect_err("unknown instrument should fail");
                assert!(
                    err.to_string()
                        .contains("not present in the supplied ISO reference crosswalk")
                );
            }
            #[test]
            fn instrument_reference_error_reports_missing_ledger_mapping() {
                let err = instrument_reference_error(
                    "037833100",
                    ReferenceDataError::MissingLedgerMapping {
                        kind: DatasetKind::IsinCusip,
                        value: "US0378331005".to_owned(),
                        mapping: "asset_definition_id_or_asset_id",
                    },
                );
                let msg = err.to_string();
                assert!(msg.contains("`--delivery-instrument-id` `037833100`"));
                assert!(msg.contains("US0378331005"));
                assert!(msg.contains("asset_definition_id_or_asset_id"));
            }
            #[test]
            fn dvp_preview_uses_placeholder_currency_without_definition_context() {
                let domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
                let seller = account_with_seed(&domain, 0x11);
                let buyer = account_with_seed(&domain, 0x22);
                let payer = account_with_seed(&domain, 0x33);
                let receiver = account_with_seed(&domain, 0x44);
                let named_payment_asset =
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").unwrap(),
                        "usd".parse().unwrap(),
                    );
                let canonical_payment_asset: AssetDefinitionId = named_payment_asset
                    .to_string()
                    .parse()
                    .expect("canonical asset id should parse");
                let dvp = DvpIsi {
                    settlement_id: "dvp_settlement".parse().unwrap(),
                    delivery_leg: SettlementLeg::new(
                        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                            DomainId::try_new("wonderland", "universal").unwrap(),
                            "bond".parse().unwrap(),
                        ),
                        Quantity::from(100_u32),
                        seller,
                        buyer,
                    ),
                    payment_leg: SettlementLeg::new(
                        canonical_payment_asset,
                        Quantity::from(1_000_u32),
                        payer,
                        receiver,
                    ),
                    plan: SettlementPlan::new(
                        SettlementExecutionOrder::DeliveryThenPayment,
                        SettlementAtomicity::CommitFirstLeg,
                    ),
                    metadata: Metadata::default(),
                };
                let xml = dvp_to_sese023(
                    &dvp,
                    Some("US0378331005"),
                    None,
                    &SettlementPreviewOptions::default(),
                )
                .expect("preview succeeds for opaque currency asset ids");
                let parsed = ivm::iso20022::parse_message("sese.023", xml.as_bytes())
                    .expect("parse preview");
                assert_eq!(parsed.field_text("CashLeg/Ccy"), Some("XXX"));
                assert_eq!(
                    parsed.field_text("Plan/Atomicity"),
                    Some("COMMIT_FIRST_LEG")
                );
            }
            #[test]
            fn dvp_preview_applies_hold_partial_mic_and_linkages() {
                let options = SettlementPreviewOptions {
                    hold_indicator: true,
                    partial_indicator: PartialIndicatorArg::Parq,
                    settlement_condition: Some("NOMC".to_owned()),
                    place_of_settlement_mic: Some("XNAS".to_owned()),
                    linkages: vec![
                        LinkageArg {
                            relation: "WITH".to_owned(),
                            reference: "SUBST-PAIR-B".to_owned(),
                        },
                        LinkageArg {
                            relation: "BEFO".to_owned(),
                            reference: "PACS009-CLS".to_owned(),
                        },
                    ],
                    settlement_date: Some("2026-02-01".to_owned()),
                };
                let xml = dvp_to_sese023(&sample_dvp(), Some("US0378331005"), None, &options)
                    .expect("preview succeeds");
                let parsed = ivm::iso20022::parse_message("sese.023", xml.as_bytes())
                    .expect("parse preview");
                assert_eq!(parsed.field_text("SttlmParams/HldInd"), Some("true"));
                assert_eq!(parsed.field_text("SttlmParams/PrtlSttlmInd"), Some("PARQ"));
                assert_eq!(
                    parsed.field_text("SttlmParams/SttlmTxCond/Cd"),
                    Some("NOMC")
                );
                assert_eq!(parsed.field_text("PlcOfSttlm/MktId"), Some("XNAS"));
                assert_eq!(parsed.field_text("SttlmDt"), Some("2026-02-01"));
                assert_eq!(parsed.field_text("Lnkgs/Lnkg[0]/Tp/Cd"), Some("WITH"));
                assert_eq!(
                    parsed.field_text("Lnkgs/Lnkg[0]/Ref/Prtry"),
                    Some("SUBST-PAIR-B")
                );
                assert_eq!(parsed.field_text("Lnkgs/Lnkg[1]/Tp/Cd"), Some("BEFO"));
                assert_eq!(
                    parsed.field_text("Lnkgs/Lnkg[1]/Ref/Prtry"),
                    Some("PACS009-CLS")
                );
            }
            #[test]
            fn pvp_preview_applies_partial_and_condition() {
                let options = SettlementPreviewOptions {
                    hold_indicator: true,
                    partial_indicator: PartialIndicatorArg::Parc,
                    settlement_condition: Some("NOMC".to_owned()),
                    place_of_settlement_mic: Some("XLON".to_owned()),
                    linkages: Vec::new(),
                    settlement_date: Some("2026-02-02".to_owned()),
                };
                let xml = pvp_to_sese025(&sample_pvp(), &options).expect("preview succeeds");
                let parsed = ivm::iso20022::parse_message("sese.025", xml.as_bytes())
                    .expect("parse preview");
                assert_eq!(parsed.field_text("SttlmParams/HldInd"), Some("true"));
                assert_eq!(parsed.field_text("SttlmParams/PrtlSttlmInd"), Some("PARC"));
                assert_eq!(
                    parsed.field_text("SttlmParams/SttlmTxCond/Cd"),
                    Some("NOMC")
                );
                assert_eq!(parsed.field_text("SttlmDt"), Some("2026-02-02"));
                assert_eq!(parsed.field_text("PlcOfSttlm/MktId"), Some("XLON"));
            }
        }
    }
}
fn dump_json_stdout<T>(value: &T) -> Result<()>
where
    T: JsonSerialize + ?Sized,
{
    let mut rendered =
        norito::json::to_json_pretty(value).map_err(|err| eyre!("failed to render JSON: {err}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    io::stdout().write_all(rendered.as_bytes())?;
    Ok(())
}
fn parse_json_stdin<T>(context: &impl RunContext) -> Result<T>
where
    T: JsonDeserialize,
{
    if context.input_instructions() {
        eyre::bail!("Incompatible `--input` flag with the command")
    }
    parse_json_stdin_unchecked()
}
fn parse_json_stdin_unchecked<T>() -> Result<T>
where
    T: JsonDeserialize,
{
    parse_json(&string_from_stdin()?)
}
fn parse_json<T>(s: &str) -> Result<T>
where
    T: JsonDeserialize,
{
    norito::json::preflight_slice(
        s.as_bytes(),
        norito::json::JsonPreflightLimits::from_decode_limits(
            MAX_CLI_STDIN_BYTES_V1,
            CLI_JSON_DECODE_LIMITS_V1,
        ),
    )
    .map_err(|error| eyre!("failed to admit JSON input: {error}"))?;
    norito::with_decode_limits_scope(CLI_JSON_DECODE_LIMITS_V1, || norito::json::from_json(s))
        .map_err(|err| eyre!("failed to parse JSON: {err}"))
}
fn resolve_account_id_with(literal: &str) -> Result<AccountId> {
    let trimmed = literal.trim();
    if trimmed.is_empty() {
        eyre::bail!("account literal must not be empty");
    }
    if trimmed.contains('@') {
        eyre::bail!("account literal must not include '@domain'; use canonical I105 only");
    }
    if trimmed
        .get(..2)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("0x"))
    {
        eyre::bail!("account literal must be canonical I105; canonical hex is not accepted");
    }
    let parsed = AccountId::parse_encoded(trimmed)
        .map_err(|err| eyre!("account literal must be canonical I105: {err}"))?;
    Ok(parsed.into_account_id())
}
pub(crate) fn resolve_account_id<C: RunContext>(_context: &C, literal: &str) -> Result<AccountId> {
    resolve_account_id_with(literal)
}
fn parse_asset_balance_scope_literal(
    literal: &str,
) -> Result<iroha::data_model::asset::AssetBalanceScope> {
    let trimmed = literal.trim();
    if trimmed.eq_ignore_ascii_case("global") {
        return Ok(iroha::data_model::asset::AssetBalanceScope::Global);
    }
    if let Some(rest) = trimmed.strip_prefix("dataspace:") {
        let dataspace = rest
            .parse::<u64>()
            .map_err(|_| eyre!("asset balance scope must be `global` or `dataspace:<id>`"))?;
        return Ok(iroha::data_model::asset::AssetBalanceScope::Dataspace(
            iroha::data_model::nexus::DataSpaceId::new(dataspace),
        ));
    }
    Err(eyre!(
        "asset balance scope must be `global` or `dataspace:<id>`"
    ))
}
fn resolve_asset_definition_id_by_alias(
    client: &Client,
    alias: &AssetDefinitionAlias,
) -> Result<AssetDefinitionId> {
    let response = client
        .post_asset_alias_resolve(alias.as_ref())
        .wrap_err("failed to call `/v1/assets/aliases/resolve`")?;
    let status = response.status();
    if !status.is_success() {
        if status.as_u16() == 404 {
            eyre::bail!("asset alias `{alias}` is not bound to any asset definition");
        }
        eyre::bail!(
            "asset alias resolve request failed with HTTP {}",
            status.as_u16()
        );
    }
    let payload: norito::json::Value = norito::json::from_slice(response.body())
        .wrap_err("failed to decode asset alias resolve response")?;
    let map = payload
        .as_object()
        .ok_or_else(|| eyre!("asset alias resolve response must be a JSON object"))?;
    let definition_raw = map
        .get("asset_definition_id")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("asset alias resolve response missing `asset_definition_id` field"))?;
    AssetDefinitionId::parse_address_literal(definition_raw)
        .map_err(|err| eyre!("invalid `asset_definition_id` in alias response: {err}"))
}
fn parse_asset_definition_literal(literal: &str) -> Result<AssetDefinitionId> {
    AssetDefinitionId::parse_address_literal(literal)
        .map_err(|err| eyre!("asset definition literal: {err}"))
}
fn parse_domain_id_literal(literal: &str) -> std::result::Result<DomainId, String> {
    DomainId::parse_fully_qualified(literal).map_err(|err| err.to_string())
}
fn parse_register_account_id(literal: &str) -> Result<AccountId> {
    let trimmed = literal.trim();
    if trimmed.is_empty() {
        eyre::bail!("`ledger account register --id` must be a canonical I105 account id");
    }
    if trimmed.contains('@') {
        eyre::bail!(
            "`ledger account register --id` must not include '@domain'; accounts are global and aliases carry domains"
        );
    }
    if trimmed
        .get(..2)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("0x"))
    {
        eyre::bail!(
            "`ledger account register --id` must be canonical I105; canonical hex is not accepted"
        );
    }
    let parsed = AccountId::parse_encoded(trimmed).map_err(|err| {
        eyre!("`ledger account register --id` must be a canonical I105 account id: {err}")
    })?;
    Ok(parsed.into_account_id())
}
fn string_from_stdin() -> Result<String> {
    let bytes = read_cli_input_bounded(&mut io::stdin().lock(), MAX_CLI_STDIN_BYTES_V1, "stdin")?;
    String::from_utf8(bytes).map_err(|error| eyre!("stdin is not valid UTF-8: {error}"))
}
fn bytes_from_stdin() -> Result<Vec<u8>> {
    read_cli_input_bounded(&mut io::stdin().lock(), MAX_CLI_STDIN_BYTES_V1, "stdin")
}
fn read_cli_input_bounded<R: Read>(
    reader: &mut R,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8 * 1024];
    while bytes.len() < max_bytes {
        let remaining = max_bytes - bytes.len();
        let read_len = remaining.min(chunk.len());
        let count = loop {
            match reader.read(&mut chunk[..read_len]) {
                Ok(count) => break count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        };
        if count == 0 {
            return Ok(bytes);
        }
        bytes
            .try_reserve_exact(count)
            .map_err(|error| eyre!("failed to reserve {label} buffer storage: {error}"))?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    let mut growth_probe = [0_u8; 1];
    let extra = loop {
        match reader.read(&mut growth_probe) {
            Ok(count) => break count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    };
    if extra != 0 {
        eyre::bail!("{label} exceeds the first-release limit of {max_bytes} bytes");
    }
    Ok(bytes)
}
fn read_cli_file_bounded(path: &Path, label: &str) -> Result<Vec<u8>> {
    let mut file = fs::File::open(path)
        .wrap_err_with(|| format!("failed to open {label} {}", path.display()))?;
    let before = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect {label} {}", path.display()))?;
    if !before.is_file() {
        eyre::bail!("{label} must be a regular file: {}", path.display());
    }
    if before.len() > MAX_CLI_STDIN_BYTES_V1 as u64 {
        eyre::bail!(
            "{label} {} exceeds the first-release limit of {} bytes",
            path.display(),
            MAX_CLI_STDIN_BYTES_V1
        );
    }
    let bytes = read_cli_input_bounded(&mut file, MAX_CLI_STDIN_BYTES_V1, label)?;
    let after = file
        .metadata()
        .wrap_err_with(|| format!("failed to reinspect {label} {}", path.display()))?;
    if before.len() != after.len() || after.len() != bytes.len() as u64 {
        eyre::bail!(
            "{label} changed while it was being read: {}",
            path.display()
        );
    }
    Ok(bytes)
}
fn read_cli_text_file_bounded(path: &Path, label: &str) -> Result<String> {
    let bytes = read_cli_file_bounded(path, label)?;
    String::from_utf8(bytes)
        .map_err(|error| eyre!("{label} {} is not valid UTF-8: {error}", path.display()))
}
fn decode_base64_or_hex(
    input: &str,
    hex_length_err: &'static str,
    hex_parse_context: &'static str,
) -> Result<Vec<u8>> {
    let trimmed = input.trim();
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(trimmed) {
        return Ok(bytes);
    }
    let stripped = trimmed.trim_start_matches("0x");
    if !stripped.len().is_multiple_of(2) {
        eyre::bail!(hex_length_err);
    }
    let mut out = Vec::with_capacity(stripped.len() / 2);
    let mut index = 0;
    while index < stripped.len() {
        let byte =
            u8::from_str_radix(&stripped[index..index + 2], 16).wrap_err(hex_parse_context)?;
        out.push(byte);
        index += 2;
    }
    Ok(out)
}
type ReportResult<T, E> = core::result::Result<T, Report<E>>;
fn error_kind_for_report(report: &Report<MainError>) -> CliErrorKind {
    match report.current_context() {
        MainError::CliArgs(_) => CliErrorKind::Input,
        MainError::Config => CliErrorKind::Config,
        MainError::TransactionMetadata => CliErrorKind::Input,
        MainError::SerializeConfig => CliErrorKind::Internal,
        MainError::Command(_) => CliErrorKind::Command,
    }
}
struct CliRenderedError {
    kind: CliErrorKind,
    output: String,
}
fn render_cli_error(
    report: &Report<MainError>,
    output_format: CliOutputFormat,
) -> CliRenderedError {
    let kind = error_kind_for_report(&report);
    let message = report.to_string();
    let output = match output_format {
        CliOutputFormat::Text => format!("error: {message}\n"),
        CliOutputFormat::Json => {
            let rendered = json_utils::json_object(vec![(
                "error",
                json_utils::json_object(vec![
                    (
                        "kind",
                        json_utils::json_value(&kind.label()).unwrap_or(json::Value::Null),
                    ),
                    (
                        "message",
                        json_utils::json_value(&message).unwrap_or(json::Value::Null),
                    ),
                    (
                        "exit_code",
                        json_utils::json_value(&kind.exit_code()).unwrap_or(json::Value::Null),
                    ),
                ])
                .unwrap_or(json::Value::Null),
            )])
            .and_then(|value| {
                norito::json::to_json_pretty(&value)
                    .map_err(|err| eyre!("failed to render error JSON: {err}"))
            });
            match rendered {
                Ok(mut payload) => {
                    if !payload.ends_with('\n') {
                        payload.push('\n');
                    }
                    payload
                }
                Err(err) => {
                    format!("error: {message}\nerror: failed to render JSON payload: {err}\n")
                }
            }
        }
    };
    CliRenderedError { kind, output }
}
#[cfg(test)]
#[path = "main_shared_tests.rs"]
mod tests;
#[cfg(test)]
mod multisig_json_tests {
    use super::*;
    use iroha::crypto::{Algorithm, KeyPair};
    use iroha::data_model::{
        account::AccountId,
        domain::DomainId,
        isi::{CustomInstruction, InstructionBox},
    };
    use iroha::executor_data_model::isi::multisig::{
        DEFAULT_MULTISIG_TTL_MS, MultisigRegister, MultisigSpec,
    };
    use std::collections::BTreeMap;
    use std::num::{NonZeroU16, NonZeroU64};
    fn fixture_key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }
    fn multisig_account() -> AccountId {
        let key_pair = fixture_key_pair(0xD6);
        AccountId::new(key_pair.public_key().clone())
    }
    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair(0xD6).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
    #[test]
    fn multisig_register_payload_contains_account() {
        let account = multisig_account();
        let mut signatories = BTreeMap::new();
        signatories.insert(account.clone(), 1);
        let spec = MultisigSpec::new(
            signatories,
            NonZeroU16::new(1).expect("nonzero quorum"),
            NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).expect("nonzero ttl"),
        );
        let register = MultisigRegister::with_account(account.clone(), None::<DomainId>, spec);
        let instruction: InstructionBox = register.into();
        let payload = instruction
            .as_any()
            .downcast_ref::<CustomInstruction>()
            .expect("custom multisig instruction")
            .payload()
            .as_ref()
            .to_owned();
        assert!(
            payload.contains("\"account\""),
            "serialized payload missing account field: {payload}"
        );
    }
}
#[cfg(all(test, feature = "cli_integration_harness"))]
mod cli_integration_harness_tests {
    use super::*;
    use iroha::crypto::KeyPair;
    use iroha::data_model::query::{
        QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryWithParams,
        builder::{QueryBuilder, QueryExecutor},
        parameters::{FetchSize, Pagination, Sorting},
    };
    use iroha::data_model::{
        domain::{Domain, DomainId},
        prelude::FindDomains,
    };
    use iroha_crypto::Algorithm;
    use std::cmp::Ordering as CmpOrdering;
    use std::num::NonZeroU64;
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn fixture_key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }
    struct DummyExec;
    impl QueryExecutor for DummyExec {
        type Cursor = ();
        type Error = eyre::Report;
        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }
        fn start_query(
            &self,
            _query: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            // Return an empty Domain batch to satisfy type expectations
            Ok((
                QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Domain(vec![])),
                Some(0),
                None,
            ))
        }
        fn continue_query(
            _cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            unreachable!("no continuation in this test")
        }
    }
    fn sample_account_id(_domain: &str, seed: u8) -> AccountId {
        let key_pair = fixture_key_pair(seed);
        AccountId::new(key_pair.public_key().clone())
    }
    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair(1).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
    #[test]
    fn compound_predicate_json_roundtrip() {
        use iroha::data_model::query::dsl::CompoundPredicate;
        let owner = sample_account_id("wonderland", 1);
        let raw = format!(r#"{{"op":"eq","args":[{{"FieldPath":"authority"}},"{owner}"]}}"#);
        let predicate = super::parse_json::<CompoundPredicate<Domain>>(&raw)
            .expect("predicate JSON should parse");
        let serialized = norito::json::to_json(&predicate).expect("serialize predicate");
        assert!(serialized.contains("\"authority\""));
        assert_eq!(
            predicate
                .json_payload()
                .expect("json payload should be stored"),
            serialized.as_str()
        );
        let encoded = norito::codec::Encode::encode(&predicate);
        let decoded: CompoundPredicate<Domain> =
            norito::codec::Decode::decode(&mut encoded.as_slice()).expect("decode predicate");
        let decoded_json = norito::json::to_json(&decoded).expect("re-serialize predicate");
        assert_eq!(decoded_json, serialized);
    }
    #[test]
    fn compound_predicate_json_invalid() {
        use iroha::data_model::query::dsl::CompoundPredicate;
        let err = super::parse_json::<CompoundPredicate<Domain>>("{invalid}")
            .expect_err("invalid JSON must fail");
        assert!(err.to_string().contains("failed to parse JSON"));
    }
    #[test]
    fn parse_selector_and_apply_to_builder() {
        // Selector tuple parses from null under the lightweight DSL.
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Domain> =
            super::parse_json("null").expect("parse selector JSON");
        // Build a query with a non-default selector and ensure it executes via a dummy executor
        let exec = DummyExec;
        let builder = QueryBuilder::new(&exec, FindDomains).with_selector_tuple(tuple);
        // Also exercise other params to ensure they pass through
        let builder = builder
            .with_sorting(Sorting::default())
            .with_pagination(Pagination::default())
            .with_fetch_size(FetchSize::default());
        let out: Vec<Domain> = builder.execute_all().expect("exec ok");
        assert!(out.is_empty());
    }

    trait HarnessQueryFixture {
        type Item: Clone;

        fn ranked_three() -> Vec<Self::Item>;
        fn ranked_five(seed: u8) -> Vec<Self::Item>;
        fn positioned_five(seed: u8) -> Vec<Self::Item>;
        fn metadata<'a>(item: &'a Self::Item, key: &Name) -> Option<&'a Json>;
        fn batch(items: Vec<Self::Item>) -> QueryOutputBatchBox;

        fn sort_by_metadata(items: &mut [Self::Item], sorting: &Sorting) {
            let Some(key) = sorting.sort_by_metadata_key.as_ref() else {
                return;
            };
            let descending = matches!(
                sorting.order,
                Some(iroha::data_model::query::parameters::SortOrder::Desc)
            );
            items.sort_by(|left, right| {
                match (Self::metadata(left, key), Self::metadata(right, key)) {
                    (Some(left), Some(right)) => {
                        let order = left.cmp(right);
                        if descending { order.reverse() } else { order }
                    }
                    (Some(_), None) => CmpOrdering::Less,
                    (None, Some(_)) => CmpOrdering::Greater,
                    (None, None) => CmpOrdering::Equal,
                }
            });
        }
    }

    struct DomainFixture;
    impl HarnessQueryFixture for DomainFixture {
        type Item = Domain;

        fn ranked_three() -> Vec<Self::Item> {
            [("d1", Some(2), 0x10), ("d2", Some(1), 0x11), ("d3", None, 0x12)]
                .into_iter()
                .map(|(name, rank, seed)| {
                    let owner = sample_account_id("land", seed);
                    let mut domain = Domain::new(DomainId::try_new(name, "universal").unwrap())
                        .build(owner.account());
                    if let Some(rank) = rank {
                        domain.metadata_mut().insert(
                            "rank".parse().unwrap(),
                            Json::from(norito::json!(rank)),
                        );
                    }
                    domain
                })
                .collect()
        }

        fn ranked_five(seed: u8) -> Vec<Self::Item> {
            [("d0", Some(2)), ("d1", Some(4)), ("d2", None), ("d3", Some(1)), ("d4", Some(3))]
                .into_iter()
                .map(|(name, rank)| {
                    let owner = sample_account_id("universal", seed);
                    let mut domain = Domain::new(DomainId::try_new(name, "universal").unwrap())
                        .build(&owner);
                    if let Some(rank) = rank {
                        domain.metadata_mut().insert(
                            "rank".parse().unwrap(),
                            Json::from(norito::json!(rank)),
                        );
                    }
                    domain
                })
                .collect()
        }

        fn positioned_five(seed: u8) -> Vec<Self::Item> {
            (0..5)
                .map(|index| {
                    let owner = sample_account_id("universal", seed + index as u8);
                    Domain::new(
                        DomainId::try_new(&format!("d{index}"), "universal").unwrap(),
                    )
                    .build(&owner)
                })
                .collect()
        }

        fn metadata<'a>(item: &'a Self::Item, key: &Name) -> Option<&'a Json> {
            item.metadata().get(key)
        }

        fn batch(items: Vec<Self::Item>) -> QueryOutputBatchBox {
            QueryOutputBatchBox::Domain(items)
        }
    }

    struct AccountFixture;
    impl HarnessQueryFixture for AccountFixture {
        type Item = iroha::data_model::account::Account;

        fn ranked_three() -> Vec<Self::Item> {
            use iroha::data_model::account::Account;

            let ids = [0x20, 0x21, 0x22].map(|seed| sample_account_id("land", seed));
            let authority = ids[0].clone();
            let mut accounts: Vec<_> = ids
                .into_iter()
                .map(|id| Account::new(id).build(&authority))
                .collect();
            let key: Name = "rank".parse().unwrap();
            accounts[0]
                .metadata
                .insert(key.clone(), Json::from(norito::json!(2)));
            accounts[1]
                .metadata
                .insert(key, Json::from(norito::json!(1)));
            accounts
        }

        fn ranked_five(seed: u8) -> Vec<Self::Item> {
            use iroha::data_model::account::Account;

            let mut accounts: Vec<_> = (0..5)
                .map(|index| {
                    let id = sample_account_id("land", seed + index as u8);
                    Account::new(id.clone()).build(&id)
                })
                .collect();
            let key: Name = "rank".parse().unwrap();
            for (index, rank) in [(0, 2), (1, 4), (3, 1), (4, 3)] {
                accounts[index]
                    .metadata
                    .insert(key.clone(), Json::from(norito::json!(rank)));
            }
            accounts
        }

        fn positioned_five(seed: u8) -> Vec<Self::Item> {
            use iroha::data_model::account::Account;

            (0..5)
                .map(|index| {
                    let id = sample_account_id("land", seed + index as u8);
                    let mut account = Account::new(id.clone()).build(&id);
                    account.metadata.insert(
                        "pos".parse().unwrap(),
                        Json::from(norito::json!(index)),
                    );
                    account
                })
                .collect()
        }

        fn metadata<'a>(item: &'a Self::Item, key: &Name) -> Option<&'a Json> {
            item.metadata().get(key)
        }

        fn batch(items: Vec<Self::Item>) -> QueryOutputBatchBox {
            QueryOutputBatchBox::Account(items)
        }
    }

    struct AssetDefinitionFixture;
    impl HarnessQueryFixture for AssetDefinitionFixture {
        type Item = iroha::data_model::asset::definition::AssetDefinition;

        fn ranked_three() -> Vec<Self::Item> {
            use iroha::data_model::asset::{AssetBalancePolicy, definition::AssetDefinition};

            let owner = sample_account_id("land", 0x30);
            let domain = DomainId::try_new("land", "universal").unwrap();
            let mut definitions: Vec<_> = ["gold", "silver", "bronze"]
                .into_iter()
                .map(|name| {
                    let id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        domain.clone(),
                        name.parse().unwrap(),
                    );
                    AssetDefinition::numeric(id, name.to_owned(), AssetBalancePolicy::Global, None)
                        .build(owner.account())
                })
                .collect();
            let key: Name = "rank".parse().unwrap();
            definitions[0]
                .metadata_mut()
                .insert(key.clone(), Json::from(norito::json!(2)));
            definitions[1]
                .metadata_mut()
                .insert(key, Json::from(norito::json!(1)));
            definitions
        }

        fn ranked_five(seed: u8) -> Vec<Self::Item> {
            let mut definitions = Self::definitions(seed);
            let key: Name = "rank".parse().unwrap();
            for (index, rank) in [(0, 2), (1, 4), (3, 1), (4, 3)] {
                definitions[index]
                    .metadata_mut()
                    .insert(key.clone(), Json::from(norito::json!(rank)));
            }
            definitions
        }

        fn positioned_five(seed: u8) -> Vec<Self::Item> {
            let mut definitions = Self::definitions(seed);
            for (index, definition) in definitions.iter_mut().enumerate() {
                definition.metadata_mut().insert(
                    "pos".parse().unwrap(),
                    Json::from(norito::json!(index)),
                );
            }
            definitions
        }

        fn metadata<'a>(item: &'a Self::Item, key: &Name) -> Option<&'a Json> {
            item.metadata().get(key)
        }

        fn batch(items: Vec<Self::Item>) -> QueryOutputBatchBox {
            QueryOutputBatchBox::AssetDefinition(items)
        }
    }
    impl AssetDefinitionFixture {
        fn definitions(seed: u8) -> Vec<iroha::data_model::asset::definition::AssetDefinition> {
            use iroha::data_model::asset::{AssetBalancePolicy, definition::AssetDefinition};

            let domain = DomainId::try_new("land", "universal").unwrap();
            let owner = sample_account_id("land", seed);
            (0..5)
                .map(|index| {
                    let name = format!("ad{index}");
                    let id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        domain.clone(),
                        name.parse().unwrap(),
                    );
                    AssetDefinition::numeric(id, name, AssetBalancePolicy::Global, None)
                        .build(owner.account())
                })
                .collect()
        }
    }

    struct NftFixture;
    impl HarnessQueryFixture for NftFixture {
        type Item = iroha::data_model::nft::Nft;

        fn ranked_three() -> Vec<Self::Item> {
            use iroha::data_model::nft::Nft;

            let owner = sample_account_id("art", 0x70);
            let mut nfts: Vec<_> = ["n1$art", "n2$art", "n3$art"]
                .into_iter()
                .map(|id| Nft::new(id.parse().unwrap(), Default::default()).build(owner.account()))
                .collect();
            let key: Name = "rank".parse().unwrap();
            nfts[0]
                .content
                .insert(key.clone(), Json::from(norito::json!(2)));
            nfts[1]
                .content
                .insert(key, Json::from(norito::json!(1)));
            nfts
        }

        fn ranked_five(seed: u8) -> Vec<Self::Item> {
            let mut nfts = Self::nfts(seed);
            let key: Name = "rank".parse().unwrap();
            for (index, rank) in [(0, 2), (1, 4), (3, 1), (4, 3)] {
                nfts[index]
                    .content
                    .insert(key.clone(), Json::from(norito::json!(rank)));
            }
            nfts
        }

        fn positioned_five(seed: u8) -> Vec<Self::Item> {
            let mut nfts = Self::nfts(seed);
            for (index, nft) in nfts.iter_mut().enumerate() {
                nft.content.insert(
                    "pos".parse().unwrap(),
                    Json::from(norito::json!(index)),
                );
            }
            nfts
        }

        fn metadata<'a>(item: &'a Self::Item, key: &Name) -> Option<&'a Json> {
            item.content().get(key)
        }

        fn batch(items: Vec<Self::Item>) -> QueryOutputBatchBox {
            QueryOutputBatchBox::Nft(items)
        }
    }
    impl NftFixture {
        fn nfts(seed: u8) -> Vec<iroha::data_model::nft::Nft> {
            use iroha::data_model::nft::Nft;

            let owner = sample_account_id("art", seed);
            (0..5)
                .map(|index| {
                    Nft::new(
                        format!("n{index}$art").parse().unwrap(),
                        Default::default(),
                    )
                    .build(owner.account())
                })
                .collect()
        }
    }

    #[derive(Clone, Copy)]
    enum HarnessRows {
        RankedThree,
        RankedFive(u8),
        PositionedFive(u8),
    }

    struct HarnessQueryExecutor<F: HarnessQueryFixture> {
        rows: HarnessRows,
        starts: Option<&'static AtomicUsize>,
        continues: Option<&'static AtomicUsize>,
        _fixture: std::marker::PhantomData<F>,
    }
    impl<F: HarnessQueryFixture> HarnessQueryExecutor<F> {
        fn ranked_three() -> Self {
            Self {
                rows: HarnessRows::RankedThree,
                starts: None,
                continues: None,
                _fixture: std::marker::PhantomData,
            }
        }

        fn ranked_five(
            seed: u8,
            starts: &'static AtomicUsize,
            continues: &'static AtomicUsize,
        ) -> Self {
            Self {
                rows: HarnessRows::RankedFive(seed),
                starts: Some(starts),
                continues: Some(continues),
                _fixture: std::marker::PhantomData,
            }
        }

        fn positioned_five(
            seed: u8,
            starts: &'static AtomicUsize,
            continues: &'static AtomicUsize,
        ) -> Self {
            Self {
                rows: HarnessRows::PositionedFive(seed),
                starts: Some(starts),
                continues: Some(continues),
                _fixture: std::marker::PhantomData,
            }
        }
    }

    struct HarnessCursor<F: HarnessQueryFixture> {
        items: Vec<F::Item>,
        index: usize,
        end: usize,
        fetch: usize,
        continues: &'static AtomicUsize,
    }
    impl<F: HarnessQueryFixture> QueryExecutor for HarnessQueryExecutor<F> {
        type Cursor = HarnessCursor<F>;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            query: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            if let Some(starts) = self.starts {
                starts.fetch_add(1, Ordering::SeqCst);
            }
            let (mut items, paginated) = match self.rows {
                HarnessRows::RankedThree => (F::ranked_three(), false),
                HarnessRows::RankedFive(seed) => (F::ranked_five(seed), true),
                HarnessRows::PositionedFive(seed) => (F::positioned_five(seed), true),
            };
            if !matches!(self.rows, HarnessRows::PositionedFive(_)) {
                F::sort_by_metadata(&mut items, &query.params.sorting);
            }
            if !paginated {
                return Ok((
                    QueryOutputBatchBoxTuple::from_batch(F::batch(items)),
                    Some(0),
                    None,
                ));
            }

            let fetch = query
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get()
                .try_into()
                .unwrap_or(100);
            let offset = query.params.pagination.offset_value() as usize;
            let limit = query
                .params
                .pagination
                .limit_value()
                .map(|value| value.get() as usize);
            let start = offset.min(items.len());
            let end = limit
                .map(|limit| (start + limit).min(items.len()))
                .unwrap_or(items.len());
            let first_end = start.saturating_add(fetch).min(end);
            let first = items[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let cursor = if remaining > 0 {
                Some(HarnessCursor {
                    items,
                    index: first_end,
                    end,
                    fetch,
                    continues: self
                        .continues
                        .expect("paginated harness must count continuations"),
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple::from_batch(F::batch(first)),
                Some(remaining),
                cursor,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            cursor.continues.fetch_add(1, Ordering::SeqCst);
            let next_end = cursor.index.saturating_add(cursor.fetch).min(cursor.end);
            let batch = cursor.items[cursor.index..next_end].to_vec();
            let remaining = cursor.end.saturating_sub(next_end) as u64;
            let next = if remaining > 0 {
                Some(HarnessCursor {
                    items: cursor.items,
                    index: next_end,
                    end: cursor.end,
                    fetch: cursor.fetch,
                    continues: cursor.continues,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple::from_batch(F::batch(batch)),
                Some(remaining),
                next,
            ))
        }
    }

    static PAGED_DOMAINS_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PAGED_DOMAINS_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PSD_ASC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSD_ASC_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PSD_DESC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSD_DESC_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PSA_ASC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSA_ASC_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PSA_DESC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSA_DESC_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PSAD_ASC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSAD_ASC_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PSAD_DESC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSAD_DESC_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PSN_ASC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSN_ASC_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PSN_DESC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSN_DESC_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PAGED_ACCOUNTS_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PAGED_ACCOUNTS_CONTS: AtomicUsize = AtomicUsize::new(0);
    static PAGED_ADS_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PAGED_ADS_CONTS: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn metadata_sorting_end_to_end() {
        let exec = HarnessQueryExecutor::<DomainFixture>::ranked_three();
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Asc),
        };
        // Also assert selector tuple parsing is accepted
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Domain> =
            super::parse_json("null").expect("parse selector JSON");
        let builder = QueryBuilder::new(&exec, FindDomains)
            .with_selector_tuple(tuple)
            .with_sorting(sorting);
        let out: Vec<Domain> = builder.execute_all().expect("exec ok");
        // Expect d2 (rank=1), d1 (rank=2), then d3 (no rank)
        assert_eq!(out[0].id().name().as_ref(), "d2");
        assert_eq!(out[1].id().name().as_ref(), "d1");
        assert_eq!(out[2].id().name().as_ref(), "d3");
    }
    #[test]
    fn metadata_sorting_domains_desc_end_to_end() {
        let exec = HarnessQueryExecutor::<DomainFixture>::ranked_three();
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Desc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Domain> =
            super::parse_json("null").expect("parse selector JSON");
        let builder = QueryBuilder::new(&exec, FindDomains)
            .with_selector_tuple(tuple)
            .with_sorting(sorting);
        let out: Vec<Domain> = builder.execute_all().expect("exec ok");
        // Descending: d1 (2), d2 (1), then d3 (None)
        assert_eq!(out[0].id().name().as_ref(), "d1");
        assert_eq!(out[1].id().name().as_ref(), "d2");
        assert_eq!(out[2].id().name().as_ref(), "d3");
    }
    #[test]
    fn metadata_sorting_accounts_end_to_end() {
        use iroha::data_model::account::Account;
        use iroha::data_model::prelude::FindAccounts;
        let exec = HarnessQueryExecutor::<AccountFixture>::ranked_three();
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Asc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Account> =
            super::parse_json("null").expect("parse selector JSON");
        let builder = QueryBuilder::new(&exec, FindAccounts)
            .with_selector_tuple(tuple)
            .with_sorting(sorting);
        let out: Vec<Account> = builder.execute_all().expect("exec ok");
        // Expect a2 (rank=1), a1 (rank=2), then a3 (no rank)
        // Check by presence of metadata key for first two and existence of three items
        assert_eq!(out.len(), 3);
        assert!(
            out[0]
                .metadata()
                .get(&"rank".parse::<Name>().unwrap())
                .is_some()
        );
        assert!(
            out[1]
                .metadata()
                .get(&"rank".parse::<Name>().unwrap())
                .is_some()
        );
        assert!(
            out[2]
                .metadata()
                .get(&"rank".parse::<Name>().unwrap())
                .is_none()
        );
    }
    #[test]
    fn metadata_sorting_accounts_desc_end_to_end() {
        use iroha::data_model::account::Account;
        use iroha::data_model::prelude::FindAccounts;
        let exec = HarnessQueryExecutor::<AccountFixture>::ranked_three();
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Desc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Account> =
            super::parse_json("null").expect("parse selector JSON");
        let builder = QueryBuilder::new(&exec, FindAccounts)
            .with_selector_tuple(tuple)
            .with_sorting(sorting);
        let out: Vec<Account> = builder.execute_all().expect("exec ok");
        // Descending: ranks [2, 1, None]
        let key: Name = "rank".parse().unwrap();
        let ranks: Vec<Option<i64>> = out
            .iter()
            .map(|a| {
                a.metadata()
                    .get(&key)
                    .and_then(|j| j.try_into_any_norito::<i64>().ok())
            })
            .collect();
        assert_eq!(ranks, vec![Some(2), Some(1), None]);
    }
    #[test]
    fn metadata_sorting_asset_defs_end_to_end() {
        use iroha::data_model::asset::definition::AssetDefinition;
        use iroha::data_model::prelude::FindAssetsDefinitions;
        let exec = HarnessQueryExecutor::<AssetDefinitionFixture>::ranked_three();
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Asc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<AssetDefinition> =
            super::parse_json("null").expect("parse selector JSON");
        let builder = QueryBuilder::new(&exec, FindAssetsDefinitions)
            .with_selector_tuple(tuple)
            .with_sorting(sorting);
        let out: Vec<AssetDefinition> = builder.execute_all().expect("exec ok");
        // Expect silver (rank=1), gold (rank=2), then bronze (no rank)
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id().name().as_ref(), "silver");
        assert_eq!(out[1].id().name().as_ref(), "gold");
        assert_eq!(out[2].id().name().as_ref(), "bronze");
    }
    #[test]
    fn metadata_sorting_asset_defs_desc_end_to_end() {
        use iroha::data_model::asset::definition::AssetDefinition;
        use iroha::data_model::prelude::FindAssetsDefinitions;
        let exec = HarnessQueryExecutor::<AssetDefinitionFixture>::ranked_three();
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Desc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<AssetDefinition> =
            super::parse_json("null").expect("parse selector JSON");
        let builder = QueryBuilder::new(&exec, FindAssetsDefinitions)
            .with_selector_tuple(tuple)
            .with_sorting(sorting);
        let out: Vec<AssetDefinition> = builder.execute_all().expect("exec ok");
        // Descending: gold (2), silver (1), bronze (None)
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id().name().as_ref(), "gold");
        assert_eq!(out[1].id().name().as_ref(), "silver");
        assert_eq!(out[2].id().name().as_ref(), "bronze");
    }
    #[test]
    fn pagination_and_fetch_size_domains() {
        use iroha::data_model::query::parameters::{FetchSize, Pagination};
        let exec = HarnessQueryExecutor::<DomainFixture>::positioned_five(
            0x40,
            &PAGED_DOMAINS_STARTS,
            &PAGED_DOMAINS_CONTS,
        );
        PAGED_DOMAINS_STARTS.store(0, Ordering::SeqCst);
        PAGED_DOMAINS_CONTS.store(0, Ordering::SeqCst);
        let builder = QueryBuilder::new(&exec, FindDomains)
            .with_pagination(Pagination {
                limit: Some(NonZeroU64::new(3).unwrap()),
                offset: 1,
            })
            .with_fetch_size(FetchSize {
                fetch_size: Some(NonZeroU64::new(2).unwrap()),
            });
        let out: Vec<Domain> = builder.execute_all().expect("exec ok");
        // Expect exactly 3 items: d1, d2, d3 (offset=1, limit=3), split across batches of 2 and 1 internally
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id().name().as_ref(), "d1");
        assert_eq!(out[1].id().name().as_ref(), "d2");
        assert_eq!(out[2].id().name().as_ref(), "d3");
        // Cross-check that batches followed fetch_size boundary: first 2, then 1 → 2 batches total
        assert_eq!(PAGED_DOMAINS_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PAGED_DOMAINS_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pagination_sorting_domains_asc() {
        PSD_ASC_STARTS.store(0, Ordering::SeqCst);
        PSD_ASC_CONTS.store(0, Ordering::SeqCst);
        let exec = HarnessQueryExecutor::<DomainFixture>::ranked_five(
            0x50,
            &PSD_ASC_STARTS,
            &PSD_ASC_CONTS,
        );
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Asc),
        };
        let out: Vec<Domain> = QueryBuilder::new(&exec, FindDomains)
            .with_sorting(sorting)
            .with_pagination(Pagination {
                limit: Some(NonZeroU64::new(3).unwrap()),
                offset: 1,
            })
            .with_fetch_size(FetchSize {
                fetch_size: Some(NonZeroU64::new(2).unwrap()),
            })
            .execute_all()
            .expect("exec ok");
        // Asc sorted ranks: d3(1), d0(2), d4(3), d1(4), d2(None)
        // Offset 1, limit 3 => d0, d4, d1
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id().name().as_ref(), "d0");
        assert_eq!(out[1].id().name().as_ref(), "d4");
        assert_eq!(out[2].id().name().as_ref(), "d1");
        assert_eq!(PSD_ASC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSD_ASC_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pagination_sorting_domains_desc() {
        PSD_DESC_STARTS.store(0, Ordering::SeqCst);
        PSD_DESC_CONTS.store(0, Ordering::SeqCst);
        let exec = HarnessQueryExecutor::<DomainFixture>::ranked_five(
            0x51,
            &PSD_DESC_STARTS,
            &PSD_DESC_CONTS,
        );
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Desc),
        };
        let out: Vec<Domain> = QueryBuilder::new(&exec, FindDomains)
            .with_sorting(sorting)
            .with_pagination(Pagination {
                limit: Some(NonZeroU64::new(3).unwrap()),
                offset: 1,
            })
            .with_fetch_size(FetchSize {
                fetch_size: Some(NonZeroU64::new(2).unwrap()),
            })
            .execute_all()
            .expect("exec ok");
        // Desc sorted (missing last): d1(4), d4(3), d0(2), d3(1), d2(None)
        // Offset 1, limit 3 => d4, d0, d3
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id().name().as_ref(), "d4");
        assert_eq!(out[1].id().name().as_ref(), "d0");
        assert_eq!(out[2].id().name().as_ref(), "d3");
        assert_eq!(PSD_DESC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSD_DESC_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pagination_sorting_accounts_asc() {
        use iroha::data_model::prelude::FindAccounts;
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        PSA_ASC_STARTS.store(0, Ordering::SeqCst);
        PSA_ASC_CONTS.store(0, Ordering::SeqCst);
        let exec = HarnessQueryExecutor::<AccountFixture>::ranked_five(
            0xA0,
            &PSA_ASC_STARTS,
            &PSA_ASC_CONTS,
        );
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Asc),
        };
        let out: Vec<iroha::data_model::account::Account> = QueryBuilder::new(&exec, FindAccounts)
            .with_sorting(sorting)
            .with_pagination(Pagination {
                limit: Some(NonZeroU64::new(3).unwrap()),
                offset: 1,
            })
            .with_fetch_size(FetchSize {
                fetch_size: Some(NonZeroU64::new(2).unwrap()),
            })
            .execute_all()
            .expect("exec ok");
        // Asc ranks -> a3(1), a0(2), a4(3), a1(4), a2(None) => offset1,limit3 => a0,a4,a1
        let names: Vec<Option<i64>> = out
            .iter()
            .map(|a| {
                a.metadata()
                    .get(&"rank".parse::<Name>().unwrap())
                    .and_then(|j| j.try_into_any_norito::<i64>().ok())
            })
            .collect();
        assert_eq!(names, vec![Some(2), Some(3), Some(4)]);
        assert_eq!(PSA_ASC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSA_ASC_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pagination_sorting_accounts_desc() {
        use iroha::data_model::prelude::FindAccounts;
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        PSA_DESC_STARTS.store(0, Ordering::SeqCst);
        PSA_DESC_CONTS.store(0, Ordering::SeqCst);
        let exec = HarnessQueryExecutor::<AccountFixture>::ranked_five(
            0xB0,
            &PSA_DESC_STARTS,
            &PSA_DESC_CONTS,
        );
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Desc),
        };
        let out: Vec<iroha::data_model::account::Account> = QueryBuilder::new(&exec, FindAccounts)
            .with_sorting(sorting)
            .with_pagination(Pagination {
                limit: Some(NonZeroU64::new(3).unwrap()),
                offset: 1,
            })
            .with_fetch_size(FetchSize {
                fetch_size: Some(NonZeroU64::new(2).unwrap()),
            })
            .execute_all()
            .expect("exec ok");
        // Desc ranks -> a1(4), a4(3), a0(2), a3(1), a2(None) => offset1,limit3 => a4,a0,a3
        let names: Vec<Option<i64>> = out
            .iter()
            .map(|a| {
                a.metadata()
                    .get(&"rank".parse::<Name>().unwrap())
                    .and_then(|j| j.try_into_any_norito::<i64>().ok())
            })
            .collect();
        assert_eq!(names, vec![Some(3), Some(2), Some(1)]);
        assert_eq!(PSA_DESC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSA_DESC_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pagination_sorting_asset_defs_asc() {
        use iroha::data_model::prelude::FindAssetsDefinitions;
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        PSAD_ASC_STARTS.store(0, Ordering::SeqCst);
        PSAD_ASC_CONTS.store(0, Ordering::SeqCst);
        let exec = HarnessQueryExecutor::<AssetDefinitionFixture>::ranked_five(
            0x60,
            &PSAD_ASC_STARTS,
            &PSAD_ASC_CONTS,
        );
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Asc),
        };
        let out: Vec<iroha::data_model::asset::definition::AssetDefinition> =
            QueryBuilder::new(&exec, FindAssetsDefinitions)
                .with_sorting(sorting)
                .with_pagination(Pagination {
                    limit: Some(NonZeroU64::new(3).unwrap()),
                    offset: 1,
                })
                .with_fetch_size(FetchSize {
                    fetch_size: Some(NonZeroU64::new(2).unwrap()),
                })
                .execute_all()
                .expect("exec ok");
        // Asc ranks -> ad3(1), ad0(2), ad4(3), ad1(4), ad2(None) => offset1,limit3 => ad0,ad4,ad1
        let ranks: Vec<Option<i64>> = out
            .iter()
            .map(|ad| {
                ad.metadata()
                    .get(&"rank".parse::<Name>().unwrap())
                    .and_then(|j| j.try_into_any_norito::<i64>().ok())
            })
            .collect();
        assert_eq!(ranks, vec![Some(2), Some(3), Some(4)]);
        assert_eq!(PSAD_ASC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSAD_ASC_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pagination_sorting_asset_defs_desc() {
        use iroha::data_model::prelude::FindAssetsDefinitions;
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        PSAD_DESC_STARTS.store(0, Ordering::SeqCst);
        PSAD_DESC_CONTS.store(0, Ordering::SeqCst);
        let exec = HarnessQueryExecutor::<AssetDefinitionFixture>::ranked_five(
            0x61,
            &PSAD_DESC_STARTS,
            &PSAD_DESC_CONTS,
        );
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Desc),
        };
        let out: Vec<iroha::data_model::asset::definition::AssetDefinition> =
            QueryBuilder::new(&exec, FindAssetsDefinitions)
                .with_sorting(sorting)
                .with_pagination(Pagination {
                    limit: Some(NonZeroU64::new(3).unwrap()),
                    offset: 1,
                })
                .with_fetch_size(FetchSize {
                    fetch_size: Some(NonZeroU64::new(2).unwrap()),
                })
                .execute_all()
                .expect("exec ok");
        // Desc ranks -> ad1(4), ad4(3), ad0(2), ad3(1), ad2(None) => offset1,limit3 => ad4,ad0,ad3
        let ranks: Vec<Option<i64>> = out
            .iter()
            .map(|ad| {
                ad.metadata()
                    .get(&"rank".parse::<Name>().unwrap())
                    .and_then(|j| j.try_into_any_norito::<i64>().ok())
            })
            .collect();
        assert_eq!(ranks, vec![Some(3), Some(2), Some(1)]);
        assert_eq!(PSAD_DESC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSAD_DESC_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn metadata_sorting_nfts_end_to_end() {
        use iroha::data_model::nft::Nft;
        use iroha::data_model::prelude::FindNfts;
        let exec = HarnessQueryExecutor::<NftFixture>::ranked_three();
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Asc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Nft> =
            super::parse_json("null").expect("parse selector JSON");
        let builder = QueryBuilder::new(&exec, FindNfts)
            .with_selector_tuple(tuple)
            .with_sorting(sorting);
        let out: Vec<Nft> = builder.execute_all().expect("exec ok");
        // Expect n2 (rank=1), n1 (rank=2), then n3 (no rank)
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id().name().as_ref(), "n2");
        assert_eq!(out[1].id().name().as_ref(), "n1");
        assert_eq!(out[2].id().name().as_ref(), "n3");
    }
    #[test]
    fn metadata_sorting_nfts_desc_end_to_end() {
        use iroha::data_model::nft::Nft;
        use iroha::data_model::prelude::FindNfts;
        let exec = HarnessQueryExecutor::<NftFixture>::ranked_three();
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Desc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Nft> =
            super::parse_json("null").expect("parse selector JSON");
        let builder = QueryBuilder::new(&exec, FindNfts)
            .with_selector_tuple(tuple)
            .with_sorting(sorting);
        let out: Vec<Nft> = builder.execute_all().expect("exec ok");
        // Descending: n1 (2), n2 (1), then n3 (None)
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id().name().as_ref(), "n1");
        assert_eq!(out[1].id().name().as_ref(), "n2");
        assert_eq!(out[2].id().name().as_ref(), "n3");
    }
    #[test]
    fn pagination_sorting_nfts_asc() {
        use iroha::data_model::prelude::FindNfts;
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        PSN_ASC_STARTS.store(0, Ordering::SeqCst);
        PSN_ASC_CONTS.store(0, Ordering::SeqCst);
        let exec = HarnessQueryExecutor::<NftFixture>::ranked_five(
            0x71,
            &PSN_ASC_STARTS,
            &PSN_ASC_CONTS,
        );
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Asc),
        };
        let out: Vec<iroha::data_model::nft::Nft> = QueryBuilder::new(&exec, FindNfts)
            .with_sorting(sorting)
            .with_pagination(Pagination {
                limit: Some(NonZeroU64::new(3).unwrap()),
                offset: 1,
            })
            .with_fetch_size(FetchSize {
                fetch_size: Some(NonZeroU64::new(2).unwrap()),
            })
            .execute_all()
            .expect("exec ok");
        // Asc ranks -> n3(1), n0(2), n4(3), n1(4), n2(None) => offset1,limit3 => n0,n4,n1
        let names: Vec<&str> = out.iter().map(|n| n.id().name().as_ref()).collect();
        assert_eq!(names, vec!["n0", "n4", "n1"]);
        assert_eq!(PSN_ASC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSN_ASC_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pagination_sorting_nfts_desc() {
        use iroha::data_model::prelude::FindNfts;
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        PSN_DESC_STARTS.store(0, Ordering::SeqCst);
        PSN_DESC_CONTS.store(0, Ordering::SeqCst);
        let exec = HarnessQueryExecutor::<NftFixture>::ranked_five(
            0x72,
            &PSN_DESC_STARTS,
            &PSN_DESC_CONTS,
        );
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Desc),
        };
        let out: Vec<iroha::data_model::nft::Nft> = QueryBuilder::new(&exec, FindNfts)
            .with_sorting(sorting)
            .with_pagination(Pagination {
                limit: Some(NonZeroU64::new(3).unwrap()),
                offset: 1,
            })
            .with_fetch_size(FetchSize {
                fetch_size: Some(NonZeroU64::new(2).unwrap()),
            })
            .execute_all()
            .expect("exec ok");
        // Desc ranks -> n1(4), n4(3), n0(2), n3(1), n2(None) => offset1,limit3 => n4,n0,n3
        let names: Vec<&str> = out.iter().map(|n| n.id().name().as_ref()).collect();
        assert_eq!(names, vec!["n4", "n0", "n3"]);
        assert_eq!(PSN_DESC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSN_DESC_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pagination_and_fetch_size_accounts() {
        use iroha::data_model::account::Account;
        use iroha::data_model::prelude::FindAccounts;
        use iroha::data_model::query::parameters::{FetchSize, Pagination};
        let exec = HarnessQueryExecutor::<AccountFixture>::positioned_five(
            0x80,
            &PAGED_ACCOUNTS_STARTS,
            &PAGED_ACCOUNTS_CONTS,
        );
        PAGED_ACCOUNTS_STARTS.store(0, Ordering::SeqCst);
        PAGED_ACCOUNTS_CONTS.store(0, Ordering::SeqCst);
        let builder = QueryBuilder::new(&exec, FindAccounts)
            .with_pagination(Pagination {
                limit: Some(NonZeroU64::new(3).unwrap()),
                offset: 1,
            })
            .with_fetch_size(FetchSize {
                fetch_size: Some(NonZeroU64::new(2).unwrap()),
            });
        let out: Vec<Account> = builder.execute_all().expect("exec ok");
        // Expect exactly 3 items with pos metadata 1,2,3
        let key: Name = "pos".parse().unwrap();
        let positions: Vec<Option<i64>> = out
            .iter()
            .map(|a| {
                a.metadata()
                    .get(&key)
                    .and_then(|j| j.try_into_any_norito::<i64>().ok())
            })
            .collect();
        assert_eq!(positions, vec![Some(1), Some(2), Some(3)]);
        assert_eq!(PAGED_ACCOUNTS_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PAGED_ACCOUNTS_CONTS.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn pagination_and_fetch_size_asset_defs() {
        use iroha::data_model::asset::definition::AssetDefinition;
        use iroha::data_model::prelude::FindAssetsDefinitions;
        use iroha::data_model::query::parameters::{FetchSize, Pagination};
        let exec = HarnessQueryExecutor::<AssetDefinitionFixture>::positioned_five(
            0x90,
            &PAGED_ADS_STARTS,
            &PAGED_ADS_CONTS,
        );
        PAGED_ADS_STARTS.store(0, Ordering::SeqCst);
        PAGED_ADS_CONTS.store(0, Ordering::SeqCst);
        let builder = QueryBuilder::new(&exec, FindAssetsDefinitions)
            .with_pagination(Pagination {
                limit: Some(NonZeroU64::new(3).unwrap()),
                offset: 1,
            })
            .with_fetch_size(FetchSize {
                fetch_size: Some(NonZeroU64::new(2).unwrap()),
            });
        let out: Vec<AssetDefinition> = builder.execute_all().expect("exec ok");
        // Expect exactly 3 items with pos metadata 1,2,3
        let key: Name = "pos".parse().unwrap();
        let positions: Vec<Option<i64>> = out
            .iter()
            .map(|ad| {
                ad.metadata()
                    .get(&key)
                    .and_then(|j| j.try_into_any_norito::<i64>().ok())
            })
            .collect();
        assert_eq!(positions, vec![Some(1), Some(2), Some(3)]);
        assert_eq!(PAGED_ADS_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PAGED_ADS_CONTS.load(Ordering::SeqCst), 1);
    }
}
// Experimental: feature-gated integration harness for CLI queries.
//
// This module sketches how to exercise CLI query flows against a mock server or
// embedded state once server-side selectors/projections are fully enabled.
// It is intentionally behind a feature and unused by default to avoid pulling
// additional dependencies or affecting production builds.
#[cfg(all(test, feature = "cli_integration_harness"))]
mod cli_integration_harness {
    use super::*;
    use std::collections::BTreeMap;
    use eyre::eyre;
    use iroha::crypto::KeyPair;
    #[cfg(feature = "ids_projection")]
    use iroha::data_model::query::QueryItemKind;
    use iroha::data_model::query::runtime::AbiVersion;
    use iroha::data_model::{
        account::AccountId,
        asset::{Asset, AssetId},
        domain::DomainId,
        executor::ExecutorDataModel,
        parameter::Parameters,
        proof::{ProofId, ProofRecord},
        query::{
            QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryWithParams, SingularQueryBox,
            SingularQueryOutputBox, builder::QueryExecutor,
        },
        smart_contract::manifest::ContractManifest,
    };
    use iroha_crypto::{Algorithm, Hash};
    #[cfg(feature = "ids_projection")]
    use norito::codec::Decode;
    fn fixture_key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must derive a valid keypair")
    }
    /// Minimal mock server for iterable queries; returns static payloads by type.
    pub struct MockQueryServer {
        pub domains: Vec<iroha::data_model::domain::Domain>,
        pub accounts: Vec<iroha::data_model::account::Account>,
        pub triggers: Vec<iroha::data_model::trigger::Trigger>,
        pub asset_defs: Vec<iroha::data_model::asset::definition::AssetDefinition>,
        pub executor_data_model: Option<ExecutorDataModel>,
        pub parameters: Option<Parameters>,
        pub proof_records: BTreeMap<ProofId, ProofRecord>,
        pub manifests: BTreeMap<Hash, ContractManifest>,
        pub abi_version: Option<AbiVersion>,
        pub assets: BTreeMap<AssetId, Asset>,
    }
    impl Default for MockQueryServer {
        fn default() -> Self {
            Self {
                domains: vec![],
                accounts: vec![],
                triggers: vec![],
                asset_defs: vec![],
                executor_data_model: None,
                parameters: None,
                proof_records: BTreeMap::new(),
                manifests: BTreeMap::new(),
                abi_version: None,
                assets: BTreeMap::new(),
            }
        }
    }
    fn sample_account_id(_domain: &str, seed: u8) -> AccountId {
        let key_pair = fixture_key_pair(seed);
        AccountId::new(key_pair.public_key().clone())
    }
    #[test]
    fn fixture_key_pair_uses_checked_seed_derivation() {
        assert_eq!(fixture_key_pair(1).algorithm(), Algorithm::Ed25519);
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }
    #[cfg(feature = "ids_projection")]
    fn build_query_with_params<T, Q, F>(
        predicate: iroha::data_model::query::dsl::CompoundPredicate<T>,
        selector: iroha::data_model::query::dsl::SelectorTuple<T>,
        params: iroha::data_model::query::parameters::QueryParams,
        builder: F,
    ) -> QueryWithParams
    where
        T: iroha::data_model::query::dsl::HasProjection<
                iroha::data_model::query::dsl::PredicateMarker,
            > + iroha::data_model::query::dsl::HasProjection<
                iroha::data_model::query::dsl::SelectorMarker,
                AtomType = (),
            > + Send
            + Sync
            + iroha::data_model::query::ItemKindTag
            + 'static,
        Q: iroha::data_model::query::Query<Item = T> + norito::codec::Encode,
        F: FnOnce() -> Q,
    {
        let query = builder();
        QueryWithParams {
            query: (),
            query_payload: query.dyn_encode(),
            item: query.query_item_kind(),
            predicate_bytes: norito::codec::Encode::encode(&predicate),
            selector_bytes: norito::codec::Encode::encode(&selector),
            params,
        }
    }
    #[cfg(feature = "ids_projection")]
    fn query_projects_domain_ids(query: &QueryWithParams) -> bool {
        let (item_kind, _, selector_bytes, _) = query.parts();
        if item_kind != QueryItemKind::Domain {
            return false;
        }
        let mut cursor = selector_bytes;
        let selector: iroha::data_model::query::dsl::SelectorTuple<
            iroha::data_model::domain::Domain,
        > = match Decode::decode(&mut cursor) {
            Ok(selector) => selector,
            Err(_) => return false,
        };
        selector.is_ids_only()
    }
    #[cfg(feature = "ids_projection")]
    fn query_projects_account_ids(query: &QueryWithParams) -> bool {
        let (item_kind, _, selector_bytes, _) = query.parts();
        if item_kind != QueryItemKind::Account {
            return false;
        }
        let mut cursor = selector_bytes;
        let selector: iroha::data_model::query::dsl::SelectorTuple<
            iroha::data_model::account::Account,
        > = match Decode::decode(&mut cursor) {
            Ok(selector) => selector,
            Err(_) => return false,
        };
        selector.is_ids_only()
    }
    #[cfg(feature = "ids_projection")]
    fn query_projects_asset_definition_ids(query: &QueryWithParams) -> bool {
        let (item_kind, _, selector_bytes, _) = query.parts();
        if item_kind != QueryItemKind::AssetDefinition {
            return false;
        }
        let mut cursor = selector_bytes;
        let selector: iroha::data_model::query::dsl::SelectorTuple<
            iroha::data_model::asset::definition::AssetDefinition,
        > = match Decode::decode(&mut cursor) {
            Ok(selector) => selector,
            Err(_) => return false,
        };
        selector.is_ids_only()
    }
    // Cursor that carries the remaining items and fetch size
    pub enum MockCursor {
        Domains {
            items: Vec<iroha::data_model::domain::Domain>,
            idx: usize,
            fetch: usize,
        },
        Accounts {
            items: Vec<iroha::data_model::account::Account>,
            idx: usize,
            fetch: usize,
        },
        AssetDefs {
            items: Vec<iroha::data_model::asset::definition::AssetDefinition>,
            idx: usize,
            fetch: usize,
        },
        #[cfg(feature = "ids_projection")]
        DomainIds {
            ids: Vec<iroha::data_model::domain::DomainId>,
            idx: usize,
            fetch: usize,
        },
        #[cfg(feature = "ids_projection")]
        AccountIds {
            ids: Vec<iroha::data_model::account::AccountId>,
            idx: usize,
            fetch: usize,
        },
        #[cfg(feature = "ids_projection")]
        AssetDefIds {
            ids: Vec<iroha::data_model::asset::id::AssetDefinitionId>,
            idx: usize,
            fetch: usize,
        },
    }
    impl QueryExecutor for MockQueryServer {
        type Cursor = MockCursor;
        type Error = eyre::Report;
        fn execute_singular_query(
            &self,
            query: SingularQueryBox,
        ) -> Result<SingularQueryOutputBox, Self::Error> {
            match query {
                SingularQueryBox::FindExecutorDataModel(_) => self
                    .executor_data_model
                    .clone()
                    .map(SingularQueryOutputBox::ExecutorDataModel)
                    .ok_or_else(|| eyre!("executor data model not configured in MockQueryServer")),
                SingularQueryBox::FindParameters(_) => self
                    .parameters
                    .clone()
                    .map(SingularQueryOutputBox::Parameters)
                    .ok_or_else(|| eyre!("parameters not configured in MockQueryServer")),
                SingularQueryBox::FindAccountById(req) => self
                    .accounts
                    .iter()
                    .find(|account| account.id() == req.account_id())
                    .cloned()
                    .map(SingularQueryOutputBox::Account)
                    .ok_or_else(|| eyre!(format!("account `{}` not found", req.account_id()))),
                SingularQueryBox::FindProofRecordById(req) => self
                    .proof_records
                    .get(&req.id)
                    .cloned()
                    .map(SingularQueryOutputBox::ProofRecord)
                    .ok_or_else(|| eyre!(format!("proof record `{}` not found", req.id))),
                SingularQueryBox::FindContractManifestByCodeHash(req) => self
                    .manifests
                    .get(&req.code_hash)
                    .cloned()
                    .map(SingularQueryOutputBox::ContractManifest)
                    .ok_or_else(|| eyre!("contract manifest not found for supplied code hash")),
                SingularQueryBox::FindAbiVersion(_) => self
                    .abi_version
                    .clone()
                    .map(SingularQueryOutputBox::AbiVersion)
                    .ok_or_else(|| eyre!("ABI version not configured in MockQueryServer")),
                SingularQueryBox::FindAssetById(req) => self
                    .assets
                    .get(req.asset_id())
                    .cloned()
                    .map(SingularQueryOutputBox::Asset)
                    .ok_or_else(|| eyre!(format!("asset `{}` not found", req.asset_id()))),
                SingularQueryBox::FindTriggerById(req) => self
                    .triggers
                    .iter()
                    .find(|trigger| trigger.id() == req.trigger_id())
                    .cloned()
                    .map(SingularQueryOutputBox::Trigger)
                    .ok_or_else(|| eyre!(format!("trigger `{}` not found", req.trigger_id()))),
                other => Err(eyre!(format!(
                    "query `{other:?}` not supported in MockQueryServer"
                ))),
            }
        }
        fn start_query(
            &self,
            query: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            // Apply simple sorting by metadata key for known item types
            let sort_by = query.params.sorting.sort_by_metadata_key.clone();
            let desc = matches!(
                query.params.sorting.order,
                Some(iroha::data_model::query::parameters::SortOrder::Desc)
            );
            let fetch: usize = query
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get()
                .try_into()
                .unwrap_or(100);
            let offset: usize = query.params.pagination.offset_value() as usize;
            let limit: Option<usize> = query
                .params
                .pagination
                .limit_value()
                .map(|n| n.get() as usize);
            if !self.domains.is_empty() {
                let mut v = self.domains.clone();
                if let Some(key) = sort_by {
                    v.sort_by(|a, b| {
                        let la = a.metadata().get(&key);
                        let lb = b.metadata().get(&key);
                        match (la, lb) {
                            (Some(l), Some(r)) => {
                                let ord = l.cmp(r);
                                if desc { ord.reverse() } else { ord }
                            }
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => std::cmp::Ordering::Equal,
                        }
                    });
                }
                let start = offset.min(v.len());
                let end = limit.map(|l| (start + l).min(v.len())).unwrap_or(v.len());
                let first_end = start.saturating_add(fetch).min(end);
                let first = v[start..first_end].to_vec();
                let remaining = end.saturating_sub(first_end) as u64;
                // Detect ids-only selector for domains
                #[cfg(feature = "ids_projection")]
                if query_projects_domain_ids(&query) {
                    let first_ids: Vec<_> = first.iter().map(|d| d.id().clone()).collect();
                    let remaining_ids: Vec<_> =
                        v[first_end..end].iter().map(|d| d.id().clone()).collect();
                    let next = if remaining > 0 {
                        Some(MockCursor::DomainIds {
                            ids: remaining_ids,
                            idx: 0,
                            fetch,
                        })
                    } else {
                        None
                    };
                    return Ok((
                        QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::DomainId(
                            first_ids,
                        )),
                        Some(remaining),
                        next,
                    ));
                }
                let next = if remaining > 0 {
                    Some(MockCursor::Domains {
                        items: v[first_end..end].to_vec(),
                        idx: 0,
                        fetch,
                    })
                } else {
                    None
                };
                return Ok((
                    QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Domain(first)),
                    Some(remaining),
                    next,
                ));
            }
            if !self.accounts.is_empty() {
                let mut v = self.accounts.clone();
                if let Some(key) = sort_by {
                    v.sort_by(|a, b| {
                        let la = a.metadata().get(&key);
                        let lb = b.metadata().get(&key);
                        match (la, lb) {
                            (Some(l), Some(r)) => {
                                let ord = l.cmp(r);
                                if desc { ord.reverse() } else { ord }
                            }
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => std::cmp::Ordering::Equal,
                        }
                    });
                }
                let start = offset.min(v.len());
                let end = limit.map(|l| (start + l).min(v.len())).unwrap_or(v.len());
                let first_end = start.saturating_add(fetch).min(end);
                let first = v[start..first_end].to_vec();
                let remaining = end.saturating_sub(first_end) as u64;
                #[cfg(feature = "ids_projection")]
                if query_projects_account_ids(&query) {
                    let first_ids: Vec<_> = first.iter().map(|a| a.id().clone()).collect();
                    let remaining_ids: Vec<_> =
                        v[first_end..end].iter().map(|a| a.id().clone()).collect();
                    let next = if remaining > 0 {
                        Some(MockCursor::AccountIds {
                            ids: remaining_ids,
                            idx: 0,
                            fetch,
                        })
                    } else {
                        None
                    };
                    return Ok((
                        QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::AccountId(
                            first_ids,
                        )),
                        Some(remaining),
                        next,
                    ));
                }
                let next = if remaining > 0 {
                    Some(MockCursor::Accounts {
                        items: v[first_end..end].to_vec(),
                        idx: 0,
                        fetch,
                    })
                } else {
                    None
                };
                return Ok((
                    QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Account(first)),
                    Some(remaining),
                    next,
                ));
            }
            if !self.asset_defs.is_empty() {
                let mut v = self.asset_defs.clone();
                if let Some(key) = sort_by {
                    v.sort_by(|a, b| {
                        let la = a.metadata().get(&key);
                        let lb = b.metadata().get(&key);
                        match (la, lb) {
                            (Some(l), Some(r)) => {
                                let ord = l.cmp(r);
                                if desc { ord.reverse() } else { ord }
                            }
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => std::cmp::Ordering::Equal,
                        }
                    });
                }
                let start = offset.min(v.len());
                let end = limit.map(|l| (start + l).min(v.len())).unwrap_or(v.len());
                let first_end = start.saturating_add(fetch).min(end);
                let first = v[start..first_end].to_vec();
                let remaining = end.saturating_sub(first_end) as u64;
                #[cfg(feature = "ids_projection")]
                if query_projects_asset_definition_ids(&query) {
                    let first_ids: Vec<_> = first.iter().map(|ad| ad.id().clone()).collect();
                    let remaining_ids: Vec<_> =
                        v[first_end..end].iter().map(|ad| ad.id().clone()).collect();
                    let next = if remaining > 0 {
                        Some(MockCursor::AssetDefIds {
                            ids: remaining_ids,
                            idx: 0,
                            fetch,
                        })
                    } else {
                        None
                    };
                    return Ok((
                        QueryOutputBatchBoxTuple::from_batch(
                            QueryOutputBatchBox::AssetDefinitionId(first_ids),
                        ),
                        Some(remaining),
                        next,
                    ));
                }
                let next = if remaining > 0 {
                    Some(MockCursor::AssetDefs {
                        items: v[first_end..end].to_vec(),
                        idx: 0,
                        fetch,
                    })
                } else {
                    None
                };
                return Ok((
                    QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::AssetDefinition(
                        first,
                    )),
                    Some(remaining),
                    next,
                ));
            }
            Ok((
                QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(vec![])),
                Some(0),
                None,
            ))
        }
        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            match cursor {
                MockCursor::Domains { items, idx, fetch } => {
                    let end = (idx + fetch).min(items.len());
                    let batch = items[idx..end].to_vec();
                    let remaining = items.len().saturating_sub(end) as u64;
                    let next = if remaining > 0 {
                        Some(MockCursor::Domains {
                            items,
                            idx: end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Domain(batch)),
                        Some(remaining),
                        next,
                    ))
                }
                MockCursor::Accounts { items, idx, fetch } => {
                    let end = (idx + fetch).min(items.len());
                    let batch = items[idx..end].to_vec();
                    let remaining = items.len().saturating_sub(end) as u64;
                    let next = if remaining > 0 {
                        Some(MockCursor::Accounts {
                            items,
                            idx: end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Account(batch)),
                        Some(remaining),
                        next,
                    ))
                }
                MockCursor::AssetDefs { items, idx, fetch } => {
                    let end = (idx + fetch).min(items.len());
                    let batch = items[idx..end].to_vec();
                    let remaining = items.len().saturating_sub(end) as u64;
                    let next = if remaining > 0 {
                        Some(MockCursor::AssetDefs {
                            items,
                            idx: end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::AssetDefinition(
                            batch,
                        )),
                        Some(remaining),
                        next,
                    ))
                }
                #[cfg(feature = "ids_projection")]
                MockCursor::DomainIds { ids, idx, fetch } => {
                    let end = (idx + fetch).min(ids.len());
                    let batch = ids[idx..end].to_vec();
                    let remaining = ids.len().saturating_sub(end) as u64;
                    let next = if remaining > 0 {
                        Some(MockCursor::DomainIds {
                            ids,
                            idx: end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::DomainId(batch)),
                        Some(remaining),
                        next,
                    ))
                }
                #[cfg(feature = "ids_projection")]
                MockCursor::AccountIds { ids, idx, fetch } => {
                    let end = (idx + fetch).min(ids.len());
                    let batch = ids[idx..end].to_vec();
                    let remaining = ids.len().saturating_sub(end) as u64;
                    let next = if remaining > 0 {
                        Some(MockCursor::AccountIds {
                            ids,
                            idx: end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::AccountId(batch)),
                        Some(remaining),
                        next,
                    ))
                }
                #[cfg(feature = "ids_projection")]
                MockCursor::AssetDefIds { ids, idx, fetch } => {
                    let end = (idx + fetch).min(ids.len());
                    let batch = ids[idx..end].to_vec();
                    let remaining = ids.len().saturating_sub(end) as u64;
                    let next = if remaining > 0 {
                        Some(MockCursor::AssetDefIds {
                            ids,
                            idx: end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple::from_batch(
                            QueryOutputBatchBox::AssetDefinitionId(batch),
                        ),
                        Some(remaining),
                        next,
                    ))
                }
            }
        }
    }
    #[test]
    fn mock_query_domains_roundtrip() {
        use iroha::data_model::domain::Domain;
        use iroha::data_model::prelude::FindDomains;
        use iroha::data_model::query::builder::QueryBuilder;
        // Prepare a mock server with two domains
        let owner_w1 = sample_account_id("w1", 9);
        let owner_w2 = sample_account_id("w2", 10);
        let mut server = MockQueryServer::default();
        server.domains = vec![
            Domain::new(DomainId::try_new("w1", "universal").unwrap()).build(owner_w1.account()),
            Domain::new(DomainId::try_new("w2", "universal").unwrap()).build(owner_w2.account()),
        ];
        // Build and execute the query via QueryBuilder against the mock server
        let builder = QueryBuilder::new(&server, FindDomains);
        let out: Vec<Domain> = builder.execute_all().expect("exec ok");
        assert_eq!(out.len(), 2);
    }
    #[test]
    fn mock_query_domains_sorting_desc() {
        use iroha::data_model::domain::Domain;
        use iroha::data_model::prelude::FindDomains;
        use iroha::data_model::query::builder::QueryBuilder;
        use iroha::data_model::query::parameters::{SortOrder, Sorting};
        use iroha_primitives::json::Json;
        // Mock server with three domains; two have a rank key
        let owner_w1 = sample_account_id("w1", 11);
        let owner_w2 = sample_account_id("w2", 12);
        let owner_w3 = sample_account_id("w3", 13);
        let mut server = MockQueryServer::default();
        let mut w1 =
            Domain::new(DomainId::try_new("w1", "universal").unwrap()).build(owner_w1.account());
        let mut w2 =
            Domain::new(DomainId::try_new("w2", "universal").unwrap()).build(owner_w2.account());
        let w3 =
            Domain::new(DomainId::try_new("w3", "universal").unwrap()).build(owner_w3.account()); // no rank
        w1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        w2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        server.domains = vec![w1.clone(), w2.clone(), w3.clone()];
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Desc),
        };
        let builder = QueryBuilder::new(&server, FindDomains).with_sorting(sorting);
        let out: Vec<Domain> = builder.execute_all().expect("exec ok");
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id(), w2.id());
        assert_eq!(out[1].id(), w1.id());
        assert_eq!(out[2].id(), w3.id());
    }
    #[cfg(feature = "ids_projection")]
    #[test]
    fn mock_query_domains_ids_projection() {
        use iroha::data_model::domain::{Domain, DomainId};
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::QueryParams;
        use iroha::data_model::query::{self};
        let owner_w1 = sample_account_id("w1", 1);
        let owner_w2 = sample_account_id("w2", 2);
        let mut server = MockQueryServer::default();
        server.domains = vec![
            Domain::new(DomainId::try_new("w1", "universal").unwrap()).build(owner_w1.account()),
            Domain::new(DomainId::try_new("w2", "universal").unwrap()).build(owner_w2.account()),
        ];
        let qwp = build_query_with_params(
            CompoundPredicate::PASS,
            SelectorTuple::<Domain>::ids_only(),
            QueryParams::default(),
            || query::domain::prelude::FindDomains,
        );
        let (batch, _rem, _cur) = server.start_query(qwp).expect("start ok");
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::DomainId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], DomainId::try_new("w1", "universal").unwrap());
        assert_eq!(ids[1], DomainId::try_new("w2", "universal").unwrap());
    }
    #[cfg(feature = "ids_projection")]
    #[test]
    fn mock_query_accounts_ids_projection() {
        use iroha::data_model::account::Account;
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::QueryParams;
        use iroha::data_model::query::{self};
        let alice = sample_account_id("w", 1);
        let bob = sample_account_id("w", 2);
        let mut server = MockQueryServer::default();
        server.accounts = vec![
            Account::new(alice.clone()).build(&alice),
            Account::new(bob.clone()).build(&bob),
        ];
        let qwp = build_query_with_params(
            CompoundPredicate::PASS,
            SelectorTuple::<Account>::ids_only(),
            QueryParams::default(),
            || query::account::prelude::FindAccounts,
        );
        let (batch, _rem, _cur) = server.start_query(qwp).expect("start ok");
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AccountId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|id| id == &alice));
        assert!(ids.iter().any(|id| id == &bob));
    }
    #[cfg(feature = "ids_projection")]
    #[test]
    fn mock_query_asset_defs_ids_projection() {
        use iroha::data_model::asset::{definition::AssetDefinition, id::AssetDefinitionId};
        use iroha::data_model::prelude::NumericSpec;
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::QueryParams;
        use iroha::data_model::query::{self};
        let owner_w = sample_account_id("w", 1);
        let mut server = MockQueryServer::default();
        server.asset_defs = vec![
            {
                let __asset_definition_id =
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("w", "universal").unwrap(),
                        "rose".parse().unwrap(),
                    );
                AssetDefinition::new(
                    __asset_definition_id.clone(),
                    "rose".to_owned(),
                    NumericSpec::default(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
            }
            .build(owner_w.account()),
            {
                let __asset_definition_id =
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("w", "universal").unwrap(),
                        "tulip".parse().unwrap(),
                    );
                AssetDefinition::new(
                    __asset_definition_id.clone(),
                    "tulip".to_owned(),
                    NumericSpec::default(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
            }
            .build(owner_w.account()),
        ];
        let qwp = build_query_with_params(
            CompoundPredicate::PASS,
            SelectorTuple::<AssetDefinition>::ids_only(),
            QueryParams::default(),
            || query::asset::prelude::FindAssetsDefinitions,
        );
        let (batch, _rem, _cur) = server.start_query(qwp).expect("start ok");
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AssetDefinitionId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|id| id
            == &AssetDefinitionId::derive_from_components(
                DomainId::try_new("w", "universal").unwrap(),
                "rose".parse().unwrap()
            )));
        assert!(ids.iter().any(|id| id
            == &AssetDefinitionId::derive_from_components(
                DomainId::try_new("w", "universal").unwrap(),
                "tulip".parse().unwrap()
            )));
    }
    #[cfg(feature = "ids_projection")]
    #[test]
    fn mock_query_asset_defs_ids_projection_batched() {
        use iroha::data_model::asset::{definition::AssetDefinition, id::AssetDefinitionId};
        use iroha::data_model::prelude::NumericSpec;
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::{FetchSize, QueryParams};
        use iroha::data_model::query::{self};
        use std::num::NonZeroU64;
        let owner_w = sample_account_id("w", 2);
        let mut server = MockQueryServer::default();
        server.asset_defs = vec![
            {
                let __asset_definition_id =
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("w", "universal").unwrap(),
                        "rose".parse().unwrap(),
                    );
                AssetDefinition::new(
                    __asset_definition_id.clone(),
                    "rose".to_owned(),
                    NumericSpec::default(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
            }
            .build(owner_w.account()),
            {
                let __asset_definition_id =
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("w", "universal").unwrap(),
                        "tulip".parse().unwrap(),
                    );
                AssetDefinition::new(
                    __asset_definition_id.clone(),
                    "tulip".to_owned(),
                    NumericSpec::default(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
            }
            .build(owner_w.account()),
            {
                let __asset_definition_id =
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        DomainId::try_new("w", "universal").unwrap(),
                        "peony".parse().unwrap(),
                    );
                AssetDefinition::new(
                    __asset_definition_id.clone(),
                    "peony".to_owned(),
                    NumericSpec::default(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
            }
            .build(owner_w.account()),
        ];
        let mut params = QueryParams::default();
        params.fetch_size = FetchSize::new(Some(NonZeroU64::new(2).unwrap()));
        let qwp = build_query_with_params(
            CompoundPredicate::PASS,
            SelectorTuple::<AssetDefinition>::ids_only(),
            params,
            || query::asset::prelude::FindAssetsDefinitions,
        );
        let (batch1, rem, cur) = server.start_query(qwp).expect("start ok");
        let ids1 = match batch1.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AssetDefinitionId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids1.len(), 2);
        assert!(ids1.contains(&AssetDefinitionId::derive_from_components(
            DomainId::try_new("w", "universal").unwrap(),
            "rose".parse().unwrap()
        )));
        assert!(ids1.contains(&AssetDefinitionId::derive_from_components(
            DomainId::try_new("w", "universal").unwrap(),
            "tulip".parse().unwrap()
        )));
        assert_eq!(rem, Some(1));
        let cur = cur.expect("should continue");
        let (batch2, rem2, cur2) =
            <MockQueryServer as query::builder::QueryExecutor>::continue_query(cur)
                .expect("cont ok");
        let ids2 = match batch2.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AssetDefinitionId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids2.len(), 1);
        assert!(ids2.contains(&AssetDefinitionId::derive_from_components(
            DomainId::try_new("w", "universal").unwrap(),
            "peony".parse().unwrap()
        )));
        assert_eq!(rem2, Some(0));
        assert!(cur2.is_none());
    }
    #[cfg(feature = "ids_projection")]
    #[test]
    fn mock_query_accounts_ids_projection_batched() {
        use iroha::data_model::account::Account;
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::{FetchSize, QueryParams};
        use iroha::data_model::query::{self};
        use std::num::NonZeroU64;
        let alice = sample_account_id("w", 3);
        let bob = sample_account_id("w", 4);
        let carol = sample_account_id("w", 5);
        let mut server = MockQueryServer::default();
        server.accounts = vec![
            Account::new(alice.clone()).build(&alice),
            Account::new(bob.clone()).build(&bob),
            Account::new(carol.clone()).build(&carol),
        ];
        let mut params = QueryParams::default();
        params.fetch_size = FetchSize::new(Some(NonZeroU64::new(2).unwrap()));
        let qwp = build_query_with_params(
            CompoundPredicate::PASS,
            SelectorTuple::<Account>::ids_only(),
            params,
            || query::account::prelude::FindAccounts,
        );
        let (batch1, rem, cur) = server.start_query(qwp).expect("start ok");
        let ids1 = match batch1.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AccountId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids1.len(), 2);
        assert!(ids1.contains(&alice));
        assert!(ids1.contains(&bob));
        assert_eq!(rem, Some(1));
        let cur = cur.expect("should continue");
        let (batch2, rem2, cur2) =
            <MockQueryServer as query::builder::QueryExecutor>::continue_query(cur)
                .expect("cont ok");
        let ids2 = match batch2.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AccountId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids2.len(), 1);
        assert!(ids2.contains(&carol));
        assert_eq!(rem2, Some(0));
        assert!(cur2.is_none());
    }
    #[cfg(feature = "ids_projection")]
    #[test]
    fn mock_query_domains_ids_projection_batched() {
        use iroha::data_model::domain::{Domain, DomainId};
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::{FetchSize, QueryParams};
        use iroha::data_model::query::{self};
        use std::num::NonZeroU64;
        let owner_d1 = sample_account_id("d1", 1);
        let owner_d2 = sample_account_id("d2", 2);
        let owner_d3 = sample_account_id("d3", 3);
        let mut server = MockQueryServer::default();
        server.domains = vec![
            Domain::new(DomainId::try_new("d1", "universal").unwrap()).build(owner_d1.account()),
            Domain::new(DomainId::try_new("d2", "universal").unwrap()).build(owner_d2.account()),
            Domain::new(DomainId::try_new("d3", "universal").unwrap()).build(owner_d3.account()),
        ];
        let mut params = QueryParams::default();
        params.fetch_size = FetchSize::new(Some(NonZeroU64::new(2).unwrap()));
        let qwp = build_query_with_params(
            CompoundPredicate::PASS,
            SelectorTuple::<Domain>::ids_only(),
            params,
            || query::domain::prelude::FindDomains,
        );
        let (batch1, rem, cur) = server.start_query(qwp).expect("start ok");
        let ids1 = match batch1.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::DomainId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids1.len(), 2);
        assert!(ids1.contains(&DomainId::try_new("d1", "universal").unwrap()));
        assert!(ids1.contains(&DomainId::try_new("d2", "universal").unwrap()));
        assert_eq!(rem, Some(1));
        let cur = cur.expect("should continue");
        let (batch2, rem2, cur2) =
            <MockQueryServer as query::builder::QueryExecutor>::continue_query(cur)
                .expect("cont ok");
        let ids2 = match batch2.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::DomainId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids2.len(), 1);
        assert!(ids2.contains(&DomainId::try_new("d3", "universal").unwrap()));
        assert_eq!(rem2, Some(0));
        assert!(cur2.is_none());
    }
    #[test]
    fn mock_query_domains_sorting_desc_batched() {
        use iroha::data_model::domain::Domain;
        use iroha::data_model::prelude::FindDomains;
        use iroha::data_model::query::builder::QueryBuilder;
        use iroha::data_model::query::parameters::{FetchSize, SortOrder, Sorting};
        use iroha_primitives::json::Json;
        use std::num::NonZeroU64;
        let mut server = MockQueryServer::default();
        let owner_w1 = sample_account_id("w1", 6);
        let owner_w2 = sample_account_id("w2", 7);
        let owner_w3 = sample_account_id("w3", 8);
        let mut w1 =
            Domain::new(DomainId::try_new("w1", "universal").unwrap()).build(owner_w1.account());
        let mut w2 =
            Domain::new(DomainId::try_new("w2", "universal").unwrap()).build(owner_w2.account());
        let mut w3 =
            Domain::new(DomainId::try_new("w3", "universal").unwrap()).build(owner_w3.account());
        w1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        w2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        w3.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(0)));
        server.domains = vec![w1.clone(), w2.clone(), w3.clone()];
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(SortOrder::Desc),
        };
        let fetch = FetchSize::new(Some(NonZeroU64::new(2).unwrap()));
        let mut iter = QueryBuilder::new(&server, FindDomains)
            .with_sorting(sorting)
            .with_fetch_size(fetch)
            .execute()
            .expect("iter ok");
        let first = iter.next().unwrap().unwrap();
        let second = iter.next().unwrap().unwrap();
        let third = iter.next().unwrap().unwrap();
        assert_eq!(first.id(), w2.id());
        assert_eq!(second.id(), w1.id());
        assert_eq!(third.id(), w3.id());
        assert!(iter.next().is_none());
    }
    #[test]
    fn harness_singular_asset_roundtrip() {
        use iroha::data_model::{
            asset::{Asset, AssetId, id::AssetDefinitionId},
            query::asset::prelude::FindAssetById,
        };
        let mut server = MockQueryServer::default();
        let asset_def_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let account_id = sample_account_id("wonderland", 14);
        let asset_id = AssetId::new(asset_def_id, account_id.account().clone());
        let asset = Asset::new(asset_id.clone(), 77_u32);
        server.assets.insert(asset_id.clone(), asset.clone());
        let out = server
            .execute_singular_query(SingularQueryBox::FindAssetById(FindAssetById::new(
                asset_id.clone(),
            )))
            .expect("asset present");
        match out {
            SingularQueryOutputBox::Asset(found) => {
                assert_eq!(found.id, asset.id);
                assert_eq!(found.value(), asset.value());
            }
            other => panic!("unexpected output variant: {other:?}"),
        }
    }
    #[test]
    fn harness_singular_account_roundtrip() {
        use iroha::data_model::{account::Account, query::account::prelude::FindAccountById};
        let account_id = sample_account_id("wonderland", 15);
        let account = Account::new(account_id.clone()).build(&account_id);
        let mut server = MockQueryServer::default();
        server.accounts = vec![account.clone()];
        let out = server
            .execute_singular_query(SingularQueryBox::FindAccountById(FindAccountById::new(
                account_id.account().clone(),
            )))
            .expect("account present");
        match out {
            SingularQueryOutputBox::Account(found) => assert_eq!(found.id(), account.id()),
            other => panic!("unexpected output variant: {other:?}"),
        }
    }
    #[test]
    fn harness_singular_trigger_roundtrip() {
        use iroha::data_model::{
            events::execute_trigger::ExecuteTriggerEventFilter,
            query::trigger::prelude::FindTriggerById,
            transaction::{Executable, IvmBytecode},
            trigger::{
                Trigger, TriggerId,
                action::{Action, Repeats},
            },
        };
        let authority = sample_account_id("wonderland", 16);
        let trigger_id: TriggerId = "demo_trigger".parse().expect("trigger id");
        let action = Action::new(
            Executable::Ivm(IvmBytecode::from_compiled(Vec::new())),
            Repeats::Exactly(1),
            authority.account().clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants");
        let trigger = Trigger::new(trigger_id.clone(), action);
        let mut server = MockQueryServer::default();
        server.triggers = vec![trigger.clone()];
        let out = server
            .execute_singular_query(SingularQueryBox::FindTriggerById(FindTriggerById::new(
                trigger_id.clone(),
            )))
            .expect("trigger present");
        match out {
            SingularQueryOutputBox::Trigger(found) => assert_eq!(found.id(), trigger.id()),
            other => panic!("unexpected output variant: {other:?}"),
        }
    }
    #[test]
    fn harness_singular_contract_manifest() {
        use iroha::data_model::query::smart_contract::prelude::FindContractManifestByCodeHash;
        let mut server = MockQueryServer::default();
        let code_hash = Hash::new(b"manifest-demo");
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash.clone()),
            abi_hash: None,
            compiler_fingerprint: Some("kotodama-compiler".into()),
            features_bitmap: Some(0b1010),
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        };
        server.manifests.insert(code_hash.clone(), manifest.clone());
        let out = server
            .execute_singular_query(SingularQueryBox::FindContractManifestByCodeHash(
                FindContractManifestByCodeHash { code_hash },
            ))
            .expect("manifest present");
        match out {
            SingularQueryOutputBox::ContractManifest(found) => assert_eq!(found, manifest),
            other => panic!("unexpected output variant: {other:?}"),
        }
    }
    #[test]
    fn harness_singular_proof_record() {
        use iroha::data_model::{
            proof::{ProofId, ProofRecord, ProofStatus, VerifyingKeyId},
            query::proof::prelude::FindProofRecordById,
        };
        let mut server = MockQueryServer::default();
        let proof_id = ProofId {
            backend: "halo2/ipa".into(),
            proof_hash: [0xAB; 32],
        };
        let record = ProofRecord {
            id: proof_id.clone(),
            vk_ref: Some(VerifyingKeyId::new("halo2/ipa", "vk_demo")),
            vk_commitment: Some([0x11; 32]),
            status: ProofStatus::Verified,
            verified_at_height: Some(123),
            bridge: None,
        };
        server
            .proof_records
            .insert(proof_id.clone(), record.clone());
        let out = server
            .execute_singular_query(SingularQueryBox::FindProofRecordById(FindProofRecordById {
                id: proof_id,
            }))
            .expect("proof record present");
        match out {
            SingularQueryOutputBox::ProofRecord(found) => assert_eq!(found, record),
            other => panic!("unexpected output variant: {other:?}"),
        }
    }
    #[test]
    fn harness_singular_executor_and_parameters() {
        use iroha::data_model::{
            executor::ExecutorDataModel,
            parameter::Parameters,
            query::{
                executor::prelude::{FindExecutorDataModel, FindParameters},
                runtime::{AbiVersion, prelude::FindAbiVersion},
            },
        };
        use std::collections::BTreeSet;
        let mut server = MockQueryServer::default();
        let executor_model = ExecutorDataModel {
            parameters: Default::default(),
            instructions: BTreeSet::new(),
            permissions: BTreeSet::new(),
            schema: Json::from(norito::json!({ "kind": "demo" })),
        };
        server.executor_data_model = Some(executor_model.clone());
        server.parameters = Some(Parameters::default());
        server.abi_version = Some(AbiVersion { abi_version: 1 });
        let exec_out = server
            .execute_singular_query(SingularQueryBox::FindExecutorDataModel(
                FindExecutorDataModel,
            ))
            .expect("executor data model present");
        match exec_out {
            SingularQueryOutputBox::ExecutorDataModel(model) => assert_eq!(model, executor_model),
            other => panic!("unexpected output variant: {other:?}"),
        }
        let params_out = server
            .execute_singular_query(SingularQueryBox::FindParameters(FindParameters))
            .expect("parameters present");
        match params_out {
            SingularQueryOutputBox::Parameters(params) => assert_eq!(params, Parameters::default()),
            other => panic!("unexpected output variant: {other:?}"),
        }
        let abi_out = server
            .execute_singular_query(SingularQueryBox::FindAbiVersion(FindAbiVersion))
            .expect("ABI version present");
        match abi_out {
            SingularQueryOutputBox::AbiVersion(versions) => {
                assert_eq!(versions, AbiVersion { abi_version: 1 })
            }
            other => panic!("unexpected output variant: {other:?}"),
        }
    }
}
