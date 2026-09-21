//! Integrated developer wallet commands over canonical custody and native account services.

use super::*;
use iroha_data_model::{
    NetworkId,
    account::{AccountId, address::ChainDiscriminantGuard},
    transaction::FeePaymentIntent,
};
use iroha_primitives::numeric::Quantity;
use iroha_wallet::{WalletInfo, WalletNetwork, WalletStore, default_wallet_dir};
use iroha_wallet::{
    onboarding::{FaucetRequest, OnboardingService, PreparationOptions},
    operations::{
        AccountService, NativeOperationKind, OperationReport, TransferRequest, XOR_ASSET_DEFINITION,
    },
};
use std::time::Duration;

const TAIRA_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";

#[derive(Args, Debug)]
pub(super) struct WalletArgs {
    /// Private wallet store outside projects; defaults to the platform user data directory.
    #[arg(long, global = true, value_name = "DIRECTORY")]
    wallet_dir: Option<PathBuf>,
    /// Local wallet name used for this operation.
    #[arg(long, global = true, default_value = "default", value_name = "NAME")]
    wallet: String,
    #[command(subcommand)]
    command: WalletCommand,
}

#[derive(Subcommand, Debug)]
enum WalletCommand {
    /// Create a private signer after discovering and pinning the exact network identity.
    Create(WalletNetworkArgs),
    /// Import a private key file or connect an existing native client configuration.
    Import(ImportArgs),
    /// List public wallet identities without loading signing keys.
    List,
    /// Show the selected wallet's account, network and public key.
    Show,
    /// Read the selected account's authoritative XOR balance.
    Balance,
    /// Register/fund a testnet account using its public faucet; the faucet pays transaction fees.
    Fund(OperationArgs),
    /// Send XOR using the wallet's signing key and an exact quoted fee-paying transaction.
    Send(SendArgs),
    /// Acquire a domain you own for contract deployment, paying its quoted lease and transaction fees.
    Namespace(NamespaceArgs),
}

#[derive(Args, Debug, Default)]
struct WalletNetworkArgs {
    /// Named public network; defaults to Taira.
    #[arg(long)]
    network: Option<String>,
    /// Explicit trusted Torii root for a custom deployment.
    #[arg(long, value_name = "URL")]
    torii_url: Option<url::Url>,
    /// Canonical chain label for a custom network; exact genesis is discovered independently.
    #[arg(long)]
    chain_id: Option<String>,
    /// Optional expected genesis identity; reject a different discovered network.
    #[arg(long)]
    network_id: Option<NetworkId>,
    /// Optional expected account address discriminant.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    chain_discriminant: Option<u16>,
}

#[derive(Args, Debug)]
struct ImportArgs {
    /// Owner-private native client configuration; imported signing material remains outside projects.
    #[arg(long, required_unless_present = "private_key_file", conflicts_with_all = ["private_key_file", "network", "torii_url", "chain_id", "network_id", "chain_discriminant"])]
    config: Option<PathBuf>,
    /// Owner-private file containing the canonical private key; never pass a secret in an argument.
    #[arg(long, required_unless_present = "config")]
    private_key_file: Option<PathBuf>,
    #[command(flatten)]
    network: WalletNetworkArgs,
}

#[derive(Args, Debug)]
struct OperationArgs {
    /// Prepare and retain the exact signed transaction without submitting it.
    #[arg(long, conflicts_with_all = ["resume", "submit"])]
    prepare: bool,
    /// Submit a previously prepared exact transaction; an existing attempt is only recovered.
    #[arg(long, value_name = "JOURNAL", conflicts_with = "resume")]
    submit: Option<PathBuf>,
    /// Recover the exact retained transaction without submitting or reconstructing it.
    #[arg(long, value_name = "JOURNAL")]
    resume: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct SendArgs {
    /// Destination's canonical account address.
    #[arg(required_unless_present_any = ["resume", "submit"], conflicts_with_all = ["resume", "submit"])]
    to: Option<String>,
    /// Positive XOR quantity, written as a decimal string.
    #[arg(required_unless_present_any = ["resume", "submit"], conflicts_with_all = ["resume", "submit"])]
    amount: Option<Quantity>,
    #[command(flatten)]
    operation: OperationArgs,
    #[command(flatten)]
    fee: WalletFeeArgs,
}

#[derive(Args, Debug)]
struct NamespaceArgs {
    /// Full owned domain to acquire, for example developer.universal.
    #[arg(required_unless_present_any = ["resume", "submit"], conflicts_with_all = ["resume", "submit"])]
    domain: Option<String>,
    #[command(flatten)]
    operation: OperationArgs,
    #[command(flatten)]
    fee: WalletFeeArgs,
}

#[derive(Args, Debug, Default)]
pub(super) struct WalletFeeArgs {
    /// Exact sponsor program; omission selects payment from the transaction authority's XOR balance.
    #[arg(long, requires = "fee_program_revision", conflicts_with_all = ["resume", "submit"])]
    fee_program: Option<String>,
    /// Exact nonzero immutable sponsor revision.
    #[arg(long, requires = "fee_program", conflicts_with_all = ["resume", "submit"], value_parser = clap::value_parser!(u64).range(1..))]
    fee_program_revision: Option<u64>,
}

impl WalletFeeArgs {
    fn intent(&self) -> Result<FeePaymentIntent, Diagnostic> {
        let intent = match (&self.fee_program, self.fee_program_revision) {
            (None, None) => FeePaymentIntent::authority(Vec::new(), None),
            (Some(program), Some(revision)) => FeePaymentIntent::sponsor(
                program.parse().map_err(wallet_error)?,
                revision,
                Vec::new(),
                None,
            ),
            _ => {
                return Err(Diagnostic::new(
                    ErrorCode::Usage,
                    "sponsorship requires an exact program and nonzero revision",
                ));
            }
        };
        intent.validate().map_err(wallet_error)?;
        Ok(intent)
    }
}

pub(super) fn run_wallet(
    manifest: Option<&Path>,
    args: &WalletArgs,
    progress: &mut dyn FnMut(&str),
) -> CommandResult {
    let store = open_store(manifest, args.wallet_dir.as_deref())?;
    match &args.command {
        WalletCommand::Create(network) => {
            progress("Discovering the exact network and supported signing policy...");
            let network = discover_network(network)?;
            let info = store.create(&args.wallet, &network).map_err(wallet_error)?;
            wallet_info_output(&info, "Created", store.root())
        }
        WalletCommand::Import(import) => {
            let info = if let Some(config) = &import.config {
                store
                    .import_client_file(&args.wallet, config)
                    .map_err(wallet_error)?
            } else {
                let network = discover_network(&import.network)?;
                store
                    .import_key_file(
                        &args.wallet,
                        &network,
                        import.private_key_file.as_deref().ok_or_else(|| {
                            Diagnostic::new(ErrorCode::Usage, "a private key file is required")
                        })?,
                    )
                    .map_err(wallet_error)?
            };
            wallet_info_output(&info, "Imported", store.root())
        }
        WalletCommand::List => {
            let wallets = store.list().map_err(wallet_error)?;
            let message = if wallets.is_empty() {
                format!(
                    "No wallets yet. Next: musubi wallet --wallet-dir {} create",
                    quote_cli_argument(&store.root().display().to_string())
                )
            } else {
                wallets
                    .iter()
                    .map(|wallet| {
                        format!(
                            "{}  {}  {}",
                            wallet.name, wallet.account_id, wallet.network.torii_url
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok(Success {
                message,
                data: norito::json::to_value(&wallets).map_err(wallet_error)?,
            })
        }
        WalletCommand::Show => wallet_info_output(
            &store.show(&args.wallet).map_err(wallet_error)?,
            "Wallet",
            store.root(),
        ),
        WalletCommand::Balance => {
            let service =
                AccountService::new(store.load_config(&args.wallet).map_err(wallet_error)?)
                    .map_err(wallet_error)?;
            let balance = service.xor_balance().map_err(wallet_error)?;
            let _profile = ChainDiscriminantGuard::enter(balance.chain_discriminant);
            Ok(Success {
                message: format!("{}\nBalance: {} XOR", balance.account_id, balance.amount),
                data: balance.to_json().map_err(wallet_error)?,
            })
        }
        WalletCommand::Fund(operation) => run_fund(&store, &args.wallet, operation, progress),
        WalletCommand::Send(send) => run_send(&store, &args.wallet, send, progress),
        WalletCommand::Namespace(namespace) => {
            run_namespace(&store, &args.wallet, namespace, progress)
        }
    }
}

pub(super) fn open_store(
    manifest: Option<&Path>,
    directory: Option<&Path>,
) -> Result<WalletStore, Diagnostic> {
    let root = directory
        .map(Path::to_path_buf)
        .map_or_else(|| default_wallet_dir().map_err(wallet_error), Ok)?;
    let current = std::env::current_dir().map_err(wallet_error)?;
    let project = if let Some(manifest) = manifest {
        {
            let absolute = if manifest.is_absolute() {
                manifest.to_path_buf()
            } else {
                current.join(manifest)
            };
            absolute.parent().map(Path::to_path_buf)
        }
    } else {
        current
            .ancestors()
            .find(|ancestor| {
                ancestor.join("Musubi.toml").exists() || ancestor.join(".git").exists()
            })
            .map(Path::to_path_buf)
    };
    WalletStore::open(&root, project.as_deref()).map_err(wallet_error)
}

fn discover_network(args: &WalletNetworkArgs) -> Result<WalletNetwork, Diagnostic> {
    let name = args.network.as_deref().unwrap_or("taira");
    let (endpoint, chain) = if name == "taira" {
        if args
            .chain_id
            .as_deref()
            .is_some_and(|chain| chain != TAIRA_CHAIN_ID)
        {
            return Err(Diagnostic::new(
                ErrorCode::Usage,
                "Taira requires its canonical chain identity",
            ));
        }
        (
            args.torii_url.clone().unwrap_or_else(|| {
                url::Url::parse("https://taira.sora.org").expect("fixed public URL")
            }),
            TAIRA_CHAIN_ID.to_owned(),
        )
    } else {
        (
            args.torii_url.clone().ok_or_else(|| {
                Diagnostic::new(ErrorCode::Usage, "a custom network requires --torii-url")
            })?,
            args.chain_id.clone().ok_or_else(|| {
                Diagnostic::new(ErrorCode::Usage, "a custom network requires --chain-id")
            })?,
        )
    };
    let discovery =
        iroha::blocking::account_bootstrap::Client::new(endpoint.clone(), Duration::from_secs(30))
            .map_err(wallet_error)?;
    let policy = discovery.capabilities().map_err(wallet_error)?;
    if args
        .network_id
        .is_some_and(|expected| expected != policy.network_id)
        || args
            .chain_discriminant
            .is_some_and(|expected| expected != policy.network_prefix)
        || name == "taira" && policy.network_prefix != 369
    {
        return Err(Diagnostic::new(
            ErrorCode::Network,
            "discovered network identity does not match the selected wallet network",
        ));
    }
    WalletNetwork::new(
        policy.network_id,
        chain.parse().map_err(wallet_error)?,
        endpoint,
        policy.network_prefix,
    )
    .map_err(wallet_error)
}

fn wallet_info_output(info: &WalletInfo, action: &str, wallet_dir: &Path) -> CommandResult {
    let next = if action == "Wallet"
        || info.network.chain_discriminant != 369
        || info.network.chain_id != TAIRA_CHAIN_ID
    {
        "balance"
    } else {
        "fund"
    };
    Ok(Success {
        message: format!(
            "{action} {}\nAccount: {}\nPublic key: {}\nNetwork: {}\nNetwork ID: {}\nAddress profile: {}\nNext: musubi wallet --wallet-dir {} --wallet {} {next}",
            info.name,
            info.account_id,
            info.public_key,
            info.network.torii_url,
            info.network.network_id,
            info.network.chain_discriminant,
            quote_cli_argument(&wallet_dir.display().to_string()),
            quote_cli_argument(&info.name)
        ),
        data: norito::json::to_value(info).map_err(wallet_error)?,
    })
}

fn run_fund(
    store: &WalletStore,
    name: &str,
    args: &OperationArgs,
    progress: &mut dyn FnMut(&str),
) -> CommandResult {
    let config = store.load_config(name).map_err(wallet_error)?;
    if config.account_chain_discriminant != 369 || config.chain.to_string() != TAIRA_CHAIN_ID {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "testnet funding requires a Taira wallet",
        ));
    }
    let service = OnboardingService::new(config.clone()).map_err(wallet_error)?;
    if let Some(journal) = &args.resume {
        return operation_output(
            service.resume_faucet(journal, 30).map_err(|error| {
                retained_operation_error(error, journal, "fund", "resume", name, store.root())
            })?,
            journal,
            false,
            "fund",
            name,
            store.root(),
        );
    }
    if let Some(journal) = &args.submit {
        return operation_output(
            service.submit_faucet(journal, 30).map_err(|error| {
                retained_operation_error(error, journal, "fund", "submit", name, store.root())
            })?,
            journal,
            false,
            "fund",
            name,
            store.root(),
        );
    }
    let discovery = iroha::blocking::account_bootstrap::Client::new(
        config.torii_api_url.clone(),
        Duration::from_secs(30),
    )
    .map_err(wallet_error)?;
    let policy = discovery
        .faucet_policy(config.network_id, config.account_chain_discriminant)
        .map_err(|error| {
            let missing = error
                .downcast_ref::<iroha::account_bootstrap::DiscoveryHttpError>()
                .is_some_and(|error| error.status == 404);
            let diagnostic = wallet_error(error);
            if missing {
                diagnostic.with_help(
                    "The selected Torii deployment must expose /v1/accounts/faucet/policy. Deploy the current wallet funding API before retrying this command.",
                )
            } else {
                diagnostic
            }
        })?;
    if policy.asset_definition_id.to_string() != XOR_ASSET_DEFINITION {
        return Err(Diagnostic::new(
            ErrorCode::Network,
            "the selected testnet faucet does not issue the canonical XOR fee asset",
        ));
    }
    progress(&format!(
        "Requesting {} testnet XOR. The faucet registers a missing account and pays its transaction fees.",
        policy.amount
    ));
    let journal = store.operation_path(name, "fund").map_err(wallet_error)?;
    let request = FaucetRequest {
        issuer: policy.authority,
        asset_definition: policy.asset_definition_id,
        amount: policy.amount,
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
    };
    let prepared = service
        .prepare_faucet(&request, &PreparationOptions::default(), &journal)
        .map_err(wallet_error)?;
    progress(&render_operation(&prepared, &journal));
    if args.prepare {
        return operation_output(prepared, &journal, true, "fund", name, store.root());
    }
    operation_output(
        service.submit_faucet(&journal, 30).map_err(|error| {
            retained_operation_error(error, &journal, "fund", "submit", name, store.root())
        })?,
        &journal,
        false,
        "fund",
        name,
        store.root(),
    )
}

fn run_send(
    store: &WalletStore,
    name: &str,
    args: &SendArgs,
    progress: &mut dyn FnMut(&str),
) -> CommandResult {
    let config = store.load_config(name).map_err(wallet_error)?;
    let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
    let service = AccountService::new(config).map_err(wallet_error)?;
    if let Some(journal) = &args.operation.resume {
        return operation_output(
            service
                .resume(journal, NativeOperationKind::Transfer)
                .map_err(|error| {
                    retained_operation_error(error, journal, "send", "resume", name, store.root())
                })?,
            journal,
            false,
            "send",
            name,
            store.root(),
        );
    }
    if let Some(journal) = &args.operation.submit {
        return operation_output(
            service
                .submit(journal, NativeOperationKind::Transfer)
                .map_err(|error| {
                    retained_operation_error(error, journal, "send", "submit", name, store.root())
                })?,
            journal,
            false,
            "send",
            name,
            store.root(),
        );
    }
    let recipient = args
        .to
        .as_deref()
        .ok_or_else(|| Diagnostic::new(ErrorCode::Usage, "send requires a destination"))?;
    let destination = AccountId::parse_encoded(recipient).map_err(wallet_error)?;
    if destination.to_string() != recipient {
        return Err(Diagnostic::new(
            ErrorCode::Usage,
            "destination must be a canonical account address for the wallet network",
        ));
    }
    let request = TransferRequest {
        destination,
        amount: args.amount.clone().ok_or_else(|| {
            Diagnostic::new(ErrorCode::Usage, "send requires a positive XOR quantity")
        })?,
        fee_payment: args.fee.intent()?,
    };
    let journal = store.operation_path(name, "send").map_err(wallet_error)?;
    let prepared = service
        .prepare_transfer(&request, &journal)
        .map_err(wallet_error)?;
    progress(&render_operation(&prepared, &journal));
    if args.operation.prepare {
        return operation_output(prepared, &journal, true, "send", name, store.root());
    }
    operation_output(
        service
            .submit(&journal, NativeOperationKind::Transfer)
            .map_err(|error| {
                retained_operation_error(error, &journal, "send", "submit", name, store.root())
            })?,
        &journal,
        false,
        "send",
        name,
        store.root(),
    )
}

fn run_namespace(
    store: &WalletStore,
    name: &str,
    args: &NamespaceArgs,
    progress: &mut dyn FnMut(&str),
) -> CommandResult {
    let config = store.load_config(name).map_err(wallet_error)?;
    let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
    let service = AccountService::new(config.clone()).map_err(wallet_error)?;
    if let Some(journal) = &args.operation.resume {
        return operation_output(
            service
                .resume(journal, NativeOperationKind::AliasSetup)
                .map_err(|error| {
                    retained_operation_error(
                        error,
                        journal,
                        "namespace",
                        "resume",
                        name,
                        store.root(),
                    )
                })?,
            journal,
            false,
            "namespace",
            name,
            store.root(),
        );
    }
    if let Some(journal) = &args.operation.submit {
        return operation_output(
            service
                .submit(journal, NativeOperationKind::AliasSetup)
                .map_err(|error| {
                    retained_operation_error(
                        error,
                        journal,
                        "namespace",
                        "submit",
                        name,
                        store.root(),
                    )
                })?,
            journal,
            false,
            "namespace",
            name,
            store.root(),
        );
    }
    let domain = args.domain.as_deref().ok_or_else(|| {
        Diagnostic::new(ErrorCode::Usage, "namespace requires a full domain name")
    })?;
    let quote =
        iroha_wallet::namespace::prepare_domain_request(&config, domain).map_err(wallet_error)?;
    progress(&format!(
        "Domain: {}\nLease: 1 year\nMaximum rent: {} {}\nTransaction fees are quoted separately.",
        quote.domain, quote.rent, quote.payment_asset
    ));
    let journal = store
        .operation_path(name, "namespace")
        .map_err(wallet_error)?;
    let prepared = service
        .prepare_alias(&quote.request, args.fee.intent()?, &journal)
        .map_err(wallet_error)?;
    progress(&render_operation(&prepared, &journal));
    if prepared.status.is_complete() || args.operation.prepare {
        let is_prepared = args.operation.prepare && !prepared.status.is_complete();
        return operation_output(
            prepared,
            &journal,
            is_prepared,
            "namespace",
            name,
            store.root(),
        );
    }
    operation_output(
        service
            .submit(&journal, NativeOperationKind::AliasSetup)
            .map_err(|error| {
                retained_operation_error(error, &journal, "namespace", "submit", name, store.root())
            })?,
        &journal,
        false,
        "namespace",
        name,
        store.root(),
    )
}

fn render_operation(report: &OperationReport, journal: &Path) -> String {
    let mut text = report.status.as_str().to_owned();
    if report.data.get("journal").is_some_and(Value::is_null) {
        text.push_str("\nThe requested state is already present; no transaction was submitted.");
    } else {
        let _ = write!(text, "\nJournal: {}", journal.display());
    }
    for (key, label) in [
        ("network_id", "Network ID"),
        ("chain_discriminant", "Address profile"),
        ("account_id", "Account"),
        ("destination", "To"),
        ("amount", "Amount"),
        ("asset_definition", "Asset"),
        ("issuer", "Faucet payer"),
        ("transaction_hash", "Transaction"),
    ] {
        if let Some(value) = report.data.get(key).filter(|value| !value.is_null()) {
            let encoded = value
                .as_str()
                .map(str::to_owned)
                .or_else(|| norito::json::to_json(value).ok());
            if let Some(encoded) = encoded {
                let _ = write!(text, "\n{label}: {encoded}");
            }
        }
    }
    if let Some(deadline) = ["deadline_ms", "expires_at_unix_ms"]
        .iter()
        .find_map(|key| report.data.get(*key).and_then(Value::as_u64))
    {
        if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            let now = u64::try_from(now.as_millis()).unwrap_or(u64::MAX);
            let _ = write!(text, "\n{}", render_deadline(deadline, now));
        } else {
            let _ = write!(text, "\nExecution deadline (Unix ms): {deadline}");
        }
    }
    if let Some(fee) = report
        .data
        .get("fee_quote")
        .and_then(|quote| quote.get("intent"))
        .or_else(|| report.data.get("fee_payment"))
    {
        let _ = write!(text, "\n{}", render_fees(fee));
    }
    text
}

fn render_deadline(deadline_ms: u64, now_ms: u64) -> String {
    if deadline_ms <= now_ms {
        "Execution deadline: elapsed".to_owned()
    } else {
        format!(
            "Execution deadline: in {} seconds",
            (deadline_ms - now_ms).div_ceil(1000)
        )
    }
}

fn render_fees(value: &Value) -> String {
    let Ok(intent) = norito::json::from_value::<FeePaymentIntent>(value.clone()) else {
        return "Fees: invalid fee details; inspect the retained operation before submitting."
            .to_owned();
    };
    let payer = match &intent {
        FeePaymentIntent::Authority(_) => "transaction authority".to_owned(),
        FeePaymentIntent::Sponsor(sponsor) => format!(
            "sponsor {} at revision {}",
            sponsor.program_id, sponsor.program_revision
        ),
    };
    let amounts = intent
        .charge_limits()
        .iter()
        .map(|limit| {
            let asset = limit.asset_definition_id.to_string();
            format!(
                "{} {}",
                limit.max_amount,
                if asset == XOR_ASSET_DEFINITION {
                    "XOR"
                } else {
                    asset.as_str()
                }
            )
        })
        .collect::<Vec<_>>();
    format!(
        "Fee payer: {payer}\nMaximum fees: {}",
        if amounts.is_empty() {
            "0 (quoted by the node)".to_owned()
        } else {
            amounts.join(" + ")
        }
    )
}

fn operation_output(
    report: OperationReport,
    journal: &Path,
    preparation: bool,
    command: &str,
    wallet: &str,
    wallet_dir: &Path,
) -> CommandResult {
    use iroha_wallet::operations::OperationStatus;
    let mut message = render_operation(&report, journal);
    let base = format!(
        "musubi wallet --wallet-dir {} --wallet {} {command}",
        quote_cli_argument(&wallet_dir.display().to_string()),
        quote_cli_argument(wallet),
    );
    let next = match report.status {
        OperationStatus::Rejected | OperationStatus::Expired | OperationStatus::AliasConflict => {
            let reason = match report.status {
                OperationStatus::Rejected => {
                    "The exact transaction was rejected. Correct the reported cause"
                }
                OperationStatus::Expired => {
                    "The saved transaction expired before submission. Use a fresh deadline"
                }
                _ => "The requested alias belongs to another account. Choose an available name",
            };
            let parameters = match command {
                "send" => " <recipient-account> <amount>",
                "namespace" => " <available-domain>",
                _ => "",
            };
            format!(
                "{reason}, keep this journal as evidence, and prepare a new operation with `{base}{parameters} --prepare`."
            )
        }
        _ => format!(
            "{base} --{} {}",
            if preparation || report.status == OperationStatus::Prepared {
                "submit"
            } else {
                "resume"
            },
            quote_cli_argument(&journal.display().to_string()),
        ),
    };
    if preparation {
        let _ = write!(message, "\nNext: {next}");
    }
    if !preparation && !report.status.is_complete() {
        return Err(Diagnostic::new(
            ErrorCode::Network,
            format!("wallet operation is {}", report.status.as_str()),
        )
        .with_help(next)
        .with_details(message, report.data));
    }
    Ok(Success {
        message,
        data: report.data,
    })
}

fn retained_operation_error(
    error: impl std::fmt::Display,
    journal: &Path,
    command: &str,
    action: &str,
    wallet: &str,
    wallet_dir: &Path,
) -> Diagnostic {
    wallet_error(error)
        .with_context("journal", journal.display().to_string())
        .with_help(format!(
            "Continue the exact saved operation: musubi wallet --wallet-dir {} --wallet {} {command} --{action} {}. An existing submission attempt is recovered without another dispatch.",
            quote_cli_argument(&wallet_dir.display().to_string()),
            quote_cli_argument(wallet),
            quote_cli_argument(&journal.display().to_string()),
        ))
}

pub(super) fn wallet_error(error: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new(ErrorCode::Network, format!("{error:#}"))
}

#[cfg(test)]
#[path = "command_wallet_tests.rs"]
mod tests;
