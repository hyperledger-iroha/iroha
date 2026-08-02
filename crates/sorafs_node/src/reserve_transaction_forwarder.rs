//! Durable forwarding for native SoraFS reserve and rent transactions.
//!
//! The forwarder persists only validated native operations and finalized-ledger
//! bindings. It never mutates a process-local reserve model. Exact governed or
//! provider authorities are derived from the finalized projection, signing is
//! isolated from submission, and exact signed bytes are durable before they can
//! be exposed to a submitter.

use std::{
    collections::BTreeSet,
    path::Path,
    sync::{Arc, Mutex},
};

use iroha_data_model::{
    ChainId,
    account::AccountId,
    isi::{
        InstructionBox,
        sorafs::{
            AdvanceSorafsReserveLifecycle, ChargeSorafsReserveRent, DecideSorafsReserveAppeal,
            DecideSorafsReserveMovement, DrawSorafsReserveCredit, RegisterSorafsReserveAccount,
            RepaySorafsReserveCredit, RequestSorafsReserveMovement, SubmitSorafsReserveAppeal,
        },
    },
    sorafs::{
        capacity::ProviderId,
        reserve::{
            RESERVE_MAX_OPEN_APPEALS_V1, RESERVE_MAX_PENDING_MOVEMENTS_V1,
            RESERVE_MAX_REASON_BYTES_V1, RESERVE_RENT_MAX_BILLING_PERIODS_V1,
            ReserveAppealRecordV1, ReserveAppealStatusV1, ReserveAuthorityPolicyRecordV1,
            ReserveFinalizedCursorV1, ReserveMovementRecordV1, ReserveMovementStatusV1,
            ReserveProviderAccountV1,
        },
    },
    transaction::{Executable, SignedTransaction},
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::durable_transaction_forwarder::{
    self as durable, AtomicCheckpointStore, CheckpointStoreError, DeliveryRecord,
    DeliveryTransitionError, FinalizedCursorV1, RetryBoundOutcome, StoredDeliveryStateV1,
};

/// Durable reserve-transaction checkpoint schema version.
pub const RESERVE_TRANSACTION_FORWARDER_CHECKPOINT_VERSION_V1: u8 = 1;
/// Canonical reserve-transaction checkpoint file.
pub const RESERVE_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1: &str =
    "reserve-transaction-forwarder-state.to";
/// Default bounded attempt count for one semantic reserve operation.
pub const RESERVE_TRANSACTION_FORWARDER_DEFAULT_MAX_ATTEMPTS_V1: u32 = 8;
/// Maximum entries returned by one worker scan.
pub const RESERVE_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1: usize = 1_000;
/// Hard ceiling for one canonical signed reserve transaction.
pub const RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1: usize = 2 * 1024 * 1024;
/// Maximum active chain identifier retained by the V1 forwarder.
pub const RESERVE_TRANSACTION_MAX_CHAIN_ID_BYTES_V1: usize = 128;

const CHECKPOINT_LOCK_FILE_NAME: &str = "reserve-transaction-forwarder-state.lock";
const REVISION_IDENTITY_DOMAIN_V1: &[u8] =
    b"sorafs.reserve.transaction-forwarder.revision-identity.v1\0";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"sorafs.reserve.transaction-forwarder.operation.v1\0";
const SEMANTIC_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.reserve.transaction-forwarder.semantic.v1\0";
const CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT: usize = 8;
const CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT: usize = 20;
const CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 1024 * 1024;
const CHECKPOINT_MAX_NESTING_DEPTH: usize = 128;
const TRANSACTION_ELEMENT_AMPLIFICATION_LIMIT: usize = 8;
const TRANSACTION_ALLOCATION_AMPLIFICATION_LIMIT: usize = 20;
const TRANSACTION_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 512 * 1024;
const TRANSACTION_MAX_NESTING_DEPTH: usize = 128;

/// Bounded persistence and retry policy for native reserve operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReserveTransactionForwarderPolicyV1 {
    /// Maximum pending semantic operations.
    pub max_pending: usize,
    /// Maximum finalized idempotency tombstones.
    pub max_completed: usize,
    /// Maximum terminal dead letters.
    pub max_dead_letters: usize,
    /// Maximum signing/submission attempts for one semantic operation.
    pub max_attempts: u32,
    /// Maximum accepted canonical transaction bytes.
    pub max_transaction_bytes: usize,
    /// Maximum canonical checkpoint bytes.
    pub checkpoint_max_bytes: u64,
}

impl ReserveTransactionForwarderPolicyV1 {
    /// Validate all first-release resource bounds.
    pub fn validate(self) -> Result<(), ReserveTransactionForwarderError> {
        if self.max_pending == 0
            || self.max_completed == 0
            || self.max_dead_letters == 0
            || self.max_attempts == 0
            || self.max_transaction_bytes == 0
            || self.max_transaction_bytes > RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1
            || self.checkpoint_max_bytes == 0
        {
            return Err(ReserveTransactionForwarderError::InvalidPolicy);
        }
        Ok(())
    }
}

/// Exact finalized reserve projection required to validate an operation.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum ReserveTransactionProjectionV1 {
    /// Provider registration, bound to the separate finalized provider registry.
    Registration {
        /// Exact registry owner observed for the provider identifier.
        provider_owner: AccountId,
    },
    /// Mutation of one existing provider reserve account.
    Provider {
        /// Exact finalized provider account.
        account: ReserveProviderAccountV1,
    },
    /// Decision of one pending movement.
    MovementDecision {
        /// Exact finalized provider account.
        account: ReserveProviderAccountV1,
        /// Exact finalized pending movement.
        movement: ReserveMovementRecordV1,
    },
    /// Decision of one pending appeal.
    AppealDecision {
        /// Exact finalized provider account.
        account: ReserveProviderAccountV1,
        /// Exact finalized pending appeal.
        appeal: ReserveAppealRecordV1,
    },
}

/// Finalized governance and domain snapshot used to admit one operation.
///
/// The policy and projection must be read from the same immutable state view
/// identified by `finalized_cursor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveTransactionContextV1 {
    /// Exact active chain identity for signing and signed-ingress validation.
    pub chain_id: ChainId,
    /// Exact active governance-authenticated policy record.
    pub policy_record: ReserveAuthorityPolicyRecordV1,
    /// Operation-specific finalized reserve or provider-registry projection.
    pub projection: ReserveTransactionProjectionV1,
    /// Finalized block anchor shared by every supplied record.
    pub finalized_cursor: ReserveFinalizedCursorV1,
}

impl ReserveTransactionContextV1 {
    fn validate(&self) -> Result<(), ReserveTransactionForwarderError> {
        validate_finalized_cursor(self.finalized_cursor)?;
        if self.chain_id.as_str().is_empty()
            || self.chain_id.as_str().len() > RESERVE_TRANSACTION_MAX_CHAIN_ID_BYTES_V1
        {
            return Err(ReserveTransactionForwarderError::InvalidGovernanceContext);
        }
        self.policy_record
            .policy
            .validate()
            .map_err(|_| ReserveTransactionForwarderError::InvalidGovernanceContext)?;
        let digest = self
            .policy_record
            .policy
            .digest()
            .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
        if digest == [0; 32]
            || digest != self.policy_record.policy_digest
            || self.policy_record.activated_at_unix == 0
        {
            return Err(ReserveTransactionForwarderError::InvalidGovernanceContext);
        }
        match &self.projection {
            ReserveTransactionProjectionV1::Registration { .. } => {}
            ReserveTransactionProjectionV1::Provider { account } => {
                validate_provider_account(account)?;
            }
            ReserveTransactionProjectionV1::MovementDecision { account, movement } => {
                validate_provider_account(account)?;
                validate_movement_record(movement)?;
                if account.terms.provider_id != movement.provider_id
                    || movement.status != ReserveMovementStatusV1::Pending
                {
                    return Err(ReserveTransactionForwarderError::InvalidFinalizedProjection);
                }
            }
            ReserveTransactionProjectionV1::AppealDecision { account, appeal } => {
                validate_provider_account(account)?;
                validate_appeal_record(appeal)?;
                if account.terms.provider_id != appeal.provider_id
                    || appeal.status != ReserveAppealStatusV1::Pending
                {
                    return Err(ReserveTransactionForwarderError::InvalidFinalizedProjection);
                }
            }
        }
        Ok(())
    }
}

/// Native reserve instruction kind retained by the forwarder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveTransactionKindV1 {
    /// Register one provider reserve partition.
    RegisterProvider,
    /// Submit one provider-owned movement request.
    RequestMovement,
    /// Decide one pending movement.
    DecideMovement,
    /// Charge deterministic rent.
    ChargeRent,
    /// Advance deterministic lifecycle state.
    AdvanceLifecycle,
    /// Draw governed reserve credit.
    DrawCredit,
    /// Repay provider reserve credit.
    RepayCredit,
    /// Submit one provider-owned appeal.
    SubmitAppeal,
    /// Decide one pending appeal.
    DecideAppeal,
}

/// Validated native reserve operation retained for isolated external signing.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum ReserveOperationV1 {
    /// Register one provider reserve partition.
    RegisterProvider(RegisterSorafsReserveAccount),
    /// Submit one provider-owned movement request.
    RequestMovement(RequestSorafsReserveMovement),
    /// Decide one pending movement.
    DecideMovement(DecideSorafsReserveMovement),
    /// Charge deterministic rent.
    ChargeRent(ChargeSorafsReserveRent),
    /// Advance deterministic lifecycle state.
    AdvanceLifecycle(AdvanceSorafsReserveLifecycle),
    /// Draw governed reserve credit.
    DrawCredit(DrawSorafsReserveCredit),
    /// Repay provider reserve credit.
    RepayCredit(RepaySorafsReserveCredit),
    /// Submit one provider-owned appeal.
    SubmitAppeal(SubmitSorafsReserveAppeal),
    /// Decide one pending appeal.
    DecideAppeal(DecideSorafsReserveAppeal),
}

impl ReserveOperationV1 {
    /// Return the native reserve instruction kind.
    #[must_use]
    pub const fn kind(&self) -> ReserveTransactionKindV1 {
        match self {
            Self::RegisterProvider(_) => ReserveTransactionKindV1::RegisterProvider,
            Self::RequestMovement(_) => ReserveTransactionKindV1::RequestMovement,
            Self::DecideMovement(_) => ReserveTransactionKindV1::DecideMovement,
            Self::ChargeRent(_) => ReserveTransactionKindV1::ChargeRent,
            Self::AdvanceLifecycle(_) => ReserveTransactionKindV1::AdvanceLifecycle,
            Self::DrawCredit(_) => ReserveTransactionKindV1::DrawCredit,
            Self::RepayCredit(_) => ReserveTransactionKindV1::RepayCredit,
            Self::SubmitAppeal(_) => ReserveTransactionKindV1::SubmitAppeal,
            Self::DecideAppeal(_) => ReserveTransactionKindV1::DecideAppeal,
        }
    }

    /// Return the active policy digest embedded in the native instruction.
    #[must_use]
    pub fn policy_digest(&self) -> [u8; 32] {
        match self {
            Self::RegisterProvider(instruction) => *instruction.policy_digest(),
            Self::RequestMovement(instruction) => *instruction.policy_digest(),
            Self::DecideMovement(instruction) => *instruction.policy_digest(),
            Self::ChargeRent(instruction) => *instruction.policy_digest(),
            Self::AdvanceLifecycle(instruction) => *instruction.policy_digest(),
            Self::DrawCredit(instruction) => *instruction.policy_digest(),
            Self::RepayCredit(instruction) => *instruction.policy_digest(),
            Self::SubmitAppeal(instruction) => *instruction.policy_digest(),
            Self::DecideAppeal(instruction) => *instruction.policy_digest(),
        }
    }

    /// Return the provider identifier affected by the operation.
    #[must_use]
    pub fn provider_id(&self) -> Option<ProviderId> {
        match self {
            Self::RegisterProvider(instruction) => Some(instruction.terms().provider_id),
            Self::RequestMovement(instruction) => Some(*instruction.provider_id()),
            Self::DecideMovement(_) | Self::DecideAppeal(_) => None,
            Self::ChargeRent(instruction) => Some(*instruction.provider_id()),
            Self::AdvanceLifecycle(instruction) => Some(*instruction.provider_id()),
            Self::DrawCredit(instruction) => Some(*instruction.provider_id()),
            Self::RepayCredit(instruction) => Some(*instruction.provider_id()),
            Self::SubmitAppeal(instruction) => Some(*instruction.provider_id()),
        }
    }

    /// Return the exact provider revision used for compare-and-set admission.
    #[must_use]
    pub fn expected_provider_revision(&self) -> Option<u64> {
        match self {
            Self::RegisterProvider(_) => None,
            Self::RequestMovement(instruction) => Some(*instruction.expected_provider_revision()),
            Self::DecideMovement(instruction) => Some(*instruction.expected_provider_revision()),
            Self::ChargeRent(instruction) => Some(*instruction.expected_provider_revision()),
            Self::AdvanceLifecycle(instruction) => Some(*instruction.expected_provider_revision()),
            Self::DrawCredit(instruction) => Some(*instruction.expected_provider_revision()),
            Self::RepayCredit(instruction) => Some(*instruction.expected_provider_revision()),
            Self::SubmitAppeal(instruction) => Some(*instruction.expected_provider_revision()),
            Self::DecideAppeal(instruction) => Some(*instruction.expected_provider_revision()),
        }
    }
}

impl From<ReserveOperationV1> for InstructionBox {
    fn from(operation: ReserveOperationV1) -> Self {
        match operation {
            ReserveOperationV1::RegisterProvider(instruction) => instruction.into(),
            ReserveOperationV1::RequestMovement(instruction) => instruction.into(),
            ReserveOperationV1::DecideMovement(instruction) => instruction.into(),
            ReserveOperationV1::ChargeRent(instruction) => instruction.into(),
            ReserveOperationV1::AdvanceLifecycle(instruction) => instruction.into(),
            ReserveOperationV1::DrawCredit(instruction) => instruction.into(),
            ReserveOperationV1::RepayCredit(instruction) => instruction.into(),
            ReserveOperationV1::SubmitAppeal(instruction) => instruction.into(),
            ReserveOperationV1::DecideAppeal(instruction) => instruction.into(),
        }
    }
}

/// Exact signer work item returned after a durable claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveTransactionSigningRequestV1 {
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Exact active chain identity.
    pub chain_id: ChainId,
    /// Exact governed or provider transaction authority.
    pub authority: AccountId,
    /// Exact validated native operation.
    pub operation: ReserveOperationV1,
}

/// Exact material retained for finalized-ledger reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveTransactionReconciliationV1 {
    /// Exact signer request material.
    pub request: ReserveTransactionSigningRequestV1,
    /// Full policy record against which the operation was admitted.
    pub policy_record: ReserveAuthorityPolicyRecordV1,
    /// Full operation-specific finalized projection.
    pub projection: ReserveTransactionProjectionV1,
}

/// Durable enqueue result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveTransactionEnqueueResultV1 {
    /// A new semantic operation was persisted.
    Inserted {
        /// Stable semantic operation identity.
        operation_id: [u8; 32],
    },
    /// The same semantic operation was already pending or finalized.
    Existing {
        /// Stable semantic operation identity.
        operation_id: [u8; 32],
    },
}

impl ReserveTransactionEnqueueResultV1 {
    /// Return the stable semantic operation identity.
    #[must_use]
    pub const fn operation_id(self) -> [u8; 32] {
        match self {
            Self::Inserted { operation_id } | Self::Existing { operation_id } => operation_id,
        }
    }
}

/// Runtime-visible crash state for one reserve transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveTransactionDeliveryStateV1 {
    /// Validated semantic material is ready for external signing.
    Ready,
    /// An external signer claim is in progress.
    Signing,
    /// Exact signed bytes are durable and not yet exposed.
    Signed,
    /// Submission may have happened and requires reconciliation.
    Ambiguous,
    /// Exact bytes are known pending or applied.
    Submitted,
}

impl From<StoredDeliveryStateV1> for ReserveTransactionDeliveryStateV1 {
    fn from(value: StoredDeliveryStateV1) -> Self {
        match value {
            StoredDeliveryStateV1::Ready => Self::Ready,
            StoredDeliveryStateV1::Signing => Self::Signing,
            StoredDeliveryStateV1::Signed => Self::Signed,
            StoredDeliveryStateV1::Ambiguous => Self::Ambiguous,
            StoredDeliveryStateV1::Submitted => Self::Submitted,
        }
    }
}

/// Payload-bounded pending reserve-transaction snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveTransactionPendingV1 {
    /// Monotonic queue sequence.
    pub sequence: u64,
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Native operation kind.
    pub kind: ReserveTransactionKindV1,
    /// Exact active chain identity.
    pub chain_id: ChainId,
    /// Exact transaction authority.
    pub authority: AccountId,
    /// Exact active policy digest.
    pub policy_digest: [u8; 32],
    /// Exact active policy revision.
    pub policy_revision: u64,
    /// Affected provider, when known directly from the instruction.
    pub provider_id: Option<ProviderId>,
    /// Exact provider CAS revision, when applicable.
    pub expected_provider_revision: Option<u64>,
    /// Domain-separated digest of authority, operation, and finalized binding.
    pub semantic_digest: [u8; 32],
    /// Digest of exact retained signed bytes.
    pub transaction_digest: Option<[u8; 32]>,
    /// Durable delivery state.
    pub state: ReserveTransactionDeliveryStateV1,
    /// Attempts consumed.
    pub attempts: u32,
    /// Finalized baseline height.
    pub baseline_finalized_height: u64,
    /// Finalized baseline hash.
    pub baseline_finalized_block_hash: [u8; 32],
    /// Exact signed transaction bytes, present after signing.
    pub signed_transaction_bytes: Option<Vec<u8>>,
}

/// Validate one payload-bounded pending snapshot exported by the durable forwarder.
///
/// This is the canonical runtime boundary for delivery-state and exact
/// signed-byte digest invariants. Workers must call this helper instead of
/// reproducing the forwarder's private digest algorithm.
pub fn validate_reserve_pending_delivery_v1(
    delivery: &ReserveTransactionPendingV1,
) -> Result<(), ReserveTransactionForwarderError> {
    if delivery.sequence == 0
        || delivery.operation_id == [0; 32]
        || delivery.semantic_digest == [0; 32]
        || delivery.policy_digest == [0; 32]
        || delivery.policy_revision == 0
        || delivery.chain_id.as_str().is_empty()
        || delivery.chain_id.as_str().len() > RESERVE_TRANSACTION_MAX_CHAIN_ID_BYTES_V1
        || delivery.expected_provider_revision == Some(0)
        || delivery.baseline_finalized_height == 0
        || delivery.baseline_finalized_block_hash == [0; 32]
    {
        return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
    }
    let signed_material_is_complete =
        delivery
            .signed_transaction_bytes
            .as_ref()
            .is_some_and(|bytes| {
                !bytes.is_empty()
                    && bytes.len() <= RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1
                    && delivery.transaction_digest == Some(transaction_digest(bytes))
            });
    let signed_material_is_absent =
        delivery.signed_transaction_bytes.is_none() && delivery.transaction_digest.is_none();
    let valid_state = match delivery.state {
        ReserveTransactionDeliveryStateV1::Ready => signed_material_is_absent,
        ReserveTransactionDeliveryStateV1::Signing => {
            signed_material_is_absent && delivery.attempts != 0
        }
        ReserveTransactionDeliveryStateV1::Signed
        | ReserveTransactionDeliveryStateV1::Ambiguous
        | ReserveTransactionDeliveryStateV1::Submitted => {
            signed_material_is_complete && delivery.attempts != 0
        }
    };
    if !valid_state {
        return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
    }
    Ok(())
}

/// Validate that exported pending and reconciliation snapshots retain one operation.
///
/// Stable identity and semantic digests are recomputed through the forwarder's
/// private canonical domains. This prevents a worker from accepting a
/// different operation that happens to share the same kind, provider, policy,
/// and revision fields.
pub fn validate_reserve_reconciliation_material_v1(
    delivery: &ReserveTransactionPendingV1,
    retained: &ReserveTransactionReconciliationV1,
) -> Result<(), ReserveTransactionForwarderError> {
    validate_reserve_pending_delivery_v1(delivery)?;
    let prepared = PreparedReserveOperation::new_bounded(
        retained.request.chain_id.clone(),
        retained.request.authority.clone(),
        retained.policy_record.clone(),
        retained.projection.clone(),
        retained.request.operation.clone(),
        RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1,
    )
    .map_err(|_| ReserveTransactionForwarderError::InvalidCheckpoint)?;
    if retained.request.operation_id != delivery.operation_id
        || retained.request.chain_id != delivery.chain_id
        || retained.request.authority != delivery.authority
        || retained.request.operation.kind() != delivery.kind
        || retained.request.operation.policy_digest() != delivery.policy_digest
        || retained.policy_record.policy_digest != delivery.policy_digest
        || retained.policy_record.policy.revision != delivery.policy_revision
        || operation_provider_id(&retained.request.operation, &retained.projection)
            != delivery.provider_id
        || retained.request.operation.expected_provider_revision()
            != delivery.expected_provider_revision
        || prepared.identity_digest == [0; 32]
        || prepared.semantic_digest != delivery.semantic_digest
        || operation_id(&prepared) != delivery.operation_id
    {
        return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
    }
    Ok(())
}

/// Terminal reason retained without private transaction payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveTransactionDeadLetterReasonV1 {
    /// Retry budget was exhausted.
    RetryExhausted,
    /// Finalized state conflicts with the retained semantic operation.
    FinalizedConflict,
    /// The transaction pipeline terminally rejected the envelope.
    TransactionRejected,
}

/// Payload-free dead-letter snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveTransactionDeadLetterV1 {
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Native operation kind.
    pub kind: ReserveTransactionKindV1,
    /// Exact active chain identity.
    pub chain_id: ChainId,
    /// Exact policy digest.
    pub policy_digest: [u8; 32],
    /// Exact policy revision.
    pub policy_revision: u64,
    /// Affected provider, when directly available.
    pub provider_id: Option<ProviderId>,
    /// Exact provider revision, when applicable.
    pub expected_provider_revision: Option<u64>,
    /// Stable semantic digest.
    pub semantic_digest: [u8; 32],
    /// Digest of retained signed bytes, if any.
    pub transaction_digest: Option<[u8; 32]>,
    /// Terminal reason.
    pub reason: ReserveTransactionDeadLetterReasonV1,
    /// Finalized height at the terminal decision.
    pub observed_finalized_height: u64,
    /// Finalized block hash at the terminal decision.
    pub observed_finalized_block_hash: [u8; 32],
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
enum StoredReserveIdentityScopeV1 {
    ProviderRegistration,
    MovementRequest,
    MovementDecision,
    RentRevision,
    LifecycleRevision,
    CreditDrawRevision,
    CreditRepaymentRevision,
    AppealSubmission,
    AppealDecision,
}

impl StoredReserveIdentityScopeV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::ProviderRegistration => 0,
            Self::MovementRequest => 1,
            Self::MovementDecision => 2,
            Self::RentRevision => 3,
            Self::LifecycleRevision => 4,
            Self::CreditDrawRevision => 5,
            Self::CreditRepaymentRevision => 6,
            Self::AppealSubmission => 7,
            Self::AppealDecision => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredDeadLetterReasonV1 {
    RetryExhausted,
    FinalizedConflict,
    TransactionRejected,
}

impl From<StoredDeadLetterReasonV1> for ReserveTransactionDeadLetterReasonV1 {
    fn from(value: StoredDeadLetterReasonV1) -> Self {
        match value {
            StoredDeadLetterReasonV1::RetryExhausted => Self::RetryExhausted,
            StoredDeadLetterReasonV1::FinalizedConflict => Self::FinalizedConflict,
            StoredDeadLetterReasonV1::TransactionRejected => Self::TransactionRejected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredPendingReserveTransactionV1 {
    sequence: u64,
    operation_id: [u8; 32],
    identity_scope: StoredReserveIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    chain_id: ChainId,
    authority: AccountId,
    policy_record: ReserveAuthorityPolicyRecordV1,
    projection: ReserveTransactionProjectionV1,
    operation: ReserveOperationV1,
    state: StoredDeliveryStateV1,
    attempts: u32,
    baseline_finalized_height: u64,
    baseline_finalized_block_hash: [u8; 32],
    signed_transaction_bytes: Option<Vec<u8>>,
}

impl StoredPendingReserveTransactionV1 {
    fn snapshot(&self) -> ReserveTransactionPendingV1 {
        ReserveTransactionPendingV1 {
            sequence: self.sequence,
            operation_id: self.operation_id,
            kind: self.operation.kind(),
            chain_id: self.chain_id.clone(),
            authority: self.authority.clone(),
            policy_digest: self.policy_record.policy_digest,
            policy_revision: self.policy_record.policy.revision,
            provider_id: operation_provider_id(&self.operation, &self.projection),
            expected_provider_revision: self.operation.expected_provider_revision(),
            semantic_digest: self.semantic_digest,
            transaction_digest: self
                .signed_transaction_bytes
                .as_deref()
                .map(transaction_digest),
            state: self.state.into(),
            attempts: self.attempts,
            baseline_finalized_height: self.baseline_finalized_height,
            baseline_finalized_block_hash: self.baseline_finalized_block_hash,
            signed_transaction_bytes: self.signed_transaction_bytes.clone(),
        }
    }
}

impl DeliveryRecord for StoredPendingReserveTransactionV1 {
    type Transaction = Vec<u8>;

    fn delivery_state(&self) -> StoredDeliveryStateV1 {
        self.state
    }

    fn set_delivery_state(&mut self, state: StoredDeliveryStateV1) {
        self.state = state;
    }

    fn attempts(&self) -> u32 {
        self.attempts
    }

    fn set_attempts(&mut self, attempts: u32) {
        self.attempts = attempts;
    }

    fn baseline_finalized_height(&self) -> u64 {
        self.baseline_finalized_height
    }

    fn set_baseline_finalized_height(&mut self, height: u64) {
        self.baseline_finalized_height = height;
    }

    fn baseline_finalized_block_hash(&self) -> [u8; 32] {
        self.baseline_finalized_block_hash
    }

    fn set_baseline_finalized_block_hash(&mut self, block_hash: [u8; 32]) {
        self.baseline_finalized_block_hash = block_hash;
    }

    fn signed_transaction(&self) -> Option<&Self::Transaction> {
        self.signed_transaction_bytes.as_ref()
    }

    fn set_signed_transaction(&mut self, transaction: Option<Self::Transaction>) {
        self.signed_transaction_bytes = transaction;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredCompletedReserveTransactionV1 {
    operation_id: [u8; 32],
    identity_scope: StoredReserveIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredDeadReserveTransactionV1 {
    operation_id: [u8; 32],
    identity_scope: StoredReserveIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    chain_id: ChainId,
    authority: AccountId,
    policy_record: ReserveAuthorityPolicyRecordV1,
    projection: ReserveTransactionProjectionV1,
    operation: ReserveOperationV1,
    signed_transaction_bytes: Option<Vec<u8>>,
    reason: StoredDeadLetterReasonV1,
    observed_finalized_height: u64,
    observed_finalized_block_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReserveTransactionForwarderCheckpointV1 {
    version: u8,
    next_sequence: u64,
    pending: Vec<StoredPendingReserveTransactionV1>,
    completed: Vec<StoredCompletedReserveTransactionV1>,
    dead_letters: Vec<StoredDeadReserveTransactionV1>,
}

impl Default for ReserveTransactionForwarderCheckpointV1 {
    fn default() -> Self {
        Self {
            version: RESERVE_TRANSACTION_FORWARDER_CHECKPOINT_VERSION_V1,
            next_sequence: 1,
            pending: Vec::new(),
            completed: Vec::new(),
            dead_letters: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct DurableState {
    checkpoint: ReserveTransactionForwarderCheckpointV1,
    fingerprint: Option<[u8; 32]>,
    durability_failure: bool,
}

/// Durable bounded native reserve transaction forwarder.
#[derive(Debug, Clone)]
pub struct ReserveTransactionForwarder {
    policy: ReserveTransactionForwarderPolicyV1,
    state: Arc<Mutex<DurableState>>,
    store: Option<Arc<AtomicCheckpointStore>>,
}

impl ReserveTransactionForwarder {
    /// Construct a non-persistent forwarder for focused composition tests.
    pub fn in_memory(
        policy: ReserveTransactionForwarderPolicyV1,
    ) -> Result<Self, ReserveTransactionForwarderError> {
        policy.validate()?;
        Ok(Self {
            policy,
            state: Arc::new(Mutex::new(DurableState {
                checkpoint: ReserveTransactionForwarderCheckpointV1::default(),
                fingerprint: None,
                durability_failure: false,
            })),
            store: None,
        })
    }

    /// Open or create a durable forwarder below `state_dir`.
    pub fn open(
        state_dir: &Path,
        policy: ReserveTransactionForwarderPolicyV1,
    ) -> Result<Self, ReserveTransactionForwarderError> {
        policy.validate()?;
        let store = Arc::new(AtomicCheckpointStore::new(
            state_dir,
            RESERVE_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1,
            CHECKPOINT_LOCK_FILE_NAME,
            policy.checkpoint_max_bytes,
        )?);
        let (bytes, fingerprint) = store.load_bytes()?;
        let mut checkpoint = match bytes {
            Some(bytes) => decode_checkpoint(&bytes, policy)?,
            None => ReserveTransactionForwarderCheckpointV1::default(),
        };
        let mut recovered = false;
        for entry in &mut checkpoint.pending {
            recovered |= recover_interrupted_signing(entry);
        }
        let forwarder = Self {
            policy,
            state: Arc::new(Mutex::new(DurableState {
                checkpoint,
                fingerprint,
                durability_failure: false,
            })),
            store: Some(store),
        };
        if recovered {
            let mut state = forwarder.lock_state()?;
            let candidate = state.checkpoint.clone();
            forwarder.commit_candidate(&mut state, candidate)?;
        }
        Ok(forwarder)
    }

    /// Validate and durably accept one unsigned native reserve operation.
    ///
    /// The transaction authority is always derived from the finalized context.
    pub fn enqueue_unsigned_operation(
        &self,
        operation: ReserveOperationV1,
        context: &ReserveTransactionContextV1,
    ) -> Result<ReserveTransactionEnqueueResultV1, ReserveTransactionForwarderError> {
        context.validate()?;
        let prepared = PreparedReserveOperation::from_unsigned(
            operation,
            context,
            self.policy.max_transaction_bytes,
        )?;
        self.enqueue_prepared(prepared, None, context.finalized_cursor)
    }

    /// Validate and durably accept one exact canonical signed transaction.
    ///
    /// Broader permission ownership cannot substitute for the exact authority
    /// committed in the supplied finalized policy/provider projection.
    pub fn enqueue_signed_transaction(
        &self,
        signed_transaction_bytes: &[u8],
        context: &ReserveTransactionContextV1,
    ) -> Result<ReserveTransactionEnqueueResultV1, ReserveTransactionForwarderError> {
        context.validate()?;
        let prepared = PreparedReserveOperation::decode_signed_transaction(
            signed_transaction_bytes,
            context,
            self.policy.max_transaction_bytes,
        )?;
        self.enqueue_prepared(
            prepared,
            Some(signed_transaction_bytes),
            context.finalized_cursor,
        )
    }

    fn enqueue_prepared(
        &self,
        prepared: PreparedReserveOperation,
        signed_transaction_bytes: Option<&[u8]>,
        baseline_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<ReserveTransactionEnqueueResultV1, ReserveTransactionForwarderError> {
        let operation_id = operation_id(&prepared);
        let mut state = self.lock_state()?;
        if let Some(existing) = state.checkpoint.pending.iter().find(|entry| {
            entry.identity_scope == prepared.identity_scope
                && entry.identity_digest == prepared.identity_digest
        }) {
            if existing.operation_id == operation_id
                && existing.semantic_digest == prepared.semantic_digest
            {
                return Ok(ReserveTransactionEnqueueResultV1::Existing { operation_id });
            }
            return Err(ReserveTransactionForwarderError::IdentityConflict);
        }
        if let Some(existing) = state.checkpoint.completed.iter().find(|entry| {
            entry.identity_scope == prepared.identity_scope
                && entry.identity_digest == prepared.identity_digest
        }) {
            if existing.operation_id == operation_id
                && existing.semantic_digest == prepared.semantic_digest
            {
                return Ok(ReserveTransactionEnqueueResultV1::Existing { operation_id });
            }
            return Err(ReserveTransactionForwarderError::IdentityConflict);
        }
        if state.checkpoint.dead_letters.iter().any(|entry| {
            entry.identity_scope == prepared.identity_scope
                && entry.identity_digest == prepared.identity_digest
        }) {
            return Err(ReserveTransactionForwarderError::DeadLetterConflict);
        }
        if state.checkpoint.pending.len() >= self.policy.max_pending {
            return Err(ReserveTransactionForwarderError::PendingCapacityExhausted);
        }
        let sequence = state.checkpoint.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(ReserveTransactionForwarderError::SequenceExhausted)?;
        let mut candidate = state.checkpoint.clone();
        candidate.next_sequence = next_sequence;
        candidate.pending.push(StoredPendingReserveTransactionV1 {
            sequence,
            operation_id,
            identity_scope: prepared.identity_scope,
            identity_digest: prepared.identity_digest,
            semantic_digest: prepared.semantic_digest,
            chain_id: prepared.chain_id,
            authority: prepared.authority,
            policy_record: prepared.policy_record,
            projection: prepared.projection,
            operation: prepared.operation,
            state: if signed_transaction_bytes.is_some() {
                StoredDeliveryStateV1::Signed
            } else {
                StoredDeliveryStateV1::Ready
            },
            attempts: u32::from(signed_transaction_bytes.is_some()),
            baseline_finalized_height: baseline_finalized_cursor.height,
            baseline_finalized_block_hash: baseline_finalized_cursor.block_hash,
            signed_transaction_bytes: signed_transaction_bytes.map(<[u8]>::to_vec),
        });
        candidate.pending.sort_by_key(|entry| entry.sequence);
        self.commit_candidate(&mut state, candidate)?;
        Ok(ReserveTransactionEnqueueResultV1::Inserted { operation_id })
    }

    /// Return pending entries in stable sequence order.
    pub fn pending(
        &self,
        limit: usize,
    ) -> Result<Vec<ReserveTransactionPendingV1>, ReserveTransactionForwarderError> {
        self.pending_after(None, limit)
    }

    /// Return a circular page after an immutable sequence cursor.
    ///
    /// Newer entries are returned first and the scan then wraps to the oldest
    /// retained entries. At most one snapshot of each entry is returned.
    pub fn pending_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<ReserveTransactionPendingV1>, ReserveTransactionForwarderError> {
        if limit == 0 || limit > RESERVE_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1 {
            return Err(ReserveTransactionForwarderError::InvalidScanLimit);
        }
        let state = self.lock_state()?;
        let pending = &state.checkpoint.pending;
        if pending.is_empty() {
            return Ok(Vec::new());
        }
        let start = after_sequence.map_or(0, |sequence| {
            pending.partition_point(|entry| entry.sequence <= sequence)
        });
        Ok(pending[start..]
            .iter()
            .chain(pending[..start].iter())
            .take(limit.min(pending.len()))
            .map(StoredPendingReserveTransactionV1::snapshot)
            .collect())
    }

    /// Return exact semantic and finalized binding material for reconciliation.
    pub fn operation_for_reconciliation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<ReserveTransactionReconciliationV1, ReserveTransactionForwarderError> {
        let state = self.lock_state()?;
        let entry = state
            .checkpoint
            .pending
            .iter()
            .find(|entry| entry.operation_id == operation_id)
            .ok_or(ReserveTransactionForwarderError::UnknownOperation)?;
        Ok(reconciliation_material(entry))
    }

    /// Return payload-free dead letters in stable operation order.
    pub fn dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<ReserveTransactionDeadLetterV1>, ReserveTransactionForwarderError> {
        if limit == 0 || limit > RESERVE_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1 {
            return Err(ReserveTransactionForwarderError::InvalidScanLimit);
        }
        let state = self.lock_state()?;
        Ok(state
            .checkpoint
            .dead_letters
            .iter()
            .take(limit)
            .map(|entry| ReserveTransactionDeadLetterV1 {
                operation_id: entry.operation_id,
                kind: entry.operation.kind(),
                chain_id: entry.chain_id.clone(),
                policy_digest: entry.policy_record.policy_digest,
                policy_revision: entry.policy_record.policy.revision,
                provider_id: operation_provider_id(&entry.operation, &entry.projection),
                expected_provider_revision: entry.operation.expected_provider_revision(),
                semantic_digest: entry.semantic_digest,
                transaction_digest: entry
                    .signed_transaction_bytes
                    .as_deref()
                    .map(transaction_digest),
                reason: entry.reason.into(),
                observed_finalized_height: entry.observed_finalized_height,
                observed_finalized_block_hash: entry.observed_finalized_block_hash,
            })
            .collect())
    }

    /// Atomically claim a ready operation for isolated external signing.
    ///
    /// The attempt is consumed before signer material is exposed. Crash
    /// recovery resets only the signer-only state and never refunds an attempt.
    pub fn claim_for_signing(
        &self,
        operation_id: [u8; 32],
    ) -> Result<ReserveTransactionSigningRequestV1, ReserveTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        if candidate.pending[position].state == StoredDeliveryStateV1::Ready
            && candidate.pending[position]
                .signed_transaction_bytes
                .is_none()
            && candidate.pending[position].attempts >= self.policy.max_attempts
        {
            let cursor = entry_baseline_cursor(&candidate.pending[position])?;
            self.move_to_dead_letter(
                &mut candidate,
                position,
                StoredDeadLetterReasonV1::RetryExhausted,
                cursor,
            )?;
            self.commit_candidate(&mut state, candidate)?;
            return Err(ReserveTransactionForwarderError::RetryExhausted);
        }
        let entry = &mut candidate.pending[position];
        claim_for_signing(entry, self.policy.max_attempts)?;
        let request = ReserveTransactionSigningRequestV1 {
            operation_id: entry.operation_id,
            chain_id: entry.chain_id.clone(),
            authority: entry.authority.clone(),
            operation: entry.operation.clone(),
        };
        self.commit_candidate(&mut state, candidate)?;
        Ok(request)
    }

    /// Persist exact signed bytes for a claimed authority and operation.
    pub fn store_signed_transaction(
        &self,
        expected_operation_id: [u8; 32],
        signed_transaction_bytes: &[u8],
    ) -> Result<[u8; 32], ReserveTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let entry = find_pending_mut(&mut candidate, expected_operation_id)?;
        let context = context_for_stored_entry(entry)?;
        let prepared = PreparedReserveOperation::decode_signed_transaction(
            signed_transaction_bytes,
            &context,
            self.policy.max_transaction_bytes,
        )?;
        if prepared.identity_scope != entry.identity_scope
            || prepared.identity_digest != entry.identity_digest
            || prepared.semantic_digest != entry.semantic_digest
            || prepared.chain_id != entry.chain_id
            || prepared.authority != entry.authority
            || prepared.policy_record != entry.policy_record
            || prepared.projection != entry.projection
            || prepared.operation != entry.operation
            || operation_id(&prepared) != entry.operation_id
        {
            return Err(ReserveTransactionForwarderError::InvalidSignedTransaction);
        }
        store_signed_transaction(entry, signed_transaction_bytes.to_vec())?;
        let digest = transaction_digest(signed_transaction_bytes);
        self.commit_candidate(&mut state, candidate)?;
        Ok(digest)
    }

    /// Release a signer claim after a failure known to precede submission.
    pub fn release_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ReserveTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        release_signing_claim(&mut candidate.pending[position])?;
        if candidate.pending[position].attempts >= self.policy.max_attempts {
            let cursor = entry_baseline_cursor(&candidate.pending[position])?;
            self.move_to_dead_letter(
                &mut candidate,
                position,
                StoredDeadLetterReasonV1::RetryExhausted,
                cursor,
            )?;
        }
        self.commit_candidate(&mut state, candidate)
    }

    /// Mark exact bytes ambiguous before exposing them to a submitter.
    pub fn begin_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Vec<u8>, ReserveTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let entry = find_pending_mut(&mut candidate, operation_id)?;
        let bytes = durable::begin_submission(entry)?;
        self.commit_candidate(&mut state, candidate)?;
        Ok(bytes)
    }

    /// Record that the exact transaction is known pending or applied.
    pub fn mark_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.mutate_entry(operation_id, |entry| {
            durable::mark_submitted(entry).map_err(Into::into)
        })
    }

    /// Record a failure proven to have happened before queue submission.
    pub fn mark_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.mutate_entry(operation_id, |entry| {
            durable::mark_not_submitted(entry).map_err(Into::into)
        })
    }

    /// Retry the same exact bytes only after finalized absence is proven.
    pub fn mark_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        if durable::mark_finalized_absent(
            &mut candidate.pending[position],
            finalized_cursor(observed_finalized_cursor),
            self.policy.max_attempts,
        )? == RetryBoundOutcome::Exhausted
        {
            self.move_to_dead_letter(
                &mut candidate,
                position,
                StoredDeadLetterReasonV1::RetryExhausted,
                observed_finalized_cursor,
            )?;
        }
        self.commit_candidate(&mut state, candidate)
    }

    /// Reconcile exact finalized success using the retained transaction digest.
    pub fn mark_finalized(
        &self,
        operation_id: [u8; 32],
        expected_transaction_digest: [u8; 32],
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        validate_finalized_cursor(finalized_cursor)?;
        if expected_transaction_digest == [0; 32] {
            return Err(ReserveTransactionForwarderError::InvalidSignedTransaction);
        }
        let mut state = self.lock_state()?;
        let candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        let entry = &candidate.pending[position];
        if entry
            .signed_transaction_bytes
            .as_deref()
            .map(transaction_digest)
            != Some(expected_transaction_digest)
        {
            return Err(ReserveTransactionForwarderError::InvalidSignedTransaction);
        }
        self.commit_finalized_operation(&mut state, candidate, position, finalized_cursor)
    }

    /// Reconcile exact semantic success committed through another ingress.
    ///
    /// The caller must first compare `operation_for_reconciliation` against one
    /// coherent finalized ledger view.
    pub fn mark_semantic_finalized(
        &self,
        operation_id: [u8; 32],
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        validate_finalized_cursor(finalized_cursor)?;
        let mut state = self.lock_state()?;
        let candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        self.commit_finalized_operation(&mut state, candidate, position, finalized_cursor)
    }

    fn commit_finalized_operation(
        &self,
        state: &mut DurableState,
        mut candidate: ReserveTransactionForwarderCheckpointV1,
        position: usize,
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        if candidate.completed.len() >= self.policy.max_completed {
            let oldest = candidate
                .completed
                .iter()
                .enumerate()
                .min_by_key(|(_, completed)| (completed.finalized_height, completed.operation_id))
                .map(|(index, _)| index)
                .ok_or(ReserveTransactionForwarderError::InvalidCheckpoint)?;
            candidate.completed.remove(oldest);
        }
        let entry = candidate.pending.remove(position);
        candidate
            .completed
            .push(StoredCompletedReserveTransactionV1 {
                operation_id: entry.operation_id,
                identity_scope: entry.identity_scope,
                identity_digest: entry.identity_digest,
                semantic_digest: entry.semantic_digest,
                finalized_height: finalized_cursor.height,
                finalized_block_hash: finalized_cursor.block_hash,
            });
        candidate
            .completed
            .sort_by_key(|completed| completed.operation_id);
        self.commit_candidate(state, candidate)
    }

    /// Atomically dead-letter an operation that conflicts with finalized state.
    pub fn mark_finalized_conflict(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        self.move_to_dead_letter(
            &mut candidate,
            position,
            StoredDeadLetterReasonV1::FinalizedConflict,
            observed_finalized_cursor,
        )?;
        self.commit_candidate(&mut state, candidate)
    }

    /// Clear a terminally rejected envelope for bounded replacement signing.
    pub fn mark_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        validate_finalized_cursor(observed_finalized_cursor)?;
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        let entry = &candidate.pending[position];
        if !matches!(
            entry.state,
            StoredDeliveryStateV1::Ambiguous | StoredDeliveryStateV1::Submitted
        ) || entry.signed_transaction_bytes.is_none()
        {
            return Err(ReserveTransactionForwarderError::InvalidTransition);
        }
        if durable::mark_transaction_rejected(
            &mut candidate.pending[position],
            self.policy.max_attempts,
        ) == RetryBoundOutcome::Exhausted
        {
            self.move_to_dead_letter(
                &mut candidate,
                position,
                StoredDeadLetterReasonV1::TransactionRejected,
                observed_finalized_cursor,
            )?;
        } else {
            candidate.pending[position].baseline_finalized_height =
                observed_finalized_cursor.height;
            candidate.pending[position].baseline_finalized_block_hash =
                observed_finalized_cursor.block_hash;
        }
        self.commit_candidate(&mut state, candidate)
    }

    fn mutate_entry(
        &self,
        operation_id: [u8; 32],
        mutate: impl FnOnce(
            &mut StoredPendingReserveTransactionV1,
        ) -> Result<(), ReserveTransactionForwarderError>,
    ) -> Result<(), ReserveTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        mutate(find_pending_mut(&mut candidate, operation_id)?)?;
        self.commit_candidate(&mut state, candidate)
    }

    fn move_to_dead_letter(
        &self,
        checkpoint: &mut ReserveTransactionForwarderCheckpointV1,
        position: usize,
        reason: StoredDeadLetterReasonV1,
        cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        validate_finalized_cursor(cursor)?;
        if checkpoint.dead_letters.len() >= self.policy.max_dead_letters {
            return Err(ReserveTransactionForwarderError::DeadLetterCapacityExhausted);
        }
        let entry = checkpoint.pending.remove(position);
        checkpoint
            .dead_letters
            .push(StoredDeadReserveTransactionV1 {
                operation_id: entry.operation_id,
                identity_scope: entry.identity_scope,
                identity_digest: entry.identity_digest,
                semantic_digest: entry.semantic_digest,
                chain_id: entry.chain_id,
                authority: entry.authority,
                policy_record: entry.policy_record,
                projection: entry.projection,
                operation: entry.operation,
                signed_transaction_bytes: entry.signed_transaction_bytes,
                reason,
                observed_finalized_height: cursor.height,
                observed_finalized_block_hash: cursor.block_hash,
            });
        checkpoint
            .dead_letters
            .sort_by_key(|dead| dead.operation_id);
        Ok(())
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, DurableState>, ReserveTransactionForwarderError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ReserveTransactionForwarderError::RuntimePoisoned)?;
        if state.durability_failure {
            return Err(ReserveTransactionForwarderError::DurabilityPoisoned);
        }
        Ok(state)
    }

    fn commit_candidate(
        &self,
        state: &mut DurableState,
        candidate: ReserveTransactionForwarderCheckpointV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        validate_checkpoint(&candidate, self.policy)?;
        if let Some(store) = self.store.as_ref() {
            let bytes = norito::to_bytes(&candidate)
                .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
            match store.commit_bytes(&bytes, state.fingerprint) {
                Ok(fingerprint) => state.fingerprint = Some(fingerprint),
                Err(error) => {
                    if error == CheckpointStoreError::DurabilityUncertain {
                        state.durability_failure = true;
                    }
                    return Err(error.into());
                }
            }
        }
        state.checkpoint = candidate;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PreparedReserveOperation {
    identity_scope: StoredReserveIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    chain_id: ChainId,
    authority: AccountId,
    policy_record: ReserveAuthorityPolicyRecordV1,
    projection: ReserveTransactionProjectionV1,
    operation: ReserveOperationV1,
}

impl PreparedReserveOperation {
    fn from_unsigned(
        operation: ReserveOperationV1,
        context: &ReserveTransactionContextV1,
        max_transaction_bytes: usize,
    ) -> Result<Self, ReserveTransactionForwarderError> {
        let authority = governed_authority(
            &context.policy_record,
            &context.projection,
            operation.kind(),
        )?
        .clone();
        Self::new_bounded(
            context.chain_id.clone(),
            authority,
            context.policy_record.clone(),
            context.projection.clone(),
            operation,
            max_transaction_bytes,
        )
    }

    fn new(
        chain_id: ChainId,
        authority: AccountId,
        policy_record: ReserveAuthorityPolicyRecordV1,
        projection: ReserveTransactionProjectionV1,
        operation: ReserveOperationV1,
    ) -> Result<Self, ReserveTransactionForwarderError> {
        if chain_id.as_str().is_empty()
            || chain_id.as_str().len() > RESERVE_TRANSACTION_MAX_CHAIN_ID_BYTES_V1
        {
            return Err(ReserveTransactionForwarderError::InvalidGovernanceContext);
        }
        validate_operation(&operation, &authority, &policy_record, &projection)?;
        let (identity_scope, identity_digest) = operation_identity(&operation, &projection)?;
        if identity_digest == [0; 32] {
            return Err(ReserveTransactionForwarderError::InvalidReserveOperation);
        }
        let semantic_digest = semantic_digest(
            &chain_id,
            &authority,
            &policy_record,
            &projection,
            &operation,
        )?;
        Ok(Self {
            identity_scope,
            identity_digest,
            semantic_digest,
            chain_id,
            authority,
            policy_record,
            projection,
            operation,
        })
    }

    fn new_bounded(
        chain_id: ChainId,
        authority: AccountId,
        policy_record: ReserveAuthorityPolicyRecordV1,
        projection: ReserveTransactionProjectionV1,
        operation: ReserveOperationV1,
        max_transaction_bytes: usize,
    ) -> Result<Self, ReserveTransactionForwarderError> {
        let prepared = Self::new(chain_id, authority, policy_record, projection, operation)?;
        let chain_id_bytes = norito::to_bytes(&prepared.chain_id)
            .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
        let authority_bytes = norito::to_bytes(&prepared.authority)
            .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
        let policy_bytes = norito::to_bytes(&prepared.policy_record)
            .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
        let projection_bytes = norito::to_bytes(&prepared.projection)
            .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
        let operation_bytes = norito::to_bytes(&prepared.operation)
            .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
        if chain_id_bytes
            .len()
            .checked_add(authority_bytes.len())
            .and_then(|length| length.checked_add(policy_bytes.len()))
            .and_then(|length| length.checked_add(projection_bytes.len()))
            .and_then(|length| length.checked_add(operation_bytes.len()))
            .is_none_or(|length| length > max_transaction_bytes)
        {
            return Err(ReserveTransactionForwarderError::ResourceLimitExceeded);
        }
        Ok(prepared)
    }

    fn decode_signed_transaction(
        bytes: &[u8],
        context: &ReserveTransactionContextV1,
        max_transaction_bytes: usize,
    ) -> Result<Self, ReserveTransactionForwarderError> {
        if bytes.is_empty()
            || bytes.len() > max_transaction_bytes
            || bytes.len() > RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1
        {
            return Err(ReserveTransactionForwarderError::InvalidSignedTransaction);
        }
        norito::core::from_bytes_view(bytes)
            .map_err(|_| ReserveTransactionForwarderError::InvalidSignedTransaction)?;
        let transaction = norito::decode_from_bytes_with_limits::<SignedTransaction>(
            bytes,
            transaction_decode_limits(bytes.len(), max_transaction_bytes)?,
        )
        .map_err(|_| ReserveTransactionForwarderError::InvalidSignedTransaction)?;
        if norito::to_bytes(&transaction)
            .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?
            != bytes
            || transaction.verify_signature().is_err()
        {
            return Err(ReserveTransactionForwarderError::InvalidSignedTransaction);
        }
        if transaction.chain() != &context.chain_id {
            return Err(ReserveTransactionForwarderError::ChainIdMismatch);
        }
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(ReserveTransactionForwarderError::InvalidSignedTransaction);
        };
        if instructions.len() != 1 {
            return Err(ReserveTransactionForwarderError::InvalidSignedTransaction);
        }
        let operation = decode_native_operation(&instructions[0])?;
        Self::new_bounded(
            context.chain_id.clone(),
            transaction.authority().clone(),
            context.policy_record.clone(),
            context.projection.clone(),
            operation,
            max_transaction_bytes,
        )
    }
}

fn decode_native_operation(
    instruction: &InstructionBox,
) -> Result<ReserveOperationV1, ReserveTransactionForwarderError> {
    if let Some(instruction) = instruction
        .as_any()
        .downcast_ref::<RegisterSorafsReserveAccount>()
    {
        Ok(ReserveOperationV1::RegisterProvider(instruction.clone()))
    } else if let Some(instruction) = instruction
        .as_any()
        .downcast_ref::<RequestSorafsReserveMovement>()
    {
        Ok(ReserveOperationV1::RequestMovement(instruction.clone()))
    } else if let Some(instruction) = instruction
        .as_any()
        .downcast_ref::<DecideSorafsReserveMovement>()
    {
        Ok(ReserveOperationV1::DecideMovement(instruction.clone()))
    } else if let Some(instruction) = instruction
        .as_any()
        .downcast_ref::<ChargeSorafsReserveRent>()
    {
        Ok(ReserveOperationV1::ChargeRent(instruction.clone()))
    } else if let Some(instruction) = instruction
        .as_any()
        .downcast_ref::<AdvanceSorafsReserveLifecycle>()
    {
        Ok(ReserveOperationV1::AdvanceLifecycle(instruction.clone()))
    } else if let Some(instruction) = instruction
        .as_any()
        .downcast_ref::<DrawSorafsReserveCredit>()
    {
        Ok(ReserveOperationV1::DrawCredit(instruction.clone()))
    } else if let Some(instruction) = instruction
        .as_any()
        .downcast_ref::<RepaySorafsReserveCredit>()
    {
        Ok(ReserveOperationV1::RepayCredit(instruction.clone()))
    } else if let Some(instruction) = instruction
        .as_any()
        .downcast_ref::<SubmitSorafsReserveAppeal>()
    {
        Ok(ReserveOperationV1::SubmitAppeal(instruction.clone()))
    } else if let Some(instruction) = instruction
        .as_any()
        .downcast_ref::<DecideSorafsReserveAppeal>()
    {
        Ok(ReserveOperationV1::DecideAppeal(instruction.clone()))
    } else {
        Err(ReserveTransactionForwarderError::InvalidSignedTransaction)
    }
}

fn governed_authority<'a>(
    policy_record: &'a ReserveAuthorityPolicyRecordV1,
    projection: &'a ReserveTransactionProjectionV1,
    kind: ReserveTransactionKindV1,
) -> Result<&'a AccountId, ReserveTransactionForwarderError> {
    match kind {
        ReserveTransactionKindV1::RegisterProvider
        | ReserveTransactionKindV1::ChargeRent
        | ReserveTransactionKindV1::AdvanceLifecycle
        | ReserveTransactionKindV1::DrawCredit => Ok(&policy_record.policy.operations_authority),
        ReserveTransactionKindV1::DecideMovement | ReserveTransactionKindV1::DecideAppeal => {
            Ok(&policy_record.policy.decision_authority)
        }
        ReserveTransactionKindV1::RequestMovement
        | ReserveTransactionKindV1::RepayCredit
        | ReserveTransactionKindV1::SubmitAppeal => match projection {
            ReserveTransactionProjectionV1::Provider { account } => {
                Ok(&account.terms.provider_account)
            }
            _ => Err(ReserveTransactionForwarderError::ProjectionKindMismatch),
        },
    }
}

fn validate_operation(
    operation: &ReserveOperationV1,
    authority: &AccountId,
    policy_record: &ReserveAuthorityPolicyRecordV1,
    projection: &ReserveTransactionProjectionV1,
) -> Result<(), ReserveTransactionForwarderError> {
    policy_record
        .policy
        .validate()
        .map_err(|_| ReserveTransactionForwarderError::InvalidGovernanceContext)?;
    let policy_digest = policy_record
        .policy
        .digest()
        .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
    if policy_digest == [0; 32] || policy_record.policy_digest != policy_digest {
        return Err(ReserveTransactionForwarderError::InvalidGovernanceContext);
    }
    if operation.policy_digest() != policy_digest {
        return Err(ReserveTransactionForwarderError::PolicyDigestMismatch);
    }
    if authority != governed_authority(policy_record, projection, operation.kind())? {
        return Err(ReserveTransactionForwarderError::GovernedAuthorityMismatch);
    }

    match (operation, projection) {
        (
            ReserveOperationV1::RegisterProvider(instruction),
            ReserveTransactionProjectionV1::Registration { provider_owner },
        ) => {
            let terms = instruction.terms();
            if terms.provider_id.as_bytes() == &[0; 32]
                || terms.capacity_gib == 0
                || &terms.provider_account != provider_owner
            {
                return Err(ReserveTransactionForwarderError::ProviderOwnerMismatch);
            }
        }
        (
            ReserveOperationV1::RequestMovement(instruction),
            ReserveTransactionProjectionV1::Provider { account },
        ) => {
            validate_provider_binding(
                account,
                *instruction.provider_id(),
                *instruction.expected_provider_revision(),
            )?;
            if *instruction.movement_id() == [0; 32]
                || instruction.amount().is_zero()
                || account.pending_movements
                    >= policy_record.policy.max_pending_movements_per_provider
            {
                return Err(ReserveTransactionForwarderError::InvalidReserveOperation);
            }
        }
        (
            ReserveOperationV1::DecideMovement(instruction),
            ReserveTransactionProjectionV1::MovementDecision { account, movement },
        ) => {
            validate_provider_binding(
                account,
                movement.provider_id,
                *instruction.expected_provider_revision(),
            )?;
            if *instruction.movement_id() != movement.movement_id
                || movement.status != ReserveMovementStatusV1::Pending
                || account.pending_movements == 0
                || !valid_reason(instruction.rationale())
            {
                return Err(ReserveTransactionForwarderError::InvalidReserveOperation);
            }
        }
        (
            ReserveOperationV1::ChargeRent(instruction),
            ReserveTransactionProjectionV1::Provider { account },
        ) => {
            validate_provider_binding(
                account,
                *instruction.provider_id(),
                *instruction.expected_provider_revision(),
            )?;
            if !(1..=RESERVE_RENT_MAX_BILLING_PERIODS_V1).contains(instruction.billing_periods()) {
                return Err(ReserveTransactionForwarderError::InvalidReserveOperation);
            }
        }
        (
            ReserveOperationV1::AdvanceLifecycle(instruction),
            ReserveTransactionProjectionV1::Provider { account },
        ) => validate_provider_binding(
            account,
            *instruction.provider_id(),
            *instruction.expected_provider_revision(),
        )?,
        (
            ReserveOperationV1::DrawCredit(instruction),
            ReserveTransactionProjectionV1::Provider { account },
        ) => {
            validate_provider_binding(
                account,
                *instruction.provider_id(),
                *instruction.expected_provider_revision(),
            )?;
            if instruction.amount().is_zero() {
                return Err(ReserveTransactionForwarderError::InvalidReserveOperation);
            }
        }
        (
            ReserveOperationV1::RepayCredit(instruction),
            ReserveTransactionProjectionV1::Provider { account },
        ) => {
            validate_provider_binding(
                account,
                *instruction.provider_id(),
                *instruction.expected_provider_revision(),
            )?;
            if instruction.amount().is_zero() {
                return Err(ReserveTransactionForwarderError::InvalidReserveOperation);
            }
        }
        (
            ReserveOperationV1::SubmitAppeal(instruction),
            ReserveTransactionProjectionV1::Provider { account },
        ) => {
            validate_provider_binding(
                account,
                *instruction.provider_id(),
                *instruction.expected_provider_revision(),
            )?;
            if *instruction.appeal_id() == [0; 32]
                || !valid_reason(instruction.reason())
                || *instruction.evidence_digest() == Some([0; 32])
                || account.open_appeals >= policy_record.policy.max_open_appeals_per_provider
            {
                return Err(ReserveTransactionForwarderError::InvalidReserveOperation);
            }
        }
        (
            ReserveOperationV1::DecideAppeal(instruction),
            ReserveTransactionProjectionV1::AppealDecision { account, appeal },
        ) => {
            validate_provider_binding(
                account,
                appeal.provider_id,
                *instruction.expected_provider_revision(),
            )?;
            if *instruction.appeal_id() != appeal.appeal_id
                || appeal.status != ReserveAppealStatusV1::Pending
                || account.open_appeals == 0
                || !valid_reason(instruction.rationale())
            {
                return Err(ReserveTransactionForwarderError::InvalidReserveOperation);
            }
        }
        _ => return Err(ReserveTransactionForwarderError::ProjectionKindMismatch),
    }
    Ok(())
}

fn validate_provider_binding(
    account: &ReserveProviderAccountV1,
    provider_id: ProviderId,
    expected_provider_revision: u64,
) -> Result<(), ReserveTransactionForwarderError> {
    validate_provider_account(account)?;
    if account.terms.provider_id != provider_id {
        return Err(ReserveTransactionForwarderError::ProviderMismatch);
    }
    if expected_provider_revision == 0 || account.revision != expected_provider_revision {
        return Err(ReserveTransactionForwarderError::ProviderRevisionMismatch);
    }
    Ok(())
}

fn validate_provider_account(
    account: &ReserveProviderAccountV1,
) -> Result<(), ReserveTransactionForwarderError> {
    if account.terms.provider_id.as_bytes() == &[0; 32]
        || account.terms.capacity_gib == 0
        || account.policy_digest == [0; 32]
        || account.revision == 0
        || account.debt_principal > account.credit_cap
        || account.pending_movements > RESERVE_MAX_PENDING_MOVEMENTS_V1
        || account.open_appeals > RESERVE_MAX_OPEN_APPEALS_V1
        || account.rent_charged_through_unix == 0
        || account.interest_accrued_at_unix == 0
        || account.updated_at_unix == 0
        || account.rent_charged_through_unix > account.updated_at_unix
        || account.interest_accrued_at_unix > account.updated_at_unix
    {
        return Err(ReserveTransactionForwarderError::InvalidFinalizedProjection);
    }
    Ok(())
}

fn validate_movement_record(
    movement: &ReserveMovementRecordV1,
) -> Result<(), ReserveTransactionForwarderError> {
    let terminal_fields = movement.decided_by.is_some()
        && movement.decided_at_unix.is_some()
        && movement.rationale.as_deref().is_some_and(valid_reason);
    let valid_status = match movement.status {
        ReserveMovementStatusV1::Pending => {
            movement.decided_by.is_none()
                && movement.decided_at_unix.is_none()
                && movement.rationale.is_none()
        }
        ReserveMovementStatusV1::Approved | ReserveMovementStatusV1::Rejected => terminal_fields,
    };
    if movement.movement_id == [0; 32]
        || movement.provider_id.as_bytes() == &[0; 32]
        || movement.amount.is_zero()
        || movement.expected_provider_revision == 0
        || movement.policy_digest == [0; 32]
        || movement.requested_at_unix == 0
        || !valid_status
    {
        return Err(ReserveTransactionForwarderError::InvalidFinalizedProjection);
    }
    Ok(())
}

fn validate_appeal_record(
    appeal: &ReserveAppealRecordV1,
) -> Result<(), ReserveTransactionForwarderError> {
    let terminal_fields = appeal.decided_by.is_some()
        && appeal.decided_at_unix.is_some()
        && appeal.rationale.as_deref().is_some_and(valid_reason);
    let valid_status = match appeal.status {
        ReserveAppealStatusV1::Pending => {
            appeal.decided_by.is_none()
                && appeal.decided_at_unix.is_none()
                && appeal.rationale.is_none()
        }
        ReserveAppealStatusV1::Accepted | ReserveAppealStatusV1::Rejected => terminal_fields,
    };
    if appeal.appeal_id == [0; 32]
        || appeal.provider_id.as_bytes() == &[0; 32]
        || !valid_reason(&appeal.reason)
        || appeal.evidence_digest == Some([0; 32])
        || appeal.expected_provider_revision == 0
        || appeal.submitted_at_unix == 0
        || !valid_status
    {
        return Err(ReserveTransactionForwarderError::InvalidFinalizedProjection);
    }
    Ok(())
}

fn valid_reason(reason: &str) -> bool {
    !reason.is_empty() && reason.len() <= RESERVE_MAX_REASON_BYTES_V1
}

fn operation_provider_id(
    operation: &ReserveOperationV1,
    projection: &ReserveTransactionProjectionV1,
) -> Option<ProviderId> {
    operation.provider_id().or(match projection {
        ReserveTransactionProjectionV1::MovementDecision { movement, .. } => {
            Some(movement.provider_id)
        }
        ReserveTransactionProjectionV1::AppealDecision { appeal, .. } => Some(appeal.provider_id),
        ReserveTransactionProjectionV1::Registration { .. }
        | ReserveTransactionProjectionV1::Provider { .. } => None,
    })
}

fn operation_identity(
    operation: &ReserveOperationV1,
    projection: &ReserveTransactionProjectionV1,
) -> Result<(StoredReserveIdentityScopeV1, [u8; 32]), ReserveTransactionForwarderError> {
    match operation {
        ReserveOperationV1::RegisterProvider(instruction) => Ok((
            StoredReserveIdentityScopeV1::ProviderRegistration,
            *instruction.terms().provider_id.as_bytes(),
        )),
        ReserveOperationV1::RequestMovement(instruction) => Ok((
            StoredReserveIdentityScopeV1::MovementRequest,
            *instruction.movement_id(),
        )),
        ReserveOperationV1::DecideMovement(instruction) => Ok((
            StoredReserveIdentityScopeV1::MovementDecision,
            *instruction.movement_id(),
        )),
        ReserveOperationV1::ChargeRent(instruction) => Ok((
            StoredReserveIdentityScopeV1::RentRevision,
            revision_identity_digest(
                operation.policy_digest(),
                *instruction.provider_id(),
                *instruction.expected_provider_revision(),
            ),
        )),
        ReserveOperationV1::AdvanceLifecycle(instruction) => Ok((
            StoredReserveIdentityScopeV1::LifecycleRevision,
            revision_identity_digest(
                operation.policy_digest(),
                *instruction.provider_id(),
                *instruction.expected_provider_revision(),
            ),
        )),
        ReserveOperationV1::DrawCredit(instruction) => Ok((
            StoredReserveIdentityScopeV1::CreditDrawRevision,
            revision_identity_digest(
                operation.policy_digest(),
                *instruction.provider_id(),
                *instruction.expected_provider_revision(),
            ),
        )),
        ReserveOperationV1::RepayCredit(instruction) => Ok((
            StoredReserveIdentityScopeV1::CreditRepaymentRevision,
            revision_identity_digest(
                operation.policy_digest(),
                *instruction.provider_id(),
                *instruction.expected_provider_revision(),
            ),
        )),
        ReserveOperationV1::SubmitAppeal(instruction) => Ok((
            StoredReserveIdentityScopeV1::AppealSubmission,
            *instruction.appeal_id(),
        )),
        ReserveOperationV1::DecideAppeal(instruction) => Ok((
            StoredReserveIdentityScopeV1::AppealDecision,
            *instruction.appeal_id(),
        )),
    }
    .and_then(|identity| {
        if operation_provider_id(operation, projection).is_none()
            && !matches!(operation, ReserveOperationV1::RegisterProvider(_))
        {
            Err(ReserveTransactionForwarderError::InvalidFinalizedProjection)
        } else {
            Ok(identity)
        }
    })
}

fn revision_identity_digest(
    policy_digest: [u8; 32],
    provider_id: ProviderId,
    expected_provider_revision: u64,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(REVISION_IDENTITY_DOMAIN_V1);
    hasher.update(&policy_digest);
    hasher.update(provider_id.as_bytes());
    hasher.update(&expected_provider_revision.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn semantic_digest(
    chain_id: &ChainId,
    authority: &AccountId,
    policy_record: &ReserveAuthorityPolicyRecordV1,
    projection: &ReserveTransactionProjectionV1,
    operation: &ReserveOperationV1,
) -> Result<[u8; 32], ReserveTransactionForwarderError> {
    let authority = authority.to_string();
    let policy = norito::to_bytes(policy_record)
        .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
    let projection = norito::to_bytes(projection)
        .map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
    let operation =
        norito::to_bytes(operation).map_err(ReserveTransactionForwarderError::CanonicalEncoding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(SEMANTIC_DIGEST_DOMAIN_V1);
    update_length_prefixed(&mut hasher, chain_id.as_str().as_bytes())?;
    update_length_prefixed(&mut hasher, authority.as_bytes())?;
    update_length_prefixed(&mut hasher, &policy)?;
    update_length_prefixed(&mut hasher, &projection)?;
    update_length_prefixed(&mut hasher, &operation)?;
    Ok(*hasher.finalize().as_bytes())
}

fn update_length_prefixed(
    hasher: &mut blake3::Hasher,
    bytes: &[u8],
) -> Result<(), ReserveTransactionForwarderError> {
    let length = u64::try_from(bytes.len())
        .map_err(|_| ReserveTransactionForwarderError::ResourceLimitExceeded)?;
    hasher.update(&length.to_le_bytes());
    hasher.update(bytes);
    Ok(())
}

fn operation_id(prepared: &PreparedReserveOperation) -> [u8; 32] {
    operation_id_from_parts(
        prepared.identity_scope,
        prepared.identity_digest,
        prepared.semantic_digest,
    )
}

fn operation_id_from_parts(
    identity_scope: StoredReserveIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(OPERATION_ID_DOMAIN_V1);
    hasher.update(&[identity_scope.tag()]);
    hasher.update(&identity_digest);
    hasher.update(&semantic_digest);
    *hasher.finalize().as_bytes()
}

fn transaction_digest(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn validate_reserve_delivery(entry: &StoredPendingReserveTransactionV1, max_attempts: u32) -> bool {
    let has_baseline =
        entry.baseline_finalized_height != 0 && entry.baseline_finalized_block_hash != [0; 32];
    let state_is_valid = match entry.state {
        StoredDeliveryStateV1::Ready => entry.signed_transaction_bytes.is_none(),
        StoredDeliveryStateV1::Signing => {
            entry.signed_transaction_bytes.is_none() && entry.attempts != 0
        }
        StoredDeliveryStateV1::Signed
        | StoredDeliveryStateV1::Ambiguous
        | StoredDeliveryStateV1::Submitted => {
            entry.signed_transaction_bytes.is_some() && entry.attempts != 0
        }
    };
    has_baseline && state_is_valid && entry.attempts <= max_attempts
}

fn recover_interrupted_signing(entry: &mut StoredPendingReserveTransactionV1) -> bool {
    if entry.state != StoredDeliveryStateV1::Signing {
        return false;
    }
    entry.state = StoredDeliveryStateV1::Ready;
    true
}

fn claim_for_signing(
    entry: &mut StoredPendingReserveTransactionV1,
    max_attempts: u32,
) -> Result<(), DeliveryTransitionError> {
    durable::validate_finalized_cursor(FinalizedCursorV1 {
        height: entry.baseline_finalized_height,
        block_hash: entry.baseline_finalized_block_hash,
    })?;
    if entry.state != StoredDeliveryStateV1::Ready || entry.signed_transaction_bytes.is_some() {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    if entry.attempts >= max_attempts {
        return Err(DeliveryTransitionError::RetryExhausted);
    }
    entry.attempts = entry
        .attempts
        .checked_add(1)
        .ok_or(DeliveryTransitionError::RetryExhausted)?;
    entry.state = StoredDeliveryStateV1::Signing;
    Ok(())
}

fn store_signed_transaction(
    entry: &mut StoredPendingReserveTransactionV1,
    signed_transaction_bytes: Vec<u8>,
) -> Result<(), DeliveryTransitionError> {
    if entry.state != StoredDeliveryStateV1::Signing
        || entry.signed_transaction_bytes.is_some()
        || entry.attempts == 0
    {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    entry.signed_transaction_bytes = Some(signed_transaction_bytes);
    entry.state = StoredDeliveryStateV1::Signed;
    Ok(())
}

fn release_signing_claim(
    entry: &mut StoredPendingReserveTransactionV1,
) -> Result<(), DeliveryTransitionError> {
    if entry.state != StoredDeliveryStateV1::Signing
        || entry.signed_transaction_bytes.is_some()
        || entry.attempts == 0
    {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    entry.state = StoredDeliveryStateV1::Ready;
    Ok(())
}

fn entry_baseline_cursor(
    entry: &StoredPendingReserveTransactionV1,
) -> Result<ReserveFinalizedCursorV1, ReserveTransactionForwarderError> {
    let cursor = ReserveFinalizedCursorV1 {
        height: entry.baseline_finalized_height,
        block_hash: entry.baseline_finalized_block_hash,
    };
    validate_finalized_cursor(cursor)?;
    Ok(cursor)
}

fn context_for_stored_entry(
    entry: &StoredPendingReserveTransactionV1,
) -> Result<ReserveTransactionContextV1, ReserveTransactionForwarderError> {
    let context = ReserveTransactionContextV1 {
        chain_id: entry.chain_id.clone(),
        policy_record: entry.policy_record.clone(),
        projection: entry.projection.clone(),
        finalized_cursor: entry_baseline_cursor(entry)?,
    };
    context.validate()?;
    Ok(context)
}

fn reconciliation_material(
    entry: &StoredPendingReserveTransactionV1,
) -> ReserveTransactionReconciliationV1 {
    ReserveTransactionReconciliationV1 {
        request: ReserveTransactionSigningRequestV1 {
            operation_id: entry.operation_id,
            chain_id: entry.chain_id.clone(),
            authority: entry.authority.clone(),
            operation: entry.operation.clone(),
        },
        policy_record: entry.policy_record.clone(),
        projection: entry.projection.clone(),
    }
}

fn find_pending_mut(
    checkpoint: &mut ReserveTransactionForwarderCheckpointV1,
    operation_id: [u8; 32],
) -> Result<&mut StoredPendingReserveTransactionV1, ReserveTransactionForwarderError> {
    checkpoint
        .pending
        .iter_mut()
        .find(|entry| entry.operation_id == operation_id)
        .ok_or(ReserveTransactionForwarderError::UnknownOperation)
}

fn pending_position(
    checkpoint: &ReserveTransactionForwarderCheckpointV1,
    operation_id: [u8; 32],
) -> Result<usize, ReserveTransactionForwarderError> {
    checkpoint
        .pending
        .iter()
        .position(|entry| entry.operation_id == operation_id)
        .ok_or(ReserveTransactionForwarderError::UnknownOperation)
}

fn validate_checkpoint(
    checkpoint: &ReserveTransactionForwarderCheckpointV1,
    policy: ReserveTransactionForwarderPolicyV1,
) -> Result<(), ReserveTransactionForwarderError> {
    if checkpoint.version != RESERVE_TRANSACTION_FORWARDER_CHECKPOINT_VERSION_V1
        || checkpoint.next_sequence == 0
        || checkpoint.pending.len() > policy.max_pending
        || checkpoint.completed.len() > policy.max_completed
        || checkpoint.dead_letters.len() > policy.max_dead_letters
    {
        return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
    }
    let mut identities = BTreeSet::new();
    let mut operations = BTreeSet::new();
    let mut previous_sequence = 0_u64;
    for entry in &checkpoint.pending {
        let prepared = PreparedReserveOperation::new_bounded(
            entry.chain_id.clone(),
            entry.authority.clone(),
            entry.policy_record.clone(),
            entry.projection.clone(),
            entry.operation.clone(),
            policy.max_transaction_bytes,
        )
        .map_err(|_| ReserveTransactionForwarderError::InvalidCheckpoint)?;
        if entry.sequence == 0
            || entry.sequence <= previous_sequence
            || entry.sequence >= checkpoint.next_sequence
            || entry.operation_id == [0; 32]
            || entry.identity_digest == [0; 32]
            || entry.semantic_digest == [0; 32]
            || prepared.identity_scope != entry.identity_scope
            || prepared.identity_digest != entry.identity_digest
            || prepared.semantic_digest != entry.semantic_digest
            || prepared.chain_id != entry.chain_id
            || operation_id(&prepared) != entry.operation_id
            || !validate_reserve_delivery(entry, policy.max_attempts)
            || !identities.insert((entry.identity_scope, entry.identity_digest))
            || !operations.insert(entry.operation_id)
        {
            return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
        }
        if let Some(bytes) = entry.signed_transaction_bytes.as_deref() {
            let context = context_for_stored_entry(entry)
                .map_err(|_| ReserveTransactionForwarderError::InvalidCheckpoint)?;
            let decoded = PreparedReserveOperation::decode_signed_transaction(
                bytes,
                &context,
                policy.max_transaction_bytes,
            )
            .map_err(|_| ReserveTransactionForwarderError::InvalidCheckpoint)?;
            if decoded.identity_scope != entry.identity_scope
                || decoded.identity_digest != entry.identity_digest
                || decoded.semantic_digest != entry.semantic_digest
                || decoded.chain_id != entry.chain_id
                || decoded.authority != entry.authority
                || decoded.policy_record != entry.policy_record
                || decoded.projection != entry.projection
                || decoded.operation != entry.operation
            {
                return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
            }
        }
        previous_sequence = entry.sequence;
    }
    for entry in &checkpoint.completed {
        if entry.operation_id == [0; 32]
            || entry.identity_digest == [0; 32]
            || entry.semantic_digest == [0; 32]
            || entry.finalized_height == 0
            || entry.finalized_block_hash == [0; 32]
            || operation_id_from_parts(
                entry.identity_scope,
                entry.identity_digest,
                entry.semantic_digest,
            ) != entry.operation_id
            || !identities.insert((entry.identity_scope, entry.identity_digest))
            || !operations.insert(entry.operation_id)
        {
            return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
        }
    }
    for entry in &checkpoint.dead_letters {
        let prepared = PreparedReserveOperation::new_bounded(
            entry.chain_id.clone(),
            entry.authority.clone(),
            entry.policy_record.clone(),
            entry.projection.clone(),
            entry.operation.clone(),
            policy.max_transaction_bytes,
        )
        .map_err(|_| ReserveTransactionForwarderError::InvalidCheckpoint)?;
        if entry.operation_id == [0; 32]
            || entry.identity_digest == [0; 32]
            || entry.semantic_digest == [0; 32]
            || entry.observed_finalized_height == 0
            || entry.observed_finalized_block_hash == [0; 32]
            || prepared.identity_scope != entry.identity_scope
            || prepared.identity_digest != entry.identity_digest
            || prepared.semantic_digest != entry.semantic_digest
            || prepared.chain_id != entry.chain_id
            || operation_id(&prepared) != entry.operation_id
            || !identities.insert((entry.identity_scope, entry.identity_digest))
            || !operations.insert(entry.operation_id)
        {
            return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
        }
        if let Some(bytes) = entry.signed_transaction_bytes.as_deref() {
            let synthetic_entry = StoredPendingReserveTransactionV1 {
                sequence: 1,
                operation_id: entry.operation_id,
                identity_scope: entry.identity_scope,
                identity_digest: entry.identity_digest,
                semantic_digest: entry.semantic_digest,
                chain_id: entry.chain_id.clone(),
                authority: entry.authority.clone(),
                policy_record: entry.policy_record.clone(),
                projection: entry.projection.clone(),
                operation: entry.operation.clone(),
                state: StoredDeliveryStateV1::Signed,
                attempts: 1,
                baseline_finalized_height: entry.observed_finalized_height,
                baseline_finalized_block_hash: entry.observed_finalized_block_hash,
                signed_transaction_bytes: Some(bytes.to_vec()),
            };
            let context = context_for_stored_entry(&synthetic_entry)
                .map_err(|_| ReserveTransactionForwarderError::InvalidCheckpoint)?;
            let decoded = PreparedReserveOperation::decode_signed_transaction(
                bytes,
                &context,
                policy.max_transaction_bytes,
            )
            .map_err(|_| ReserveTransactionForwarderError::InvalidCheckpoint)?;
            if decoded.identity_scope != entry.identity_scope
                || decoded.identity_digest != entry.identity_digest
                || decoded.semantic_digest != entry.semantic_digest
                || decoded.chain_id != entry.chain_id
                || decoded.authority != entry.authority
                || decoded.policy_record != entry.policy_record
                || decoded.projection != entry.projection
                || decoded.operation != entry.operation
            {
                return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
            }
        }
    }
    Ok(())
}

fn decode_checkpoint(
    bytes: &[u8],
    policy: ReserveTransactionForwarderPolicyV1,
) -> Result<ReserveTransactionForwarderCheckpointV1, ReserveTransactionForwarderError> {
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > policy.checkpoint_max_bytes
    {
        return Err(ReserveTransactionForwarderError::CheckpointTooLarge);
    }
    norito::core::from_bytes_view(bytes)
        .map_err(|_| ReserveTransactionForwarderError::InvalidCheckpoint)?;
    let checkpoint =
        norito::decode_from_bytes_with_limits::<ReserveTransactionForwarderCheckpointV1>(
            bytes,
            checkpoint_decode_limits(bytes.len())?,
        )
        .map_err(|_| ReserveTransactionForwarderError::InvalidCheckpoint)?;
    if norito::to_bytes(&checkpoint).map_err(ReserveTransactionForwarderError::CanonicalEncoding)?
        != bytes
    {
        return Err(ReserveTransactionForwarderError::InvalidCheckpoint);
    }
    validate_checkpoint(&checkpoint, policy)?;
    Ok(checkpoint)
}

fn validate_finalized_cursor(
    cursor: ReserveFinalizedCursorV1,
) -> Result<(), ReserveTransactionForwarderError> {
    durable::validate_finalized_cursor(finalized_cursor(cursor)).map_err(Into::into)
}

const fn finalized_cursor(cursor: ReserveFinalizedCursorV1) -> FinalizedCursorV1 {
    FinalizedCursorV1 {
        height: cursor.height,
        block_hash: cursor.block_hash,
    }
}

fn checkpoint_decode_limits(
    encoded_bytes: usize,
) -> Result<norito::DecodeLimits, ReserveTransactionForwarderError> {
    decode_limits(
        encoded_bytes,
        encoded_bytes,
        CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT,
        CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT,
        CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES,
        CHECKPOINT_MAX_NESTING_DEPTH,
    )
}

fn transaction_decode_limits(
    encoded_bytes: usize,
    max_transaction_bytes: usize,
) -> Result<norito::DecodeLimits, ReserveTransactionForwarderError> {
    decode_limits(
        encoded_bytes,
        max_transaction_bytes,
        TRANSACTION_ELEMENT_AMPLIFICATION_LIMIT,
        TRANSACTION_ALLOCATION_AMPLIFICATION_LIMIT,
        TRANSACTION_ALLOCATION_FIXED_OVERHEAD_BYTES,
        TRANSACTION_MAX_NESTING_DEPTH,
    )
}

fn decode_limits(
    encoded_bytes: usize,
    max_bytes: usize,
    element_amplification: usize,
    allocation_amplification: usize,
    fixed_allocation: usize,
    max_depth: usize,
) -> Result<norito::DecodeLimits, ReserveTransactionForwarderError> {
    if encoded_bytes == 0 || encoded_bytes > max_bytes {
        return Err(ReserveTransactionForwarderError::ResourceLimitExceeded);
    }
    let total_elements = encoded_bytes
        .checked_mul(element_amplification)
        .ok_or(ReserveTransactionForwarderError::ResourceLimitExceeded)?;
    let total_allocated_bytes = encoded_bytes
        .checked_mul(allocation_amplification)
        .and_then(|budget| budget.checked_add(fixed_allocation))
        .ok_or(ReserveTransactionForwarderError::ResourceLimitExceeded)?;
    Ok(norito::DecodeLimits::new(
        max_bytes,
        max_bytes,
        total_elements,
        total_allocated_bytes,
        max_depth,
    ))
}

/// Durable native reserve transaction forwarding error.
#[derive(Debug, Error)]
pub enum ReserveTransactionForwarderError {
    /// Forwarder policy contains an invalid or unbounded limit.
    #[error("reserve transaction forwarder policy is invalid")]
    InvalidPolicy,
    /// Finalized policy record/cursor is invalid or internally inconsistent.
    #[error("reserve transaction governance context is invalid")]
    InvalidGovernanceContext,
    /// Finalized provider, movement, appeal, or owner projection is malformed.
    #[error("reserve transaction finalized projection is invalid")]
    InvalidFinalizedProjection,
    /// The supplied projection kind cannot validate this operation kind.
    #[error("reserve transaction projection kind does not match the operation")]
    ProjectionKindMismatch,
    /// Signed bytes are malformed, noncanonical, unsigned, or have the wrong executable.
    #[error("signed reserve transaction is invalid")]
    InvalidSignedTransaction,
    /// Signed transaction belongs to a different chain.
    #[error("signed reserve transaction chain id does not match the active chain")]
    ChainIdMismatch,
    /// Native reserve operation violates a first-release bound.
    #[error("native reserve operation is invalid")]
    InvalidReserveOperation,
    /// Instruction policy digest differs from finalized governance.
    #[error("reserve transaction policy digest does not match finalized governance")]
    PolicyDigestMismatch,
    /// Signed authority is not the exact governed/provider account.
    #[error("reserve transaction authority does not match finalized governance")]
    GovernedAuthorityMismatch,
    /// Provider identifier differs from the finalized reserve account.
    #[error("reserve transaction provider does not match finalized state")]
    ProviderMismatch,
    /// Provider registration owner differs from the finalized provider registry.
    #[error("reserve transaction provider owner does not match finalized registry")]
    ProviderOwnerMismatch,
    /// Provider CAS revision differs from finalized state.
    #[error("reserve transaction provider revision does not match finalized state")]
    ProviderRevisionMismatch,
    /// Canonical encoding failed.
    #[error("reserve transaction canonical encoding failed: {0}")]
    CanonicalEncoding(#[source] norito::Error),
    /// A semantic identity is retained with different bound material.
    #[error("reserve transaction identity conflicts with retained state")]
    IdentityConflict,
    /// The semantic identity already has a terminal dead letter.
    #[error("reserve transaction identity has a terminal dead letter")]
    DeadLetterConflict,
    /// Pending capacity is exhausted.
    #[error("reserve transaction pending capacity is exhausted")]
    PendingCapacityExhausted,
    /// Dead-letter capacity is exhausted.
    #[error("reserve transaction dead-letter capacity is exhausted")]
    DeadLetterCapacityExhausted,
    /// Sequence allocation overflowed.
    #[error("reserve transaction sequence is exhausted")]
    SequenceExhausted,
    /// Worker scan limit is outside the fixed bound.
    #[error("reserve transaction scan limit is invalid")]
    InvalidScanLimit,
    /// Operation is not pending.
    #[error("reserve transaction operation is not pending")]
    UnknownOperation,
    /// State-machine transition is unsafe.
    #[error("reserve transaction transition is invalid")]
    InvalidTransition,
    /// Finalized cursor is zero or did not advance enough to prove absence.
    #[error("reserve transaction finalized cursor is invalid")]
    InvalidFinalizedCursor,
    /// Retry budget is exhausted.
    #[error("reserve transaction retry bound is exhausted")]
    RetryExhausted,
    /// A bounded decode or canonical payload exceeds a resource ceiling.
    #[error("reserve transaction resource limit is exceeded")]
    ResourceLimitExceeded,
    /// Checkpoint is malformed, inconsistent, or noncanonical.
    #[error("reserve transaction checkpoint is invalid")]
    InvalidCheckpoint,
    /// Checkpoint exceeds its configured byte ceiling.
    #[error("reserve transaction checkpoint exceeds its byte limit")]
    CheckpointTooLarge,
    /// Checkpoint path is unsafe or inaccessible.
    #[error("reserve transaction checkpoint I/O failed")]
    CheckpointIo,
    /// Another runtime changed the checkpoint.
    #[error("reserve transaction checkpoint changed concurrently")]
    StaleCheckpoint,
    /// Another writer owns the checkpoint.
    #[error("reserve transaction checkpoint writer is busy")]
    CheckpointBusy,
    /// Rename may be visible but directory durability is unknown.
    #[error("reserve transaction checkpoint durability is uncertain")]
    CheckpointDurabilityUncertain,
    /// Runtime stopped after uncertain durability.
    #[error("reserve transaction checkpoint durability is poisoned")]
    DurabilityPoisoned,
    /// Runtime state mutex is poisoned.
    #[error("reserve transaction runtime lock is poisoned")]
    RuntimePoisoned,
}

impl From<DeliveryTransitionError> for ReserveTransactionForwarderError {
    fn from(error: DeliveryTransitionError) -> Self {
        match error {
            DeliveryTransitionError::InvalidFinalizedCursor => Self::InvalidFinalizedCursor,
            DeliveryTransitionError::InvalidTransition => Self::InvalidTransition,
            DeliveryTransitionError::RetryExhausted => Self::RetryExhausted,
        }
    }
}

impl From<CheckpointStoreError> for ReserveTransactionForwarderError {
    fn from(error: CheckpointStoreError) -> Self {
        match error {
            CheckpointStoreError::Io => Self::CheckpointIo,
            CheckpointStoreError::TooLarge => Self::CheckpointTooLarge,
            CheckpointStoreError::Busy => Self::CheckpointBusy,
            CheckpointStoreError::Stale => Self::StaleCheckpoint,
            CheckpointStoreError::DurabilityUncertain => Self::CheckpointDurabilityUncertain,
            CheckpointStoreError::RuntimePoisoned => Self::RuntimePoisoned,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc, thread, time::Duration};

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        asset::AssetDefinitionId,
        domain::DomainId,
        isi::{InstructionBox, Log},
        sorafs::{
            pin_registry::StorageClass,
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAppealStatusV1, ReserveDuration,
                ReserveLifecycleStage, ReserveMovementKindV1, ReservePolicyV1,
                ReserveProviderTermsV1, ReserveTier,
            },
        },
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_manifest::deal::XorQuantity;
    use tempfile::TempDir;

    use super::*;

    fn forwarder_policy() -> ReserveTransactionForwarderPolicyV1 {
        ReserveTransactionForwarderPolicyV1 {
            max_pending: 32,
            max_completed: 16,
            max_dead_letters: 16,
            max_attempts: 3,
            max_transaction_bytes: 512 * 1024,
            checkpoint_max_bytes: 4 * 1024 * 1024,
        }
    }

    fn key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap()
    }

    fn account(key: &KeyPair) -> AccountId {
        AccountId::new(key.public_key().clone())
    }

    fn provider_id(seed: u8) -> ProviderId {
        ProviderId::new([seed; 32])
    }

    fn cursor(height: u64, hash_byte: u8) -> ReserveFinalizedCursorV1 {
        ReserveFinalizedCursorV1 {
            height,
            block_hash: [hash_byte; 32],
        }
    }

    fn policy_record(
        operations: &KeyPair,
        decision: &KeyPair,
        revision: u64,
        predecessor_policy_digest: Option<[u8; 32]>,
    ) -> ReserveAuthorityPolicyRecordV1 {
        let custody = account(&key(0xC1));
        let treasury = account(&key(0xC2));
        let policy = iroha_data_model::sorafs::reserve::ReserveAuthorityPolicyV1 {
            version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision,
            predecessor_policy_digest,
            economics: ReservePolicyV1::default(),
            asset_definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("reserve", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            custody_account: custody,
            treasury_account: treasury,
            operations_authority: account(operations),
            decision_authority: account(decision),
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: XorQuantity::try_from_micro(1_000_000_000).unwrap(),
            max_pending_movements_per_provider: 8,
            max_open_appeals_per_provider: 4,
        };
        let policy_digest = policy.digest().unwrap();
        ReserveAuthorityPolicyRecordV1 {
            policy,
            policy_digest,
            activated_by: account(operations),
            activated_at_unix: 1,
        }
    }

    fn provider_account(
        provider: &KeyPair,
        policy_digest: [u8; 32],
        revision: u64,
    ) -> ReserveProviderAccountV1 {
        ReserveProviderAccountV1 {
            terms: ReserveProviderTermsV1 {
                provider_id: provider_id(0x51),
                provider_account: account(provider),
                tier: ReserveTier::TierA,
                storage_class: StorageClass::Hot,
                duration: ReserveDuration::Monthly,
                capacity_gib: 64,
            },
            policy_digest,
            revision,
            reserve_balance: XorQuantity::try_from_micro(100_000_000).unwrap(),
            debt_principal: XorQuantity::try_from_micro(10_000_000).unwrap(),
            accrued_interest: XorQuantity::try_from_micro(1_000_000).unwrap(),
            credit_cap: XorQuantity::try_from_micro(100_000_000).unwrap(),
            lifecycle_stage: ReserveLifecycleStage::Warning,
            days_past_due: 2,
            pending_movements: 1,
            open_appeals: 1,
            rent_charged_through_unix: 100,
            interest_accrued_at_unix: 100,
            updated_at_unix: 100,
        }
    }

    fn provider_context(
        operations: &KeyPair,
        decision: &KeyPair,
        provider: &KeyPair,
        revision: u64,
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> ReserveTransactionContextV1 {
        let policy_record = policy_record(operations, decision, 1, None);
        let account = provider_account(provider, policy_record.policy_digest, revision);
        ReserveTransactionContextV1 {
            chain_id: ChainId::from("reserve-transaction-forwarder-test"),
            policy_record,
            projection: ReserveTransactionProjectionV1::Provider { account },
            finalized_cursor,
        }
    }

    fn movement_context(
        operations: &KeyPair,
        decision: &KeyPair,
        provider: &KeyPair,
        revision: u64,
        movement_id: [u8; 32],
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> ReserveTransactionContextV1 {
        let mut context =
            provider_context(operations, decision, provider, revision, finalized_cursor);
        let ReserveTransactionProjectionV1::Provider { account } = context.projection else {
            unreachable!()
        };
        context.projection = ReserveTransactionProjectionV1::MovementDecision {
            movement: ReserveMovementRecordV1 {
                movement_id,
                provider_id: account.terms.provider_id,
                kind: ReserveMovementKindV1::TopUp,
                amount: XorQuantity::try_from_micro(10_000_000).unwrap(),
                requested_by: account.terms.provider_account.clone(),
                expected_provider_revision: revision - 1,
                policy_digest: context.policy_record.policy_digest,
                status: ReserveMovementStatusV1::Pending,
                requested_at_unix: 90,
                decided_by: None,
                decided_at_unix: None,
                rationale: None,
            },
            account,
        };
        context
    }

    fn appeal_context(
        operations: &KeyPair,
        decision: &KeyPair,
        provider: &KeyPair,
        revision: u64,
        appeal_id: [u8; 32],
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> ReserveTransactionContextV1 {
        let mut context =
            provider_context(operations, decision, provider, revision, finalized_cursor);
        let ReserveTransactionProjectionV1::Provider { account } = context.projection else {
            unreachable!()
        };
        context.projection = ReserveTransactionProjectionV1::AppealDecision {
            appeal: ReserveAppealRecordV1 {
                appeal_id,
                provider_id: account.terms.provider_id,
                submitted_by: account.terms.provider_account.clone(),
                requested_stage: ReserveLifecycleStage::Active,
                reason: "review lifecycle evidence".to_owned(),
                evidence_digest: Some([0xA1; 32]),
                expected_provider_revision: revision - 1,
                status: ReserveAppealStatusV1::Pending,
                submitted_at_unix: 90,
                decided_by: None,
                decided_at_unix: None,
                rationale: None,
            },
            account,
        };
        context
    }

    fn registration_context(
        operations: &KeyPair,
        decision: &KeyPair,
        provider: &KeyPair,
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> ReserveTransactionContextV1 {
        ReserveTransactionContextV1 {
            chain_id: ChainId::from("reserve-transaction-forwarder-test"),
            policy_record: policy_record(operations, decision, 1, None),
            projection: ReserveTransactionProjectionV1::Registration {
                provider_owner: account(provider),
            },
            finalized_cursor,
        }
    }

    fn request_operation(context: &ReserveTransactionContextV1) -> ReserveOperationV1 {
        let ReserveTransactionProjectionV1::Provider { account } = &context.projection else {
            panic!("provider context")
        };
        ReserveOperationV1::RequestMovement(RequestSorafsReserveMovement::new(
            [0x61; 32],
            account.terms.provider_id,
            ReserveMovementKindV1::TopUp,
            XorQuantity::try_from_micro(1_000_000).unwrap(),
            account.revision,
            context.policy_record.policy_digest,
        ))
    }

    fn charge_operation(context: &ReserveTransactionContextV1) -> ReserveOperationV1 {
        let ReserveTransactionProjectionV1::Provider { account } = &context.projection else {
            panic!("provider context")
        };
        ReserveOperationV1::ChargeRent(ChargeSorafsReserveRent::new(
            account.terms.provider_id,
            account.revision,
            1,
            context.policy_record.policy_digest,
        ))
    }

    fn signed_bytes(
        signer: &KeyPair,
        authority: AccountId,
        instructions: impl IntoIterator<Item = InstructionBox>,
        creation_time_ms: u64,
    ) -> Vec<u8> {
        signed_bytes_on_chain(
            ChainId::from("reserve-transaction-forwarder-test"),
            signer,
            authority,
            instructions,
            creation_time_ms,
        )
    }

    fn signed_bytes_on_chain(
        chain_id: ChainId,
        signer: &KeyPair,
        authority: AccountId,
        instructions: impl IntoIterator<Item = InstructionBox>,
        creation_time_ms: u64,
    ) -> Vec<u8> {
        let mut builder = TransactionBuilder::new(
            chain_id,
            authority,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions);
        builder.set_creation_time(Duration::from_millis(creation_time_ms));
        let transaction = builder.try_sign(signer.private_key()).unwrap();
        norito::to_bytes(&transaction).unwrap()
    }

    fn signed_operation(
        signer: &KeyPair,
        operation: ReserveOperationV1,
        creation_time_ms: u64,
    ) -> Vec<u8> {
        signed_bytes(
            signer,
            account(signer),
            [operation.into()],
            creation_time_ms,
        )
    }

    #[test]
    fn all_native_kinds_bind_exact_finalized_authorities_and_revisions() {
        let operations = key(1);
        let decision = key(2);
        let provider = key(3);
        let finalized = cursor(10, 0xA1);
        let provider_context = provider_context(&operations, &decision, &provider, 7, finalized);
        let registration_context =
            registration_context(&operations, &decision, &provider, finalized);
        let movement_context =
            movement_context(&operations, &decision, &provider, 7, [0x62; 32], finalized);
        let appeal_context =
            appeal_context(&operations, &decision, &provider, 7, [0x63; 32], finalized);
        let ReserveTransactionProjectionV1::Provider {
            account: provider_state,
        } = &provider_context.projection
        else {
            unreachable!()
        };

        let operations_and_contexts = vec![
            (
                ReserveOperationV1::RegisterProvider(RegisterSorafsReserveAccount::new(
                    provider_state.terms.clone(),
                    registration_context.policy_record.policy_digest,
                )),
                registration_context,
                account(&operations),
            ),
            (
                request_operation(&provider_context),
                provider_context.clone(),
                account(&provider),
            ),
            (
                ReserveOperationV1::DecideMovement(DecideSorafsReserveMovement::new(
                    [0x62; 32],
                    7,
                    movement_context.policy_record.policy_digest,
                    true,
                    "approved".to_owned(),
                )),
                movement_context,
                account(&decision),
            ),
            (
                charge_operation(&provider_context),
                provider_context.clone(),
                account(&operations),
            ),
            (
                ReserveOperationV1::AdvanceLifecycle(AdvanceSorafsReserveLifecycle::new(
                    provider_state.terms.provider_id,
                    7,
                    8,
                    provider_context.policy_record.policy_digest,
                )),
                provider_context.clone(),
                account(&operations),
            ),
            (
                ReserveOperationV1::DrawCredit(DrawSorafsReserveCredit::new(
                    provider_state.terms.provider_id,
                    7,
                    XorQuantity::try_from_micro(1_000_000).unwrap(),
                    provider_context.policy_record.policy_digest,
                )),
                provider_context.clone(),
                account(&operations),
            ),
            (
                ReserveOperationV1::RepayCredit(RepaySorafsReserveCredit::new(
                    provider_state.terms.provider_id,
                    7,
                    XorQuantity::try_from_micro(1_000_000).unwrap(),
                    provider_context.policy_record.policy_digest,
                )),
                provider_context.clone(),
                account(&provider),
            ),
            (
                ReserveOperationV1::SubmitAppeal(SubmitSorafsReserveAppeal::new(
                    [0x64; 32],
                    provider_state.terms.provider_id,
                    7,
                    ReserveLifecycleStage::Active,
                    "review reserve state".to_owned(),
                    Some([0x65; 32]),
                    provider_context.policy_record.policy_digest,
                )),
                provider_context,
                account(&provider),
            ),
            (
                ReserveOperationV1::DecideAppeal(DecideSorafsReserveAppeal::new(
                    [0x63; 32],
                    7,
                    appeal_context.policy_record.policy_digest,
                    false,
                    "evidence insufficient".to_owned(),
                )),
                appeal_context,
                account(&decision),
            ),
        ];
        let forwarder = ReserveTransactionForwarder::in_memory(forwarder_policy()).unwrap();
        for (operation, context, expected_authority) in operations_and_contexts {
            let operation_id = forwarder
                .enqueue_unsigned_operation(operation, &context)
                .unwrap()
                .operation_id();
            let retained = forwarder
                .operation_for_reconciliation(operation_id)
                .unwrap();
            assert_eq!(retained.request.authority, expected_authority);
            assert_eq!(
                retained.policy_record.policy_digest,
                context.policy_record.policy_digest
            );
            assert_eq!(retained.projection, context.projection);
        }
        assert_eq!(forwarder.pending(32).unwrap().len(), 9);
    }

    #[test]
    fn registration_owner_policy_rotation_revision_and_signer_substitution_fail_closed() {
        let operations = key(4);
        let decision = key(5);
        let provider = key(6);
        let attacker = key(7);
        let finalized = cursor(20, 0xA2);
        let context = provider_context(&operations, &decision, &provider, 9, finalized);
        let operation = charge_operation(&context);
        let valid = signed_operation(&operations, operation.clone(), 1);
        let forwarder = ReserveTransactionForwarder::in_memory(forwarder_policy()).unwrap();
        let operation_id = forwarder
            .enqueue_signed_transaction(&valid, &context)
            .unwrap()
            .operation_id();
        assert_eq!(forwarder.begin_submission(operation_id).unwrap(), valid);

        let wrong_signer = signed_operation(&attacker, operation.clone(), 2);
        assert!(matches!(
            ReserveTransactionForwarder::in_memory(forwarder_policy())
                .unwrap()
                .enqueue_signed_transaction(&wrong_signer, &context),
            Err(ReserveTransactionForwarderError::GovernedAuthorityMismatch)
        ));

        assert!("".parse::<ChainId>().is_err());
        assert!(
            "x".repeat(RESERVE_TRANSACTION_MAX_CHAIN_ID_BYTES_V1 + 1)
                .parse::<ChainId>()
                .is_err()
        );

        let mut stale_context = context.clone();
        let ReserveTransactionProjectionV1::Provider {
            account: provider_state,
        } = &mut stale_context.projection
        else {
            unreachable!()
        };
        provider_state.revision += 1;
        assert!(matches!(
            ReserveTransactionForwarder::in_memory(forwarder_policy())
                .unwrap()
                .enqueue_unsigned_operation(operation.clone(), &stale_context),
            Err(ReserveTransactionForwarderError::ProviderRevisionMismatch)
        ));

        let mut rotated = context.clone();
        rotated.policy_record.policy.operations_authority = account(&attacker);
        rotated.policy_record.policy.revision = 2;
        rotated.policy_record.policy.predecessor_policy_digest =
            Some(context.policy_record.policy_digest);
        rotated.policy_record.policy_digest = rotated.policy_record.policy.digest().unwrap();
        assert!(matches!(
            ReserveTransactionForwarder::in_memory(forwarder_policy())
                .unwrap()
                .enqueue_unsigned_operation(operation.clone(), &rotated),
            Err(ReserveTransactionForwarderError::PolicyDigestMismatch)
        ));

        let mut custody_rotated = context.clone();
        custody_rotated.policy_record.policy.custody_account = account(&attacker);
        custody_rotated.policy_record.policy.revision = 2;
        custody_rotated
            .policy_record
            .policy
            .predecessor_policy_digest = Some(context.policy_record.policy_digest);
        custody_rotated.policy_record.policy_digest =
            custody_rotated.policy_record.policy.digest().unwrap();
        let custody_forwarder = ReserveTransactionForwarder::in_memory(forwarder_policy()).unwrap();
        assert!(matches!(
            custody_forwarder.enqueue_unsigned_operation(operation, &custody_rotated),
            Err(ReserveTransactionForwarderError::PolicyDigestMismatch)
        ));
        assert!(
            custody_forwarder.pending(8).unwrap().is_empty(),
            "custody rotation conflict must not publish a pending operation"
        );

        let registration = registration_context(&operations, &decision, &provider, finalized);
        let ReserveTransactionProjectionV1::Registration { provider_owner } =
            &registration.projection
        else {
            unreachable!()
        };
        let bad_registration =
            ReserveOperationV1::RegisterProvider(RegisterSorafsReserveAccount::new(
                ReserveProviderTermsV1 {
                    provider_id: provider_id(0x52),
                    provider_account: account(&attacker),
                    tier: ReserveTier::TierA,
                    storage_class: StorageClass::Hot,
                    duration: ReserveDuration::Monthly,
                    capacity_gib: 1,
                },
                registration.policy_record.policy_digest,
            ));
        assert_ne!(provider_owner, &account(&attacker));
        assert!(matches!(
            ReserveTransactionForwarder::in_memory(forwarder_policy())
                .unwrap()
                .enqueue_unsigned_operation(bad_registration, &registration),
            Err(ReserveTransactionForwarderError::ProviderOwnerMismatch)
        ));
    }

    #[test]
    fn signer_output_is_exact_and_multiple_or_foreign_instructions_are_rejected() {
        let operations = key(8);
        let decision = key(9);
        let provider = key(10);
        let context = provider_context(&operations, &decision, &provider, 4, cursor(30, 0xA3));
        let forwarder = ReserveTransactionForwarder::in_memory(forwarder_policy()).unwrap();
        let operation = charge_operation(&context);
        let operation_id = forwarder
            .enqueue_unsigned_operation(operation.clone(), &context)
            .unwrap()
            .operation_id();
        let request = forwarder.claim_for_signing(operation_id).unwrap();
        assert_eq!(request.chain_id, context.chain_id);

        let substituted = signed_operation(
            &operations,
            ReserveOperationV1::ChargeRent(ChargeSorafsReserveRent::new(
                operation_provider_id(&operation, &context.projection).unwrap(),
                4,
                2,
                context.policy_record.policy_digest,
            )),
            10,
        );
        assert!(matches!(
            forwarder.store_signed_transaction(operation_id, &substituted),
            Err(ReserveTransactionForwarderError::InvalidSignedTransaction)
        ));

        let exact = signed_operation(&operations, request.operation, 11);
        assert_eq!(
            forwarder
                .store_signed_transaction(operation_id, &exact)
                .unwrap(),
            transaction_digest(&exact)
        );
        assert_eq!(forwarder.begin_submission(operation_id).unwrap(), exact);
        let pending = forwarder.pending(1).unwrap().remove(0);
        validate_reserve_pending_delivery_v1(&pending)
            .expect("forwarder snapshot has canonical signed-byte metadata");
        let mut digest_corrupted = pending.clone();
        digest_corrupted.transaction_digest.as_mut().unwrap()[0] ^= 0x80;
        let mut bytes_corrupted = pending;
        bytes_corrupted.signed_transaction_bytes.as_mut().unwrap()[0] ^= 0x80;
        for corrupted in [digest_corrupted, bytes_corrupted] {
            assert!(matches!(
                validate_reserve_pending_delivery_v1(&corrupted),
                Err(ReserveTransactionForwarderError::InvalidCheckpoint)
            ));
        }

        let wrong_chain = signed_bytes_on_chain(
            ChainId::from("foreign-reserve-chain"),
            &operations,
            account(&operations),
            [operation.clone().into()],
            12,
        );
        assert!(matches!(
            ReserveTransactionForwarder::in_memory(forwarder_policy())
                .unwrap()
                .enqueue_signed_transaction(&wrong_chain, &context),
            Err(ReserveTransactionForwarderError::ChainIdMismatch)
        ));

        let multiple = signed_bytes(
            &operations,
            account(&operations),
            [operation.clone().into(), charge_operation(&context).into()],
            13,
        );
        assert!(matches!(
            ReserveTransactionForwarder::in_memory(forwarder_policy())
                .unwrap()
                .enqueue_signed_transaction(&multiple, &context),
            Err(ReserveTransactionForwarderError::InvalidSignedTransaction)
        ));
        let foreign = signed_bytes(
            &operations,
            account(&operations),
            [Log::new(iroha_data_model::Level::INFO, "not reserve".to_owned()).into()],
            14,
        );
        assert!(matches!(
            ReserveTransactionForwarder::in_memory(forwarder_policy())
                .unwrap()
                .enqueue_signed_transaction(&foreign, &context),
            Err(ReserveTransactionForwarderError::InvalidSignedTransaction)
        ));
    }

    #[test]
    fn circular_scan_finalized_replay_conflict_and_absence_are_bounded() {
        let operations = key(11);
        let decision = key(12);
        let provider = key(13);
        let context = provider_context(&operations, &decision, &provider, 5, cursor(40, 0xA4));
        let forwarder = ReserveTransactionForwarder::in_memory(forwarder_policy()).unwrap();
        let mut ids = Vec::new();
        for operation in [
            charge_operation(&context),
            ReserveOperationV1::AdvanceLifecycle(AdvanceSorafsReserveLifecycle::new(
                provider_id(0x51),
                5,
                8,
                context.policy_record.policy_digest,
            )),
            ReserveOperationV1::DrawCredit(DrawSorafsReserveCredit::new(
                provider_id(0x51),
                5,
                XorQuantity::try_from_micro(1_000_000).unwrap(),
                context.policy_record.policy_digest,
            )),
        ] {
            ids.push(
                forwarder
                    .enqueue_unsigned_operation(operation, &context)
                    .unwrap()
                    .operation_id(),
            );
        }
        let mut single_item_cursor = None;
        let mut single_item_visits = Vec::new();
        for _ in 0..6 {
            let page = forwarder.pending_after(single_item_cursor, 1).unwrap();
            let sequence = page
                .first()
                .expect("unchanged pending entry remains visitable")
                .sequence;
            single_item_visits.push(sequence);
            single_item_cursor = Some(sequence);
        }
        assert_eq!(single_item_visits, vec![1, 2, 3, 1, 2, 3]);

        let first_page = forwarder.pending_after(Some(2), 3).unwrap();
        assert_eq!(
            first_page
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            vec![3, 1, 2]
        );

        let request = forwarder.claim_for_signing(ids[0]).unwrap();
        let exact = signed_operation(&operations, request.operation, 20);
        let digest = forwarder.store_signed_transaction(ids[0], &exact).unwrap();
        forwarder.begin_submission(ids[0]).unwrap();
        forwarder.mark_submitted(ids[0]).unwrap();
        forwarder
            .mark_finalized(ids[0], digest, cursor(41, 0xB1))
            .unwrap();
        let replay = ReserveOperationV1::ChargeRent(ChargeSorafsReserveRent::new(
            provider_id(0x51),
            5,
            1,
            context.policy_record.policy_digest,
        ));
        assert!(matches!(
            forwarder
                .enqueue_unsigned_operation(replay, &context)
                .unwrap(),
            ReserveTransactionEnqueueResultV1::Existing { .. }
        ));

        let request = forwarder.claim_for_signing(ids[1]).unwrap();
        let exact = signed_operation(&operations, request.operation, 21);
        forwarder.store_signed_transaction(ids[1], &exact).unwrap();
        forwarder.begin_submission(ids[1]).unwrap();
        forwarder
            .mark_finalized_absent(ids[1], cursor(42, 0xB2))
            .unwrap();
        assert_eq!(
            forwarder
                .pending(8)
                .unwrap()
                .into_iter()
                .find(|entry| entry.operation_id == ids[1])
                .unwrap()
                .state,
            ReserveTransactionDeliveryStateV1::Signed
        );
        forwarder.begin_submission(ids[1]).unwrap();
        forwarder
            .mark_finalized_absent(ids[1], cursor(43, 0xB3))
            .unwrap();
        let third_attempt = forwarder
            .pending(8)
            .unwrap()
            .into_iter()
            .find(|entry| entry.operation_id == ids[1])
            .unwrap();
        assert_eq!(
            third_attempt.state,
            ReserveTransactionDeliveryStateV1::Signed
        );
        assert_eq!(third_attempt.attempts, 3);
        forwarder.begin_submission(ids[1]).unwrap();
        forwarder
            .mark_finalized_absent(ids[1], cursor(44, 0xB4))
            .unwrap();
        assert!(
            forwarder
                .pending(8)
                .unwrap()
                .iter()
                .all(|entry| entry.operation_id != ids[1])
        );
        assert_eq!(
            forwarder.dead_letters(8).unwrap()[0].reason,
            ReserveTransactionDeadLetterReasonV1::RetryExhausted
        );
    }

    #[test]
    fn crash_recovery_preserves_ambiguous_bytes_and_resets_only_signing() {
        let operations = key(14);
        let decision = key(15);
        let provider = key(16);
        let context = provider_context(&operations, &decision, &provider, 6, cursor(50, 0xA5));
        let temp = TempDir::new().unwrap();
        let operation_id = {
            let forwarder =
                ReserveTransactionForwarder::open(temp.path(), forwarder_policy()).unwrap();
            let operation_id = forwarder
                .enqueue_unsigned_operation(charge_operation(&context), &context)
                .unwrap()
                .operation_id();
            forwarder.claim_for_signing(operation_id).unwrap();
            operation_id
        };
        let exact = {
            let recovered =
                ReserveTransactionForwarder::open(temp.path(), forwarder_policy()).unwrap();
            let pending = recovered.pending(1).unwrap();
            assert_eq!(pending[0].state, ReserveTransactionDeliveryStateV1::Ready);
            assert_eq!(pending[0].attempts, 1);
            let request = recovered.claim_for_signing(operation_id).unwrap();
            let exact = signed_operation(&operations, request.operation, 30);
            recovered
                .store_signed_transaction(operation_id, &exact)
                .unwrap();
            recovered.begin_submission(operation_id).unwrap();
            exact
        };
        let recovered = ReserveTransactionForwarder::open(temp.path(), forwarder_policy()).unwrap();
        let pending = recovered.pending(1).unwrap();
        assert_eq!(
            pending[0].state,
            ReserveTransactionDeliveryStateV1::Ambiguous
        );
        assert_eq!(
            pending[0].signed_transaction_bytes.as_deref(),
            Some(exact.as_slice())
        );
    }

    #[test]
    fn corrupt_truncated_oversized_and_poisoned_state_fail_closed() {
        let operations = key(17);
        let decision = key(18);
        let provider = key(19);
        let context = provider_context(&operations, &decision, &provider, 8, cursor(60, 0xA6));
        let temp = TempDir::new().unwrap();
        {
            let forwarder =
                ReserveTransactionForwarder::open(temp.path(), forwarder_policy()).unwrap();
            forwarder
                .enqueue_unsigned_operation(charge_operation(&context), &context)
                .unwrap();
        }
        let path = temp
            .path()
            .join(RESERVE_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1);
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        assert!(matches!(
            ReserveTransactionForwarder::open(temp.path(), forwarder_policy()),
            Err(ReserveTransactionForwarderError::InvalidCheckpoint)
        ));

        let oversized = TempDir::new().unwrap();
        let mut restrictive = forwarder_policy();
        restrictive.checkpoint_max_bytes = 32;
        fs::write(
            oversized
                .path()
                .join(RESERVE_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1),
            vec![0xA5; 33],
        )
        .unwrap();
        assert!(matches!(
            ReserveTransactionForwarder::open(oversized.path(), restrictive),
            Err(ReserveTransactionForwarderError::CheckpointTooLarge)
        ));

        let poisoned = ReserveTransactionForwarder::in_memory(forwarder_policy()).unwrap();
        let state = Arc::clone(&poisoned.state);
        assert!(
            thread::spawn(move || {
                let _guard = state.lock().unwrap();
                panic!("poison reserve forwarder");
            })
            .join()
            .is_err()
        );
        assert!(matches!(
            poisoned.pending(1),
            Err(ReserveTransactionForwarderError::RuntimePoisoned)
        ));
    }
}
