mod address;
mod audit;
#[cfg(feature = "bridge")]
mod bridge;
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
mod runtime;
mod staking;
mod subscriptions;
mod space_directory;
mod sumeragi;
mod zk; // ZK helpers (app API convenience) // IVM/ABI helpers
use clap::{CommandFactory, FromArgMatches, error::ErrorKind};
use iroha_i18n::{Bundle, Localizer, detect_language};
use iroha_version::BuildLine;
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
use iroha::{
    client::Client,
    config::{BasicAuth, Config, LoadPath},
    data_model::{prelude::*, transaction::IvmBytecode},
    secrecy::SecretString,
};
use iroha_config::parameters::{actual::SorafsRolloutPhase, defaults};
use iroha_crypto::{Algorithm, KeyPair};
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

fn build_line() -> BuildLine {
    BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))
}
/// Norito JSON derive macros exported for CLI data definitions.
pub mod json_macros {
    pub use norito::derive::{FastJsonWrite, JsonDeserialize, JsonSerialize};
}

/// Iroha Client CLI provides a simple way to interact with the Iroha Web API.
#[derive(clap::Parser, Debug)]
#[command(name = env!("CARGO_BIN_NAME"), version = env!("CARGO_PKG_VERSION"), author)]
struct Args {
    /// Path to the configuration file.
    ///
    /// By default, `iroha` will try to read `client.toml` file, but would proceed if it is not found.
    #[arg(short, long, value_name("PATH"))]
    config: Option<PathBuf>,
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
    /// `echo "[]" | iroha -io domain register --id "domain" | iroha -i asset definition register --id "asset#domain" -t Numeric`
    #[arg(short, long)]
    input: bool,
    /// Outputs instructions to stdout without submitting them.
    ///
    /// Example usage:
    ///
    /// `iroha -o domain register --id "domain" | iroha -io asset definition register --id "asset#domain" -t Numeric | iroha transaction stdin`
    #[arg(short, long)]
    output: bool,
    /// Language code for messages, overrides system language
    #[arg(long, value_name("LANG"))]
    language: Option<String>,
    /// Commands
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Read and write domains
    #[command(subcommand)]
    Domain(domain::Command),
    /// Read and write accounts
    #[command(subcommand)]
    Account(account::Command),
    /// Account address helpers (IH58/compressed conversions)
    #[command(subcommand)]
    Address(address::Command),
    /// Read and write assets
    #[command(subcommand)]
    Asset(asset::Command),
    /// Read and write NFTs
    #[command(subcommand)]
    Nft(nft::Command),
    /// Read and write peers
    #[command(subcommand)]
    Peer(peer::Command),
    /// Subscribe to events: state changes, transaction/block/trigger progress
    Events(events::Args),
    /// Subscribe to blocks
    Blocks(blocks::Args),
    /// Read and write multi-signature accounts and transactions.
    ///
    /// See the [usage guide](./docs/multisig.md) for details
    #[command(subcommand)]
    Multisig(multisig::Command),
    /// Read various data
    #[command(subcommand)]
    Query(query::Command),
    /// Read transactions and write various data
    #[command(subcommand)]
    Transaction(transaction::Command),
    /// Read and write roles
    #[command(subcommand)]
    Role(role::Command),
    /// Read and write system parameters
    #[command(subcommand)]
    Parameter(parameter::Command),
    /// Read and write triggers
    #[command(subcommand)]
    Trigger(trigger::Command),
    /// Inspect offline allowances and offline-to-online bundles
    #[command(subcommand)]
    Offline(offline::Command),
    /// Read and write the executor
    #[command(subcommand)]
    Executor(executor::Command),
    /// Output CLI documentation in Markdown format
    MarkdownHelp(MarkdownHelp),
    /// Show versions and git SHA of client and server
    Version(Version),
    /// Zero-knowledge helpers (roots, etc.)
    #[command(subcommand)]
    Zk(zk::Command),
    /// Cryptography helpers (SM2/SM3/SM4)
    #[command(subcommand)]
    Crypto(crypto::Command),
    /// Confidential asset tooling helpers
    #[command(subcommand)]
    Confidential(confidential::Command),
    /// IVM/ABI helpers (e.g., compute ABI hash)
    #[command(subcommand)]
    Ivm(ivm_cli::Command),
    /// Governance helpers (app API convenience)
    #[command(subcommand)]
    Gov(gov::Command),
    /// Sumeragi helpers (status)
    #[command(subcommand)]
    Sumeragi(sumeragi::Command),
    /// Taikai publisher tooling (CAR bundler, envelopes)
    #[command(subcommand)]
    Taikai(commands::taikai::Command),
    /// Content hosting helpers
    #[command(subcommand)]
    Content(content::Command),
    /// Data availability helpers (ingest tooling)
    #[command(subcommand)]
    Da(commands::da::Command),
    /// Streaming helpers (HPKE fingerprints, suite listings)
    #[command(subcommand)]
    Streaming(commands::streaming::Command),
    /// Nexus helpers (lanes, governance)
    #[command(subcommand)]
    Nexus(nexus::Command),
    /// Public-lane staking helpers (register/activate/exit)
    #[command(subcommand)]
    Staking(staking::Command),
    /// Subscription plan and billing helpers
    #[command(subcommand)]
    Subscriptions(subscriptions::Command),
    /// Domain endorsement helpers (committees, policies, submissions)
    #[command(subcommand)]
    Endorsement(endorsement::Command),
    /// Jurisdiction Data Guardian helpers (attestations and SDN registries)
    #[command(subcommand)]
    Jurisdiction(jurisdiction::Command),
    /// Contracts helpers (code storage)
    #[command(subcommand)]
    Contracts(contracts::Command),
    /// Runtime ABI/upgrades
    #[command(subcommand)]
    Runtime(runtime::Command),
    /// Compute lane simulation helpers
    #[command(subcommand)]
    Compute(compute::Command),
    /// Social incentive helpers (viral follow rewards and escrows)
    #[command(subcommand)]
    Social(commands::social::Command),
    /// Space Directory helpers (UAID capability manifests)
    #[command(subcommand)]
    SpaceDirectory(space_directory::Command),
    /// Connect diagnostics helpers (queue inspection, evidence export)
    #[command(subcommand)]
    Connect(commands::connect::Command),
    /// Bridge tools (feature: bridge)
    #[cfg(feature = "bridge")]
    #[command(subcommand)]
    Bridge(crate::bridge::Command),
    /// Audit helpers (debug endpoints)
    #[command(subcommand)]
    Audit(audit::Command),
    /// Kaigi session helpers
    #[command(subcommand)]
    Kaigi(commands::kaigi::Command),
    /// `SoraFS` helpers (pin registry, aliases, replication orders, storage)
    #[command(subcommand)]
    Sorafs(commands::sorafs::Command),
    /// Soracles helpers (evidence bundling)
    #[command(subcommand)]
    Soracles(commands::soracles::Command),
    /// Sora Name Service helpers (registrar + policy tooling)
    #[command(subcommand)]
    Sns(commands::sns::Command),
    /// Alias helpers (placeholder pipeline)
    #[command(subcommand)]
    Alias(commands::alias::Command),
    /// Repo settlement helpers
    #[command(subcommand)]
    Repo(repo::Command),
    /// Delivery-versus-payment and payment-versus-payment helpers
    #[command(subcommand)]
    Settlement(settlement::Command),
}

/// Context inside which commands run
trait RunContext {
    fn config(&self) -> &Config;

    fn transaction_metadata(&self) -> Option<&Metadata>;

    fn input_instructions(&self) -> bool;

    fn output_instructions(&self) -> bool;

    fn i18n(&self) -> &Localizer;

    fn print_data<T>(&mut self, data: &T) -> Result<()>
    where
        T: JsonSerialize + ?Sized;

    fn println(&mut self, data: impl Display) -> Result<()>;

    fn client_from_config(&self) -> Client {
        Client::new(self.config().clone())
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
        let executable = instructions.into();
        let executable = match executable {
            Executable::Ivm(bytecode) => {
                if self.input_instructions() || self.output_instructions() {
                    eyre::bail!(
                        "Incompatible `--input` `--output` flags with `iroha transaction ivm`"
                    )
                }
                Executable::Ivm(bytecode)
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
        let client = self.client_from_config();
        let transaction = client.build_transaction(executable, metadata);
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

        self.println(confirmation_msg)?;
        self.print_data(&transaction)?;
        self.println(i18n.t("label.hash"))?;
        self.print_data(&hash)
    }
}

struct PrintJsonContext<W> {
    write: W,
    config: Config,
    transaction_metadata: Option<Metadata>,
    input_instructions: bool,
    output_instructions: bool,
    i18n: Localizer,
}

impl<W: std::io::Write> RunContext for PrintJsonContext<W> {
    fn config(&self) -> &Config {
        &self.config
    }

    fn transaction_metadata(&self) -> Option<&Metadata> {
        self.transaction_metadata.as_ref()
    }

    fn input_instructions(&self) -> bool {
        self.input_instructions
    }

    fn output_instructions(&self) -> bool {
        self.output_instructions
    }

    fn i18n(&self) -> &Localizer {
        &self.i18n
    }

    /// Serialize and print data
    ///
    /// # Errors
    ///
    /// - if serialization fails
    /// - if printing fails
    fn print_data<T>(&mut self, data: &T) -> Result<()>
    where
        T: JsonSerialize + ?Sized,
    {
        let mut rendered = norito::json::to_json_pretty(data)
            .map_err(|err| eyre!("failed to render JSON: {err}"))?;
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        self.write.write_all(rendered.as_bytes())?;
        Ok(())
    }

    fn println(&mut self, data: impl Display) -> Result<()> {
        writeln!(&mut self.write, "{data}")?;
        Ok(())
    }
}

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
            Domain(variant) => Run::run(variant, context),
            Account(variant) => Run::run(variant, context),
            Address(variant) => Run::run(variant, context),
            Asset(variant) => Run::run(variant, context),
            Nft(variant) => Run::run(variant, context),
            Peer(variant) => Run::run(variant, context),
            Events(variant) => Run::run(variant, context),
            Blocks(variant) => Run::run(variant, context),
            Multisig(variant) => Run::run(variant, context),
            Query(variant) => Run::run(variant, context),
            Transaction(variant) => Run::run(variant, context),
            Role(variant) => Run::run(variant, context),
            Parameter(variant) => Run::run(variant, context),
            Trigger(variant) => Run::run(variant, context),
            Offline(variant) => Run::run(variant, context),
            Executor(variant) => Run::run(variant, context),
            MarkdownHelp(variant) => Run::run(variant, context),
            Version(variant) => Run::run(variant, context),
            Zk(variant) => Run::run(variant, context),
            Crypto(variant) => Run::run(variant, context),
            Confidential(variant) => Run::run(variant, context),
            Ivm(variant) => Run::run(variant, context),
            Gov(variant) => Run::run(variant, context),
            Contracts(variant) => Run::run(variant, context),
            Runtime(variant) => Run::run(variant, context),
            Compute(variant) => Run::run(variant, context),
            SpaceDirectory(variant) => Run::run(variant, context),
            Connect(variant) => Run::run(variant, context),
            Sumeragi(variant) => Run::run(variant, context),
            Taikai(variant) => Run::run(variant, context),
            Da(variant) => Run::run(variant, context),
            Streaming(variant) => Run::run(variant, context),
            Nexus(variant) => Run::run(variant, context),
            Staking(variant) => Run::run(variant, context),
            Subscriptions(variant) => Run::run(variant, context),
            Endorsement(variant) => Run::run(variant, context),
            Jurisdiction(variant) => Run::run(variant, context),
            #[cfg(feature = "bridge")]
            Bridge(variant) => Run::run(variant, context),
            Audit(variant) => Run::run(variant, context),
            Kaigi(variant) => Run::run(variant, context),
            Sorafs(variant) => Run::run(variant, context),
            Soracles(variant) => Run::run(variant, context),
            Sns(variant) => Run::run(variant, context),
            Alias(variant) => Run::run(variant, context),
            Repo(variant) => Run::run(variant, context),
            Settlement(variant) => Run::run(variant, context),
            Content(variant) => Run::run(variant, context),
            Social(variant) => Run::run(variant, context),
        }
    }
}

#[derive(Error, Debug)]
enum MainError {
    #[error("Failed to load config")]
    Config,
    #[error("Failed to serialize config")]
    SerializeConfig,
    #[error("Failed to get transaction metadata from file")]
    TransactionMetadata,
    #[error("Failed to run the command")]
    Command,
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
        let i18n = context.i18n();
        println!(
            "{}",
            i18n.t_with("info.client_git_sha", &[("sha", VERGEN_GIT_SHA)])
        );
        let client_version = env!("CARGO_PKG_VERSION");
        println!(
            "{}",
            i18n.t_with("info.client_version", &[("version", client_version)])
        );
        let client = context.client_from_config();
        let response = client.get_server_version()?;
        println!(
            "{}",
            i18n.t_with(
                "info.server_version",
                &[("version", response.as_str())]
            )
        );
        Ok(())
    }
}

fn main() -> ReportResult<(), MainError> {
    run_with_line(build_line())
}

#[allow(clippy::too_many_lines)]
fn run_with_line(build_line: BuildLine) -> ReportResult<(), MainError> {
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
            _ => err.exit(),
        },
    };
    let args = Args::from_arg_matches(&matches).unwrap_or_else(|err| err.exit());

    let language = detect_language(args.language.as_deref());
    let i18n = Localizer::new(Bundle::Cli, language);
    eprintln!("{}", i18n.t("info.started"));
    let build_line_value = build_line.to_string();
    eprintln!(
        "{}",
        i18n.t_with("info.build_line", &[("build_line", build_line_value.as_str())])
    );

    if let Command::MarkdownHelp(_md) = args.command {
        clap_markdown::print_help_markdown::<Args>();
        return Ok(());
    }

    error_stack::Report::set_color_mode(color_mode());

    let (load_path, config_was_explicit) = args.config.as_ref().map_or_else(
        || (LoadPath::Default(PathBuf::from("client.toml")), false),
        |path| (LoadPath::Explicit(resolve_config_path(path)), true),
    );
    let explicit_path = args.config.as_ref().map(|p| resolve_config_path(p));
    let config_path = match &load_path {
        LoadPath::Explicit(path) | LoadPath::Default(path) => Some(path.clone()),
    };

    let mut config = match Config::load(load_path) {
        Ok(cfg) => cfg,
        Err(report) => {
            if config_was_explicit {
                if let Some(path) = explicit_path {
                    if let Some(cfg) = load_client_toml_fallback(&path) {
                        let path_display = path.display().to_string();
                        eprintln!(
                            "{}",
                            i18n.t_with(
                                "warning.config_parse_fallback",
                                &[("path", path_display.as_str())]
                            )
                        );
                        cfg
                    } else {
                        return Err(report
                            .change_context(MainError::Config)
                            .attach(i18n.t("error.config_path")));
                    }
                } else {
                    return Err(report
                        .change_context(MainError::Config)
                        .attach(i18n.t("error.config_path")));
                }
            } else {
                eprintln!("{}", i18n.t("warning.config_missing_offline"));
                fallback_config()
            }
        }
    };
    if let Some(path) = config_path
        && let Ok(raw) = fs::read_to_string(&path)
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

    let mut context = PrintJsonContext {
        write: io::stdout(),
        config,
        transaction_metadata: None,
        input_instructions: args.input,
        output_instructions: args.output,
        i18n: i18n.clone(),
    };
    if let Some(path) = args.metadata {
        let str = fs::read_to_string(&path)
            .change_context(MainError::TransactionMetadata)
            .attach("failed to read to string")?;
        let metadata: Metadata = parse_json(&str)
            .wrap_err("failed to deserialize metadata from JSON")
            .into_report()
            .map_err(|report| report.change_context(MainError::TransactionMetadata))?;
        context.transaction_metadata = Some(metadata);
    }

    args.command
        .run(&mut context)
        .into_report()
        .map_err(|report| report.change_context(MainError::Command))?;

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
        } else if let Some(value) = arg.strip_prefix("--language=") && !value.is_empty() {
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
        toml::Value::Integer(ms) if *ms >= 0 => {
            u64::try_from(*ms).ok().map(Duration::from_millis)
        }
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
    if let Some(ttl) = raw.get("transaction_ttl").and_then(parse_duration_value) {
        config.transaction_ttl = ttl;
    }
    if let Some(status) = raw
        .get("transaction_status_timeout")
        .and_then(parse_duration_value)
    {
        config.transaction_status_timeout = status;
    }
}

/// Fallback loader for simple client `toml` configs (`chain/torii/account/basic_auth`).
fn load_client_toml_fallback(path: &Path) -> Option<Config> {
    let raw = fs::read_to_string(path).ok()?;
    let value: toml::Value = toml::from_str(&raw).ok()?;

    let chain = value.get("chain")?.as_str()?.to_string();
    let torii_url = value.get("torii_url")?.as_str()?.to_string();

    let transaction_ttl = value
        .get("transaction_ttl")
        .and_then(|raw| raw.as_str())
        .and_then(|s| humantime::parse_duration(s).ok())
        .unwrap_or(iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE);

    let transaction_status_timeout = value
        .get("transaction_status_timeout")
        .and_then(|raw| raw.as_str())
        .and_then(|s| humantime::parse_duration(s).ok())
        .unwrap_or(iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT);
    let torii_request_timeout = value
        .get("torii_request_timeout_ms")
        .and_then(parse_duration_value)
        .or_else(|| value.get("torii_request_timeout").and_then(parse_duration_value))
        .unwrap_or(iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT);

    let account_table = value.get("account")?.as_table()?;
    let domain: DomainId = account_table.get("domain")?.as_str()?.parse().ok()?;
    let public_key: PublicKey = account_table.get("public_key")?.as_str()?.parse().ok()?;
    let private_key: ExposedPrivateKey =
        account_table.get("private_key")?.as_str()?.parse().ok()?;
    let key_pair = KeyPair::new(public_key.clone(), private_key.0.clone()).ok()?;
    let account = AccountId::new(domain, public_key);

    let basic_auth = value.get("basic_auth").and_then(|table| {
        table.as_table().and_then(|t| {
            let login = t.get("web_login")?.as_str()?.parse().ok()?;
            let password = t
                .get("password")?
                .as_str()
                .map(|s| SecretString::new(s.to_owned()))?;
            Some(BasicAuth {
                web_login: login,
                password,
            })
        })
    });

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

    Some(Config {
        chain: ChainId::from(chain),
        account,
        key_pair,
        basic_auth,
        torii_api_url: Url::parse(&torii_url).ok()?,
        torii_api_version: iroha::config::default_torii_api_version(),
        torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION.to_string(),
        torii_request_timeout,
        transaction_ttl,
        transaction_status_timeout,
        transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
        connect_queue_root: iroha::config::default_connect_queue_root(),
        sorafs_alias_cache: alias_cache,
        sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
        sorafs_rollout_phase: SorafsRolloutPhase::Default,
    })
}

fn fallback_config() -> Config {
    let chain = ChainId::from("offline-cli");
    let domain: DomainId = "offline".parse().expect("offline domain parses");
    let seed = vec![0u8; 32];
    let key_pair = KeyPair::from_seed(seed, Algorithm::Ed25519);
    let account = AccountId::new(domain, key_pair.public_key().clone());
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
    Config {
        chain,
        account,
        key_pair,
        basic_auth: None,
        torii_api_url: Url::parse("http://127.0.0.1:8080/").expect("fallback url"),
        torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
        transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
        transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
        transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
        torii_api_version: defaults::torii::API_DEFAULT_VERSION.to_string(),
        torii_api_min_proof_version: defaults::torii::API_MIN_PROOF_VERSION.to_string(),
        connect_queue_root: iroha::config::default_connect_queue_root(),
        sorafs_alias_cache: alias_cache,
        sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
        sorafs_rollout_phase: SorafsRolloutPhase::Default,
    }
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
        ("account", json_utils::json_value(&config.account)?),
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
    ])
}

fn account_admission_hint(err: &(dyn std::error::Error + 'static)) -> Option<String> {
    use iroha::data_model::isi::error::{AccountAdmissionError, AccountAdmissionQuotaScope};

    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(cause) = current {
        if let Some(admission) = cause.downcast_ref::<AccountAdmissionError>() {
            return Some(match admission {
                AccountAdmissionError::ImplicitAccountCreationDisabled(domain) => format!(
                    "Implicit account creation is disabled for domain `{domain}`; register the destination or pass `--ensure-destination` to add an explicit registration."
                ),
                AccountAdmissionError::InvalidPolicy(invalid) => format!(
                    "Account admission policy for `{}` is invalid: {}",
                    invalid.domain, invalid.reason
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
        /// Register a domain
        Register(Id),
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
                Register(args) => {
                    let instruction =
                        iroha::data_model::isi::Register::domain(Domain::new(args.id));
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register domain")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::domain(args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to unregister domain")
                }
                Transfer(args) => {
                    let instruction =
                        iroha::data_model::isi::Transfer::domain(args.from, args.id, args.to);
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
        #[arg(short, long)]
        pub id: DomainId,
        /// Source account, in the format "multihash@domain"
        #[arg(short, long)]
        pub from: AccountId,
        /// Destination account, in the format "multihash@domain"
        #[arg(short, long)]
        pub to: AccountId,
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Domain name
        #[arg(short, long)]
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
        Register(Id),
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
                    let client = context.client_from_config();
                    let entries = client
                        .query(FindAccounts)
                        .execute_all()
                        .wrap_err("Failed to get account")?;
                    let entry = entries
                        .into_iter()
                        .find(|e| e.id() == &args.id)
                        .ok_or_else(|| eyre!("Account not found"))?;
                    context.print_data(&entry)
                }
                Register(args) => {
                    let instruction =
                        iroha::data_model::isi::Register::account(Account::new(args.id));
                    context
                        .finish([instruction])
                        .wrap_err("Failed to register account")
                }
                Unregister(args) => {
                    let instruction = iroha::data_model::isi::Unregister::account(args.id);
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
                    let client = context.client_from_config();
                    let mut builder = client.query(FindRolesByAccountId::new(args.id));
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
                    let instruction =
                        iroha::data_model::isi::Grant::account_role(args.role, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to grant the role to the account")
                }
                Revoke(args) => {
                    let instruction =
                        iroha::data_model::isi::Revoke::account_role(args.role, args.id);
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
        Grant(Id),
        /// Revoke an account permission using JSON input from stdin
        Revoke(Id),
    }

    impl Run for PermissionCommand {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::PermissionCommand::*;
            match self {
                List(args) => {
                    let client = context.client_from_config();
                    let mut builder = client.query(FindPermissionsByAccountId::new(args.id));
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
                    let instruction =
                        iroha::data_model::isi::Grant::account_permission(permission, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to grant the permission to the account")
                }
                Revoke(args) => {
                    let permission: Permission = parse_json_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::Revoke::account_permission(permission, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to revoke the permission from the account")
                }
            }
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Account in the format "multihash@domain"
        #[arg(short, long)]
        id: AccountId,
    }

    #[derive(clap::Args, Debug)]
    pub struct RoleList {
        /// Account in the format "multihash@domain"
        #[arg(short, long)]
        id: AccountId,
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
        /// Account in the format "multihash@domain"
        #[arg(short, long)]
        id: AccountId,
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
        /// Account in the format "multihash@domain"
        #[arg(short, long)]
        pub id: AccountId,
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
                List::All(args) => list_all(context, client.query(FindAccounts), &args),
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
        builder: iroha::data_model::query::builder::QueryBuilder<'_, Client, FindAccounts, Account>,
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
}

mod asset {
    use super::*;
    use iroha::data_model::{
        account::{Account, AccountAdmissionMode},
        isi::Register,
    };

    fn admission_policy_for_domain(
        client: &Client,
        domain: &DomainId,
    ) -> Result<iroha::data_model::account::AccountAdmissionPolicy> {
        use iroha::data_model::{
            account::{ACCOUNT_ADMISSION_POLICY_METADATA_KEY, AccountAdmissionPolicy},
            name::Name,
            parameter::Parameters,
            prelude::{FindDomains, FindParameters},
        };

        let domains = client.query(FindDomains).execute_all()?;
        let domain = domains
            .into_iter()
            .find(|entry| entry.id() == domain)
            .ok_or_else(|| eyre!("Domain `{domain}` not found"))?;

        let policy_key: Name = ACCOUNT_ADMISSION_POLICY_METADATA_KEY
            .parse()
            .wrap_err("invalid account admission policy metadata key")?;
        if let Some(policy_json) = domain.metadata().get(&policy_key) {
            return policy_json
                .try_into_any_norito::<AccountAdmissionPolicy>()
                .wrap_err("failed to decode domain account admission policy");
        }

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
        args: &Transfer,
        policy: Option<&AccountAdmissionPolicy>,
    ) -> Vec<InstructionBox> {
        let mut instructions: Vec<InstructionBox> = Vec::new();
        if args.ensure_destination
            && matches!(
                policy.map(|p| p.mode),
                Some(AccountAdmissionMode::ExplicitOnly)
            )
        {
            instructions.push(InstructionBox::from(Register::account(Account::new(
                args.to.clone(),
            ))));
        }

        instructions.push(InstructionBox::from(
            iroha::data_model::isi::Transfer::asset_numeric(
                args.id.clone(),
                args.quantity.clone(),
                args.to.clone(),
            ),
        ));
        instructions
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Definition(cmd) => cmd.run(context),
                Get(args) => {
                    let client = context.client_from_config();
                    let entries = client
                        .query(FindAssets)
                        .execute_all()
                        .wrap_err("Failed to get asset")?;
                    let entry = entries
                        .into_iter()
                        .find(|e| e.id() == &args.id)
                        .ok_or_else(|| eyre!("Asset not found"))?;
                    context.print_data(&entry)
                }
                List(cmd) => cmd.run(context),
                Mint(args) => {
                    let instruction =
                        iroha::data_model::isi::Mint::asset_numeric(args.quantity, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to mint numeric asset")
                }
                Burn(args) => {
                    let instruction =
                        iroha::data_model::isi::Burn::asset_numeric(args.quantity, args.id);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to burn numeric asset")
                }
                Transfer(args) => {
                    let policy = if args.ensure_destination {
                        let client = context.client_from_config();
                        Some(admission_policy_for_domain(&client, args.to.domain())?)
                    } else {
                        None
                    };

                    let instructions = asset_transfer_instructions(&args, policy.as_ref());
                    context
                        .finish(instructions)
                        .wrap_err("Failed to transfer numeric asset")
                }
            }
        }
    }

    mod definition {
        use std::str::FromStr;

        use clap::ValueEnum;
        use iroha::{
            crypto::Hash,
            data_model::asset::{
                AssetDefinition, AssetDefinitionId,
                definition::{AssetConfidentialPolicy, ConfidentialPolicyMode},
            },
        };

        use super::*;

        #[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ConfidentialPolicyModeArg {
            TransparentOnly,
            ShieldedOnly,
            Convertible,
        }

        impl From<ConfidentialPolicyModeArg> for ConfidentialPolicyMode {
            fn from(value: ConfidentialPolicyModeArg) -> Self {
                match value {
                    ConfidentialPolicyModeArg::TransparentOnly => {
                        ConfidentialPolicyMode::TransparentOnly
                    }
                    ConfidentialPolicyModeArg::ShieldedOnly => ConfidentialPolicyMode::ShieldedOnly,
                    ConfidentialPolicyModeArg::Convertible => ConfidentialPolicyMode::Convertible,
                }
            }
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
                        let client = context.client_from_config();
                        let entries = client
                            .query(FindAssetsDefinitions)
                            .execute_all()
                            .wrap_err("Failed to get asset definition")?;
                        let entry = entries
                            .into_iter()
                            .find(|e| e.id() == &args.id)
                            .ok_or_else(|| eyre!("Asset definition not found"))?;
                        context.print_data(&entry)
                    }
                    Register(args) => {
                        let policy = confidential_policy_from_args(&args)
                            .wrap_err("invalid confidential policy arguments")?;
                        let mut entry = AssetDefinition::new(args.id, args.scale.into());
                        if args.mint_once {
                            entry = entry.mintable_once();
                        }
                        entry = entry.confidential_policy(policy);
                        let instruction = iroha::data_model::isi::Register::asset_definition(entry);
                        context
                            .finish([instruction])
                            .wrap_err("Failed to register asset")
                    }
                    Unregister(args) => {
                        let instruction =
                            iroha::data_model::isi::Unregister::asset_definition(args.id);
                        context
                            .finish([instruction])
                            .wrap_err("Failed to unregister asset")
                    }
                    Transfer(args) => {
                        let instruction = iroha::data_model::isi::Transfer::asset_definition(
                            args.from, args.id, args.to,
                        );
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
            /// Asset definition in the format "asset#domain"
            #[arg(short, long)]
            pub id: AssetDefinitionId,
            /// Disables minting after the first instance
            #[arg(short, long)]
            pub mint_once: bool,
            /// Numeric scale of the asset. No value means unconstrained.
            #[arg(short, long)]
            pub scale: Option<u32>,
            /// Confidential policy mode for this asset definition.
            #[arg(
                long,
                value_enum,
                default_value_t = ConfidentialPolicyModeArg::TransparentOnly
            )]
            pub confidential_mode: ConfidentialPolicyModeArg,
            /// Hex-encoded hash summarising the expected verifying key set.
            #[arg(long)]
            pub confidential_vk_set_hash: Option<String>,
            /// Poseidon parameter set identifier expected for confidential proofs.
            #[arg(long)]
            pub confidential_poseidon_params: Option<u32>,
            /// Pedersen parameter set identifier expected for confidential commitments.
            #[arg(long)]
            pub confidential_pedersen_params: Option<u32>,
        }

        #[derive(clap::Args, Debug)]
        pub struct Transfer {
            /// Asset definition in the format "asset#domain"
            #[arg(short, long)]
            pub id: AssetDefinitionId,
            /// Source account, in the format "multihash@domain"
            #[arg(short, long)]
            pub from: AccountId,
            /// Destination account, in the format "multihash@domain"
            #[arg(short, long)]
            pub to: AccountId,
        }

        #[derive(clap::Args, Debug)]
        pub struct Id {
            /// Asset definition in the format "asset#domain"
            #[arg(short, long)]
            pub id: AssetDefinitionId,
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

        fn confidential_policy_from_args(args: &Register) -> Result<AssetConfidentialPolicy> {
            let mode = ConfidentialPolicyMode::from(args.confidential_mode);
            let vk_set_hash = args
                .confidential_vk_set_hash
                .as_deref()
                .map(parse_vk_set_hash)
                .transpose()?;
            Ok(AssetConfidentialPolicy {
                mode,
                vk_set_hash,
                poseidon_params_id: args.confidential_poseidon_params,
                pedersen_params_id: args.confidential_pedersen_params,
                pending_transition: None,
            })
        }

        fn parse_vk_set_hash(input: &str) -> Result<Hash> {
            let trimmed = input.trim();
            Hash::from_str(trimmed)
                .wrap_err_with(|| format!("invalid hash literal `{trimmed}` for vk-set hash"))
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use iroha::data_model::asset::definition::ConfidentialPolicyMode;

            fn base_register_args() -> Register {
                Register {
                    id: "rose#wonderland".parse().expect("valid id"),
                    mint_once: false,
                    scale: None,
                    confidential_mode: ConfidentialPolicyModeArg::TransparentOnly,
                    confidential_vk_set_hash: None,
                    confidential_poseidon_params: None,
                    confidential_pedersen_params: None,
                }
            }

            #[test]
            fn confidential_policy_defaults_to_transparent() {
                let args = base_register_args();
                let policy = confidential_policy_from_args(&args).expect("policy should build");
                assert_eq!(policy.mode, ConfidentialPolicyMode::TransparentOnly);
                assert!(policy.vk_set_hash.is_none());
                assert!(policy.poseidon_params_id.is_none());
                assert!(policy.pedersen_params_id.is_none());
            }

            #[test]
            fn confidential_policy_parses_hash_and_params() {
                let mut args = base_register_args();
                args.confidential_mode = ConfidentialPolicyModeArg::ShieldedOnly;
                let hash = Hash::new(b"vk-digest");
                args.confidential_vk_set_hash = Some(hash.to_string());
                args.confidential_poseidon_params = Some(7);
                args.confidential_pedersen_params = Some(9);

                let policy = confidential_policy_from_args(&args).expect("policy should build");
                assert_eq!(policy.mode, ConfidentialPolicyMode::ShieldedOnly);
                assert_eq!(policy.vk_set_hash, Some(hash));
                assert_eq!(policy.poseidon_params_id, Some(7));
                assert_eq!(policy.pedersen_params_id, Some(9));
            }

            #[test]
            fn confidential_policy_rejects_invalid_hash() {
                let mut args = base_register_args();
                args.confidential_vk_set_hash = Some("not-a-hash".to_string());
                let err = confidential_policy_from_args(&args).expect_err("must fail");
                assert!(
                    err.to_string().contains("invalid hash literal"),
                    "unexpected error: {err}"
                );
            }
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Transfer {
        /// Asset in the format `asset##account@domain` or `asset#another_domain#account@domain`
        #[arg(short, long)]
        pub id: AssetId,
        /// Destination account, in the format "multihash@domain"
        #[arg(short, long)]
        pub to: AccountId,
        /// Transfer amount (integer or decimal)
        #[arg(short, long)]
        pub quantity: Numeric,
        /// Attempt to register the destination when implicit receive is disabled.
        #[arg(long)]
        pub ensure_destination: bool,
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Asset in the format `asset##account@domain` or `asset#another_domain#account@domain`
        #[arg(short, long)]
        pub id: AssetId,
    }

    #[derive(clap::Args, Debug)]
    pub struct IdQuantity {
        /// Asset in the format `asset##account@domain` or `asset#another_domain#account@domain`
        #[arg(short, long)]
        pub id: AssetId,
        /// Amount of change (integer or decimal)
        #[arg(short, long)]
        pub quantity: Numeric,
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
        use iroha::data_model::isi::{Instruction, RegisterBox, TransferBox};
        use iroha_crypto::Algorithm;
        use iroha_primitives::numeric::Numeric;

        fn sample_transfer_args(ensure_destination: bool) -> Transfer {
            let domain: DomainId = "wonderland".parse().expect("domain id");
            let src = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
            let dest = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
            let owner = AccountId::new(domain.clone(), src.public_key().clone());
            let to = AccountId::new(domain, dest.public_key().clone());
            let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def");
            let asset_id = AssetId::new(asset_def_id, owner);

            Transfer {
                id: asset_id,
                to,
                quantity: Numeric::new(5, 0),
                ensure_destination,
            }
        }

        fn assert_register_account(instruction: &InstructionBox, expected: &AccountId) {
            let any: &dyn Instruction = &**instruction;
            let reg = any
                .as_any()
                .downcast_ref::<RegisterBox>()
                .expect("register instruction");
            match reg {
                RegisterBox::Account(account_reg) => {
                    assert_eq!(account_reg.object().id(), expected);
                }
                other => panic!("unexpected register variant: {other:?}"),
            }
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
        fn explicit_policy_adds_register_instruction() {
            let args = sample_transfer_args(true);
            let policy = AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ExplicitOnly,
                ..AccountAdmissionPolicy::default()
            };

            let instructions = asset_transfer_instructions(&args, Some(&policy));
            assert_eq!(instructions.len(), 2);
            assert_register_account(&instructions[0], &args.to);
            assert_transfer_destination(&instructions[1], &args.to);
        }

        #[test]
        fn implicit_policy_skips_register_instruction() {
            let args = sample_transfer_args(true);
            let instructions =
                asset_transfer_instructions(&args, Some(&AccountAdmissionPolicy::default()));
            assert_eq!(instructions.len(), 1);
            assert_transfer_destination(&instructions[0], &args.to);
        }

        #[test]
        fn ensure_flag_off_sends_transfer_only() {
            let args = sample_transfer_args(false);
            let policy = AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ExplicitOnly,
                ..AccountAdmissionPolicy::default()
            };
            let instructions = asset_transfer_instructions(&args, Some(&policy));
            assert_eq!(instructions.len(), 1);
            assert_transfer_destination(&instructions[0], &args.to);
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
        /// Get a value from NFT
        #[command(name = "getkv")]
        GetKeyValue(IdKey),
        /// Create or update a key-value entry of NFT using JSON input from stdin
        #[command(name = "setkv")]
        SetKeyValue(IdKey),
        /// Remove a key-value entry from NFT
        #[command(name = "removekv")]
        RemoveKeyValue(IdKey),
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
                    let instruction =
                        iroha::data_model::isi::Transfer::nft(args.from, args.id, args.to);
                    context
                        .finish([instruction])
                        .wrap_err("Failed to transfer NFT")
                }
                GetKeyValue(args) => {
                    let client = context.client_from_config();
                    let entries = client
                        .query(FindNfts)
                        .execute_all()
                        .wrap_err("Failed to get value")?;
                    let nft = entries
                        .into_iter()
                        .find(|e| e.id() == &args.id)
                        .ok_or_else(|| eyre!("NFT not found"))?;
                    let value = nft
                        .content()
                        .get(&args.key)
                        .cloned()
                        .ok_or_else(|| eyre!("Key not found"))?;
                    context.print_data(&value)
                }
                SetKeyValue(args) => {
                    let value: Json = parse_json_stdin(context)?;
                    let instruction =
                        iroha::data_model::isi::SetKeyValue::nft(args.id, args.key, value);
                    context.finish([instruction])
                }
                RemoveKeyValue(args) => {
                    let instruction =
                        iroha::data_model::isi::RemoveKeyValue::nft(args.id, args.key);
                    context.finish([instruction])
                }
            }
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Transfer {
        /// NFT in the format "name$domain"
        #[arg(short, long)]
        pub id: NftId,
        /// Source account, in the format "multihash@domain"
        #[arg(short, long)]
        pub from: AccountId,
        /// Destination account, in the format "multihash@domain"
        #[arg(short, long)]
        pub to: AccountId,
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// NFT in the format "name$domain"
        #[arg(short, long)]
        pub id: NftId,
    }

    #[derive(clap::Args, Debug)]
    pub struct IdKey {
        /// NFT in the format "name$domain"
        #[arg(short, long)]
        pub id: NftId,
        #[arg(short, long)]
        pub key: Name,
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
                    let mut builder = client.query(FindPeers);
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
                    let entries = builder.execute_all()?;
                    context.print_data(&entries)
                }
            }
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Id {
        /// Peer's public key in multihash format
        #[arg(short, long)]
        pub key: PublicKey,
    }
}

mod multisig {
    use core::convert::TryFrom;
    use std::{
        collections::BTreeMap,
        num::{NonZeroU16, NonZeroU64},
        time::{Duration, SystemTime},
    };

    use derive_more::{Constructor, Display};
    use iroha::data_model::isi::CustomInstruction;
    use iroha::executor_data_model::isi::multisig::*;

    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// List pending multisig transactions relevant to you
        #[command(subcommand)]
        List(List),
        /// Register a multisig account
        Register(Box<Register>),
        /// Propose a multisig transaction using JSON input from stdin
        Propose(Propose),
        /// Approve a multisig transaction
        Approve(Approve),
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
                Inspect(cmd) => cmd.run(context),
            }
        }
    }
    #[derive(clap::Args, Debug)]
    pub struct Register {
        /// List of signatories for the multisig account
        #[arg(short, long, num_args(2..))]
        pub signatories: Vec<AccountId>,
        /// Relative weights of signatories' responsibilities
        #[arg(short, long, num_args(2..))]
        pub weights: Vec<u8>,
        /// Threshold of total weight required for authentication
        #[arg(short, long)]
        pub quorum: u16,
        /// Account id to use for the multisig controller. If omitted, a new
        /// random account is generated in the signatory domain and the private
        /// key is discarded locally.
        #[arg(long)]
        pub account: Option<AccountId>,
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
            let signatories = self.signatories;
            if signatories.len() != self.weights.len() {
                return Err(eyre!("signatories and weights must be equal in length"));
            }
            let domain = signatories
                .first()
                .expect("clap enforces non-empty signatories")
                .domain()
                .clone();
            if !signatories.iter().all(|id| id.domain() == &domain) {
                return Err(eyre!(
                    "all signatories must belong to domain `{domain}` to register a multisig account"
                ));
            }
            let account = if let Some(account) = self.account {
                if account.domain() != &domain {
                    return Err(eyre!(
                        "multisig account `{account}` must belong to the signatory domain `{domain}`"
                    ));
                }
                account
            } else {
                let generated = KeyPair::random();
                AccountId::new(domain.clone(), generated.public_key().clone())
            };
            let spec = MultisigSpec::new(
                signatories.into_iter().zip(self.weights).collect(),
                NonZeroU16::new(self.quorum).expect("quorum should not be 0"),
                self.transaction_ttl
                    .as_millis()
                    .try_into()
                    .ok()
                    .and_then(NonZeroU64::new)
                    .expect("ttl should be between 1 ms and 584942417 years"),
            );
            if !context.output_instructions() {
                context.println(format!("multisig account id: {account}"))?;
            }
            let instruction = MultisigRegister::with_account(account, spec);

            context
                .finish([iroha::data_model::isi::InstructionBox::from(instruction)])
                .wrap_err("Failed to register multisig account")
        }
    }


    #[derive(clap::Args, Debug)]
    pub struct Propose {
        /// Multisig authority managing the proposed transaction
        #[arg(short, long)]
        pub account: AccountId,
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

            if !context.output_instructions() {
                surface_policy_ttl(context, &self.account, transaction_ttl_ms)?;
            }

            let instructions_hash = HashOf::new(&instructions);
            println!("{instructions_hash}");

            let propose_multisig_transaction =
                MultisigPropose::new(self.account, instructions, transaction_ttl_ms);

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
        pub account: AccountId,
        /// Hash of the instructions to approve
        #[arg(short, long)]
        pub instructions_hash: ProposalKey,
    }

    impl Run for Approve {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let approve_multisig_transaction =
                MultisigApprove::new(self.account, self.instructions_hash);

            context
                .finish([iroha::data_model::isi::InstructionBox::from(
                    approve_multisig_transaction,
                )])
                .wrap_err("Failed to approve transaction")
        }
    }

    #[derive(clap::Args, Debug)]
    pub struct Inspect {
        /// Multisig account identifier to inspect
        #[arg(short, long)]
        pub account: AccountId,
        /// Emit JSON instead of human-readable output
        #[arg(long)]
        pub json: bool,
    }

    impl Run for Inspect {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use iroha::data_model::prelude::FindAccounts;
            let client = context.client_from_config();
            let mut accounts = client.query(FindAccounts).execute_all()?;
            let account = accounts
                .iter_mut()
                .find(|entry| entry.id() == &self.account)
                .ok_or_else(|| eyre!("account `{}` not found", self.account))?;

            let controller =
                account.id().controller().multisig_policy().ok_or_else(|| {
                    eyre!("account `{}` is not multisig-controlled", self.account)
                })?;

            let ctap2 = controller.encode_ctap2();
            let digest = controller.digest_blake2b256();
            let ctap2_hex = hex::encode_upper(&ctap2);
            let digest_hex = hex::encode_upper(digest);

            if self.json {
                let members = controller
                    .members()
                    .iter()
                    .map(|member| {
                        let (algorithm, payload) = member.public_key().to_bytes();
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
                        json::Value::from(member_map)
                    })
                    .collect::<Vec<_>>();
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
                    let (algorithm, payload) = member.public_key().to_bytes();
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
        /// List all pending multisig transactions relevant to you
        All {
            /// Maximum number of role IDs to scan for multisig (server-side limit)
            #[arg(long)]
            limit: Option<u64>,
            /// Offset into the role ID set (server-side offset)
            #[arg(long, default_value_t = 0)]
            offset: u64,
            /// Batch fetch size for roles query
            #[arg(long)]
            fetch_size: Option<u64>,
        },
    }

    impl Run for List {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let client = context.client_from_config();
            let me = client.account.clone();
            // Query my roles with optional pagination/fetch-size
            let mut roles_builder = client.query(FindRolesByAccountId::new(me.clone()));
            let (limit, offset, fetch_size) = match self {
                Self::All {
                    limit,
                    offset,
                    fetch_size,
                } => (limit, offset, fetch_size),
            };
            {
                if limit.is_some() || offset > 0 {
                    let pagination = iroha::data_model::query::parameters::Pagination::new(
                        limit.and_then(NonZeroU64::new),
                        offset,
                    );
                    roles_builder = roles_builder.with_pagination(pagination);
                }
                if let Some(n) = fetch_size.and_then(NonZeroU64::new) {
                    let fs = iroha::data_model::query::parameters::FetchSize::new(Some(n));
                    roles_builder = roles_builder.with_fetch_size(fs);
                }
            }
            let Ok(my_multisig_roles) = roles_builder.execute_all().map(|roles: Vec<RoleId>| {
                roles
                    .into_iter()
                    .filter(|role_id| role_id.name().as_ref().starts_with(MULTISIG_SIGNATORY))
                    .collect::<Vec<_>>()
            }) else {
                return Ok(());
            };
            let mut stack = my_multisig_roles
                .iter()
                .filter_map(multisig_account_from)
                .map(|account_id| Context::new(me.clone(), account_id, None))
                .collect();
            let mut proposals = BTreeMap::new();

            fold_proposals(&mut proposals, &mut stack, &client)?;
            context.print_data(&proposals)
        }
    }

    const DELIMITER: char = '/';
    const MULTISIG: &str = "multisig";
    const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";

    fn surface_policy_ttl<C: RunContext>(
        context: &mut C,
        multisig_account: &AccountId,
        override_ttl_ms: Option<NonZeroU64>,
    ) -> Result<()> {
        use iroha::data_model::prelude::FindAccounts;

        let client = context.client_from_config();
        let account = match client.query(FindAccounts).execute_all() {
            Ok(accounts) => {
                if let Some(account) = accounts
                    .into_iter()
                    .find(|entry| entry.id() == multisig_account)
                {
                    account
                } else {
                    context.println(format!(
                        "Unable to fetch multisig policy for {multisig_account}: account not found"
                    ))?;
                    return Ok(());
                }
            }
            Err(err) => {
                context.println(format!("Unable to fetch multisig policy for {multisig_account}: {err}"))?;
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
            emit_ttl_hint(context, ttl_ms, policy.transaction_ttl_ms, Some(multisig_account))?;
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

        let account_note = account
            .map_or_else(String::new, |id| format!(" on {id}"));
        context.println(format!(
            "Multisig TTL{account_note}: using {effective_ttl_ms} ms (policy cap {policy_ttl_ms} ms), expires approximately {expiry}"
        ))
    }

    fn spec_key() -> Name {
        format!("{MULTISIG}{DELIMITER}spec").parse().unwrap()
    }

    fn proposal_key_prefix() -> String {
        format!("{MULTISIG}{DELIMITER}proposals{DELIMITER}")
    }

    fn multisig_account_from(role: &RoleId) -> Option<AccountId> {
        role.name()
            .as_ref()
            .strip_prefix(MULTISIG_SIGNATORY)?
            .rsplit_once(DELIMITER)
            .and_then(|(init, last)| {
                format!("{last}@{}", init.trim_matches(DELIMITER))
                    .parse()
                    .ok()
            })
    }

    type PendingProposals = BTreeMap<ProposalKey, ProposalView>;

    type ProposalKey = HashOf<Vec<InstructionBox>>;

    #[derive(Debug, Clone, Default, crate::json_macros::FastJsonWrite)]
    struct ProposalView {
        instructions: Vec<InstructionBox>,
        proposed_at: String,
        expires_in: String,
        approval_path: Vec<String>,
    }

    #[derive(Debug, Display, Constructor)]
    #[display("{weight} {} [{got}/{quorum}] {target}", self.relation())]
    struct ApprovalEdge {
        weight: u8,
        has_approved: bool,
        got: u16,
        quorum: u16,
        target: AccountId,
    }

    impl ApprovalEdge {
        fn relation(&self) -> &str {
            if self.has_approved { "joined" } else { "->" }
        }
    }

    #[derive(Debug, Constructor, Clone, PartialEq, Eq)]
    struct Context {
        child: AccountId,
        this: AccountId,
        key_span: Option<(ProposalKey, ProposalKey)>,
    }

    fn fold_proposals(
        proposals: &mut PendingProposals,
        stack: &mut Vec<Context>,
        client: &Client,
    ) -> Result<()> {
        let mut fetch = |account_id: &AccountId| {
            client
                .query(FindAccounts)
                .execute_all()?
                .into_iter()
                .find(|account| account.id() == account_id)
                .ok_or_else(|| eyre!("Account not found"))
        };
        fold_proposals_with(proposals, stack, &mut fetch)
    }

    fn fold_proposals_with<F>(
        proposals: &mut PendingProposals,
        stack: &mut Vec<Context>,
        fetch: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&AccountId) -> Result<Account>,
    {
        let Some(context) = stack.pop() else {
            return Ok(());
        };
        let account = fetch(&context.this)?;
        let Some(spec_value) = account.metadata().get(&spec_key()) else {
            return fold_proposals_with(proposals, stack, fetch);
        };
        let spec: MultisigSpec = spec_value.clone().try_into_any_norito()?;
        for (proposal_key, proposal_value) in account
            .metadata()
            .iter()
            .filter_map(|(k, v)| {
                k.as_ref().strip_prefix(&proposal_key_prefix()).map(|k| {
                    (
                        k.parse::<ProposalKey>().unwrap(),
                        v.try_into_any_norito::<MultisigProposalValue>().unwrap(),
                    )
                })
            })
            .filter(|(k, _v)| context.key_span.is_none_or(|(_, top)| *k == top))
        {
            process_proposal(
                proposals,
                stack,
                &context,
                &proposal_key,
                &proposal_value,
                &spec,
            );
        }
        fold_proposals_with(proposals, stack, fetch)
    }

    fn process_proposal(
        proposals: &mut PendingProposals,
        stack: &mut Vec<Context>,
        context: &Context,
        proposal_key: &ProposalKey,
        proposal_value: &MultisigProposalValue,
        spec: &MultisigSpec,
    ) {
        let root_key = context
            .key_span
            .as_ref()
            .map_or(*proposal_key, |(leaf, _)| *leaf);

        let mut is_root_proposal = context.key_span.is_none();

        for instruction in &proposal_value.instructions {
            if let Some(MultisigInstructionBox::Approve(approve)) =
                decode_multisig_instruction(instruction)
            {
                let next_context = Context::new(
                    context.this.clone(),
                    approve.account.clone(),
                    Some((root_key, approve.instructions_hash)),
                );
                if !stack.contains(&next_context) {
                    stack.push(next_context);
                }
                is_root_proposal = false;
            }
        }

        let proposal_status = proposals.entry(root_key).or_default();

        let edge = ApprovalEdge::new(
            *spec.signatories.get(&context.child).unwrap(),
            proposal_value.approvals.contains(&context.child),
            spec.signatories
                .iter()
                .filter(|(id, _)| proposal_value.approvals.contains::<ivm::AccountId>(id))
                .map(|(_, weight)| u16::from(*weight))
                .sum(),
            spec.quorum.into(),
            context.this.clone(),
        );
        proposal_status.approval_path.push(format!("{edge}"));

        if is_root_proposal {
            proposal_status
                .instructions
                .clone_from(&proposal_value.instructions);
            proposal_status.proposed_at = {
                let proposed_at = Duration::from_secs(
                    Duration::from_millis(proposal_value.proposed_at_ms).as_secs(),
                );
                let timestamp = SystemTime::UNIX_EPOCH.checked_add(proposed_at).unwrap();
                humantime::Timestamp::from(timestamp).to_string()
            };
            proposal_status.expires_in = {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap();
                let expires_at = Duration::from_millis(proposal_value.expires_at_ms);
                humantime::Duration::from(Duration::from_secs(
                    expires_at.saturating_sub(now).as_secs(),
                ))
                .to_string()
            };
        }
    }

    fn decode_multisig_instruction(instruction: &InstructionBox) -> Option<MultisigInstructionBox> {
        let custom = instruction.as_any().downcast_ref::<CustomInstruction>()?;
        MultisigInstructionBox::try_from(&custom.payload).ok()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use iroha::crypto::{Algorithm, KeyPair};
        use iroha::data_model::{
            Level,
            account::{Account, AccountId},
            domain::DomainId,
            isi::Log,
        };
        use iroha_crypto::HashOf;
        use std::{
            collections::{BTreeMap, BTreeSet},
            num::{NonZeroU16, NonZeroU64},
        };

        fn account_from_seed(seed: u8, domain: &DomainId) -> AccountId {
            let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            AccountId::new(domain.clone(), key_pair.public_key().clone())
        }

        #[test]
        fn fold_proposals_skips_accounts_without_spec_metadata() {
            let domain: DomainId = "wonderland".parse().unwrap();
            let account_id = account_from_seed(9, &domain);
            let account = Account::new(account_id.clone()).build(&account_id);

            let mut accounts = BTreeMap::new();
            accounts.insert(account_id.clone(), account);

            let mut proposals = BTreeMap::new();
            let mut stack = vec![Context::new(account_id.clone(), account_id.clone(), None)];
            let mut fetch = |id: &AccountId| {
                accounts
                    .get(id)
                    .cloned()
                    .ok_or_else(|| eyre!("Account not found in test map"))
            };

            fold_proposals_with(&mut proposals, &mut stack, &mut fetch)
                .expect("fold proposals");
            assert!(proposals.is_empty());
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
        fn process_proposal_enqueues_relay_context() {
            let domain: DomainId = "wonderland".parse().unwrap();
            let parent_account = account_from_seed(1, &domain);
            let current_account = account_from_seed(2, &domain);
            let child_account = account_from_seed(3, &domain);

            let root_instructions = vec![InstructionBox::from(Log::new(
                Level::INFO,
                "root".to_string(),
            ))];
            let root_key = HashOf::new(&root_instructions);

            let child_instructions = vec![InstructionBox::from(Log::new(
                Level::INFO,
                "child".to_string(),
            ))];
            let child_hash = HashOf::new(&child_instructions);

            let relay_instruction: InstructionBox =
                MultisigApprove::new(child_account.clone(), child_hash).into();
            let relay_instructions = vec![relay_instruction.clone()];
            let current_key = HashOf::new(&relay_instructions);
            let proposal_value = MultisigProposalValue::new(
                relay_instructions,
                1_000,
                2_000,
                BTreeSet::new(),
                Some(false),
            );

            let mut signatories = BTreeMap::new();
            signatories.insert(parent_account.clone(), 3);
            let spec = MultisigSpec::new(
                signatories,
                NonZeroU16::new(5).unwrap(),
                NonZeroU64::new(60_000).unwrap(),
            );

            let mut proposals = PendingProposals::new();
            proposals.insert(root_key, ProposalView::default());
            let mut stack = Vec::new();

            let context = Context::new(
                parent_account.clone(),
                current_account.clone(),
                Some((root_key, current_key)),
            );

            process_proposal(
                &mut proposals,
                &mut stack,
                &context,
                &current_key,
                &proposal_value,
                &spec,
            );

            assert_eq!(stack.len(), 1);
            let expected = Context::new(
                current_account.clone(),
                child_account.clone(),
                Some((root_key, child_hash)),
            );
            assert_eq!(stack.pop().unwrap(), expected);

            let view = proposals.get(&root_key).unwrap();
            assert!(view.instructions.is_empty());
            assert_eq!(view.approval_path.len(), 1);
            assert!(view.approval_path[0].contains(current_account.to_string().as_str()));
        }

        #[test]
        fn process_proposal_records_root_instructions() {
            let domain: DomainId = "wonderland".parse().unwrap();
            let user_account = account_from_seed(7, &domain);
            let current_account = account_from_seed(9, &domain);

            let root_instructions = vec![InstructionBox::from(Log::new(
                Level::INFO,
                "execute".to_string(),
            ))];
            let root_key = HashOf::new(&root_instructions);

            let mut approvals = BTreeSet::new();
            approvals.insert(user_account.clone());
            let proposal_value = MultisigProposalValue::new(
                root_instructions.clone(),
                5_000,
                10_000,
                approvals,
                None,
            );

            let mut signatories = BTreeMap::new();
            signatories.insert(user_account.clone(), 4);
            let spec = MultisigSpec::new(
                signatories,
                NonZeroU16::new(6).unwrap(),
                NonZeroU64::new(90_000).unwrap(),
            );

            let mut proposals = PendingProposals::new();
            let mut stack = Vec::new();
            let context = Context::new(user_account.clone(), current_account.clone(), None);

            process_proposal(
                &mut proposals,
                &mut stack,
                &context,
                &root_key,
                &proposal_value,
                &spec,
            );

            assert!(stack.is_empty());
            let view = proposals.get(&root_key).unwrap();
            assert_eq!(view.instructions, root_instructions);
            assert!(!view.proposed_at.is_empty());
            assert!(!view.expires_in.is_empty());
            assert_eq!(view.approval_path.len(), 1);
            assert!(view.approval_path[0].contains("joined"));
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
            // Read stdin
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            let envelope: QueryEnvelopeJson =
                norito::json::from_str(&buf).wrap_err("decode query envelope")?;
            let with_auth = envelope
                .into_signed_request(client.account.clone())
                .map_err(|err| eyre!(format!("invalid query JSON: {err}")))?;
            let signed = with_auth.sign(&client.key_pair);
            let payload = norito::codec::Encode::encode(&signed);
            let resp = client.execute_signed_query_raw(&payload)?;
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
            use iroha::data_model::query::{QueryRequest, QueryRequestWithAuthority};
            let client = context.client_from_config();
            let bytes = decode_base64_or_hex(
                &self.cursor,
                "invalid hex length for ForwardCursor",
                "invalid cursor hex",
            )?;
            let cursor: iroha::data_model::query::parameters::ForwardCursor =
                norito::decode_from_bytes(&bytes).wrap_err("decode ForwardCursor")?;
            let req = QueryRequest::Continue(cursor);
            let with_auth = QueryRequestWithAuthority {
                authority: client.account.clone(),
                request: req,
            };
            let signed = with_auth.sign(&client.key_pair);
            let payload = norito::codec::Encode::encode(&signed);
            let resp = client.execute_signed_query_raw(&payload)?;
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
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
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
    use iroha::data_model::{Level as LogLevel, isi::Log};
    use std::{
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
    };

    use super::*;

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Retrieve details of a specific transaction
        Get(Get),
        /// Send an empty transaction that logs a message
        Ping(Ping),
        /// Send a transaction using IVM bytecode
        Ivm(Ivm),
        /// Send a transaction using JSON input from stdin
        Stdin(Stdin),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            use self::Command::*;
            match self {
                Get(cmd) => cmd.run(context),
                Ping(cmd) => cmd.run(context),
                Ivm(cmd) => cmd.run(context),
                Stdin(cmd) => cmd.run(context),
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
        #[arg(short, long, default_value = "INFO")]
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

    pub(crate) const DEFAULT_PING_PARALLEL_CAP: usize = 1024;

    fn ping_message(base: &str, index: usize, count: usize, no_index: bool) -> String {
        if count <= 1 || no_index {
            return base.to_owned();
        }
        format!("{base}-{}", index + 1)
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
                let client = Client::new(context.config().clone());
                let metadata = context.transaction_metadata().cloned().unwrap_or_default();
                let i18n = context.i18n().clone();
                let base_msg = msg;
                let result = dispatch_ping_work(count, parallel, move || {
                    let client = client.clone();
                    let metadata = metadata.clone();
                    let i18n = i18n.clone();
                    let base_msg = base_msg.clone();
                    move |index| {
                        let message = ping_message(&base_msg, index, count, no_index);
                        let instruction = Log::new(log_level, message);
                        let transaction = client.build_transaction([instruction], metadata.clone());
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
                        return Err(err.wrap_err(format!(
                            "{} ping submissions failed",
                            result.failed
                        )));
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
    }

    #[derive(clap::Args, Debug)]
    pub struct Ivm {
        /// Path to the IVM bytecode file. If omitted, reads from stdin
        #[arg(short, long)]
        path: Option<PathBuf>,
    }

    impl Run for Ivm {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let blob = if let Some(path) = self.path {
                fs::read(path)
                    .wrap_err("Failed to read IVM bytecode from the file into the buffer")?
            } else {
                bytes_from_stdin()
                    .wrap_err("Failed to read IVM bytecode from stdin into the buffer")?
            };

            context
                .finish(IvmBytecode::from_compiled(blob))
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
                    let entries = client
                        .query(FindTriggers)
                        .execute_all()
                        .wrap_err("Failed to get trigger")?;
                    let entry = entries
                        .into_iter()
                        .find(|t| t.id() == &args.id)
                        .ok_or_else(|| eyre!("Trigger not found"))?;
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
                Meta(cmd) => cmd.run(context),
            }
        }
    }

    #[derive(clap::Subcommand, Debug)]
    pub enum List {
        /// List all trigger IDs
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
        /// Account executing the trigger (default: current config account)
        #[arg(long)]
        pub authority: Option<AccountId>,
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
        #[arg(long)]
        pub data_domain: Option<DomainId>,
        /// Data filter preset: events for an account
        #[arg(long)]
        pub data_account: Option<AccountId>,
        /// Data filter preset: events for an asset
        #[arg(long)]
        pub data_asset: Option<AssetId>,
        /// Data filter preset: events for an asset definition
        #[arg(long)]
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
                    let bc = fs::read(path)
                        .map(IvmBytecode::from_compiled)
                        .wrap_err("Failed to read IVM bytecode from the file")?;
                    Executable::from(bc)
                }
                (None, Some(file), false) => {
                    let s =
                        fs::read_to_string(file).wrap_err("Failed to read instructions file")?;
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
            let authority = self
                .authority
                .unwrap_or_else(|| context.config().account.clone());
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
                    } else if let Some(acc) = self.data_account.clone() {
                        DataEventFilter::from(
                            iroha::data_model::events::data::prelude::AccountEventFilter::new()
                                .for_account(acc),
                        )
                    } else if let Some(asset) = self.data_asset.clone() {
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
                        // Parse `<backend>:<name>` into VerifyingKeyId
                        let mut parts = vk_spec.splitn(2, ':');
                        let Some(backend) = parts.next() else {
                            eyre::bail!("--data-verifying-key requires BACKEND:NAME format")
                        };
                        let Some(name) = parts.next() else {
                            eyre::bail!("--data-verifying-key requires BACKEND:NAME format")
                        };
                        let id = iroha::data_model::proof::VerifyingKeyId::new(
                            backend.to_string(),
                            name.to_string(),
                        );
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
                        // Parse `<backend>:<HEX>` into ProofId
                        let mut parts = pf_spec.splitn(2, ':');
                        let Some(backend) = parts.next() else {
                            eyre::bail!("--data-proof requires BACKEND:HEX format")
                        };
                        let Some(hex_hash) = parts.next() else {
                            eyre::bail!("--data-proof requires BACKEND:HEX format")
                        };
                        let bytes = {
                            let s = hex_hash.trim_start_matches("0x");
                            if s.len() % 2 != 0 {
                                return Err(eyre!("invalid hex length for proof hash"));
                            }
                            let mut v = Vec::with_capacity(s.len() / 2);
                            let mut i = 0;
                            while i < s.len() {
                                let byte = u8::from_str_radix(&s[i..i + 2], 16)
                                    .map_err(|e| eyre!("invalid hex for proof hash: {e}"))?;
                                v.push(byte);
                                i += 2;
                            }
                            v
                        };
                        if bytes.len() != 32 {
                            eyre::bail!("proof hash must be 32 bytes (64 hex chars)")
                        }
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&bytes);
                        let pid = iroha::data_model::proof::ProofId {
                            backend: backend.to_string(),
                            proof_hash: arr,
                        };
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
            );
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
            Executable::Ivm(bytecode) => {
                let mut inner = BTreeMap::<String, Value>::new();
                inner.insert("hash".into(), to_value(&HashOf::new(bytecode))?);
                inner.insert("size_bytes".into(), to_value(&bytecode.size_bytes())?);
                let mut outer = BTreeMap::<String, Value>::new();
                outer.insert("Ivm".into(), Value::Object(inner));
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
                    let instruction = fs::read(args.path)
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
            #[arg(short, long)]
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
            pub id: AccountId,
            #[arg(short, long)]
            pub key: Name,
        }

        impl Run for Command {
            fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
                use self::Command::*;
                match self {
                    Get(args) => {
                        let client = context.client_from_config();
                        let entries: Vec<Account> = client
                            .query(FindAccounts)
                            .execute_all()
                            .wrap_err("Failed to get value")?;
                        let entry = entries
                            .into_iter()
                            .find(|e| e.id() == &args.id)
                            .ok_or_else(|| eyre!("Account not found"))?;
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
                            iroha::data_model::isi::SetKeyValue::account(args.id, args.key, value);
                        context.finish([instruction])
                    }
                    Remove(args) => {
                        let instruction =
                            iroha::data_model::isi::RemoveKeyValue::account(args.id, args.key);
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
                        let entries: Vec<Trigger> = client
                            .query(FindTriggers)
                            .execute_all()
                            .wrap_err("Failed to get value")?;
                        let entry = entries
                            .into_iter()
                            .find(|e| e.id() == &args.id)
                            .ok_or_else(|| eyre!("Trigger not found"))?;
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
        prelude::{AccountId, AssetDefinitionId, Numeric},
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
        pub initiator: AccountId,
        /// Counterparty receiving the repo cash leg
        #[arg(long)]
        pub counterparty: AccountId,
        /// Optional custodian account holding pledged collateral in tri-party agreements
        #[arg(long)]
        pub custodian: Option<AccountId>,
        /// Cash asset definition identifier
        #[arg(long)]
        pub cash_asset: AssetDefinitionId,
        /// Cash quantity exchanged at initiation (integer or decimal)
        #[arg(long)]
        pub cash_quantity: Numeric,
        /// Collateral asset definition identifier
        #[arg(long)]
        pub collateral_asset: AssetDefinitionId,
        /// Collateral quantity pledged at initiation (integer or decimal)
        #[arg(long)]
        pub collateral_quantity: Numeric,
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
                self.initiator,
                self.counterparty,
                self.custodian,
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
        /// Stable identifier assigned to the repo agreement lifecycle
        #[arg(long)]
        pub agreement_id: RepoAgreementId,
        /// Initiating account performing the unwind
        #[arg(long)]
        pub initiator: AccountId,
        /// Counterparty receiving the unwind settlement
        #[arg(long)]
        pub counterparty: AccountId,
        /// Cash asset definition identifier
        #[arg(long)]
        pub cash_asset: AssetDefinitionId,
        /// Cash quantity returned at unwind (integer or decimal)
        #[arg(long)]
        pub cash_quantity: Numeric,
        /// Collateral asset definition identifier
        #[arg(long)]
        pub collateral_asset: AssetDefinitionId,
        /// Collateral quantity released at unwind (integer or decimal)
        #[arg(long)]
        pub collateral_quantity: Numeric,
        /// Unix timestamp (milliseconds) when the unwind was agreed
        #[arg(long)]
        pub settlement_timestamp_ms: u64,
    }

    impl Unwind {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let cash_leg = RepoCashLeg {
                asset_definition_id: self.cash_asset,
                quantity: self.cash_quantity,
            };
            let collateral_leg = RepoCollateralLeg {
                asset_definition_id: self.collateral_asset,
                quantity: self.collateral_quantity,
                metadata: Metadata::default(),
            };
            let instruction = ReverseRepoIsi::new(
                self.agreement_id,
                self.initiator,
                self.counterparty,
                cash_leg,
                collateral_leg,
                self.settlement_timestamp_ms,
            );
            let instruction: RepoInstructionBox = instruction.into();
            context
                .finish([InstructionBox::from(instruction)])
                .wrap_err("Failed to unwind repo agreement")
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
    use super::*;
    use clap::ValueEnum;
    use iroha::data_model::{
        isi::{
            InstructionBox,
            settlement::{
                DvpIsi, PvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementId,
                SettlementInstructionBox, SettlementLeg, SettlementPlan,
            },
        },
        metadata::Metadata,
        prelude::{AccountId, AssetDefinitionId, Numeric},
    };

    #[derive(clap::Subcommand, Debug)]
    pub enum Command {
        /// Create a delivery-versus-payment instruction
        Dvp(DvpArgs),
        /// Create a payment-versus-payment instruction
        Pvp(PvpArgs),
    }

    impl Run for Command {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            match self {
                Command::Dvp(args) => args.run(context),
                Command::Pvp(args) => args.run(context),
            }
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
        pub delivery_quantity: Numeric,
        /// Account delivering the asset
        #[arg(long)]
        pub delivery_from: AccountId,
        /// Account receiving the delivery leg
        #[arg(long)]
        pub delivery_to: AccountId,
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
        pub payment_quantity: Numeric,
        /// Account sending the payment leg
        #[arg(long)]
        pub payment_from: AccountId,
        /// Account receiving the payment leg
        #[arg(long)]
        pub payment_to: AccountId,
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
        /// Optional path to emit a sese.023 XML preview of the settlement
        #[arg(long = "iso-xml-out")]
        pub iso_xml_out: Option<std::path::PathBuf>,
    }

    impl DvpArgs {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let plan = SettlementPlan::new(self.order.into(), self.atomicity.to_model());

            let delivery_leg = SettlementLeg::new(
                self.delivery_asset,
                self.delivery_quantity,
                self.delivery_from,
                self.delivery_to,
            );
            let payment_leg = SettlementLeg::new(
                self.payment_asset,
                self.payment_quantity,
                self.payment_from,
                self.payment_to,
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
        pub primary_quantity: Numeric,
        /// Account delivering the primary currency
        #[arg(long)]
        pub primary_from: AccountId,
        /// Account receiving the primary currency
        #[arg(long)]
        pub primary_to: AccountId,
        /// Counter currency leg asset definition
        #[arg(long)]
        pub counter_asset: AssetDefinitionId,
        /// Quantity of the counter currency (integer or decimal)
        #[arg(long)]
        pub counter_quantity: Numeric,
        /// Account delivering the counter currency
        #[arg(long)]
        pub counter_from: AccountId,
        /// Account receiving the counter currency
        #[arg(long)]
        pub counter_to: AccountId,
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
        /// Optional path to emit a sese.025 XML preview of the settlement
        #[arg(long = "iso-xml-out")]
        pub iso_xml_out: Option<std::path::PathBuf>,
    }

    impl PvpArgs {
        fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
            let plan = SettlementPlan::new(self.order.into(), self.atomicity.to_model());

            let primary_leg = SettlementLeg::new(
                self.primary_asset,
                self.primary_quantity,
                self.primary_from,
                self.primary_to,
            );
            let counter_leg = SettlementLeg::new(
                self.counter_asset,
                self.counter_quantity,
                self.counter_from,
                self.counter_to,
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
            ReferenceDataError, ReferenceDataSnapshots, SnapshotState, ValidationOutcome,
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
        }

        impl Default for SettlementPreviewOptions {
            fn default() -> Self {
                Self {
                    hold_indicator: false,
                    partial_indicator: PartialIndicatorArg::Npar,
                    settlement_condition: None,
                    place_of_settlement_mic: None,
                    linkages: Vec::new(),
                }
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
                        Ok(ValidationOutcome::Enforced | ValidationOutcome::Skipped) => {}
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
                    currency_code(isi.payment_leg.asset_definition_id()).as_bytes(),
                );
                msg_set("SttlmDt", settlement_date_string().as_bytes());
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
            }
        }

        fn settlement_date_string() -> String {
            OffsetDateTime::now_utc()
                .date()
                .format(&Iso8601::DATE)
                .unwrap_or_else(|_| "1970-01-01".to_string())
        }

        pub fn pvp_to_sese025(isi: &PvpIsi, options: &SettlementPreviewOptions) -> Result<String> {
            iso_scope(|| {
                msg_create("sese.025");
                msg_set("TxId", isi.settlement_id.to_string().as_bytes());
                msg_set("SttlmTpAndAddtlParams/SctiesMvmntTp", b"RECE");
                msg_set("SttlmTpAndAddtlParams/Pmt", b"APMT");
                msg_set("SttlmDt", settlement_date_string().as_bytes());
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
                    currency_code(isi.primary_leg.asset_definition_id()).as_bytes(),
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
            let mut country: String = account
                .domain()
                .name()
                .as_ref()
                .chars()
                .filter(char::is_ascii_alphabetic)
                .take(2)
                .map(|c| c.to_ascii_uppercase())
                .collect();
            while country.len() < 2 {
                country.push('X');
            }
            let mut location: String = account
                .signatory()
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

        fn currency_code(asset: &AssetDefinitionId) -> String {
            asset.name().as_ref().to_ascii_uppercase()
        }

        fn counter_info(leg: &SettlementLeg) -> String {
            format!(
                "{{\"counter_currency\":\"{}\",\"amount\":\"{}\"}}",
                currency_code(leg.asset_definition_id()),
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
            use iroha_data_model::domain::DomainId;
            use iroha_primitives::numeric::Numeric;
            use std::io::Write;
            use tempfile::NamedTempFile;

            fn account_with_seed(domain: &DomainId, seed: u8) -> AccountId {
                let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
                AccountId::new(domain.clone(), key_pair.public_key().clone())
            }

            fn sample_dvp() -> DvpIsi {
                let domain: DomainId = "wonderland".parse().unwrap();
                let seller = account_with_seed(&domain, 0x11);
                let buyer = account_with_seed(&domain, 0x22);
                let payer = account_with_seed(&domain, 0x33);
                let receiver = account_with_seed(&domain, 0x44);

                let delivery_leg = SettlementLeg::new(
                    "bond#wonderland".parse().unwrap(),
                    Numeric::new(100, 0),
                    seller,
                    buyer,
                );
                let payment_leg = SettlementLeg::new(
                    "usd#wonderland".parse().unwrap(),
                    Numeric::new(1000, 0),
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
                let domain: DomainId = "wonderland".parse().unwrap();
                let payer = account_with_seed(&domain, 0x55);
                let receiver = account_with_seed(&domain, 0x66);
                let counter_payer = account_with_seed(&domain, 0x77);
                let counter_receiver = account_with_seed(&domain, 0x88);

                let primary_leg = SettlementLeg::new(
                    "usd#wonderland".parse().unwrap(),
                    Numeric::new(1000, 0),
                    payer,
                    receiver,
                );
                let counter_leg = SettlementLeg::new(
                    "eur#wonderland".parse().unwrap(),
                    Numeric::new(900, 0),
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
            fn dvp_preview_reports_currency_validation_error() {
                let snapshots = crosswalk_snapshot(
                    r#"{
                        "version":"2024-05-01",
                        "source":"ANNA",
                        "entries":[{"isin":"US0378331005"}]
                    }"#,
                );

                let domain: DomainId = "wonderland".parse().unwrap();
                let dvp = DvpIsi {
                    settlement_id: "dvp_settlement".parse().unwrap(),
                    delivery_leg: SettlementLeg::new(
                        "bond#wonderland".parse().unwrap(),
                        Numeric::new(100, 0),
                        account_with_seed(&domain, 0x55),
                        account_with_seed(&domain, 0x66),
                    ),
                    payment_leg: SettlementLeg::new(
                        "doge#wonderland".parse().unwrap(),
                        Numeric::new(1000, 0),
                        account_with_seed(&domain, 0x77),
                        account_with_seed(&domain, 0x88),
                    ),
                    plan: SettlementPlan::new(
                        SettlementExecutionOrder::DeliveryThenPayment,
                        SettlementAtomicity::AllOrNothing,
                    ),
                    metadata: Metadata::default(),
                };

                let err = dvp_to_sese023(
                    &dvp,
                    Some("US0378331005"),
                    Some(&snapshots),
                    &SettlementPreviewOptions::default(),
                )
                .expect_err("invalid currency must be rejected");
                let msg = err.to_string();
                assert!(
                    msg.contains("invalid ISO 4217 currency value for field `CashLeg/Ccy`"),
                    "unexpected error message: {msg}"
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
    norito::json::from_json(s).map_err(|err| eyre!("failed to parse JSON: {err}"))
}

fn string_from_stdin() -> Result<String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

fn bytes_from_stdin() -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf)?;
    Ok(buf)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_macros::JsonSerialize;
    use eyre::eyre;
    use futures::stream;
    use iroha::crypto::{Algorithm, KeyPair};
    use iroha::data_model::{
        ChainId,
        Level,
        events::{data::DataEventFilter, execute_trigger::ExecuteTriggerEventFilter, EventFilterBox},
        isi::Log,
        metadata::Metadata,
        transaction::Executable,
    };
    use iroha_i18n::Language;
    use std::{
        fs,
        num::NonZeroU64,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tokio::runtime::Runtime;
    use url::Url;

    #[derive(Clone, Copy, JsonSerialize)]
    struct DummyEvent;

    #[test]
    fn stream_timeout_driver_propagates_errors() {
        let mut stream = stream::iter(vec![Result::<DummyEvent, eyre::Report>::Err(eyre!(
            "connection failed"
        ))]);
        let mut processed = 0usize;
        let rt = Runtime::new().expect("runtime");
        let result = rt.block_on(async {
            drive_try_stream_until_timeout(
                &mut stream,
                |_event| -> Result<()> {
                    processed += 1;
                    Ok(())
                },
                Duration::from_millis(1),
                "timeout",
            )
            .await
        });
        let err = result.expect_err("stream error should propagate");
        assert!(err.to_string().contains("connection failed"));
        assert_eq!(processed, 0);
    }

    #[test]
    fn account_admission_rejected_message_includes_hint() {
        let i18n = Localizer::new(Bundle::Cli, Language::English);
        let message = account_admission_rejected_message("hint text", &i18n);
        assert!(message.contains("Account admission rejected"));
        assert!(message.contains("hint text"));
    }

    #[test]
    fn listen_events_message_formats_context() {
        let i18n = Localizer::new(Bundle::Cli, Language::English);
        let filter = EventFilterBox::Data(DataEventFilter::Any);
        let timeout = Duration::from_secs(1);
        let message = listen_events_message(&filter, Some(timeout), &i18n);
        let expected = format!(
            "Listening to events with filter: {filter:?} and timeout: {timeout:?}"
        );
        assert_eq!(message, expected);

        let message = listen_events_message(&filter, None, &i18n);
        let expected = format!("Listening to events with filter: {filter:?}");
        assert_eq!(message, expected);
    }

    #[test]
    fn listen_blocks_message_formats_context() {
        let i18n = Localizer::new(Bundle::Cli, Language::English);
        let height = NonZeroU64::new(7).expect("height");
        let timeout = Duration::from_secs(2);
        let message = listen_blocks_message(height, Some(timeout), &i18n);
        let expected =
            format!("Listening to blocks from height: {height} and timeout: {timeout:?}");
        assert_eq!(message, expected);

        let message = listen_blocks_message(height, None, &i18n);
        let expected = format!("Listening to blocks from height: {height}");
        assert_eq!(message, expected);
    }

    #[test]
    fn help_text_localization_preserves_headings_when_translation_matches() {
        let i18n = Localizer::new(Bundle::Cli, Language::English);
        let raw = "Usage:\nOptions:\n";
        let localized = localize_help_text(raw, &i18n);
        assert_eq!(localized, raw);
        assert!(!localized.contains("help.heading.usage"));
    }

    #[test]
    fn language_override_from_args_parses_flags() {
        let args = vec!["--language".to_string(), "ja".to_string()];
        assert_eq!(
            language_override_from_args(args),
            Some("ja".to_string())
        );

        let args = vec!["--language=fr".to_string()];
        assert_eq!(
            language_override_from_args(args),
            Some("fr".to_string())
        );
    }

    struct CaptureContext {
        cfg: iroha::config::Config,
        captured: Option<Executable>,
        i18n: Localizer,
    }

    impl CaptureContext {
        fn new(account: AccountId) -> Self {
            let key_pair = KeyPair::from_seed(vec![0u8; 32], Algorithm::Ed25519);
            let cfg = iroha::config::Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                account,
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                captured: None,
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for CaptureContext {
        fn config(&self) -> &iroha::config::Config {
            &self.cfg
        }

        fn transaction_metadata(&self) -> Option<&Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn print_data<T>(&mut self, _data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            Ok(())
        }

        fn println(&mut self, _data: impl std::fmt::Display) -> Result<()> {
            Ok(())
        }

        fn submit_with_metadata(
            &mut self,
            instructions: impl Into<Executable>,
            _metadata: Metadata,
            _wait_for_confirmation: bool,
        ) -> Result<()> {
            self.captured = Some(instructions.into());
            Ok(())
        }

        fn submit(&mut self, instructions: impl Into<Executable>) -> Result<()> {
            self.captured = Some(instructions.into());
            Ok(())
        }
    }

    #[test]
    fn ping_rejects_zero_count() {
        let account: AccountId =
            "ed25519:ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C@wonderland"
                .parse()
                .expect("account");
        let mut ctx = CaptureContext::new(account);
        let cmd = transaction::Ping {
            log_level: Level::INFO,
            msg: "ping".to_string(),
            count: 0,
            parallel: 1,
            parallel_cap: transaction::DEFAULT_PING_PARALLEL_CAP,
            no_wait: false,
            no_index: false,
        };
        let err = cmd.run(&mut ctx).expect_err("count must be rejected");
        assert!(err.to_string().contains("`--count` must be greater than zero"));
    }

    #[test]
    fn ping_submits_single_log_instruction() {
        let account: AccountId =
            "ed25519:ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C@wonderland"
                .parse()
                .expect("account");
        let mut ctx = CaptureContext::new(account);
        let cmd = transaction::Ping {
            log_level: Level::WARN,
            msg: "hello".to_string(),
            count: 1,
            parallel: 1,
            parallel_cap: transaction::DEFAULT_PING_PARALLEL_CAP,
            no_wait: false,
            no_index: false,
        };
        cmd.run(&mut ctx).expect("ping run");
        let exec = ctx.captured.expect("captured instructions");
        let instructions = match exec {
            Executable::Instructions(instructions) => instructions.into_vec(),
            Executable::Ivm(_) => panic!("expected instructions"),
        };
        assert_eq!(instructions.len(), 1);
        let log = instructions[0]
            .as_any()
            .downcast_ref::<Log>()
            .expect("log instruction");
        assert_eq!(log.level, Level::WARN);
        assert_eq!(log.msg, "hello");
    }

    #[test]
    fn admission_hint_reports_disabled_domain() {
        use iroha::data_model::isi::error::AccountAdmissionError;

        let domain: DomainId = "wonderland".parse().expect("domain");
        let err =
            eyre::Report::from(AccountAdmissionError::ImplicitAccountCreationDisabled(domain));
        let hint = account_admission_hint(err.as_ref()).expect("hint should be present");
        assert!(
            hint.contains("Implicit account creation is disabled"),
            "unexpected hint: {hint}"
        );
        assert!(hint.contains("wonderland"), "hint must reference domain: {hint}");
    }

    #[test]
    fn admission_hint_reports_quota_scope() {
        use iroha::data_model::isi::error::{
            AccountAdmissionError, AccountAdmissionQuotaExceeded, AccountAdmissionQuotaScope,
        };

        let err = eyre::Report::from(AccountAdmissionError::QuotaExceeded(
            AccountAdmissionQuotaExceeded {
                scope: AccountAdmissionQuotaScope::Transaction,
                created: 3,
                cap: 2,
            },
        ));
        let hint = account_admission_hint(err.as_ref()).expect("hint should be present");
        assert!(hint.contains("quota"), "unexpected hint: {hint}");
        assert!(
            hint.contains("transaction"),
            "quota scope should be surfaced: {hint}"
        );
    }

    #[test]
    fn admission_rejected_message_includes_hint() {
        let i18n = Localizer::new(Bundle::Cli, Language::English);
        let message = account_admission_rejected_message("hint text", &i18n);
        assert!(
            message.contains("Account admission rejected"),
            "unexpected message: {message}"
        );
        assert!(message.contains("hint text"), "unexpected message: {message}");
    }

    #[test]
    fn listen_messages_include_timeout_context() {
        let i18n = Localizer::new(Bundle::Cli, Language::English);
        let filter = EventFilterBox::ExecuteTrigger(ExecuteTriggerEventFilter::new());
        let message = listen_events_message(&filter, Some(Duration::from_secs(1)), &i18n);
        assert!(
            message.contains("Listening to events"),
            "unexpected message: {message}"
        );
        assert!(message.contains("timeout"), "unexpected message: {message}");

        let height = NonZeroU64::new(7).expect("height");
        let message = listen_blocks_message(height, Some(Duration::from_secs(2)), &i18n);
        assert!(
            message.contains("Listening to blocks"),
            "unexpected message: {message}"
        );
        assert!(message.contains("timeout"), "unexpected message: {message}");
    }

    #[test]
    fn trigger_register_builds_expected_instruction() {
        // Prepare a tiny bytecode blob file
        let dir = std::env::temp_dir();
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = dir.join(format!("iroha_cli_trigger_test_{ts}.to"));
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(&[1, 2, 3, 4]).unwrap();

        // Use a deterministic account for authority
        let account: AccountId =
            "ed25519:ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C@wonderland"
                .parse()
                .unwrap();
        let mut ctx = CaptureContext::new(account.clone());

        let args = trigger::Register {
            id: "my_trigger".parse().unwrap(),
            path: Some(path),
            instructions_stdin: false,
            instructions: None,
            repeats: Some(2),
            authority: None,
            filter: trigger::FilterType::Execute,
            time_start_ms: None,
            time_period_ms: None,
            data_filter: None,
            data_domain: None,
            data_account: None,
            data_asset: None,
            data_asset_definition: None,
            data_role: None,
            data_trigger: None,
            data_verifying_key: None,
            data_proof: None,
            data_proof_only: None,
            data_vk_only: None,
            time_start: None,
            time_start_rfc3339: None,
        };

        args.run(&mut ctx).expect("run ok");

        let exec = ctx.captured.expect("captured");
        let Executable::Instructions(instructions) = exec else {
            panic!("expected instructions executable");
        };
        assert_eq!(instructions.len(), 1);

        let ib = &instructions[0];
        let any: &dyn iroha::data_model::isi::Instruction = &**ib;
        let reg = any
            .as_any()
            .downcast_ref::<iroha::data_model::isi::RegisterBox>()
            .expect("register instruction");

        let iroha::data_model::isi::RegisterBox::Trigger(reg_tr) = reg else {
            panic!("expected trigger register")
        };
        let trig = reg_tr.object();
        assert_eq!(trig.id(), &"my_trigger".parse().unwrap());
        assert_eq!(
            trig.action().repeats(),
            iroha::data_model::trigger::action::Repeats::Exactly(2)
        );
        assert_eq!(trig.action().authority(), &account);

        match trig.action().filter() {
            iroha::data_model::events::EventFilterBox::ExecuteTrigger(f) => {
                let expected = ExecuteTriggerEventFilter::new()
                    .for_trigger("my_trigger".parse().unwrap())
                    .under_authority(account);
                assert_eq!(f, &expected);
            }
            _ => panic!("expected ExecuteTrigger filter"),
        }
    }

    #[test]
    fn trigger_register_time_filter_defaults_to_exactly_one() {
        // Prepare tiny bytecode file
        let dir = std::env::temp_dir();
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = dir.join(format!("iroha_cli_trigger_test_time_{ts}.to"));
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(&[0xAA, 0xBB]).unwrap();

        // Deterministic account
        let account: AccountId =
            "ed25519:ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C@wonderland"
                .parse()
                .unwrap();
        let mut ctx = CaptureContext::new(account.clone());

        let start_ms = 1_700_000_000_000u64;
        // No repeats provided; non-periodic schedule should imply Exactly(1)
        let args = trigger::Register {
            id: "once".parse().unwrap(),
            path: Some(path),
            instructions_stdin: false,
            instructions: None,
            repeats: None,
            authority: None,
            filter: trigger::FilterType::Time,
            time_start_ms: Some(start_ms),
            time_period_ms: None,
            data_filter: None,
            data_domain: None,
            data_account: None,
            data_asset: None,
            data_asset_definition: None,
            data_role: None,
            data_trigger: None,
            data_verifying_key: None,
            data_proof: None,
            data_proof_only: None,
            data_vk_only: None,
            time_start: None,
            time_start_rfc3339: None,
        };

        args.run(&mut ctx).expect("run ok");

        let exec = ctx.captured.expect("captured");
        let Executable::Instructions(instructions) = exec else {
            panic!("expected instructions executable");
        };
        assert_eq!(instructions.len(), 1);
        let ib = &instructions[0];
        let reg = (**ib)
            .as_any()
            .downcast_ref::<iroha::data_model::isi::RegisterBox>()
            .expect("register");
        let iroha::data_model::isi::RegisterBox::Trigger(reg_tr) = reg else {
            panic!("expected trigger register")
        };
        let trig = reg_tr.object();
        assert_eq!(
            trig.action().repeats(),
            iroha::data_model::trigger::action::Repeats::Exactly(1)
        );
        match trig.action().filter() {
            iroha::data_model::events::EventFilterBox::Time(_) => {}
            _ => panic!("expected time filter"),
        }
    }

    #[test]
    fn trigger_register_data_domain_filter_builds() {
        // Prepare tiny bytecode file
        let dir = std::env::temp_dir();
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = dir.join(format!("iroha_cli_trigger_test_data_{ts}.to"));
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(&[0x01]).unwrap();

        let account: AccountId =
            "ed25519:ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C@wonderland"
                .parse()
                .unwrap();
        let mut ctx = CaptureContext::new(account);

        let args = trigger::Register {
            id: "data_trig".parse().unwrap(),
            path: Some(path),
            instructions_stdin: false,
            instructions: None,
            repeats: Some(1),
            authority: None,
            filter: trigger::FilterType::Data,
            time_start_ms: None,
            time_period_ms: None,
            data_filter: None,
            data_domain: Some("wonderland".parse().unwrap()),
            data_account: None,
            data_asset: None,
            data_asset_definition: None,
            data_role: None,
            data_trigger: None,
            data_verifying_key: None,
            data_proof: None,
            data_proof_only: None,
            data_vk_only: None,
            time_start: None,
            time_start_rfc3339: None,
        };

        args.run(&mut ctx).expect("run ok");
        let exec = ctx.captured.expect("captured");
        let Executable::Instructions(instructions) = exec else {
            panic!("expected instructions executable");
        };
        assert_eq!(instructions.len(), 1);
        let ib = &instructions[0];
        let reg = (**ib)
            .as_any()
            .downcast_ref::<iroha::data_model::isi::RegisterBox>()
            .expect("register");
        let iroha::data_model::isi::RegisterBox::Trigger(reg_tr) = reg else {
            panic!("expected trigger register")
        };
        let trig = reg_tr.object();
        match trig.action().filter() {
            iroha::data_model::events::EventFilterBox::Data(_) => {}
            _ => panic!("expected data filter"),
        }
    }
}

#[cfg(test)]
mod multisig_json_tests {
    use super::*;
    use iroha::data_model::{
        account::AccountId,
        isi::{CustomInstruction, InstructionBox},
    };
    use iroha::executor_data_model::isi::multisig::{
        MultisigRegister, MultisigSpec, DEFAULT_MULTISIG_TTL_MS,
    };
    use std::collections::BTreeMap;
    use std::num::{NonZeroU16, NonZeroU64};

    #[test]
    fn multisig_register_payload_contains_account() {
        let account: AccountId = "ed0120D6BBC55AD6CF0081E7EF561A6EC9F61CECC2A9920D0A542EA18D3FA4C9E6D176@sbp"
            .parse()
            .expect("valid account id");
        let mut signatories = BTreeMap::new();
        signatories.insert(account.clone(), 1);
        let spec = MultisigSpec::new(
            signatories,
            NonZeroU16::new(1).expect("nonzero quorum"),
            NonZeroU64::new(DEFAULT_MULTISIG_TTL_MS).expect("nonzero ttl"),
        );
        let register = MultisigRegister::with_account(account, spec);
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
mod tests {
    use super::cli_integration_harness::MockQueryServer;
    use super::*;
    use iroha::crypto::KeyPair;
    use iroha::data_model::query::{
        QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryWithParams, SingularQueryBox,
        SingularQueryOutputBox,
        builder::{QueryBuilder, QueryExecutor},
        parameters::{FetchSize, Pagination, QueryParams, Sorting},
    };
    use iroha::data_model::smart_contract::manifest::ContractManifest;
    use iroha::data_model::{domain::Domain, prelude::FindDomains};
    use iroha_crypto::Hash;
    use std::cmp::Ordering as CmpOrdering;
    use std::num::NonZeroU64;

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
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            // Return an empty Domain batch to satisfy type expectations
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Domain(vec![])],
                },
                0,
                None,
            ))
        }

        fn continue_query(
            _cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            unreachable!("no continuation in this test")
        }
    }

    #[test]
    fn compound_predicate_json_roundtrip() {
        use iroha::data_model::query::dsl::CompoundPredicate;

        let raw = r#"{"op":"eq","args":[{"FieldPath":"authority"},"alice@wonderland"]}"#;
        let predicate = super::parse_json::<CompoundPredicate<Domain>>(raw)
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

    // Small helper: sort optional-JSON-key ascending/descending, then compute window bounds
    fn sort_and_bounds<T, Get>(
        items: &mut Vec<T>,
        key: Option<Name>,
        desc: bool,
        get: Get,
        offset: usize,
        limit: Option<usize>,
        fetch: usize,
    ) -> (usize, usize, usize)
    where
        Get: Fn(&T, &Name) -> Option<&Json>,
    {
        if let Some(k) = key {
            items.sort_by(|a, b| {
                let la = get(a, &k);
                let lb = get(b, &k);
                match (la, lb) {
                    (Some(l), Some(r)) => {
                        let ord = l.cmp(r);
                        if desc { ord.reverse() } else { ord }
                    }
                    (Some(_), None) => CmpOrdering::Less,
                    (None, Some(_)) => CmpOrdering::Greater,
                    (None, None) => CmpOrdering::Equal,
                }
            });
        }
        let start = offset.min(items.len());
        let end = limit
            .map(|l| (start + l).min(items.len()))
            .unwrap_or(items.len());
        let first_end = start.saturating_add(fetch).min(end);
        (start, end, first_end)
    }

    #[test]
    fn parse_selector_and_apply_to_builder() {
        // Selector tuple parses from empty JSON array under lightweight DSL
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Domain> =
            super::parse_json("[]").expect("parse selector JSON");

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

    struct SortExec;

    impl QueryExecutor for SortExec {
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
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            // Build three domains with metadata key `rank`: 2, 1, and None
            let domain_id1: iroha::data_model::domain::DomainId = "d1".parse().unwrap();
            let domain_id2: iroha::data_model::domain::DomainId = "d2".parse().unwrap();
            let domain_id3: iroha::data_model::domain::DomainId = "d3".parse().unwrap();
            let kp = KeyPair::random();
            let owner1 = iroha::data_model::account::AccountId::new(
                domain_id1.clone(),
                kp.public_key().clone(),
            );
            let owner2 = iroha::data_model::account::AccountId::new(
                domain_id2.clone(),
                kp.public_key().clone(),
            );
            let owner3 = iroha::data_model::account::AccountId::new(
                domain_id3.clone(),
                kp.public_key().clone(),
            );

            let mut d1 = Domain::new(domain_id1).build(&owner1);
            let mut d2 = Domain::new(domain_id2).build(&owner2);
            let d3 = Domain::new(domain_id3).build(&owner3);
            d1.metadata_mut()
                .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
            d2.metadata_mut()
                .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

            // Apply sorting if provided
            let mut v = vec![d1, d2, d3];
            if let Some(key) = q.params.sorting.sort_by_metadata_key.clone() {
                let desc = matches!(
                    q.params.sorting.order,
                    Some(iroha::data_model::query::parameters::SortOrder::Desc)
                );
                v.sort_by(|a, b| {
                    let la = a.metadata().get(&key);
                    let lb = b.metadata().get(&key);
                    let ord = match (la, lb) {
                        (Some(l), Some(r)) => l.cmp(r),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    };
                    if desc { ord.reverse() } else { ord }
                });
            }

            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Domain(v)],
                },
                0,
                None,
            ))
        }

        fn continue_query(
            _cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            unreachable!("single batch only")
        }
    }

    #[test]
    fn metadata_sorting_end_to_end() {
        let exec = SortExec;
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Asc),
        };
        // Also assert selector tuple parsing is accepted
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Domain> =
            super::parse_json("[]").expect("parse selector JSON");
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
        let exec = SortExec;
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Desc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Domain> =
            super::parse_json("[]").expect("parse selector JSON");
        let builder = QueryBuilder::new(&exec, FindDomains)
            .with_selector_tuple(tuple)
            .with_sorting(sorting);
        let out: Vec<Domain> = builder.execute_all().expect("exec ok");
        // Descending: d1 (2), d2 (1), then d3 (None)
        assert_eq!(out[0].id().name().as_ref(), "d1");
        assert_eq!(out[1].id().name().as_ref(), "d2");
        assert_eq!(out[2].id().name().as_ref(), "d3");
    }

    struct SortAccountsExec;

    impl QueryExecutor for SortAccountsExec {
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
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            use iroha::data_model::account::{Account, AccountId};
            use iroha::data_model::domain::DomainId;

            let domain: DomainId = "land".parse().unwrap();
            let kp1 = KeyPair::random();
            let kp2 = KeyPair::random();
            let kp3 = KeyPair::random();
            let id1 = AccountId::new(domain.clone(), kp1.public_key().clone());
            let id2 = AccountId::new(domain.clone(), kp2.public_key().clone());
            let id3 = AccountId::new(domain.clone(), kp3.public_key().clone());

            // Build accounts; builder API needs an authority, use id1 for simplicity
            let mut a1 = Account::new(id1.clone()).build(&id1);
            let mut a2 = Account::new(id2.clone()).build(&id1);
            let mut a3 = Account::new(id3.clone()).build(&id1);

            // Insert ranks: a2=1, a1=2, a3=None
            a1.metadata
                .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
            a2.metadata
                .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

            // Apply sorting if provided
            let mut v = vec![a1, a2, a3];
            if let Some(key) = q.params.sorting.sort_by_metadata_key.clone() {
                let desc = matches!(
                    q.params.sorting.order,
                    Some(iroha::data_model::query::parameters::SortOrder::Desc)
                );
                v.sort_by(|a, b| {
                    let la = a.metadata().get(&key);
                    let lb = b.metadata().get(&key);
                    let ord = match (la, lb) {
                        (Some(l), Some(r)) => l.cmp(r),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    };
                    if desc { ord.reverse() } else { ord }
                });
            }

            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Account(v)],
                },
                0,
                None,
            ))
        }

        fn continue_query(
            _cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            unreachable!("single batch only")
        }
    }

    #[test]
    fn metadata_sorting_accounts_end_to_end() {
        use iroha::data_model::account::Account;
        use iroha::data_model::prelude::FindAccounts;

        let exec = SortAccountsExec;
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Asc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Account> =
            super::parse_json("[]").expect("parse selector JSON");
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

        let exec = SortAccountsExec;
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Desc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Account> =
            super::parse_json("[]").expect("parse selector JSON");
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

    struct SortAssetDefsExec;

    impl QueryExecutor for SortAssetDefsExec {
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
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            use iroha::data_model::account::AccountId;
            use iroha::data_model::asset::definition::AssetDefinition;
            use iroha::data_model::asset::id::AssetDefinitionId;
            use iroha::data_model::domain::DomainId;

            let domain: DomainId = "land".parse().unwrap();
            let kp = KeyPair::random();
            let owner = AccountId::new(domain.clone(), kp.public_key().clone());
            let id1: AssetDefinitionId = "gold#land".parse().unwrap();
            let id2: AssetDefinitionId = "silver#land".parse().unwrap();
            let id3: AssetDefinitionId = "bronze#land".parse().unwrap();

            let mut ad1 = AssetDefinition::numeric(id1).build(&owner);
            let mut ad2 = AssetDefinition::numeric(id2).build(&owner);
            let ad3 = AssetDefinition::numeric(id3).build(&owner);

            // Insert ranks: ad1=2, ad2=1, ad3=None
            ad1.metadata_mut()
                .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
            ad2.metadata_mut()
                .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

            let mut v = vec![ad1, ad2, ad3];
            if let Some(key) = q.params.sorting.sort_by_metadata_key.clone() {
                let desc = matches!(
                    q.params.sorting.order,
                    Some(iroha::data_model::query::parameters::SortOrder::Desc)
                );
                v.sort_by(|a, b| {
                    let la = a.metadata().get(&key);
                    let lb = b.metadata().get(&key);
                    let ord = match (la, lb) {
                        (Some(l), Some(r)) => l.cmp(r),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    };
                    if desc { ord.reverse() } else { ord }
                });
            }

            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::AssetDefinition(v)],
                },
                0,
                None,
            ))
        }

        fn continue_query(
            _cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            unreachable!("single batch only")
        }
    }

    #[test]
    fn metadata_sorting_asset_defs_end_to_end() {
        use iroha::data_model::asset::definition::AssetDefinition;
        use iroha::data_model::prelude::FindAssetsDefinitions;

        let exec = SortAssetDefsExec;
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Asc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<AssetDefinition> =
            super::parse_json("[]").expect("parse selector JSON");
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

        let exec = SortAssetDefsExec;
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Desc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<AssetDefinition> =
            super::parse_json("[]").expect("parse selector JSON");
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

    // Pagination + fetch_size over Domains: ensure offset/limit and batching are respected
    use std::sync::atomic::{AtomicUsize, Ordering};

    static PAGED_DOMAINS_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PAGED_DOMAINS_CONTS: AtomicUsize = AtomicUsize::new(0);

    struct PaginatedDomainsExec;

    enum DomCursor {
        Domains {
            items: Vec<Domain>,
            idx: usize,
            end: usize,
            fetch: usize,
        },
    }

    impl QueryExecutor for PaginatedDomainsExec {
        type Cursor = DomCursor;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            PAGED_DOMAINS_STARTS.fetch_add(1, Ordering::SeqCst);
            use iroha::data_model::account::AccountId;
            use iroha::data_model::domain::DomainId;

            // Build 5 domains d0..d4
            let kp = KeyPair::random();
            let mut domains = Vec::new();
            for i in 0..5 {
                let name = format!("d{i}");
                let did: DomainId = name.parse().unwrap();
                let owner = AccountId::new(did.clone(), kp.public_key().clone());
                domains.push(Domain::new(did).build(&owner));
            }

            let fetch: usize = q
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get()
                .try_into()
                .unwrap_or(100);
            let offset: usize = q.params.pagination.offset_value() as usize;
            let limit: Option<usize> = q.params.pagination.limit_value().map(|n| n.get() as usize);
            let start = offset.min(domains.len());
            let end = limit
                .map(|l| (start + l).min(domains.len()))
                .unwrap_or(domains.len());
            let first_end = start.saturating_add(fetch).min(end);
            let first = domains[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let next = if remaining > 0 {
                Some(DomCursor::Domains {
                    items: domains,
                    idx: first_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Domain(first)],
                },
                remaining,
                next,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            match cursor {
                DomCursor::Domains {
                    items,
                    idx,
                    end,
                    fetch,
                } => {
                    PAGED_DOMAINS_CONTS.fetch_add(1, Ordering::SeqCst);
                    let next_end = idx.saturating_add(fetch).min(end);
                    let batch = items[idx..next_end].to_vec();
                    let remaining = end.saturating_sub(next_end) as u64;
                    let next = if remaining > 0 {
                        Some(DomCursor::Domains {
                            items,
                            idx: next_end,
                            end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::Domain(batch)],
                        },
                        remaining,
                        next,
                    ))
                }
            }
        }
    }

    #[test]
    fn pagination_and_fetch_size_domains() {
        use iroha::data_model::query::parameters::{FetchSize, Pagination};
        let exec = PaginatedDomainsExec;
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

    // Pagination + sorting combined for Domains (ascending and descending)
    enum PSDCursor {
        Domains {
            items: Vec<Domain>,
            idx: usize,
            end: usize,
            fetch: usize,
        },
    }

    static PSD_ASC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSD_ASC_CONTS: AtomicUsize = AtomicUsize::new(0);
    struct PagedSortedDomainsExecAsc;
    impl QueryExecutor for PagedSortedDomainsExecAsc {
        type Cursor = PSDCursor;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            PSD_ASC_STARTS.fetch_add(1, Ordering::SeqCst);
            use iroha::data_model::account::AccountId;
            use iroha::data_model::domain::DomainId;

            let kp = KeyPair::random();
            // Build domains d0..d4 with ranks: d0=2, d1=4, d2=None, d3=1, d4=3
            let mut domains = Vec::new();
            let mut mk = |name: &str, rank: Option<i64>| {
                let did: DomainId = name.parse().unwrap();
                let owner = AccountId::new(did.clone(), kp.public_key().clone());
                let mut d = Domain::new(did).build(&owner);
                if let Some(r) = rank {
                    d.metadata_mut()
                        .insert("rank".parse().unwrap(), Json::from(norito::json!(r)));
                }
                d
            };
            domains.push(mk("d0", Some(2)));
            domains.push(mk("d1", Some(4)));
            domains.push(mk("d2", None));
            domains.push(mk("d3", Some(1)));
            domains.push(mk("d4", Some(3)));

            let desc = matches!(
                q.params.sorting.order,
                Some(iroha::data_model::query::parameters::SortOrder::Desc)
            );
            let fetch: usize = q
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get()
                .try_into()
                .unwrap_or(100);
            let offset: usize = q.params.pagination.offset_value() as usize;
            let limit: Option<usize> = q.params.pagination.limit_value().map(|n| n.get() as usize);
            let (start, end, first_end) = sort_and_bounds(
                &mut domains,
                q.params.sorting.sort_by_metadata_key.clone(),
                desc,
                |d, k| d.metadata().get(k),
                offset,
                limit,
                fetch,
            );
            let first = domains[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let next = if remaining > 0 {
                Some(PSDCursor::Domains {
                    items: domains,
                    idx: first_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Domain(first)],
                },
                remaining,
                next,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            match cursor {
                PSDCursor::Domains {
                    items,
                    idx,
                    end,
                    fetch,
                } => {
                    PSD_ASC_CONTS.fetch_add(1, Ordering::SeqCst);
                    let next_end = idx.saturating_add(fetch).min(end);
                    let batch = items[idx..next_end].to_vec();
                    let remaining = end.saturating_sub(next_end) as u64;
                    let next = if remaining > 0 {
                        Some(PSDCursor::Domains {
                            items,
                            idx: next_end,
                            end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::Domain(batch)],
                        },
                        remaining,
                        next,
                    ))
                }
            }
        }
    }

    static PSD_DESC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSD_DESC_CONTS: AtomicUsize = AtomicUsize::new(0);
    struct PagedSortedDomainsExecDesc;
    impl QueryExecutor for PagedSortedDomainsExecDesc {
        type Cursor = PSDCursor;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            PSD_DESC_STARTS.fetch_add(1, Ordering::SeqCst);
            use iroha::data_model::account::AccountId;
            use iroha::data_model::domain::DomainId;

            let kp = KeyPair::random();
            let mut domains = Vec::new();
            let mut mk = |name: &str, rank: Option<i64>| {
                let did: DomainId = name.parse().unwrap();
                let owner = AccountId::new(did.clone(), kp.public_key().clone());
                let mut d = Domain::new(did).build(&owner);
                if let Some(r) = rank {
                    d.metadata_mut()
                        .insert("rank".parse().unwrap(), Json::from(norito::json!(r)));
                }
                d
            };
            domains.push(mk("d0", Some(2)));
            domains.push(mk("d1", Some(4)));
            domains.push(mk("d2", None));
            domains.push(mk("d3", Some(1)));
            domains.push(mk("d4", Some(3)));

            if let Some(key) = q.params.sorting.sort_by_metadata_key.clone() {
                let desc = matches!(
                    q.params.sorting.order,
                    Some(iroha::data_model::query::parameters::SortOrder::Desc)
                );
                domains.sort_by(|a, b| {
                    let la = a.metadata().get(&key);
                    let lb = b.metadata().get(&key);
                    match (la, lb, desc) {
                        (Some(l), Some(r), false) => l.cmp(r),
                        (Some(l), Some(r), true) => r.cmp(l),
                        (Some(_), None, _) => std::cmp::Ordering::Less,
                        (None, Some(_), _) => std::cmp::Ordering::Greater,
                        (None, None, _) => std::cmp::Ordering::Equal,
                    }
                });
            }

            let fetch: usize = q
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get() as usize;
            let offset: usize = q.params.pagination.offset_value() as usize;
            let limit: Option<usize> = q.params.pagination.limit_value().map(|n| n.get() as usize);
            let start = offset.min(domains.len());
            let end = limit
                .map(|l| (start + l).min(domains.len()))
                .unwrap_or(domains.len());
            let first_end = start.saturating_add(fetch).min(end);
            let first = domains[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let next = if remaining > 0 {
                Some(PSDCursor::Domains {
                    items: domains,
                    idx: first_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Domain(first)],
                },
                remaining,
                next,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            match cursor {
                PSDCursor::Domains {
                    items,
                    idx,
                    end,
                    fetch,
                } => {
                    PSD_DESC_CONTS.fetch_add(1, Ordering::SeqCst);
                    let next_end = idx.saturating_add(fetch).min(end);
                    let batch = items[idx..next_end].to_vec();
                    let remaining = end.saturating_sub(next_end) as u64;
                    let next = if remaining > 0 {
                        Some(PSDCursor::Domains {
                            items,
                            idx: next_end,
                            end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::Domain(batch)],
                        },
                        remaining,
                        next,
                    ))
                }
            }
        }
    }
    #[test]
    fn pagination_sorting_domains_asc() {
        PSD_ASC_STARTS.store(0, Ordering::SeqCst);
        PSD_ASC_CONTS.store(0, Ordering::SeqCst);
        let exec = PagedSortedDomainsExecAsc;
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
        let exec = PagedSortedDomainsExecDesc;
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

    // Sorting + pagination + batching for Accounts
    static PSA_ASC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSA_ASC_CONTS: AtomicUsize = AtomicUsize::new(0);
    struct PagedSortedAccountsExecAsc;

    enum PSACursor {
        Accounts {
            items: Vec<iroha::data_model::account::Account>,
            idx: usize,
            end: usize,
            fetch: usize,
        },
    }

    impl QueryExecutor for PagedSortedAccountsExecAsc {
        type Cursor = PSACursor;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            PSA_ASC_STARTS.fetch_add(1, Ordering::SeqCst);
            use iroha::data_model::account::{Account, AccountId};
            use iroha::data_model::domain::DomainId;

            let domain: DomainId = "land".parse().unwrap();
            // Build accounts a0..a4 with ranks: a0=2, a1=4, a2=None, a3=1, a4=3
            let mut accounts: Vec<Account> = (0..5)
                .map(|_| {
                    let kp = KeyPair::random();
                    let id = AccountId::new(domain.clone(), kp.public_key().clone());
                    // owner for builder is arbitrary for this harness
                    Account::new(id).build(&AccountId::new(
                        domain.clone(),
                        KeyPair::random().public_key().clone(),
                    ))
                })
                .collect();
            let key: Name = "rank".parse().unwrap();
            accounts[0]
                .metadata
                .insert(key.clone(), Json::from(norito::json!(2)));
            accounts[1]
                .metadata
                .insert(key.clone(), Json::from(norito::json!(4)));
            // accounts[2] -> no rank
            accounts[3]
                .metadata
                .insert(key.clone(), Json::from(norito::json!(1)));
            accounts[4]
                .metadata
                .insert(key, Json::from(norito::json!(3)));

            let desc = matches!(
                q.params.sorting.order,
                Some(iroha::data_model::query::parameters::SortOrder::Desc)
            );
            let fetch: usize = q
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get()
                .try_into()
                .unwrap_or(100);
            let offset: usize = q.params.pagination.offset_value() as usize;
            let limit: Option<usize> = q.params.pagination.limit_value().map(|n| n.get() as usize);
            let (start, end, first_end) = sort_and_bounds(
                &mut accounts,
                q.params.sorting.sort_by_metadata_key.clone(),
                desc,
                |a, k| a.metadata().get(k),
                offset,
                limit,
                fetch,
            );
            let first = accounts[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let next = if remaining > 0 {
                Some(PSACursor::Accounts {
                    items: accounts,
                    idx: first_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Account(first)],
                },
                remaining,
                next,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            match cursor {
                PSACursor::Accounts {
                    items,
                    idx,
                    end,
                    fetch,
                } => {
                    PSA_ASC_CONTS.fetch_add(1, Ordering::SeqCst);
                    let next_end = idx.saturating_add(fetch).min(end);
                    let batch = items[idx..next_end].to_vec();
                    let remaining = end.saturating_sub(next_end) as u64;
                    let next = if remaining > 0 {
                        Some(PSACursor::Accounts {
                            items,
                            idx: next_end,
                            end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::Account(batch)],
                        },
                        remaining,
                        next,
                    ))
                }
            }
        }
    }

    static PSA_DESC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSA_DESC_CONTS: AtomicUsize = AtomicUsize::new(0);
    struct PagedSortedAccountsExecDesc;
    impl QueryExecutor for PagedSortedAccountsExecDesc {
        type Cursor = PSACursor;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            PSA_DESC_STARTS.fetch_add(1, Ordering::SeqCst);
            use iroha::data_model::account::{Account, AccountId};
            use iroha::data_model::domain::DomainId;

            let domain: DomainId = "land".parse().unwrap();
            let mut accounts: Vec<Account> = (0..5)
                .map(|_| {
                    let kp = KeyPair::random();
                    let id = AccountId::new(domain.clone(), kp.public_key().clone());
                    Account::new(id).build(&AccountId::new(
                        domain.clone(),
                        KeyPair::random().public_key().clone(),
                    ))
                })
                .collect();
            let key: Name = "rank".parse().unwrap();
            accounts[0]
                .metadata
                .insert(key.clone(), Json::from(norito::json!(2)));
            accounts[1]
                .metadata
                .insert(key.clone(), Json::from(norito::json!(4)));
            accounts[3]
                .metadata
                .insert(key.clone(), Json::from(norito::json!(1)));
            accounts[4]
                .metadata
                .insert(key, Json::from(norito::json!(3)));

            if let Some(key) = q.params.sorting.sort_by_metadata_key.clone() {
                let desc = matches!(
                    q.params.sorting.order,
                    Some(iroha::data_model::query::parameters::SortOrder::Desc)
                );
                accounts.sort_by(|a, b| {
                    let la = a.metadata().get(&key);
                    let lb = b.metadata().get(&key);
                    match (la, lb, desc) {
                        (Some(l), Some(r), false) => l.cmp(r),
                        (Some(l), Some(r), true) => r.cmp(l),
                        (Some(_), None, _) => std::cmp::Ordering::Less,
                        (None, Some(_), _) => std::cmp::Ordering::Greater,
                        (None, None, _) => std::cmp::Ordering::Equal,
                    }
                });
            }

            let fetch: usize = q
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get() as usize;
            let offset: usize = q.params.pagination.offset_value() as usize;
            let limit: Option<usize> = q.params.pagination.limit_value().map(|n| n.get() as usize);
            let start = offset.min(accounts.len());
            let end = limit
                .map(|l| (start + l).min(accounts.len()))
                .unwrap_or(accounts.len());
            let first_end = start.saturating_add(fetch).min(end);
            let first = accounts[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let next = if remaining > 0 {
                Some(PSACursor::Accounts {
                    items: accounts,
                    idx: first_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Account(first)],
                },
                remaining,
                next,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            match cursor {
                PSACursor::Accounts {
                    items,
                    idx,
                    end,
                    fetch,
                } => {
                    PSA_DESC_CONTS.fetch_add(1, Ordering::SeqCst);
                    let next_end = idx.saturating_add(fetch).min(end);
                    let batch = items[idx..next_end].to_vec();
                    let remaining = end.saturating_sub(next_end) as u64;
                    let next = if remaining > 0 {
                        Some(PSACursor::Accounts {
                            items,
                            idx: next_end,
                            end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::Account(batch)],
                        },
                        remaining,
                        next,
                    ))
                }
            }
        }
    }
    #[test]
    fn pagination_sorting_accounts_asc() {
        use iroha::data_model::prelude::FindAccounts;
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        PSA_ASC_STARTS.store(0, Ordering::SeqCst);
        PSA_ASC_CONTS.store(0, Ordering::SeqCst);
        let exec = PagedSortedAccountsExecAsc;
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
                    .get(&"rank".parse().unwrap())
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
        let exec = PagedSortedAccountsExecDesc;
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
                    .get(&"rank".parse().unwrap())
                    .and_then(|j| j.try_into_any_norito::<i64>().ok())
            })
            .collect();
        assert_eq!(names, vec![Some(3), Some(2), Some(1)]);
        assert_eq!(PSA_DESC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSA_DESC_CONTS.load(Ordering::SeqCst), 1);
    }

    // Sorting + pagination + batching for AssetDefinitions
    static PSAD_ASC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSAD_ASC_CONTS: AtomicUsize = AtomicUsize::new(0);
    struct PagedSortedAssetDefsExecAsc;

    enum PSADCursor {
        Ads {
            items: Vec<iroha::data_model::asset::definition::AssetDefinition>,
            idx: usize,
            end: usize,
            fetch: usize,
        },
    }

    impl QueryExecutor for PagedSortedAssetDefsExecAsc {
        type Cursor = PSADCursor;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            PSAD_ASC_STARTS.fetch_add(1, Ordering::SeqCst);
            use iroha::data_model::account::AccountId;
            use iroha::data_model::asset::definition::AssetDefinition;
            use iroha::data_model::asset::id::AssetDefinitionId;
            use iroha::data_model::domain::DomainId;

            let domain: DomainId = "land".parse().unwrap();
            let owner = AccountId::new(domain.clone(), KeyPair::random().public_key().clone());

            // Build asset defs ad0..ad4 with ranks: ad0=2, ad1=4, ad2=None, ad3=1, ad4=3
            let ids: Vec<AssetDefinitionId> = (0..5)
                .map(|i| format!("ad{i}#land").parse().unwrap())
                .collect();
            let mut defs: Vec<AssetDefinition> = ids
                .into_iter()
                .map(|id| AssetDefinition::numeric(id).build(&owner))
                .collect();
            let key: Name = "rank".parse().unwrap();
            defs[0]
                .metadata_mut()
                .insert(key.clone(), Json::from(norito::json!(2)));
            defs[1]
                .metadata_mut()
                .insert(key.clone(), Json::from(norito::json!(4)));
            // defs[2] -> no rank
            defs[3]
                .metadata_mut()
                .insert(key.clone(), Json::from(norito::json!(1)));
            defs[4]
                .metadata_mut()
                .insert(key, Json::from(norito::json!(3)));

            let desc = matches!(
                q.params.sorting.order,
                Some(iroha::data_model::query::parameters::SortOrder::Desc)
            );
            let fetch: usize = q
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get()
                .try_into()
                .unwrap_or(100);
            let offset: usize = q.params.pagination.offset_value() as usize;
            let limit: Option<usize> = q.params.pagination.limit_value().map(|n| n.get() as usize);
            let (start, end, first_end) = sort_and_bounds(
                &mut defs,
                q.params.sorting.sort_by_metadata_key.clone(),
                desc,
                |ad, k| ad.metadata().get(k),
                offset,
                limit,
                fetch,
            );
            let first = defs[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let next = if remaining > 0 {
                Some(PSADCursor::Ads {
                    items: defs,
                    idx: first_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::AssetDefinition(first)],
                },
                remaining,
                next,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            match cursor {
                PSADCursor::Ads {
                    items,
                    idx,
                    end,
                    fetch,
                } => {
                    PSAD_ASC_CONTS.fetch_add(1, Ordering::SeqCst);
                    let next_end = idx.saturating_add(fetch).min(end);
                    let batch = items[idx..next_end].to_vec();
                    let remaining = end.saturating_sub(next_end) as u64;
                    let next = if remaining > 0 {
                        Some(PSADCursor::Ads {
                            items,
                            idx: next_end,
                            end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::AssetDefinition(batch)],
                        },
                        remaining,
                        next,
                    ))
                }
            }
        }
    }

    #[test]
    fn pagination_sorting_asset_defs_asc() {
        use iroha::data_model::prelude::FindAssetsDefinitions;
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        PSAD_ASC_STARTS.store(0, Ordering::SeqCst);
        PSAD_ASC_CONTS.store(0, Ordering::SeqCst);
        let exec = PagedSortedAssetDefsExecAsc;
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
                    .get(&"rank".parse().unwrap())
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
        static PSAD_DESC_STARTS: AtomicUsize = AtomicUsize::new(0);
        static PSAD_DESC_CONTS: AtomicUsize = AtomicUsize::new(0);
        struct PagedSortedAssetDefsExecDesc;
        impl QueryExecutor for PagedSortedAssetDefsExecDesc {
            type Cursor = PSADCursor;
            type Error = eyre::Report;

            fn execute_singular_query(
                &self,
                _query: iroha::data_model::query::SingularQueryBox,
            ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
                unreachable!("not used in this test")
            }

            fn start_query(
                &self,
                q: QueryWithParams,
            ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error>
            {
                PSAD_DESC_STARTS.fetch_add(1, Ordering::SeqCst);
                use iroha::data_model::account::AccountId;
                use iroha::data_model::asset::definition::AssetDefinition;
                use iroha::data_model::asset::id::AssetDefinitionId;
                use iroha::data_model::domain::DomainId;

                let domain: DomainId = "land".parse().unwrap();
                let owner = AccountId::new(domain.clone(), KeyPair::random().public_key().clone());
                let ids: Vec<AssetDefinitionId> = (0..5)
                    .map(|i| format!("ad{i}#land").parse().unwrap())
                    .collect();
                let mut defs: Vec<AssetDefinition> = ids
                    .into_iter()
                    .map(|id| AssetDefinition::numeric(id).build(&owner))
                    .collect();
                let key: Name = "rank".parse().unwrap();
                defs[0]
                    .metadata_mut()
                    .insert(key.clone(), Json::from(norito::json!(2)));
                defs[1]
                    .metadata_mut()
                    .insert(key.clone(), Json::from(norito::json!(4)));
                defs[3]
                    .metadata_mut()
                    .insert(key.clone(), Json::from(norito::json!(1)));
                defs[4]
                    .metadata_mut()
                    .insert(key, Json::from(norito::json!(3)));

                if let Some(key) = q.params.sorting.sort_by_metadata_key.clone() {
                    let desc = matches!(
                        q.params.sorting.order,
                        Some(iroha::data_model::query::parameters::SortOrder::Desc)
                    );
                    defs.sort_by(|a, b| {
                        let la = a.metadata().get(&key);
                        let lb = b.metadata().get(&key);
                        match (la, lb, desc) {
                            (Some(l), Some(r), false) => l.cmp(r),
                            (Some(l), Some(r), true) => r.cmp(l),
                            (Some(_), None, _) => std::cmp::Ordering::Less,
                            (None, Some(_), _) => std::cmp::Ordering::Greater,
                            (None, None, _) => std::cmp::Ordering::Equal,
                        }
                    });
                }

                let fetch: usize = q
                    .params
                    .fetch_size
                    .fetch_size
                    .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                    .get() as usize;
                let offset: usize = q.params.pagination.offset_value() as usize;
                let limit: Option<usize> =
                    q.params.pagination.limit_value().map(|n| n.get() as usize);
                let start = offset.min(defs.len());
                let end = limit
                    .map(|l| (start + l).min(defs.len()))
                    .unwrap_or(defs.len());
                let first_end = start.saturating_add(fetch).min(end);
                let first = defs[start..first_end].to_vec();
                let remaining = end.saturating_sub(first_end) as u64;
                let next = if remaining > 0 {
                    Some(PSADCursor::Ads {
                        items: defs,
                        idx: first_end,
                        end,
                        fetch,
                    })
                } else {
                    None
                };
                Ok((
                    QueryOutputBatchBoxTuple {
                        tuple: vec![QueryOutputBatchBox::AssetDefinition(first)],
                    },
                    remaining,
                    next,
                ))
            }

            fn continue_query(
                cursor: Self::Cursor,
            ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error>
            {
                match cursor {
                    PSADCursor::Ads {
                        items,
                        idx,
                        end,
                        fetch,
                    } => {
                        PSAD_DESC_CONTS.fetch_add(1, Ordering::SeqCst);
                        let next_end = idx.saturating_add(fetch).min(end);
                        let batch = items[idx..next_end].to_vec();
                        let remaining = end.saturating_sub(next_end) as u64;
                        let next = if remaining > 0 {
                            Some(PSADCursor::Ads {
                                items,
                                idx: next_end,
                                end,
                                fetch,
                            })
                        } else {
                            None
                        };
                        Ok((
                            QueryOutputBatchBoxTuple {
                                tuple: vec![QueryOutputBatchBox::AssetDefinition(batch)],
                            },
                            remaining,
                            next,
                        ))
                    }
                }
            }
        }

        PSAD_DESC_STARTS.store(0, Ordering::SeqCst);
        PSAD_DESC_CONTS.store(0, Ordering::SeqCst);
        let exec = PagedSortedAssetDefsExecDesc;
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
                    .get(&"rank".parse().unwrap())
                    .and_then(|j| j.try_into_any_norito::<i64>().ok())
            })
            .collect();
        assert_eq!(ranks, vec![Some(3), Some(2), Some(1)]);
        assert_eq!(PSAD_DESC_STARTS.load(Ordering::SeqCst), 1);
        assert_eq!(PSAD_DESC_CONTS.load(Ordering::SeqCst), 1);
    }

    // NFT sorting by content metadata
    struct SortNftsExec;

    impl QueryExecutor for SortNftsExec {
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
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            use iroha::data_model::account::AccountId;
            use iroha::data_model::domain::DomainId;
            use iroha::data_model::nft::{Nft, NftId};

            let domain: DomainId = "art".parse().unwrap();
            let kp = KeyPair::random();
            let owner = AccountId::new(domain.clone(), kp.public_key().clone());
            let id1: NftId = "n1$art".parse().unwrap();
            let id2: NftId = "n2$art".parse().unwrap();
            let id3: NftId = "n3$art".parse().unwrap();

            let mut n1 = Nft::new(id1, Default::default()).build(&owner);
            let mut n2 = Nft::new(id2, Default::default()).build(&owner);
            let n3 = Nft::new(id3, Default::default()).build(&owner);

            n1.content
                .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
            n2.content
                .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

            let mut v = vec![n1, n2, n3];
            let desc = matches!(
                q.params.sorting.order,
                Some(iroha::data_model::query::parameters::SortOrder::Desc)
            );
            // No pagination in this executor; keep simple sort for single-batch end-to-end test
            if let Some(key) = q.params.sorting.sort_by_metadata_key.clone() {
                v.sort_by(|a, b| {
                    let la = a.content().get(&key);
                    let lb = b.content().get(&key);
                    let ord = match (la, lb) {
                        (Some(l), Some(r)) => l.cmp(r),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    };
                    if desc { ord.reverse() } else { ord }
                });
            }

            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Nft(v)],
                },
                0,
                None,
            ))
        }

        fn continue_query(
            _cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            unreachable!("single batch only")
        }
    }

    #[test]
    fn metadata_sorting_nfts_end_to_end() {
        use iroha::data_model::nft::Nft;
        use iroha::data_model::prelude::FindNfts;

        let exec = SortNftsExec;
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Asc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Nft> =
            super::parse_json("[]").expect("parse selector JSON");
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

        let exec = SortNftsExec;
        let sorting = Sorting {
            sort_by_metadata_key: Some("rank".parse().unwrap()),
            order: Some(iroha::data_model::query::parameters::SortOrder::Desc),
        };
        let tuple: iroha::data_model::query::dsl::SelectorTuple<Nft> =
            super::parse_json("[]").expect("parse selector JSON");
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

    // Sorting + pagination + batching for NFTs
    static PSN_ASC_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PSN_ASC_CONTS: AtomicUsize = AtomicUsize::new(0);
    struct PagedSortedNftsExecAsc;

    enum PSNCursor {
        Nfts {
            items: Vec<iroha::data_model::nft::Nft>,
            idx: usize,
            end: usize,
            fetch: usize,
        },
    }

    impl QueryExecutor for PagedSortedNftsExecAsc {
        type Cursor = PSNCursor;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            PSN_ASC_STARTS.fetch_add(1, Ordering::SeqCst);
            use iroha::data_model::account::AccountId;
            use iroha::data_model::domain::DomainId;
            use iroha::data_model::nft::{Nft, NftId};

            let domain: DomainId = "art".parse().unwrap();
            let owner = AccountId::new(domain.clone(), KeyPair::random().public_key().clone());

            // Build NFTs n0..n4 with ranks: n0=2, n1=4, n2=None, n3=1, n4=3
            let ids: Vec<NftId> = (0..5)
                .map(|i| format!("n{i}$art").parse().unwrap())
                .collect();
            let mut nfts: Vec<Nft> = ids
                .into_iter()
                .map(|id| Nft::new(id, Default::default()).build(&owner))
                .collect();
            let key: Name = "rank".parse().unwrap();
            nfts[0]
                .content
                .insert(key.clone(), Json::from(norito::json!(2)));
            nfts[1]
                .content
                .insert(key.clone(), Json::from(norito::json!(4)));
            // nfts[2] -> no rank
            nfts[3]
                .content
                .insert(key.clone(), Json::from(norito::json!(1)));
            nfts[4].content.insert(key, Json::from(norito::json!(3)));

            if let Some(key) = q.params.sorting.sort_by_metadata_key.clone() {
                let desc = matches!(
                    q.params.sorting.order,
                    Some(iroha::data_model::query::parameters::SortOrder::Desc)
                );
                nfts.sort_by(|a, b| {
                    let la = a.content().get(&key);
                    let lb = b.content().get(&key);
                    match (la, lb, desc) {
                        (Some(l), Some(r), false) => l.cmp(r),
                        (Some(l), Some(r), true) => r.cmp(l),
                        (Some(_), None, _) => std::cmp::Ordering::Less,
                        (None, Some(_), _) => std::cmp::Ordering::Greater,
                        (None, None, _) => std::cmp::Ordering::Equal,
                    }
                });
            }

            let fetch: usize = q
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get()
                .try_into()
                .unwrap_or(100);
            let offset: usize = q.params.pagination.offset_value() as usize;
            let limit: Option<usize> = q.params.pagination.limit_value().map(|n| n.get() as usize);
            let start = offset.min(nfts.len());
            let end = limit
                .map(|l| (start + l).min(nfts.len()))
                .unwrap_or(nfts.len());
            let first_end = start.saturating_add(fetch).min(end);
            let first = nfts[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let next = if remaining > 0 {
                Some(PSNCursor::Nfts {
                    items: nfts,
                    idx: first_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Nft(first)],
                },
                remaining,
                next,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            match cursor {
                PSNCursor::Nfts {
                    items,
                    idx,
                    end,
                    fetch,
                } => {
                    PSN_ASC_CONTS.fetch_add(1, Ordering::SeqCst);
                    let next_end = idx.saturating_add(fetch).min(end);
                    let batch = items[idx..next_end].to_vec();
                    let remaining = end.saturating_sub(next_end) as u64;
                    let next = if remaining > 0 {
                        Some(PSNCursor::Nfts {
                            items,
                            idx: next_end,
                            end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::Nft(batch)],
                        },
                        remaining,
                        next,
                    ))
                }
            }
        }
    }

    #[test]
    fn pagination_sorting_nfts_asc() {
        use iroha::data_model::prelude::FindNfts;
        use iroha::data_model::query::parameters::{FetchSize, Pagination, SortOrder};
        PSN_ASC_STARTS.store(0, Ordering::SeqCst);
        PSN_ASC_CONTS.store(0, Ordering::SeqCst);
        let exec = PagedSortedNftsExecAsc;
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
        static PSN_DESC_STARTS: AtomicUsize = AtomicUsize::new(0);
        static PSN_DESC_CONTS: AtomicUsize = AtomicUsize::new(0);
        struct PagedSortedNftsExecDesc;
        impl QueryExecutor for PagedSortedNftsExecDesc {
            type Cursor = PSNCursor;
            type Error = eyre::Report;

            fn execute_singular_query(
                &self,
                _query: iroha::data_model::query::SingularQueryBox,
            ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
                unreachable!("not used in this test")
            }

            fn start_query(
                &self,
                q: QueryWithParams,
            ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error>
            {
                PSN_DESC_STARTS.fetch_add(1, Ordering::SeqCst);
                use iroha::data_model::account::AccountId;
                use iroha::data_model::domain::DomainId;
                use iroha::data_model::nft::{Nft, NftId};

                let domain: DomainId = "art".parse().unwrap();
                let owner = AccountId::new(domain.clone(), KeyPair::random().public_key().clone());

                let ids: Vec<NftId> = (0..5)
                    .map(|i| format!("n{i}$art").parse().unwrap())
                    .collect();
                let mut nfts: Vec<Nft> = ids
                    .into_iter()
                    .map(|id| Nft::new(id, Default::default()).build(&owner))
                    .collect();
                let key: Name = "rank".parse().unwrap();
                nfts[0]
                    .content
                    .insert(key.clone(), Json::from(norito::json!(2)));
                nfts[1]
                    .content
                    .insert(key.clone(), Json::from(norito::json!(4)));
                // nfts[2] none
                nfts[3]
                    .content
                    .insert(key.clone(), Json::from(norito::json!(1)));
                nfts[4].content.insert(key, Json::from(norito::json!(3)));

                if let Some(key) = q.params.sorting.sort_by_metadata_key.clone() {
                    let desc = matches!(
                        q.params.sorting.order,
                        Some(iroha::data_model::query::parameters::SortOrder::Desc)
                    );
                    nfts.sort_by(|a, b| {
                        let la = a.content().get(&key);
                        let lb = b.content().get(&key);
                        match (la, lb, desc) {
                            (Some(l), Some(r), false) => l.cmp(r),
                            (Some(l), Some(r), true) => r.cmp(l),
                            (Some(_), None, _) => std::cmp::Ordering::Less,
                            (None, Some(_), _) => std::cmp::Ordering::Greater,
                            (None, None, _) => std::cmp::Ordering::Equal,
                        }
                    });
                }

                let fetch: usize = q
                    .params
                    .fetch_size
                    .fetch_size
                    .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                    .get() as usize;
                let offset: usize = q.params.pagination.offset_value() as usize;
                let limit: Option<usize> =
                    q.params.pagination.limit_value().map(|n| n.get() as usize);
                let start = offset.min(nfts.len());
                let end = limit
                    .map(|l| (start + l).min(nfts.len()))
                    .unwrap_or(nfts.len());
                let first_end = start.saturating_add(fetch).min(end);
                let first = nfts[start..first_end].to_vec();
                let remaining = end.saturating_sub(first_end) as u64;
                let next = if remaining > 0 {
                    Some(PSNCursor::Nfts {
                        items: nfts,
                        idx: first_end,
                        end,
                        fetch,
                    })
                } else {
                    None
                };
                Ok((
                    QueryOutputBatchBoxTuple {
                        tuple: vec![QueryOutputBatchBox::Nft(first)],
                    },
                    remaining,
                    next,
                ))
            }

            fn continue_query(
                cursor: Self::Cursor,
            ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error>
            {
                match cursor {
                    PSNCursor::Nfts {
                        items,
                        idx,
                        end,
                        fetch,
                    } => {
                        PSN_DESC_CONTS.fetch_add(1, Ordering::SeqCst);
                        let next_end = idx.saturating_add(fetch).min(end);
                        let batch = items[idx..next_end].to_vec();
                        let remaining = end.saturating_sub(next_end) as u64;
                        let next = if remaining > 0 {
                            Some(PSNCursor::Nfts {
                                items,
                                idx: next_end,
                                end,
                                fetch,
                            })
                        } else {
                            None
                        };
                        Ok((
                            QueryOutputBatchBoxTuple {
                                tuple: vec![QueryOutputBatchBox::Nft(batch)],
                            },
                            remaining,
                            next,
                        ))
                    }
                }
            }
        }

        PSN_DESC_STARTS.store(0, Ordering::SeqCst);
        PSN_DESC_CONTS.store(0, Ordering::SeqCst);
        let exec = PagedSortedNftsExecDesc;
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

    // Pagination for Accounts and Asset Definitions
    static PAGED_ACCOUNTS_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PAGED_ACCOUNTS_CONTS: AtomicUsize = AtomicUsize::new(0);

    struct PaginatedAccountsExec;

    enum AccCursor {
        Accounts {
            items: Vec<iroha::data_model::account::Account>,
            idx: usize,
            end: usize,
            fetch: usize,
        },
    }

    impl QueryExecutor for PaginatedAccountsExec {
        type Cursor = AccCursor;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            PAGED_ACCOUNTS_STARTS.fetch_add(1, Ordering::SeqCst);
            use iroha::data_model::account::{Account, AccountId};
            use iroha::data_model::domain::DomainId;

            // Build 5 accounts a0..a4 in the same domain, annotate metadata pos = index
            let domain: DomainId = "land".parse().unwrap();
            let mut accounts = Vec::new();
            for i in 0..5 {
                let kp = KeyPair::random();
                let id = AccountId::new(domain.clone(), kp.public_key().clone());
                let mut a = Account::new(id).build(
                    // owner is arbitrary in builder path
                    &AccountId::new(domain.clone(), KeyPair::random().public_key().clone()),
                );
                a.metadata
                    .insert("pos".parse().unwrap(), Json::from(norito::json!(i)));
                accounts.push(a);
            }

            let fetch: usize = q
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get()
                .try_into()
                .unwrap_or(100);
            let offset: usize = q.params.pagination.offset_value() as usize;
            let limit: Option<usize> = q.params.pagination.limit_value().map(|n| n.get() as usize);
            let start = offset.min(accounts.len());
            let end = limit
                .map(|l| (start + l).min(accounts.len()))
                .unwrap_or(accounts.len());
            let first_end = start.saturating_add(fetch).min(end);
            let first = accounts[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let next = if remaining > 0 {
                Some(AccCursor::Accounts {
                    items: accounts,
                    idx: first_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Account(first)],
                },
                remaining,
                next,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            let AccCursor::Accounts {
                items,
                idx,
                end,
                fetch,
            } = cursor;
            PAGED_ACCOUNTS_CONTS.fetch_add(1, Ordering::SeqCst);
            let next_end = idx.saturating_add(fetch).min(end);
            let batch = items[idx..next_end].to_vec();
            let remaining = end.saturating_sub(next_end) as u64;
            let next = if remaining > 0 {
                Some(AccCursor::Accounts {
                    items,
                    idx: next_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::Account(batch)],
                },
                remaining,
                next,
            ))
        }
    }

    #[test]
    fn pagination_and_fetch_size_accounts() {
        use iroha::data_model::account::Account;
        use iroha::data_model::prelude::FindAccounts;
        use iroha::data_model::query::parameters::{FetchSize, Pagination};

        let exec = PaginatedAccountsExec;
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

    static PAGED_ADS_STARTS: AtomicUsize = AtomicUsize::new(0);
    static PAGED_ADS_CONTS: AtomicUsize = AtomicUsize::new(0);

    struct PaginatedAssetDefsExec;

    enum AdCursor {
        Ads {
            items: Vec<iroha::data_model::asset::definition::AssetDefinition>,
            idx: usize,
            end: usize,
            fetch: usize,
        },
    }

    impl QueryExecutor for PaginatedAssetDefsExec {
        type Cursor = AdCursor;
        type Error = eyre::Report;

        fn execute_singular_query(
            &self,
            _query: iroha::data_model::query::SingularQueryBox,
        ) -> Result<iroha::data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("not used in this test")
        }

        fn start_query(
            &self,
            q: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            PAGED_ADS_STARTS.fetch_add(1, Ordering::SeqCst);
            use iroha::data_model::account::AccountId;
            use iroha::data_model::asset::definition::AssetDefinition;
            use iroha::data_model::asset::id::AssetDefinitionId;
            use iroha::data_model::domain::DomainId;

            let domain: DomainId = "land".parse().unwrap();
            let owner = AccountId::new(domain.clone(), KeyPair::random().public_key().clone());

            // Build 5 defs ad0..ad4 and tag pos metadata
            let mut defs = Vec::new();
            for i in 0..5 {
                let id: AssetDefinitionId = format!("ad{i}#land").parse().unwrap();
                let mut ad = AssetDefinition::numeric(id).build(&owner);
                ad.metadata_mut()
                    .insert("pos".parse().unwrap(), Json::from(norito::json!(i)));
                defs.push(ad);
            }

            let fetch: usize = q
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(iroha::data_model::query::parameters::DEFAULT_FETCH_SIZE)
                .get()
                .try_into()
                .unwrap_or(100);
            let offset: usize = q.params.pagination.offset_value() as usize;
            let limit: Option<usize> = q.params.pagination.limit_value().map(|n| n.get() as usize);
            let start = offset.min(defs.len());
            let end = limit
                .map(|l| (start + l).min(defs.len()))
                .unwrap_or(defs.len());
            let first_end = start.saturating_add(fetch).min(end);
            let first = defs[start..first_end].to_vec();
            let remaining = end.saturating_sub(first_end) as u64;
            let next = if remaining > 0 {
                Some(AdCursor::Ads {
                    items: defs,
                    idx: first_end,
                    end,
                    fetch,
                })
            } else {
                None
            };
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::AssetDefinition(first)],
                },
                remaining,
                next,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            match cursor {
                AdCursor::Ads {
                    items,
                    idx,
                    end,
                    fetch,
                } => {
                    PAGED_ADS_CONTS.fetch_add(1, Ordering::SeqCst);
                    let next_end = idx.saturating_add(fetch).min(end);
                    let batch = items[idx..next_end].to_vec();
                    let remaining = end.saturating_sub(next_end) as u64;
                    let next = if remaining > 0 {
                        Some(AdCursor::Ads {
                            items,
                            idx: next_end,
                            end,
                            fetch,
                        })
                    } else {
                        None
                    };
                    Ok((
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::AssetDefinition(batch)],
                        },
                        remaining,
                        next,
                    ))
                }
            }
        }
    }

    #[test]
    fn pagination_and_fetch_size_asset_defs() {
        use iroha::data_model::asset::definition::AssetDefinition;
        use iroha::data_model::prelude::FindAssetsDefinitions;
        use iroha::data_model::query::parameters::{FetchSize, Pagination};

        let exec = PaginatedAssetDefsExec;
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
#[cfg(feature = "cli_integration_harness")]
mod cli_integration_harness {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use eyre::eyre;
    use iroha::data_model::query::runtime::ActiveAbiVersions;
    use iroha::data_model::{
        asset::{Asset, AssetId},
        executor::ExecutorDataModel,
        parameter::Parameters,
        proof::{ProofId, ProofRecord, ProofStatus},
        query::{
            QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryWithParams, SingularQueryBox,
            SingularQueryOutputBox,
            builder::QueryExecutor,
            parameters::{FetchSize, Pagination, QueryParams, Sorting},
        },
        smart_contract::manifest::ContractManifest,
    };
    use iroha_crypto::Hash;
    use iroha_primitives::json::Json;

    /// Minimal mock server for iterable queries; returns static payloads by type.
    pub struct MockQueryServer {
        pub domains: Vec<iroha::data_model::domain::Domain>,
        pub accounts: Vec<iroha::data_model::account::Account>,
        pub asset_defs: Vec<iroha::data_model::asset::definition::AssetDefinition>,
        pub params: QueryParams,
        pub executor_data_model: Option<ExecutorDataModel>,
        pub parameters: Option<Parameters>,
        pub proof_records: BTreeMap<ProofId, ProofRecord>,
        pub manifests: BTreeMap<Hash, ContractManifest>,
        pub active_abi_versions: Option<ActiveAbiVersions>,
        pub assets: BTreeMap<AssetId, Asset>,
    }

    impl Default for MockQueryServer {
        fn default() -> Self {
            Self {
                domains: vec![],
                accounts: vec![],
                asset_defs: vec![],
                params: QueryParams {
                    pagination: Pagination::default(),
                    sorting: Sorting::default(),
                    fetch_size: FetchSize::default(),
                },
                executor_data_model: None,
                parameters: None,
                proof_records: BTreeMap::new(),
                manifests: BTreeMap::new(),
                active_abi_versions: None,
                assets: BTreeMap::new(),
            }
        }
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
            ids: Vec<iroha::data_model::asset::definition::AssetDefinitionId>,
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
                SingularQueryBox::FindActiveAbiVersions(_) => self
                    .active_abi_versions
                    .clone()
                    .map(SingularQueryOutputBox::ActiveAbiVersions)
                    .ok_or_else(|| eyre!("active ABI versions not configured in MockQueryServer")),
                SingularQueryBox::FindAssetById(req) => self
                    .assets
                    .get(&req.asset_id)
                    .cloned()
                    .map(SingularQueryOutputBox::Asset)
                    .ok_or_else(|| eyre!(format!("asset `{}` not found", req.asset_id))),
                other => Err(eyre!(format!(
                    "query `{other:?}` not supported in MockQueryServer"
                ))),
            }
        }

        fn start_query(
            &self,
            query: QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
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
                        let ord = match (la, lb) {
                            (Some(l), Some(r)) => l.cmp(r),
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => std::cmp::Ordering::Equal,
                        };
                        if desc { ord.reverse() } else { ord }
                    });
                }
                let start = offset.min(v.len());
                let end = limit.map(|l| (start + l).min(v.len())).unwrap_or(v.len());
                let first_end = start.saturating_add(fetch).min(end);
                let first = v[start..first_end].to_vec();
                let remaining = end.saturating_sub(first_end) as u64;
                // Detect ids-only selector for domains
                if let Some(erased) = iroha::data_model::query::iter_query_inner::<
                    iroha::data_model::domain::Domain,
                >(&query.query)
                {
                    #[cfg(feature = "ids_projection")]
                    if erased.selector().is_ids_only() {
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
                            QueryOutputBatchBoxTuple {
                                tuple: vec![QueryOutputBatchBox::DomainId(first_ids)],
                            },
                            remaining,
                            next,
                        ));
                    }
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
                    QueryOutputBatchBoxTuple {
                        tuple: vec![QueryOutputBatchBox::Domain(first)],
                    },
                    remaining,
                    next,
                ));
            }
            if !self.accounts.is_empty() {
                let mut v = self.accounts.clone();
                if let Some(key) = sort_by {
                    v.sort_by(|a, b| {
                        let la = a.metadata().get(&key);
                        let lb = b.metadata().get(&key);
                        let ord = match (la, lb) {
                            (Some(l), Some(r)) => l.cmp(r),
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => std::cmp::Ordering::Equal,
                        };
                        if desc { ord.reverse() } else { ord }
                    });
                }
                let start = offset.min(v.len());
                let end = limit.map(|l| (start + l).min(v.len())).unwrap_or(v.len());
                let first_end = start.saturating_add(fetch).min(end);
                let first = v[start..first_end].to_vec();
                let remaining = end.saturating_sub(first_end) as u64;
                #[cfg(feature = "ids_projection")]
                if let Some(erased) = iroha::data_model::query::iter_query_inner::<
                    iroha::data_model::account::Account,
                >(&query.query)
                {
                    if erased.selector().is_ids_only() {
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
                            QueryOutputBatchBoxTuple {
                                tuple: vec![QueryOutputBatchBox::AccountId(first_ids)],
                            },
                            remaining,
                            next,
                        ));
                    }
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
                    QueryOutputBatchBoxTuple {
                        tuple: vec![QueryOutputBatchBox::Account(first)],
                    },
                    remaining,
                    next,
                ));
            }
            if !self.asset_defs.is_empty() {
                let mut v = self.asset_defs.clone();
                if let Some(key) = sort_by {
                    v.sort_by(|a, b| {
                        let la = a.metadata().get(&key);
                        let lb = b.metadata().get(&key);
                        let ord = match (la, lb) {
                            (Some(l), Some(r)) => l.cmp(r),
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => std::cmp::Ordering::Equal,
                        };
                        if desc { ord.reverse() } else { ord }
                    });
                }
                let start = offset.min(v.len());
                let end = limit.map(|l| (start + l).min(v.len())).unwrap_or(v.len());
                let first_end = start.saturating_add(fetch).min(end);
                let first = v[start..first_end].to_vec();
                let remaining = end.saturating_sub(first_end) as u64;
                #[cfg(feature = "ids_projection")]
                if let Some(erased) = iroha::data_model::query::iter_query_inner::<
                    iroha::data_model::asset::definition::AssetDefinition,
                >(&query.query)
                {
                    if erased.selector().is_ids_only() {
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
                            QueryOutputBatchBoxTuple {
                                tuple: vec![QueryOutputBatchBox::AssetDefinitionId(first_ids)],
                            },
                            remaining,
                            next,
                        ));
                    }
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
                    QueryOutputBatchBoxTuple {
                        tuple: vec![QueryOutputBatchBox::AssetDefinition(first)],
                    },
                    remaining,
                    next,
                ));
            }
            Ok((
                QueryOutputBatchBoxTuple {
                    tuple: vec![QueryOutputBatchBox::String(vec![])],
                },
                0,
                None,
            ))
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
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
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::Domain(batch)],
                        },
                        remaining,
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
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::Account(batch)],
                        },
                        remaining,
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
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::AssetDefinition(batch)],
                        },
                        remaining,
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
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::DomainId(batch)],
                        },
                        remaining,
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
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::AccountId(batch)],
                        },
                        remaining,
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
                        QueryOutputBatchBoxTuple {
                            tuple: vec![QueryOutputBatchBox::AssetDefinitionId(batch)],
                        },
                        remaining,
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
        let mut server = MockQueryServer::default();
        server.domains = vec![
            Domain::new("w1".parse().unwrap()).build(&"alice@w1".parse().unwrap()),
            Domain::new("w2".parse().unwrap()).build(&"alice@w2".parse().unwrap()),
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
        let mut server = MockQueryServer::default();
        let mut w1 = Domain::new("w1".parse().unwrap()).build(&"alice@w1".parse().unwrap());
        let mut w2 = Domain::new("w2".parse().unwrap()).build(&"alice@w2".parse().unwrap());
        let w3 = Domain::new("w3".parse().unwrap()).build(&"alice@w3".parse().unwrap()); // no rank
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

    #[test]
    fn mock_query_domains_ids_projection() {
        use iroha::data_model::domain::{Domain, DomainId};
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::QueryParams;
        use iroha::data_model::query::{self, QueryWithFilter, QueryWithParams};

        let mut server = MockQueryServer::default();
        server.domains = vec![
            Domain::new("w1".parse().unwrap()).build(&"alice@w1".parse().unwrap()),
            Domain::new("w2".parse().unwrap()).build(&"alice@w2".parse().unwrap()),
        ];

        // Build an erased iter query with ids-only selector
        let with_filter: QueryWithFilter<_> = QueryWithFilter::new(
            Box::new(query::domain::prelude::FindDomains),
            CompoundPredicate::PASS,
            SelectorTuple::<Domain>::ids_only(),
        );
        let qbox: query::QueryBox<query::QueryOutputBatchBox> = with_filter.into();
        let qwp = QueryWithParams::new(qbox, QueryParams::default());

        let (batch, _rem, _cur) = server.start_query(qwp).expect("start ok");
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::DomainId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], DomainId::new("w1".parse().unwrap(),));
        assert_eq!(ids[1], DomainId::new("w2".parse().unwrap(),));
    }

    #[test]
    fn mock_query_accounts_ids_projection() {
        use iroha::data_model::account::{Account, AccountId};
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::QueryParams;
        use iroha::data_model::query::{self, QueryWithFilter, QueryWithParams};

        let mut server = MockQueryServer::default();
        server.accounts = vec![
            Account::new("alice@w".parse().unwrap()).build(&"alice@w".parse().unwrap()),
            Account::new("bob@w".parse().unwrap()).build(&"bob@w".parse().unwrap()),
        ];

        let with_filter: QueryWithFilter<_> = QueryWithFilter::new(
            Box::new(query::account::prelude::FindAccounts),
            CompoundPredicate::PASS,
            SelectorTuple::<Account>::ids_only(),
        );
        let qbox: query::QueryBox<query::QueryOutputBatchBox> = with_filter.into();
        let qwp = QueryWithParams::new(qbox, QueryParams::default());

        let (batch, _rem, _cur) = server.start_query(qwp).expect("start ok");
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AccountId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(
            ids.iter()
                .any(|id| id == &AccountId::from_str("alice@w").unwrap())
        );
        assert!(
            ids.iter()
                .any(|id| id == &AccountId::from_str("bob@w").unwrap())
        );
    }

    #[test]
    fn mock_query_asset_defs_ids_projection() {
        use iroha::data_model::asset::definition::{AssetDefinition, AssetDefinitionId};
        use iroha::data_model::prelude::NumericSpec;
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::QueryParams;
        use iroha::data_model::query::{self, QueryWithFilter, QueryWithParams};

        let mut server = MockQueryServer::default();
        server.asset_defs = vec![
            AssetDefinition::new("rose#w".parse().unwrap(), NumericSpec::default())
                .build(&"alice@w".parse().unwrap()),
            AssetDefinition::new("tulip#w".parse().unwrap(), NumericSpec::default())
                .build(&"alice@w".parse().unwrap()),
        ];

        let with_filter: QueryWithFilter<_> = QueryWithFilter::new(
            Box::new(query::asset::prelude::FindAssetsDefinitions),
            CompoundPredicate::PASS,
            SelectorTuple::<AssetDefinition>::ids_only(),
        );
        let qbox: query::QueryBox<query::QueryOutputBatchBox> = with_filter.into();
        let qwp = QueryWithParams::new(qbox, QueryParams::default());

        let (batch, _rem, _cur) = server.start_query(qwp).expect("start ok");
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AssetDefinitionId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(
            ids.iter()
                .any(|id| id == &AssetDefinitionId::from_str("rose#w").unwrap())
        );
        assert!(
            ids.iter()
                .any(|id| id == &AssetDefinitionId::from_str("tulip#w").unwrap())
        );
    }

    #[test]
    fn mock_query_asset_defs_ids_projection_batched() {
        use iroha::data_model::asset::definition::{AssetDefinition, AssetDefinitionId};
        use iroha::data_model::prelude::NumericSpec;
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::{FetchSize, QueryParams};
        use iroha::data_model::query::{self, QueryWithFilter, QueryWithParams};
        use std::num::NonZeroU64;

        let mut server = MockQueryServer::default();
        server.asset_defs = vec![
            AssetDefinition::new("rose#w".parse().unwrap(), NumericSpec::default())
                .build(&"alice@w".parse().unwrap()),
            AssetDefinition::new("tulip#w".parse().unwrap(), NumericSpec::default())
                .build(&"alice@w".parse().unwrap()),
            AssetDefinition::new("peony#w".parse().unwrap(), NumericSpec::default())
                .build(&"alice@w".parse().unwrap()),
        ];

        let with_filter: QueryWithFilter<_> = QueryWithFilter::new(
            Box::new(query::asset::prelude::FindAssetsDefinitions),
            CompoundPredicate::PASS,
            SelectorTuple::<AssetDefinition>::ids_only(),
        );
        let qbox: query::QueryBox<query::QueryOutputBatchBox> = with_filter.into();
        let mut params = QueryParams::default();
        params.fetch_size = FetchSize::new(Some(NonZeroU64::new(2).unwrap()));
        let qwp = QueryWithParams::new(qbox, params);

        let (batch1, rem, cur) = server.start_query(qwp).expect("start ok");
        let ids1 = match batch1.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AssetDefinitionId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids1.len(), 2);
        assert!(ids1.contains(&AssetDefinitionId::from_str("rose#w").unwrap()));
        assert!(ids1.contains(&AssetDefinitionId::from_str("tulip#w").unwrap()));
        assert_eq!(rem, 1);
        let cur = cur.expect("should continue");

        let (batch2, rem2, cur2) =
            <MockQueryServer as query::builder::QueryExecutor>::continue_query(cur)
                .expect("cont ok");
        let ids2 = match batch2.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AssetDefinitionId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids2.len(), 1);
        assert!(ids2.contains(&AssetDefinitionId::from_str("peony#w").unwrap()));
        assert_eq!(rem2, 0);
        assert!(cur2.is_none());
    }

    #[test]
    fn mock_query_accounts_ids_projection_batched() {
        use iroha::data_model::account::{Account, AccountId};
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::{FetchSize, QueryParams};
        use iroha::data_model::query::{self, QueryWithFilter, QueryWithParams};
        use std::num::NonZeroU64;
        use std::str::FromStr;

        let mut server = MockQueryServer::default();
        server.accounts = vec![
            Account::new(AccountId::from_str("alice@w").unwrap())
                .build(&AccountId::from_str("alice@w").unwrap()),
            Account::new(AccountId::from_str("bob@w").unwrap())
                .build(&AccountId::from_str("bob@w").unwrap()),
            Account::new(AccountId::from_str("carol@w").unwrap())
                .build(&AccountId::from_str("carol@w").unwrap()),
        ];

        let with_filter: QueryWithFilter<_> = QueryWithFilter::new(
            Box::new(query::account::prelude::FindAccounts),
            CompoundPredicate::PASS,
            SelectorTuple::<Account>::ids_only(),
        );
        let qbox: query::QueryBox<query::QueryOutputBatchBox> = with_filter.into();
        let mut params = QueryParams::default();
        params.fetch_size = FetchSize::new(Some(NonZeroU64::new(2).unwrap()));
        let qwp = QueryWithParams::new(qbox, params);

        let (batch1, rem, cur) = server.start_query(qwp).expect("start ok");
        let ids1 = match batch1.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AccountId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids1.len(), 2);
        assert!(ids1.contains(&AccountId::from_str("alice@w").unwrap()));
        assert!(ids1.contains(&AccountId::from_str("bob@w").unwrap()));
        assert_eq!(rem, 1);
        let cur = cur.expect("should continue");

        let (batch2, rem2, cur2) =
            <MockQueryServer as query::builder::QueryExecutor>::continue_query(cur)
                .expect("cont ok");
        let ids2 = match batch2.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AccountId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids2.len(), 1);
        assert!(ids2.contains(&AccountId::from_str("carol@w").unwrap()));
        assert_eq!(rem2, 0);
        assert!(cur2.is_none());
    }

    #[test]
    fn mock_query_domains_ids_projection_batched() {
        use iroha::data_model::domain::{Domain, DomainId};
        use iroha::data_model::query::dsl::{CompoundPredicate, SelectorTuple};
        use iroha::data_model::query::parameters::{FetchSize, QueryParams};
        use iroha::data_model::query::{self, QueryWithFilter, QueryWithParams};
        use std::num::NonZeroU64;

        let mut server = MockQueryServer::default();
        server.domains = vec![
            Domain::new("d1".parse().unwrap()).build(&"alice@d1".parse().unwrap()),
            Domain::new("d2".parse().unwrap()).build(&"alice@d2".parse().unwrap()),
            Domain::new("d3".parse().unwrap()).build(&"alice@d3".parse().unwrap()),
        ];

        let with_filter: QueryWithFilter<_> = QueryWithFilter::new(
            Box::new(query::domain::prelude::FindDomains),
            CompoundPredicate::PASS,
            SelectorTuple::<Domain>::ids_only(),
        );
        let qbox: query::QueryBox<query::QueryOutputBatchBox> = with_filter.into();
        let mut params = QueryParams::default();
        params.fetch_size = FetchSize::new(Some(NonZeroU64::new(2).unwrap()));
        let qwp = QueryWithParams::new(qbox, params);

        let (batch1, rem, cur) = server.start_query(qwp).expect("start ok");
        let ids1 = match batch1.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::DomainId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids1.len(), 2);
        assert!(ids1.contains(&DomainId::from_str("d1").unwrap()));
        assert!(ids1.contains(&DomainId::from_str("d2").unwrap()));
        assert_eq!(rem, 1);
        let cur = cur.expect("should continue");

        let (batch2, rem2, cur2) =
            <MockQueryServer as query::builder::QueryExecutor>::continue_query(cur)
                .expect("cont ok");
        let ids2 = match batch2.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::DomainId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids2.len(), 1);
        assert!(ids2.contains(&DomainId::from_str("d3").unwrap()));
        assert_eq!(rem2, 0);
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
        let mut w1 = Domain::new("w1".parse().unwrap()).build(&"alice@w1".parse().unwrap());
        let mut w2 = Domain::new("w2".parse().unwrap()).build(&"alice@w2".parse().unwrap());
        let mut w3 = Domain::new("w3".parse().unwrap()).build(&"alice@w3".parse().unwrap());
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
            account::AccountId,
            asset::{Asset, AssetId, id::AssetDefinitionId},
            query::asset::prelude::FindAssetById,
        };
        use std::str::FromStr;

        let mut server = MockQueryServer::default();
        let asset_def_id = AssetDefinitionId::from_str("coin#wonderland").unwrap();
        let account_id = AccountId::from_str("alice@wonderland").unwrap();
        let asset_id = AssetId::new(asset_def_id, account_id);
        let asset = Asset::new(asset_id.clone(), 77_u32);
        server.assets.insert(asset_id.clone(), asset.clone());

        let out = server
            .execute_singular_query(SingularQueryBox::FindAssetById(FindAssetById {
                asset_id: asset_id.clone(),
            }))
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
    fn harness_singular_contract_manifest() {
        use iroha::data_model::query::smart_contract::prelude::FindContractManifestByCodeHash;

        let mut server = MockQueryServer::default();
        let code_hash = Hash::new(b"manifest-demo");
        let manifest = ContractManifest {
            code_hash: Some(code_hash.clone()),
            abi_hash: None,
            compiler_fingerprint: Some("kotodama-compiler".into()),
            features_bitmap: Some(0b1010),
            access_set_hints: None,
            entrypoints: None,
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
                runtime::{ActiveAbiVersions, prelude::FindActiveAbiVersions},
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
        server.active_abi_versions = Some(ActiveAbiVersions {
            active_versions: vec![1, 2],
            default_compile_target: 2,
        });

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
            .execute_singular_query(SingularQueryBox::FindActiveAbiVersions(
                FindActiveAbiVersions,
            ))
            .expect("ABI versions present");
        match abi_out {
            SingularQueryOutputBox::ActiveAbiVersions(versions) => assert_eq!(
                versions,
                ActiveAbiVersions {
                    active_versions: vec![1, 2],
                    default_compile_target: 2
                }
            ),
            other => panic!("unexpected output variant: {other:?}"),
        }
    }
}
