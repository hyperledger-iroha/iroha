//! Production adapter boundaries for the finalized-chain SoraFS moderation orchestrator.
//!
//! These adapters intentionally own no moderation consensus state. Transaction
//! idempotency and terminal handoff deduplication are delegated to injected
//! durable boundaries, while finalized projections are read from one immutable
//! [`State::query_view`] and cross-checked through the native committed-event
//! query.

use std::{sync::Arc, time::Duration};

use iroha_core::{
    queue::Queue,
    smartcontracts::ValidSingularQuery,
    state::{
        State, StateQueryView, StateReadOnly, StateReadOnlyWithTransactions, TransactionsReadOnly,
    },
};
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    query::sorafs::prelude::{FindSorafsModerationEvents, FindSorafsModerationSnapshot},
    sorafs::moderation_ledger::{
        MODERATION_FINALIZED_SNAPSHOT_VERSION_V1, MODERATION_QUERY_MAX_CASES_V1,
        MODERATION_QUERY_MAX_EVENTS_V1, ModerationFinalizedEventCursorV1,
        ModerationFinalizedEventPageV1, ModerationFinalizedLedgerSnapshotV1,
        is_canonical_moderation_identifier_v1,
    },
    transaction::{
        Executable, FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionEntrypoint,
        TransactionPayload,
    },
};
use mv::storage::StorageReadOnly;
use sorafs_node::moderation_orchestrator::{
    MODERATION_SIGNED_TRANSACTION_MAX_BYTES_V1, MODERATION_TRANSACTION_TTL_MS_V1,
    ModerationFinalizedSnapshotReaderV1, ModerationHandoffFailureV1, ModerationSignedTransactionV1,
    ModerationSnapshotReadErrorV1, ModerationSubmissionFailureV1, ModerationSubmissionLookupV1,
    ModerationTerminalHandoffKindV1, ModerationTerminalHandoffSinkV1, ModerationTerminalHandoffV1,
    ModerationTransactionReceiptV1, ModerationTransactionRequestV1,
    ModerationTransactionSubmitterV1,
};

const MODERATION_HANDOFF_MAX_BYTES_V1: usize = 64 * 1024;
const DEFAULT_MODERATION_EVENT_PAGE_SIZE_V1: u32 = 256;
const MODERATION_TRANSACTION_TTL_V1: Duration =
    Duration::from_millis(MODERATION_TRANSACTION_TTL_MS_V1);
const MODERATION_TRANSACTION_PAYLOAD_MAX_BYTES_V1: usize = 4 * 1024 * 1024;

/// Fixed runtime signing failures that are safe to surface to the orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationSigningFailureV1 {
    /// The HSM or signer service is temporarily unavailable.
    Unavailable,
    /// The signer queue is full and no signature was produced.
    Backpressure,
    /// The signer permanently refused the exact request.
    Refused,
}

/// Runtime-only signer for one exact native moderation transaction.
///
/// Implementations may delegate to PKCS#11 or a remote HSM. A returned
/// envelope is durably retained by the orchestrator before ingress; signing
/// itself is never used as an idempotency or crash-recovery boundary.
pub trait ModerationSignedTransactionSignerV1: Send + Sync {
    /// Sign the exact fee-quoted payload supplied by Torii.
    ///
    /// # Errors
    ///
    /// Returns a fixed failure class without provider diagnostics when no
    /// acceptable signature was produced.
    fn sign(
        &self,
        payload: TransactionPayload,
    ) -> Result<SignedTransaction, ModerationSigningFailureV1>;
}

impl ModerationSignedTransactionSignerV1 for KeyPair {
    fn sign(
        &self,
        payload: TransactionPayload,
    ) -> Result<SignedTransaction, ModerationSigningFailureV1> {
        TransactionBuilder::from_payload(payload)
            .and_then(|builder| builder.try_sign(self.private_key()))
            .map_err(|_| ModerationSigningFailureV1::Refused)
    }
}

/// Fixed fee-quote failures safe to return across the signer boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationFeeQuoteFailureV1 {
    /// The finalized fee view or routing policy is temporarily unavailable.
    Unavailable,
    /// The exact payload cannot satisfy governed fee policy.
    Rejected,
}

/// Runtime fee quoter used after Torii has built the exact V1 payload.
pub trait ModerationFeeQuoterV1: Send + Sync {
    /// Quote the signature-bound fee intent without changing any other field.
    fn quote(
        &self,
        payload: &TransactionPayload,
    ) -> Result<FeePaymentIntent, ModerationFeeQuoteFailureV1>;
}

/// Receipt returned by the strict, durable transaction ingress boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModerationStrictIngressReceiptV1 {
    /// Hash of the exact signed transaction durably admitted by ingress.
    pub transaction_id: [u8; 32],
    /// Finalized height observed while admitting or replaying the operation.
    pub observed_finalized_height: u64,
    /// Whether ingress returned an already retained operation.
    pub replay: bool,
}

/// Fixed strict-ingress failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationStrictIngressFailureV1 {
    /// No admission occurred and a later retry is safe.
    Unavailable,
    /// No admission occurred because the bounded ingress queue is full.
    Backpressure,
    /// Admission may have occurred; lookup by `operation_id` is required.
    Ambiguous,
    /// The signed transaction was permanently rejected before admission.
    PermanentRejection,
    /// Runtime-only policy or credentials are unavailable.
    RuntimeUnavailable,
}

/// Strict signed-transaction ingress used by the moderation adapter.
///
/// The orchestrator has already persisted the exact operation-to-transaction
/// binding before `submit_exact`. Ingress must run the canonical Torii
/// signature, chain, fee, queue-plan, and durable-admission checks without
/// replacing that transaction. Distinct envelopes signed by racing replicas
/// are resolved by native ledger CAS semantics and finalized reconciliation;
/// no process-local operation map is authoritative.
pub trait ModerationStrictTransactionIngressV1: Send + Sync {
    /// Durably admit or replay one exact signed transaction.
    ///
    /// # Errors
    ///
    /// Returns a fixed admission class. An ambiguous result must be resolved
    /// with [`Self::lookup_exact`] before any replacement is signed.
    fn submit_exact(
        &self,
        request: &ModerationTransactionRequestV1,
        transaction: SignedTransaction,
    ) -> Result<ModerationStrictIngressReceiptV1, ModerationStrictIngressFailureV1>;

    /// Resolve a retained operation through durable ingress/committed state.
    fn lookup_exact(
        &self,
        operation_id: [u8; 32],
        transaction_id: Option<[u8; 32]>,
    ) -> ModerationSubmissionLookupV1;
}

/// Fail-closed bridge from moderation operations to signed Torii ingress.
pub struct ModerationTransactionSubmitterAdapterV1 {
    chain_id: ChainId,
    signer: Arc<dyn ModerationSignedTransactionSignerV1>,
    fee_quoter: Arc<dyn ModerationFeeQuoterV1>,
    ingress: Arc<dyn ModerationStrictTransactionIngressV1>,
}

impl core::fmt::Debug for ModerationTransactionSubmitterAdapterV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ModerationTransactionSubmitterAdapterV1")
            .field("chain_id", &self.chain_id)
            .field("signer", &"<runtime-only>")
            .field("fee_quoter", &"<finalized-policy>")
            .field("ingress", &"<durable-strict-ingress>")
            .finish()
    }
}

impl ModerationTransactionSubmitterAdapterV1 {
    /// Construct a submitter for one exact chain.
    #[must_use]
    pub fn new(
        chain_id: ChainId,
        signer: Arc<dyn ModerationSignedTransactionSignerV1>,
        fee_quoter: Arc<dyn ModerationFeeQuoterV1>,
        ingress: Arc<dyn ModerationStrictTransactionIngressV1>,
    ) -> Self {
        Self {
            chain_id,
            signer,
            fee_quoter,
            ingress,
        }
    }
}

impl ModerationTransactionSubmitterV1 for ModerationTransactionSubmitterAdapterV1 {
    fn chain_id(&self) -> ChainId {
        self.chain_id.clone()
    }

    fn sign(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1> {
        validate_moderation_transaction_request(request)?;
        let mut builder = TransactionBuilder::new(
            self.chain_id.clone(),
            request.authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([request.action.instruction()]);
        builder.set_ttl(MODERATION_TRANSACTION_TTL_V1);
        let mut payload = builder
            .into_payload()
            .map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)?;
        validate_unsigned_moderation_payload(&self.chain_id, request, &payload)?;
        payload.fee_payment = self
            .fee_quoter
            .quote(&payload)
            .map_err(|error| match error {
                ModerationFeeQuoteFailureV1::Unavailable => {
                    ModerationSubmissionFailureV1::RuntimeUnavailable
                }
                ModerationFeeQuoteFailureV1::Rejected => {
                    ModerationSubmissionFailureV1::PermanentRejection
                }
            })?;
        validate_unsigned_moderation_payload(&self.chain_id, request, &payload)?;
        let expected_payload = payload.clone();
        let transaction = self.signer.sign(payload).map_err(map_signing_failure)?;
        if transaction.payload() != &expected_payload {
            return Err(ModerationSubmissionFailureV1::PermanentRejection);
        }
        validate_signed_moderation_transaction(&self.chain_id, request, &transaction)?;
        ModerationSignedTransactionV1::from_signed_transaction(request, &transaction)
    }

    fn submit_signed(
        &self,
        request: &ModerationTransactionRequestV1,
        signed: &ModerationSignedTransactionV1,
    ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
        validate_moderation_transaction_request(request)?;
        let transaction = signed.decode_for_request(request)?;
        validate_signed_moderation_transaction(&self.chain_id, request, &transaction)?;
        let expected_transaction_id = signed.transaction_id;
        let receipt = self
            .ingress
            .submit_exact(request, transaction)
            .map_err(map_ingress_failure)?;
        if receipt.transaction_id != expected_transaction_id
            || receipt.observed_finalized_height < request.baseline_finalized_height
        {
            // Ingress may already have accepted a transaction. Reconciliation
            // is mandatory; signing or submitting a replacement is unsafe.
            return Err(ModerationSubmissionFailureV1::Ambiguous);
        }
        Ok(ModerationTransactionReceiptV1 {
            transaction_id: receipt.transaction_id,
            observed_finalized_height: receipt.observed_finalized_height,
        })
    }

    fn lookup(
        &self,
        operation_id: [u8; 32],
        transaction_id: Option<[u8; 32]>,
    ) -> ModerationSubmissionLookupV1 {
        if operation_id == [0; 32] || transaction_id == Some([0; 32]) {
            return ModerationSubmissionLookupV1::Unknown;
        }
        sanitize_submission_lookup(
            self.ingress.lookup_exact(operation_id, transaction_id),
            transaction_id,
        )
    }
}

fn validate_moderation_transaction_request(
    request: &ModerationTransactionRequestV1,
) -> Result<(), ModerationSubmissionFailureV1> {
    request
        .validate()
        .map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)
}

fn validate_unsigned_moderation_payload(
    chain_id: &ChainId,
    request: &ModerationTransactionRequestV1,
    payload: &TransactionPayload,
) -> Result<(), ModerationSubmissionFailureV1> {
    let canonical =
        norito::to_bytes(payload).map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)?;
    let expected_ttl_ms =
        u64::try_from(MODERATION_TRANSACTION_TTL_V1.as_millis()).unwrap_or(u64::MAX);
    if canonical.is_empty()
        || canonical.len() > MODERATION_TRANSACTION_PAYLOAD_MAX_BYTES_V1
        || request.chain_id != *chain_id
        || payload.chain != *chain_id
        || payload.authority != request.authority
        || payload.creation_time_ms == 0
        || payload.time_to_live_ms.map(core::num::NonZeroU64::get) != Some(expected_ttl_ms)
        || payload.nonce.is_some()
        || !payload.metadata.is_empty()
        || payload.fee_payment.validate().is_err()
    {
        return Err(ModerationSubmissionFailureV1::PermanentRejection);
    }
    let expected = request.action.instruction();
    match &payload.instructions {
        Executable::Instructions(instructions)
            if instructions.len() == 1 && instructions.first() == Some(&expected) =>
        {
            Ok(())
        }
        _ => Err(ModerationSubmissionFailureV1::PermanentRejection),
    }
}

fn validate_signed_moderation_transaction(
    chain_id: &ChainId,
    request: &ModerationTransactionRequestV1,
    transaction: &SignedTransaction,
) -> Result<(), ModerationSubmissionFailureV1> {
    if transaction.verify_signature().is_err()
        || request.chain_id != *chain_id
        || transaction.chain() != chain_id
        || transaction.authority() != &request.authority
    {
        return Err(ModerationSubmissionFailureV1::PermanentRejection);
    }
    let expected = request.action.instruction();
    match transaction.instructions() {
        Executable::Instructions(instructions)
            if instructions.len() == 1 && instructions.first() == Some(&expected) =>
        {
            Ok(())
        }
        _ => Err(ModerationSubmissionFailureV1::PermanentRejection),
    }
}

/// Canonical Torii fee quoter for the exact moderation payload.
pub(crate) struct ToriiModerationFeeQuoterV1 {
    chain_id: Arc<ChainId>,
    queue: Arc<Queue>,
    state: Arc<State>,
}

impl core::fmt::Debug for ToriiModerationFeeQuoterV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ToriiModerationFeeQuoterV1")
            .field("chain_id", &self.chain_id)
            .field("queue", &"<canonical-routing>")
            .field("state", &"<finalized-fee-view>")
            .finish()
    }
}

impl ToriiModerationFeeQuoterV1 {
    #[must_use]
    pub(crate) fn new(chain_id: Arc<ChainId>, queue: Arc<Queue>, state: Arc<State>) -> Self {
        Self {
            chain_id,
            queue,
            state,
        }
    }
}

impl ModerationFeeQuoterV1 for ToriiModerationFeeQuoterV1 {
    fn quote(
        &self,
        payload: &TransactionPayload,
    ) -> Result<FeePaymentIntent, ModerationFeeQuoteFailureV1> {
        crate::quote_internal_fee_payment_from_parts(
            self.chain_id.as_ref(),
            self.queue.as_ref(),
            self.state.as_ref(),
            payload,
        )
        .map_err(|_| ModerationFeeQuoteFailureV1::Rejected)
    }
}

/// Canonical local strict-durable ingress and exact finalized transaction observer.
pub(crate) struct ToriiModerationStrictTransactionIngressV1 {
    chain_id: Arc<ChainId>,
    queue: Arc<Queue>,
    state: Arc<State>,
    telemetry: crate::routing::MaybeTelemetry,
    pipeline_status_cache: Arc<crate::PipelineStatusCache>,
}

impl core::fmt::Debug for ToriiModerationStrictTransactionIngressV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ToriiModerationStrictTransactionIngressV1")
            .field("chain_id", &self.chain_id)
            .field("queue", &"<strict-durable>")
            .field("state", &"<authoritative>")
            .field("pipeline_status_cache", &"<positive-hints-only>")
            .finish()
    }
}

impl ToriiModerationStrictTransactionIngressV1 {
    #[must_use]
    pub(crate) fn new(
        chain_id: Arc<ChainId>,
        queue: Arc<Queue>,
        state: Arc<State>,
        telemetry: crate::routing::MaybeTelemetry,
        pipeline_status_cache: Arc<crate::PipelineStatusCache>,
    ) -> Self {
        Self {
            chain_id,
            queue,
            state,
            telemetry,
            pipeline_status_cache,
        }
    }

    fn validate_retained_baseline(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<u64, ModerationStrictIngressFailureV1> {
        let view = self.state.view();
        let observed_finalized_height = u64::try_from(view.block_hashes().len())
            .map_err(|_| ModerationStrictIngressFailureV1::Unavailable)?;
        let baseline_index = usize::try_from(request.baseline_finalized_height)
            .ok()
            .and_then(|height| height.checked_sub(1))
            .ok_or(ModerationStrictIngressFailureV1::PermanentRejection)?;
        if observed_finalized_height == 0 {
            return Err(ModerationStrictIngressFailureV1::Unavailable);
        }
        let Some(baseline_hash) = view.block_hashes().get(baseline_index) else {
            return Err(ModerationStrictIngressFailureV1::Unavailable);
        };
        if baseline_hash.as_ref() != &request.baseline_finalized_block_hash {
            return Err(ModerationStrictIngressFailureV1::PermanentRejection);
        }
        Ok(observed_finalized_height)
    }

    fn has_positive_pending_hint(&self, transaction_hash: &HashOf<SignedTransaction>) -> bool {
        self.queue
            .contains_pending_hash(transaction_hash.clone(), self.state.as_ref())
            || self
                .pipeline_status_cache
                .lookup(transaction_hash)
                .is_some_and(|entry| {
                    matches!(
                        entry.kind,
                        crate::PipelineStatusKind::Queued
                            | crate::PipelineStatusKind::Approved
                            | crate::PipelineStatusKind::Committed
                            | crate::PipelineStatusKind::Applied
                    )
                })
    }
}

impl ModerationStrictTransactionIngressV1 for ToriiModerationStrictTransactionIngressV1 {
    fn submit_exact(
        &self,
        request: &ModerationTransactionRequestV1,
        transaction: SignedTransaction,
    ) -> Result<ModerationStrictIngressReceiptV1, ModerationStrictIngressFailureV1> {
        if request.operation_id == [0; 32]
            || transaction.chain() != self.chain_id.as_ref()
            || *transaction.hash().as_ref() == [0; 32]
        {
            return Err(ModerationStrictIngressFailureV1::PermanentRejection);
        }
        let observed_finalized_height = self.validate_retained_baseline(request)?;
        let transaction_id = *transaction.hash().as_ref();
        let accepted = crate::routing::accept_transaction_for_ingress(
            Arc::clone(&self.chain_id),
            Arc::clone(&self.state),
            transaction,
            &self.telemetry,
        )
        .map_err(|error| match error {
            crate::Error::AcceptTransaction(
                iroha_core::tx::AcceptTransactionFail::NetworkTimeUnhealthy { .. }
                | iroha_core::tx::AcceptTransactionFail::TransactionInTheFuture,
            ) => ModerationStrictIngressFailureV1::Unavailable,
            crate::Error::AcceptTransaction(_) => {
                ModerationStrictIngressFailureV1::PermanentRejection
            }
            _ => ModerationStrictIngressFailureV1::Unavailable,
        })?;
        let routing_plan = self
            .queue
            .durable_plan_admission_claim_with_state(&accepted, self.state.as_ref())
            .map_err(|_| ModerationStrictIngressFailureV1::Unavailable)?
            .map_or_else(
                || {
                    self.queue
                        .route_plan_with_state(&accepted, self.state.as_ref())
                },
                |claim| Ok(claim.routing_plan),
            )
            .map_err(|_| ModerationStrictIngressFailureV1::Unavailable)?;
        match crate::routing::push_accepted_transaction_for_ingress_with_routing_plan_strict_durable(
            Arc::clone(&self.queue),
            Arc::clone(&self.state),
            accepted,
            routing_plan,
        ) {
            Ok(_) => Ok(ModerationStrictIngressReceiptV1 {
                transaction_id,
                observed_finalized_height,
                replay: false,
            }),
            Err(crate::Error::PushIntoQueue { source, .. }) => match source.as_ref() {
                iroha_core::queue::Error::InBlockchain | iroha_core::queue::Error::IsInQueue => {
                    Ok(ModerationStrictIngressReceiptV1 {
                        transaction_id,
                        observed_finalized_height,
                        replay: true,
                    })
                }
                iroha_core::queue::Error::Full
                | iroha_core::queue::Error::LatencySaturated
                | iroha_core::queue::Error::MaximumTransactionsPerUser => {
                    Err(ModerationStrictIngressFailureV1::Backpressure)
                }
                iroha_core::queue::Error::PlanJournalDurabilityIndeterminate { .. } => {
                    Err(ModerationStrictIngressFailureV1::Ambiguous)
                }
                iroha_core::queue::Error::PlanJournalDurabilityRejected { .. }
                | iroha_core::queue::Error::UnresolvedRoute { .. } => {
                    Err(ModerationStrictIngressFailureV1::Unavailable)
                }
                iroha_core::queue::Error::Expired
                | iroha_core::queue::Error::Governance(_)
                | iroha_core::queue::Error::GovernanceNotPermitted { .. }
                | iroha_core::queue::Error::LaneComplianceDenied { .. }
                | iroha_core::queue::Error::LanePrivacyProofRejected { .. }
                | iroha_core::queue::Error::NexusFeeAdmissionRejected { .. }
                | iroha_core::queue::Error::NexusFeeAdmissionConfigInvalid { .. }
                | iroha_core::queue::Error::ConfidentialPolicyAdmissionRejected { .. } => {
                    Err(ModerationStrictIngressFailureV1::PermanentRejection)
                }
            },
            Err(_) => Err(ModerationStrictIngressFailureV1::Unavailable),
        }
    }

    fn lookup_exact(
        &self,
        operation_id: [u8; 32],
        transaction_id: Option<[u8; 32]>,
    ) -> ModerationSubmissionLookupV1 {
        let Some(transaction_id) = transaction_id.filter(|id| *id != [0; 32]) else {
            return ModerationSubmissionLookupV1::Unknown;
        };
        if operation_id == [0; 32] {
            return ModerationSubmissionLookupV1::Unknown;
        }
        let transaction_hash = HashOf::from_untyped_unchecked(Hash::prehashed(transaction_id));
        let view = self.state.view();
        let Some(observed_finalized_height) = u64::try_from(view.block_hashes().len())
            .ok()
            .filter(|height| *height != 0)
        else {
            return ModerationSubmissionLookupV1::Unknown;
        };
        let Some(block_height) = view.transactions().get(&transaction_hash) else {
            drop(view);
            return if self.has_positive_pending_hint(&transaction_hash) {
                ModerationSubmissionLookupV1::Pending { transaction_id }
            } else {
                ModerationSubmissionLookupV1::NotFound {
                    observed_finalized_height,
                }
            };
        };
        if block_height.get() > view.block_hashes().len() {
            return ModerationSubmissionLookupV1::Unknown;
        }
        let Some(expected_block_hash) = view
            .block_hashes()
            .get(block_height.get().saturating_sub(1))
            .copied()
        else {
            return ModerationSubmissionLookupV1::Unknown;
        };
        let Some(block) = view.kura().get_block(block_height) else {
            return ModerationSubmissionLookupV1::Unknown;
        };
        let Ok(block_height_u64) = u64::try_from(block_height.get()) else {
            return ModerationSubmissionLookupV1::Unknown;
        };
        if block.header().height().get() != block_height_u64 || block.hash() != expected_block_hash
        {
            return ModerationSubmissionLookupV1::Unknown;
        }
        let external_entrypoint_count = block.external_entrypoint_count();
        let mut exact_results = block
            .entrypoint_results()
            .take(external_entrypoint_count)
            .filter_map(|(_, entrypoint, result)| {
                let TransactionEntrypoint::External(transaction) = entrypoint else {
                    return None;
                };
                (transaction.hash() == transaction_hash).then_some(result.0.is_ok())
            });
        match (exact_results.next(), exact_results.next()) {
            (Some(true), None) => ModerationSubmissionLookupV1::Applied { transaction_id },
            (Some(false), None) => ModerationSubmissionLookupV1::Rejected {
                transaction_id: Some(transaction_id),
                observed_finalized_height,
            },
            _ => ModerationSubmissionLookupV1::Unknown,
        }
    }
}

fn map_signing_failure(error: ModerationSigningFailureV1) -> ModerationSubmissionFailureV1 {
    match error {
        ModerationSigningFailureV1::Unavailable => {
            ModerationSubmissionFailureV1::RuntimeUnavailable
        }
        ModerationSigningFailureV1::Backpressure => {
            ModerationSubmissionFailureV1::NotSubmittedBackpressure
        }
        ModerationSigningFailureV1::Refused => ModerationSubmissionFailureV1::PermanentRejection,
    }
}

fn map_ingress_failure(error: ModerationStrictIngressFailureV1) -> ModerationSubmissionFailureV1 {
    match error {
        ModerationStrictIngressFailureV1::Unavailable => {
            ModerationSubmissionFailureV1::NotSubmittedUnavailable
        }
        ModerationStrictIngressFailureV1::Backpressure => {
            ModerationSubmissionFailureV1::NotSubmittedBackpressure
        }
        ModerationStrictIngressFailureV1::Ambiguous => ModerationSubmissionFailureV1::Ambiguous,
        ModerationStrictIngressFailureV1::PermanentRejection => {
            ModerationSubmissionFailureV1::PermanentRejection
        }
        ModerationStrictIngressFailureV1::RuntimeUnavailable => {
            ModerationSubmissionFailureV1::RuntimeUnavailable
        }
    }
}

fn sanitize_submission_lookup(
    lookup: ModerationSubmissionLookupV1,
    expected_transaction_id: Option<[u8; 32]>,
) -> ModerationSubmissionLookupV1 {
    let matches_expected = |candidate: [u8; 32]| {
        candidate != [0; 32] && expected_transaction_id.is_none_or(|expected| expected == candidate)
    };
    match lookup {
        ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height,
        } if observed_finalized_height != 0 => ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height,
        },
        ModerationSubmissionLookupV1::Pending { transaction_id }
            if matches_expected(transaction_id) =>
        {
            ModerationSubmissionLookupV1::Pending { transaction_id }
        }
        ModerationSubmissionLookupV1::Applied { transaction_id }
            if matches_expected(transaction_id) =>
        {
            ModerationSubmissionLookupV1::Applied { transaction_id }
        }
        ModerationSubmissionLookupV1::Rejected {
            transaction_id,
            observed_finalized_height,
        } if observed_finalized_height != 0
            && transaction_id.is_none_or(matches_expected)
            && transaction_id != Some([0; 32]) =>
        {
            ModerationSubmissionLookupV1::Rejected {
                transaction_id,
                observed_finalized_height,
            }
        }
        ModerationSubmissionLookupV1::Unknown
        | ModerationSubmissionLookupV1::NotFound { .. }
        | ModerationSubmissionLookupV1::Pending { .. }
        | ModerationSubmissionLookupV1::Applied { .. }
        | ModerationSubmissionLookupV1::Rejected { .. } => ModerationSubmissionLookupV1::Unknown,
    }
}

/// Finalized snapshot reader backed directly by native state queries.
pub struct ModerationStateSnapshotReaderV1 {
    state: Arc<State>,
    event_page_size: u32,
}

impl core::fmt::Debug for ModerationStateSnapshotReaderV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ModerationStateSnapshotReaderV1")
            .field("state", &"<immutable-query-view>")
            .field("event_page_size", &self.event_page_size)
            .finish()
    }
}

impl ModerationStateSnapshotReaderV1 {
    /// Construct a reader with the default bounded committed-event page size.
    #[must_use]
    pub fn new(state: Arc<State>) -> Self {
        Self {
            state,
            event_page_size: DEFAULT_MODERATION_EVENT_PAGE_SIZE_V1,
        }
    }

    /// Construct a reader with an explicit native committed-event page size.
    ///
    /// # Errors
    ///
    /// Returns [`ModerationSnapshotReadErrorV1::ResourceExhausted`] when the
    /// page size is zero or exceeds the native query ceiling.
    pub fn with_event_page_size(
        state: Arc<State>,
        event_page_size: u32,
    ) -> Result<Self, ModerationSnapshotReadErrorV1> {
        if !(1..=MODERATION_QUERY_MAX_EVENTS_V1).contains(&event_page_size) {
            return Err(ModerationSnapshotReadErrorV1::ResourceExhausted);
        }
        Ok(Self {
            state,
            event_page_size,
        })
    }
}

impl ModerationFinalizedSnapshotReaderV1 for ModerationStateSnapshotReaderV1 {
    fn read_finalized_snapshot(
        &self,
        max_cases: usize,
        max_events: usize,
    ) -> Result<ModerationFinalizedLedgerSnapshotV1, ModerationSnapshotReadErrorV1> {
        let max_cases = bounded_query_limit(max_cases, MODERATION_QUERY_MAX_CASES_V1)?;
        let max_events = bounded_query_limit(max_events, MODERATION_QUERY_MAX_EVENTS_V1)?;
        // Both the snapshot and every validation page borrow this exact query
        // view. No field can be observed from a later finalized fork/tip.
        let view = self.state.query_view();
        let queries = StateModerationQueryViewV1 { view: &view };
        let snapshot =
            read_and_validate_snapshot(&queries, max_cases, max_events, self.event_page_size)?;
        validate_snapshot_finalized_block(&view, &snapshot)?;
        Ok(snapshot)
    }
}

fn validate_snapshot_finalized_block(
    view: &impl StateReadOnly,
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
) -> Result<(), ModerationSnapshotReadErrorV1> {
    let block = view
        .latest_block()
        .ok_or(ModerationSnapshotReadErrorV1::Unavailable)?;
    validate_snapshot_finalized_block_fields(
        snapshot,
        block.header().height().get(),
        *block.hash().as_ref(),
        block.header().creation_time_ms,
    )
}

fn validate_snapshot_finalized_block_fields(
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
    block_height: u64,
    block_hash: [u8; 32],
    block_creation_time_ms: u64,
) -> Result<(), ModerationSnapshotReadErrorV1> {
    if block_height != snapshot.finalized_height
        || block_hash != snapshot.finalized_block_hash
        || block_creation_time_ms == 0
        || block_creation_time_ms != snapshot.finalized_at_unix_ms
    {
        return Err(ModerationSnapshotReadErrorV1::InvalidSnapshot);
    }
    Ok(())
}

fn bounded_query_limit(
    requested: usize,
    hard_max: u32,
) -> Result<u32, ModerationSnapshotReadErrorV1> {
    let requested =
        u32::try_from(requested).map_err(|_| ModerationSnapshotReadErrorV1::ResourceExhausted)?;
    if !(1..=hard_max).contains(&requested) {
        return Err(ModerationSnapshotReadErrorV1::ResourceExhausted);
    }
    Ok(requested)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeModerationQueryFailureV1 {
    Unavailable,
}

trait ModerationQueryViewV1 {
    fn snapshot(
        &self,
        max_cases: u32,
        max_events: u32,
    ) -> Result<ModerationFinalizedLedgerSnapshotV1, NativeModerationQueryFailureV1>;

    fn event_page(
        &self,
        expected_finalized_cursor:
            iroha_data_model::sorafs::moderation_ledger::ModerationFinalizedCursorV1,
        after: Option<ModerationFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<ModerationFinalizedEventPageV1, NativeModerationQueryFailureV1>;
}

struct StateModerationQueryViewV1<'view, 'state> {
    view: &'view StateQueryView<'state>,
}

impl ModerationQueryViewV1 for StateModerationQueryViewV1<'_, '_> {
    fn snapshot(
        &self,
        max_cases: u32,
        max_events: u32,
    ) -> Result<ModerationFinalizedLedgerSnapshotV1, NativeModerationQueryFailureV1> {
        FindSorafsModerationSnapshot {
            max_cases,
            max_events,
        }
        .execute(self.view)
        .map_err(|_| NativeModerationQueryFailureV1::Unavailable)
    }

    fn event_page(
        &self,
        expected_finalized_cursor:
            iroha_data_model::sorafs::moderation_ledger::ModerationFinalizedCursorV1,
        after: Option<ModerationFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<ModerationFinalizedEventPageV1, NativeModerationQueryFailureV1> {
        FindSorafsModerationEvents {
            expected_finalized_cursor,
            after,
            limit,
        }
        .execute(self.view)
        .map_err(|_| NativeModerationQueryFailureV1::Unavailable)
    }
}

fn read_and_validate_snapshot(
    queries: &impl ModerationQueryViewV1,
    max_cases: u32,
    max_events: u32,
    page_size: u32,
) -> Result<ModerationFinalizedLedgerSnapshotV1, ModerationSnapshotReadErrorV1> {
    if !(1..=MODERATION_QUERY_MAX_CASES_V1).contains(&max_cases)
        || !(1..=MODERATION_QUERY_MAX_EVENTS_V1).contains(&max_events)
        || !(1..=MODERATION_QUERY_MAX_EVENTS_V1).contains(&page_size)
    {
        return Err(ModerationSnapshotReadErrorV1::ResourceExhausted);
    }
    let snapshot = queries
        .snapshot(max_cases, max_events)
        .map_err(|_| ModerationSnapshotReadErrorV1::Unavailable)?;
    let max_cases_usize =
        usize::try_from(max_cases).map_err(|_| ModerationSnapshotReadErrorV1::ResourceExhausted)?;
    let max_events_usize = usize::try_from(max_events)
        .map_err(|_| ModerationSnapshotReadErrorV1::ResourceExhausted)?;
    if snapshot.version != MODERATION_FINALIZED_SNAPSHOT_VERSION_V1
        || snapshot.finalized_height == 0
        || snapshot.finalized_block_hash == [0; 32]
        || snapshot.finalized_at_unix_ms == 0
        || snapshot.appeals.len() > max_cases_usize
        || snapshot.cases.len() > max_cases_usize
        || snapshot.events.len() > max_events_usize
    {
        return Err(ModerationSnapshotReadErrorV1::InvalidSnapshot);
    }
    validate_snapshot_event_pages(queries, &snapshot, page_size, max_events)?;
    Ok(snapshot)
}

fn validate_snapshot_event_pages(
    queries: &impl ModerationQueryViewV1,
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
    page_size: u32,
    max_events: u32,
) -> Result<(), ModerationSnapshotReadErrorV1> {
    let anchor = snapshot.anchor();
    let Some(first_event) = snapshot.events.first() else {
        let page = queries
            .event_page(anchor, None, page_size)
            .map_err(|_| ModerationSnapshotReadErrorV1::Unavailable)?;
        return if page.finalized_cursor == anchor
            && page.events.is_empty()
            && !page.has_more
            && page.next_after.is_none()
        {
            Ok(())
        } else {
            Err(ModerationSnapshotReadErrorV1::InvalidSnapshot)
        };
    };
    let mut after = if first_event.sequence == 1 {
        None
    } else {
        Some(first_event.cursor())
    };
    let mut expected_index = usize::from(first_event.sequence != 1);
    let maximum_pages = usize::try_from(max_events.div_ceil(page_size))
        .map_err(|_| ModerationSnapshotReadErrorV1::ResourceExhausted)?
        .saturating_add(1);
    let page_size_usize =
        usize::try_from(page_size).map_err(|_| ModerationSnapshotReadErrorV1::ResourceExhausted)?;

    for _ in 0..maximum_pages {
        let page = queries
            .event_page(anchor, after, page_size)
            .map_err(|_| ModerationSnapshotReadErrorV1::Unavailable)?;
        if page.finalized_cursor != anchor
            || page.events.len() > page_size_usize
            || page.next_after.is_some() != page.has_more
        {
            return Err(ModerationSnapshotReadErrorV1::InvalidSnapshot);
        }
        for event in &page.events {
            if snapshot.events.get(expected_index) != Some(event) {
                return Err(ModerationSnapshotReadErrorV1::InvalidSnapshot);
            }
            expected_index = expected_index
                .checked_add(1)
                .ok_or(ModerationSnapshotReadErrorV1::ResourceExhausted)?;
        }
        if page.has_more {
            let Some(last) = page.events.last() else {
                return Err(ModerationSnapshotReadErrorV1::InvalidSnapshot);
            };
            if page.next_after != Some(last.cursor()) || after == page.next_after {
                return Err(ModerationSnapshotReadErrorV1::InvalidSnapshot);
            }
            after = page.next_after;
            continue;
        }
        return if expected_index == snapshot.events.len() {
            Ok(())
        } else {
            Err(ModerationSnapshotReadErrorV1::InvalidSnapshot)
        };
    }
    Err(ModerationSnapshotReadErrorV1::ResourceExhausted)
}

/// Canonical handoff request supplied to a durable downstream boundary.
#[derive(Debug, Clone)]
pub struct ModerationDurableHandoffRequestV1 {
    /// Exact typed payload-free finalized handoff.
    pub handoff: ModerationTerminalHandoffV1,
    /// Canonical Norito encoding of `handoff`.
    pub canonical_handoff: Vec<u8>,
}

/// Successful result from a durable handoff boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationDurableHandoffOutcomeV1 {
    /// This call durably accepted the handoff.
    Delivered,
    /// The same handoff identity and bytes were already durably accepted.
    AlreadyDelivered,
}

/// Fixed durable-boundary failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationDurableHandoffFailureV1 {
    /// No delivery occurred and retry is safe.
    NotDelivered,
    /// Delivery may have occurred; retrying the same identity is required.
    Ambiguous,
    /// The exact handoff was permanently rejected.
    Permanent,
}

/// Durable, idempotent terminal settlement or publication boundary.
///
/// Implementations must atomically retain `handoff_id`, the digest of
/// `canonical_handoff`, and their downstream outbox effect before returning
/// [`ModerationDurableHandoffOutcomeV1::Delivered`]. A replay with different
/// bytes must return [`ModerationDurableHandoffFailureV1::Permanent`].
pub trait ModerationDurableHandoffBoundaryV1: Send + Sync {
    /// Deliver or replay one exact terminal handoff.
    ///
    /// # Errors
    ///
    /// Returns a fixed delivery class. An ambiguous result is retried with the
    /// same handoff identity and canonical bytes.
    fn deliver_once(
        &self,
        request: &ModerationDurableHandoffRequestV1,
    ) -> Result<ModerationDurableHandoffOutcomeV1, ModerationDurableHandoffFailureV1>;
}

/// Destination-bound terminal handoff adapter.
pub struct ModerationTerminalHandoffSinkAdapterV1 {
    kind: ModerationTerminalHandoffKindV1,
    boundary: Arc<dyn ModerationDurableHandoffBoundaryV1>,
}

impl core::fmt::Debug for ModerationTerminalHandoffSinkAdapterV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ModerationTerminalHandoffSinkAdapterV1")
            .field("kind", &self.kind)
            .field("boundary", &"<durable-idempotent-boundary>")
            .finish()
    }
}

impl ModerationTerminalHandoffSinkAdapterV1 {
    /// Construct the appeal-finance settlement sink.
    #[must_use]
    pub fn settlement(boundary: Arc<dyn ModerationDurableHandoffBoundaryV1>) -> Self {
        Self {
            kind: ModerationTerminalHandoffKindV1::Settlement,
            boundary,
        }
    }

    /// Construct the governance/transparency publication sink.
    #[must_use]
    pub fn publication(boundary: Arc<dyn ModerationDurableHandoffBoundaryV1>) -> Self {
        Self {
            kind: ModerationTerminalHandoffKindV1::Publication,
            boundary,
        }
    }
}

impl ModerationTerminalHandoffSinkV1 for ModerationTerminalHandoffSinkAdapterV1 {
    fn deliver(
        &self,
        handoff: &ModerationTerminalHandoffV1,
    ) -> Result<(), ModerationHandoffFailureV1> {
        if handoff.kind != self.kind
            || handoff.handoff_id == [0; 32]
            || handoff.outcome_digest == [0; 32]
            || handoff.finalized_cursor.height == 0
            || handoff.finalized_cursor.block_hash == [0; 32]
            || !is_canonical_moderation_identifier_v1(&handoff.case_id)
            || !is_canonical_moderation_identifier_v1(&handoff.round_id)
        {
            return Err(ModerationHandoffFailureV1::Permanent);
        }
        let canonical_handoff =
            norito::to_bytes(handoff).map_err(|_| ModerationHandoffFailureV1::Permanent)?;
        if canonical_handoff.is_empty() || canonical_handoff.len() > MODERATION_HANDOFF_MAX_BYTES_V1
        {
            return Err(ModerationHandoffFailureV1::Permanent);
        }
        let request = ModerationDurableHandoffRequestV1 {
            handoff: handoff.clone(),
            canonical_handoff,
        };
        self.boundary
            .deliver_once(&request)
            .map(|_| ())
            .map_err(|error| match error {
                ModerationDurableHandoffFailureV1::NotDelivered => {
                    ModerationHandoffFailureV1::NotDelivered
                }
                ModerationDurableHandoffFailureV1::Ambiguous => {
                    ModerationHandoffFailureV1::Ambiguous
                }
                ModerationDurableHandoffFailureV1::Permanent => {
                    ModerationHandoffFailureV1::Permanent
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, VecDeque},
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        events::data::sorafs::{SorafsModerationLedgerEvent, SorafsModerationLedgerEventKind},
        isi::sorafs::FinalizeSorafsModerationCase,
        sorafs::moderation_ledger::{ModerationFinalizedCursorV1, ModerationFinalizedEventV1},
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_node::moderation_orchestrator::ModerationNativeActionV1;

    use super::*;

    const TEST_CHAIN: &str = "moderation-runtime-test";

    fn key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("test Ed25519 key")
    }

    fn account(key_pair: &KeyPair) -> AccountId {
        AccountId::new(key_pair.public_key().clone())
    }

    fn action() -> ModerationNativeActionV1 {
        ModerationNativeActionV1::FinalizeCase(FinalizeSorafsModerationCase::new(
            "case-1".to_owned(),
            "round-1".to_owned(),
        ))
    }

    fn transaction_request(authority: AccountId) -> ModerationTransactionRequestV1 {
        ModerationTransactionRequestV1::new(
            ChainId::from(TEST_CHAIN),
            1,
            authority,
            action(),
            [0x42; 32],
            7,
            [0x43; 32],
        )
        .expect("canonical transaction request")
    }

    fn sign_and_submit(
        adapter: &ModerationTransactionSubmitterAdapterV1,
        request: &ModerationTransactionRequestV1,
    ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
        let signed = adapter.sign(request)?;
        adapter.submit_signed(request, &signed)
    }

    #[derive(Debug)]
    enum FixedSignerBehavior {
        Exact,
        SubstituteChain,
        Forged(KeyPair),
    }

    #[derive(Debug)]
    struct FixedSigner {
        key_pair: KeyPair,
        behavior: FixedSignerBehavior,
        calls: AtomicUsize,
    }

    impl FixedSigner {
        fn exact(key_pair: KeyPair) -> Self {
            Self {
                key_pair,
                behavior: FixedSignerBehavior::Exact,
                calls: AtomicUsize::new(0),
            }
        }

        fn substitute_chain(key_pair: KeyPair) -> Self {
            Self {
                key_pair,
                behavior: FixedSignerBehavior::SubstituteChain,
                calls: AtomicUsize::new(0),
            }
        }

        fn forged(key_pair: KeyPair, forgery_key: KeyPair) -> Self {
            Self {
                key_pair,
                behavior: FixedSignerBehavior::Forged(forgery_key),
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl ModerationSignedTransactionSignerV1 for FixedSigner {
        fn sign(
            &self,
            mut payload: TransactionPayload,
        ) -> Result<SignedTransaction, ModerationSigningFailureV1> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            match &self.behavior {
                FixedSignerBehavior::Exact => TransactionBuilder::from_payload(payload)
                    .and_then(|builder| builder.try_sign(self.key_pair.private_key()))
                    .map_err(|_| ModerationSigningFailureV1::Refused),
                FixedSignerBehavior::SubstituteChain => {
                    payload.chain = ChainId::from("substituted-chain");
                    TransactionBuilder::from_payload(payload)
                        .and_then(|builder| builder.try_sign(self.key_pair.private_key()))
                        .map_err(|_| ModerationSigningFailureV1::Refused)
                }
                FixedSignerBehavior::Forged(forgery_key) => {
                    Ok(TransactionBuilder::from_payload(payload)
                        .map_err(|_| ModerationSigningFailureV1::Refused)?
                        .build_with_signature(Signature::new(
                            forgery_key.private_key(),
                            b"not-the-transaction-payload",
                        )))
                }
            }
        }
    }

    #[derive(Debug)]
    struct TestFeeQuoter;

    impl ModerationFeeQuoterV1 for TestFeeQuoter {
        fn quote(
            &self,
            payload: &TransactionPayload,
        ) -> Result<FeePaymentIntent, ModerationFeeQuoteFailureV1> {
            Ok(payload.fee_payment.clone())
        }
    }

    fn adapter(
        signer: Arc<dyn ModerationSignedTransactionSignerV1>,
        ingress: Arc<dyn ModerationStrictTransactionIngressV1>,
    ) -> ModerationTransactionSubmitterAdapterV1 {
        ModerationTransactionSubmitterAdapterV1::new(
            ChainId::from(TEST_CHAIN),
            signer,
            Arc::new(TestFeeQuoter),
            ingress,
        )
    }

    #[derive(Debug, Default)]
    struct TestIngressState {
        calls: usize,
        admissions: BTreeMap<[u8; 32], [u8; 32]>,
    }

    #[derive(Debug, Default)]
    struct TestIngress {
        state: Mutex<TestIngressState>,
    }

    impl TestIngress {
        fn calls(&self) -> usize {
            self.state.lock().expect("ingress lock").calls
        }

        fn unique_admissions(&self) -> usize {
            self.state.lock().expect("ingress lock").admissions.len()
        }
    }

    impl ModerationStrictTransactionIngressV1 for TestIngress {
        fn submit_exact(
            &self,
            request: &ModerationTransactionRequestV1,
            transaction: SignedTransaction,
        ) -> Result<ModerationStrictIngressReceiptV1, ModerationStrictIngressFailureV1> {
            let transaction_id = *transaction.hash().as_ref();
            let mut state = self.state.lock().expect("ingress lock");
            state.calls = state.calls.saturating_add(1);
            let replay = match state.admissions.get(&request.operation_id) {
                Some(existing) if *existing == transaction_id => true,
                Some(_) => return Err(ModerationStrictIngressFailureV1::PermanentRejection),
                None => {
                    state
                        .admissions
                        .insert(request.operation_id, transaction_id);
                    false
                }
            };
            Ok(ModerationStrictIngressReceiptV1 {
                transaction_id,
                observed_finalized_height: 7,
                replay,
            })
        }

        fn lookup_exact(
            &self,
            operation_id: [u8; 32],
            _transaction_id: Option<[u8; 32]>,
        ) -> ModerationSubmissionLookupV1 {
            self.state
                .lock()
                .expect("ingress lock")
                .admissions
                .get(&operation_id)
                .copied()
                .map_or(
                    ModerationSubmissionLookupV1::NotFound {
                        observed_finalized_height: 7,
                    },
                    |transaction_id| ModerationSubmissionLookupV1::Pending { transaction_id },
                )
        }
    }

    #[test]
    fn signer_authority_mismatch_is_rejected_before_ingress() {
        let expected_key = key(1);
        let substituted_key = key(2);
        let request = transaction_request(account(&expected_key));
        let signer = Arc::new(FixedSigner::exact(substituted_key));
        let ingress = Arc::new(TestIngress::default());
        let adapter = adapter(signer, ingress.clone());

        assert_eq!(
            sign_and_submit(&adapter, &request),
            Err(ModerationSubmissionFailureV1::PermanentRejection)
        );
        assert_eq!(ingress.calls(), 0);
        assert_eq!(ingress.unique_admissions(), 0);
    }

    #[test]
    fn canonical_request_digest_tampering_is_rejected_before_signing() {
        let signer_key = key(12);
        let mut request = transaction_request(account(&signer_key));
        let signer = Arc::new(FixedSigner::exact(signer_key));
        let ingress = Arc::new(TestIngress::default());
        let adapter = adapter(signer.clone(), ingress.clone());
        request.action_digest[0] ^= 0x80;

        assert_eq!(
            sign_and_submit(&adapter, &request),
            Err(ModerationSubmissionFailureV1::PermanentRejection)
        );
        assert_eq!(signer.calls(), 0);
        assert_eq!(ingress.calls(), 0);
    }

    #[test]
    fn forged_transaction_signature_is_rejected_before_ingress() {
        let authority_key = key(13);
        let forgery_key = key(14);
        let request = transaction_request(account(&authority_key));
        let signer = Arc::new(FixedSigner::forged(authority_key, forgery_key));
        let ingress = Arc::new(TestIngress::default());
        let adapter = adapter(signer, ingress.clone());

        assert_eq!(
            sign_and_submit(&adapter, &request),
            Err(ModerationSubmissionFailureV1::PermanentRejection)
        );
        assert_eq!(ingress.calls(), 0);
    }

    #[test]
    fn signer_payload_substitution_is_rejected_before_ingress() {
        let signer_key = key(15);
        let request = transaction_request(account(&signer_key));
        let signer = Arc::new(FixedSigner::substitute_chain(signer_key));
        let ingress = Arc::new(TestIngress::default());
        let adapter = adapter(signer, ingress.clone());

        assert_eq!(
            sign_and_submit(&adapter, &request),
            Err(ModerationSubmissionFailureV1::PermanentRejection)
        );
        assert_eq!(ingress.calls(), 0);
    }

    #[test]
    fn strict_ingress_replay_admits_one_exact_retained_transaction() {
        let signer_key = key(3);
        let request = transaction_request(account(&signer_key));
        let signer = Arc::new(FixedSigner::exact(signer_key));
        let ingress = Arc::new(TestIngress::default());
        let adapter = adapter(signer, ingress.clone());
        let signed = adapter.sign(&request).expect("sign exact payload");
        let expected_transaction_id = signed.transaction_id;

        let first = adapter
            .submit_signed(&request, &signed)
            .expect("first submission");
        let replay = adapter
            .submit_signed(&request, &signed)
            .expect("idempotent replay");
        assert_eq!(first, replay);
        assert_eq!(first.transaction_id, expected_transaction_id);
        assert_eq!(ingress.calls(), 2);
        assert_eq!(ingress.unique_admissions(), 1);
    }

    #[test]
    fn lookup_rejects_a_foreign_transaction_identity() {
        assert_eq!(
            sanitize_submission_lookup(
                ModerationSubmissionLookupV1::Applied {
                    transaction_id: [0x51; 32],
                },
                Some([0x52; 32]),
            ),
            ModerationSubmissionLookupV1::Unknown
        );
    }

    #[derive(Debug)]
    struct TestQueries {
        snapshot: Result<ModerationFinalizedLedgerSnapshotV1, NativeModerationQueryFailureV1>,
        pages:
            Mutex<VecDeque<Result<ModerationFinalizedEventPageV1, NativeModerationQueryFailureV1>>>,
        requests: Mutex<Vec<(ModerationFinalizedCursorV1, Option<u64>, u32)>>,
    }

    impl TestQueries {
        fn new(
            snapshot: ModerationFinalizedLedgerSnapshotV1,
            pages: impl IntoIterator<
                Item = Result<ModerationFinalizedEventPageV1, NativeModerationQueryFailureV1>,
            >,
        ) -> Self {
            Self {
                snapshot: Ok(snapshot),
                pages: Mutex::new(pages.into_iter().collect()),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn request_count(&self) -> usize {
            self.requests.lock().expect("query requests lock").len()
        }

        fn requested_limits(&self) -> Vec<u32> {
            self.requests
                .lock()
                .expect("query requests lock")
                .iter()
                .map(|(_, _, limit)| *limit)
                .collect()
        }
    }

    impl ModerationQueryViewV1 for TestQueries {
        fn snapshot(
            &self,
            _max_cases: u32,
            _max_events: u32,
        ) -> Result<ModerationFinalizedLedgerSnapshotV1, NativeModerationQueryFailureV1> {
            self.snapshot.clone()
        }

        fn event_page(
            &self,
            expected_finalized_cursor: ModerationFinalizedCursorV1,
            after: Option<ModerationFinalizedEventCursorV1>,
            limit: u32,
        ) -> Result<ModerationFinalizedEventPageV1, NativeModerationQueryFailureV1> {
            self.requests.lock().expect("query requests lock").push((
                expected_finalized_cursor,
                after.map(|cursor| cursor.sequence),
                limit,
            ));
            self.pages
                .lock()
                .expect("query pages lock")
                .pop_front()
                .unwrap_or(Err(NativeModerationQueryFailureV1::Unavailable))
        }
    }

    fn finalized_event(
        sequence: u64,
        finalized_height: u64,
        finalized_hash: [u8; 32],
        authority: AccountId,
    ) -> ModerationFinalizedEventV1 {
        ModerationFinalizedEventV1 {
            sequence,
            block_height: finalized_height,
            block_hash: finalized_hash,
            event_index: u32::try_from(sequence.saturating_sub(1)).expect("event index"),
            event: SorafsModerationLedgerEvent::new(
                SorafsModerationLedgerEventKind::PolicyActivated,
                None,
                None,
                authority,
                sequence,
            ),
        }
    }

    fn snapshot_with_events(
        finalized_height: u64,
        finalized_hash: [u8; 32],
        events: Vec<ModerationFinalizedEventV1>,
    ) -> ModerationFinalizedLedgerSnapshotV1 {
        ModerationFinalizedLedgerSnapshotV1 {
            version: MODERATION_FINALIZED_SNAPSHOT_VERSION_V1,
            finalized_height,
            finalized_block_hash: finalized_hash,
            finalized_at_unix_ms: finalized_height.max(1),
            policy: None,
            status: None,
            appeals: Vec::new(),
            cases: Vec::new(),
            events,
        }
    }

    fn event_page(
        cursor: ModerationFinalizedCursorV1,
        events: Vec<ModerationFinalizedEventV1>,
        has_more: bool,
    ) -> ModerationFinalizedEventPageV1 {
        let next_after = has_more.then(|| {
            events
                .last()
                .expect("continuing page must contain an event")
                .cursor()
        });
        ModerationFinalizedEventPageV1 {
            finalized_cursor: cursor,
            events,
            has_more,
            next_after,
        }
    }

    #[test]
    fn finalized_reader_binds_timestamp_to_the_exact_tip_block() {
        let mut snapshot = snapshot_with_events(8, [8; 32], Vec::new());
        snapshot.finalized_at_unix_ms = 8_000;

        assert_eq!(
            validate_snapshot_finalized_block_fields(&snapshot, 8, [8; 32], 8_000),
            Ok(())
        );
        for (height, hash, creation_time_ms) in [
            (7, [8; 32], 8_000),
            (8, [9; 32], 8_000),
            (8, [8; 32], 0),
            (8, [8; 32], 7_999),
            (8, [8; 32], 8_001),
        ] {
            assert_eq!(
                validate_snapshot_finalized_block_fields(&snapshot, height, hash, creation_time_ms,),
                Err(ModerationSnapshotReadErrorV1::InvalidSnapshot)
            );
        }
    }

    #[test]
    fn finalized_reader_rejects_stale_and_forked_page_cursors() {
        let authority = account(&key(4));
        let snapshot_cursor = ModerationFinalizedCursorV1 {
            height: 8,
            block_hash: [8; 32],
        };
        let events = vec![finalized_event(
            1,
            snapshot_cursor.height,
            snapshot_cursor.block_hash,
            authority,
        )];
        let snapshot = snapshot_with_events(
            snapshot_cursor.height,
            snapshot_cursor.block_hash,
            events.clone(),
        );
        for conflicting_cursor in [
            ModerationFinalizedCursorV1 {
                height: 7,
                block_hash: [7; 32],
            },
            ModerationFinalizedCursorV1 {
                height: snapshot_cursor.height,
                block_hash: [9; 32],
            },
        ] {
            let queries = TestQueries::new(
                snapshot.clone(),
                [Ok(event_page(conflicting_cursor, events.clone(), false))],
            );
            assert_eq!(
                read_and_validate_snapshot(&queries, 1, 1, 1),
                Err(ModerationSnapshotReadErrorV1::InvalidSnapshot)
            );
        }
    }

    #[test]
    fn finalized_reader_fails_closed_after_partial_query_failure() {
        let authority = account(&key(5));
        let cursor = ModerationFinalizedCursorV1 {
            height: 9,
            block_hash: [9; 32],
        };
        let snapshot = snapshot_with_events(
            cursor.height,
            cursor.block_hash,
            vec![finalized_event(
                1,
                cursor.height,
                cursor.block_hash,
                authority,
            )],
        );
        let queries =
            TestQueries::new(snapshot, [Err(NativeModerationQueryFailureV1::Unavailable)]);

        assert_eq!(
            read_and_validate_snapshot(&queries, 1, 1, 1),
            Err(ModerationSnapshotReadErrorV1::Unavailable)
        );
        assert_eq!(queries.request_count(), 1);
    }

    #[test]
    fn finalized_reader_rejects_events_omitted_from_an_empty_snapshot() {
        let authority = account(&key(15));
        let cursor = ModerationFinalizedCursorV1 {
            height: 12,
            block_hash: [12; 32],
        };
        let omitted = finalized_event(1, cursor.height, cursor.block_hash, authority);
        let queries = TestQueries::new(
            snapshot_with_events(cursor.height, cursor.block_hash, Vec::new()),
            [Ok(event_page(cursor, vec![omitted], false))],
        );

        assert_eq!(
            read_and_validate_snapshot(&queries, 1, 1, 1),
            Err(ModerationSnapshotReadErrorV1::InvalidSnapshot)
        );
        assert_eq!(queries.request_count(), 1);
    }

    #[test]
    fn finalized_reader_pages_within_the_requested_bound() {
        let authority = account(&key(6));
        let cursor = ModerationFinalizedCursorV1 {
            height: 10,
            block_hash: [10; 32],
        };
        let events = (1..=5)
            .map(|sequence| {
                finalized_event(
                    sequence,
                    cursor.height,
                    cursor.block_hash,
                    authority.clone(),
                )
            })
            .collect::<Vec<_>>();
        let queries = TestQueries::new(
            snapshot_with_events(cursor.height, cursor.block_hash, events.clone()),
            [
                Ok(event_page(cursor, events[0..2].to_vec(), true)),
                Ok(event_page(cursor, events[2..4].to_vec(), true)),
                Ok(event_page(cursor, events[4..5].to_vec(), false)),
            ],
        );

        let snapshot =
            read_and_validate_snapshot(&queries, 1, 5, 2).expect("bounded snapshot pages");
        assert_eq!(snapshot.events, events);
        assert_eq!(queries.request_count(), 3);
        assert_eq!(queries.requested_limits(), vec![2, 2, 2]);
    }

    #[derive(Debug, Default)]
    struct TestHandoffBoundaryState {
        calls: usize,
        fail_next: Option<ModerationDurableHandoffFailureV1>,
        delivered: BTreeMap<[u8; 32], Vec<u8>>,
    }

    #[derive(Debug, Default)]
    struct TestHandoffBoundary {
        state: Mutex<TestHandoffBoundaryState>,
    }

    impl TestHandoffBoundary {
        fn fail_next(&self, error: ModerationDurableHandoffFailureV1) {
            self.state.lock().expect("handoff lock").fail_next = Some(error);
        }

        fn calls(&self) -> usize {
            self.state.lock().expect("handoff lock").calls
        }

        fn deliveries(&self) -> usize {
            self.state.lock().expect("handoff lock").delivered.len()
        }
    }

    impl ModerationDurableHandoffBoundaryV1 for TestHandoffBoundary {
        fn deliver_once(
            &self,
            request: &ModerationDurableHandoffRequestV1,
        ) -> Result<ModerationDurableHandoffOutcomeV1, ModerationDurableHandoffFailureV1> {
            let mut state = self.state.lock().expect("handoff lock");
            state.calls = state.calls.saturating_add(1);
            if let Some(error) = state.fail_next.take() {
                return Err(error);
            }
            match state.delivered.get(&request.handoff.handoff_id) {
                Some(existing) if existing == &request.canonical_handoff => {
                    Ok(ModerationDurableHandoffOutcomeV1::AlreadyDelivered)
                }
                Some(_) => Err(ModerationDurableHandoffFailureV1::Permanent),
                None => {
                    state.delivered.insert(
                        request.handoff.handoff_id,
                        request.canonical_handoff.clone(),
                    );
                    Ok(ModerationDurableHandoffOutcomeV1::Delivered)
                }
            }
        }
    }

    fn terminal_handoff(kind: ModerationTerminalHandoffKindV1) -> ModerationTerminalHandoffV1 {
        ModerationTerminalHandoffV1 {
            handoff_id: [0x61; 32],
            kind,
            case_id: "case-1".to_owned(),
            round_id: "round-1".to_owned(),
            outcome_digest: [0x62; 32],
            finalized_cursor: ModerationFinalizedCursorV1 {
                height: 11,
                block_hash: [0x63; 32],
            },
        }
    }

    #[test]
    fn terminal_handoff_failure_retries_the_same_idempotency_identity() {
        let boundary = Arc::new(TestHandoffBoundary::default());
        boundary.fail_next(ModerationDurableHandoffFailureV1::NotDelivered);
        let sink = ModerationTerminalHandoffSinkAdapterV1::settlement(boundary.clone());
        let handoff = terminal_handoff(ModerationTerminalHandoffKindV1::Settlement);

        assert_eq!(
            sink.deliver(&handoff),
            Err(ModerationHandoffFailureV1::NotDelivered)
        );
        sink.deliver(&handoff).expect("retry delivery");
        sink.deliver(&handoff).expect("idempotent replay");
        assert_eq!(boundary.calls(), 3);
        assert_eq!(boundary.deliveries(), 1);
    }

    #[test]
    fn terminal_handoff_cannot_cross_destination_boundaries() {
        let boundary = Arc::new(TestHandoffBoundary::default());
        let sink = ModerationTerminalHandoffSinkAdapterV1::publication(boundary.clone());
        let settlement = terminal_handoff(ModerationTerminalHandoffKindV1::Settlement);

        assert_eq!(
            sink.deliver(&settlement),
            Err(ModerationHandoffFailureV1::Permanent)
        );
        assert_eq!(boundary.calls(), 0);
    }
}
