//! Account balances and exact, quoted transfers with durable submission evidence.
use crate::operation_journal::Journal;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    blocking::{Client, funding::BalanceReport},
    client::{
        AccountTransactionDraft, Client as NativeClient, FeeQuoteRequest,
        decode_and_verify_alias_setup_plan_for_request,
    },
    config::Config,
    data_model::{
        NetworkId,
        account::{AccountId, address::ChainDiscriminantGuard},
        alias_setup::{AliasPlanDispositionV1, AliasSetupPlanRequestV1, AliasTransactionPlanV1},
        asset::{AssetDefinitionId, AssetId},
        isi::{InstructionBox, Transfer},
        prelude::{SignedTransaction, TransactionEntrypoint},
        transaction::{Executable, FeePaymentIntent},
    },
};
use iroha_model_base::metadata::Metadata;
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::FeeQuoteResponse;
use iroha_version::codec::{DecodeVersioned as _, EncodeVersioned as _};
use norito::json::{JsonDeserialize, JsonSerialize, Value};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Canonical XOR asset definition used by Taira's native fee economy.
pub const XOR_ASSET_DEFINITION: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";

/// Observed lifecycle state, never inferred from a successful submission alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationStatus {
    /// The exact signed operation is durable and has never been dispatched.
    Prepared,
    /// An authenticated onboarding response requires current-state proof.
    ProofRequired,
    /// The exact transaction and committed wire agree on global state-resolved application.
    Applied,
    /// The requested account or alias state already agrees with authenticated native evidence.
    AlreadyPresent,
    /// The queried node does not currently observe the exact transaction.
    Absent,
    /// An attempted or observed operation remains unresolved.
    Pending,
    /// Canonical terminal rejection was observed.
    Rejected,
    /// The unattempted deadline elapsed locally or canonical terminal expiry was observed.
    Expired,
    /// The requested alias currently identifies a different account.
    AliasConflict,
}
impl OperationStatus {
    /// Stable public status spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "Prepared",
            Self::ProofRequired => "ProofRequired",
            Self::Applied => "Applied",
            Self::AlreadyPresent => "AlreadyPresent",
            Self::Absent => "Absent",
            Self::Pending => "Pending",
            Self::Rejected => "Rejected",
            Self::Expired => "Expired",
            Self::AliasConflict => "AliasConflict",
        }
    }
    /// Whether verified operation completion permits a caller to advance its workflow.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Applied | Self::AlreadyPresent)
    }
}

/// Presentation-free account-operation evidence.
#[derive(Clone, Debug)]
pub struct OperationReport {
    /// Exact observed lifecycle state.
    pub status: OperationStatus,
    /// Public native evidence; contains no keys or runtime authorization material.
    pub data: Value,
}
impl OperationReport {
    /// Require verified completion before advancing to another operation.
    ///
    /// # Errors
    /// Returns an error for pending, absent, prepared, rejected, expired or conflicting evidence.
    pub fn require_complete(&self) -> Result<()> {
        if self.status.is_complete() {
            return Ok(());
        }
        eyre::bail!(
            "wallet operation is {}; recover the same journal before advancing",
            self.status.as_str()
        )
    }
}

/// One XOR transfer with an explicit caller-selected authority or sponsor fee policy.
#[derive(Clone, Debug)]
pub struct TransferRequest {
    /// Canonical receiving account.
    pub destination: AccountId,
    /// Exact positive XOR quantity.
    pub amount: Quantity,
    /// Explicit payer, sponsor revision and gas bound; maxima come from the SDK quote.
    pub fee_payment: FeePaymentIntent,
}

/// Expected native operation selected by the calling command before journal recovery or submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeOperationKind {
    /// Exact XOR transfer to another account.
    Transfer,
    /// Indivisible paid alias setup planned and verified by the SDK.
    AliasSetup,
}

/// Account-authorized shared native wallet operations.
pub struct AccountService {
    config: Config,
    client: Client,
}
impl AccountService {
    /// Bind a native client to one exact configured network, account and signing key.
    ///
    /// # Errors
    /// Rejects embedded endpoint credentials, an invalid signer, or invalid SDK configuration.
    pub fn new(config: Config) -> Result<Self> {
        validate_config(&config)?;
        let client = Client::new(config.clone())?;
        Ok(Self { config, client })
    }
    /// Read this account's exact XOR balance without preparing a transaction.
    ///
    /// # Errors
    /// Returns authentication, routing, account/asset-definition absence or query errors.
    pub fn xor_balance(&self) -> Result<BalanceReport> {
        self.client.balance(&XOR_ASSET_DEFINITION.parse()?)
    }
    /// Quote, sign once and privately persist one transfer before any transaction submission.
    ///
    /// # Errors
    /// Returns invalid quantity/fee, quote/signing, or immutable-journal failures.
    pub fn prepare_transfer(
        &self,
        request: &TransferRequest,
        journal: &Path,
    ) -> Result<OperationReport> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        validate_transfer_request(request, &self.config.account)?;
        self.prepare_native(
            NativeOperation::Transfer {
                destination: request.destination.clone(),
                amount: request.amount.clone(),
            },
            &request.fee_payment,
            journal,
        )
    }
    /// Verify one paid native alias setup plan, quote its exact indivisible instruction vector and save it.
    ///
    /// # Errors
    /// Rejects changed authority/network/request terms, expired plans, invalid fees and unsafe journals.
    pub fn prepare_alias(
        &self,
        request: &AliasSetupPlanRequestV1,
        fee_payment: FeePaymentIntent,
        journal: &Path,
    ) -> Result<OperationReport> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        fee_payment.validate()?;
        let plan = self.client.client().plan_alias_setup(request)?;
        self.client
            .client()
            .verify_alias_setup_plan_for_request(request, &plan)?;
        if !plan.body.resources.is_empty()
            && plan
                .body
                .resources
                .iter()
                .all(|resource| resource.disposition == AliasPlanDispositionV1::NoOp)
        {
            return Ok(OperationReport {
                status: OperationStatus::AlreadyPresent,
                data: norito::json!({
                    "schema": "iroha.wallet.native-transaction-result.v1", "operation": "alias_setup", "status": "AlreadyPresent",
                    "network_id": (self.config.network_id.to_string()), "chain_discriminant": (self.config.account_chain_discriminant),
                    "account_id": (self.config.account.to_string()), "request": (request), "plan": (plan),
                    "transaction_hash": null, "journal": null,
                }),
            });
        }
        self.prepare_native(
            NativeOperation::AliasSetup {
                request: request.clone(),
                plan,
            },
            &fee_payment,
            journal,
        )
    }
    fn prepare_native(
        &self,
        operation: NativeOperation,
        requested_fee: &FeePaymentIntent,
        journal: &Path,
    ) -> Result<OperationReport> {
        requested_fee.validate()?;
        self.client
            .refresh_capabilities()
            .wrap_err("wallet transaction submission compatibility")?;
        let instructions = operation.instructions(&self.config)?;
        let mut draft =
            AccountTransactionDraft::new(instructions, requested_fee.clone(), Metadata::default());
        if let NativeOperation::AliasSetup { plan, .. } = &operation {
            let remaining = plan
                .body
                .valid_until_ms
                .checked_sub(current_unix_ms()?)
                .filter(|remaining| *remaining > 0)
                .ok_or_else(|| eyre!("alias plan expired before transaction preparation"))?;
            draft = draft.with_time_to_live(
                Duration::from_millis(remaining).min(self.config.transaction_ttl),
            );
        }
        let mut payload = self.client.account_client().prepare_transaction(draft)?;
        let quote = self
            .client
            .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })?;
        if !requested_fee.has_same_payer_and_gas_bound(&quote.intent) {
            eyre::bail!(
                "transfer fee quote changed the selected payer, sponsor revision or gas bound"
            );
        }
        self.client
            .check_funding(&operation.principal()?, std::slice::from_ref(&quote))?;
        payload.fee_payment = quote.intent.clone();
        let signed = self.client.account_client().sign_transaction(payload)?;
        let record = TransactionJournal {
            schema: "iroha.wallet.native-transaction.v1".to_owned(),
            torii_url: self.config.torii_api_url.to_string(),
            chain_id: self.config.chain.to_string(),
            network_id: self.config.network_id,
            chain_discriminant: (self.config.account_chain_discriminant),
            account_id: self.config.account.clone(),
            operation,
            requested_fee: requested_fee.clone(),
            quote,
            transaction_hash: signed.hash().to_string(),
            signed_transaction_hex: hex::encode(signed.encode_versioned()),
            deadline_ms: transaction_deadline(&signed)?,
        };
        record.verify(&self.config)?;
        let journal = Journal::create(journal)?;
        journal.write_operation(&record)?;
        transfer_report(&journal, &record, OperationStatus::Prepared, None)
    }
    /// Submit a wholly unattempted saved transfer once, then verify its exact committed wire.
    ///
    /// An existing attempt is reconciled by hash without another submission.
    ///
    /// # Errors
    /// Returns invalid/unsafe journal or untrusted observation errors. Unresolved dispatch returns Pending.
    pub fn submit(&self, journal: &Path, expected: NativeOperationKind) -> Result<OperationReport> {
        self.run_transaction(journal, expected, true)
    }
    /// Read-only reconciliation of the saved transfer; no rebuilding, signing or submission occurs.
    ///
    /// # Errors
    /// Returns changed identity, unsafe evidence or malformed status/committed-wire errors.
    pub fn resume(&self, journal: &Path, expected: NativeOperationKind) -> Result<OperationReport> {
        self.run_transaction(journal, expected, false)
    }
    fn run_transaction(
        &self,
        path: &Path,
        expected: NativeOperationKind,
        submit: bool,
    ) -> Result<OperationReport> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        let journal = Journal::open(path)?;
        let record: TransactionJournal = journal.read_operation()?;
        if record.operation.kind() != expected {
            eyre::bail!(
                "saved journal contains a different native operation than the selected command"
            );
        }
        let transaction = record.verify(&self.config)?;
        let mut before = observe_transaction(self.client.client(), &transaction)?;
        if before.status == OperationStatus::Absent && journal.submission_recorded(&record)? {
            before.status = OperationStatus::Pending;
        }
        if !submit || before.status != OperationStatus::Absent {
            return transfer_report(&journal, &record, before.status, before.evidence);
        }
        if transaction_expired(&transaction)? {
            return transfer_report(&journal, &record, OperationStatus::Expired, None);
        }
        self.client.refresh_capabilities().wrap_err(
            "wallet transaction submission compatibility; saved operation remains unattempted",
        )?;
        if !journal.record_submission(&record)? {
            return transfer_report(&journal, &record, OperationStatus::Pending, None);
        }
        // The marker is durable before the only dispatch. Its existence permanently prevents replay.
        let _submission = self.client.submit_transaction_and_wait(&transaction);
        let after =
            observe_transaction(self.client.client(), &transaction).unwrap_or(Observation {
                status: OperationStatus::Pending,
                evidence: None,
            });
        let status = if after.status == OperationStatus::Absent {
            OperationStatus::Pending
        } else {
            after.status
        };
        if status == OperationStatus::Applied {
            journal.write_evidence_exact("applied.json", &after.evidence)?;
        }
        transfer_report(&journal, &record, status, after.evidence)
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct TransactionJournal {
    schema: String,
    torii_url: String,
    chain_id: String,
    network_id: NetworkId,
    chain_discriminant: u16,
    account_id: AccountId,
    operation: NativeOperation,
    requested_fee: FeePaymentIntent,
    quote: FeeQuoteResponse,
    transaction_hash: String,
    signed_transaction_hex: String,
    deadline_ms: u64,
}
impl TransactionJournal {
    fn verify(&self, config: &Config) -> Result<SignedTransaction> {
        if self.schema != "iroha.wallet.native-transaction.v1"
            || self.torii_url != config.torii_api_url.as_str()
            || self.chain_id != config.chain.to_string()
            || self.network_id != config.network_id
            || self.chain_discriminant != config.account_chain_discriminant
            || self.account_id != config.account
            || self.signed_transaction_hex.len() > 2 * 1024 * 1024
        {
            eyre::bail!(
                "transfer journal differs from the exact wallet, endpoint or network, or exceeds its bound"
            );
        }
        self.requested_fee.validate()?;
        let expected_instructions = self.operation.instructions(config)?;
        let bytes = hex::decode(&self.signed_transaction_hex)?;
        let transaction = SignedTransaction::decode_all_versioned(&bytes)?;
        transaction.verify_signature()?;
        let Executable::Instructions(instructions) = transaction.instructions() else {
            eyre::bail!("transfer must contain native instructions");
        };
        if hex::encode(&bytes) != self.signed_transaction_hex
            || transaction.encode_versioned() != bytes
            || transaction_deadline(&transaction)? != self.deadline_ms
            || transaction.hash().to_string() != self.transaction_hash
            || transaction.authority() != &config.account
            || transaction.network_id() != Some(&config.network_id)
            || instructions.as_ref() != expected_instructions.as_slice()
            || transaction.metadata() != &Metadata::default()
            || !self
                .requested_fee
                .has_same_payer_and_gas_bound(&self.quote.intent)
            || transaction.payload().fee_payment != self.quote.intent
        {
            eyre::bail!(
                "transfer journal has substituted transaction, destination, amount, fee or signer evidence"
            );
        }
        self.quote
            .validate_for_draft(transaction.payload())
            .map_err(|error| eyre!(error))?;
        Ok(transaction)
    }
}
fn validate_transfer_request(request: &TransferRequest, authority: &AccountId) -> Result<()> {
    request.fee_payment.validate()?;
    if request.amount.is_zero() || &request.destination == authority {
        eyre::bail!("transfer requires a positive amount and a different destination account");
    }
    Ok(())
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum NativeOperation {
    Transfer {
        destination: AccountId,
        amount: Quantity,
    },
    AliasSetup {
        request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
    },
}
impl NativeOperation {
    fn principal(&self) -> Result<BTreeMap<AssetDefinitionId, Quantity>> {
        match self {
            Self::Transfer { amount, .. } => Ok(BTreeMap::from([(
                XOR_ASSET_DEFINITION.parse()?,
                amount.clone(),
            )])),
            Self::AliasSetup { plan, .. } => {
                let mut result = BTreeMap::new();
                for total in &plan.body.totals_by_asset {
                    add_quantity(&mut result, &total.payment_asset, &total.amount)?;
                }
                Ok(result)
            }
        }
    }
    fn kind(&self) -> NativeOperationKind {
        match self {
            Self::Transfer { .. } => NativeOperationKind::Transfer,
            Self::AliasSetup { .. } => NativeOperationKind::AliasSetup,
        }
    }
    fn instructions(&self, config: &Config) -> Result<Vec<InstructionBox>> {
        match self {
            Self::Transfer {
                destination,
                amount,
            } => {
                if amount.is_zero() || destination == &config.account {
                    eyre::bail!(
                        "transfer requires a positive amount and a different destination account"
                    );
                }
                Ok(vec![
                    Transfer::asset_quantity(
                        AssetId::new(XOR_ASSET_DEFINITION.parse()?, config.account.clone()),
                        amount.clone(),
                        destination.clone(),
                    )
                    .into(),
                ])
            }
            Self::AliasSetup { request, plan } => {
                if plan.body.network_id != config.network_id
                    || plan.body.authority != config.account
                {
                    eyre::bail!("alias plan differs from the exact wallet authority or network");
                }
                // Recovery checks the retained request and plan without applying today's clock to historical evidence.
                decode_and_verify_alias_setup_plan_for_request(request, plan)
            }
        }
    }
}
fn transfer_report(
    journal: &Journal,
    record: &TransactionJournal,
    status: OperationStatus,
    evidence: Option<Value>,
) -> Result<OperationReport> {
    let (kind, operation) = match &record.operation {
        NativeOperation::Transfer {
            destination,
            amount,
        } => (
            "transfer",
            norito::json!({"destination": (destination.to_string()), "asset_definition": XOR_ASSET_DEFINITION, "amount": (amount.to_string())}),
        ),
        NativeOperation::AliasSetup { request, plan } => (
            "alias_setup",
            norito::json!({"request": (request), "plan": (plan)}),
        ),
    };
    let mut data = norito::json!({
        "schema": "iroha.wallet.native-transaction-result.v1", "operation": kind, "status": (status.as_str()),
        "network_id": (record.network_id.to_string()), "chain_discriminant": (record.chain_discriminant),
        "account_id": (record.account_id.to_string()), "transaction_hash": (record.transaction_hash),
        "fee_quote": (record.quote), "deadline_ms": (record.deadline_ms), "journal": (journal.path().display().to_string()), "evidence": evidence,
    });
    if let (Some(target), Some(fields)) = (data.as_object_mut(), operation.as_object()) {
        target.extend(fields.clone());
    }
    Ok(OperationReport { status, data })
}
pub(crate) fn validate_config(config: &Config) -> Result<()> {
    iroha::account_bootstrap::validate_endpoint(&config.torii_api_url)?;
    if config.account.try_signatory() != Some(config.key_pair.public_key()) {
        eyre::bail!("wallet account must match the configured native signing key");
    }
    Ok(())
}
pub(crate) fn current_unix_ms() -> Result<u64> {
    u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
        .map_err(|error| eyre!(error))
}
fn transaction_deadline(transaction: &SignedTransaction) -> Result<u64> {
    let ttl = transaction
        .time_to_live()
        .ok_or_else(|| eyre!("wallet transactions require an exact execution deadline"))?;
    let expiry = transaction
        .creation_time()
        .checked_add(ttl)
        .ok_or_else(|| eyre!("wallet transaction deadline overflow"))?;
    u64::try_from(expiry.as_millis()).map_err(|error| eyre!(error))
}
fn transaction_expired(transaction: &SignedTransaction) -> Result<bool> {
    Ok(current_unix_ms()? >= transaction_deadline(transaction)?)
}
fn add_quantity(
    required: &mut BTreeMap<AssetDefinitionId, Quantity>,
    asset: &AssetDefinitionId,
    amount: &Quantity,
) -> Result<()> {
    let total = required.entry(asset.clone()).or_insert_with(Quantity::zero);
    *total = total.checked_add(amount)?;
    Ok(())
}
pub(crate) struct Observation {
    pub(crate) status: OperationStatus,
    pub(crate) evidence: Option<Value>,
}
pub(crate) fn observe_transaction(
    client: &NativeClient,
    transaction: &SignedTransaction,
) -> Result<Observation> {
    let hash = transaction.hash();
    let Some(status) = client.get_transaction_status_response_global(hash)? else {
        return Ok(Observation {
            status: OperationStatus::Absent,
            evidence: None,
        });
    };
    if status.hash != hex::encode(hash.as_ref()) || status.scope != "global" {
        eyre::bail!("transaction status differs from the saved exact hash or global scope");
    }
    if matches!(status.status.kind.as_str(), "Rejected" | "Expired")
        && status.resolved_from == "state"
    {
        return Ok(Observation {
            status: if status.status.kind == "Expired" {
                OperationStatus::Expired
            } else {
                OperationStatus::Rejected
            },
            evidence: None,
        });
    }
    if status.status.kind != "Applied" || status.resolved_from != "state" {
        if !matches!(
            status.status.kind.as_str(),
            "Queued" | "Approved" | "Committed" | "Applied" | "Rejected" | "Expired"
        ) {
            eyre::bail!("transaction status uses an unsupported first-release state");
        }
        return Ok(Observation {
            status: OperationStatus::Pending,
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
        Err(error) if typed_not_found(&error) => {
            return Ok(Observation {
                status: OperationStatus::Pending,
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
        status: OperationStatus::Applied,
        evidence: Some(norito::json!({
            "kind": "exact_committed_transaction", "block_height": block_height,
            "entrypoint_hash": (transaction.hash_as_entrypoint().to_string()),
        })),
    })
}
pub(crate) fn typed_not_found(error: &eyre::Report) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<iroha::query::QueryError>(),
            Some(iroha::query::QueryError::Validation(
                iroha::data_model::ValidationFail::QueryFailed(
                    iroha::data_model::query::error::QueryExecutionFail::NotFound
                )
            ))
        )
    })
}

#[cfg(test)]
#[path = "operations_tests.rs"]
pub(crate) mod tests;
