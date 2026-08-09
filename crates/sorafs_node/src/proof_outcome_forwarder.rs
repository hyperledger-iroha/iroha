//! Durable delivery outbox for chain-authoritative SoraFS proof outcomes.
//!
//! PDP and PoTR provider state remains the source for the canonical proof
//! material until a corresponding [`SubmitSorafsProofOutcome`] transaction is
//! finalized.  This module retains only bounded delivery metadata and the exact
//! signed transaction selected by the runtime signer.  It never projects an
//! outcome independently of finalized ledger state.

use std::{
    collections::BTreeSet,
    path::Path,
    sync::{Arc, Mutex},
};

use iroha_data_model::{
    isi::sorafs::{
        SorafsPdpProofOutcomeSubmissionV1, SorafsPotrProofOutcomeSubmissionV1,
        SorafsProofOutcomeSubmissionV1, SubmitSorafsProofOutcome,
    },
    sorafs::{
        capacity::ProviderId,
        pin_registry::ManifestDigest,
        proof_ledger::{
            PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1, ProofOutcomeFinalizedCursorV1,
            ProofOutcomeFinalizedRecordV1, ProofOutcomeKindV1, ProofOutcomeRecordV1,
        },
    },
    transaction::{Executable, SignedTransaction},
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::{
    PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1, PdpGovernanceArchiveV1, PotrReceiptV1,
};
use thiserror::Error;

#[cfg(test)]
use crate::durable_transaction_forwarder::CheckpointWriterGuard;
use crate::durable_transaction_forwarder::{
    self as durable, AtomicCheckpointStore, CheckpointStoreError, DeliveryRecord,
    DeliveryTransitionError, FinalizedCursorV1, RetryBoundOutcome, StoredDeliveryStateV1,
};

/// Durable outbox checkpoint schema version.
pub const PROOF_OUTCOME_OUTBOX_CHECKPOINT_VERSION_V1: u8 = 1;
/// File containing the canonical proof-outcome delivery checkpoint.
pub const PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1: &str = "proof-outcome-forwarder-state.to";
/// Default bounded number of delivery attempts for one unchanged signed transaction.
pub const PROOF_OUTCOME_OUTBOX_DEFAULT_MAX_ATTEMPTS_V1: u32 = 8;
/// Maximum number of entries returned by one worker scan.
pub const PROOF_OUTCOME_OUTBOX_MAX_SCAN_ITEMS_V1: usize = 1_000;

const CHECKPOINT_LOCK_FILE_NAME: &str = "proof-outcome-forwarder-state.lock";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"sorafs.proof-outcome.forwarder.operation.v1\0";
const CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT: usize = 4;
const CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT: usize = 16;
// Maximum-valid PDP/PoTR state requires 463,560 bytes beyond 16x wire.
// Round up to eight 64 KiB quanta and retain one further quantum as margin.
const CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 9 * 64 * 1024;
const CHECKPOINT_MAX_NESTING_DEPTH: usize = 128;
const PDP_ARCHIVE_DECODE_LIMITS: norito::DecodeLimits = norito::DecodeLimits::new(
    128 * 1024,
    PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1,
    512 * 1024,
    4 * PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1 + 64 * 1024,
    128,
);
const POTR_RECEIPT_DECODE_LIMITS: norito::DecodeLimits = norito::DecodeLimits::new(
    PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
    PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
    2 * PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
    4 * PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1 + 4 * 1024,
    32,
);

/// Bounded persistence and retry policy for the delivery outbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofOutcomeOutboxPolicyV1 {
    /// Maximum pending deliveries.
    pub max_pending: usize,
    /// Maximum finalized idempotency tombstones.
    pub max_completed: usize,
    /// Maximum terminal dead letters.
    pub max_dead_letters: usize,
    /// Maximum submission attempts after finalized absence is proven.
    pub max_attempts: u32,
    /// Maximum canonical checkpoint bytes.
    pub checkpoint_max_bytes: u64,
}

impl ProofOutcomeOutboxPolicyV1 {
    /// Validate the bounded first-release policy.
    pub fn validate(self) -> Result<(), ProofOutcomeOutboxError> {
        if self.max_pending == 0
            || self.max_completed == 0
            || self.max_dead_letters == 0
            || self.max_attempts == 0
            || self.checkpoint_max_bytes == 0
        {
            return Err(ProofOutcomeOutboxError::InvalidPolicy);
        }
        Ok(())
    }
}

/// Durable enqueue result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofOutcomeEnqueueResultV1 {
    /// A new exact delivery was persisted.
    Inserted {
        /// Stable semantic operation identity.
        operation_id: [u8; 32],
    },
    /// The same exact delivery was already pending or finalized.
    Existing {
        /// Stable semantic operation identity.
        operation_id: [u8; 32],
    },
}

impl ProofOutcomeEnqueueResultV1 {
    /// Return the stable operation identity.
    #[must_use]
    pub const fn operation_id(self) -> [u8; 32] {
        match self {
            Self::Inserted { operation_id } | Self::Existing { operation_id } => operation_id,
        }
    }
}

/// Runtime-visible durable delivery state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofOutcomeDeliveryStateV1 {
    /// Canonical material is ready for a runtime signer.
    Ready,
    /// A signer claim is in progress; no submission can have occurred yet.
    Signing,
    /// An exact signed transaction is durable and ready for submission.
    Signed,
    /// Submission may have occurred and must be reconciled before retry.
    Ambiguous,
    /// The exact transaction is known pending or applied but not finalized as an outcome.
    Submitted,
}

impl From<StoredDeliveryStateV1> for ProofOutcomeDeliveryStateV1 {
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

/// Exact pending delivery returned to the Torii worker.
#[derive(Debug, Clone)]
pub struct ProofOutcomePendingDeliveryV1 {
    /// Insertion sequence.
    pub sequence: u64,
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Protocol kind.
    pub kind: ProofOutcomeKindV1,
    /// Protocol-scoped identity.
    pub identity_digest: [u8; 32],
    /// Digest of the canonical archive or signed receipt.
    pub outcome_digest: [u8; 32],
    /// Provider identity.
    pub provider_id: ProviderId,
    /// Manifest identity.
    pub manifest_digest: ManifestDigest,
    /// Council admission envelope identity.
    pub admission_envelope_digest: [u8; 32],
    /// Exact native instruction payload.
    pub submission: SorafsProofOutcomeSubmissionV1,
    /// Current durable delivery state.
    pub state: ProofOutcomeDeliveryStateV1,
    /// Number of signer/submission attempts.
    pub attempts: u32,
    /// Finalized height observed before the current ambiguous attempt.
    pub baseline_finalized_height: u64,
    /// Finalized block hash paired with the baseline height.
    pub baseline_finalized_block_hash: [u8; 32],
    /// Exact signed transaction retained before queue submission.
    pub signed_transaction: Option<SignedTransaction>,
}

/// Reason retained for a payload-free terminal dead letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofOutcomeDeadLetterReasonV1 {
    /// Finalized chain state contains different cryptographic material for the identity.
    FinalizedConflict,
    /// The exact signed transaction was rejected or expired terminally.
    TransactionRejected,
    /// Bounded retries were exhausted after finalized absence was proven.
    RetryExhausted,
}

/// Payload-free terminal delivery record for operator reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofOutcomeDeadLetterV1 {
    /// Stable semantic operation identity.
    pub operation_id: [u8; 32],
    /// Protocol kind.
    pub kind: ProofOutcomeKindV1,
    /// Protocol-scoped identity.
    pub identity_digest: [u8; 32],
    /// Digest of the canonical archive or signed receipt.
    pub outcome_digest: [u8; 32],
    /// Terminal reason.
    pub reason: ProofOutcomeDeadLetterReasonV1,
    /// Finalized height at which the terminal condition was observed.
    pub observed_finalized_height: u64,
    /// Finalized block hash paired with the observed height.
    pub observed_finalized_block_hash: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredDeadLetterReasonV1 {
    FinalizedConflict,
    TransactionRejected,
    RetryExhausted,
}

impl From<ProofOutcomeDeadLetterReasonV1> for StoredDeadLetterReasonV1 {
    fn from(value: ProofOutcomeDeadLetterReasonV1) -> Self {
        match value {
            ProofOutcomeDeadLetterReasonV1::FinalizedConflict => Self::FinalizedConflict,
            ProofOutcomeDeadLetterReasonV1::TransactionRejected => Self::TransactionRejected,
            ProofOutcomeDeadLetterReasonV1::RetryExhausted => Self::RetryExhausted,
        }
    }
}

impl From<StoredDeadLetterReasonV1> for ProofOutcomeDeadLetterReasonV1 {
    fn from(value: StoredDeadLetterReasonV1) -> Self {
        match value {
            StoredDeadLetterReasonV1::FinalizedConflict => Self::FinalizedConflict,
            StoredDeadLetterReasonV1::TransactionRejected => Self::TransactionRejected,
            StoredDeadLetterReasonV1::RetryExhausted => Self::RetryExhausted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredPendingDeliveryV1 {
    sequence: u64,
    operation_id: [u8; 32],
    kind: ProofOutcomeKindV1,
    identity_digest: [u8; 32],
    outcome_digest: [u8; 32],
    provider_id: ProviderId,
    manifest_digest: ManifestDigest,
    admission_envelope_digest: [u8; 32],
    submission: SorafsProofOutcomeSubmissionV1,
    state: StoredDeliveryStateV1,
    attempts: u32,
    baseline_finalized_height: u64,
    baseline_finalized_block_hash: [u8; 32],
    signed_transaction: Option<SignedTransaction>,
}

impl StoredPendingDeliveryV1 {
    fn snapshot(&self) -> ProofOutcomePendingDeliveryV1 {
        ProofOutcomePendingDeliveryV1 {
            sequence: self.sequence,
            operation_id: self.operation_id,
            kind: self.kind,
            identity_digest: self.identity_digest,
            outcome_digest: self.outcome_digest,
            provider_id: self.provider_id,
            manifest_digest: self.manifest_digest,
            admission_envelope_digest: self.admission_envelope_digest,
            submission: self.submission.clone(),
            state: self.state.into(),
            attempts: self.attempts,
            baseline_finalized_height: self.baseline_finalized_height,
            baseline_finalized_block_hash: self.baseline_finalized_block_hash,
            signed_transaction: self.signed_transaction.clone(),
        }
    }
}

impl DeliveryRecord for StoredPendingDeliveryV1 {
    type Transaction = SignedTransaction;

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
        self.signed_transaction.as_ref()
    }

    fn set_signed_transaction(&mut self, transaction: Option<Self::Transaction>) {
        self.signed_transaction = transaction;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredCompletedDeliveryV1 {
    operation_id: [u8; 32],
    kind: ProofOutcomeKindV1,
    identity_digest: [u8; 32],
    outcome_digest: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredDeadLetterV1 {
    operation_id: [u8; 32],
    kind: ProofOutcomeKindV1,
    identity_digest: [u8; 32],
    outcome_digest: [u8; 32],
    provider_id: ProviderId,
    manifest_digest: ManifestDigest,
    admission_envelope_digest: [u8; 32],
    submission: SorafsProofOutcomeSubmissionV1,
    reason: StoredDeadLetterReasonV1,
    observed_finalized_height: u64,
    observed_finalized_block_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ProofOutcomeOutboxCheckpointV1 {
    version: u8,
    next_sequence: u64,
    pending: Vec<StoredPendingDeliveryV1>,
    completed: Vec<StoredCompletedDeliveryV1>,
    dead_letters: Vec<StoredDeadLetterV1>,
}

impl Default for ProofOutcomeOutboxCheckpointV1 {
    fn default() -> Self {
        Self {
            version: PROOF_OUTCOME_OUTBOX_CHECKPOINT_VERSION_V1,
            next_sequence: 1,
            pending: Vec::new(),
            completed: Vec::new(),
            dead_letters: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct DurableState {
    checkpoint: ProofOutcomeOutboxCheckpointV1,
    fingerprint: Option<[u8; 32]>,
    durability_failure: Option<String>,
}

/// Durable, bounded delivery outbox.
#[derive(Debug, Clone)]
pub struct ProofOutcomeOutbox {
    policy: ProofOutcomeOutboxPolicyV1,
    state: Arc<Mutex<DurableState>>,
    store: Option<Arc<CheckpointStore>>,
}

impl ProofOutcomeOutbox {
    /// Construct a bounded non-persistent outbox for unit tests.
    #[cfg(test)]
    fn in_memory(policy: ProofOutcomeOutboxPolicyV1) -> Result<Self, ProofOutcomeOutboxError> {
        policy.validate()?;
        Ok(Self {
            policy,
            state: Arc::new(Mutex::new(DurableState {
                checkpoint: ProofOutcomeOutboxCheckpointV1::default(),
                fingerprint: None,
                durability_failure: None,
            })),
            store: None,
        })
    }

    /// Open or create a durable outbox below `state_dir`.
    pub fn open(
        state_dir: &Path,
        policy: ProofOutcomeOutboxPolicyV1,
    ) -> Result<Self, ProofOutcomeOutboxError> {
        policy.validate()?;
        let store = Arc::new(CheckpointStore::new(
            state_dir,
            policy.checkpoint_max_bytes,
        )?);
        let (mut checkpoint, fingerprint) = store.load(policy)?;
        // `Signing` means the signer call had not yet returned.  The signer is
        // not authorized to submit, so this crash state is provably safe to
        // return to `Ready` without a ledger lookup.
        let mut recovered = false;
        for entry in &mut checkpoint.pending {
            recovered |= durable::recover_interrupted_signing(entry);
        }
        let outbox = Self {
            policy,
            state: Arc::new(Mutex::new(DurableState {
                checkpoint,
                fingerprint,
                durability_failure: None,
            })),
            store: Some(store),
        };
        if recovered {
            let mut state = outbox.lock_state()?;
            let candidate = state.checkpoint.clone();
            outbox.commit_candidate(&mut state, candidate)?;
        }
        Ok(outbox)
    }

    /// Persist one exact PDP governance archive before local terminal handoff completes.
    pub fn enqueue_pdp(
        &self,
        archive: &PdpGovernanceArchiveV1,
    ) -> Result<ProofOutcomeEnqueueResultV1, ProofOutcomeOutboxError> {
        archive
            .validate()
            .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
        let archive_payload =
            norito::to_bytes(archive).map_err(ProofOutcomeOutboxError::CanonicalEncoding)?;
        let outcome_digest = archive
            .digest()
            .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
        self.enqueue(PreparedDelivery {
            kind: ProofOutcomeKindV1::Pdp,
            identity_digest: archive.challenge_id,
            outcome_digest,
            provider_id: ProviderId::new(archive.provider_id),
            manifest_digest: ManifestDigest::new(archive.manifest_digest),
            admission_envelope_digest: archive.admission_envelope_digest,
            submission: SorafsProofOutcomeSubmissionV1::Pdp(SorafsPdpProofOutcomeSubmissionV1 {
                archive_payload,
            }),
        })
    }

    /// Persist one exact governed PoTR receipt before local terminal handoff completes.
    pub fn enqueue_potr(
        &self,
        receipt: &PotrReceiptV1,
        admission_envelope_digest: [u8; 32],
    ) -> Result<ProofOutcomeEnqueueResultV1, ProofOutcomeOutboxError> {
        self.enqueue(prepare_potr_delivery(receipt, admission_envelope_digest)?)
    }

    /// Return pending entries in stable sequence order.
    pub fn pending(
        &self,
        limit: usize,
    ) -> Result<Vec<ProofOutcomePendingDeliveryV1>, ProofOutcomeOutboxError> {
        self.pending_after(None, limit)
    }

    /// Return a circular page of pending entries after an immutable sequence cursor.
    ///
    /// Entries with a sequence greater than `after_sequence` are returned first,
    /// followed by the oldest entries when the page wraps.  At most one snapshot
    /// of each pending entry is returned.
    pub fn pending_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<ProofOutcomePendingDeliveryV1>, ProofOutcomeOutboxError> {
        if limit == 0 || limit > PROOF_OUTCOME_OUTBOX_MAX_SCAN_ITEMS_V1 {
            return Err(ProofOutcomeOutboxError::InvalidScanLimit);
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
            .map(StoredPendingDeliveryV1::snapshot)
            .collect())
    }

    /// Return payload-free dead letters in stable operation order.
    pub fn dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<ProofOutcomeDeadLetterV1>, ProofOutcomeOutboxError> {
        if limit == 0 || limit > PROOF_OUTCOME_OUTBOX_MAX_SCAN_ITEMS_V1 {
            return Err(ProofOutcomeOutboxError::InvalidScanLimit);
        }
        let state = self.lock_state()?;
        Ok(state
            .checkpoint
            .dead_letters
            .iter()
            .take(limit)
            .map(|entry| ProofOutcomeDeadLetterV1 {
                operation_id: entry.operation_id,
                kind: entry.kind,
                identity_digest: entry.identity_digest,
                outcome_digest: entry.outcome_digest,
                reason: entry.reason.into(),
                observed_finalized_height: entry.observed_finalized_height,
                observed_finalized_block_hash: entry.observed_finalized_block_hash,
            })
            .collect())
    }

    /// Restore an explicitly selected dead letter for operator-controlled replay.
    ///
    /// The expected outcome digest prevents a stale administrative command from
    /// reviving a different cryptographic outcome under the same operation id.
    pub fn retry_dead_letter(
        &self,
        operation_id: [u8; 32],
        expected_outcome_digest: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        let mut state = self.lock_state()?;
        if state.checkpoint.pending.len() >= self.policy.max_pending {
            return Err(ProofOutcomeOutboxError::PendingCapacityExhausted);
        }
        let mut candidate = state.checkpoint.clone();
        let position = candidate
            .dead_letters
            .iter()
            .position(|entry| {
                entry.operation_id == operation_id
                    && entry.outcome_digest == expected_outcome_digest
            })
            .ok_or(ProofOutcomeOutboxError::UnknownOperation)?;
        let dead = candidate.dead_letters.remove(position);
        let sequence = candidate.next_sequence;
        candidate.next_sequence = sequence
            .checked_add(1)
            .ok_or(ProofOutcomeOutboxError::SequenceExhausted)?;
        candidate.pending.push(StoredPendingDeliveryV1 {
            sequence,
            operation_id: dead.operation_id,
            kind: dead.kind,
            identity_digest: dead.identity_digest,
            outcome_digest: dead.outcome_digest,
            provider_id: dead.provider_id,
            manifest_digest: dead.manifest_digest,
            admission_envelope_digest: dead.admission_envelope_digest,
            submission: dead.submission,
            state: StoredDeliveryStateV1::Ready,
            attempts: 0,
            baseline_finalized_height: 0,
            baseline_finalized_block_hash: [0; 32],
            signed_transaction: None,
        });
        candidate.pending.sort_by_key(|entry| entry.sequence);
        self.commit_candidate(&mut state, candidate)
    }

    /// Claim a ready entry for isolated runtime signing.
    ///
    /// The transition is durable before the signer is invoked.  The signer
    /// receives no queue capability, so a crash in `Signing` is safe to replay.
    pub fn claim_for_signing(
        &self,
        operation_id: [u8; 32],
        baseline_finalized_cursor: ProofOutcomeFinalizedCursorV1,
    ) -> Result<ProofOutcomePendingDeliveryV1, ProofOutcomeOutboxError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let entry = find_pending_mut(&mut candidate, operation_id)?;
        durable::claim_for_signing(
            entry,
            finalized_cursor(baseline_finalized_cursor),
            self.policy.max_attempts,
        )?;
        let snapshot = entry.snapshot();
        self.commit_candidate(&mut state, candidate)?;
        Ok(snapshot)
    }

    /// Persist the exact signed transaction before exposing it to the queue.
    pub fn store_signed_transaction(
        &self,
        operation_id: [u8; 32],
        transaction: SignedTransaction,
    ) -> Result<[u8; 32], ProofOutcomeOutboxError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let entry = find_pending_mut(&mut candidate, operation_id)?;
        validate_signed_transaction(entry, &transaction)?;
        let transaction_hash = *transaction.hash().as_ref();
        if transaction_hash == [0; 32] {
            return Err(ProofOutcomeOutboxError::InvalidSignedTransaction);
        }
        durable::store_signed_transaction(entry, transaction)?;
        self.commit_candidate(&mut state, candidate)?;
        Ok(transaction_hash)
    }

    /// Release an isolated signing claim after a signer failure.
    ///
    /// No signed transaction or queue capability existed in this state, so
    /// returning to `Ready` is exact and does not consume a submission retry.
    pub fn release_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.mutate_entry(operation_id, |entry| {
            durable::release_signing_claim(entry).map_err(Into::into)
        })
    }

    /// Mark a durable signed transaction ambiguous before queue submission.
    pub fn begin_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<SignedTransaction, ProofOutcomeOutboxError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let entry = find_pending_mut(&mut candidate, operation_id)?;
        let transaction = durable::begin_submission(entry)?;
        self.commit_candidate(&mut state, candidate)?;
        Ok(transaction)
    }

    /// Record that the exact signed transaction is pending or applied.
    pub fn mark_submitted(&self, operation_id: [u8; 32]) -> Result<(), ProofOutcomeOutboxError> {
        self.mutate_entry(operation_id, |entry| {
            durable::mark_submitted(entry).map_err(Into::into)
        })
    }

    /// Record a queue failure known to have happened before submission.
    pub fn mark_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.mutate_entry(operation_id, |entry| {
            durable::mark_not_submitted(entry).map_err(Into::into)
        })
    }

    /// Permit retry of the same signed transaction after finalized absence is proven.
    pub fn mark_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ProofOutcomeFinalizedCursorV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
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

    /// Reconcile one exact finalized ledger outcome and retain a bounded tombstone.
    pub fn mark_finalized(
        &self,
        operation_id: [u8; 32],
        finalized: &ProofOutcomeFinalizedRecordV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
        let entry = &candidate.pending[position];
        if finalized.finalized_cursor.height == 0
            || finalized.finalized_cursor.block_hash == [0; 32]
        {
            return Err(ProofOutcomeOutboxError::InvalidFinalizedCursor);
        }
        if !finalized_matches(entry, &finalized.outcome) {
            self.move_to_dead_letter(
                &mut candidate,
                position,
                StoredDeadLetterReasonV1::FinalizedConflict,
                finalized.finalized_cursor,
            )?;
            self.commit_candidate(&mut state, candidate)?;
            return Err(ProofOutcomeOutboxError::FinalizedConflict);
        }
        if candidate.completed.len() >= self.policy.max_completed {
            let oldest = candidate
                .completed
                .iter()
                .enumerate()
                .min_by_key(|(_, completed)| (completed.finalized_height, completed.operation_id))
                .map(|(index, _)| index)
                .ok_or(ProofOutcomeOutboxError::InvalidCheckpoint)?;
            candidate.completed.remove(oldest);
        }
        let entry = candidate.pending.remove(position);
        candidate.completed.push(StoredCompletedDeliveryV1 {
            operation_id: entry.operation_id,
            kind: entry.kind,
            identity_digest: entry.identity_digest,
            outcome_digest: entry.outcome_digest,
            finalized_height: finalized.finalized_cursor.height,
            finalized_block_hash: finalized.finalized_cursor.block_hash,
        });
        candidate
            .completed
            .sort_by_key(|completed| completed.operation_id);
        self.commit_candidate(&mut state, candidate)
    }

    /// Re-sign an exact rejected or expired operation, or dead-letter at the bound.
    ///
    /// A terminal pipeline rejection proves the prior transaction cannot later
    /// commit. Discarding only that signed envelope permits runtime signer
    /// rotation while retaining the same semantic operation identity.
    pub fn mark_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ProofOutcomeFinalizedCursorV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        validate_finalized_cursor(observed_finalized_cursor)?;
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        let position = pending_position(&candidate, operation_id)?;
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
        }
        self.commit_candidate(&mut state, candidate)
    }

    fn enqueue(
        &self,
        prepared: PreparedDelivery,
    ) -> Result<ProofOutcomeEnqueueResultV1, ProofOutcomeOutboxError> {
        prepared.validate()?;
        let operation_id = operation_id(&prepared)?;
        let mut state = self.lock_state()?;
        if let Some(existing) = state.checkpoint.pending.iter().find(|entry| {
            entry.kind == prepared.kind && entry.identity_digest == prepared.identity_digest
        }) {
            if existing.operation_id == operation_id
                && existing.outcome_digest == prepared.outcome_digest
            {
                return Ok(ProofOutcomeEnqueueResultV1::Existing { operation_id });
            }
            return Err(ProofOutcomeOutboxError::IdentityConflict);
        }
        if let Some(existing) = state.checkpoint.completed.iter().find(|entry| {
            entry.kind == prepared.kind && entry.identity_digest == prepared.identity_digest
        }) {
            if existing.operation_id == operation_id
                && existing.outcome_digest == prepared.outcome_digest
            {
                return Ok(ProofOutcomeEnqueueResultV1::Existing { operation_id });
            }
            return Err(ProofOutcomeOutboxError::IdentityConflict);
        }
        if state.checkpoint.dead_letters.iter().any(|entry| {
            entry.kind == prepared.kind && entry.identity_digest == prepared.identity_digest
        }) {
            return Err(ProofOutcomeOutboxError::DeadLetterConflict);
        }
        if state.checkpoint.pending.len() >= self.policy.max_pending {
            return Err(ProofOutcomeOutboxError::PendingCapacityExhausted);
        }
        let sequence = state.checkpoint.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(ProofOutcomeOutboxError::SequenceExhausted)?;
        let mut candidate = state.checkpoint.clone();
        candidate.next_sequence = next_sequence;
        candidate.pending.push(StoredPendingDeliveryV1 {
            sequence,
            operation_id,
            kind: prepared.kind,
            identity_digest: prepared.identity_digest,
            outcome_digest: prepared.outcome_digest,
            provider_id: prepared.provider_id,
            manifest_digest: prepared.manifest_digest,
            admission_envelope_digest: prepared.admission_envelope_digest,
            submission: prepared.submission,
            state: StoredDeliveryStateV1::Ready,
            attempts: 0,
            baseline_finalized_height: 0,
            baseline_finalized_block_hash: [0; 32],
            signed_transaction: None,
        });
        candidate.pending.sort_by_key(|entry| entry.sequence);
        self.commit_candidate(&mut state, candidate)?;
        Ok(ProofOutcomeEnqueueResultV1::Inserted { operation_id })
    }

    fn mutate_entry(
        &self,
        operation_id: [u8; 32],
        mutate: impl FnOnce(&mut StoredPendingDeliveryV1) -> Result<(), ProofOutcomeOutboxError>,
    ) -> Result<(), ProofOutcomeOutboxError> {
        let mut state = self.lock_state()?;
        let mut candidate = state.checkpoint.clone();
        mutate(find_pending_mut(&mut candidate, operation_id)?)?;
        self.commit_candidate(&mut state, candidate)
    }

    fn move_to_dead_letter(
        &self,
        checkpoint: &mut ProofOutcomeOutboxCheckpointV1,
        position: usize,
        reason: StoredDeadLetterReasonV1,
        observed_finalized_cursor: ProofOutcomeFinalizedCursorV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        validate_finalized_cursor(observed_finalized_cursor)?;
        if checkpoint.dead_letters.len() >= self.policy.max_dead_letters {
            return Err(ProofOutcomeOutboxError::DeadLetterCapacityExhausted);
        }
        let entry = checkpoint.pending.remove(position);
        checkpoint.dead_letters.push(StoredDeadLetterV1 {
            operation_id: entry.operation_id,
            kind: entry.kind,
            identity_digest: entry.identity_digest,
            outcome_digest: entry.outcome_digest,
            provider_id: entry.provider_id,
            manifest_digest: entry.manifest_digest,
            admission_envelope_digest: entry.admission_envelope_digest,
            submission: entry.submission,
            reason,
            observed_finalized_height: observed_finalized_cursor.height,
            observed_finalized_block_hash: observed_finalized_cursor.block_hash,
        });
        checkpoint
            .dead_letters
            .sort_by_key(|dead| dead.operation_id);
        Ok(())
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, DurableState>, ProofOutcomeOutboxError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ProofOutcomeOutboxError::RuntimePoisoned)?;
        if state.durability_failure.is_some() {
            return Err(ProofOutcomeOutboxError::DurabilityPoisoned);
        }
        Ok(state)
    }

    fn commit_candidate(
        &self,
        state: &mut DurableState,
        candidate: ProofOutcomeOutboxCheckpointV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        validate_checkpoint(&candidate, self.policy)?;
        if let Some(store) = self.store.as_ref() {
            match store.commit(&candidate, state.fingerprint) {
                Ok(fingerprint) => state.fingerprint = Some(fingerprint),
                Err(error) => {
                    if matches!(
                        error,
                        ProofOutcomeOutboxError::CheckpointDurabilityUncertain
                    ) {
                        state.durability_failure =
                            Some("checkpoint durability uncertain".to_owned());
                    }
                    return Err(error);
                }
            }
        }
        state.checkpoint = candidate;
        Ok(())
    }
}

struct PreparedDelivery {
    kind: ProofOutcomeKindV1,
    identity_digest: [u8; 32],
    outcome_digest: [u8; 32],
    provider_id: ProviderId,
    manifest_digest: ManifestDigest,
    admission_envelope_digest: [u8; 32],
    submission: SorafsProofOutcomeSubmissionV1,
}

fn prepare_potr_delivery(
    receipt: &PotrReceiptV1,
    admission_envelope_digest: [u8; 32],
) -> Result<PreparedDelivery, ProofOutcomeOutboxError> {
    receipt
        .validate()
        .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
    if admission_envelope_digest == [0; 32] {
        return Err(ProofOutcomeOutboxError::InvalidSubmission);
    }
    let receipt_payload = receipt
        .signed_receipt_bytes()
        .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
    let outcome_digest = receipt
        .signed_receipt_digest()
        .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
    let identity_digest = receipt
        .request_scope_digest()
        .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
    Ok(PreparedDelivery {
        kind: ProofOutcomeKindV1::Potr,
        identity_digest,
        outcome_digest,
        provider_id: ProviderId::new(receipt.provider_id),
        manifest_digest: ManifestDigest::new(receipt.manifest_digest),
        admission_envelope_digest,
        submission: SorafsProofOutcomeSubmissionV1::Potr(SorafsPotrProofOutcomeSubmissionV1 {
            receipt_payload,
            admission_envelope_digest,
        }),
    })
}

/// Derive the canonical durable outbox operation identity for one governed
/// final PoTR receipt.
///
/// Exactly-once handoff clients use this value to authenticate the
/// acknowledgement returned by the proof-outcome outbox.
///
/// # Errors
///
/// Rejects an invalid receipt, a zero admission-envelope digest, or canonical
/// encoding failure.
pub fn potr_proof_outcome_operation_id_v1(
    receipt: &PotrReceiptV1,
    admission_envelope_digest: [u8; 32],
) -> Result<[u8; 32], ProofOutcomeOutboxError> {
    operation_id(&prepare_potr_delivery(receipt, admission_envelope_digest)?)
}

impl PreparedDelivery {
    fn validate(&self) -> Result<(), ProofOutcomeOutboxError> {
        if self.identity_digest == [0; 32]
            || self.outcome_digest == [0; 32]
            || self.provider_id.as_bytes() == &[0; 32]
            || self.manifest_digest.as_bytes() == &[0; 32]
            || self.admission_envelope_digest == [0; 32]
            || !matches!(
                (self.kind, &self.submission),
                (
                    ProofOutcomeKindV1::Pdp,
                    SorafsProofOutcomeSubmissionV1::Pdp(_)
                ) | (
                    ProofOutcomeKindV1::Potr,
                    SorafsProofOutcomeSubmissionV1::Potr(_)
                )
            )
        {
            return Err(ProofOutcomeOutboxError::InvalidSubmission);
        }
        Ok(())
    }

    fn validate_source_binding(&self) -> Result<(), ProofOutcomeOutboxError> {
        self.validate()?;
        match &self.submission {
            SorafsProofOutcomeSubmissionV1::Pdp(submission) => {
                let archive = decode_pdp_archive_source(&submission.archive_payload)?;
                archive
                    .validate()
                    .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
                if archive.challenge_id != self.identity_digest
                    || archive
                        .digest()
                        .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?
                        != self.outcome_digest
                    || ProviderId::new(archive.provider_id) != self.provider_id
                    || ManifestDigest::new(archive.manifest_digest) != self.manifest_digest
                    || archive.admission_envelope_digest != self.admission_envelope_digest
                {
                    return Err(ProofOutcomeOutboxError::InvalidSubmission);
                }
            }
            SorafsProofOutcomeSubmissionV1::Potr(submission) => {
                let receipt = decode_potr_receipt_source(&submission.receipt_payload)?;
                receipt
                    .validate()
                    .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
                if receipt
                    .request_scope_digest()
                    .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?
                    != self.identity_digest
                    || receipt
                        .signed_receipt_digest()
                        .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?
                        != self.outcome_digest
                    || ProviderId::new(receipt.provider_id) != self.provider_id
                    || ManifestDigest::new(receipt.manifest_digest) != self.manifest_digest
                    || submission.admission_envelope_digest != self.admission_envelope_digest
                {
                    return Err(ProofOutcomeOutboxError::InvalidSubmission);
                }
            }
        }
        Ok(())
    }
}

fn decode_pdp_archive_source(
    bytes: &[u8],
) -> Result<PdpGovernanceArchiveV1, ProofOutcomeOutboxError> {
    if bytes.is_empty() || bytes.len() > PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1 {
        return Err(ProofOutcomeOutboxError::InvalidSubmission);
    }
    norito::core::from_bytes_view(bytes).map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
    let archive = norito::decode_from_bytes_with_limits::<PdpGovernanceArchiveV1>(
        bytes,
        PDP_ARCHIVE_DECODE_LIMITS,
    )
    .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
    if norito::to_bytes(&archive).map_err(ProofOutcomeOutboxError::CanonicalEncoding)? != bytes {
        return Err(ProofOutcomeOutboxError::InvalidSubmission);
    }
    Ok(archive)
}

fn decode_potr_receipt_source(bytes: &[u8]) -> Result<PotrReceiptV1, ProofOutcomeOutboxError> {
    if bytes.is_empty() || bytes.len() > PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1 {
        return Err(ProofOutcomeOutboxError::InvalidSubmission);
    }
    norito::core::from_bytes_view(bytes).map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
    let receipt =
        norito::decode_from_bytes_with_limits::<PotrReceiptV1>(bytes, POTR_RECEIPT_DECODE_LIMITS)
            .map_err(|_| ProofOutcomeOutboxError::InvalidSubmission)?;
    if norito::to_bytes(&receipt).map_err(ProofOutcomeOutboxError::CanonicalEncoding)? != bytes {
        return Err(ProofOutcomeOutboxError::InvalidSubmission);
    }
    Ok(receipt)
}

fn operation_id(prepared: &PreparedDelivery) -> Result<[u8; 32], ProofOutcomeOutboxError> {
    let submission = norito::to_bytes(&prepared.submission)
        .map_err(ProofOutcomeOutboxError::CanonicalEncoding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(OPERATION_ID_DOMAIN_V1);
    hasher.update(&[match prepared.kind {
        ProofOutcomeKindV1::Pdp => 0,
        ProofOutcomeKindV1::Potr => 1,
    }]);
    hasher.update(&prepared.identity_digest);
    hasher.update(&prepared.outcome_digest);
    hasher.update(&(submission.len() as u64).to_le_bytes());
    hasher.update(&submission);
    Ok(*hasher.finalize().as_bytes())
}

fn find_pending_mut(
    checkpoint: &mut ProofOutcomeOutboxCheckpointV1,
    operation_id: [u8; 32],
) -> Result<&mut StoredPendingDeliveryV1, ProofOutcomeOutboxError> {
    checkpoint
        .pending
        .iter_mut()
        .find(|entry| entry.operation_id == operation_id)
        .ok_or(ProofOutcomeOutboxError::UnknownOperation)
}

fn pending_position(
    checkpoint: &ProofOutcomeOutboxCheckpointV1,
    operation_id: [u8; 32],
) -> Result<usize, ProofOutcomeOutboxError> {
    checkpoint
        .pending
        .iter()
        .position(|entry| entry.operation_id == operation_id)
        .ok_or(ProofOutcomeOutboxError::UnknownOperation)
}

fn validate_signed_transaction(
    entry: &StoredPendingDeliveryV1,
    transaction: &SignedTransaction,
) -> Result<(), ProofOutcomeOutboxError> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(ProofOutcomeOutboxError::InvalidSignedTransaction);
    };
    if instructions.len() != 1 {
        return Err(ProofOutcomeOutboxError::InvalidSignedTransaction);
    }
    let instruction = instructions[0]
        .as_any()
        .downcast_ref::<SubmitSorafsProofOutcome>()
        .ok_or(ProofOutcomeOutboxError::InvalidSignedTransaction)?;
    if instruction.submission() != &entry.submission {
        return Err(ProofOutcomeOutboxError::InvalidSignedTransaction);
    }
    Ok(())
}

fn finalized_matches(entry: &StoredPendingDeliveryV1, outcome: &ProofOutcomeRecordV1) -> bool {
    outcome.kind() == entry.kind
        && outcome.identity_digest == entry.identity_digest
        && outcome.outcome_digest == entry.outcome_digest
        && outcome.provider_id == entry.provider_id
        && outcome.manifest_digest == entry.manifest_digest
        && outcome.admission_envelope_digest == entry.admission_envelope_digest
}

fn validate_checkpoint(
    checkpoint: &ProofOutcomeOutboxCheckpointV1,
    policy: ProofOutcomeOutboxPolicyV1,
) -> Result<(), ProofOutcomeOutboxError> {
    if checkpoint.version != PROOF_OUTCOME_OUTBOX_CHECKPOINT_VERSION_V1
        || checkpoint.next_sequence == 0
        || checkpoint.pending.len() > policy.max_pending
        || checkpoint.completed.len() > policy.max_completed
        || checkpoint.dead_letters.len() > policy.max_dead_letters
    {
        return Err(ProofOutcomeOutboxError::InvalidCheckpoint);
    }
    let mut identities = BTreeSet::new();
    let mut operations = BTreeSet::new();
    let mut previous_sequence = 0_u64;
    for entry in &checkpoint.pending {
        if entry.sequence == 0
            || entry.sequence <= previous_sequence
            || entry.sequence >= checkpoint.next_sequence
            || entry.operation_id == [0; 32]
            || entry.identity_digest == [0; 32]
            || entry.outcome_digest == [0; 32]
            || entry.admission_envelope_digest == [0; 32]
            || entry.attempts > policy.max_attempts
            || !operations.insert(entry.operation_id)
            || !identities.insert((entry.kind, entry.identity_digest))
            || !durable::validate_delivery(entry, policy.max_attempts)
        {
            return Err(ProofOutcomeOutboxError::InvalidCheckpoint);
        }
        let prepared = PreparedDelivery {
            kind: entry.kind,
            identity_digest: entry.identity_digest,
            outcome_digest: entry.outcome_digest,
            provider_id: entry.provider_id,
            manifest_digest: entry.manifest_digest,
            admission_envelope_digest: entry.admission_envelope_digest,
            submission: entry.submission.clone(),
        };
        prepared.validate()?;
        if operation_id(&prepared)? != entry.operation_id {
            return Err(ProofOutcomeOutboxError::InvalidCheckpoint);
        }
        if let Some(transaction) = entry.signed_transaction.as_ref() {
            validate_signed_transaction(entry, transaction)?;
        }
        previous_sequence = entry.sequence;
    }
    for entry in &checkpoint.completed {
        if entry.operation_id == [0; 32]
            || entry.identity_digest == [0; 32]
            || entry.outcome_digest == [0; 32]
            || entry.finalized_height == 0
            || entry.finalized_block_hash == [0; 32]
            || !operations.insert(entry.operation_id)
            || !identities.insert((entry.kind, entry.identity_digest))
        {
            return Err(ProofOutcomeOutboxError::InvalidCheckpoint);
        }
    }
    for entry in &checkpoint.dead_letters {
        let prepared = PreparedDelivery {
            kind: entry.kind,
            identity_digest: entry.identity_digest,
            outcome_digest: entry.outcome_digest,
            provider_id: entry.provider_id,
            manifest_digest: entry.manifest_digest,
            admission_envelope_digest: entry.admission_envelope_digest,
            submission: entry.submission.clone(),
        };
        if entry.operation_id == [0; 32]
            || entry.identity_digest == [0; 32]
            || entry.outcome_digest == [0; 32]
            || entry.observed_finalized_height == 0
            || entry.observed_finalized_block_hash == [0; 32]
            || !operations.insert(entry.operation_id)
            || !identities.insert((entry.kind, entry.identity_digest))
            || prepared.validate().is_err()
            || operation_id(&prepared)? != entry.operation_id
        {
            return Err(ProofOutcomeOutboxError::InvalidCheckpoint);
        }
    }
    Ok(())
}

fn validate_finalized_cursor(
    cursor: ProofOutcomeFinalizedCursorV1,
) -> Result<(), ProofOutcomeOutboxError> {
    durable::validate_finalized_cursor(finalized_cursor(cursor)).map_err(Into::into)
}

const fn finalized_cursor(cursor: ProofOutcomeFinalizedCursorV1) -> FinalizedCursorV1 {
    FinalizedCursorV1 {
        height: cursor.height,
        block_hash: cursor.block_hash,
    }
}

#[derive(Debug)]
struct CheckpointStore {
    inner: AtomicCheckpointStore,
}

impl CheckpointStore {
    fn new(root: &Path, max_bytes: u64) -> Result<Self, ProofOutcomeOutboxError> {
        Ok(Self {
            inner: AtomicCheckpointStore::new(
                root,
                PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1,
                CHECKPOINT_LOCK_FILE_NAME,
                max_bytes,
            )?,
        })
    }

    fn load(
        &self,
        policy: ProofOutcomeOutboxPolicyV1,
    ) -> Result<(ProofOutcomeOutboxCheckpointV1, Option<[u8; 32]>), ProofOutcomeOutboxError> {
        let (bytes, fingerprint) = self.inner.load_bytes()?;
        let Some(bytes) = bytes else {
            return Ok((ProofOutcomeOutboxCheckpointV1::default(), None));
        };
        norito::core::from_bytes_view(&bytes)
            .map_err(|_| ProofOutcomeOutboxError::InvalidCheckpoint)?;
        let limits = checkpoint_decode_limits(bytes.len())?;
        let checkpoint: ProofOutcomeOutboxCheckpointV1 =
            norito::decode_from_bytes_with_limits(&bytes, limits)
                .map_err(|_| ProofOutcomeOutboxError::InvalidCheckpoint)?;
        if norito::to_bytes(&checkpoint).map_err(ProofOutcomeOutboxError::CanonicalEncoding)?
            != bytes
        {
            return Err(ProofOutcomeOutboxError::InvalidCheckpoint);
        }
        validate_checkpoint(&checkpoint, policy)?;
        validate_checkpoint_source_bindings(&checkpoint)
            .map_err(|_| ProofOutcomeOutboxError::InvalidCheckpoint)?;
        Ok((checkpoint, fingerprint))
    }

    fn commit(
        &self,
        checkpoint: &ProofOutcomeOutboxCheckpointV1,
        expected_fingerprint: Option<[u8; 32]>,
    ) -> Result<[u8; 32], ProofOutcomeOutboxError> {
        let bytes =
            norito::to_bytes(checkpoint).map_err(ProofOutcomeOutboxError::CanonicalEncoding)?;
        self.inner
            .commit_bytes(&bytes, expected_fingerprint)
            .map_err(Into::into)
    }
}

fn validate_checkpoint_source_bindings(
    checkpoint: &ProofOutcomeOutboxCheckpointV1,
) -> Result<(), ProofOutcomeOutboxError> {
    for entry in &checkpoint.pending {
        PreparedDelivery {
            kind: entry.kind,
            identity_digest: entry.identity_digest,
            outcome_digest: entry.outcome_digest,
            provider_id: entry.provider_id,
            manifest_digest: entry.manifest_digest,
            admission_envelope_digest: entry.admission_envelope_digest,
            submission: entry.submission.clone(),
        }
        .validate_source_binding()?;
    }
    for entry in &checkpoint.dead_letters {
        PreparedDelivery {
            kind: entry.kind,
            identity_digest: entry.identity_digest,
            outcome_digest: entry.outcome_digest,
            provider_id: entry.provider_id,
            manifest_digest: entry.manifest_digest,
            admission_envelope_digest: entry.admission_envelope_digest,
            submission: entry.submission.clone(),
        }
        .validate_source_binding()?;
    }
    Ok(())
}

fn checkpoint_decode_limits(
    encoded_bytes: usize,
) -> Result<norito::DecodeLimits, ProofOutcomeOutboxError> {
    if encoded_bytes == 0 {
        return Err(ProofOutcomeOutboxError::InvalidCheckpoint);
    }
    let total_elements = encoded_bytes
        .checked_mul(CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT)
        .ok_or(ProofOutcomeOutboxError::CheckpointTooLarge)?;
    let total_allocated_bytes = encoded_bytes
        .checked_mul(CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT)
        .and_then(|budget| budget.checked_add(CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES))
        .ok_or(ProofOutcomeOutboxError::CheckpointTooLarge)?;
    Ok(norito::DecodeLimits::new(
        encoded_bytes,
        encoded_bytes,
        total_elements,
        total_allocated_bytes,
        CHECKPOINT_MAX_NESTING_DEPTH,
    ))
}

/// Durable proof-outcome delivery errors.
#[derive(Debug, Error)]
pub enum ProofOutcomeOutboxError {
    /// Policy contains a zero bound.
    #[error("proof-outcome outbox policy is invalid")]
    InvalidPolicy,
    /// Canonical source material is invalid.
    #[error("proof-outcome source submission is invalid")]
    InvalidSubmission,
    /// Canonical encoding failed.
    #[error("proof-outcome canonical encoding failed: {0}")]
    CanonicalEncoding(#[source] norito::Error),
    /// The identity is retained with different cryptographic material.
    #[error("proof-outcome identity conflicts with retained delivery state")]
    IdentityConflict,
    /// The identity already has a terminal dead letter.
    #[error("proof-outcome identity has a terminal dead letter")]
    DeadLetterConflict,
    /// Pending capacity is exhausted.
    #[error("proof-outcome pending delivery capacity is exhausted")]
    PendingCapacityExhausted,
    /// Dead-letter capacity is exhausted.
    #[error("proof-outcome dead-letter capacity is exhausted")]
    DeadLetterCapacityExhausted,
    /// Sequence allocation overflowed.
    #[error("proof-outcome outbox sequence is exhausted")]
    SequenceExhausted,
    /// Worker scan limit is outside the protocol bound.
    #[error("proof-outcome outbox scan limit is invalid")]
    InvalidScanLimit,
    /// Operation is not pending.
    #[error("proof-outcome operation is not pending")]
    UnknownOperation,
    /// State-machine transition is unsafe.
    #[error("proof-outcome delivery transition is invalid")]
    InvalidTransition,
    /// Signed transaction does not contain the exact queued native instruction.
    #[error("proof-outcome signed transaction is invalid")]
    InvalidSignedTransaction,
    /// Finalized cursor is zero or did not advance enough to prove absence.
    #[error("proof-outcome finalized cursor is invalid")]
    InvalidFinalizedCursor,
    /// Finalized state conflicts with the exact queued outcome.
    #[error("proof-outcome finalized state conflicts with queued cryptographic material")]
    FinalizedConflict,
    /// Retry bound was exhausted.
    #[error("proof-outcome delivery retry bound is exhausted")]
    RetryExhausted,
    /// Checkpoint is malformed or noncanonical.
    #[error("proof-outcome outbox checkpoint is invalid")]
    InvalidCheckpoint,
    /// Checkpoint is above its configured byte ceiling.
    #[error("proof-outcome outbox checkpoint exceeds its byte limit")]
    CheckpointTooLarge,
    /// Checkpoint path is unsafe or inaccessible.
    #[error("proof-outcome outbox checkpoint I/O failed")]
    CheckpointIo,
    /// Another runtime changed the checkpoint.
    #[error("proof-outcome outbox checkpoint changed concurrently")]
    StaleCheckpoint,
    /// Another writer owns the checkpoint lock.
    #[error("proof-outcome outbox checkpoint writer is busy")]
    CheckpointBusy,
    /// Rename was visible but directory durability could not be established.
    #[error("proof-outcome outbox checkpoint durability is uncertain")]
    CheckpointDurabilityUncertain,
    /// The runtime stopped after uncertain durability.
    #[error("proof-outcome outbox durability is poisoned")]
    DurabilityPoisoned,
    /// Runtime state mutex was poisoned.
    #[error("proof-outcome outbox runtime lock is poisoned")]
    RuntimePoisoned,
}

impl From<DeliveryTransitionError> for ProofOutcomeOutboxError {
    fn from(error: DeliveryTransitionError) -> Self {
        match error {
            DeliveryTransitionError::InvalidFinalizedCursor => Self::InvalidFinalizedCursor,
            DeliveryTransitionError::InvalidTransition => Self::InvalidTransition,
            DeliveryTransitionError::RetryExhausted => Self::RetryExhausted,
        }
    }
}

impl From<CheckpointStoreError> for ProofOutcomeOutboxError {
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
    use std::{fs, io::Write as _, time::Duration};

    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt as _;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        isi::InstructionBox,
        sorafs::proof_ledger::{
            PROOF_OUTCOME_RECORD_VERSION_V1, PotrOutcomeProjectionV1, PotrOutcomeStatusV1,
            ProofOutcomeFinalizedCursorV1, ProofOutcomeProjectionV1,
        },
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_manifest::{
        ChunkingProfileV1, PDP_GOVERNANCE_ARCHIVE_VERSION_V1, PdpChallengeV1,
        PdpGovernanceArchiveV1, PdpRejectionReasonV1, PdpSampleV1, PdpTerminalDecisionV1,
        ProfileId, ProofStreamTier,
        pdp::{
            PDP_HOT_LEAVES_PER_SEGMENT_V1, PDP_MAX_SEGMENT_SAMPLES_V1,
            PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1,
        },
        potr::{
            POTR_RECEIPT_MAX_NOTE_BYTES_V1, POTR_RECEIPT_VERSION_V1, PotrReceiptV1, PotrStatus,
            sign_potr_receipt_v1,
        },
    };
    use tempfile::TempDir;

    use super::*;

    fn test_network_id() -> iroha_data_model::NetworkId {
        iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"proof-outcome-forwarder-test"),
        ))
    }

    #[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
    struct NestedSourceDepthBomb(Option<Box<NestedSourceDepthBomb>>);

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

    fn policy() -> ProofOutcomeOutboxPolicyV1 {
        ProofOutcomeOutboxPolicyV1 {
            max_pending: 8,
            max_completed: 8,
            max_dead_letters: 8,
            max_attempts: 2,
            checkpoint_max_bytes: 4 * 1024 * 1024,
        }
    }

    fn cursor(height: u64, hash_byte: u8) -> ProofOutcomeFinalizedCursorV1 {
        ProofOutcomeFinalizedCursorV1 {
            height,
            block_hash: [hash_byte; 32],
        }
    }

    fn signed_receipt_with_request_id(request_id: [u8; 16]) -> PotrReceiptV1 {
        let gateway = KeyPair::try_from_seed(vec![1; 32], Algorithm::Ed25519).unwrap();
        let provider = KeyPair::try_from_seed(vec![2; 32], Algorithm::MlDsa).unwrap();
        sign_potr_receipt_v1(
            PotrReceiptV1 {
                version: POTR_RECEIPT_VERSION_V1,
                manifest_digest: [3; 32],
                provider_id: [4; 32],
                tier: ProofStreamTier::Hot,
                deadline_ms: 100,
                latency_ms: 10,
                status: PotrStatus::Success,
                requested_at_ms: 1_000,
                responded_at_ms: 1_010,
                recorded_at_ms: 1_011,
                range_start: 0,
                range_end: 31,
                request_id: Some(request_id),
                trace_id: None,
                note: None,
                gateway_signature: None,
                provider_signature: None,
            },
            &gateway,
            &provider,
        )
        .unwrap()
    }

    fn signed_receipt() -> PotrReceiptV1 {
        signed_receipt_with_request_id([5; 16])
    }

    fn maximum_bounded_signed_receipt() -> PotrReceiptV1 {
        let gateway = KeyPair::try_from_seed(vec![1; 32], Algorithm::Ed25519).unwrap();
        let provider = KeyPair::try_from_seed(vec![2; 32], Algorithm::MlDsa).unwrap();
        sign_potr_receipt_v1(
            PotrReceiptV1 {
                version: POTR_RECEIPT_VERSION_V1,
                manifest_digest: [3; 32],
                provider_id: [4; 32],
                tier: ProofStreamTier::Hot,
                deadline_ms: 100,
                latency_ms: 10,
                status: PotrStatus::Success,
                requested_at_ms: 1_000,
                responded_at_ms: 1_010,
                recorded_at_ms: 1_011,
                range_start: 0,
                range_end: 31,
                request_id: Some([0xA5; 16]),
                trace_id: Some([0x5A; 16]),
                note: Some("x".repeat(POTR_RECEIPT_MAX_NOTE_BYTES_V1)),
                gateway_signature: None,
                provider_signature: None,
            },
            &gateway,
            &provider,
        )
        .unwrap()
    }

    fn maximum_bounded_pdp_archive() -> PdpGovernanceArchiveV1 {
        let mut remaining_hot_leaves = PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1;
        let mut samples = Vec::with_capacity(PDP_MAX_SEGMENT_SAMPLES_V1);
        for segment in 0..PDP_MAX_SEGMENT_SAMPLES_V1 {
            let remaining_segments = PDP_MAX_SEGMENT_SAMPLES_V1 - segment;
            let hot_leaf_count = remaining_hot_leaves
                .saturating_sub(remaining_segments - 1)
                .min(PDP_HOT_LEAVES_PER_SEGMENT_V1 as usize);
            samples.push(PdpSampleV1 {
                segment_index: u64::try_from(segment).unwrap(),
                hot_leaf_indices: (0..hot_leaf_count)
                    .map(|index| u16::try_from(index).unwrap())
                    .collect(),
            });
            remaining_hot_leaves -= hot_leaf_count;
        }
        assert_eq!(remaining_hot_leaves, 0);
        let chunk_profile = ChunkingProfileV1::from_descriptor(
            sorafs_manifest::chunker_registry::lookup(ProfileId(1)).expect("SF1 profile"),
        );
        let challenge = PdpChallengeV1::new(
            [0x71; 32],
            [0x72; 32],
            [0x73; 32],
            chunk_profile,
            [0x74; 32],
            9,
            11,
            1_000,
            1_300,
            samples,
        )
        .expect("maximum bounded challenge");
        let archive = PdpGovernanceArchiveV1 {
            version: PDP_GOVERNANCE_ARCHIVE_VERSION_V1,
            sequence: 1,
            challenge_id: challenge.challenge_id,
            commitment_digest: challenge.commitment_digest,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            epoch_id: challenge.epoch_id,
            decision: PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::DeadlineExpired),
            proof_digest: None,
            sampled_segments: u16::try_from(challenge.samples.len()).unwrap(),
            sampled_hot_leaves: u16::try_from(PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1).unwrap(),
            sampled_bytes: 0,
            issued_at_unix: challenge.issued_at_unix,
            response_deadline_unix: challenge.response_deadline_unix,
            decided_at_unix: challenge.response_deadline_unix + 1,
            admission_envelope_digest: [0x75; 32],
            canonical_challenge: norito::to_bytes(&challenge).unwrap(),
            canonical_proof: None,
        };
        archive.validate().expect("maximum bounded PDP archive");
        archive
    }

    fn replace_norito_schema<T: norito::NoritoSerialize>(bytes: &mut [u8]) {
        const SCHEMA_OFFSET: usize = 4 + 1 + 1;
        const SCHEMA_LEN: usize = 16;
        assert!(bytes.len() >= norito::core::Header::SIZE);
        bytes[SCHEMA_OFFSET..SCHEMA_OFFSET + SCHEMA_LEN]
            .copy_from_slice(&<T as norito::NoritoSerialize>::schema_hash());
    }

    fn checkpoint_with_corrupt_potr_source(
        receipt_payload: Vec<u8>,
    ) -> ProofOutcomeOutboxCheckpointV1 {
        let outbox = ProofOutcomeOutbox::in_memory(policy()).unwrap();
        outbox.enqueue_potr(&signed_receipt(), [6; 32]).unwrap();
        let mut checkpoint = outbox.state.lock().unwrap().checkpoint.clone();
        let entry = checkpoint.pending.first_mut().unwrap();
        let SorafsProofOutcomeSubmissionV1::Potr(submission) = &mut entry.submission else {
            panic!("PoTR fixture must produce a PoTR submission");
        };
        submission.receipt_payload = receipt_payload;
        let prepared = PreparedDelivery {
            kind: entry.kind,
            identity_digest: entry.identity_digest,
            outcome_digest: entry.outcome_digest,
            provider_id: entry.provider_id,
            manifest_digest: entry.manifest_digest,
            admission_envelope_digest: entry.admission_envelope_digest,
            submission: entry.submission.clone(),
        };
        entry.operation_id = operation_id(&prepared).unwrap();
        checkpoint
    }

    fn minimum_checkpoint_allocation_budget(bytes: &[u8]) -> usize {
        let encoded_bytes = bytes.len();
        let total_elements = encoded_bytes
            .checked_mul(CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT)
            .expect("test checkpoint element ceiling");
        let ceiling = encoded_bytes
            .checked_mul(64)
            .and_then(|budget| budget.checked_add(2 * 1024 * 1024))
            .expect("finite test allocation ceiling");
        let decodes = |allocation_budget| {
            let limits = norito::DecodeLimits::new(
                encoded_bytes,
                encoded_bytes,
                total_elements,
                allocation_budget,
                CHECKPOINT_MAX_NESTING_DEPTH,
            );
            norito::decode_from_bytes_with_limits::<ProofOutcomeOutboxCheckpointV1>(bytes, limits)
                .is_ok()
        };
        assert!(
            decodes(ceiling),
            "finite 64x wire plus 2 MiB ceiling must decode the maximum-valid checkpoint"
        );
        let mut lower = 0;
        let mut upper = ceiling;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if decodes(middle) {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(decodes(lower));
        if lower != 0 {
            assert!(!decodes(lower - 1));
        }
        lower
    }

    fn signed_transaction(pending: &ProofOutcomePendingDeliveryV1, seed: u8) -> SignedTransaction {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap();
        let mut builder = TransactionBuilder::new(
            test_network_id(),
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(SubmitSorafsProofOutcome::new(
            pending.submission.clone(),
        ))]);
        builder.set_creation_time(Duration::from_secs(1));
        builder.try_sign(key.private_key()).unwrap()
    }

    fn finalized(
        pending: &ProofOutcomePendingDeliveryV1,
        receipt: &PotrReceiptV1,
    ) -> ProofOutcomeFinalizedRecordV1 {
        ProofOutcomeFinalizedRecordV1 {
            finalized_cursor: ProofOutcomeFinalizedCursorV1 {
                height: 2,
                block_hash: [9; 32],
            },
            outcome: ProofOutcomeRecordV1 {
                version: PROOF_OUTCOME_RECORD_VERSION_V1,
                identity_digest: pending.identity_digest,
                outcome_digest: pending.outcome_digest,
                provider_id: pending.provider_id,
                manifest_digest: pending.manifest_digest,
                admission_envelope_digest: pending.admission_envelope_digest,
                submitted_by: AccountId::new(
                    KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519)
                        .unwrap()
                        .public_key()
                        .clone(),
                ),
                committed_at_unix_ms: 2_000,
                projection: ProofOutcomeProjectionV1::Potr(PotrOutcomeProjectionV1 {
                    status: PotrOutcomeStatusV1::Success,
                    deadline_ms: receipt.deadline_ms,
                    latency_ms: receipt.latency_ms,
                    requested_at_ms: receipt.requested_at_ms,
                    responded_at_ms: receipt.responded_at_ms,
                    recorded_at_ms: receipt.recorded_at_ms,
                    range_start: receipt.range_start,
                    range_end: receipt.range_end,
                    gateway_public_key: receipt
                        .gateway_signature
                        .as_ref()
                        .unwrap()
                        .public_key
                        .clone()
                        .try_into()
                        .unwrap(),
                    governed_provider_key_digest: *blake3::hash(
                        &receipt.provider_signature.as_ref().unwrap().public_key,
                    )
                    .as_bytes(),
                    canonical_signed_receipt: receipt.signed_receipt_bytes().unwrap(),
                }),
            },
        }
    }

    #[test]
    fn signed_transaction_is_persisted_before_ambiguous_submission_and_finalization() {
        let outbox = ProofOutcomeOutbox::in_memory(policy()).unwrap();
        let receipt = signed_receipt();
        let operation = outbox
            .enqueue_potr(&receipt, [6; 32])
            .unwrap()
            .operation_id();
        assert_eq!(
            operation,
            potr_proof_outcome_operation_id_v1(&receipt, [6; 32])
                .expect("canonical PoTR operation identity")
        );
        let claimed = outbox.claim_for_signing(operation, cursor(1, 1)).unwrap();
        let transaction = signed_transaction(&claimed, 8);
        let hash = outbox
            .store_signed_transaction(operation, transaction.clone())
            .unwrap();
        assert_eq!(hash, *transaction.hash().as_ref());
        assert_eq!(outbox.begin_submission(operation).unwrap(), transaction);
        outbox.mark_submitted(operation).unwrap();
        let pending = outbox.pending(8).unwrap().remove(0);
        outbox
            .mark_finalized(operation, &finalized(&pending, &receipt))
            .unwrap();
        assert!(outbox.pending(8).unwrap().is_empty());
        assert!(matches!(
            outbox.enqueue_potr(&receipt, [6; 32]).unwrap(),
            ProofOutcomeEnqueueResultV1::Existing { .. }
        ));
    }

    #[test]
    fn signer_failure_releases_claim_without_consuming_an_attempt() {
        let outbox = ProofOutcomeOutbox::in_memory(policy()).unwrap();
        let receipt = signed_receipt();
        let operation = outbox
            .enqueue_potr(&receipt, [6; 32])
            .unwrap()
            .operation_id();

        outbox.claim_for_signing(operation, cursor(1, 1)).unwrap();
        outbox.release_signing_claim(operation).unwrap();
        let ready = outbox.pending(8).unwrap().remove(0);
        assert_eq!(ready.state, ProofOutcomeDeliveryStateV1::Ready);
        assert_eq!(ready.attempts, 0);
        assert_eq!(ready.baseline_finalized_height, 0);
        assert_eq!(ready.baseline_finalized_block_hash, [0; 32]);
        assert!(ready.signed_transaction.is_none());

        let reclaimed = outbox.claim_for_signing(operation, cursor(2, 2)).unwrap();
        assert_eq!(reclaimed.operation_id, operation);
        assert_eq!(reclaimed.attempts, 0);
    }

    #[test]
    fn known_prequeue_failure_retains_the_exact_signed_transaction() {
        let outbox = ProofOutcomeOutbox::in_memory(policy()).unwrap();
        let receipt = signed_receipt();
        let operation = outbox
            .enqueue_potr(&receipt, [6; 32])
            .unwrap()
            .operation_id();
        let claimed = outbox.claim_for_signing(operation, cursor(1, 1)).unwrap();
        let transaction = signed_transaction(&claimed, 9);
        outbox
            .store_signed_transaction(operation, transaction.clone())
            .unwrap();

        assert_eq!(outbox.begin_submission(operation).unwrap(), transaction);
        outbox.mark_not_submitted(operation).unwrap();
        let retry = outbox.pending(8).unwrap().remove(0);
        assert_eq!(retry.state, ProofOutcomeDeliveryStateV1::Signed);
        assert_eq!(retry.attempts, 1);
        assert_eq!(retry.signed_transaction.as_ref(), Some(&transaction));
        assert_eq!(outbox.begin_submission(operation).unwrap(), transaction);
    }

    #[test]
    fn pending_after_advances_and_wraps_without_repeating_a_page_entry() {
        let outbox = ProofOutcomeOutbox::in_memory(policy()).unwrap();
        for request_id in 1..=5 {
            outbox
                .enqueue_potr(
                    &signed_receipt_with_request_id([request_id; 16]),
                    [request_id.saturating_add(16); 32],
                )
                .unwrap();
        }

        let first = outbox.pending_after(None, 3).unwrap();
        assert_eq!(
            first.iter().map(|entry| entry.sequence).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let second = outbox
            .pending_after(first.last().map(|entry| entry.sequence), 3)
            .unwrap();
        assert_eq!(
            second
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            vec![4, 5, 1]
        );
        assert_eq!(
            second
                .iter()
                .map(|entry| entry.operation_id)
                .collect::<BTreeSet<_>>()
                .len(),
            second.len()
        );
    }

    #[test]
    fn crash_recovery_resets_only_isolated_signing_and_retains_ambiguous_transaction() {
        let dir = TempDir::new().unwrap();
        let receipt_a = signed_receipt();
        let mut receipt_b = signed_receipt();
        receipt_b.request_id = Some([10; 16]);
        let gateway = KeyPair::try_from_seed(vec![1; 32], Algorithm::Ed25519).unwrap();
        let provider = KeyPair::try_from_seed(vec![2; 32], Algorithm::MlDsa).unwrap();
        receipt_b.gateway_signature = None;
        receipt_b.provider_signature = None;
        let receipt_b = sign_potr_receipt_v1(receipt_b, &gateway, &provider).unwrap();

        let outbox = ProofOutcomeOutbox::open(dir.path(), policy()).unwrap();
        let operation_a = outbox
            .enqueue_potr(&receipt_a, [6; 32])
            .unwrap()
            .operation_id();
        let operation_b = outbox
            .enqueue_potr(&receipt_b, [6; 32])
            .unwrap()
            .operation_id();
        outbox.claim_for_signing(operation_a, cursor(1, 1)).unwrap();
        let claimed_b = outbox.claim_for_signing(operation_b, cursor(1, 1)).unwrap();
        outbox
            .store_signed_transaction(operation_b, signed_transaction(&claimed_b, 11))
            .unwrap();
        outbox.begin_submission(operation_b).unwrap();
        drop(outbox);

        let restored = ProofOutcomeOutbox::open(dir.path(), policy()).unwrap();
        let pending = restored.pending(8).unwrap();
        assert_eq!(
            pending
                .iter()
                .find(|entry| entry.operation_id == operation_a)
                .unwrap()
                .state,
            ProofOutcomeDeliveryStateV1::Ready
        );
        let ambiguous = pending
            .iter()
            .find(|entry| entry.operation_id == operation_b)
            .unwrap();
        assert_eq!(ambiguous.state, ProofOutcomeDeliveryStateV1::Ambiguous);
        assert!(ambiguous.signed_transaction.is_some());
    }

    #[test]
    fn crash_before_rename_orphan_does_not_replace_the_last_durable_checkpoint() {
        let dir = TempDir::new().unwrap();
        let receipt = signed_receipt();
        let outbox = ProofOutcomeOutbox::open(dir.path(), policy()).unwrap();
        let operation = outbox
            .enqueue_potr(&receipt, [6; 32])
            .unwrap()
            .operation_id();
        drop(outbox);

        let orphan = dir.path().join(format!(
            ".{PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1}.999999.999999.tmp"
        ));
        std::fs::write(&orphan, b"crash-before-rename partial checkpoint").unwrap();

        let restored = ProofOutcomeOutbox::open(dir.path(), policy()).unwrap();
        let pending = restored.pending(8).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].operation_id, operation);
    }

    #[test]
    fn poisoned_checkpoint_source_binding_fails_closed_on_restart() {
        let dir = TempDir::new().unwrap();
        let receipt = signed_receipt();
        let outbox = ProofOutcomeOutbox::open(dir.path(), policy()).unwrap();
        outbox.enqueue_potr(&receipt, [6; 32]).unwrap();
        drop(outbox);

        let checkpoint_path = dir
            .path()
            .join(PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1);
        let bytes = std::fs::read(&checkpoint_path).unwrap();
        let mut checkpoint: ProofOutcomeOutboxCheckpointV1 =
            norito::decode_from_bytes(&bytes).unwrap();
        checkpoint.pending[0].provider_id = ProviderId::new([0xEE; 32]);
        std::fs::write(&checkpoint_path, norito::to_bytes(&checkpoint).unwrap()).unwrap();

        assert!(matches!(
            ProofOutcomeOutbox::open(dir.path(), policy()),
            Err(ProofOutcomeOutboxError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn nested_source_allocation_and_depth_bombs_fail_closed_on_restart() {
        let mut allocation_bomb = norito::to_compressed_bytes(
            &vec![0xA5_u8; 2 * 1024 * 1024],
            Some(norito::CompressionConfig::default()),
        )
        .unwrap();
        assert!(
            allocation_bomb.len() < PROOF_OUTCOME_MAX_POTR_RECEIPT_BYTES_V1,
            "the nested allocation bomb must pass the outer byte-vector ceiling"
        );
        assert!(
            norito::decode_from_bytes_with_limits::<Vec<u8>>(
                &allocation_bomb,
                POTR_RECEIPT_DECODE_LIMITS,
            )
            .is_err(),
            "the production allocation budget must reject decompression amplification"
        );
        replace_norito_schema::<PotrReceiptV1>(&mut allocation_bomb);
        let allocation_checkpoint = checkpoint_with_corrupt_potr_source(allocation_bomb);
        let allocation_dir = TempDir::new().unwrap();
        write_private_checkpoint(
            &allocation_dir
                .path()
                .join(PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1),
            &norito::to_bytes(&allocation_checkpoint).unwrap(),
        );
        assert!(matches!(
            ProofOutcomeOutbox::open(allocation_dir.path(), policy()),
            Err(ProofOutcomeOutboxError::InvalidCheckpoint)
        ));

        let mut depth_bomb = NestedSourceDepthBomb(None);
        for _ in 0..64 {
            depth_bomb = NestedSourceDepthBomb(Some(Box::new(depth_bomb)));
        }
        let mut depth_bytes = norito::to_bytes(&depth_bomb).unwrap();
        let depth_limits = norito::DecodeLimits::new(
            depth_bytes.len(),
            depth_bytes.len(),
            depth_bytes.len().saturating_mul(4),
            depth_bytes.len().saturating_mul(16),
            POTR_RECEIPT_DECODE_LIMITS.max_nesting_depth(),
        );
        assert!(
            norito::decode_from_bytes_with_limits::<NestedSourceDepthBomb>(
                &depth_bytes,
                depth_limits,
            )
            .is_err(),
            "the constructed nested source must exceed the production depth ceiling"
        );
        replace_norito_schema::<PotrReceiptV1>(&mut depth_bytes);
        let depth_checkpoint = checkpoint_with_corrupt_potr_source(depth_bytes);
        let depth_dir = TempDir::new().unwrap();
        write_private_checkpoint(
            &depth_dir
                .path()
                .join(PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1),
            &norito::to_bytes(&depth_checkpoint).unwrap(),
        );
        assert!(matches!(
            ProofOutcomeOutbox::open(depth_dir.path(), policy()),
            Err(ProofOutcomeOutboxError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn checkpoint_decode_budget_accepts_exact_policy_maximum_and_rejects_one_byte_over() {
        let outbox = ProofOutcomeOutbox::in_memory(ProofOutcomeOutboxPolicyV1 {
            checkpoint_max_bytes: 64 * 1024 * 1024,
            ..policy()
        })
        .unwrap();
        outbox.enqueue_pdp(&maximum_bounded_pdp_archive()).unwrap();
        outbox
            .enqueue_potr(&maximum_bounded_signed_receipt(), [0x76; 32])
            .unwrap();
        for (index, pending) in outbox.pending(8).unwrap().into_iter().enumerate() {
            let claimed = outbox
                .claim_for_signing(pending.operation_id, cursor(1, 1))
                .unwrap();
            outbox
                .store_signed_transaction(
                    pending.operation_id,
                    signed_transaction(&claimed, u8::try_from(index + 20).unwrap()),
                )
                .unwrap();
        }
        let checkpoint = outbox.state.lock().unwrap().checkpoint.clone();
        let bytes = norito::to_bytes(&checkpoint).unwrap();
        norito::core::from_bytes_view(&bytes)
            .expect("maximum-valid checkpoint must pass zero-copy canonical preflight");
        let minimum_allocation = minimum_checkpoint_allocation_budget(&bytes);
        let proportional_allocation = bytes
            .len()
            .checked_mul(CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT)
            .unwrap();
        let fixed_requirement = minimum_allocation.saturating_sub(proportional_allocation);
        const MARGIN_QUANTUM: usize = 64 * 1024;
        let rounded_requirement = fixed_requirement
            .checked_div(MARGIN_QUANTUM)
            .and_then(|quanta| quanta.checked_add(1))
            .and_then(|quanta| quanta.checked_mul(MARGIN_QUANTUM))
            .unwrap();
        let maximum_defensible_fixed = rounded_requirement.checked_add(MARGIN_QUANTUM).unwrap();
        let production_limits = checkpoint_decode_limits(bytes.len()).unwrap();
        eprintln!(
            "checkpoint wire={} minimum_allocation={} proportional={} fixed_requirement={} \
             rounded_requirement={} production_fixed={}",
            bytes.len(),
            minimum_allocation,
            proportional_allocation,
            fixed_requirement,
            rounded_requirement,
            CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES,
        );
        assert!(
            production_limits.max_total_allocated_bytes() >= minimum_allocation,
            "production allocation budget must cover the measured maximum-valid checkpoint"
        );
        assert!(
            CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES >= rounded_requirement,
            "fixed allowance must reach the first 64 KiB boundary above the measured requirement"
        );
        assert!(
            CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES <= maximum_defensible_fixed,
            "fixed allowance must stay within one 64 KiB safety margin of the rounded requirement"
        );
        let decoded: ProofOutcomeOutboxCheckpointV1 =
            norito::decode_from_bytes_with_limits(&bytes, production_limits)
                .expect("maximum bounded PDP/PoTR checkpoint must fit its decode budget");
        assert_eq!(decoded, checkpoint);
        let exact_policy = ProofOutcomeOutboxPolicyV1 {
            checkpoint_max_bytes: u64::try_from(bytes.len()).unwrap(),
            ..policy()
        };
        validate_checkpoint(&decoded, exact_policy).unwrap();
        validate_checkpoint_source_bindings(&decoded).unwrap();

        let dir = TempDir::new().unwrap();
        write_private_checkpoint(
            &dir.path()
                .join(PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1),
            &bytes,
        );
        drop(ProofOutcomeOutbox::open(dir.path(), exact_policy).unwrap());
        let one_byte_too_small = ProofOutcomeOutboxPolicyV1 {
            checkpoint_max_bytes: exact_policy.checkpoint_max_bytes - 1,
            ..exact_policy
        };
        assert!(matches!(
            ProofOutcomeOutbox::open(dir.path(), one_byte_too_small),
            Err(ProofOutcomeOutboxError::CheckpointTooLarge)
        ));
    }

    #[test]
    fn compressed_checkpoint_allocation_bomb_is_rejected_before_owned_materialization() {
        let mut bomb = ProofOutcomeOutboxCheckpointV1::default();
        bomb.completed.reserve(20_000);
        for index in 1_u64..=20_000 {
            let mut identity = [0x31; 32];
            identity[..8].copy_from_slice(&index.to_le_bytes());
            let mut operation = [0x32; 32];
            operation[..8].copy_from_slice(&index.to_le_bytes());
            let mut outcome = [0x33; 32];
            outcome[..8].copy_from_slice(&index.to_le_bytes());
            bomb.completed.push(StoredCompletedDeliveryV1 {
                operation_id: operation,
                kind: ProofOutcomeKindV1::Potr,
                identity_digest: identity,
                outcome_digest: outcome,
                finalized_height: index,
                finalized_block_hash: [0x34; 32],
            });
        }
        let canonical = norito::to_bytes(&bomb).unwrap();
        let compressed =
            norito::to_compressed_bytes(&bomb, Some(norito::CompressionConfig::default())).unwrap();
        assert!(
            compressed
                .len()
                .checked_mul(CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT)
                .and_then(|budget| {
                    budget.checked_add(CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES)
                })
                .is_some_and(|budget| budget < canonical.len()),
            "fixture must amplify beyond the production allocation budget"
        );
        assert!(
            norito::core::from_bytes_view(&compressed).is_err(),
            "zero-copy preflight must reject compressed checkpoint archives"
        );

        let dir = TempDir::new().unwrap();
        write_private_checkpoint(
            &dir.path()
                .join(PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1),
            &compressed,
        );
        let bomb_policy = ProofOutcomeOutboxPolicyV1 {
            max_completed: 20_000,
            checkpoint_max_bytes: u64::try_from(compressed.len()).unwrap(),
            ..policy()
        };
        assert!(matches!(
            ProofOutcomeOutbox::open(dir.path(), bomb_policy),
            Err(ProofOutcomeOutboxError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn finalized_conflict_is_dead_lettered_and_never_reported_as_success() {
        let outbox = ProofOutcomeOutbox::in_memory(policy()).unwrap();
        let receipt = signed_receipt();
        let operation = outbox
            .enqueue_potr(&receipt, [6; 32])
            .unwrap()
            .operation_id();
        let pending = outbox.pending(8).unwrap().remove(0);
        let mut conflicting = finalized(&pending, &receipt);
        conflicting.outcome.outcome_digest = [0xFF; 32];
        assert!(matches!(
            outbox.mark_finalized(operation, &conflicting),
            Err(ProofOutcomeOutboxError::FinalizedConflict)
        ));
        assert!(outbox.pending(8).unwrap().is_empty());
        assert!(matches!(
            outbox.enqueue_potr(&receipt, [6; 32]),
            Err(ProofOutcomeOutboxError::DeadLetterConflict)
        ));
        assert_eq!(
            outbox.dead_letters(8).unwrap()[0].reason,
            ProofOutcomeDeadLetterReasonV1::FinalizedConflict
        );
    }

    #[test]
    fn finalized_absence_advances_retry_cursor_and_dead_letters_at_bound() {
        let outbox = ProofOutcomeOutbox::in_memory(policy()).unwrap();
        let receipt = signed_receipt();
        let operation = outbox
            .enqueue_potr(&receipt, [6; 32])
            .unwrap()
            .operation_id();
        let claimed = outbox.claim_for_signing(operation, cursor(1, 1)).unwrap();
        outbox
            .store_signed_transaction(operation, signed_transaction(&claimed, 12))
            .unwrap();
        outbox.begin_submission(operation).unwrap();
        outbox
            .mark_finalized_absent(operation, cursor(2, 2))
            .unwrap();

        let retry = outbox.pending(8).unwrap().remove(0);
        assert_eq!(retry.state, ProofOutcomeDeliveryStateV1::Signed);
        assert_eq!(retry.attempts, 2);
        assert_eq!(retry.baseline_finalized_height, 2);
        assert_eq!(retry.baseline_finalized_block_hash, [2; 32]);
        outbox.begin_submission(operation).unwrap();
        assert!(matches!(
            outbox.mark_finalized_absent(operation, cursor(2, 3)),
            Err(ProofOutcomeOutboxError::InvalidTransition)
        ));
        outbox
            .mark_finalized_absent(operation, cursor(3, 3))
            .unwrap();
        assert!(outbox.pending(8).unwrap().is_empty());
        assert!(matches!(
            outbox.enqueue_potr(&receipt, [6; 32]),
            Err(ProofOutcomeOutboxError::DeadLetterConflict)
        ));
        assert_eq!(
            outbox.dead_letters(8).unwrap()[0].reason,
            ProofOutcomeDeadLetterReasonV1::RetryExhausted
        );
    }

    #[test]
    fn terminal_rejection_allows_bounded_signer_rotation() {
        let outbox = ProofOutcomeOutbox::in_memory(policy()).unwrap();
        let receipt = signed_receipt();
        let operation = outbox
            .enqueue_potr(&receipt, [6; 32])
            .unwrap()
            .operation_id();
        let first = outbox.claim_for_signing(operation, cursor(1, 1)).unwrap();
        let first_transaction = signed_transaction(&first, 13);
        outbox
            .store_signed_transaction(operation, first_transaction.clone())
            .unwrap();
        outbox.begin_submission(operation).unwrap();
        outbox
            .mark_transaction_rejected(operation, cursor(2, 2))
            .unwrap();

        let ready = outbox.pending(8).unwrap().remove(0);
        assert_eq!(ready.state, ProofOutcomeDeliveryStateV1::Ready);
        assert_eq!(ready.attempts, 1);
        assert!(ready.signed_transaction.is_none());
        let rotated = outbox.claim_for_signing(operation, cursor(2, 2)).unwrap();
        let rotated_transaction = signed_transaction(&rotated, 14);
        assert_ne!(first_transaction.hash(), rotated_transaction.hash());
        outbox
            .store_signed_transaction(operation, rotated_transaction)
            .unwrap();
        outbox.begin_submission(operation).unwrap();
        outbox
            .mark_transaction_rejected(operation, cursor(3, 3))
            .unwrap();
        assert!(outbox.pending(8).unwrap().is_empty());
        assert_eq!(
            outbox.dead_letters(8).unwrap()[0].reason,
            ProofOutcomeDeadLetterReasonV1::TransactionRejected
        );
        assert!(matches!(
            outbox.retry_dead_letter(operation, [0xFF; 32]),
            Err(ProofOutcomeOutboxError::UnknownOperation)
        ));
        outbox
            .retry_dead_letter(operation, ready.outcome_digest)
            .unwrap();
        let retried = outbox.pending(8).unwrap().remove(0);
        assert_eq!(retried.operation_id, operation);
        assert_eq!(retried.state, ProofOutcomeDeliveryStateV1::Ready);
        assert_eq!(retried.attempts, 0);
    }

    #[cfg(unix)]
    #[test]
    fn checkpoint_open_rejects_symlink_and_hardlink_targets() {
        use std::{fs, os::unix::fs::symlink};

        let symlink_dir = TempDir::new().unwrap();
        let outside = symlink_dir.path().join("outside.to");
        fs::write(&outside, b"not a checkpoint").unwrap();
        symlink(
            &outside,
            symlink_dir
                .path()
                .join(PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1),
        )
        .unwrap();
        assert!(matches!(
            ProofOutcomeOutbox::open(symlink_dir.path(), policy()),
            Err(ProofOutcomeOutboxError::CheckpointIo)
        ));

        let hardlink_dir = TempDir::new().unwrap();
        let outside = hardlink_dir.path().join("outside.to");
        fs::write(&outside, b"not a checkpoint").unwrap();
        fs::hard_link(
            &outside,
            hardlink_dir
                .path()
                .join(PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1),
        )
        .unwrap();
        assert!(matches!(
            ProofOutcomeOutbox::open(hardlink_dir.path(), policy()),
            Err(ProofOutcomeOutboxError::CheckpointIo)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn checkpoint_parent_path_swap_is_rejected_without_writing_the_replacement() {
        let outer = TempDir::new().unwrap();
        let state_dir = outer.path().join("state");
        let displaced_dir = outer.path().join("displaced-state");
        let outbox = ProofOutcomeOutbox::open(&state_dir, policy()).unwrap();
        outbox.enqueue_potr(&signed_receipt(), [6; 32]).unwrap();

        fs::rename(&state_dir, &displaced_dir).unwrap();
        fs::create_dir(&state_dir).unwrap();
        let mut second_receipt = signed_receipt();
        second_receipt.request_id = Some([0xCC; 16]);
        second_receipt.gateway_signature = None;
        second_receipt.provider_signature = None;
        let gateway = KeyPair::try_from_seed(vec![1; 32], Algorithm::Ed25519).unwrap();
        let provider = KeyPair::try_from_seed(vec![2; 32], Algorithm::MlDsa).unwrap();
        let second_receipt = sign_potr_receipt_v1(second_receipt, &gateway, &provider).unwrap();
        assert!(matches!(
            outbox.enqueue_potr(&second_receipt, [6; 32]),
            Err(ProofOutcomeOutboxError::CheckpointIo)
        ));
        assert!(
            !state_dir
                .join(PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1)
                .exists(),
            "the replacement directory must not receive checkpoint data"
        );

        drop(outbox);
        fs::remove_dir(&state_dir).unwrap();
        fs::rename(&displaced_dir, &state_dir).unwrap();
        let restored = ProofOutcomeOutbox::open(&state_dir, policy()).unwrap();
        assert_eq!(restored.pending(8).unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn writer_guard_collapses_path_aliases_and_rejects_hardlinked_lock_files() {
        let dir = TempDir::new().unwrap();
        let outbox = ProofOutcomeOutbox::open(dir.path(), policy()).unwrap();
        drop(outbox);
        let lock_path = dir.path().join(CHECKPOINT_LOCK_FILE_NAME);
        let held = CheckpointWriterGuard::acquire(&lock_path).unwrap();
        let dot_alias = dir.path().join(".").join(CHECKPOINT_LOCK_FILE_NAME);
        assert!(matches!(
            CheckpointWriterGuard::acquire(&dot_alias),
            Err(CheckpointStoreError::Busy)
        ));
        drop(held);

        let alias_dir = TempDir::new().unwrap();
        let hardlink = alias_dir.path().join(CHECKPOINT_LOCK_FILE_NAME);
        std::fs::hard_link(&lock_path, &hardlink).unwrap();
        assert!(matches!(
            CheckpointWriterGuard::acquire(&hardlink),
            Err(CheckpointStoreError::Io)
        ));
    }
}
