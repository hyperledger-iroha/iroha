//! Durable forwarding for unsigned native repair intents and exact externally signed
//! repair transactions.
//!
//! `NodeHandle` owns this forwarder, while Torii supplies the isolated signer,
//! transaction ingress, and finalized-ledger reconciliation worker. PDP, PoR,
//! and PoTR use the same native repair handoff so no process-local scheduler
//! can become authoritative in production.

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
            ApplySorafsRepairTaskAction, SorafsRepairTaskActionV1, SubmitSorafsRepairAppeal,
            SubmitSorafsRepairTask,
        },
    },
    sorafs::moderation_ledger::{
        REPAIR_LEDGER_MAX_APPEAL_REASON_BYTES_V1, REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1,
        REPAIR_LEDGER_MAX_IDEMPOTENCY_KEY_BYTES_V1, REPAIR_LEDGER_MAX_LEASE_MS_V1,
        REPAIR_LEDGER_MIN_LEASE_MS_V1, RepairFinalizedCursorV1,
        sorafs_repair_idempotency_digest_v1,
    },
    transaction::{Executable, SignedTransaction},
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::{RepairReportV1, RepairSlashProposalV1, RepairTicketId};
use thiserror::Error;

use crate::durable_transaction_forwarder::{
    self as durable, AtomicCheckpointStore, CheckpointStoreError, DeliveryRecord,
    DeliveryTransitionError, FinalizedCursorV1, RetryBoundOutcome, StoredDeliveryStateV1,
};

/// Durable repair-transaction checkpoint schema version.
pub const REPAIR_TRANSACTION_FORWARDER_CHECKPOINT_VERSION_V1: u8 = 1;
/// Canonical repair-transaction checkpoint file.
pub const REPAIR_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1: &str =
    "repair-transaction-forwarder-state.to";
/// Default number of attempts for one semantic repair operation.
pub const REPAIR_TRANSACTION_FORWARDER_DEFAULT_MAX_ATTEMPTS_V1: u32 = 8;
/// Maximum entries returned by one worker scan.
pub const REPAIR_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1: usize = 1_000;
/// Hard ceiling for one canonical signed repair transaction.
pub const REPAIR_TRANSACTION_MAX_CANONICAL_BYTES_V1: usize = 10 * 1024 * 1024;
/// Maximum UTF-8 byte length of the exact active chain identifier.
pub const REPAIR_TRANSACTION_MAX_CHAIN_ID_BYTES_V1: usize = 128;

const CHECKPOINT_LOCK_FILE_NAME: &str = "repair-transaction-forwarder-state.lock";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"sorafs.repair.transaction-forwarder.operation.v1\0";
const SEMANTIC_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.repair.transaction-forwarder.semantic.v1\0";
const CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT: usize = 6;
const CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT: usize = 20;
const CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 1024 * 1024;
const CHECKPOINT_MAX_NESTING_DEPTH: usize = 128;
const TRANSACTION_ELEMENT_AMPLIFICATION_LIMIT: usize = 8;
const TRANSACTION_ALLOCATION_AMPLIFICATION_LIMIT: usize = 20;
const TRANSACTION_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 512 * 1024;
const TRANSACTION_MAX_NESTING_DEPTH: usize = 128;

/// Bounded persistence and retry policy for native repair operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairTransactionForwarderPolicyV1 {
    /// Maximum pending semantic operations.
    pub max_pending: usize,
    /// Maximum finalized idempotency tombstones.
    pub max_completed: usize,
    /// Maximum terminal dead letters.
    pub max_dead_letters: usize,
    /// Maximum attempts for one semantic operation.
    pub max_attempts: u32,
    /// Maximum accepted canonical transaction bytes.
    pub max_transaction_bytes: usize,
    /// Maximum canonical checkpoint bytes.
    pub checkpoint_max_bytes: u64,
}

impl RepairTransactionForwarderPolicyV1 {
    /// Validate all first-release resource bounds.
    pub fn validate(self) -> Result<(), RepairTransactionForwarderError> {
        if self.max_pending == 0
            || self.max_completed == 0
            || self.max_dead_letters == 0
            || self.max_attempts == 0
            || self.max_transaction_bytes == 0
            || self.max_transaction_bytes > REPAIR_TRANSACTION_MAX_CANONICAL_BYTES_V1
            || self.checkpoint_max_bytes == 0
        {
            return Err(RepairTransactionForwarderError::InvalidPolicy);
        }
        Ok(())
    }
}

/// Finalized chain context used to admit one native repair operation.
///
/// Callers must obtain `finalized_cursor` from the same active chain named by
/// `chain_id`. The chain identifier is retained in the durable checkpoint and
/// must match every externally signed transaction exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairTransactionContextV1 {
    /// Exact active chain identity.
    pub chain_id: ChainId,
    /// Finalized block anchor used for semantic reconciliation.
    pub finalized_cursor: RepairFinalizedCursorV1,
}

impl RepairTransactionContextV1 {
    pub(crate) fn validate(&self) -> Result<(), RepairTransactionForwarderError> {
        validate_finalized_cursor(self.finalized_cursor)?;
        if self.chain_id.as_str().is_empty()
            || self.chain_id.as_str().len() > REPAIR_TRANSACTION_MAX_CHAIN_ID_BYTES_V1
        {
            return Err(RepairTransactionForwarderError::InvalidChainContext);
        }
        Ok(())
    }
}

/// Native repair instruction kind retained by the forwarder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairTransactionKindV1 {
    /// Admit one source-bound repair report.
    Submit,
    /// Apply one lease or terminal action.
    Action,
    /// Commit one provider-owner appeal.
    Appeal,
}

/// Bounded native repair operation retained for isolated external signing.
///
/// The forwarder validates the operation, its embedded canonical payloads, and
/// its authority binding before persistence. It never creates or retains a
/// private key.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum RepairOperationV1 {
    /// Admit one source-bound repair report.
    Submit(SubmitSorafsRepairTask),
    /// Apply one lease or terminal action.
    Action(ApplySorafsRepairTaskAction),
    /// Commit one provider-owner appeal.
    Appeal(SubmitSorafsRepairAppeal),
}

impl RepairOperationV1 {
    /// Return the native repair instruction kind.
    #[must_use]
    pub const fn kind(&self) -> RepairTransactionKindV1 {
        match self {
            Self::Submit(_) => RepairTransactionKindV1::Submit,
            Self::Action(_) => RepairTransactionKindV1::Action,
            Self::Appeal(_) => RepairTransactionKindV1::Appeal,
        }
    }

    fn identity(&self) -> (StoredRepairIdentityScopeV1, [u8; 32]) {
        match self {
            Self::Submit(instruction) => (
                StoredRepairIdentityScopeV1::TaskSource,
                *instruction.source_identity(),
            ),
            Self::Action(instruction) => (
                StoredRepairIdentityScopeV1::TaskMutation,
                sorafs_repair_idempotency_digest_v1(
                    instruction.ticket_id(),
                    instruction.action().idempotency_key(),
                ),
            ),
            Self::Appeal(instruction) => (
                StoredRepairIdentityScopeV1::TaskMutation,
                sorafs_repair_idempotency_digest_v1(
                    instruction.ticket_id(),
                    instruction.idempotency_key(),
                ),
            ),
        }
    }
}

impl From<RepairOperationV1> for InstructionBox {
    fn from(operation: RepairOperationV1) -> Self {
        match operation {
            RepairOperationV1::Submit(instruction) => instruction.into(),
            RepairOperationV1::Action(instruction) => instruction.into(),
            RepairOperationV1::Appeal(instruction) => instruction.into(),
        }
    }
}

/// Exact signer work item returned after a durable signing claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairTransactionSigningRequestV1 {
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Exact active chain that the signed envelope must retain.
    pub chain_id: ChainId,
    /// Transaction authority that the signed envelope must retain.
    pub authority: AccountId,
    /// Exact validated native operation that the signed envelope must retain.
    pub operation: RepairOperationV1,
}

/// Durable enqueue result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairTransactionEnqueueResultV1 {
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

impl RepairTransactionEnqueueResultV1 {
    /// Return the stable semantic operation identity.
    #[must_use]
    pub const fn operation_id(self) -> [u8; 32] {
        match self {
            Self::Inserted { operation_id } | Self::Existing { operation_id } => operation_id,
        }
    }
}

/// Runtime-visible crash state for one repair transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairTransactionDeliveryStateV1 {
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

impl From<StoredDeliveryStateV1> for RepairTransactionDeliveryStateV1 {
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

/// Exact pending repair delivery returned to a future worker.
#[derive(Debug, Clone)]
pub struct RepairTransactionPendingV1 {
    /// Insertion sequence.
    pub sequence: u64,
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Native repair instruction kind.
    pub kind: RepairTransactionKindV1,
    /// Exact active chain bound to this operation.
    pub chain_id: ChainId,
    /// Transaction authority bound by the verified signature.
    pub authority: AccountId,
    /// Digest of the semantic authority/instruction pair.
    pub semantic_digest: [u8; 32],
    /// Digest of the currently retained exact transaction bytes.
    pub transaction_digest: Option<[u8; 32]>,
    /// Current durable crash state.
    pub state: RepairTransactionDeliveryStateV1,
    /// Attempts consumed by this semantic operation.
    pub attempts: u32,
    /// Finalized height preceding the current attempt.
    pub baseline_finalized_height: u64,
    /// Finalized hash paired with the baseline height.
    pub baseline_finalized_block_hash: [u8; 32],
    /// Exact canonical signed transaction bytes, absent before external signing completes.
    pub signed_transaction_bytes: Option<Vec<u8>>,
}

/// Payload-free terminal reason retained for operator reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairTransactionDeadLetterReasonV1 {
    /// Finalized state conflicts with the semantic operation.
    FinalizedConflict,
    /// The exact transaction was rejected or expired terminally.
    TransactionRejected,
    /// Bounded retries were exhausted after finalized absence.
    RetryExhausted,
}

/// Payload-free terminal repair delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairTransactionDeadLetterV1 {
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Native repair instruction kind.
    pub kind: RepairTransactionKindV1,
    /// Digest of the semantic authority/instruction pair.
    pub semantic_digest: [u8; 32],
    /// Digest of the final exact transaction bytes, if an envelope existed.
    pub transaction_digest: Option<[u8; 32]>,
    /// Terminal reason.
    pub reason: RepairTransactionDeadLetterReasonV1,
    /// Finalized height observing the terminal condition.
    pub observed_finalized_height: u64,
    /// Finalized hash paired with the observed height.
    pub observed_finalized_block_hash: [u8; 32],
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
enum StoredRepairIdentityScopeV1 {
    TaskSource,
    TaskMutation,
}

impl StoredRepairIdentityScopeV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::TaskSource => 0,
            Self::TaskMutation => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredDeadLetterReasonV1 {
    FinalizedConflict,
    TransactionRejected,
    RetryExhausted,
}

impl From<StoredDeadLetterReasonV1> for RepairTransactionDeadLetterReasonV1 {
    fn from(value: StoredDeadLetterReasonV1) -> Self {
        match value {
            StoredDeadLetterReasonV1::FinalizedConflict => Self::FinalizedConflict,
            StoredDeadLetterReasonV1::TransactionRejected => Self::TransactionRejected,
            StoredDeadLetterReasonV1::RetryExhausted => Self::RetryExhausted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredPendingRepairTransactionV1 {
    sequence: u64,
    operation_id: [u8; 32],
    identity_scope: StoredRepairIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    chain_id: ChainId,
    authority: AccountId,
    operation: RepairOperationV1,
    state: StoredDeliveryStateV1,
    attempts: u32,
    baseline_finalized_height: u64,
    baseline_finalized_block_hash: [u8; 32],
    signed_transaction_bytes: Option<Vec<u8>>,
}

impl StoredPendingRepairTransactionV1 {
    fn snapshot(&self) -> RepairTransactionPendingV1 {
        RepairTransactionPendingV1 {
            sequence: self.sequence,
            operation_id: self.operation_id,
            kind: self.operation.kind(),
            chain_id: self.chain_id.clone(),
            authority: self.authority.clone(),
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

impl DeliveryRecord for StoredPendingRepairTransactionV1 {
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
struct StoredCompletedRepairTransactionV1 {
    operation_id: [u8; 32],
    identity_scope: StoredRepairIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredDeadRepairTransactionV1 {
    operation_id: [u8; 32],
    identity_scope: StoredRepairIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    chain_id: ChainId,
    authority: AccountId,
    operation: RepairOperationV1,
    signed_transaction_bytes: Option<Vec<u8>>,
    reason: StoredDeadLetterReasonV1,
    observed_finalized_height: u64,
    observed_finalized_block_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RepairTransactionForwarderCheckpointV1 {
    version: u8,
    next_sequence: u64,
    pending: Vec<StoredPendingRepairTransactionV1>,
    completed: Vec<StoredCompletedRepairTransactionV1>,
    dead_letters: Vec<StoredDeadRepairTransactionV1>,
}

impl Default for RepairTransactionForwarderCheckpointV1 {
    fn default() -> Self {
        Self {
            version: REPAIR_TRANSACTION_FORWARDER_CHECKPOINT_VERSION_V1,
            next_sequence: 1,
            pending: Vec::new(),
            completed: Vec::new(),
            dead_letters: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct DurableState {
    checkpoint: RepairTransactionForwarderCheckpointV1,
    fingerprint: Option<[u8; 32]>,
    durability_failure: bool,
}

/// Durable bounded forwarder for unsigned intents and externally signed repair transactions.
#[derive(Debug, Clone)]
pub struct RepairTransactionForwarder {
    policy: RepairTransactionForwarderPolicyV1,
    state: Arc<Mutex<DurableState>>,
    store: Option<Arc<AtomicCheckpointStore>>,
}

impl RepairTransactionForwarder {
    /// Construct a non-persistent forwarder for focused composition tests.
    pub fn in_memory(
        policy: RepairTransactionForwarderPolicyV1,
    ) -> Result<Self, RepairTransactionForwarderError> {
        policy.validate()?;
        Ok(Self {
            policy,
            state: Arc::new(Mutex::new(DurableState {
                checkpoint: RepairTransactionForwarderCheckpointV1::default(),
                fingerprint: None,
                durability_failure: false,
            })),
            store: None,
        })
    }

    /// Open or create a durable forwarder below `state_dir`.
    pub fn open(
        state_dir: &Path,
        policy: RepairTransactionForwarderPolicyV1,
    ) -> Result<Self, RepairTransactionForwarderError> {
        policy.validate()?;
        let store = Arc::new(AtomicCheckpointStore::new(
            state_dir,
            REPAIR_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1,
            CHECKPOINT_LOCK_FILE_NAME,
            policy.checkpoint_max_bytes,
        )?);
        let (bytes, fingerprint) = store.load_bytes()?;
        let mut checkpoint = match bytes {
            Some(bytes) => decode_checkpoint(&bytes, policy)?,
            None => RepairTransactionForwarderCheckpointV1::default(),
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

    /// Validate and durably accept one unsigned native repair operation.
    ///
    /// The operation is persisted in `Ready` state with its reviewed finalized
    /// cursor, no signed bytes, and no consumed attempt. Signing remains the
    /// responsibility of an isolated external signer.
    pub fn enqueue_unsigned_operation(
        &self,
        authority: AccountId,
        operation: RepairOperationV1,
        context: &RepairTransactionContextV1,
    ) -> Result<RepairTransactionEnqueueResultV1, RepairTransactionForwarderError> {
        context.validate()?;
        let prepared = PreparedRepairOperation::new_bounded(
            context.chain_id.clone(),
            authority,
            operation,
            self.policy.max_transaction_bytes,
        )?;
        self.enqueue_prepared(prepared, None, context.finalized_cursor)
    }

    /// Validate and durably accept one exact canonical signed repair transaction.
    ///
    /// The exact input bytes are retained unchanged. A semantic replay returns
    /// `Existing` and never replaces the first durable envelope.
    pub fn enqueue_signed_transaction(
        &self,
        signed_transaction_bytes: &[u8],
        context: &RepairTransactionContextV1,
    ) -> Result<RepairTransactionEnqueueResultV1, RepairTransactionForwarderError> {
        context.validate()?;
        let prepared = PreparedRepairOperation::decode_signed_transaction(
            signed_transaction_bytes,
            &context.chain_id,
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
        prepared: PreparedRepairOperation,
        signed_transaction_bytes: Option<&[u8]>,
        baseline_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<RepairTransactionEnqueueResultV1, RepairTransactionForwarderError> {
        let operation_id = operation_id(&prepared);
        let mut state = self.lock_state()?;
        if let Some(existing) = state.checkpoint.pending.iter().find(|entry| {
            entry.identity_scope == prepared.identity_scope
                && entry.identity_digest == prepared.identity_digest
        }) {
            if existing.operation_id == operation_id
                && existing.semantic_digest == prepared.semantic_digest
            {
                return Ok(RepairTransactionEnqueueResultV1::Existing { operation_id });
            }
            return Err(RepairTransactionForwarderError::IdentityConflict);
        }
        if let Some(existing) = state.checkpoint.completed.iter().find(|entry| {
            entry.identity_scope == prepared.identity_scope
                && entry.identity_digest == prepared.identity_digest
        }) {
            if existing.operation_id == operation_id
                && existing.semantic_digest == prepared.semantic_digest
            {
                return Ok(RepairTransactionEnqueueResultV1::Existing { operation_id });
            }
            return Err(RepairTransactionForwarderError::IdentityConflict);
        }
        if state.checkpoint.dead_letters.iter().any(|entry| {
            entry.identity_scope == prepared.identity_scope
                && entry.identity_digest == prepared.identity_digest
        }) {
            return Err(RepairTransactionForwarderError::DeadLetterConflict);
        }
        if state.checkpoint.pending.len() >= self.policy.max_pending {
            return Err(RepairTransactionForwarderError::PendingCapacityExhausted);
        }
        let sequence = state.checkpoint.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(RepairTransactionForwarderError::SequenceExhausted)?;
        let mut candidate = state.checkpoint.clone();
        candidate.next_sequence = next_sequence;
        candidate.pending.push(StoredPendingRepairTransactionV1 {
            sequence,
            operation_id,
            identity_scope: prepared.identity_scope,
            identity_digest: prepared.identity_digest,
            semantic_digest: prepared.semantic_digest,
            chain_id: prepared.chain_id,
            authority: prepared.authority,
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
        Ok(RepairTransactionEnqueueResultV1::Inserted { operation_id })
    }

    /// Return pending entries in stable sequence order.
    pub fn pending(
        &self,
        limit: usize,
    ) -> Result<Vec<RepairTransactionPendingV1>, RepairTransactionForwarderError> {
        self.pending_after(None, limit)
    }

    /// Return a circular page of pending entries after an immutable sequence cursor.
    ///
    /// Entries newer than `after_sequence` are returned first, followed by the
    /// oldest retained entries after wrapping. At most one snapshot of each
    /// pending entry is returned, so one blocked operation cannot starve later
    /// repairs indefinitely.
    pub fn pending_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<RepairTransactionPendingV1>, RepairTransactionForwarderError> {
        if limit == 0 || limit > REPAIR_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1 {
            return Err(RepairTransactionForwarderError::InvalidScanLimit);
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
            .map(StoredPendingRepairTransactionV1::snapshot)
            .collect())
    }

    /// Return the exact validated authority and operation without changing delivery state.
    ///
    /// Finalized-state reconcilers use this accessor for every crash state,
    /// including `Signed`, `Ambiguous`, and `Submitted`; reading semantic
    /// material must never consume a signer attempt or alter the outbox.
    pub fn operation_for_reconciliation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<RepairTransactionSigningRequestV1, RepairTransactionForwarderError> {
        let state = self.lock_state()?;
        let entry = state
            .checkpoint
            .pending
            .iter()
            .find(|entry| entry.operation_id == operation_id)
            .ok_or(RepairTransactionForwarderError::UnknownOperation)?;
        Ok(RepairTransactionSigningRequestV1 {
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
    ) -> Result<Vec<RepairTransactionDeadLetterV1>, RepairTransactionForwarderError> {
        if limit == 0 || limit > REPAIR_TRANSACTION_FORWARDER_MAX_SCAN_ITEMS_V1 {
            return Err(RepairTransactionForwarderError::InvalidScanLimit);
        }
        let state = self.lock_state()?;
        Ok(state
            .checkpoint
            .dead_letters
            .iter()
            .take(limit)
            .map(|entry| RepairTransactionDeadLetterV1 {
                operation_id: entry.operation_id,
                kind: entry.operation.kind(),
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
    /// The transition consumes one bounded attempt before the request is
    /// exposed. A crash or explicit release can restore `Ready`, but never
    /// refunds the consumed attempt.
    pub fn claim_for_signing(
        &self,
        operation_id: [u8; 32],
    ) -> Result<RepairTransactionSigningRequestV1, RepairTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let entry = find_pending_mut(&mut candidate, operation_id)?;
        claim_for_signing(entry, self.policy.max_attempts)?;
        let request = RepairTransactionSigningRequestV1 {
            operation_id: entry.operation_id,
            chain_id: entry.chain_id.clone(),
            authority: entry.authority.clone(),
            operation: entry.operation.clone(),
        };
        self.commit_candidate(&mut state, candidate)?;
        Ok(request)
    }

    /// Persist exact signed bytes for the claimed authority and semantic operation.
    pub fn store_signed_transaction(
        &self,
        expected_operation_id: [u8; 32],
        signed_transaction_bytes: &[u8],
    ) -> Result<[u8; 32], RepairTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let entry = find_pending_mut(&mut candidate, expected_operation_id)?;
        let prepared = PreparedRepairOperation::decode_signed_transaction(
            signed_transaction_bytes,
            &entry.chain_id,
            self.policy.max_transaction_bytes,
        )?;
        if prepared.identity_scope != entry.identity_scope
            || prepared.identity_digest != entry.identity_digest
            || prepared.semantic_digest != entry.semantic_digest
            || prepared.chain_id != entry.chain_id
            || prepared.authority != entry.authority
            || prepared.operation != entry.operation
            || operation_id(&prepared) != entry.operation_id
        {
            return Err(RepairTransactionForwarderError::InvalidSignedTransaction);
        }
        store_signed_transaction(entry, signed_transaction_bytes.to_vec())?;
        let digest = transaction_digest(signed_transaction_bytes);
        self.commit_candidate(&mut state, candidate)?;
        Ok(digest)
    }

    /// Release a signing claim after an isolated signer failure.
    ///
    /// The retained operation and finalized cursor are unchanged, and the
    /// already consumed attempt is not refunded.
    pub fn release_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), RepairTransactionForwarderError> {
        self.mutate_entry(operation_id, |entry| {
            release_signing_claim(entry).map_err(Into::into)
        })
    }

    /// Mark exact bytes ambiguous before exposing them to a transaction submitter.
    pub fn begin_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Vec<u8>, RepairTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let entry = find_pending_mut(&mut candidate, operation_id)?;
        let bytes = durable::begin_submission(entry)?;
        self.commit_candidate(&mut state, candidate)?;
        Ok(bytes)
    }

    /// Record that the exact transaction is pending or applied.
    pub fn mark_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), RepairTransactionForwarderError> {
        self.mutate_entry(operation_id, |entry| {
            durable::mark_submitted(entry).map_err(Into::into)
        })
    }

    /// Record a failure proven to have happened before queue submission.
    pub fn mark_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), RepairTransactionForwarderError> {
        self.mutate_entry(operation_id, |entry| {
            durable::mark_not_submitted(entry).map_err(Into::into)
        })
    }

    /// Retry the same exact bytes only after finalized absence is proven.
    pub fn mark_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
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

    /// Reconcile exact finalized success and retain a bounded semantic tombstone.
    pub fn mark_finalized(
        &self,
        operation_id: [u8; 32],
        expected_transaction_digest: [u8; 32],
        finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        validate_finalized_cursor(finalized_cursor)?;
        if expected_transaction_digest == [0; 32] {
            return Err(RepairTransactionForwarderError::InvalidSignedTransaction);
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
            return Err(RepairTransactionForwarderError::InvalidSignedTransaction);
        }
        self.commit_finalized_operation(&mut state, candidate, position, finalized_cursor)
    }

    /// Reconcile exact semantic success committed by this or another ingress.
    ///
    /// This transition is valid before local signing: the operation identity
    /// authenticates the exact authority and native instruction retained by
    /// the forwarder, while the caller must have matched that semantic
    /// material against a finalized ledger projection. This is required for
    /// cross-peer duplicate submission convergence.
    pub fn mark_semantic_finalized(
        &self,
        operation_id: [u8; 32],
        finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        validate_finalized_cursor(finalized_cursor)?;
        let mut state = self.lock_state()?;
        let candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        self.commit_finalized_operation(&mut state, candidate, position, finalized_cursor)
    }

    fn commit_finalized_operation(
        &self,
        state: &mut DurableState,
        mut candidate: RepairTransactionForwarderCheckpointV1,
        position: usize,
        finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        if candidate.completed.len() >= self.policy.max_completed {
            let oldest = candidate
                .completed
                .iter()
                .enumerate()
                .min_by_key(|(_, completed)| (completed.finalized_height, completed.operation_id))
                .map(|(index, _)| index)
                .ok_or(RepairTransactionForwarderError::InvalidCheckpoint)?;
            candidate.completed.remove(oldest);
        }
        let entry = candidate.pending.remove(position);
        candidate
            .completed
            .push(StoredCompletedRepairTransactionV1 {
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
        observed_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
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
        observed_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
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
            return Err(RepairTransactionForwarderError::InvalidTransition);
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
            &mut StoredPendingRepairTransactionV1,
        ) -> Result<(), RepairTransactionForwarderError>,
    ) -> Result<(), RepairTransactionForwarderError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        mutate(find_pending_mut(&mut candidate, operation_id)?)?;
        self.commit_candidate(&mut state, candidate)
    }

    fn move_to_dead_letter(
        &self,
        checkpoint: &mut RepairTransactionForwarderCheckpointV1,
        position: usize,
        reason: StoredDeadLetterReasonV1,
        cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        validate_finalized_cursor(cursor)?;
        if checkpoint.dead_letters.len() >= self.policy.max_dead_letters {
            return Err(RepairTransactionForwarderError::DeadLetterCapacityExhausted);
        }
        let entry = checkpoint.pending.remove(position);
        checkpoint.dead_letters.push(StoredDeadRepairTransactionV1 {
            operation_id: entry.operation_id,
            identity_scope: entry.identity_scope,
            identity_digest: entry.identity_digest,
            semantic_digest: entry.semantic_digest,
            chain_id: entry.chain_id,
            authority: entry.authority,
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
    ) -> Result<std::sync::MutexGuard<'_, DurableState>, RepairTransactionForwarderError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RepairTransactionForwarderError::RuntimePoisoned)?;
        if state.durability_failure {
            return Err(RepairTransactionForwarderError::DurabilityPoisoned);
        }
        Ok(state)
    }

    fn commit_candidate(
        &self,
        state: &mut DurableState,
        candidate: RepairTransactionForwarderCheckpointV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        validate_checkpoint(&candidate, self.policy)?;
        if let Some(store) = self.store.as_ref() {
            let bytes = norito::to_bytes(&candidate)
                .map_err(RepairTransactionForwarderError::CanonicalEncoding)?;
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
struct PreparedRepairOperation {
    identity_scope: StoredRepairIdentityScopeV1,
    identity_digest: [u8; 32],
    semantic_digest: [u8; 32],
    chain_id: ChainId,
    authority: AccountId,
    operation: RepairOperationV1,
}

impl PreparedRepairOperation {
    fn new(
        chain_id: ChainId,
        authority: AccountId,
        operation: RepairOperationV1,
    ) -> Result<Self, RepairTransactionForwarderError> {
        if chain_id.as_str().is_empty()
            || chain_id.as_str().len() > REPAIR_TRANSACTION_MAX_CHAIN_ID_BYTES_V1
        {
            return Err(RepairTransactionForwarderError::InvalidChainContext);
        }
        validate_operation(&operation, &authority)?;
        let (identity_scope, identity_digest) = operation.identity();
        if identity_digest == [0; 32] {
            return Err(RepairTransactionForwarderError::InvalidRepairOperation);
        }
        let semantic_digest = semantic_digest(&chain_id, &authority, &operation)?;
        Ok(Self {
            identity_scope,
            identity_digest,
            semantic_digest,
            chain_id,
            authority,
            operation,
        })
    }

    fn new_bounded(
        chain_id: ChainId,
        authority: AccountId,
        operation: RepairOperationV1,
        max_transaction_bytes: usize,
    ) -> Result<Self, RepairTransactionForwarderError> {
        let prepared = Self::new(chain_id, authority, operation)?;
        let chain_id_bytes = norito::to_bytes(&prepared.chain_id)
            .map_err(RepairTransactionForwarderError::CanonicalEncoding)?;
        let authority_bytes = norito::to_bytes(&prepared.authority)
            .map_err(RepairTransactionForwarderError::CanonicalEncoding)?;
        let operation_bytes = norito::to_bytes(&prepared.operation)
            .map_err(RepairTransactionForwarderError::CanonicalEncoding)?;
        if chain_id_bytes
            .len()
            .checked_add(authority_bytes.len())
            .and_then(|length| length.checked_add(operation_bytes.len()))
            .is_none_or(|length| length > max_transaction_bytes)
        {
            return Err(RepairTransactionForwarderError::ResourceLimitExceeded);
        }
        Ok(prepared)
    }

    fn decode_signed_transaction(
        bytes: &[u8],
        expected_chain_id: &ChainId,
        max_transaction_bytes: usize,
    ) -> Result<Self, RepairTransactionForwarderError> {
        if bytes.is_empty()
            || bytes.len() > max_transaction_bytes
            || bytes.len() > REPAIR_TRANSACTION_MAX_CANONICAL_BYTES_V1
        {
            return Err(RepairTransactionForwarderError::InvalidSignedTransaction);
        }
        norito::core::from_bytes_view(bytes)
            .map_err(|_| RepairTransactionForwarderError::InvalidSignedTransaction)?;
        let transaction = norito::decode_from_bytes_with_limits::<SignedTransaction>(
            bytes,
            transaction_decode_limits(bytes.len(), max_transaction_bytes)?,
        )
        .map_err(|_| RepairTransactionForwarderError::InvalidSignedTransaction)?;
        if norito::to_bytes(&transaction)
            .map_err(RepairTransactionForwarderError::CanonicalEncoding)?
            != bytes
            || transaction.verify_signature().is_err()
        {
            return Err(RepairTransactionForwarderError::InvalidSignedTransaction);
        }
        if transaction.chain() != expected_chain_id {
            return Err(RepairTransactionForwarderError::ChainIdMismatch);
        }
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(RepairTransactionForwarderError::InvalidSignedTransaction);
        };
        if instructions.len() != 1 {
            return Err(RepairTransactionForwarderError::InvalidSignedTransaction);
        }
        let instruction = &instructions[0];
        let operation = if let Some(instruction) = instruction
            .as_any()
            .downcast_ref::<SubmitSorafsRepairTask>()
        {
            RepairOperationV1::Submit(instruction.clone())
        } else if let Some(instruction) = instruction
            .as_any()
            .downcast_ref::<ApplySorafsRepairTaskAction>()
        {
            RepairOperationV1::Action(instruction.clone())
        } else if let Some(instruction) = instruction
            .as_any()
            .downcast_ref::<SubmitSorafsRepairAppeal>()
        {
            RepairOperationV1::Appeal(instruction.clone())
        } else {
            return Err(RepairTransactionForwarderError::InvalidSignedTransaction);
        };
        let authority = transaction.authority().clone();
        Self::new_bounded(
            expected_chain_id.clone(),
            authority,
            operation,
            max_transaction_bytes,
        )
    }
}

fn validate_operation(
    operation: &RepairOperationV1,
    authority: &AccountId,
) -> Result<(), RepairTransactionForwarderError> {
    match operation {
        RepairOperationV1::Submit(instruction) => {
            if instruction.source_identity() == &[0; 32] {
                return Err(RepairTransactionForwarderError::InvalidRepairOperation);
            }
            let report = decode_repair_report(instruction.report_payload())?;
            if report.auditor_account != authority.to_string() {
                return Err(RepairTransactionForwarderError::AuditorAuthorityMismatch);
            }
        }
        RepairOperationV1::Action(instruction) => {
            RepairTicketId(instruction.ticket_id().clone())
                .validate()
                .map_err(|_| RepairTransactionForwarderError::InvalidRepairOperation)?;
            if *instruction.expected_revision() == 0 {
                return Err(RepairTransactionForwarderError::InvalidRepairOperation);
            }
            validate_idempotency_key(instruction.action().idempotency_key())?;
            match instruction.action() {
                SorafsRepairTaskActionV1::Claim(action) => {
                    validate_lease_duration(action.lease_duration_ms)?;
                }
                SorafsRepairTaskActionV1::Renew(action) => {
                    if action.lease_generation == 0 {
                        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
                    }
                    validate_lease_duration(action.lease_duration_ms)?;
                }
                SorafsRepairTaskActionV1::Complete(action) => {
                    if action.lease_generation == 0 || action.evidence_digest == [0; 32] {
                        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
                    }
                }
                SorafsRepairTaskActionV1::Fail(action) => {
                    if action.lease_generation == 0 || action.failure_digest == [0; 32] {
                        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
                    }
                }
                SorafsRepairTaskActionV1::Escalate(action) => {
                    if action.lease_generation == 0 {
                        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
                    }
                    let proposal = decode_slash_proposal(&action.slash_proposal_payload)?;
                    if &proposal.ticket_id.0 != instruction.ticket_id()
                        || proposal.approval.is_some()
                    {
                        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
                    }
                }
            }
        }
        RepairOperationV1::Appeal(instruction) => {
            RepairTicketId(instruction.ticket_id().clone())
                .validate()
                .map_err(|_| RepairTransactionForwarderError::InvalidRepairOperation)?;
            validate_idempotency_key(instruction.idempotency_key())?;
            if *instruction.expected_revision() == 0
                || instruction.evidence_digest() == &[0; 32]
                || instruction.reason().is_empty()
                || instruction.reason() != instruction.reason().trim()
                || instruction.reason().len() > REPAIR_LEDGER_MAX_APPEAL_REASON_BYTES_V1
                || instruction.reason().chars().any(char::is_control)
            {
                return Err(RepairTransactionForwarderError::InvalidRepairOperation);
            }
        }
    }
    Ok(())
}

fn validate_idempotency_key(key: &str) -> Result<(), RepairTransactionForwarderError> {
    if key.is_empty()
        || key != key.trim()
        || key.len() > REPAIR_LEDGER_MAX_IDEMPOTENCY_KEY_BYTES_V1
        || key.chars().any(char::is_control)
    {
        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
    }
    Ok(())
}

fn validate_lease_duration(duration_ms: u64) -> Result<(), RepairTransactionForwarderError> {
    if !(REPAIR_LEDGER_MIN_LEASE_MS_V1..=REPAIR_LEDGER_MAX_LEASE_MS_V1).contains(&duration_ms) {
        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
    }
    Ok(())
}

pub(crate) fn decode_repair_report(
    bytes: &[u8],
) -> Result<RepairReportV1, RepairTransactionForwarderError> {
    if bytes.is_empty() || bytes.len() > REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1 {
        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
    }
    norito::core::from_bytes_view(bytes)
        .map_err(|_| RepairTransactionForwarderError::InvalidRepairOperation)?;
    let report = norito::decode_from_bytes_with_limits::<RepairReportV1>(
        bytes,
        embedded_decode_limits(bytes.len(), REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1)?,
    )
    .map_err(|_| RepairTransactionForwarderError::InvalidRepairOperation)?;
    if norito::to_bytes(&report).map_err(RepairTransactionForwarderError::CanonicalEncoding)?
        != bytes
        || report.validate().is_err()
    {
        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
    }
    Ok(report)
}

pub(crate) fn decode_slash_proposal(
    bytes: &[u8],
) -> Result<RepairSlashProposalV1, RepairTransactionForwarderError> {
    if bytes.is_empty() || bytes.len() > REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1 {
        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
    }
    norito::core::from_bytes_view(bytes)
        .map_err(|_| RepairTransactionForwarderError::InvalidRepairOperation)?;
    let proposal = norito::decode_from_bytes_with_limits::<RepairSlashProposalV1>(
        bytes,
        embedded_decode_limits(bytes.len(), REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1)?,
    )
    .map_err(|_| RepairTransactionForwarderError::InvalidRepairOperation)?;
    if norito::to_bytes(&proposal).map_err(RepairTransactionForwarderError::CanonicalEncoding)?
        != bytes
        || proposal.validate().is_err()
    {
        return Err(RepairTransactionForwarderError::InvalidRepairOperation);
    }
    Ok(proposal)
}

fn semantic_digest(
    chain_id: &ChainId,
    authority: &AccountId,
    operation: &RepairOperationV1,
) -> Result<[u8; 32], RepairTransactionForwarderError> {
    let chain_id = chain_id.as_str();
    let authority = authority.to_string();
    let operation =
        norito::to_bytes(operation).map_err(RepairTransactionForwarderError::CanonicalEncoding)?;
    let chain_id_len = u64::try_from(chain_id.len())
        .map_err(|_| RepairTransactionForwarderError::InvalidRepairOperation)?;
    let authority_len = u64::try_from(authority.len())
        .map_err(|_| RepairTransactionForwarderError::InvalidRepairOperation)?;
    let operation_len = u64::try_from(operation.len())
        .map_err(|_| RepairTransactionForwarderError::InvalidRepairOperation)?;
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

fn operation_id(prepared: &PreparedRepairOperation) -> [u8; 32] {
    operation_id_from_parts(
        prepared.identity_scope,
        prepared.identity_digest,
        prepared.semantic_digest,
    )
}

fn operation_id_from_parts(
    identity_scope: StoredRepairIdentityScopeV1,
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

fn validate_repair_delivery(entry: &StoredPendingRepairTransactionV1, max_attempts: u32) -> bool {
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

fn recover_interrupted_signing(entry: &mut StoredPendingRepairTransactionV1) -> bool {
    if entry.state != StoredDeliveryStateV1::Signing {
        return false;
    }
    entry.state = StoredDeliveryStateV1::Ready;
    true
}

fn claim_for_signing(
    entry: &mut StoredPendingRepairTransactionV1,
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
    entry: &mut StoredPendingRepairTransactionV1,
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
    entry: &mut StoredPendingRepairTransactionV1,
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

fn find_pending_mut(
    checkpoint: &mut RepairTransactionForwarderCheckpointV1,
    operation_id: [u8; 32],
) -> Result<&mut StoredPendingRepairTransactionV1, RepairTransactionForwarderError> {
    checkpoint
        .pending
        .iter_mut()
        .find(|entry| entry.operation_id == operation_id)
        .ok_or(RepairTransactionForwarderError::UnknownOperation)
}

fn pending_position(
    checkpoint: &RepairTransactionForwarderCheckpointV1,
    operation_id: [u8; 32],
) -> Result<usize, RepairTransactionForwarderError> {
    checkpoint
        .pending
        .iter()
        .position(|entry| entry.operation_id == operation_id)
        .ok_or(RepairTransactionForwarderError::UnknownOperation)
}

fn validate_checkpoint(
    checkpoint: &RepairTransactionForwarderCheckpointV1,
    policy: RepairTransactionForwarderPolicyV1,
) -> Result<(), RepairTransactionForwarderError> {
    if checkpoint.version != REPAIR_TRANSACTION_FORWARDER_CHECKPOINT_VERSION_V1
        || checkpoint.next_sequence == 0
        || checkpoint.pending.len() > policy.max_pending
        || checkpoint.completed.len() > policy.max_completed
        || checkpoint.dead_letters.len() > policy.max_dead_letters
    {
        return Err(RepairTransactionForwarderError::InvalidCheckpoint);
    }
    let mut identities = BTreeSet::new();
    let mut operations = BTreeSet::new();
    let mut previous_sequence = 0_u64;
    for entry in &checkpoint.pending {
        let prepare = if entry.signed_transaction_bytes.is_some() {
            PreparedRepairOperation::new(
                entry.chain_id.clone(),
                entry.authority.clone(),
                entry.operation.clone(),
            )
        } else {
            PreparedRepairOperation::new_bounded(
                entry.chain_id.clone(),
                entry.authority.clone(),
                entry.operation.clone(),
                policy.max_transaction_bytes,
            )
        };
        let prepared = prepare.map_err(|_| RepairTransactionForwarderError::InvalidCheckpoint)?;
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
            || !validate_repair_delivery(entry, policy.max_attempts)
            || !identities.insert((entry.identity_scope, entry.identity_digest))
            || !operations.insert(entry.operation_id)
        {
            return Err(RepairTransactionForwarderError::InvalidCheckpoint);
        }
        if let Some(bytes) = entry.signed_transaction_bytes.as_deref() {
            let prepared = PreparedRepairOperation::decode_signed_transaction(
                bytes,
                &entry.chain_id,
                policy.max_transaction_bytes,
            )
            .map_err(|_| RepairTransactionForwarderError::InvalidCheckpoint)?;
            if prepared.identity_scope != entry.identity_scope
                || prepared.identity_digest != entry.identity_digest
                || prepared.semantic_digest != entry.semantic_digest
                || prepared.chain_id != entry.chain_id
                || prepared.authority != entry.authority
                || prepared.operation != entry.operation
            {
                return Err(RepairTransactionForwarderError::InvalidCheckpoint);
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
            return Err(RepairTransactionForwarderError::InvalidCheckpoint);
        }
    }
    for entry in &checkpoint.dead_letters {
        let prepare = if entry.signed_transaction_bytes.is_some() {
            PreparedRepairOperation::new(
                entry.chain_id.clone(),
                entry.authority.clone(),
                entry.operation.clone(),
            )
        } else {
            PreparedRepairOperation::new_bounded(
                entry.chain_id.clone(),
                entry.authority.clone(),
                entry.operation.clone(),
                policy.max_transaction_bytes,
            )
        };
        let prepared = prepare.map_err(|_| RepairTransactionForwarderError::InvalidCheckpoint)?;
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
            return Err(RepairTransactionForwarderError::InvalidCheckpoint);
        }
        if let Some(bytes) = entry.signed_transaction_bytes.as_deref() {
            let prepared = PreparedRepairOperation::decode_signed_transaction(
                bytes,
                &entry.chain_id,
                policy.max_transaction_bytes,
            )
            .map_err(|_| RepairTransactionForwarderError::InvalidCheckpoint)?;
            if prepared.identity_scope != entry.identity_scope
                || prepared.identity_digest != entry.identity_digest
                || prepared.semantic_digest != entry.semantic_digest
                || prepared.chain_id != entry.chain_id
                || prepared.authority != entry.authority
                || prepared.operation != entry.operation
            {
                return Err(RepairTransactionForwarderError::InvalidCheckpoint);
            }
        }
    }
    Ok(())
}

fn decode_checkpoint(
    bytes: &[u8],
    policy: RepairTransactionForwarderPolicyV1,
) -> Result<RepairTransactionForwarderCheckpointV1, RepairTransactionForwarderError> {
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > policy.checkpoint_max_bytes
    {
        return Err(RepairTransactionForwarderError::CheckpointTooLarge);
    }
    norito::core::from_bytes_view(bytes)
        .map_err(|_| RepairTransactionForwarderError::InvalidCheckpoint)?;
    let checkpoint =
        norito::decode_from_bytes_with_limits::<RepairTransactionForwarderCheckpointV1>(
            bytes,
            checkpoint_decode_limits(bytes.len())?,
        )
        .map_err(|_| RepairTransactionForwarderError::InvalidCheckpoint)?;
    if norito::to_bytes(&checkpoint).map_err(RepairTransactionForwarderError::CanonicalEncoding)?
        != bytes
    {
        return Err(RepairTransactionForwarderError::InvalidCheckpoint);
    }
    validate_checkpoint(&checkpoint, policy)?;
    Ok(checkpoint)
}

fn validate_finalized_cursor(
    cursor: RepairFinalizedCursorV1,
) -> Result<(), RepairTransactionForwarderError> {
    durable::validate_finalized_cursor(finalized_cursor(cursor)).map_err(Into::into)
}

const fn finalized_cursor(cursor: RepairFinalizedCursorV1) -> FinalizedCursorV1 {
    FinalizedCursorV1 {
        height: cursor.height,
        block_hash: cursor.block_hash,
    }
}

fn checkpoint_decode_limits(
    encoded_bytes: usize,
) -> Result<norito::DecodeLimits, RepairTransactionForwarderError> {
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
) -> Result<norito::DecodeLimits, RepairTransactionForwarderError> {
    decode_limits(
        encoded_bytes,
        max_transaction_bytes,
        TRANSACTION_ELEMENT_AMPLIFICATION_LIMIT,
        TRANSACTION_ALLOCATION_AMPLIFICATION_LIMIT,
        TRANSACTION_ALLOCATION_FIXED_OVERHEAD_BYTES,
        TRANSACTION_MAX_NESTING_DEPTH,
    )
}

fn embedded_decode_limits(
    encoded_bytes: usize,
    max_bytes: usize,
) -> Result<norito::DecodeLimits, RepairTransactionForwarderError> {
    decode_limits(encoded_bytes, max_bytes, 6, 16, 64 * 1024, 64)
}

fn decode_limits(
    encoded_bytes: usize,
    max_bytes: usize,
    element_amplification: usize,
    allocation_amplification: usize,
    fixed_allocation: usize,
    max_depth: usize,
) -> Result<norito::DecodeLimits, RepairTransactionForwarderError> {
    if encoded_bytes == 0 || encoded_bytes > max_bytes {
        return Err(RepairTransactionForwarderError::ResourceLimitExceeded);
    }
    let total_elements = encoded_bytes
        .checked_mul(element_amplification)
        .ok_or(RepairTransactionForwarderError::ResourceLimitExceeded)?;
    let total_allocated_bytes = encoded_bytes
        .checked_mul(allocation_amplification)
        .and_then(|budget| budget.checked_add(fixed_allocation))
        .ok_or(RepairTransactionForwarderError::ResourceLimitExceeded)?;
    Ok(norito::DecodeLimits::new(
        max_bytes,
        max_bytes,
        total_elements,
        total_allocated_bytes,
        max_depth,
    ))
}

/// Durable repair-transaction forwarding error.
#[derive(Debug, Error)]
pub enum RepairTransactionForwarderError {
    /// Policy contains an invalid or unbounded limit.
    #[error("repair transaction forwarder policy is invalid")]
    InvalidPolicy,
    /// Active chain identity or finalized cursor is invalid.
    #[error("repair transaction chain context is invalid")]
    InvalidChainContext,
    /// Signed bytes are malformed, noncanonical, unsigned, or contain the wrong executable.
    #[error("signed repair transaction is invalid")]
    InvalidSignedTransaction,
    /// Signed transaction belongs to a different chain.
    #[error("signed repair transaction chain id does not match the active chain")]
    ChainIdMismatch,
    /// The native repair instruction or an embedded canonical payload is invalid.
    #[error("native repair operation is invalid")]
    InvalidRepairOperation,
    /// The signed authority does not match the report's embedded auditor account.
    #[error("repair report auditor does not match signed transaction authority")]
    AuditorAuthorityMismatch,
    /// Canonical encoding failed.
    #[error("repair transaction canonical encoding failed: {0}")]
    CanonicalEncoding(#[source] norito::Error),
    /// A semantic identity is retained with different authority or instruction material.
    #[error("repair transaction identity conflicts with retained state")]
    IdentityConflict,
    /// The semantic identity already has a terminal dead letter.
    #[error("repair transaction identity has a terminal dead letter")]
    DeadLetterConflict,
    /// Pending capacity is exhausted.
    #[error("repair transaction pending capacity is exhausted")]
    PendingCapacityExhausted,
    /// Dead-letter capacity is exhausted.
    #[error("repair transaction dead-letter capacity is exhausted")]
    DeadLetterCapacityExhausted,
    /// Sequence allocation overflowed.
    #[error("repair transaction sequence is exhausted")]
    SequenceExhausted,
    /// Worker scan limit is outside the fixed bound.
    #[error("repair transaction scan limit is invalid")]
    InvalidScanLimit,
    /// Operation is not pending.
    #[error("repair transaction operation is not pending")]
    UnknownOperation,
    /// State-machine transition is unsafe.
    #[error("repair transaction transition is invalid")]
    InvalidTransition,
    /// Finalized cursor is zero or did not advance enough to prove absence.
    #[error("repair transaction finalized cursor is invalid")]
    InvalidFinalizedCursor,
    /// Retry budget is exhausted.
    #[error("repair transaction retry bound is exhausted")]
    RetryExhausted,
    /// A bounded decode or canonical payload exceeds a resource ceiling.
    #[error("repair transaction resource limit is exceeded")]
    ResourceLimitExceeded,
    /// Checkpoint is malformed, inconsistent, or noncanonical.
    #[error("repair transaction checkpoint is invalid")]
    InvalidCheckpoint,
    /// Checkpoint exceeds its configured byte ceiling.
    #[error("repair transaction checkpoint exceeds its byte limit")]
    CheckpointTooLarge,
    /// Checkpoint path is unsafe or inaccessible.
    #[error("repair transaction checkpoint I/O failed")]
    CheckpointIo,
    /// Another runtime changed the checkpoint.
    #[error("repair transaction checkpoint changed concurrently")]
    StaleCheckpoint,
    /// Another writer owns the checkpoint.
    #[error("repair transaction checkpoint writer is busy")]
    CheckpointBusy,
    /// Rename may be visible but directory durability is unknown.
    #[error("repair transaction checkpoint durability is uncertain")]
    CheckpointDurabilityUncertain,
    /// Runtime stopped after uncertain durability.
    #[error("repair transaction checkpoint durability is poisoned")]
    DurabilityPoisoned,
    /// Runtime state mutex is poisoned.
    #[error("repair transaction runtime lock is poisoned")]
    RuntimePoisoned,
}

impl From<DeliveryTransitionError> for RepairTransactionForwarderError {
    fn from(error: DeliveryTransitionError) -> Self {
        match error {
            DeliveryTransitionError::InvalidFinalizedCursor => Self::InvalidFinalizedCursor,
            DeliveryTransitionError::InvalidTransition => Self::InvalidTransition,
            DeliveryTransitionError::RetryExhausted => Self::RetryExhausted,
        }
    }
}

impl From<CheckpointStoreError> for RepairTransactionForwarderError {
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
    use std::{
        fs,
        io::Write as _,
        sync::{Arc, Barrier},
        thread,
        time::Duration,
    };

    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt as _;

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        ChainId, Level,
        account::AccountId,
        isi::{
            InstructionBox, Log,
            sorafs::{SorafsRepairClaimV1, SorafsRepairCompleteV1},
        },
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_manifest::{
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, RepairCauseV1, RepairEvidenceV1,
        RepairManualCauseV1,
    };
    use tempfile::TempDir;

    use super::*;

    fn write_private_checkpoint(path: &Path, bytes: &[u8]) {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(path)
            .expect("create private checkpoint fixture");
        file.write_all(bytes)
            .expect("write private checkpoint fixture");
    }

    fn policy() -> RepairTransactionForwarderPolicyV1 {
        RepairTransactionForwarderPolicyV1 {
            max_pending: 8,
            max_completed: 8,
            max_dead_letters: 8,
            max_attempts: 2,
            max_transaction_bytes: 512 * 1024,
            checkpoint_max_bytes: 4 * 1024 * 1024,
        }
    }

    fn cursor(height: u64, hash_byte: u8) -> RepairFinalizedCursorV1 {
        RepairFinalizedCursorV1 {
            height,
            block_hash: [hash_byte; 32],
        }
    }

    fn context(height: u64, hash_byte: u8) -> RepairTransactionContextV1 {
        RepairTransactionContextV1 {
            chain_id: ChainId::from("repair-transaction-forwarder-test"),
            finalized_cursor: cursor(height, hash_byte),
        }
    }

    fn key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap()
    }

    fn report(authority: &AccountId, ticket_id: &str) -> RepairReportV1 {
        RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId(ticket_id.to_owned()),
            auditor_account: authority.to_string(),
            submitted_at_unix: 1,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x11; 32],
                provider_id: [0x22; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "audited repair".to_owned(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        }
    }

    fn report_payloads_at_ledger_boundary(report: &RepairReportV1) -> (Vec<u8>, Vec<u8>) {
        let encode_with_padding = |padding: usize| {
            let mut candidate = report.clone();
            candidate.evidence.evidence_json = Some(format!("\"{}\"", "x".repeat(padding)));
            candidate
                .validate()
                .expect("boundary repair report remains semantically valid");
            norito::to_bytes(&candidate).expect("encode boundary repair report")
        };

        let mut largest_accepted_padding = 0_usize;
        let mut first_rejected_padding = REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1;
        while encode_with_padding(first_rejected_padding).len()
            <= REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1
        {
            first_rejected_padding = first_rejected_padding
                .checked_mul(2)
                .expect("repair report boundary search remains bounded");
        }
        while largest_accepted_padding + 1 < first_rejected_padding {
            let candidate_padding =
                largest_accepted_padding + (first_rejected_padding - largest_accepted_padding) / 2;
            if encode_with_padding(candidate_padding).len()
                <= REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1
            {
                largest_accepted_padding = candidate_padding;
            } else {
                first_rejected_padding = candidate_padding;
            }
        }

        let largest_accepted = encode_with_padding(largest_accepted_padding);
        let first_rejected = encode_with_padding(first_rejected_padding);
        assert_eq!(
            first_rejected_padding,
            largest_accepted_padding + 1,
            "boundary search finds adjacent valid report payloads"
        );
        assert!(largest_accepted.len() <= REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1);
        assert!(first_rejected.len() > REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1);
        (largest_accepted, first_rejected)
    }

    fn submit_instruction(
        signer: &KeyPair,
        source_identity: [u8; 32],
        ticket_id: &str,
    ) -> SubmitSorafsRepairTask {
        let authority = AccountId::new(signer.public_key().clone());
        SubmitSorafsRepairTask::new(
            source_identity,
            norito::to_bytes(&report(&authority, ticket_id)).unwrap(),
        )
    }

    fn claim_instruction(
        ticket_id: &str,
        expected_revision: u64,
        idempotency_key: &str,
    ) -> ApplySorafsRepairTaskAction {
        ApplySorafsRepairTaskAction::new(
            ticket_id.to_owned(),
            expected_revision,
            SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
                lease_duration_ms: REPAIR_LEDGER_MIN_LEASE_MS_V1,
                idempotency_key: idempotency_key.to_owned(),
            }),
        )
    }

    fn signed_bytes(
        signer: &KeyPair,
        authority: AccountId,
        instructions: impl IntoIterator<Item = InstructionBox>,
        creation_time_ms: u64,
    ) -> Vec<u8> {
        let mut builder = TransactionBuilder::new(
            ChainId::from("repair-transaction-forwarder-test"),
            authority,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions);
        builder.set_creation_time(Duration::from_millis(creation_time_ms));
        let transaction = builder.try_sign(signer.private_key()).unwrap();
        norito::to_bytes(&transaction).unwrap()
    }

    fn signed_repair_bytes(
        signer: &KeyPair,
        instruction: impl Into<InstructionBox>,
        creation_time_ms: u64,
    ) -> Vec<u8> {
        signed_bytes(
            signer,
            AccountId::new(signer.public_key().clone()),
            [instruction.into()],
            creation_time_ms,
        )
    }

    #[test]
    fn accepts_all_native_repair_kinds_and_exact_replay() {
        let signer = key(1);

        let submit = signed_repair_bytes(
            &signer,
            submit_instruction(&signer, [0x31; 32], "REP-SUBMIT"),
            1,
        );
        let submit_forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let inserted = submit_forwarder
            .enqueue_signed_transaction(&submit, &context(1, 1))
            .unwrap();
        assert!(matches!(
            submit_forwarder
                .enqueue_signed_transaction(&submit, &context(2, 2))
                .unwrap(),
            RepairTransactionEnqueueResultV1::Existing { .. }
        ));
        assert_eq!(
            submit_forwarder
                .begin_submission(inserted.operation_id())
                .unwrap(),
            submit
        );

        let action = signed_repair_bytes(
            &signer,
            claim_instruction("REP-ACTION", 1, "claim-action"),
            2,
        );
        let action_forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        assert!(matches!(
            action_forwarder
                .enqueue_signed_transaction(&action, &context(1, 1))
                .unwrap(),
            RepairTransactionEnqueueResultV1::Inserted { .. }
        ));

        let appeal = signed_repair_bytes(
            &signer,
            SubmitSorafsRepairAppeal::new(
                "REP-APPEAL".to_owned(),
                2,
                [0x41; 32],
                "provider evidence".to_owned(),
                "appeal-1".to_owned(),
            ),
            3,
        );
        let appeal_forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        assert!(matches!(
            appeal_forwarder
                .enqueue_signed_transaction(&appeal, &context(1, 1))
                .unwrap(),
            RepairTransactionEnqueueResultV1::Inserted { .. }
        ));
    }

    #[test]
    fn signed_ingress_rejects_a_foreign_chain_before_persistence() {
        let signer = key(22);
        let authority = AccountId::new(signer.public_key().clone());
        let mut builder = TransactionBuilder::new(
            ChainId::from("foreign-repair-chain"),
            authority,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(claim_instruction(
            "REP-FOREIGN-CHAIN",
            1,
            "foreign-chain",
        ))]);
        builder.set_creation_time(Duration::from_millis(1));
        let transaction = builder.try_sign(signer.private_key()).unwrap();
        let bytes = norito::to_bytes(&transaction).unwrap();
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&bytes, &context(1, 1)),
            Err(RepairTransactionForwarderError::ChainIdMismatch)
        ));
        assert!(forwarder.pending(8).unwrap().is_empty());
    }

    #[test]
    fn unsigned_operations_claim_exact_signing_material_without_consuming_enqueue_attempt() {
        let signer = key(13);
        let authority = AccountId::new(signer.public_key().clone());
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let operations = [
            RepairOperationV1::Submit(submit_instruction(
                &signer,
                [0x91; 32],
                "REP-UNSIGNED-SUBMIT",
            )),
            RepairOperationV1::Action(claim_instruction(
                "REP-UNSIGNED-ACTION",
                1,
                "unsigned-action",
            )),
            RepairOperationV1::Appeal(SubmitSorafsRepairAppeal::new(
                "REP-UNSIGNED-APPEAL".to_owned(),
                2,
                [0x92; 32],
                "provider evidence".to_owned(),
                "unsigned-appeal".to_owned(),
            )),
        ];

        for (index, operation) in operations.into_iter().enumerate() {
            let index_u64 = u64::try_from(index).unwrap();
            let index_u8 = u8::try_from(index).unwrap();
            let baseline = cursor(10 + index_u64, 0xA0 + index_u8);
            let operation_id = forwarder
                .enqueue_unsigned_operation(
                    authority.clone(),
                    operation.clone(),
                    &RepairTransactionContextV1 {
                        chain_id: ChainId::from("repair-transaction-forwarder-test"),
                        finalized_cursor: baseline,
                    },
                )
                .unwrap()
                .operation_id();
            let pending = forwarder
                .pending(8)
                .unwrap()
                .into_iter()
                .find(|entry| entry.operation_id == operation_id)
                .unwrap();
            assert_eq!(pending.state, RepairTransactionDeliveryStateV1::Ready);
            assert_eq!(pending.attempts, 0);
            assert_eq!(pending.baseline_finalized_height, baseline.height);
            assert_eq!(pending.baseline_finalized_block_hash, baseline.block_hash);
            assert!(pending.signed_transaction_bytes.is_none());

            let reconciliation = forwarder
                .operation_for_reconciliation(operation_id)
                .unwrap();
            assert_eq!(reconciliation.operation_id, operation_id);
            assert_eq!(reconciliation.authority, authority);
            assert_eq!(reconciliation.operation, operation);
            let still_ready = forwarder
                .pending(8)
                .unwrap()
                .into_iter()
                .find(|entry| entry.operation_id == operation_id)
                .unwrap();
            assert_eq!(still_ready.state, RepairTransactionDeliveryStateV1::Ready);
            assert_eq!(still_ready.attempts, 0);

            let request = forwarder.claim_for_signing(operation_id).unwrap();
            assert_eq!(request.operation_id, operation_id);
            assert_eq!(request.authority, authority);
            assert_eq!(request.operation, operation);
            let claimed = forwarder
                .pending(8)
                .unwrap()
                .into_iter()
                .find(|entry| entry.operation_id == operation_id)
                .unwrap();
            assert_eq!(claimed.state, RepairTransactionDeliveryStateV1::Signing);
            assert_eq!(claimed.attempts, 1);
            assert!(claimed.signed_transaction_bytes.is_none());

            let signed = signed_repair_bytes(&signer, request.operation, 100 + index_u64);
            assert_eq!(
                forwarder
                    .store_signed_transaction(operation_id, &signed)
                    .unwrap(),
                transaction_digest(&signed)
            );
            let stored = forwarder
                .pending(8)
                .unwrap()
                .into_iter()
                .find(|entry| entry.operation_id == operation_id)
                .unwrap();
            assert_eq!(stored.state, RepairTransactionDeliveryStateV1::Signed);
            assert_eq!(stored.attempts, 1);
            assert_eq!(
                stored.signed_transaction_bytes.as_deref(),
                Some(signed.as_slice())
            );
            assert_eq!(
                forwarder
                    .operation_for_reconciliation(operation_id)
                    .unwrap()
                    .operation,
                operation
            );
            let still_signed = forwarder
                .pending(8)
                .unwrap()
                .into_iter()
                .find(|entry| entry.operation_id == operation_id)
                .unwrap();
            assert_eq!(still_signed.state, RepairTransactionDeliveryStateV1::Signed);
            assert_eq!(still_signed.attempts, 1);
        }
    }

    #[test]
    fn pending_after_advances_and_wraps_without_starving_older_entries() {
        let signer = key(21);
        let authority = AccountId::new(signer.public_key().clone());
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        for index in 0..3_u8 {
            forwarder
                .enqueue_unsigned_operation(
                    authority.clone(),
                    RepairOperationV1::Action(claim_instruction(
                        &format!("REP-SCAN-{index}"),
                        1,
                        &format!("scan-{index}"),
                    )),
                    &context(1, 1),
                )
                .unwrap();
        }

        let all = forwarder.pending(8).unwrap();
        assert_eq!(all.len(), 3);
        let after_first = forwarder.pending_after(Some(all[0].sequence), 2).unwrap();
        assert_eq!(
            after_first
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            vec![all[1].sequence, all[2].sequence]
        );
        let wrapped = forwarder.pending_after(Some(all[2].sequence), 2).unwrap();
        assert_eq!(
            wrapped
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            vec![all[0].sequence, all[1].sequence]
        );
    }

    #[test]
    fn semantic_finalization_before_signing_retains_exact_replay_tombstone() {
        let signer = key(22);
        let authority = AccountId::new(signer.public_key().clone());
        let operation =
            RepairOperationV1::Submit(submit_instruction(&signer, [0xD1; 32], "REP-CROSS-PEER"));
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let operation_id = forwarder
            .enqueue_unsigned_operation(authority.clone(), operation.clone(), &context(1, 1))
            .unwrap()
            .operation_id();

        forwarder
            .mark_semantic_finalized(operation_id, cursor(2, 2))
            .unwrap();
        assert!(forwarder.pending(8).unwrap().is_empty());
        assert!(matches!(
            forwarder
                .enqueue_unsigned_operation(authority.clone(), operation, &context(3, 3))
                .unwrap(),
            RepairTransactionEnqueueResultV1::Existing {
                operation_id: replay_id
            } if replay_id == operation_id
        ));

        let conflicting = RepairOperationV1::Submit(submit_instruction(
            &signer,
            [0xD1; 32],
            "REP-CROSS-PEER-CONFLICT",
        ));
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(authority, conflicting, &context(3, 3)),
            Err(RepairTransactionForwarderError::IdentityConflict)
        ));
    }

    #[test]
    fn por_source_collision_rejects_different_canonical_report() {
        let signer = key(23);
        let authority = AccountId::new(signer.public_key().clone());
        let first_intent = crate::PorFailedRepairIntentV1 {
            manifest_digest: [0xA1; 32],
            provider_id: [0xA2; 32],
            challenge_id: [0xA3; 32],
            failed_samples: 4,
            proof_digest: Some([0xA4; 32]),
            decided_at_unix: 1_700_000_001,
        };
        let mut conflicting_intent = first_intent;
        conflicting_intent.provider_id = [0xB2; 32];
        let first_report =
            crate::canonical_por_failure_repair_report_v1(first_intent, &authority.to_string())
                .expect("canonical first PoR report");
        let conflicting_report = crate::canonical_por_failure_repair_report_v1(
            conflicting_intent,
            &authority.to_string(),
        )
        .expect("canonical conflicting PoR report");
        assert_eq!(
            first_intent.source_identity(),
            conflicting_intent.source_identity(),
            "the negative must reuse the same challenge-derived source identity"
        );

        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let first_operation = RepairOperationV1::Submit(SubmitSorafsRepairTask::new(
            first_intent.source_identity(),
            norito::to_bytes(&first_report).expect("encode first PoR report"),
        ));
        forwarder
            .enqueue_unsigned_operation(authority.clone(), first_operation, &context(1, 1))
            .expect("first PoR repair");
        let conflicting_operation = RepairOperationV1::Submit(SubmitSorafsRepairTask::new(
            conflicting_intent.source_identity(),
            norito::to_bytes(&conflicting_report).expect("encode conflicting PoR report"),
        ));
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(authority, conflicting_operation, &context(1, 1),),
            Err(RepairTransactionForwarderError::IdentityConflict)
        ));
    }

    #[test]
    fn public_operation_preserves_checkpoint_v1_payload_shape() {
        #[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
        enum LegacyStoredRepairOperationV1 {
            Submit(SubmitSorafsRepairTask),
            Action(ApplySorafsRepairTaskAction),
            Appeal(SubmitSorafsRepairAppeal),
        }

        let signer = key(20);
        let pairs = [
            (
                RepairOperationV1::Submit(submit_instruction(
                    &signer,
                    [0x94; 32],
                    "REP-WIRE-SUBMIT",
                )),
                LegacyStoredRepairOperationV1::Submit(submit_instruction(
                    &signer,
                    [0x94; 32],
                    "REP-WIRE-SUBMIT",
                )),
            ),
            (
                RepairOperationV1::Action(claim_instruction("REP-WIRE-ACTION", 1, "wire-action")),
                LegacyStoredRepairOperationV1::Action(claim_instruction(
                    "REP-WIRE-ACTION",
                    1,
                    "wire-action",
                )),
            ),
            (
                RepairOperationV1::Appeal(SubmitSorafsRepairAppeal::new(
                    "REP-WIRE-APPEAL".to_owned(),
                    1,
                    [0x95; 32],
                    "wire evidence".to_owned(),
                    "wire-appeal".to_owned(),
                )),
                LegacyStoredRepairOperationV1::Appeal(SubmitSorafsRepairAppeal::new(
                    "REP-WIRE-APPEAL".to_owned(),
                    1,
                    [0x95; 32],
                    "wire evidence".to_owned(),
                    "wire-appeal".to_owned(),
                )),
            ),
        ];
        for (public, legacy) in pairs {
            assert_eq!(
                norito::codec::Encode::encode(&public),
                norito::codec::Encode::encode(&legacy)
            );
        }
    }

    #[test]
    fn unsigned_enqueue_validates_auditor_deduplicates_and_rejects_identity_rotation() {
        let signer = key(14);
        let rotated_signer = key(15);
        let authority = AccountId::new(signer.public_key().clone());
        let rotated_authority = AccountId::new(rotated_signer.public_key().clone());
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();

        let cursor_operation =
            RepairOperationV1::Action(claim_instruction("REP-UNSIGNED-CURSOR", 1, "cursor"));
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(
                authority.clone(),
                cursor_operation,
                &context(0, 0)
            ),
            Err(RepairTransactionForwarderError::InvalidFinalizedCursor)
        ));

        let mismatched_submit = RepairOperationV1::Submit(SubmitSorafsRepairTask::new(
            [0x93; 32],
            norito::to_bytes(&report(&rotated_authority, "REP-UNSIGNED-MISMATCH")).unwrap(),
        ));
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(
                authority.clone(),
                mismatched_submit,
                &context(1, 1)
            ),
            Err(RepairTransactionForwarderError::AuditorAuthorityMismatch)
        ));

        let operation =
            RepairOperationV1::Action(claim_instruction("REP-UNSIGNED-ID", 1, "stable-identity"));
        let operation_id = forwarder
            .enqueue_unsigned_operation(authority.clone(), operation.clone(), &context(1, 1))
            .unwrap()
            .operation_id();
        assert!(matches!(
            forwarder
                .enqueue_unsigned_operation(authority.clone(), operation.clone(), &context(2, 2))
                .unwrap(),
            RepairTransactionEnqueueResultV1::Existing {
                operation_id: existing
            } if existing == operation_id
        ));

        let conflicting =
            RepairOperationV1::Action(claim_instruction("REP-UNSIGNED-ID", 2, "stable-identity"));
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(authority, conflicting, &context(2, 2)),
            Err(RepairTransactionForwarderError::IdentityConflict)
        ));
        assert!(matches!(
            forwarder.enqueue_unsigned_operation(rotated_authority, operation, &context(2, 2)),
            Err(RepairTransactionForwarderError::IdentityConflict)
        ));

        let mut tiny_policy = policy();
        tiny_policy.max_transaction_bytes = 1;
        let tiny_forwarder = RepairTransactionForwarder::in_memory(tiny_policy).unwrap();
        assert!(matches!(
            tiny_forwarder.enqueue_unsigned_operation(
                AccountId::new(signer.public_key().clone()),
                RepairOperationV1::Action(claim_instruction("REP-UNSIGNED-BOUND", 1, "bounded")),
                &context(1, 1)
            ),
            Err(RepairTransactionForwarderError::ResourceLimitExceeded)
        ));
    }

    #[test]
    fn concurrent_unsigned_claim_has_exactly_one_durable_winner() {
        let signer = key(16);
        let authority = AccountId::new(signer.public_key().clone());
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let operation = RepairOperationV1::Action(claim_instruction(
            "REP-CONCURRENT-CLAIM",
            1,
            "concurrent-claim",
        ));
        let operation_id = forwarder
            .enqueue_unsigned_operation(authority, operation, &context(1, 1))
            .unwrap()
            .operation_id();
        let barrier = Arc::new(Barrier::new(4));
        let workers = (0..4)
            .map(|_| {
                let worker = forwarder.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    match worker.claim_for_signing(operation_id) {
                        Ok(request) => {
                            assert_eq!(request.operation_id, operation_id);
                            true
                        }
                        Err(RepairTransactionForwarderError::InvalidTransition) => false,
                        Err(error) => panic!("unexpected concurrent claim failure: {error:?}"),
                    }
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .filter(|won| *won)
                .count(),
            1
        );
        let pending = forwarder.pending(8).unwrap();
        assert_eq!(pending[0].state, RepairTransactionDeliveryStateV1::Signing);
        assert_eq!(pending[0].attempts, 1);
        assert!(pending[0].signed_transaction_bytes.is_none());
    }

    #[test]
    fn unsigned_signing_crash_recovers_ready_without_refunding_attempt() {
        let dir = TempDir::new().unwrap();
        let mut bounded = policy();
        bounded.max_attempts = 2;
        let signer = key(17);
        let authority = AccountId::new(signer.public_key().clone());
        let operation =
            RepairOperationV1::Action(claim_instruction("REP-UNSIGNED-CRASH", 1, "unsigned-crash"));
        let forwarder = RepairTransactionForwarder::open(dir.path(), bounded).unwrap();
        let operation_id = forwarder
            .enqueue_unsigned_operation(authority, operation.clone(), &context(7, 7))
            .unwrap()
            .operation_id();
        assert_eq!(
            forwarder.claim_for_signing(operation_id).unwrap().operation,
            operation
        );
        drop(forwarder);

        let restored = RepairTransactionForwarder::open(dir.path(), bounded).unwrap();
        let pending = restored.pending(8).unwrap();
        assert_eq!(pending[0].state, RepairTransactionDeliveryStateV1::Ready);
        assert_eq!(pending[0].attempts, 1);
        assert_eq!(pending[0].baseline_finalized_height, 7);
        assert_eq!(pending[0].baseline_finalized_block_hash, [7; 32]);
        assert!(pending[0].signed_transaction_bytes.is_none());
        assert_eq!(
            restored.claim_for_signing(operation_id).unwrap().operation,
            operation
        );
        restored.release_signing_claim(operation_id).unwrap();
        let released = restored.pending(8).unwrap();
        assert_eq!(released[0].state, RepairTransactionDeliveryStateV1::Ready);
        assert_eq!(released[0].attempts, 2);
        assert!(matches!(
            restored.claim_for_signing(operation_id),
            Err(RepairTransactionForwarderError::RetryExhausted)
        ));
    }

    #[test]
    fn signer_worker_rotation_preserves_request_and_rejects_wrong_authority_or_operation() {
        let signer = key(18);
        let wrong_signer = key(19);
        let authority = AccountId::new(signer.public_key().clone());
        let wrong_authority = AccountId::new(wrong_signer.public_key().clone());
        let mut bounded = policy();
        bounded.max_attempts = 3;
        let forwarder = RepairTransactionForwarder::in_memory(bounded).unwrap();
        let operation = RepairOperationV1::Action(claim_instruction(
            "REP-SIGNER-ROTATION",
            1,
            "signer-rotation",
        ));
        let operation_id = forwarder
            .enqueue_unsigned_operation(authority.clone(), operation.clone(), &context(1, 1))
            .unwrap()
            .operation_id();

        let first_worker = forwarder.claim_for_signing(operation_id).unwrap();
        forwarder.release_signing_claim(operation_id).unwrap();
        let rotated_worker = forwarder.claim_for_signing(operation_id).unwrap();
        assert_eq!(rotated_worker, first_worker);

        let wrong_authority_bytes = signed_bytes(
            &wrong_signer,
            wrong_authority,
            [InstructionBox::from(operation.clone())],
            1,
        );
        assert!(matches!(
            forwarder.store_signed_transaction(operation_id, &wrong_authority_bytes),
            Err(RepairTransactionForwarderError::InvalidSignedTransaction)
        ));
        let substituted = signed_repair_bytes(
            &signer,
            RepairOperationV1::Action(claim_instruction(
                "REP-SIGNER-ROTATION",
                2,
                "signer-rotation",
            )),
            2,
        );
        assert!(matches!(
            forwarder.store_signed_transaction(operation_id, &substituted),
            Err(RepairTransactionForwarderError::InvalidSignedTransaction)
        ));
        let still_claimed = forwarder.pending(8).unwrap();
        assert_eq!(
            still_claimed[0].state,
            RepairTransactionDeliveryStateV1::Signing
        );
        assert_eq!(still_claimed[0].attempts, 2);
        assert!(still_claimed[0].signed_transaction_bytes.is_none());

        let exact = signed_repair_bytes(&signer, rotated_worker.operation, 3);
        forwarder
            .store_signed_transaction(operation_id, &exact)
            .unwrap();
        assert_eq!(forwarder.begin_submission(operation_id).unwrap(), exact);
    }

    #[test]
    fn rejects_wrong_executable_multiple_instructions_and_invalid_signature() {
        let signer = key(2);
        let authority = AccountId::new(signer.public_key().clone());
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();

        let wrong = signed_bytes(
            &signer,
            authority.clone(),
            [InstructionBox::from(Log::new(
                Level::INFO,
                "not repair".to_owned(),
            ))],
            1,
        );
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&wrong, &context(1, 1)),
            Err(RepairTransactionForwarderError::InvalidSignedTransaction)
        ));

        let first = submit_instruction(&signer, [0x51; 32], "REP-MULTI-1");
        let second = submit_instruction(&signer, [0x52; 32], "REP-MULTI-2");
        let multiple = signed_bytes(
            &signer,
            authority.clone(),
            [InstructionBox::from(first), InstructionBox::from(second)],
            2,
        );
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&multiple, &context(1, 1)),
            Err(RepairTransactionForwarderError::InvalidSignedTransaction)
        ));

        let wrong_signer = key(3);
        let mut invalid_builder = TransactionBuilder::new(
            ChainId::from("repair-transaction-forwarder-test"),
            authority,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(submit_instruction(
            &signer,
            [0x53; 32],
            "REP-BAD-SIGNATURE",
        ))]);
        invalid_builder.set_creation_time(Duration::from_millis(3));
        let invalid_signature = Signature::try_new(
            wrong_signer.private_key(),
            &invalid_builder.payload_hash_bytes(),
        )
        .unwrap();
        let invalid_signature = invalid_builder.build_with_signature(invalid_signature);
        assert!(
            invalid_signature.verify_signature().is_err(),
            "fixture must be structurally valid but carry a signature from the wrong authority"
        );
        let invalid_signature = norito::to_bytes(&invalid_signature).unwrap();
        let decoded: SignedTransaction = norito::decode_from_bytes(&invalid_signature).unwrap();
        assert!(
            decoded.verify_signature().is_err(),
            "canonical round-trip must preserve the signature verification failure"
        );
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&invalid_signature, &context(1, 1)),
            Err(RepairTransactionForwarderError::InvalidSignedTransaction)
        ));
    }

    #[test]
    fn rejects_auditor_authority_mismatch_and_noncanonical_embedded_report() {
        let signer = key(4);
        let other = key(5);
        let signer_authority = AccountId::new(signer.public_key().clone());
        let other_authority = AccountId::new(other.public_key().clone());
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();

        let mismatch = SubmitSorafsRepairTask::new(
            [0x61; 32],
            norito::to_bytes(&report(&other_authority, "REP-AUDITOR-MISMATCH")).unwrap(),
        );
        let mismatch = signed_repair_bytes(&signer, mismatch, 1);
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&mismatch, &context(1, 1)),
            Err(RepairTransactionForwarderError::AuditorAuthorityMismatch)
        ));

        let compressed_report = norito::to_compressed_bytes(
            &report(&signer_authority, "REP-COMPRESSED"),
            Some(norito::CompressionConfig::default()),
        )
        .unwrap();
        let compressed = signed_repair_bytes(
            &signer,
            SubmitSorafsRepairTask::new([0x62; 32], compressed_report),
            2,
        );
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&compressed, &context(1, 1)),
            Err(RepairTransactionForwarderError::InvalidRepairOperation)
        ));
    }

    #[test]
    fn report_preflight_matches_the_native_canonical_payload_boundary() {
        let signer = key(18);
        let authority = AccountId::new(signer.public_key().clone());
        let (largest_accepted, first_rejected) =
            report_payloads_at_ledger_boundary(&report(&authority, "REP-PAYLOAD-BOUNDARY"));

        decode_repair_report(&largest_accepted)
            .expect("largest in-bound canonical report passes forwarder preflight");
        assert!(matches!(
            decode_repair_report(&first_rejected),
            Err(RepairTransactionForwarderError::InvalidRepairOperation)
        ));
        let slash_proposal = RepairSlashProposalV1 {
            version: sorafs_manifest::REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: RepairTicketId("REP-PAYLOAD-BOUNDARY".to_owned()),
            provider_id: [0x22; 32],
            manifest_digest: [0x11; 32],
            auditor_account: authority.to_string(),
            proposed_penalty: "0.000001".parse().expect("valid XOR quantity"),
            submitted_at_unix: 1,
            rationale: "bounded slash proposal".to_owned(),
            approval: None,
        };
        let slash_proposal =
            norito::to_bytes(&slash_proposal).expect("encode bounded slash proposal");
        assert!(slash_proposal.len() <= REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1);
        decode_slash_proposal(&slash_proposal)
            .expect("valid in-bound slash proposal passes forwarder preflight");
        let oversized_slash = vec![0_u8; REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1 + 1];
        assert!(matches!(
            decode_slash_proposal(&oversized_slash),
            Err(RepairTransactionForwarderError::InvalidRepairOperation)
        ));

        let unsigned_forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        assert!(matches!(
            unsigned_forwarder
                .enqueue_unsigned_operation(
                    authority.clone(),
                    RepairOperationV1::Submit(SubmitSorafsRepairTask::new(
                        [0xA1; 32],
                        largest_accepted.clone(),
                    )),
                    &context(1, 1),
                )
                .unwrap(),
            RepairTransactionEnqueueResultV1::Inserted { .. }
        ));
        assert!(matches!(
            unsigned_forwarder.enqueue_unsigned_operation(
                authority,
                RepairOperationV1::Submit(SubmitSorafsRepairTask::new(
                    [0xA2; 32],
                    first_rejected.clone(),
                )),
                &context(1, 1),
            ),
            Err(RepairTransactionForwarderError::InvalidRepairOperation)
        ));

        let signed_forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let accepted_transaction = signed_repair_bytes(
            &signer,
            SubmitSorafsRepairTask::new([0xA3; 32], largest_accepted),
            1,
        );
        assert!(matches!(
            signed_forwarder
                .enqueue_signed_transaction(&accepted_transaction, &context(1, 1))
                .unwrap(),
            RepairTransactionEnqueueResultV1::Inserted { .. }
        ));
        let rejected_transaction = signed_repair_bytes(
            &signer,
            SubmitSorafsRepairTask::new([0xA4; 32], first_rejected),
            2,
        );
        assert!(matches!(
            signed_forwarder.enqueue_signed_transaction(&rejected_transaction, &context(1, 1)),
            Err(RepairTransactionForwarderError::InvalidRepairOperation)
        ));
    }

    #[test]
    fn semantic_replay_preserves_first_exact_envelope_and_rejects_conflict() {
        let signer = key(6);
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let operation = claim_instruction("REP-REPLAY", 1, "stable-key");
        let first = signed_repair_bytes(&signer, operation.clone(), 1);
        let re_enveloped = signed_repair_bytes(&signer, operation, 2);
        assert_ne!(first, re_enveloped);
        let operation_id = forwarder
            .enqueue_signed_transaction(&first, &context(1, 1))
            .unwrap()
            .operation_id();
        assert!(matches!(
            forwarder
                .enqueue_signed_transaction(&re_enveloped, &context(2, 2))
                .unwrap(),
            RepairTransactionEnqueueResultV1::Existing { .. }
        ));
        assert_eq!(
            forwarder.pending(8).unwrap()[0]
                .signed_transaction_bytes
                .as_deref(),
            Some(first.as_slice())
        );

        let conflicting =
            signed_repair_bytes(&signer, claim_instruction("REP-REPLAY", 2, "stable-key"), 3);
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&conflicting, &context(2, 2)),
            Err(RepairTransactionForwarderError::IdentityConflict)
        ));
        assert_eq!(forwarder.pending(8).unwrap()[0].operation_id, operation_id);
    }

    #[test]
    fn crash_recovery_preserves_ambiguous_bytes_and_resets_only_signing() {
        let dir = TempDir::new().unwrap();
        let signer = key(7);
        let forwarder = RepairTransactionForwarder::open(dir.path(), policy()).unwrap();
        let ambiguous_bytes = signed_repair_bytes(
            &signer,
            claim_instruction("REP-AMBIGUOUS", 1, "ambiguous"),
            1,
        );
        let signing_bytes =
            signed_repair_bytes(&signer, claim_instruction("REP-SIGNING", 1, "signing"), 2);
        let ambiguous = forwarder
            .enqueue_signed_transaction(&ambiguous_bytes, &context(1, 1))
            .unwrap()
            .operation_id();
        let signing = forwarder
            .enqueue_signed_transaction(&signing_bytes, &context(1, 1))
            .unwrap()
            .operation_id();
        assert_eq!(
            forwarder.begin_submission(ambiguous).unwrap(),
            ambiguous_bytes
        );
        forwarder.begin_submission(signing).unwrap();
        forwarder
            .mark_transaction_rejected(signing, cursor(2, 2))
            .unwrap();
        forwarder.claim_for_signing(signing).unwrap();
        drop(forwarder);

        let restored = RepairTransactionForwarder::open(dir.path(), policy()).unwrap();
        let pending = restored.pending(8).unwrap();
        let ambiguous = pending
            .iter()
            .find(|entry| entry.operation_id == ambiguous)
            .unwrap();
        assert_eq!(ambiguous.state, RepairTransactionDeliveryStateV1::Ambiguous);
        assert_eq!(
            ambiguous.signed_transaction_bytes.as_deref(),
            Some(ambiguous_bytes.as_slice())
        );
        let signing = pending
            .iter()
            .find(|entry| entry.operation_id == signing)
            .unwrap();
        assert_eq!(signing.state, RepairTransactionDeliveryStateV1::Ready);
        assert!(signing.signed_transaction_bytes.is_none());
    }

    #[test]
    fn finalized_absence_deadletters_exact_bytes_atomically_across_restart() {
        let dir = TempDir::new().unwrap();
        let mut bounded = policy();
        bounded.max_attempts = 1;
        let signer = key(8);
        let bytes = signed_repair_bytes(&signer, claim_instruction("REP-DEAD", 1, "dead"), 1);
        let digest = transaction_digest(&bytes);
        let forwarder = RepairTransactionForwarder::open(dir.path(), bounded).unwrap();
        let operation = forwarder
            .enqueue_signed_transaction(&bytes, &context(1, 1))
            .unwrap()
            .operation_id();
        forwarder.begin_submission(operation).unwrap();
        forwarder
            .mark_finalized_absent(operation, cursor(2, 2))
            .unwrap();
        drop(forwarder);

        let restored = RepairTransactionForwarder::open(dir.path(), bounded).unwrap();
        assert!(restored.pending(8).unwrap().is_empty());
        let dead = restored.dead_letters(8).unwrap();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].operation_id, operation);
        assert_eq!(dead[0].transaction_digest, Some(digest));
        assert_eq!(
            dead[0].reason,
            RepairTransactionDeadLetterReasonV1::RetryExhausted
        );
        let stored = restored.state.lock().unwrap();
        assert_eq!(
            stored.checkpoint.dead_letters[0]
                .signed_transaction_bytes
                .as_deref(),
            Some(bytes.as_slice())
        );
    }

    #[test]
    fn deadletter_capacity_failure_keeps_pending_exact_bytes() {
        let mut bounded = policy();
        bounded.max_attempts = 1;
        bounded.max_dead_letters = 1;
        let signer = key(9);
        let forwarder = RepairTransactionForwarder::in_memory(bounded).unwrap();
        let first_bytes =
            signed_repair_bytes(&signer, claim_instruction("REP-CAP-1", 1, "cap-1"), 1);
        let second_bytes =
            signed_repair_bytes(&signer, claim_instruction("REP-CAP-2", 1, "cap-2"), 2);
        let first = forwarder
            .enqueue_signed_transaction(&first_bytes, &context(1, 1))
            .unwrap()
            .operation_id();
        let second = forwarder
            .enqueue_signed_transaction(&second_bytes, &context(1, 1))
            .unwrap()
            .operation_id();
        forwarder.begin_submission(first).unwrap();
        forwarder
            .mark_finalized_absent(first, cursor(2, 2))
            .unwrap();
        forwarder.begin_submission(second).unwrap();
        assert!(matches!(
            forwarder.mark_finalized_absent(second, cursor(2, 2)),
            Err(RepairTransactionForwarderError::DeadLetterCapacityExhausted)
        ));
        let pending = forwarder.pending(8).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].operation_id, second);
        assert_eq!(
            pending[0].state,
            RepairTransactionDeliveryStateV1::Ambiguous
        );
        assert_eq!(
            pending[0].signed_transaction_bytes.as_deref(),
            Some(second_bytes.as_slice())
        );
    }

    #[test]
    fn bounded_preflight_rejects_oversized_transaction_and_corrupt_checkpoint() {
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let oversized = vec![0_u8; policy().max_transaction_bytes + 1];
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&oversized, &context(1, 1)),
            Err(RepairTransactionForwarderError::InvalidSignedTransaction)
        ));
        assert!(forwarder.pending(8).unwrap().is_empty());

        let compressed_transaction = {
            let signer = key(10);
            let instruction = submit_instruction(&signer, [0x71; 32], "REP-COMPRESSED-TX");
            let authority = AccountId::new(signer.public_key().clone());
            let mut builder = TransactionBuilder::new(
                ChainId::from("repair-transaction-forwarder-test"),
                authority,
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([InstructionBox::from(instruction)]);
            builder.set_creation_time(Duration::from_millis(1));
            norito::to_compressed_bytes(
                &builder.try_sign(signer.private_key()).unwrap(),
                Some(norito::CompressionConfig::default()),
            )
            .unwrap()
        };
        assert!(matches!(
            forwarder.enqueue_signed_transaction(&compressed_transaction, &context(1, 1)),
            Err(RepairTransactionForwarderError::InvalidSignedTransaction)
        ));

        let dir = TempDir::new().unwrap();
        write_private_checkpoint(
            &dir.path()
                .join(REPAIR_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1),
            b"truncated checkpoint",
        );
        assert!(matches!(
            RepairTransactionForwarder::open(dir.path(), policy()),
            Err(RepairTransactionForwarderError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn replacement_rejects_semantic_substitution_and_preserves_new_exact_bytes() {
        let signer = key(11);
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let first = signed_repair_bytes(&signer, claim_instruction("REP-REPLACE", 1, "replace"), 1);
        let operation = forwarder
            .enqueue_signed_transaction(&first, &context(1, 1))
            .unwrap()
            .operation_id();
        forwarder.begin_submission(operation).unwrap();
        forwarder
            .mark_transaction_rejected(operation, cursor(2, 2))
            .unwrap();
        forwarder.claim_for_signing(operation).unwrap();

        let substituted = signed_repair_bytes(
            &signer,
            ApplySorafsRepairTaskAction::new(
                "REP-REPLACE".to_owned(),
                1,
                SorafsRepairTaskActionV1::Complete(SorafsRepairCompleteV1 {
                    lease_generation: 1,
                    evidence_digest: [0x81; 32],
                    idempotency_key: "replace".to_owned(),
                }),
            ),
            2,
        );
        assert!(matches!(
            forwarder.store_signed_transaction(operation, &substituted),
            Err(RepairTransactionForwarderError::InvalidSignedTransaction)
        ));

        let replacement =
            signed_repair_bytes(&signer, claim_instruction("REP-REPLACE", 1, "replace"), 3);
        let digest = forwarder
            .store_signed_transaction(operation, &replacement)
            .unwrap();
        assert_eq!(digest, transaction_digest(&replacement));
        assert_eq!(forwarder.begin_submission(operation).unwrap(), replacement);
    }

    #[test]
    fn finalized_reconciliation_requires_the_exact_active_transaction_digest() {
        let signer = key(12);
        let forwarder = RepairTransactionForwarder::in_memory(policy()).unwrap();
        let bytes = signed_repair_bytes(&signer, claim_instruction("REP-FINAL", 1, "final"), 1);
        let operation = forwarder
            .enqueue_signed_transaction(&bytes, &context(1, 1))
            .unwrap()
            .operation_id();
        assert!(matches!(
            forwarder.mark_finalized(operation, [0xFF; 32], cursor(2, 2)),
            Err(RepairTransactionForwarderError::InvalidSignedTransaction)
        ));
        forwarder
            .mark_finalized(operation, transaction_digest(&bytes), cursor(2, 2))
            .unwrap();
        assert!(forwarder.pending(8).unwrap().is_empty());
        assert!(matches!(
            forwarder
                .enqueue_signed_transaction(&bytes, &context(3, 3))
                .unwrap(),
            RepairTransactionEnqueueResultV1::Existing { .. }
        ));
    }
}
