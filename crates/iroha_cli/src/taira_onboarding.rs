//! Ordinary Taira onboarding and faucet operations with immutable, secret-free recovery evidence.
//!
//! Preparing never submits. Submission consumes only the retained SDK-verified transaction;
//! recovery is read-only and requires exact state-resolved transaction evidence.
use super::*;
use iroha::client::{
    verify_account_faucet_prepared_transaction_v1,
    verify_account_onboarding_prepared_transaction_v1,
    verify_account_onboarding_proof_required_result_v1,
};

#[path = "taira_onboarding_journal.rs"]
mod journal;
use journal::Journal;

const JOURNAL_SCHEMA: &str = "iroha.taira.account-operation.v1";

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

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct OperationJournalV1 {
    schema: String,
    torii_url: String,
    chain_id: String,
    network_id: NetworkId,
    chain_discriminant: u16,
    account_id: String,
    binding: PreparedOperationBindingV1,
    fee_payment: FeePaymentIntent,
    operation: OperationV1,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum OperationV1 {
    Onboarding(Box<OnboardingV1>),
    Faucet(Box<FaucetV1>),
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct OnboardingV1 {
    issuer: AccountId,
    request: AccountOnboardingPlanRequestV1,
    receipt: AccountOnboardingPlanReceiptV1,
    response: OnboardingResponseV1,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum OnboardingResponseV1 {
    Prepared(Box<AccountOnboardingPreparedTransactionV1>),
    ProofRequired(Box<AccountOnboardingProofRequiredPrepareResponseV1>),
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct FaucetV1 {
    issuer: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Quantity,
    claim: AccountFaucetClaimV1,
    prepared: AccountFaucetPreparedTransactionV1,
}

impl FaucetV1 {
    fn policy(&self) -> Result<AccountFaucetPolicyV1> {
        AccountFaucetPolicyV1::try_new(
            self.issuer.clone(),
            self.asset_definition.clone(),
            self.amount.clone(),
        )
    }
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

impl PrepareArgs {
    fn identity(&self, now_ms: u64) -> Result<(String, u64)> {
        let request_id = self
            .request_id
            .clone()
            .unwrap_or_else(|| hex::encode(rand::random::<[u8; 32]>()));
        validate_request_id(&request_id).map_err(|error| eyre!(error))?;
        if !(1..=3600).contains(&self.expires_in_secs) {
            eyre::bail!("envelope lifetime must be between 1 and 3600 seconds");
        }
        let expires_at = self
            .expires_in_secs
            .checked_mul(1000)
            .and_then(|duration| now_ms.checked_add(duration))
            .ok_or_else(|| eyre!("envelope deadline overflow"))?;
        Ok((request_id, expires_at))
    }
}

fn validate_request_id(value: &str) -> Result<String, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("must be exactly 64 lowercase hexadecimal characters".to_owned());
    }
    Ok(value.to_owned())
}

fn canonical_issuer(value: &str) -> Result<AccountId> {
    let issuer =
        AccountId::parse_encoded(value).wrap_err("issuer must be a canonical account ID")?;
    if issuer.to_string() != value || issuer.try_signatory().is_none() {
        eyre::bail!("issuer must be an exact canonical single-signatory account ID");
    }
    Ok(issuer)
}

fn operation_client(config: &Config, timeout_secs: u64) -> Result<IrohaClient> {
    if !(1..=300).contains(&timeout_secs) {
        eyre::bail!("request timeout must be between 1 and 300 seconds");
    }
    let mut config = config.clone();
    config.torii_request_timeout =
        Duration::from_secs(timeout_secs).min(config.torii_request_timeout);
    Ok(IrohaClient::builder(config).build()?)
}

fn validate_public_endpoint(url: &Url) -> Result<()> {
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        eyre::bail!(
            "account-operation endpoints must be HTTP(S) roots without embedded credentials, query parameters, or fragments"
        );
    }
    Ok(())
}

impl Run for AccountCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let config = context.config().clone();
        validate_public_endpoint(&config.torii_api_url)?;
        if config.chain.to_string() != DEFAULT_CHAIN_ID
            || config.account_chain_discriminant != DEFAULT_CHAIN_DISCRIMINANT
        {
            eyre::bail!(
                "Taira account operations require the canonical Taira chain and profile {DEFAULT_CHAIN_DISCRIMINANT}"
            );
        }
        let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
        match self {
            Self::Onboard(OnboardCommand::Prepare(args)) => {
                prepare_onboarding(context, &config, args)
            }
            Self::Faucet(FaucetCommand::Prepare(args)) => prepare_faucet(context, &config, args),
            Self::Onboard(OnboardCommand::Submit(args)) => run_saved(
                context,
                &config,
                args.journal,
                "onboarding",
                Some(args.token),
            ),
            Self::Onboard(OnboardCommand::Resume(args)) => {
                run_saved(context, &config, args, "onboarding", None)
            }
            Self::Faucet(FaucetCommand::Submit(args)) => {
                run_saved_faucet(context, &config, args, true)
            }
            Self::Faucet(FaucetCommand::Resume(args)) => {
                run_saved_faucet(context, &config, args, false)
            }
        }
    }
}

fn prepare_onboarding<C: RunContext>(
    context: &mut C,
    config: &Config,
    args: OnboardPrepare,
) -> Result<()> {
    let issuer = canonical_issuer(&args.issuer)?;
    let request =
        AccountOnboardingPlanRequestV1::try_new(args.alias, &config.account, args.permissions)?;
    let requested_fee = context.transaction_fee_payment()?;
    let token = args.token.read()?;
    let (request_id, deadline) = args.prepare.identity(current_unix_ms()?)?;
    let journal = Journal::create(&args.prepare.journal.journal)?;
    let client = operation_client(config, args.prepare.journal.timeout_secs)?;
    let receipt = client.plan_account_onboarding(&request, &token)?;
    verify_trusted_issuer(&receipt, &issuer)?;
    let binding = PreparedOperationBindingV1::onboarding(
        &receipt,
        request_id,
        deadline.min(receipt.body.valid_until_ms),
    )?;
    let response = client.prepare_account_onboarding_transaction(
        &request,
        &receipt,
        &binding,
        &requested_fee,
        &token,
    )?;
    let (response, fee_payment) = match response {
        AccountOnboardingPrepareResponseV1::Prepared(prepared) => {
            let fee = prepared.fee_payment.clone();
            (OnboardingResponseV1::Prepared(prepared), fee)
        }
        AccountOnboardingPrepareResponseV1::ProofRequired(proof) => {
            (OnboardingResponseV1::ProofRequired(proof), requested_fee)
        }
    };
    let operation = new_operation(
        config,
        binding,
        fee_payment,
        OperationV1::Onboarding(Box::new(OnboardingV1 {
            issuer,
            request,
            receipt,
            response,
        })),
    );
    let transaction = operation.verify(config, "onboarding")?;
    journal.write_operation(&operation)?;
    report(
        context,
        &journal,
        &operation,
        if transaction.is_some() {
            "Prepared"
        } else {
            "ProofRequired"
        },
        None,
    )
}

fn prepare_faucet<C: RunContext>(
    context: &mut C,
    config: &Config,
    args: FaucetPrepare,
) -> Result<()> {
    let issuer = canonical_issuer(&args.issuer)?;
    let asset_definition: AssetDefinitionId = args
        .asset_definition
        .parse()
        .wrap_err("invalid faucet asset definition")?;
    let amount: Quantity = args.amount.parse().wrap_err("invalid faucet amount")?;
    if asset_definition.to_string() != args.asset_definition || amount.to_string() != args.amount {
        eyre::bail!("faucet asset definition and amount must use exact canonical spellings");
    }
    let policy =
        AccountFaucetPolicyV1::try_new(issuer.clone(), asset_definition.clone(), amount.clone())?;
    let requested_fee = context.transaction_fee_payment()?;
    let (request_id, expires_at) = args.prepare.identity(current_unix_ms()?)?;
    let journal = Journal::create(&args.prepare.journal.journal)?;
    let client = operation_client(config, args.prepare.journal.timeout_secs)?;
    let deadline =
        prepared_observation_deadline(args.prepare.journal.timeout_secs, Some(expires_at))?;
    let claim = solve_account_faucet_claim(
        config.torii_api_url.as_str(),
        &config.account,
        &config.network_id,
        deadline,
    )?;
    let binding = PreparedOperationBindingV1::faucet(&claim, request_id, expires_at)?;
    let prepared =
        client.prepare_account_faucet_transaction(&claim, &binding, &requested_fee, &policy)?;
    let operation = new_operation(
        config,
        binding,
        prepared.fee_payment.clone(),
        OperationV1::Faucet(Box::new(FaucetV1 {
            issuer,
            asset_definition,
            amount,
            claim,
            prepared,
        })),
    );
    operation.verify(config, "faucet")?;
    journal.write_operation(&operation)?;
    report(context, &journal, &operation, "Prepared", None)
}

fn new_operation(
    config: &Config,
    binding: PreparedOperationBindingV1,
    fee_payment: FeePaymentIntent,
    operation: OperationV1,
) -> OperationJournalV1 {
    OperationJournalV1 {
        schema: JOURNAL_SCHEMA.to_owned(),
        torii_url: config.torii_api_url.to_string(),
        chain_id: config.chain.to_string(),
        network_id: config.network_id,
        chain_discriminant: config.account_chain_discriminant,
        account_id: config.account.to_string(),
        binding,
        fee_payment,
        operation,
    }
}

fn verify_trusted_issuer(
    receipt: &AccountOnboardingPlanReceiptV1,
    issuer: &AccountId,
) -> Result<()> {
    if &receipt.body.authority != issuer || !receipt.verify() {
        eyre::bail!("onboarding receipt is not signed by the independently trusted issuer");
    }
    Ok(())
}

impl OperationJournalV1 {
    fn kind(&self) -> &'static str {
        match self.operation {
            OperationV1::Onboarding(_) => "onboarding",
            OperationV1::Faucet(_) => "faucet",
        }
    }

    fn verify(&self, config: &Config, expected_kind: &str) -> Result<Option<SignedTransaction>> {
        if self.schema != JOURNAL_SCHEMA
            || self.kind() != expected_kind
            || self.torii_url != config.torii_api_url.as_str()
            || self.chain_id != config.chain.to_string()
            || self.network_id != config.network_id
            || self.chain_discriminant != config.account_chain_discriminant
            || self.account_id != config.account.to_string()
        {
            eyre::bail!(
                "operation journal differs from the configured endpoint, network, profile, account, or operation"
            );
        }
        match &self.operation {
            OperationV1::Onboarding(operation) => {
                verify_trusted_issuer(&operation.receipt, &operation.issuer)?;
                if operation.request.account_id != self.account_id {
                    eyre::bail!("onboarding journal targets another account");
                }
                match &operation.response {
                    OnboardingResponseV1::Prepared(prepared) => {
                        if prepared.fee_payment != self.fee_payment {
                            eyre::bail!("journal fee intent differs from its signed transaction");
                        }
                        verify_account_onboarding_prepared_transaction_v1(
                            self.network_id,
                            &operation.request,
                            prepared,
                            &operation.receipt,
                            &self.binding,
                            &self.fee_payment,
                        )
                        .map(Some)
                    }
                    OnboardingResponseV1::ProofRequired(proof) => {
                        verify_account_onboarding_proof_required_result_v1(
                            self.network_id,
                            &operation.request,
                            proof,
                            &operation.receipt,
                            &self.binding,
                        )?;
                        Ok(None)
                    }
                }
            }
            OperationV1::Faucet(operation) => {
                if operation.claim.account_id != self.account_id
                    || operation.prepared.fee_payment != self.fee_payment
                {
                    eyre::bail!("faucet journal targets another account or fee intent");
                }
                verify_account_faucet_prepared_transaction_v1(
                    self.network_id,
                    &operation.prepared,
                    &operation.claim,
                    &self.binding,
                    &self.fee_payment,
                    &operation.policy()?,
                )
                .map(Some)
            }
        }
    }
}

fn run_saved<C: RunContext>(
    context: &mut C,
    config: &Config,
    args: JournalArgs,
    expected_kind: &str,
    token: Option<TokenArgs>,
) -> Result<()> {
    let submit = token.is_some();
    run_saved_operation(context, config, args, expected_kind, submit, token)
}

fn run_saved_faucet<C: RunContext>(
    context: &mut C,
    config: &Config,
    args: JournalArgs,
    submit: bool,
) -> Result<()> {
    run_saved_operation(context, config, args, "faucet", submit, None)
}

fn run_saved_operation<C: RunContext>(
    context: &mut C,
    config: &Config,
    args: JournalArgs,
    expected_kind: &str,
    submit: bool,
    token: Option<TokenArgs>,
) -> Result<()> {
    let journal = Journal::open(&args.journal)?;
    let operation = journal.read_operation()?;
    let transaction = operation.verify(config, expected_kind)?;
    let client = operation_client(config, args.timeout_secs)?;
    let before = observe(&client, &operation, transaction.as_ref())?;
    if !submit || before.status != "Absent" {
        report(
            context,
            &journal,
            &operation,
            before.status,
            before.evidence,
        )?;
        return require_completed(before.status);
    }
    if current_unix_ms()? >= operation.binding.execution_expires_at_unix_ms {
        report(context, &journal, &operation, "Expired", None)?;
        eyre::bail!(
            "the saved transaction expired; recovery remains read-only and no replacement was prepared"
        );
    }
    // This durable marker is installed before the only mutation call. A failed response is
    // ambiguous; the immutable transaction remains the sole object accepted on every retry.
    journal.record_submission(&operation)?;
    let _submission = match &operation.operation {
        OperationV1::Onboarding(onboarding) => match &onboarding.response {
            OnboardingResponseV1::Prepared(prepared) => {
                let token = token
                    .ok_or_else(|| eyre!("onboarding submission requires a private token"))?
                    .read()?;
                client.submit_prepared_account_onboarding_transaction(
                    &onboarding.request,
                    prepared,
                    &operation.fee_payment,
                    &token,
                )
            }
            OnboardingResponseV1::ProofRequired(_) => {
                eyre::bail!("proof-required onboarding has no transaction to submit")
            }
        },
        OperationV1::Faucet(faucet) => client.submit_prepared_account_faucet_transaction(
            &faucet.prepared,
            &operation.fee_payment,
            &faucet.policy()?,
        ),
    };
    let after = observe(&client, &operation, transaction.as_ref());
    match after {
        Ok(observation) if observation.status != "Absent" => {
            report(
                context,
                &journal,
                &operation,
                observation.status,
                observation.evidence,
            )?;
            require_completed(observation.status)
        }
        _ => {
            report(context, &journal, &operation, "Pending", None)?;
            require_completed("Pending")
        }
    }
}

fn require_completed(status: &str) -> Result<()> {
    match status {
        "Applied" | "AlreadyPresent" => Ok(()),
        "Rejected" | "AliasConflict" => eyre::bail!(
            "the saved operation did not complete: {status}; no replacement was prepared"
        ),
        _ => eyre::bail!(
            "the exact operation remains unresolved; use the same journal with `resume` to check its outcome"
        ),
    }
}

struct Observation {
    status: &'static str,
    evidence: Option<Value>,
}

fn observe(
    client: &IrohaClient,
    operation: &OperationJournalV1,
    transaction: Option<&SignedTransaction>,
) -> Result<Observation> {
    let Some(transaction) = transaction else {
        let OperationV1::Onboarding(onboarding) = &operation.operation else {
            eyre::bail!("faucet journal omitted its transaction");
        };
        let account = AccountId::parse_encoded(&operation.account_id)?;
        let alias = onboarding.request.alias.parse::<AccountAliasName>()?;
        return Ok(
            match client.prove_account_onboarding_current_state(&account, &alias)? {
                AccountOnboardingCurrentStateV1::Applied {
                    block_height,
                    block_hash,
                } => Observation {
                    status: "AlreadyPresent",
                    evidence: Some(norito::json!({
                        "kind": "account_alias_current_state",
                        "block_height": (block_height.get()),
                        "block_hash": (block_hash.to_string())
                    })),
                },
                AccountOnboardingCurrentStateV1::AliasAbsent { .. } => Observation {
                    status: "ProofRequired",
                    evidence: None,
                },
                AccountOnboardingCurrentStateV1::AliasConflict { .. } => Observation {
                    status: "AliasConflict",
                    evidence: None,
                },
            },
        );
    };
    let hash = transaction.hash();
    let Some(status) = client.get_transaction_status_response_global(hash)? else {
        return Ok(Observation {
            status: "Absent",
            evidence: None,
        });
    };
    if status.hash != hex::encode(hash.as_ref()) || status.scope != "global" {
        eyre::bail!("transaction status differs from the saved exact hash or global scope");
    }
    if prepared_recovery_status_is_final_failure(&status) {
        return Ok(Observation {
            status: "Rejected",
            evidence: None,
        });
    }
    if !prepared_recovery_status_is_final_applied(&status) {
        if !matches!(
            status.status.kind.as_str(),
            "Queued" | "Approved" | "Committed" | "Applied" | "Rejected" | "Expired"
        ) {
            eyre::bail!("exact transaction status uses an unsupported first-release state");
        }
        return Ok(Observation {
            status: "Pending",
            evidence: None,
        });
    }
    let block_height = status
        .status
        .block_height
        .filter(|height| *height > 0)
        .ok_or_else(|| eyre!("Applied status has no committed block height"))?;
    let details = match client
        .get_transaction_details(transaction.hash_as_entrypoint())
        .wrap_err("read exact committed transaction evidence")
    {
        Ok(details) => details,
        Err(error) if exact_transaction_details_not_found(&error) => {
            return Ok(Observation {
                status: "Pending",
                evidence: None,
            });
        }
        Err(error) => return Err(error),
    };
    let TransactionEntrypoint::External(committed) = details.transaction.entrypoint() else {
        eyre::bail!("Applied evidence is not an external transaction");
    };
    if details.transaction.result().is_err()
        || committed.hash() != hash
        || committed.encode_wire_v1()? != transaction.encode_wire_v1()?
    {
        eyre::bail!("Applied evidence differs from the exact saved transaction");
    }
    Ok(Observation {
        status: "Applied",
        evidence: Some(norito::json!({
            "kind": "exact_committed_transaction",
            "block_height": block_height,
            "entrypoint_hash": (transaction.hash_as_entrypoint().to_string())
        })),
    })
}

fn report<C: RunContext>(
    context: &mut C,
    journal: &Journal,
    operation: &OperationJournalV1,
    status: &str,
    evidence: Option<Value>,
) -> Result<()> {
    let transaction_hash = match &operation.operation {
        OperationV1::Onboarding(onboarding) => match &onboarding.response {
            OnboardingResponseV1::Prepared(prepared) => {
                Some(prepared.transaction_hash_hex.as_str())
            }
            OnboardingResponseV1::ProofRequired(_) => None,
        },
        OperationV1::Faucet(faucet) => Some(faucet.prepared.transaction_hash_hex.as_str()),
    };
    let command = if operation.kind() == "onboarding" {
        "onboard"
    } else {
        "faucet"
    };
    let next = if status == "Prepared" {
        "submit"
    } else {
        "resume"
    };
    let mut value = norito::json!({
        "schema": "iroha.taira.account-operation-result.v1",
        "operation": (operation.kind()),
        "status": status,
        "account_id": (operation.account_id),
        "network_id": (operation.network_id.to_string()),
        "chain_discriminant": (operation.chain_discriminant),
        "request_id": (operation.binding.request_id),
        "expires_at_unix_ms": (operation.binding.execution_expires_at_unix_ms),
        "fee_payment": (operation.fee_payment),
        "transaction_hash": transaction_hash,
        "journal": (journal.path().to_string_lossy().to_string()),
        "evidence": evidence
    });
    if let Value::Object(fields) = &mut value {
        match &operation.operation {
            OperationV1::Onboarding(onboarding) => {
                fields.insert(
                    "issuer".to_owned(),
                    Value::String(onboarding.issuer.to_string()),
                );
                fields.insert(
                    "alias".to_owned(),
                    Value::String(onboarding.request.alias.clone()),
                );
                fields.insert(
                    "permissions_requested".to_owned(),
                    json::to_value(&onboarding.request.permissions)?,
                );
                fields.insert(
                    "owner_auto_renew_follow_up".to_owned(),
                    Value::Bool(
                        onboarding
                            .receipt
                            .body
                            .owner_auto_renew_instruction
                            .is_some(),
                    ),
                );
            }
            OperationV1::Faucet(faucet) => {
                fields.insert(
                    "issuer".to_owned(),
                    Value::String(faucet.issuer.to_string()),
                );
                fields.insert(
                    "asset_definition".to_owned(),
                    Value::String(faucet.asset_definition.to_string()),
                );
                fields.insert(
                    "amount".to_owned(),
                    Value::String(faucet.amount.to_string()),
                );
            }
        }
    }
    if context.output_format() == CliOutputFormat::Json {
        context.print_data(&value)
    } else {
        context.println(format!(
            "{status}: {} for {}",
            operation.kind(),
            operation.account_id
        ))?;
        context.println(format!(
            "Network: {}\nAddress profile: {}\nExpires at (Unix ms): {}\nFee intent: {}",
            operation.network_id,
            operation.chain_discriminant,
            operation.binding.execution_expires_at_unix_ms,
            json::to_string(&operation.fee_payment)?
        ))?;
        if let Some(hash) = transaction_hash {
            context.println(format!("Transaction: {hash}"))?;
        }
        match &operation.operation {
            OperationV1::Onboarding(onboarding) => {
                context.println(format!(
                    "Alias: {}\nIssuer: {}",
                    onboarding.request.alias, onboarding.issuer
                ))?;
                if !onboarding.request.permissions.is_empty() {
                    context.println(format!(
                        "Permissions requested: {}",
                        onboarding.request.permissions.join(", ")
                    ))?;
                }
                if onboarding
                    .receipt
                    .body
                    .owner_auto_renew_instruction
                    .is_some()
                {
                    context.println("Alias auto-renew requires the separately signed owner follow-up recorded in the receipt.")?;
                }
            }
            OperationV1::Faucet(faucet) => {
                context.println(format!(
                    "Funding: {} of {}\nIssuer: {}",
                    faucet.amount, faucet.asset_definition, faucet.issuer
                ))?;
            }
        }
        context.println(format!("Journal: {}", journal.path().display()))?;
        if !matches!(
            status,
            "Applied" | "AlreadyPresent" | "Rejected" | "AliasConflict" | "Expired"
        ) {
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
}

#[cfg(test)]
#[path = "taira_onboarding_tests.rs"]
mod tests;
