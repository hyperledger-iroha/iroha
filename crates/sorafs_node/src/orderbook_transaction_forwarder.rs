//! Durable forwarding for native SoraFS orderbook transactions.
//!
//! The forwarder persists validated matching, maintenance, and settlement
//! operations without mutating a process-local orderbook. An isolated signer
//! receives the exact configured transaction authority and native instruction, while a
//! separate submitter receives exact canonical signed transaction bytes only
//! after those bytes are durable. Finalized-ledger reconcilers retain sole
//! responsibility for deciding whether an operation committed, remained
//! absent, or conflicted with a newer policy/book revision.

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
            MaintainSorafsOrderbook, MatchSorafsOrderbook, RecordSorafsOrderbookSettlementReceipt,
        },
    },
    sorafs::orderbook::{
        ORDERBOOK_MAX_FILLS_PER_EXECUTION_V1, ORDERBOOK_MAX_MAINTENANCE_ITEMS_V1,
        OrderbookAdmissionPolicyRecord, OrderbookAdmissionPolicyV1, OrderbookFinalizedCursorV1,
    },
    transaction::{Executable, SignedTransaction},
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::orderbook::{
    decode_settlement_receipt_v1, verify_settlement_receipt_signature_v1,
};
use thiserror::Error;

use crate::durable_transaction_forwarder::{
    self as durable, AtomicCheckpointStore, CheckpointStoreError, DeliveryRecord,
    DeliveryTransitionError, FinalizedCursorV1, RetryBoundOutcome, StoredDeliveryStateV1,
};

/// Durable orderbook-transaction checkpoint schema version.
pub const ORDERBOOK_TRANSACTION_FORWARDER_CHECKPOINT_VERSION_V1: u8 = 1;
/// Canonical orderbook-transaction checkpoint file.
pub const ORDERBOOK_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1: &str =
    "orderbook-transaction-forwarder-state.to";
/// Default bounded attempt count for one semantic orderbook operation.
pub const ORDERBOOK_TRANSACTION_FORWARDER_DEFAULT_MAX_ATTEMPTS_V1: u32 = 8;
/// Maximum entries returned by one worker scan.
pub const ORDERBOOK_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1: usize = 1_000;
/// Hard ceiling for one canonical signed orderbook transaction.
pub const ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1: usize = 2 * 1024 * 1024;
/// Maximum active chain identifier retained by the V1 forwarder.
pub const ORDERBOOK_TRANSACTION_MAX_CHAIN_ID_BYTES_V1: usize = 128;

const CHECKPOINT_LOCK_FILE_NAME: &str = "orderbook-transaction-forwarder-state.lock";
const REVISION_IDENTITY_DOMAIN_V1: &[u8] =
    b"sorafs.orderbook.transaction-forwarder.revision-identity.v1\0";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"sorafs.orderbook.transaction-forwarder.operation.v1\0";
const SEMANTIC_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.orderbook.transaction-forwarder.semantic.v1\0";
const CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT: usize = 6;
const CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT: usize = 20;
const CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 1024 * 1024;
const CHECKPOINT_MAX_NESTING_DEPTH: usize = 128;
const TRANSACTION_ELEMENT_AMPLIFICATION_LIMIT: usize = 8;
const TRANSACTION_ALLOCATION_AMPLIFICATION_LIMIT: usize = 20;
const TRANSACTION_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 512 * 1024;
const TRANSACTION_MAX_NESTING_DEPTH: usize = 128;

/// Bounded persistence and retry policy for native orderbook operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderbookTransactionForwarderPolicyV1 {
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

impl OrderbookTransactionForwarderPolicyV1 {
    /// Validate all first-release resource bounds.
    pub fn validate(self) -> Result<(), OrderbookTransactionForwarderError> {
        if self.max_pending == 0
            || self.max_completed == 0
            || self.max_dead_letters == 0
            || self.max_attempts == 0
            || self.max_transaction_bytes == 0
            || self.max_transaction_bytes > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1
            || self.checkpoint_max_bytes == 0
        {
            return Err(OrderbookTransactionForwarderError::InvalidPolicy);
        }
        Ok(())
    }
}

/// Finalized governance/book snapshot used to admit one forwarding operation.
///
/// Callers must obtain the policy record and book revision from the same
/// finalized state view identified by `finalized_cursor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookTransactionContextV1 {
    /// Exact active chain identity for signing and signed-ingress validation.
    pub chain_id: ChainId,
    /// Exact active, governance-authenticated policy record.
    pub policy_record: OrderbookAdmissionPolicyRecord,
    /// Exact authoritative book revision observed with `policy_record`.
    pub book_revision: u64,
    /// Finalized block anchor shared by the policy and status query.
    pub finalized_cursor: OrderbookFinalizedCursorV1,
}

impl OrderbookTransactionContextV1 {
    fn validate(&self) -> Result<(), OrderbookTransactionForwarderError> {
        validate_finalized_cursor(self.finalized_cursor)?;
        if self.chain_id.as_str().is_empty()
            || self.chain_id.as_str().len() > ORDERBOOK_TRANSACTION_MAX_CHAIN_ID_BYTES_V1
        {
            return Err(OrderbookTransactionForwarderError::InvalidGovernanceContext);
        }
        self.policy_record
            .policy
            .validate()
            .map_err(|_| OrderbookTransactionForwarderError::InvalidGovernanceContext)?;
        let digest = self
            .policy_record
            .policy
            .digest()
            .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
        if digest == [0; 32] || digest != self.policy_record.policy_digest {
            return Err(OrderbookTransactionForwarderError::InvalidGovernanceContext);
        }
        Ok(())
    }
}

/// Native orderbook instruction kind retained by the forwarder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookTransactionKindV1 {
    /// Execute one deterministic bounded matching transition.
    Match,
    /// Retire expired orders/channels in one bounded transition.
    Maintain,
    /// Record one canonical provider-signed settlement receipt.
    SettlementReceipt,
}

/// Validated native orderbook operation retained for isolated external signing.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum OrderbookOperationV1 {
    /// Execute one deterministic bounded matching transition.
    Match(MatchSorafsOrderbook),
    /// Retire expired orders/channels in one bounded transition.
    Maintain(MaintainSorafsOrderbook),
    /// Record one canonical provider-signed settlement receipt.
    SettlementReceipt(RecordSorafsOrderbookSettlementReceipt),
}

impl OrderbookOperationV1 {
    /// Return the native orderbook instruction kind.
    #[must_use]
    pub const fn kind(&self) -> OrderbookTransactionKindV1 {
        match self {
            Self::Match(_) => OrderbookTransactionKindV1::Match,
            Self::Maintain(_) => OrderbookTransactionKindV1::Maintain,
            Self::SettlementReceipt(_) => OrderbookTransactionKindV1::SettlementReceipt,
        }
    }

    /// Return the policy digest embedded in the native instruction.
    #[must_use]
    pub fn policy_digest(&self) -> [u8; 32] {
        match self {
            Self::Match(instruction) => *instruction.policy_digest(),
            Self::Maintain(instruction) => *instruction.policy_digest(),
            Self::SettlementReceipt(instruction) => *instruction.policy_digest(),
        }
    }

    /// Return the expected authoritative book revision for CAS operations.
    #[must_use]
    pub fn expected_book_revision(&self) -> Option<u64> {
        match self {
            Self::Match(instruction) => Some(*instruction.expected_book_revision()),
            Self::Maintain(instruction) => Some(*instruction.expected_book_revision()),
            Self::SettlementReceipt(_) => None,
        }
    }
}

impl From<OrderbookOperationV1> for InstructionBox {
    fn from(operation: OrderbookOperationV1) -> Self {
        match operation {
            OrderbookOperationV1::Match(instruction) => instruction.into(),
            OrderbookOperationV1::Maintain(instruction) => instruction.into(),
            OrderbookOperationV1::SettlementReceipt(instruction) => instruction.into(),
        }
    }
}

/// Exact signer work item returned after a durable claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookTransactionSigningRequestV1 {
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Exact active chain identity.
    pub chain_id: ChainId,
    /// Exact matcher authority or explicitly configured receipt relayer.
    pub authority: AccountId,
    /// Exact validated native operation.
    pub operation: OrderbookOperationV1,
}

/// Fail-closed result for a retained operation against current finalized policy/book state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookFinalizedContextValidationV1 {
    /// Retained policy, authority, and revision still match finalized state.
    Ready,
    /// Finalized state validly rotated or consumed the retained precondition.
    Conflict,
    /// Pending and retained checkpoint snapshots do not describe one operation.
    InvalidDurableState,
    /// The supplied finalized policy record is malformed or internally inconsistent.
    InvalidFinalizedContext,
}

/// Validate the payload-free pending snapshot exported by the durable forwarder.
///
/// This owns delivery-state and exact signed-byte digest validation so workers
/// do not reproduce the forwarder's checkpoint invariants.
pub fn validate_orderbook_pending_delivery_v1(
    delivery: &OrderbookTransactionPendingV1,
) -> Result<(), OrderbookTransactionForwarderError> {
    if delivery.sequence == 0
        || delivery.operation_id == [0; 32]
        || delivery.semantic_digest == [0; 32]
        || delivery.policy_digest == [0; 32]
        || delivery.chain_id.as_str().is_empty()
        || delivery.chain_id.as_str().len() > ORDERBOOK_TRANSACTION_MAX_CHAIN_ID_BYTES_V1
        || delivery.baseline_finalized_height == 0
        || delivery.baseline_finalized_block_hash == [0; 32]
    {
        return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
    }
    let signed_material_is_complete =
        delivery
            .signed_transaction_bytes
            .as_ref()
            .is_some_and(|bytes| {
                !bytes.is_empty()
                    && bytes.len() <= ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1
                    && delivery.transaction_digest == Some(transaction_digest(bytes))
            });
    let signed_material_is_absent =
        delivery.signed_transaction_bytes.is_none() && delivery.transaction_digest.is_none();
    let valid_state = match delivery.state {
        OrderbookTransactionDeliveryStateV1::Ready => signed_material_is_absent,
        OrderbookTransactionDeliveryStateV1::Signing => {
            signed_material_is_absent && delivery.attempts != 0
        }
        OrderbookTransactionDeliveryStateV1::Signed
        | OrderbookTransactionDeliveryStateV1::Ambiguous
        | OrderbookTransactionDeliveryStateV1::Submitted => {
            signed_material_is_complete && delivery.attempts != 0
        }
    };
    let valid_revision_shape = match delivery.kind {
        OrderbookTransactionKindV1::Match | OrderbookTransactionKindV1::Maintain => {
            delivery.expected_book_revision.is_some()
        }
        OrderbookTransactionKindV1::SettlementReceipt => delivery.expected_book_revision.is_none(),
    };
    if !valid_state || !valid_revision_shape {
        return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
    }
    Ok(())
}

/// Validate that exported pending and signing snapshots retain one canonical operation.
///
/// Stable identity digests and operation identifiers are recomputed with the
/// forwarder's private canonical domains and helpers. This is the sole public
/// validation boundary; Torii must not duplicate those algorithms.
pub fn validate_orderbook_reconciliation_material_v1(
    delivery: &OrderbookTransactionPendingV1,
    retained: &OrderbookTransactionSigningRequestV1,
) -> Result<(), OrderbookTransactionForwarderError> {
    validate_orderbook_pending_delivery_v1(delivery)?;
    if retained.operation_id != delivery.operation_id
        || retained.chain_id != delivery.chain_id
        || retained.authority != delivery.authority
        || retained.operation.kind() != delivery.kind
        || retained.operation.policy_digest() != delivery.policy_digest
        || retained.operation.expected_book_revision() != delivery.expected_book_revision
    {
        return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
    }
    validate_reconciliation_operation_shape(&retained.operation)?;
    let (identity_scope, identity_digest) = operation_identity(&retained.operation)
        .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
    let semantic_digest =
        semantic_digest(&retained.chain_id, &retained.authority, &retained.operation)
            .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
    if identity_digest == [0; 32]
        || semantic_digest != delivery.semantic_digest
        || operation_id_from_parts(identity_scope, identity_digest, semantic_digest)
            != delivery.operation_id
    {
        return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
    }
    Ok(())
}

/// Validate retained material against one coherent finalized policy/book snapshot.
///
/// Governed matcher authority selection and revision CAS remain private
/// implementation details of this forwarder. Provider-signed settlement
/// receipts are relayable, so their outer transaction authority is retained
/// for audit/idempotency but is not compared with the custody release
/// authority.
#[must_use]
pub fn validate_orderbook_finalized_context_v1(
    delivery: &OrderbookTransactionPendingV1,
    retained: &OrderbookTransactionSigningRequestV1,
    policy_record: &OrderbookAdmissionPolicyRecord,
    authoritative_book_revision: u64,
) -> OrderbookFinalizedContextValidationV1 {
    if validate_orderbook_reconciliation_material_v1(delivery, retained).is_err() {
        return OrderbookFinalizedContextValidationV1::InvalidDurableState;
    }
    if policy_record.activated_at_unix == 0
        || policy_record.policy_digest == [0; 32]
        || policy_record.policy.validate().is_err()
        || !policy_record
            .policy
            .digest()
            .is_ok_and(|digest| digest == policy_record.policy_digest)
    {
        return OrderbookFinalizedContextValidationV1::InvalidFinalizedContext;
    }
    if policy_record.policy_digest != delivery.policy_digest {
        return OrderbookFinalizedContextValidationV1::Conflict;
    }
    match validate_operation(
        &retained.operation,
        &retained.authority,
        &policy_record.policy,
        authoritative_book_revision,
    ) {
        Ok(()) => OrderbookFinalizedContextValidationV1::Ready,
        Err(
            OrderbookTransactionForwarderError::PolicyDigestMismatch
            | OrderbookTransactionForwarderError::GovernedAuthorityMismatch
            | OrderbookTransactionForwarderError::BookRevisionMismatch,
        ) => OrderbookFinalizedContextValidationV1::Conflict,
        Err(OrderbookTransactionForwarderError::InvalidGovernanceContext) => {
            OrderbookFinalizedContextValidationV1::InvalidFinalizedContext
        }
        Err(_) => OrderbookFinalizedContextValidationV1::InvalidDurableState,
    }
}

/// Durable enqueue result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookTransactionEnqueueResultV1 {
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

impl OrderbookTransactionEnqueueResultV1 {
    /// Return the stable semantic operation identity.
    #[must_use]
    pub const fn operation_id(self) -> [u8; 32] {
        match self {
            Self::Inserted { operation_id } | Self::Existing { operation_id } => operation_id,
        }
    }
}

/// Runtime-visible crash state for one orderbook transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookTransactionDeliveryStateV1 {
    /// Validated semantic material is ready for external signing.
    Ready,
    /// An external signer claim is in progress.
    Signing,
    /// Exact signed bytes are durable and ready for submission.
    Signed,
    /// Submission may have happened and must be reconciled.
    Ambiguous,
    /// The exact transaction is known pending or applied.
    Submitted,
}

impl From<StoredDeliveryStateV1> for OrderbookTransactionDeliveryStateV1 {
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

/// Exact pending delivery returned to a bounded worker scan.
#[derive(Debug, Clone)]
pub struct OrderbookTransactionPendingV1 {
    /// Insertion sequence.
    pub sequence: u64,
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Native orderbook instruction kind.
    pub kind: OrderbookTransactionKindV1,
    /// Exact active chain identity.
    pub chain_id: ChainId,
    /// Exact matcher authority or explicitly configured receipt relayer.
    pub authority: AccountId,
    /// Active policy digest bound by the operation.
    pub policy_digest: [u8; 32],
    /// Expected book revision for match/maintenance.
    pub expected_book_revision: Option<u64>,
    /// Digest of the semantic authority/instruction pair.
    pub semantic_digest: [u8; 32],
    /// Digest of the retained exact transaction bytes.
    pub transaction_digest: Option<[u8; 32]>,
    /// Current durable crash state.
    pub state: OrderbookTransactionDeliveryStateV1,
    /// Attempts consumed by this semantic operation.
    pub attempts: u32,
    /// Finalized height preceding the current attempt.
    pub baseline_finalized_height: u64,
    /// Finalized hash paired with the baseline height.
    pub baseline_finalized_block_hash: [u8; 32],
    /// Exact canonical signed transaction bytes, absent before signing.
    pub signed_transaction_bytes: Option<Vec<u8>>,
}

/// Payload-free terminal reason retained for operator reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookTransactionDeadLetterReasonV1 {
    /// Finalized policy/book state conflicts with the semantic operation.
    FinalizedConflict,
    /// The exact transaction was rejected or expired terminally.
    TransactionRejected,
    /// Bounded retries were exhausted after finalized absence.
    RetryExhausted,
}

/// Payload-free terminal orderbook delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookTransactionDeadLetterV1 {
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Native orderbook instruction kind.
    pub kind: OrderbookTransactionKindV1,
    /// Exact active chain identity.
    pub chain_id: ChainId,
    /// Active policy digest bound by the operation.
    pub policy_digest: [u8; 32],
    /// Expected book revision for match/maintenance.
    pub expected_book_revision: Option<u64>,
    /// Digest of the semantic authority/instruction pair.
    pub semantic_digest: [u8; 32],
    /// Digest of the final exact transaction bytes, if one existed.
    pub transaction_digest: Option<[u8; 32]>,
    /// Terminal reason.
    pub reason: OrderbookTransactionDeadLetterReasonV1,
    /// Finalized height observing the terminal condition.
    pub observed_finalized_height: u64,
    /// Finalized hash paired with the observed height.
    pub observed_finalized_block_hash: [u8; 32],
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
enum StoredOrderbookIdentityScopeV1 {
    MatchRevision,
    MaintenanceRevision,
    SettlementReceipt,
}

impl StoredOrderbookIdentityScopeV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::MatchRevision => 0,
            Self::MaintenanceRevision => 1,
            Self::SettlementReceipt => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredDeadLetterReasonV1 {
    FinalizedConflict,
    TransactionRejected,
    RetryExhausted,
}

impl From<StoredDeadLetterReasonV1> for OrderbookTransactionDeadLetterReasonV1 {
    fn from(value: StoredDeadLetterReasonV1) -> Self {
        match value {
            StoredDeadLetterReasonV1::FinalizedConflict => Self::FinalizedConflict,
            StoredDeadLetterReasonV1::TransactionRejected => Self::TransactionRejected,
            StoredDeadLetterReasonV1::RetryExhausted => Self::RetryExhausted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredPendingOrderbookTransactionV1 {
    sequence: u64,
    operation_id: [u8; 32],
    identity_scope: StoredOrderbookIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    chain_id: ChainId,
    authority: AccountId,
    governed_policy: OrderbookAdmissionPolicyV1,
    operation: OrderbookOperationV1,
    state: StoredDeliveryStateV1,
    attempts: u32,
    baseline_finalized_height: u64,
    baseline_finalized_block_hash: [u8; 32],
    signed_transaction_bytes: Option<Vec<u8>>,
}

impl StoredPendingOrderbookTransactionV1 {
    fn snapshot(&self) -> OrderbookTransactionPendingV1 {
        OrderbookTransactionPendingV1 {
            sequence: self.sequence,
            operation_id: self.operation_id,
            kind: self.operation.kind(),
            chain_id: self.chain_id.clone(),
            authority: self.authority.clone(),
            policy_digest: self.operation.policy_digest(),
            expected_book_revision: self.operation.expected_book_revision(),
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

impl DeliveryRecord for StoredPendingOrderbookTransactionV1 {
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
struct StoredCompletedOrderbookTransactionV1 {
    operation_id: [u8; 32],
    identity_scope: StoredOrderbookIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredDeadOrderbookTransactionV1 {
    operation_id: [u8; 32],
    identity_scope: StoredOrderbookIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    chain_id: ChainId,
    authority: AccountId,
    governed_policy: OrderbookAdmissionPolicyV1,
    operation: OrderbookOperationV1,
    signed_transaction_bytes: Option<Vec<u8>>,
    reason: StoredDeadLetterReasonV1,
    observed_finalized_height: u64,
    observed_finalized_block_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct OrderbookTransactionForwarderCheckpointV1 {
    version: u8,
    next_sequence: u64,
    pending: Vec<StoredPendingOrderbookTransactionV1>,
    completed: Vec<StoredCompletedOrderbookTransactionV1>,
    dead_letters: Vec<StoredDeadOrderbookTransactionV1>,
}

impl Default for OrderbookTransactionForwarderCheckpointV1 {
    fn default() -> Self {
        Self {
            version: ORDERBOOK_TRANSACTION_FORWARDER_CHECKPOINT_VERSION_V1,
            next_sequence: 1,
            pending: Vec::new(),
            completed: Vec::new(),
            dead_letters: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct DurableState {
    checkpoint: OrderbookTransactionForwarderCheckpointV1,
    fingerprint: Option<[u8; 32]>,
    durability_failure: bool,
}

/// Durable bounded native orderbook transaction forwarder.
#[derive(Debug, Clone)]
pub struct OrderbookTransactionForwarder {
    policy: OrderbookTransactionForwarderPolicyV1,
    state: Arc<Mutex<DurableState>>,
    store: Option<Arc<AtomicCheckpointStore>>,
}

impl OrderbookTransactionForwarder {
    /// Construct a non-persistent forwarder for focused composition tests.
    pub fn in_memory(
        policy: OrderbookTransactionForwarderPolicyV1,
    ) -> Result<Self, OrderbookTransactionForwarderError> {
        policy.validate()?;
        Ok(Self {
            policy,
            state: Arc::new(Mutex::new(DurableState {
                checkpoint: OrderbookTransactionForwarderCheckpointV1::default(),
                fingerprint: None,
                durability_failure: false,
            })),
            store: None,
        })
    }

    /// Open or create a durable forwarder below `state_dir`.
    pub fn open(
        state_dir: &Path,
        policy: OrderbookTransactionForwarderPolicyV1,
    ) -> Result<Self, OrderbookTransactionForwarderError> {
        policy.validate()?;
        let store = Arc::new(AtomicCheckpointStore::new(
            state_dir,
            ORDERBOOK_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1,
            CHECKPOINT_LOCK_FILE_NAME,
            policy.checkpoint_max_bytes,
        )?);
        let (bytes, fingerprint) = store.load_bytes()?;
        let mut checkpoint = match bytes {
            Some(bytes) => decode_checkpoint(&bytes, policy)?,
            None => OrderbookTransactionForwarderCheckpointV1::default(),
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

    /// Validate and durably accept one unsigned matcher/maintenance operation.
    ///
    /// Settlement receipts require [`Self::enqueue_unsigned_operation_with_authority`]
    /// so an explicitly configured relayer is never silently replaced with the
    /// governed custody authority.
    pub fn enqueue_unsigned_operation(
        &self,
        operation: OrderbookOperationV1,
        context: &OrderbookTransactionContextV1,
    ) -> Result<OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarderError> {
        context.validate()?;
        if operation.kind() == OrderbookTransactionKindV1::SettlementReceipt {
            return Err(OrderbookTransactionForwarderError::ExplicitRelayerAuthorityRequired);
        }
        let prepared = PreparedOrderbookOperation::from_unsigned(
            operation,
            context,
            self.policy.max_transaction_bytes,
        )?;
        self.enqueue_prepared(prepared, None, context.finalized_cursor)
    }

    /// Validate and durably accept one unsigned operation for an explicit signer.
    ///
    /// Match and maintenance still require the active governed matcher.
    /// Settlement receipts accept any explicitly configured relayer account;
    /// the canonical provider signature remains the delivery authorization and
    /// ledger execution uses the channel's immutable custody authority.
    pub fn enqueue_unsigned_operation_with_authority(
        &self,
        authority: AccountId,
        operation: OrderbookOperationV1,
        context: &OrderbookTransactionContextV1,
    ) -> Result<OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarderError> {
        context.validate()?;
        let prepared = PreparedOrderbookOperation::new_bounded(
            context.chain_id.clone(),
            authority,
            context.policy_record.policy.clone(),
            operation,
            context.book_revision,
            self.policy.max_transaction_bytes,
        )?;
        self.enqueue_prepared(prepared, None, context.finalized_cursor)
    }

    /// Validate and durably accept one exact canonical signed transaction.
    ///
    /// Match and maintenance signatures must use the exact governed matcher.
    /// A settlement receipt may be wrapped by any valid transaction signer
    /// because the canonical receipt carries the provider authorization. Every
    /// signed transaction must bind the exact active chain retained in the
    /// finalized context.
    pub fn enqueue_signed_transaction(
        &self,
        signed_transaction_bytes: &[u8],
        context: &OrderbookTransactionContextV1,
    ) -> Result<OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarderError> {
        context.validate()?;
        let prepared = PreparedOrderbookOperation::decode_signed_transaction(
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
        prepared: PreparedOrderbookOperation,
        signed_transaction_bytes: Option<&[u8]>,
        baseline_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarderError> {
        let operation_id = operation_id(&prepared);
        let mut state = self.lock_state()?;
        if let Some(existing) = state.checkpoint.pending.iter().find(|entry| {
            entry.identity_scope == prepared.identity_scope
                && entry.identity_digest == prepared.identity_digest
        }) {
            if existing.operation_id == operation_id
                && existing.semantic_digest == prepared.semantic_digest
            {
                return Ok(OrderbookTransactionEnqueueResultV1::Existing { operation_id });
            }
            return Err(OrderbookTransactionForwarderError::IdentityConflict);
        }
        if let Some(existing) = state.checkpoint.completed.iter().find(|entry| {
            entry.identity_scope == prepared.identity_scope
                && entry.identity_digest == prepared.identity_digest
        }) {
            if existing.operation_id == operation_id
                && existing.semantic_digest == prepared.semantic_digest
            {
                return Ok(OrderbookTransactionEnqueueResultV1::Existing { operation_id });
            }
            return Err(OrderbookTransactionForwarderError::IdentityConflict);
        }
        if state.checkpoint.dead_letters.iter().any(|entry| {
            entry.identity_scope == prepared.identity_scope
                && entry.identity_digest == prepared.identity_digest
        }) {
            return Err(OrderbookTransactionForwarderError::DeadLetterConflict);
        }
        if state.checkpoint.pending.len() >= self.policy.max_pending {
            return Err(OrderbookTransactionForwarderError::PendingCapacityExhausted);
        }
        let sequence = state.checkpoint.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(OrderbookTransactionForwarderError::SequenceExhausted)?;
        let mut candidate = state.checkpoint.clone();
        candidate.next_sequence = next_sequence;
        candidate.pending.push(StoredPendingOrderbookTransactionV1 {
            sequence,
            operation_id,
            identity_scope: prepared.identity_scope,
            identity_digest: prepared.identity_digest,
            semantic_digest: prepared.semantic_digest,
            chain_id: prepared.chain_id,
            authority: prepared.authority,
            governed_policy: prepared.governed_policy,
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
        Ok(OrderbookTransactionEnqueueResultV1::Inserted { operation_id })
    }

    /// Return pending entries in stable sequence order.
    pub fn pending(
        &self,
        limit: usize,
    ) -> Result<Vec<OrderbookTransactionPendingV1>, OrderbookTransactionForwarderError> {
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
    ) -> Result<Vec<OrderbookTransactionPendingV1>, OrderbookTransactionForwarderError> {
        if limit == 0 || limit > ORDERBOOK_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1 {
            return Err(OrderbookTransactionForwarderError::InvalidScanLimit);
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
            .map(StoredPendingOrderbookTransactionV1::snapshot)
            .collect())
    }

    /// Return exact semantic material for finalized-ledger reconciliation.
    ///
    /// This accessor is non-mutating in every crash state. A reconciler can
    /// compare the returned policy digest/revision or receipt identity against
    /// finalized projections before choosing one of the terminal transitions.
    pub fn operation_for_reconciliation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<OrderbookTransactionSigningRequestV1, OrderbookTransactionForwarderError> {
        let state = self.lock_state()?;
        let entry = state
            .checkpoint
            .pending
            .iter()
            .find(|entry| entry.operation_id == operation_id)
            .ok_or(OrderbookTransactionForwarderError::UnknownOperation)?;
        Ok(OrderbookTransactionSigningRequestV1 {
            operation_id: entry.operation_id,
            chain_id: entry.chain_id.clone(),
            authority: entry.authority.clone(),
            operation: entry.operation.clone(),
        })
    }

    /// Return payload-free dead letters in stable operation order.
    pub fn dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<OrderbookTransactionDeadLetterV1>, OrderbookTransactionForwarderError> {
        if limit == 0 || limit > ORDERBOOK_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1 {
            return Err(OrderbookTransactionForwarderError::InvalidScanLimit);
        }
        let state = self.lock_state()?;
        Ok(state
            .checkpoint
            .dead_letters
            .iter()
            .take(limit)
            .map(|entry| OrderbookTransactionDeadLetterV1 {
                operation_id: entry.operation_id,
                kind: entry.operation.kind(),
                chain_id: entry.chain_id.clone(),
                policy_digest: entry.operation.policy_digest(),
                expected_book_revision: entry.operation.expected_book_revision(),
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
    /// The attempt is consumed before the request is exposed. A crash resets
    /// only the signer-only state and never refunds the attempt. Claiming an
    /// exhausted entry after crash recovery atomically dead-letters it.
    pub fn claim_for_signing(
        &self,
        operation_id: [u8; 32],
    ) -> Result<OrderbookTransactionSigningRequestV1, OrderbookTransactionForwarderError> {
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
            return Err(OrderbookTransactionForwarderError::RetryExhausted);
        }
        let entry = &mut candidate.pending[position];
        claim_for_signing(entry, self.policy.max_attempts)?;
        let request = OrderbookTransactionSigningRequestV1 {
            operation_id: entry.operation_id,
            chain_id: entry.chain_id.clone(),
            authority: entry.authority.clone(),
            operation: entry.operation.clone(),
        };
        self.commit_candidate(&mut state, candidate)?;
        Ok(request)
    }

    /// Persist exact signed bytes for a claimed authority/operation.
    pub fn store_signed_transaction(
        &self,
        expected_operation_id: [u8; 32],
        signed_transaction_bytes: &[u8],
    ) -> Result<[u8; 32], OrderbookTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let entry = find_pending_mut(&mut candidate, expected_operation_id)?;
        let context = context_for_stored_entry(entry)?;
        let prepared = PreparedOrderbookOperation::decode_signed_transaction(
            signed_transaction_bytes,
            &context,
            self.policy.max_transaction_bytes,
        )?;
        if prepared.identity_scope != entry.identity_scope
            || prepared.identity_digest != entry.identity_digest
            || prepared.semantic_digest != entry.semantic_digest
            || prepared.chain_id != entry.chain_id
            || prepared.authority != entry.authority
            || prepared.governed_policy != entry.governed_policy
            || prepared.operation != entry.operation
            || operation_id(&prepared) != entry.operation_id
        {
            return Err(OrderbookTransactionForwarderError::InvalidSignedTransaction);
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
    ) -> Result<(), OrderbookTransactionForwarderError> {
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
    ) -> Result<Vec<u8>, OrderbookTransactionForwarderError> {
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
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.mutate_entry(operation_id, |entry| {
            durable::mark_submitted(entry).map_err(Into::into)
        })
    }

    /// Record a failure proven to have happened before queue submission.
    pub fn mark_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.mutate_entry(operation_id, |entry| {
            durable::mark_not_submitted(entry).map_err(Into::into)
        })
    }

    /// Retry the same exact bytes only after finalized absence is proven.
    pub fn mark_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
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
        finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        validate_finalized_cursor(finalized_cursor)?;
        if expected_transaction_digest == [0; 32] {
            return Err(OrderbookTransactionForwarderError::InvalidSignedTransaction);
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
            return Err(OrderbookTransactionForwarderError::InvalidSignedTransaction);
        }
        self.commit_finalized_operation(&mut state, candidate, position, finalized_cursor)
    }

    /// Reconcile exact semantic success committed through another ingress.
    ///
    /// The caller must first compare `operation_for_reconciliation` with a
    /// finalized ledger projection. This supports cross-peer duplicate
    /// convergence before local signing has completed.
    pub fn mark_semantic_finalized(
        &self,
        operation_id: [u8; 32],
        finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        validate_finalized_cursor(finalized_cursor)?;
        let mut state = self.lock_state()?;
        let candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        self.commit_finalized_operation(&mut state, candidate, position, finalized_cursor)
    }

    fn commit_finalized_operation(
        &self,
        state: &mut DurableState,
        mut candidate: OrderbookTransactionForwarderCheckpointV1,
        position: usize,
        finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        if candidate.completed.len() >= self.policy.max_completed {
            let oldest = candidate
                .completed
                .iter()
                .enumerate()
                .min_by_key(|(_, completed)| (completed.finalized_height, completed.operation_id))
                .map(|(index, _)| index)
                .ok_or(OrderbookTransactionForwarderError::InvalidCheckpoint)?;
            candidate.completed.remove(oldest);
        }
        let entry = candidate.pending.remove(position);
        candidate
            .completed
            .push(StoredCompletedOrderbookTransactionV1 {
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
        observed_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
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
        observed_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
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
            return Err(OrderbookTransactionForwarderError::InvalidTransition);
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
            &mut StoredPendingOrderbookTransactionV1,
        ) -> Result<(), OrderbookTransactionForwarderError>,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        mutate(find_pending_mut(&mut candidate, operation_id)?)?;
        self.commit_candidate(&mut state, candidate)
    }

    fn move_to_dead_letter(
        &self,
        checkpoint: &mut OrderbookTransactionForwarderCheckpointV1,
        position: usize,
        reason: StoredDeadLetterReasonV1,
        cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        validate_finalized_cursor(cursor)?;
        if checkpoint.dead_letters.len() >= self.policy.max_dead_letters {
            return Err(OrderbookTransactionForwarderError::DeadLetterCapacityExhausted);
        }
        let entry = checkpoint.pending.remove(position);
        checkpoint
            .dead_letters
            .push(StoredDeadOrderbookTransactionV1 {
                operation_id: entry.operation_id,
                identity_scope: entry.identity_scope,
                identity_digest: entry.identity_digest,
                semantic_digest: entry.semantic_digest,
                chain_id: entry.chain_id,
                authority: entry.authority,
                governed_policy: entry.governed_policy,
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
    ) -> Result<std::sync::MutexGuard<'_, DurableState>, OrderbookTransactionForwarderError> {
        let state = self
            .state
            .lock()
            .map_err(|_| OrderbookTransactionForwarderError::RuntimePoisoned)?;
        if state.durability_failure {
            return Err(OrderbookTransactionForwarderError::DurabilityPoisoned);
        }
        Ok(state)
    }

    fn commit_candidate(
        &self,
        state: &mut DurableState,
        candidate: OrderbookTransactionForwarderCheckpointV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        validate_checkpoint(&candidate, self.policy)?;
        if let Some(store) = self.store.as_ref() {
            let bytes = norito::to_bytes(&candidate)
                .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
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
struct PreparedOrderbookOperation {
    identity_scope: StoredOrderbookIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    chain_id: ChainId,
    authority: AccountId,
    governed_policy: OrderbookAdmissionPolicyV1,
    operation: OrderbookOperationV1,
}

impl PreparedOrderbookOperation {
    fn from_unsigned(
        operation: OrderbookOperationV1,
        context: &OrderbookTransactionContextV1,
        max_transaction_bytes: usize,
    ) -> Result<Self, OrderbookTransactionForwarderError> {
        let authority =
            required_governed_authority(&context.policy_record.policy, operation.kind())
                .ok_or(OrderbookTransactionForwarderError::ExplicitRelayerAuthorityRequired)?
                .clone();
        Self::new_bounded(
            context.chain_id.clone(),
            authority,
            context.policy_record.policy.clone(),
            operation,
            context.book_revision,
            max_transaction_bytes,
        )
    }

    fn new(
        chain_id: ChainId,
        authority: AccountId,
        governed_policy: OrderbookAdmissionPolicyV1,
        operation: OrderbookOperationV1,
        authoritative_book_revision: u64,
    ) -> Result<Self, OrderbookTransactionForwarderError> {
        if chain_id.as_str().is_empty()
            || chain_id.as_str().len() > ORDERBOOK_TRANSACTION_MAX_CHAIN_ID_BYTES_V1
        {
            return Err(OrderbookTransactionForwarderError::InvalidGovernanceContext);
        }
        validate_operation(
            &operation,
            &authority,
            &governed_policy,
            authoritative_book_revision,
        )?;
        let (identity_scope, identity_digest) = operation_identity(&operation)?;
        if identity_digest == [0; 32] {
            return Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation);
        }
        let semantic_digest = semantic_digest(&chain_id, &authority, &operation)?;
        Ok(Self {
            identity_scope,
            identity_digest,
            semantic_digest,
            chain_id,
            authority,
            governed_policy,
            operation,
        })
    }

    fn new_bounded(
        chain_id: ChainId,
        authority: AccountId,
        governed_policy: OrderbookAdmissionPolicyV1,
        operation: OrderbookOperationV1,
        authoritative_book_revision: u64,
        max_transaction_bytes: usize,
    ) -> Result<Self, OrderbookTransactionForwarderError> {
        let prepared = Self::new(
            chain_id,
            authority,
            governed_policy,
            operation,
            authoritative_book_revision,
        )?;
        let authority_bytes = norito::to_bytes(&prepared.authority)
            .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
        let chain_id_bytes = norito::to_bytes(&prepared.chain_id)
            .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
        let policy_bytes = norito::to_bytes(&prepared.governed_policy)
            .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
        let operation_bytes = norito::to_bytes(&prepared.operation)
            .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
        if chain_id_bytes
            .len()
            .checked_add(authority_bytes.len())
            .and_then(|length| length.checked_add(policy_bytes.len()))
            .and_then(|length| length.checked_add(operation_bytes.len()))
            .is_none_or(|length| length > max_transaction_bytes)
        {
            return Err(OrderbookTransactionForwarderError::ResourceLimitExceeded);
        }
        Ok(prepared)
    }

    fn decode_signed_transaction(
        bytes: &[u8],
        context: &OrderbookTransactionContextV1,
        max_transaction_bytes: usize,
    ) -> Result<Self, OrderbookTransactionForwarderError> {
        if bytes.is_empty()
            || bytes.len() > max_transaction_bytes
            || bytes.len() > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1
        {
            return Err(OrderbookTransactionForwarderError::InvalidSignedTransaction);
        }
        norito::core::from_bytes_view(bytes)
            .map_err(|_| OrderbookTransactionForwarderError::InvalidSignedTransaction)?;
        let transaction = norito::decode_from_bytes_with_limits::<SignedTransaction>(
            bytes,
            transaction_decode_limits(bytes.len(), max_transaction_bytes)?,
        )
        .map_err(|_| OrderbookTransactionForwarderError::InvalidSignedTransaction)?;
        if norito::to_bytes(&transaction)
            .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?
            != bytes
            || transaction.verify_signature().is_err()
        {
            return Err(OrderbookTransactionForwarderError::InvalidSignedTransaction);
        }
        if transaction.chain() != &context.chain_id {
            return Err(OrderbookTransactionForwarderError::ChainIdMismatch);
        }
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(OrderbookTransactionForwarderError::InvalidSignedTransaction);
        };
        if instructions.len() != 1 {
            return Err(OrderbookTransactionForwarderError::InvalidSignedTransaction);
        }
        let instruction = &instructions[0];
        let operation = if let Some(instruction) =
            instruction.as_any().downcast_ref::<MatchSorafsOrderbook>()
        {
            OrderbookOperationV1::Match(instruction.clone())
        } else if let Some(instruction) = instruction
            .as_any()
            .downcast_ref::<MaintainSorafsOrderbook>()
        {
            OrderbookOperationV1::Maintain(instruction.clone())
        } else if let Some(instruction) = instruction
            .as_any()
            .downcast_ref::<RecordSorafsOrderbookSettlementReceipt>()
        {
            OrderbookOperationV1::SettlementReceipt(instruction.clone())
        } else {
            return Err(OrderbookTransactionForwarderError::InvalidSignedTransaction);
        };
        Self::new_bounded(
            context.chain_id.clone(),
            transaction.authority().clone(),
            context.policy_record.policy.clone(),
            operation,
            context.book_revision,
            max_transaction_bytes,
        )
    }
}

fn required_governed_authority(
    policy: &OrderbookAdmissionPolicyV1,
    kind: OrderbookTransactionKindV1,
) -> Option<&AccountId> {
    match kind {
        OrderbookTransactionKindV1::Match | OrderbookTransactionKindV1::Maintain => {
            Some(&policy.matcher_authority)
        }
        OrderbookTransactionKindV1::SettlementReceipt => None,
    }
}

fn validate_operation(
    operation: &OrderbookOperationV1,
    authority: &AccountId,
    governed_policy: &OrderbookAdmissionPolicyV1,
    authoritative_book_revision: u64,
) -> Result<(), OrderbookTransactionForwarderError> {
    governed_policy
        .validate()
        .map_err(|_| OrderbookTransactionForwarderError::InvalidGovernanceContext)?;
    let policy_digest = governed_policy
        .digest()
        .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
    if operation.policy_digest() != policy_digest {
        return Err(OrderbookTransactionForwarderError::PolicyDigestMismatch);
    }
    if required_governed_authority(governed_policy, operation.kind())
        .is_some_and(|required| authority != required)
    {
        return Err(OrderbookTransactionForwarderError::GovernedAuthorityMismatch);
    }
    match operation {
        OrderbookOperationV1::Match(instruction) => {
            if *instruction.expected_book_revision() != authoritative_book_revision {
                return Err(OrderbookTransactionForwarderError::BookRevisionMismatch);
            }
            if !(1..=ORDERBOOK_MAX_FILLS_PER_EXECUTION_V1).contains(instruction.max_fills()) {
                return Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation);
            }
        }
        OrderbookOperationV1::Maintain(instruction) => {
            if *instruction.expected_book_revision() != authoritative_book_revision {
                return Err(OrderbookTransactionForwarderError::BookRevisionMismatch);
            }
            if !(1..=ORDERBOOK_MAX_MAINTENANCE_ITEMS_V1).contains(instruction.max_items()) {
                return Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation);
            }
        }
        OrderbookOperationV1::SettlementReceipt(instruction) => {
            let receipt = decode_settlement_receipt_v1(instruction.receipt_payload())
                .map_err(|_| OrderbookTransactionForwarderError::InvalidOrderbookOperation)?;
            if receipt.bytes_delivered > governed_policy.max_receipt_bytes {
                return Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation);
            }
            verify_settlement_receipt_signature_v1(&receipt)
                .map_err(|_| OrderbookTransactionForwarderError::InvalidOrderbookOperation)?;
        }
    }
    Ok(())
}

fn validate_reconciliation_operation_shape(
    operation: &OrderbookOperationV1,
) -> Result<(), OrderbookTransactionForwarderError> {
    if operation.policy_digest() == [0; 32] {
        return Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation);
    }
    let operation_bytes = norito::to_bytes(operation)
        .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
    if operation_bytes.len() > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1 {
        return Err(OrderbookTransactionForwarderError::ResourceLimitExceeded);
    }
    match operation {
        OrderbookOperationV1::Match(instruction) => {
            if !(1..=ORDERBOOK_MAX_FILLS_PER_EXECUTION_V1).contains(instruction.max_fills()) {
                return Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation);
            }
        }
        OrderbookOperationV1::Maintain(instruction) => {
            if !(1..=ORDERBOOK_MAX_MAINTENANCE_ITEMS_V1).contains(instruction.max_items()) {
                return Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation);
            }
        }
        OrderbookOperationV1::SettlementReceipt(instruction) => {
            if instruction.receipt_payload().is_empty()
                || instruction.receipt_payload().len()
                    > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1
            {
                return Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation);
            }
            let receipt = decode_settlement_receipt_v1(instruction.receipt_payload())
                .map_err(|_| OrderbookTransactionForwarderError::InvalidOrderbookOperation)?;
            verify_settlement_receipt_signature_v1(&receipt)
                .map_err(|_| OrderbookTransactionForwarderError::InvalidOrderbookOperation)?;
        }
    }
    Ok(())
}

fn operation_identity(
    operation: &OrderbookOperationV1,
) -> Result<(StoredOrderbookIdentityScopeV1, [u8; 32]), OrderbookTransactionForwarderError> {
    match operation {
        OrderbookOperationV1::Match(instruction) => Ok((
            StoredOrderbookIdentityScopeV1::MatchRevision,
            revision_identity_digest(
                *instruction.policy_digest(),
                *instruction.expected_book_revision(),
            ),
        )),
        OrderbookOperationV1::Maintain(instruction) => Ok((
            StoredOrderbookIdentityScopeV1::MaintenanceRevision,
            revision_identity_digest(
                *instruction.policy_digest(),
                *instruction.expected_book_revision(),
            ),
        )),
        OrderbookOperationV1::SettlementReceipt(instruction) => {
            let receipt = decode_settlement_receipt_v1(instruction.receipt_payload())
                .map_err(|_| OrderbookTransactionForwarderError::InvalidOrderbookOperation)?;
            Ok((
                StoredOrderbookIdentityScopeV1::SettlementReceipt,
                receipt.receipt_id,
            ))
        }
    }
}

fn revision_identity_digest(policy_digest: [u8; 32], expected_book_revision: u64) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(REVISION_IDENTITY_DOMAIN_V1);
    hasher.update(&policy_digest);
    hasher.update(&expected_book_revision.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn semantic_digest(
    chain_id: &ChainId,
    authority: &AccountId,
    operation: &OrderbookOperationV1,
) -> Result<[u8; 32], OrderbookTransactionForwarderError> {
    let chain_id = chain_id.as_str();
    // A provider-signed receipt has one semantic identity regardless of which
    // valid relayer wraps it. Matcher operations remain authority-bound.
    let authority = if operation.kind() == OrderbookTransactionKindV1::SettlementReceipt {
        String::new()
    } else {
        authority.to_string()
    };
    let operation = norito::to_bytes(operation)
        .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
    let chain_id_len = u64::try_from(chain_id.len())
        .map_err(|_| OrderbookTransactionForwarderError::InvalidOrderbookOperation)?;
    let authority_len = u64::try_from(authority.len())
        .map_err(|_| OrderbookTransactionForwarderError::InvalidOrderbookOperation)?;
    let operation_len = u64::try_from(operation.len())
        .map_err(|_| OrderbookTransactionForwarderError::InvalidOrderbookOperation)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(SEMANTIC_DIGEST_DOMAIN_V1);
    hasher.update(&chain_id_len.to_le_bytes());
    hasher.update(chain_id.as_bytes());
    hasher.update(&authority_len.to_le_bytes());
    hasher.update(authority.as_bytes());
    hasher.update(&operation_len.to_le_bytes());
    hasher.update(&operation);
    Ok(*hasher.finalize().as_bytes())
}

fn operation_id(prepared: &PreparedOrderbookOperation) -> [u8; 32] {
    operation_id_from_parts(
        prepared.identity_scope,
        prepared.identity_digest,
        prepared.semantic_digest,
    )
}

fn operation_id_from_parts(
    identity_scope: StoredOrderbookIdentityScopeV1,
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

fn validate_orderbook_delivery(
    entry: &StoredPendingOrderbookTransactionV1,
    max_attempts: u32,
) -> bool {
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

fn recover_interrupted_signing(entry: &mut StoredPendingOrderbookTransactionV1) -> bool {
    if entry.state != StoredDeliveryStateV1::Signing {
        return false;
    }
    entry.state = StoredDeliveryStateV1::Ready;
    true
}

fn claim_for_signing(
    entry: &mut StoredPendingOrderbookTransactionV1,
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
    entry: &mut StoredPendingOrderbookTransactionV1,
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
    entry: &mut StoredPendingOrderbookTransactionV1,
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
    entry: &StoredPendingOrderbookTransactionV1,
) -> Result<OrderbookFinalizedCursorV1, OrderbookTransactionForwarderError> {
    let cursor = OrderbookFinalizedCursorV1 {
        height: entry.baseline_finalized_height,
        block_hash: entry.baseline_finalized_block_hash,
    };
    validate_finalized_cursor(cursor)?;
    Ok(cursor)
}

fn context_for_stored_entry(
    entry: &StoredPendingOrderbookTransactionV1,
) -> Result<OrderbookTransactionContextV1, OrderbookTransactionForwarderError> {
    let policy_digest = entry
        .governed_policy
        .digest()
        .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?;
    Ok(OrderbookTransactionContextV1 {
        chain_id: entry.chain_id.clone(),
        policy_record: OrderbookAdmissionPolicyRecord {
            policy: entry.governed_policy.clone(),
            policy_digest,
            activated_at_unix: 1,
            activated_by: entry.authority.clone(),
        },
        book_revision: entry.operation.expected_book_revision().unwrap_or(0),
        finalized_cursor: OrderbookFinalizedCursorV1 {
            height: entry.baseline_finalized_height,
            block_hash: entry.baseline_finalized_block_hash,
        },
    })
}

fn find_pending_mut(
    checkpoint: &mut OrderbookTransactionForwarderCheckpointV1,
    operation_id: [u8; 32],
) -> Result<&mut StoredPendingOrderbookTransactionV1, OrderbookTransactionForwarderError> {
    checkpoint
        .pending
        .iter_mut()
        .find(|entry| entry.operation_id == operation_id)
        .ok_or(OrderbookTransactionForwarderError::UnknownOperation)
}

fn pending_position(
    checkpoint: &OrderbookTransactionForwarderCheckpointV1,
    operation_id: [u8; 32],
) -> Result<usize, OrderbookTransactionForwarderError> {
    checkpoint
        .pending
        .iter()
        .position(|entry| entry.operation_id == operation_id)
        .ok_or(OrderbookTransactionForwarderError::UnknownOperation)
}

fn validate_checkpoint(
    checkpoint: &OrderbookTransactionForwarderCheckpointV1,
    policy: OrderbookTransactionForwarderPolicyV1,
) -> Result<(), OrderbookTransactionForwarderError> {
    if checkpoint.version != ORDERBOOK_TRANSACTION_FORWARDER_CHECKPOINT_VERSION_V1
        || checkpoint.next_sequence == 0
        || checkpoint.pending.len() > policy.max_pending
        || checkpoint.completed.len() > policy.max_completed
        || checkpoint.dead_letters.len() > policy.max_dead_letters
    {
        return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
    }
    let mut identities = BTreeSet::new();
    let mut operations = BTreeSet::new();
    let mut previous_sequence = 0_u64;
    for entry in &checkpoint.pending {
        let prepared = PreparedOrderbookOperation::new_bounded(
            entry.chain_id.clone(),
            entry.authority.clone(),
            entry.governed_policy.clone(),
            entry.operation.clone(),
            entry.operation.expected_book_revision().unwrap_or(0),
            policy.max_transaction_bytes,
        )
        .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
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
            || !validate_orderbook_delivery(entry, policy.max_attempts)
            || !identities.insert((entry.identity_scope, entry.identity_digest))
            || !operations.insert(entry.operation_id)
        {
            return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
        }
        if let Some(bytes) = entry.signed_transaction_bytes.as_deref() {
            let context = context_for_stored_entry(entry)
                .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
            let decoded = PreparedOrderbookOperation::decode_signed_transaction(
                bytes,
                &context,
                policy.max_transaction_bytes,
            )
            .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
            if decoded.identity_scope != entry.identity_scope
                || decoded.identity_digest != entry.identity_digest
                || decoded.semantic_digest != entry.semantic_digest
                || decoded.chain_id != entry.chain_id
                || decoded.authority != entry.authority
                || decoded.governed_policy != entry.governed_policy
                || decoded.operation != entry.operation
            {
                return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
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
            return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
        }
    }
    for entry in &checkpoint.dead_letters {
        let prepared = PreparedOrderbookOperation::new_bounded(
            entry.chain_id.clone(),
            entry.authority.clone(),
            entry.governed_policy.clone(),
            entry.operation.clone(),
            entry.operation.expected_book_revision().unwrap_or(0),
            policy.max_transaction_bytes,
        )
        .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
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
            return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
        }
        if let Some(bytes) = entry.signed_transaction_bytes.as_deref() {
            let synthetic_entry = StoredPendingOrderbookTransactionV1 {
                sequence: 1,
                operation_id: entry.operation_id,
                identity_scope: entry.identity_scope,
                identity_digest: entry.identity_digest,
                semantic_digest: entry.semantic_digest,
                chain_id: entry.chain_id.clone(),
                authority: entry.authority.clone(),
                governed_policy: entry.governed_policy.clone(),
                operation: entry.operation.clone(),
                state: StoredDeliveryStateV1::Signed,
                attempts: 1,
                baseline_finalized_height: entry.observed_finalized_height,
                baseline_finalized_block_hash: entry.observed_finalized_block_hash,
                signed_transaction_bytes: Some(bytes.to_vec()),
            };
            let context = context_for_stored_entry(&synthetic_entry)
                .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
            let decoded = PreparedOrderbookOperation::decode_signed_transaction(
                bytes,
                &context,
                policy.max_transaction_bytes,
            )
            .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
            if decoded.identity_scope != entry.identity_scope
                || decoded.identity_digest != entry.identity_digest
                || decoded.semantic_digest != entry.semantic_digest
                || decoded.chain_id != entry.chain_id
                || decoded.authority != entry.authority
                || decoded.governed_policy != entry.governed_policy
                || decoded.operation != entry.operation
            {
                return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
            }
        }
    }
    Ok(())
}

fn decode_checkpoint(
    bytes: &[u8],
    policy: OrderbookTransactionForwarderPolicyV1,
) -> Result<OrderbookTransactionForwarderCheckpointV1, OrderbookTransactionForwarderError> {
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > policy.checkpoint_max_bytes
    {
        return Err(OrderbookTransactionForwarderError::CheckpointTooLarge);
    }
    norito::core::from_bytes_view(bytes)
        .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
    let checkpoint = norito::decode_from_bytes_with_limits::<
        OrderbookTransactionForwarderCheckpointV1,
    >(bytes, checkpoint_decode_limits(bytes.len())?)
    .map_err(|_| OrderbookTransactionForwarderError::InvalidCheckpoint)?;
    if norito::to_bytes(&checkpoint)
        .map_err(OrderbookTransactionForwarderError::CanonicalEncoding)?
        != bytes
    {
        return Err(OrderbookTransactionForwarderError::InvalidCheckpoint);
    }
    validate_checkpoint(&checkpoint, policy)?;
    Ok(checkpoint)
}

fn validate_finalized_cursor(
    cursor: OrderbookFinalizedCursorV1,
) -> Result<(), OrderbookTransactionForwarderError> {
    durable::validate_finalized_cursor(finalized_cursor(cursor)).map_err(Into::into)
}

const fn finalized_cursor(cursor: OrderbookFinalizedCursorV1) -> FinalizedCursorV1 {
    FinalizedCursorV1 {
        height: cursor.height,
        block_hash: cursor.block_hash,
    }
}

fn checkpoint_decode_limits(
    encoded_bytes: usize,
) -> Result<norito::DecodeLimits, OrderbookTransactionForwarderError> {
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
) -> Result<norito::DecodeLimits, OrderbookTransactionForwarderError> {
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
) -> Result<norito::DecodeLimits, OrderbookTransactionForwarderError> {
    if encoded_bytes == 0 || encoded_bytes > max_bytes {
        return Err(OrderbookTransactionForwarderError::ResourceLimitExceeded);
    }
    let total_elements = encoded_bytes
        .checked_mul(element_amplification)
        .ok_or(OrderbookTransactionForwarderError::ResourceLimitExceeded)?;
    let total_allocated_bytes = encoded_bytes
        .checked_mul(allocation_amplification)
        .and_then(|budget| budget.checked_add(fixed_allocation))
        .ok_or(OrderbookTransactionForwarderError::ResourceLimitExceeded)?;
    Ok(norito::DecodeLimits::new(
        max_bytes,
        max_bytes,
        total_elements,
        total_allocated_bytes,
        max_depth,
    ))
}

/// Durable native orderbook transaction forwarding error.
#[derive(Debug, Error)]
pub enum OrderbookTransactionForwarderError {
    /// Forwarder policy contains an invalid or unbounded limit.
    #[error("orderbook transaction forwarder policy is invalid")]
    InvalidPolicy,
    /// Finalized policy record/cursor is invalid or internally inconsistent.
    #[error("orderbook transaction governance context is invalid")]
    InvalidGovernanceContext,
    /// Signed bytes are malformed, noncanonical, unsigned, or have the wrong executable.
    #[error("signed orderbook transaction is invalid")]
    InvalidSignedTransaction,
    /// Signed transaction belongs to a different chain.
    #[error("signed orderbook transaction chain id does not match the active chain")]
    ChainIdMismatch,
    /// Native orderbook instruction or embedded receipt is invalid.
    #[error("native orderbook operation is invalid")]
    InvalidOrderbookOperation,
    /// Unsigned settlement generation omitted an explicit relayer signer.
    #[error("unsigned settlement receipt requires an explicit relayer authority")]
    ExplicitRelayerAuthorityRequired,
    /// Instruction policy digest differs from the finalized governed policy.
    #[error("orderbook transaction policy digest does not match finalized governance")]
    PolicyDigestMismatch,
    /// Signed authority is not the exact governed matcher account.
    #[error("orderbook transaction authority does not match finalized governance")]
    GovernedAuthorityMismatch,
    /// Match/maintenance expected revision differs from finalized book state.
    #[error("orderbook transaction expected book revision does not match finalized state")]
    BookRevisionMismatch,
    /// Canonical encoding failed.
    #[error("orderbook transaction canonical encoding failed: {0}")]
    CanonicalEncoding(#[source] norito::Error),
    /// A semantic identity is retained with different authority/instruction material.
    #[error("orderbook transaction identity conflicts with retained state")]
    IdentityConflict,
    /// The semantic identity already has a terminal dead letter.
    #[error("orderbook transaction identity has a terminal dead letter")]
    DeadLetterConflict,
    /// Pending capacity is exhausted.
    #[error("orderbook transaction pending capacity is exhausted")]
    PendingCapacityExhausted,
    /// Dead-letter capacity is exhausted.
    #[error("orderbook transaction dead-letter capacity is exhausted")]
    DeadLetterCapacityExhausted,
    /// Sequence allocation overflowed.
    #[error("orderbook transaction sequence is exhausted")]
    SequenceExhausted,
    /// Worker scan limit is outside the fixed bound.
    #[error("orderbook transaction scan limit is invalid")]
    InvalidScanLimit,
    /// Operation is not pending.
    #[error("orderbook transaction operation is not pending")]
    UnknownOperation,
    /// State-machine transition is unsafe.
    #[error("orderbook transaction transition is invalid")]
    InvalidTransition,
    /// Finalized cursor is zero or did not advance enough to prove absence.
    #[error("orderbook transaction finalized cursor is invalid")]
    InvalidFinalizedCursor,
    /// Retry budget is exhausted.
    #[error("orderbook transaction retry bound is exhausted")]
    RetryExhausted,
    /// A bounded decode or canonical payload exceeds a resource ceiling.
    #[error("orderbook transaction resource limit is exceeded")]
    ResourceLimitExceeded,
    /// Checkpoint is malformed, inconsistent, or noncanonical.
    #[error("orderbook transaction checkpoint is invalid")]
    InvalidCheckpoint,
    /// Checkpoint exceeds its configured byte ceiling.
    #[error("orderbook transaction checkpoint exceeds its byte limit")]
    CheckpointTooLarge,
    /// Checkpoint path is unsafe or inaccessible.
    #[error("orderbook transaction checkpoint I/O failed")]
    CheckpointIo,
    /// Another runtime changed the checkpoint.
    #[error("orderbook transaction checkpoint changed concurrently")]
    StaleCheckpoint,
    /// Another writer owns the checkpoint.
    #[error("orderbook transaction checkpoint writer is busy")]
    CheckpointBusy,
    /// Rename may be visible but directory durability is unknown.
    #[error("orderbook transaction checkpoint durability is uncertain")]
    CheckpointDurabilityUncertain,
    /// Runtime stopped after uncertain durability.
    #[error("orderbook transaction checkpoint durability is poisoned")]
    DurabilityPoisoned,
    /// Runtime state mutex is poisoned.
    #[error("orderbook transaction runtime lock is poisoned")]
    RuntimePoisoned,
}

impl From<DeliveryTransitionError> for OrderbookTransactionForwarderError {
    fn from(error: DeliveryTransitionError) -> Self {
        match error {
            DeliveryTransitionError::InvalidFinalizedCursor => Self::InvalidFinalizedCursor,
            DeliveryTransitionError::InvalidTransition => Self::InvalidTransition,
            DeliveryTransitionError::RetryExhausted => Self::RetryExhausted,
        }
    }
}

impl From<CheckpointStoreError> for OrderbookTransactionForwarderError {
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

    use ed25519_dalek::SigningKey;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        isi::{InstructionBox, Log},
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_manifest::{
        deal::XorQuantity,
        orderbook::{
            ByteRangeV1, OrderbookSignatureV1, SETTLEMENT_RECEIPT_VERSION_V1, SettlementReceiptV1,
            sign_settlement_receipt_ed25519_v1,
        },
        provider_advert::SignatureAlgorithm,
    };
    use tempfile::TempDir;

    use super::*;

    fn policy() -> OrderbookTransactionForwarderPolicyV1 {
        OrderbookTransactionForwarderPolicyV1 {
            max_pending: 8,
            max_completed: 8,
            max_dead_letters: 8,
            max_attempts: 2,
            max_transaction_bytes: 512 * 1024,
            checkpoint_max_bytes: 4 * 1024 * 1024,
        }
    }

    fn key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap()
    }

    fn cursor(height: u64, hash_byte: u8) -> OrderbookFinalizedCursorV1 {
        OrderbookFinalizedCursorV1 {
            height,
            block_hash: [hash_byte; 32],
        }
    }

    fn context(
        matcher: &KeyPair,
        settlement: &KeyPair,
        book_revision: u64,
        finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> OrderbookTransactionContextV1 {
        let policy = OrderbookAdmissionPolicyV1 {
            version: 1,
            revision: 1,
            predecessor_policy_digest: None,
            market_id: [0x41; 32],
            matcher_authority: AccountId::new(matcher.public_key().clone()),
            settlement_authority: AccountId::new(settlement.public_key().clone()),
            paused: false,
            min_order_gib: 1,
            max_order_gib: 1_024,
            price_tick_micro_xor: 1,
            max_maker_fee_bps: 100,
            max_taker_fee_bps: 100,
            max_order_lifetime_secs: 86_400,
            max_receipt_age_secs: 3_600,
            max_clock_skew_secs: 30,
            max_receipt_bytes: 1 << 30,
            max_receipts_per_channel: 128,
        };
        let policy_digest = policy.digest().unwrap();
        OrderbookTransactionContextV1 {
            chain_id: ChainId::from("orderbook-transaction-forwarder-test"),
            policy_record: OrderbookAdmissionPolicyRecord {
                policy,
                policy_digest,
                activated_at_unix: 1,
                activated_by: AccountId::new(matcher.public_key().clone()),
            },
            book_revision,
            finalized_cursor,
        }
    }

    fn match_operation(context: &OrderbookTransactionContextV1) -> OrderbookOperationV1 {
        OrderbookOperationV1::Match(MatchSorafsOrderbook::new(
            context.policy_record.policy_digest,
            context.book_revision,
            8,
        ))
    }

    fn maintain_operation(context: &OrderbookTransactionContextV1) -> OrderbookOperationV1 {
        OrderbookOperationV1::Maintain(MaintainSorafsOrderbook::new(
            context.policy_record.policy_digest,
            context.book_revision,
            16,
        ))
    }

    fn settlement_operation(
        context: &OrderbookTransactionContextV1,
        receipt_id: [u8; 32],
    ) -> OrderbookOperationV1 {
        let signing_key = SigningKey::from_bytes(&[0x51; 32]);
        let receipt = SettlementReceiptV1 {
            version: SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id,
            channel_id: [0x52; 32],
            trade_id: [0x53; 32],
            range: ByteRangeV1 { start: 0, end: 32 },
            chunk_hash: [0x54; 32],
            bytes_delivered: 32,
            xor_debited: XorQuantity::try_from_micro(100).unwrap(),
            provider_credit: XorQuantity::try_from_micro(90).unwrap(),
            fee_amount: XorQuantity::try_from_micro(10).unwrap(),
            issued_at_unix: 10,
            settlement_signature: OrderbookSignatureV1 {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![1; 32],
                signature: vec![1; 64],
            },
        };
        let receipt = sign_settlement_receipt_ed25519_v1(receipt, &signing_key).unwrap();
        OrderbookOperationV1::SettlementReceipt(RecordSorafsOrderbookSettlementReceipt::new(
            norito::to_bytes(&receipt).unwrap(),
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
            ChainId::from("orderbook-transaction-forwarder-test"),
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
        operation: OrderbookOperationV1,
        creation_time_ms: u64,
    ) -> Vec<u8> {
        signed_bytes(
            signer,
            AccountId::new(signer.public_key().clone()),
            [operation.into()],
            creation_time_ms,
        )
    }

    #[test]
    fn unsigned_operations_bind_matcher_and_require_explicit_receipt_relayer() {
        let matcher = key(1);
        let settlement = key(2);
        let relayer = key(3);
        let relayer_id = AccountId::new(relayer.public_key().clone());
        let context = context(&matcher, &settlement, 7, cursor(10, 0xA1));
        let forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();

        let match_id = forwarder
            .enqueue_unsigned_operation(match_operation(&context), &context)
            .unwrap()
            .operation_id();
        let maintain_id = forwarder
            .enqueue_unsigned_operation(maintain_operation(&context), &context)
            .unwrap()
            .operation_id();
        let receipt_operation = settlement_operation(&context, [0x61; 32]);
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(receipt_operation.clone(), &context),
            Err(OrderbookTransactionForwarderError::ExplicitRelayerAuthorityRequired)
        ));
        let receipt_id = forwarder
            .enqueue_unsigned_operation_with_authority(
                relayer_id.clone(),
                receipt_operation,
                &context,
            )
            .unwrap()
            .operation_id();

        assert_eq!(
            forwarder
                .operation_for_reconciliation(match_id)
                .unwrap()
                .authority,
            context.policy_record.policy.matcher_authority
        );
        assert_eq!(
            forwarder
                .operation_for_reconciliation(maintain_id)
                .unwrap()
                .authority,
            context.policy_record.policy.matcher_authority
        );
        assert_eq!(
            forwarder
                .operation_for_reconciliation(receipt_id)
                .unwrap()
                .authority,
            relayer_id
        );

        let stale = OrderbookOperationV1::Match(MatchSorafsOrderbook::new(
            context.policy_record.policy_digest,
            6,
            1,
        ));
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(stale, &context),
            Err(OrderbookTransactionForwarderError::BookRevisionMismatch)
        ));
        let wrong_policy =
            OrderbookOperationV1::Maintain(MaintainSorafsOrderbook::new([0x99; 32], 7, 1));
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(wrong_policy, &context),
            Err(OrderbookTransactionForwarderError::PolicyDigestMismatch)
        ));
    }

    #[test]
    fn settlement_receipt_exceeding_governed_byte_bound_is_rejected_before_enqueue() {
        let matcher = key(0x21);
        let settlement = key(0x22);
        let mut context = context(&matcher, &settlement, 7, cursor(10, 0xA1));
        context.policy_record.policy.max_receipt_bytes = 16;
        context.policy_record.policy_digest = context
            .policy_record
            .policy
            .digest()
            .expect("policy digest");
        let operation = settlement_operation(&context, [0x62; 32]);

        assert!(matches!(
            OrderbookTransactionForwarder::in_memory(policy())
                .expect("forwarder")
                .enqueue_unsigned_operation_with_authority(
                    AccountId::new(key(0x23).public_key().clone()),
                    operation,
                    &context,
                ),
            Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation)
        ));
    }

    #[test]
    fn signed_transactions_require_exact_governed_authority_and_single_native_instruction() {
        let matcher = key(3);
        let settlement = key(4);
        let attacker = key(5);
        let context = context(&matcher, &settlement, 9, cursor(10, 0xA2));
        let operation = match_operation(&context);
        let valid = signed_operation(&matcher, operation.clone(), 1);
        let forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();
        let operation_id = forwarder
            .enqueue_signed_transaction(&valid, &context)
            .unwrap()
            .operation_id();
        assert_eq!(forwarder.begin_submission(operation_id).unwrap(), valid);

        let receipt_operation = settlement_operation(&context, [0x63; 32]);
        let relayed_receipt = signed_operation(&attacker, receipt_operation.clone(), 2);
        let receipt_forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();
        let relayed_id = receipt_forwarder
            .enqueue_signed_transaction(&relayed_receipt, &context)
            .expect("unrelated relayer may wrap a provider-signed receipt")
            .operation_id();
        assert_eq!(
            receipt_forwarder.begin_submission(relayed_id).unwrap(),
            relayed_receipt
        );
        let governed_wrapper = signed_operation(&settlement, receipt_operation, 3);
        assert!(matches!(
            receipt_forwarder
                .enqueue_signed_transaction(&governed_wrapper, &context)
                .expect("same provider receipt deduplicates across relayers"),
            OrderbookTransactionEnqueueResultV1::Existing { operation_id }
                if operation_id == relayed_id
        ));

        let wrong_authority = signed_operation(&attacker, operation.clone(), 4);
        assert!(matches!(
            OrderbookTransactionForwarder::in_memory(policy())
                .unwrap()
                .enqueue_signed_transaction(&wrong_authority, &context),
            Err(OrderbookTransactionForwarderError::GovernedAuthorityMismatch)
        ));

        let wrong_chain = signed_bytes_on_chain(
            ChainId::from("foreign-orderbook-chain"),
            &matcher,
            AccountId::new(matcher.public_key().clone()),
            [operation.clone().into()],
            5,
        );
        assert!(matches!(
            OrderbookTransactionForwarder::in_memory(policy())
                .unwrap()
                .enqueue_signed_transaction(&wrong_chain, &context),
            Err(OrderbookTransactionForwarderError::ChainIdMismatch)
        ));

        let multiple = signed_bytes(
            &matcher,
            AccountId::new(matcher.public_key().clone()),
            [
                operation.clone().into(),
                maintain_operation(&context).into(),
            ],
            6,
        );
        assert!(matches!(
            OrderbookTransactionForwarder::in_memory(policy())
                .unwrap()
                .enqueue_signed_transaction(&multiple, &context),
            Err(OrderbookTransactionForwarderError::InvalidSignedTransaction)
        ));

        let wrong_instruction = signed_bytes(
            &matcher,
            AccountId::new(matcher.public_key().clone()),
            [Log::new(iroha_data_model::Level::INFO, "not orderbook".to_owned()).into()],
            7,
        );
        assert!(matches!(
            OrderbookTransactionForwarder::in_memory(policy())
                .unwrap()
                .enqueue_signed_transaction(&wrong_instruction, &context),
            Err(OrderbookTransactionForwarderError::InvalidSignedTransaction)
        ));

        let mut malformed = signed_operation(&matcher, operation, 8);
        malformed.push(0);
        assert!(matches!(
            OrderbookTransactionForwarder::in_memory(policy())
                .unwrap()
                .enqueue_signed_transaction(&malformed, &context),
            Err(OrderbookTransactionForwarderError::InvalidSignedTransaction)
        ));
    }

    #[test]
    fn signer_output_cannot_substitute_authority_or_cas_instruction() {
        let matcher = key(21);
        let settlement = key(22);
        let attacker = key(23);
        let context = context(&matcher, &settlement, 10, cursor(11, 0xB1));
        let forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();
        let operation_id = forwarder
            .enqueue_unsigned_operation(match_operation(&context), &context)
            .unwrap()
            .operation_id();
        let request = forwarder.claim_for_signing(operation_id).unwrap();

        let wrong_authority = signed_operation(&attacker, request.operation.clone(), 10);
        assert!(matches!(
            forwarder.store_signed_transaction(operation_id, &wrong_authority),
            Err(OrderbookTransactionForwarderError::GovernedAuthorityMismatch)
        ));
        let substituted = signed_operation(
            &matcher,
            OrderbookOperationV1::Match(MatchSorafsOrderbook::new(
                context.policy_record.policy_digest,
                context.book_revision,
                1,
            )),
            11,
        );
        assert!(matches!(
            forwarder.store_signed_transaction(operation_id, &substituted),
            Err(OrderbookTransactionForwarderError::InvalidSignedTransaction)
        ));

        let exact = signed_operation(&matcher, request.operation, 12);
        assert_eq!(
            forwarder
                .store_signed_transaction(operation_id, &exact)
                .unwrap(),
            transaction_digest(&exact)
        );
        assert_eq!(forwarder.begin_submission(operation_id).unwrap(), exact);
    }

    #[test]
    fn revision_and_receipt_identities_are_idempotent_and_conflict_safe() {
        let matcher = key(6);
        let settlement = key(7);
        let context = context(&matcher, &settlement, 11, cursor(20, 0xA3));
        let forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();
        let operation = match_operation(&context);
        let inserted = forwarder
            .enqueue_unsigned_operation(operation.clone(), &context)
            .unwrap();
        assert!(matches!(
            forwarder
                .enqueue_unsigned_operation(operation, &context)
                .unwrap(),
            OrderbookTransactionEnqueueResultV1::Existing { operation_id }
                if operation_id == inserted.operation_id()
        ));
        let conflicting = OrderbookOperationV1::Match(MatchSorafsOrderbook::new(
            context.policy_record.policy_digest,
            context.book_revision,
            1,
        ));
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(conflicting, &context),
            Err(OrderbookTransactionForwarderError::IdentityConflict)
        ));

        let receipt_forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();
        let relayer = AccountId::new(key(0x31).public_key().clone());
        let first = settlement_operation(&context, [0x71; 32]);
        receipt_forwarder
            .enqueue_unsigned_operation_with_authority(relayer.clone(), first, &context)
            .unwrap();
        let mut second = settlement_operation(&context, [0x71; 32]);
        if let OrderbookOperationV1::SettlementReceipt(instruction) = &second {
            let policy_digest = *instruction.policy_digest();
            let mut receipt = decode_settlement_receipt_v1(instruction.receipt_payload()).unwrap();
            receipt.chunk_hash = [0x72; 32];
            receipt =
                sign_settlement_receipt_ed25519_v1(receipt, &SigningKey::from_bytes(&[0x51; 32]))
                    .unwrap();
            second = OrderbookOperationV1::SettlementReceipt(
                RecordSorafsOrderbookSettlementReceipt::new(
                    norito::to_bytes(&receipt).unwrap(),
                    policy_digest,
                ),
            );
        }
        assert!(matches!(
            receipt_forwarder.enqueue_unsigned_operation_with_authority(relayer, second, &context),
            Err(OrderbookTransactionForwarderError::IdentityConflict)
        ));
    }

    #[test]
    fn circular_scan_is_fair_and_bounded() {
        let matcher = key(8);
        let settlement = key(9);
        let forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();
        for revision in 1..=3 {
            let context = context(&matcher, &settlement, revision, cursor(20, 0xA4));
            forwarder
                .enqueue_unsigned_operation(match_operation(&context), &context)
                .unwrap();
        }
        let all = forwarder.pending(8).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(
            forwarder
                .pending_after(Some(all[2].sequence), 2)
                .unwrap()
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            vec![all[0].sequence, all[1].sequence]
        );
        assert!(matches!(
            forwarder.pending(0),
            Err(OrderbookTransactionForwarderError::InvalidScanLimit)
        ));
        assert!(matches!(
            forwarder.pending(ORDERBOOK_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1 + 1),
            Err(OrderbookTransactionForwarderError::InvalidScanLimit)
        ));
    }

    #[test]
    fn signing_crash_recovers_ready_but_ambiguous_bytes_remain_exact() {
        let matcher = key(10);
        let settlement = key(11);
        let context = context(&matcher, &settlement, 13, cursor(30, 0xA5));
        let temp = TempDir::new().unwrap();
        let signing_id = {
            let forwarder = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();
            let operation_id = forwarder
                .enqueue_unsigned_operation(match_operation(&context), &context)
                .unwrap()
                .operation_id();
            forwarder.claim_for_signing(operation_id).unwrap();
            operation_id
        };
        let recovered = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();
        let pending = recovered.pending(8).unwrap();
        assert_eq!(pending[0].operation_id, signing_id);
        assert_eq!(pending[0].state, OrderbookTransactionDeliveryStateV1::Ready);
        assert_eq!(pending[0].attempts, 1);
        drop(recovered);

        let second = TempDir::new().unwrap();
        let (ambiguous_id, exact) = {
            let forwarder = OrderbookTransactionForwarder::open(second.path(), policy()).unwrap();
            let operation_id = forwarder
                .enqueue_unsigned_operation(match_operation(&context), &context)
                .unwrap()
                .operation_id();
            let request = forwarder.claim_for_signing(operation_id).unwrap();
            let exact = signed_operation(&matcher, request.operation, 20);
            forwarder
                .store_signed_transaction(operation_id, &exact)
                .unwrap();
            assert_eq!(forwarder.begin_submission(operation_id).unwrap(), exact);
            (operation_id, exact)
        };
        let recovered = OrderbookTransactionForwarder::open(second.path(), policy()).unwrap();
        let pending = recovered.pending(8).unwrap();
        assert_eq!(pending[0].operation_id, ambiguous_id);
        assert_eq!(
            pending[0].state,
            OrderbookTransactionDeliveryStateV1::Ambiguous
        );
        assert_eq!(
            pending[0].signed_transaction_bytes.as_deref(),
            Some(exact.as_slice())
        );
    }

    #[test]
    fn signer_attempt_exhaustion_deadletters_even_after_crash_recovery() {
        let matcher = key(26);
        let settlement = key(27);
        let context = context(&matcher, &settlement, 14, cursor(35, 0xB3));
        let temp = TempDir::new().unwrap();
        let operation_id = {
            let forwarder = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();
            let operation_id = forwarder
                .enqueue_unsigned_operation(match_operation(&context), &context)
                .unwrap()
                .operation_id();
            forwarder.claim_for_signing(operation_id).unwrap();
            forwarder.release_signing_claim(operation_id).unwrap();
            forwarder.claim_for_signing(operation_id).unwrap();
            operation_id
        };

        let recovered = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();
        let pending = recovered.pending(8).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].state, OrderbookTransactionDeliveryStateV1::Ready);
        assert_eq!(pending[0].attempts, policy().max_attempts);
        assert!(matches!(
            recovered.claim_for_signing(operation_id),
            Err(OrderbookTransactionForwarderError::RetryExhausted)
        ));
        assert!(recovered.pending(8).unwrap().is_empty());
        let dead = recovered.dead_letters(8).unwrap();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].operation_id, operation_id);
        assert_eq!(
            dead[0].reason,
            OrderbookTransactionDeadLetterReasonV1::RetryExhausted
        );
    }

    #[test]
    fn finalized_absence_retries_exact_bytes_then_deadletters_atomically() {
        let matcher = key(12);
        let settlement = key(13);
        let context = context(&matcher, &settlement, 15, cursor(40, 0xA6));
        let temp = TempDir::new().unwrap();
        let operation = match_operation(&context);
        let exact = signed_operation(&matcher, operation, 30);
        let operation_id = {
            let forwarder = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();
            let operation_id = forwarder
                .enqueue_signed_transaction(&exact, &context)
                .unwrap()
                .operation_id();
            assert_eq!(forwarder.begin_submission(operation_id).unwrap(), exact);
            forwarder.mark_submitted(operation_id).unwrap();
            forwarder
                .mark_finalized_absent(operation_id, cursor(41, 0xA7))
                .unwrap();
            assert_eq!(forwarder.begin_submission(operation_id).unwrap(), exact);
            forwarder.mark_submitted(operation_id).unwrap();
            forwarder
                .mark_finalized_absent(operation_id, cursor(42, 0xA8))
                .unwrap();
            operation_id
        };
        let recovered = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();
        assert!(recovered.pending(8).unwrap().is_empty());
        let dead = recovered.dead_letters(8).unwrap();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].operation_id, operation_id);
        assert_eq!(
            dead[0].reason,
            OrderbookTransactionDeadLetterReasonV1::RetryExhausted
        );
        assert_eq!(dead[0].transaction_digest, Some(transaction_digest(&exact)));
    }

    #[test]
    fn exact_and_cross_peer_semantic_finalization_retain_tombstones() {
        let matcher = key(14);
        let settlement = key(15);
        let first_context = context(&matcher, &settlement, 17, cursor(50, 0xA9));
        let exact = signed_operation(&matcher, match_operation(&first_context), 40);
        let forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();
        let operation_id = forwarder
            .enqueue_signed_transaction(&exact, &first_context)
            .unwrap()
            .operation_id();
        assert!(matches!(
            forwarder.mark_finalized(operation_id, [0xFF; 32], cursor(51, 0xAA)),
            Err(OrderbookTransactionForwarderError::InvalidSignedTransaction)
        ));
        forwarder
            .mark_finalized(operation_id, transaction_digest(&exact), cursor(51, 0xAA))
            .unwrap();
        assert!(matches!(
            forwarder
                .enqueue_signed_transaction(&exact, &first_context)
                .unwrap(),
            OrderbookTransactionEnqueueResultV1::Existing {
                operation_id: replay_id
            } if replay_id == operation_id
        ));

        let second_context = context(&matcher, &settlement, 18, cursor(52, 0xAB));
        let semantic_id = forwarder
            .enqueue_unsigned_operation(maintain_operation(&second_context), &second_context)
            .unwrap()
            .operation_id();
        forwarder
            .mark_semantic_finalized(semantic_id, cursor(53, 0xAC))
            .unwrap();
        assert!(matches!(
            forwarder
                .enqueue_unsigned_operation(maintain_operation(&second_context), &second_context)
                .unwrap(),
            OrderbookTransactionEnqueueResultV1::Existing {
                operation_id: replay_id
            } if replay_id == semantic_id
        ));
    }

    #[test]
    fn corrupt_truncated_and_oversized_checkpoints_fail_closed() {
        let matcher = key(16);
        let settlement = key(17);
        let context = context(&matcher, &settlement, 19, cursor(60, 0xAD));
        let temp = TempDir::new().unwrap();
        {
            let forwarder = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();
            forwarder
                .enqueue_unsigned_operation(match_operation(&context), &context)
                .unwrap();
        }
        let path = temp
            .path()
            .join(ORDERBOOK_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1);
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        assert!(matches!(
            OrderbookTransactionForwarder::open(temp.path(), policy()),
            Err(OrderbookTransactionForwarderError::InvalidCheckpoint)
        ));

        let oversized = TempDir::new().unwrap();
        fs::create_dir_all(oversized.path()).unwrap();
        let mut restrictive = policy();
        restrictive.checkpoint_max_bytes = 32;
        fs::write(
            oversized
                .path()
                .join(ORDERBOOK_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1),
            vec![0xA5; 33],
        )
        .unwrap();
        assert!(matches!(
            OrderbookTransactionForwarder::open(oversized.path(), restrictive),
            Err(OrderbookTransactionForwarderError::CheckpointTooLarge)
        ));
    }

    #[test]
    fn stale_checkpoint_writer_loses_without_publishing_candidate_state() {
        let matcher = key(24);
        let settlement = key(25);
        let first_context = context(&matcher, &settlement, 22, cursor(80, 0xB2));
        let second_context = context(&matcher, &settlement, 23, cursor(80, 0xB2));
        let temp = TempDir::new().unwrap();
        let first = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();
        let stale = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();

        first
            .enqueue_unsigned_operation(match_operation(&first_context), &first_context)
            .unwrap();
        assert!(matches!(
            stale.enqueue_unsigned_operation(match_operation(&second_context), &second_context),
            Err(OrderbookTransactionForwarderError::StaleCheckpoint)
        ));
        assert!(stale.pending(8).unwrap().is_empty());

        drop(first);
        drop(stale);
        let recovered = OrderbookTransactionForwarder::open(temp.path(), policy()).unwrap();
        let pending = recovered.pending(8).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].expected_book_revision,
            Some(first_context.book_revision)
        );
    }

    #[test]
    fn poisoned_runtime_lock_fails_closed_without_state_mutation() {
        let forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();
        let poison = Arc::clone(&forwarder.state);
        let result = thread::spawn(move || {
            let _guard = poison.lock().unwrap();
            panic!("poison forwarder mutex");
        })
        .join();
        assert!(result.is_err());
        assert!(matches!(
            forwarder.pending(1),
            Err(OrderbookTransactionForwarderError::RuntimePoisoned)
        ));
    }

    #[test]
    fn policy_rotation_and_invalid_receipt_fail_before_persistence() {
        let matcher = key(18);
        let settlement = key(19);
        let context = context(&matcher, &settlement, 21, cursor(70, 0xAE));
        let mut rotated = context.clone();
        rotated.policy_record.policy.matcher_authority =
            AccountId::new(key(20).public_key().clone());
        rotated.policy_record.policy_digest = rotated.policy_record.policy.digest().unwrap();

        let bytes = signed_operation(&matcher, match_operation(&context), 50);
        let forwarder = OrderbookTransactionForwarder::in_memory(policy()).unwrap();
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&bytes, &rotated),
            Err(OrderbookTransactionForwarderError::PolicyDigestMismatch)
        ));
        assert!(forwarder.pending(8).unwrap().is_empty());

        for invalid_chain_id in [
            ChainId::from(""),
            ChainId::from("x".repeat(ORDERBOOK_TRANSACTION_MAX_CHAIN_ID_BYTES_V1 + 1)),
        ] {
            let mut invalid_context = context.clone();
            invalid_context.chain_id = invalid_chain_id;
            assert!(matches!(
                forwarder.enqueue_unsigned_operation(
                    match_operation(&invalid_context),
                    &invalid_context
                ),
                Err(OrderbookTransactionForwarderError::InvalidGovernanceContext)
            ));
            assert!(forwarder.pending(8).unwrap().is_empty());
        }

        let mut invalid = settlement_operation(&context, [0x81; 32]);
        if let OrderbookOperationV1::SettlementReceipt(instruction) = &invalid {
            let policy_digest = *instruction.policy_digest();
            let mut receipt_payload = instruction.receipt_payload().to_vec();
            let last = receipt_payload.len() - 1;
            receipt_payload[last] ^= 1;
            invalid = OrderbookOperationV1::SettlementReceipt(
                RecordSorafsOrderbookSettlementReceipt::new(receipt_payload, policy_digest),
            );
        }
        assert!(matches!(
            forwarder.enqueue_unsigned_operation_with_authority(
                AccountId::new(key(0x24).public_key().clone()),
                invalid,
                &context,
            ),
            Err(OrderbookTransactionForwarderError::InvalidOrderbookOperation)
        ));
        assert!(forwarder.pending(8).unwrap().is_empty());
    }
}
