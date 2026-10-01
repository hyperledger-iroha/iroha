//! Account balances and exact, quoted native operations with durable submission evidence.
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

#[path = "operations_alias.rs"]
mod bounded_alias;
#[path = "operations_parameter.rs"]
mod parameter_update;
#[path = "operations_private_root.rs"]
mod private_root;
use bounded_alias::AliasFeeBounds;
use iroha_data_model::private_dataspace::{
    PrivateDataspaceAnchor, PrivateDataspaceAnchorState, PrivateDataspaceRegistration,
};
pub use parameter_update::ParameterUpdateRequest;
use private_root::BoundedTerms;
pub use private_root::{
    BoundedTransactionOptions, PrivateRootAnchorRequest, PrivateRootRegistrationRequest,
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
    /// Exact committed wire agrees with the configured node's state-resolved application.
    /// Independent parent anchoring additionally requires the attachment service's verified proof.
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
    /// Exact owner-bound compact private-root registration.
    PrivateRootRegistration,
    /// Exact next compact private-root certificate against retained parent cursor state.
    PrivateRootAnchor,
    /// Exact bounded native parameter update, including owner-bound catalog transitions.
    ParameterUpdate,
}

/// Account-authorized shared native wallet operations.
pub struct AccountService {
    config: Config,
    client: Client,
    deadline: Option<std::time::Instant>,
}
impl AccountService {
    /// Bind a native client to one exact configured network, account and signing key.
    ///
    /// # Errors
    /// Rejects embedded endpoint credentials, an invalid signer, or invalid SDK configuration.
    pub fn new(config: Config) -> Result<Self> {
        validate_config(&config)?;
        let client = Client::new(config.clone())?;
        Ok(Self {
            config,
            client,
            deadline: None,
        })
    }
    /// Read this account's exact XOR balance without preparing a transaction.
    ///
    /// # Errors
    /// Returns authentication, routing, account/asset-definition absence or query errors.
    pub fn xor_balance(&self) -> Result<BalanceReport> {
        self.client.balance(&AssetId::new(
            XOR_ASSET_DEFINITION.parse()?,
            self.config.account.clone(),
        ))
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
            request.fee_payment.clone(),
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
        self.prepare_alias_with_bounds(request, fee_payment, AliasFeeBounds::Quoted, journal)
    }

    fn prepare_alias_with_bounds(
        &self,
        request: &AliasSetupPlanRequestV1,
        fee_payment: FeePaymentIntent,
        bounds: AliasFeeBounds,
        journal: &Path,
    ) -> Result<OperationReport> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        self.ensure_deadline()?;
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
                plan: Box::new(plan),
                bounds,
            },
            fee_payment,
            journal,
        )
    }
    fn ensure_deadline(&self) -> Result<()> {
        if self
            .deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            eyre::bail!("wallet operation deadline elapsed");
        }
        Ok(())
    }
    fn prepare_native(
        &self,
        operation: NativeOperation,
        requested_fee: FeePaymentIntent,
        journal: &Path,
    ) -> Result<OperationReport> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        self.ensure_deadline()?;
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
        if let Some(terms) = operation.bounded_terms() {
            terms.validate()?;
            let remaining = terms
                .deadline_ms
                .checked_sub(payload.creation_time_ms)
                .and_then(std::num::NonZeroU64::new)
                .ok_or_else(|| eyre!("bounded transaction deadline elapsed before preparation"))?;
            payload.time_to_live_ms = Some(
                payload
                    .time_to_live_ms
                    .map_or(remaining, |ttl| ttl.min(remaining)),
            );
        }
        let quote = self
            .client
            .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })?;
        verify_quote_limits(&requested_fee, &quote)?;
        if let Some(terms) = operation.bounded_terms() {
            terms.verify_quote(&quote)?;
        }
        self.client.check_funding(
            &operation.principal(&self.config.account)?,
            std::slice::from_ref(&quote),
        )?;
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
            requested_fee,
            quote,
            transaction_hash: signed.hash().to_string(),
            signed_transaction_hex: hex::encode(signed.encode_versioned()),
            deadline_ms: transaction_deadline(&signed)?,
        };
        record.verify(&self.config)?;
        if let Some(terms) = record.operation.bounded_terms()
            && current_unix_ms()? >= terms.deadline_ms
        {
            eyre::bail!("bounded operation deadline elapsed before journal publication");
        }
        self.ensure_deadline()?;
        let journal = Journal::create_prepared(journal, &record)?;
        Ok(transfer_report(
            &journal,
            &record,
            OperationStatus::Prepared,
            None,
        ))
    }
    /// Submit a wholly unattempted saved transfer or alias operation once, then verify its wire.
    ///
    /// An existing attempt is reconciled by hash without another submission.
    /// Bounded operations require their request-bound submission methods.
    ///
    /// # Errors
    /// Returns invalid/unsafe journal or untrusted observation errors. Unresolved dispatch returns Pending.
    pub fn submit(&self, journal: &Path, expected: NativeOperationKind) -> Result<OperationReport> {
        self.run_transaction(journal, expected, true, None)
    }
    /// Read-only reconciliation of a saved transfer or alias operation without rebuilding or signing.
    ///
    /// Bounded operations require their request-bound recovery methods.
    ///
    /// # Errors
    /// Returns changed identity, unsafe evidence or malformed status/committed-wire errors.
    pub fn resume(&self, journal: &Path, expected: NativeOperationKind) -> Result<OperationReport> {
        self.run_transaction(journal, expected, false, None)
    }
    fn run_transaction(
        &self,
        path: &Path,
        expected: NativeOperationKind,
        submit: bool,
        expectation: Option<private_root::BoundedOperationExpectation<'_>>,
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
        match expectation {
            Some(expectation) => expectation.verify(&record)?,
            None if record.operation.bounded_terms().is_some() => {
                eyre::bail!(
                    "bounded submission and recovery require the exact selected operation request"
                );
            }
            None => {}
        }
        let mut before = observe_transaction(self.client.client(), &transaction)?;
        if before.status == OperationStatus::Absent && journal.submission_recorded(&record)? {
            before.status = OperationStatus::Pending;
        }
        if !submit || before.status != OperationStatus::Absent {
            return Ok(transfer_report(
                &journal,
                &record,
                before.status,
                before.evidence.as_ref(),
            ));
        }
        if transaction_expired(&transaction)? {
            return Ok(transfer_report(
                &journal,
                &record,
                OperationStatus::Expired,
                None,
            ));
        }
        self.client.refresh_capabilities().wrap_err(
            "wallet transaction submission compatibility; saved operation remains unattempted",
        )?;
        self.ensure_deadline()?;
        if !journal.record_submission(&record)? {
            return Ok(transfer_report(
                &journal,
                &record,
                OperationStatus::Pending,
                None,
            ));
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
            journal.write_applied_evidence(&after.evidence)?;
        }
        Ok(transfer_report(
            &journal,
            &record,
            status,
            after.evidence.as_ref(),
        ))
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
        verify_quote_limits(&self.requested_fee, &self.quote)?;
        if let Some(terms) = self.operation.bounded_terms() {
            terms.verify_quote(&self.quote)?;
            if self.deadline_ms > terms.deadline_ms {
                eyre::bail!("saved bounded transaction exceeds its original operation deadline");
            }
        }
        self.quote
            .validate_for_draft(transaction.payload())
            .map_err(|error| eyre!(error))?;
        Ok(transaction)
    }
}
fn verify_quote_limits(requested: &FeePaymentIntent, quote: &FeeQuoteResponse) -> Result<()> {
    verify_fee_intent_limits(requested, &quote.intent)
}

/// Require the original payer/gas selection and every explicitly authorized component maximum.
///
/// # Errors
/// Rejects invalid intents, changed payer or gas, and unlisted or increased fee components.
pub(crate) fn verify_fee_intent_limits(
    requested: &FeePaymentIntent,
    actual: &FeePaymentIntent,
) -> Result<()> {
    requested.validate()?;
    actual.validate()?;
    if !requested.has_same_payer_and_gas_bound(actual) {
        eyre::bail!("fee quote changed the selected payer, sponsor revision or gas bound");
    }
    if !requested.charge_limits().is_empty() {
        for quoted in actual.charge_limits() {
            let retained = requested
                .charge_limits()
                .iter()
                .find(|limit| {
                    limit.kind() == quoted.kind()
                        && limit.asset_definition_id() == quoted.asset_definition_id()
                })
                .ok_or_else(|| {
                    eyre!("fee quote added a component outside the explicit caller limits")
                })?;
            if quoted.max_amount() > retained.max_amount() {
                eyre::bail!("fee quote increased an explicit caller maximum");
            }
        }
    }
    Ok(())
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
    ParameterUpdate {
        parameter: iroha_data_model::parameter::Parameter,
        terms: BoundedTerms,
    },
    Transfer {
        destination: AccountId,
        amount: Quantity,
    },
    AliasSetup {
        request: AliasSetupPlanRequestV1,
        plan: Box<AliasTransactionPlanV1>,
        bounds: AliasFeeBounds,
    },
    PrivateRootRegistration {
        alias: String,
        expected_ownership_generation: u64,
        registration: Box<PrivateDataspaceRegistration>,
        terms: BoundedTerms,
    },
    PrivateRootAnchor {
        state: Box<PrivateDataspaceAnchorState>,
        anchor: Box<PrivateDataspaceAnchor>,
        terms: BoundedTerms,
    },
}
impl NativeOperation {
    fn bounded_terms(&self) -> Option<&BoundedTerms> {
        match self {
            Self::PrivateRootRegistration { terms, .. }
            | Self::PrivateRootAnchor { terms, .. }
            | Self::ParameterUpdate { terms, .. } => Some(terms),
            Self::AliasSetup {
                bounds: AliasFeeBounds::Bounded(terms),
                ..
            } => Some(terms),
            Self::Transfer { .. } | Self::AliasSetup { .. } => None,
        }
    }
    fn principal(&self, authority: &AccountId) -> Result<BTreeMap<AssetId, Quantity>> {
        match self {
            Self::PrivateRootRegistration { .. }
            | Self::PrivateRootAnchor { .. }
            | Self::ParameterUpdate { .. } => Ok(BTreeMap::new()),
            Self::Transfer { amount, .. } => Ok(BTreeMap::from([(
                AssetId::new(XOR_ASSET_DEFINITION.parse()?, authority.clone()),
                amount.clone(),
            )])),
            Self::AliasSetup { plan, .. } => {
                let mut result = BTreeMap::new();
                for total in &plan.body.totals_by_asset {
                    add_quantity(
                        &mut result,
                        &AssetId::new(total.payment_asset.clone(), authority.clone()),
                        &total.amount,
                    )?;
                }
                Ok(result)
            }
        }
    }
    fn kind(&self) -> NativeOperationKind {
        match self {
            Self::Transfer { .. } => NativeOperationKind::Transfer,
            Self::AliasSetup { .. } => NativeOperationKind::AliasSetup,
            Self::PrivateRootRegistration { .. } => NativeOperationKind::PrivateRootRegistration,
            Self::PrivateRootAnchor { .. } => NativeOperationKind::PrivateRootAnchor,
            Self::ParameterUpdate { .. } => NativeOperationKind::ParameterUpdate,
        }
    }
    fn instructions(&self, config: &Config) -> Result<Vec<InstructionBox>> {
        match self {
            Self::ParameterUpdate { parameter, terms } => {
                terms.validate()?;
                Ok(vec![
                    iroha_data_model::isi::SetParameter::new(parameter.clone()).into(),
                ])
            }
            Self::PrivateRootRegistration {
                alias,
                expected_ownership_generation,
                registration,
                terms,
            } => {
                terms.validate()?;
                Ok(vec![private_root::registration_instruction(
                    config,
                    alias,
                    *expected_ownership_generation,
                    registration,
                )?])
            }
            Self::PrivateRootAnchor {
                state,
                anchor,
                terms,
            } => {
                terms.validate()?;
                Ok(vec![private_root::anchor_instruction(
                    config, state, anchor,
                )?])
            }
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
            Self::AliasSetup {
                request,
                plan,
                bounds,
            } => {
                if let AliasFeeBounds::Bounded(terms) = bounds {
                    terms.validate()?;
                }
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
    evidence: Option<&Value>,
) -> OperationReport {
    let (kind, operation) = match &record.operation {
        NativeOperation::ParameterUpdate { parameter, terms } => {
            let parameter_json =
                norito::json::to_value(parameter).expect("native parameter serializes");
            (
                "parameter_update",
                norito::json!({"parameter": parameter_json, "terms": terms}),
            )
        }
        NativeOperation::PrivateRootRegistration {
            alias,
            expected_ownership_generation,
            registration,
            terms,
        } => (
            "private_root_registration",
            norito::json!({"alias": alias, "expected_ownership_generation": expected_ownership_generation, "registration": registration, "terms": terms}),
        ),
        NativeOperation::PrivateRootAnchor {
            state,
            anchor,
            terms,
        } => (
            "private_root_anchor",
            norito::json!({"registration": (state.registration()), "previous_cursor": (state.cursor()), "anchor": anchor, "terms": terms}),
        ),
        NativeOperation::Transfer {
            destination,
            amount,
        } => (
            "transfer",
            norito::json!({"destination": (destination.to_string()), "asset_definition": XOR_ASSET_DEFINITION, "amount": (amount.to_string())}),
        ),
        NativeOperation::AliasSetup {
            request,
            plan,
            bounds,
        } => (
            "alias_setup",
            norito::json!({"request": (request), "plan": (plan), "fee_bounds": bounds}),
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
    OperationReport { status, data }
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
fn add_quantity<K: Ord + Clone>(
    required: &mut BTreeMap<K, Quantity>,
    asset: &K,
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
