//! Canonical Kagemusha carrier bindings and consensus-persisted operation outcomes.

use std::str::FromStr as _;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    isi::{InstructionBox, RegisterBox},
    offline::{
        KagemushaOperationCarrierErrorV4, KagemushaOperationCarrierV4, KagemushaOperationKindV4,
        KagemushaOperationRequestV4,
        classify_kagemusha_operation_entrypoint_v4 as classify_direct_kagemusha_operation_entrypoint_v4,
        classify_kagemusha_operation_transaction_v4 as classify_direct_kagemusha_operation_transaction_v4,
        is_kagemusha_operation_instruction_v4,
    },
    state_path::StatePath,
    transaction::{
        Executable, ExecutableBatchItem, SignedTransaction, TransactionEntrypoint,
        TransactionResult,
    },
};
use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::state::{StateBlock, StateTransaction, WorldReadOnly};

fn append_explicit_kagemusha_candidates_v4(
    executable: &Executable,
    pending: &mut Vec<InstructionBox>,
) {
    match executable {
        Executable::Instructions(instructions) => pending.extend(instructions.iter().cloned()),
        Executable::IvmProved(proved) => pending.extend(proved.overlay.iter().cloned()),
        Executable::Batch(items) => pending.extend(items.iter().filter_map(|item| match item {
            ExecutableBatchItem::Instruction(instruction) => Some(instruction.clone()),
            ExecutableBatchItem::ContractCall(_) => None,
        })),
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
    }
}

/// Return whether explicit native instructions contain a Kagemusha operation,
/// including instructions deferred inside trigger registration or a multisig proposal.
pub(crate) fn instructions_contain_kagemusha_operation_v4(instructions: &[InstructionBox]) -> bool {
    let mut pending = instructions.to_vec();
    while let Some(instruction) = pending.pop() {
        if is_kagemusha_operation_instruction_v4(&instruction) {
            return true;
        }
        if let Ok(MultisigInstructionBox::Propose(proposal)) =
            MultisigInstructionBox::try_from(&instruction)
        {
            pending.extend(proposal.instructions);
        }
        if let Some(RegisterBox::Trigger(register)) =
            instruction.as_any().downcast_ref::<RegisterBox>()
        {
            append_explicit_kagemusha_candidates_v4(
                register.object.action().executable(),
                &mut pending,
            );
        }
    }
    false
}

/// Return whether an executable contains a direct or deferred native Kagemusha operation.
pub(crate) fn executable_contains_kagemusha_operation_v4(executable: &Executable) -> bool {
    let mut instructions = Vec::new();
    append_explicit_kagemusha_candidates_v4(executable, &mut instructions);
    instructions_contain_kagemusha_operation_v4(&instructions)
}

/// Classify the only supported Kagemusha signed-transaction carrier.
///
/// The data-model classifier owns the direct carrier contract. Core adds the
/// executor-aware traversal of deferred trigger and multisig payloads so every
/// runtime admission and recovery path rejects hidden alternate carriers.
pub fn classify_kagemusha_operation_transaction_v4(
    transaction: &SignedTransaction,
) -> Result<Option<KagemushaOperationCarrierV4<'_>>, KagemushaOperationCarrierErrorV4> {
    let carrier = classify_direct_kagemusha_operation_transaction_v4(transaction)?;
    if carrier.is_some() {
        return Ok(carrier);
    }
    if executable_contains_kagemusha_operation_v4(transaction.instructions()) {
        return Err(KagemushaOperationCarrierErrorV4::NonCanonicalExecutable);
    }
    Ok(None)
}

/// Classify a Kagemusha operation across the complete runtime entrypoint boundary.
pub fn classify_kagemusha_operation_entrypoint_v4(
    entrypoint: &TransactionEntrypoint,
) -> Result<Option<KagemushaOperationCarrierV4<'_>>, KagemushaOperationCarrierErrorV4> {
    let carrier = classify_direct_kagemusha_operation_entrypoint_v4(entrypoint)?;
    if carrier.is_some() {
        return Ok(carrier);
    }
    let contains_deferred_operation = match entrypoint {
        TransactionEntrypoint::External(transaction) => {
            executable_contains_kagemusha_operation_v4(transaction.instructions())
        }
        TransactionEntrypoint::SealedReveal(reveal) => {
            executable_contains_kagemusha_operation_v4(reveal.signed_transaction().instructions())
        }
        TransactionEntrypoint::Time(time) => {
            instructions_contain_kagemusha_operation_v4(time.instructions.0.as_ref())
        }
        TransactionEntrypoint::SealedCommitment(_) => false,
    };
    if !contains_deferred_operation {
        return Ok(None);
    }
    match entrypoint {
        TransactionEntrypoint::External(_) => {
            Err(KagemushaOperationCarrierErrorV4::NonCanonicalExecutable)
        }
        TransactionEntrypoint::SealedCommitment(_)
        | TransactionEntrypoint::SealedReveal(_)
        | TransactionEntrypoint::Time(_) => {
            Err(KagemushaOperationCarrierErrorV4::NonExternalEntrypoint)
        }
    }
}

/// State-key prefix reserved for per-submitter Kagemusha operation attempts.
pub const KAGEMUSHA_OPERATION_OUTCOME_STATE_PREFIX_V4: &str = "kagemusha_operation_outcome_v4_";
/// State-key prefix reserved for the globally applied Kagemusha operation record.
///
/// This remains below the outcome namespace so existing deterministic state
/// access fencing covers both attempt and finality records.
pub const KAGEMUSHA_OPERATION_FINALITY_STATE_PREFIX_V4: &str =
    "kagemusha_operation_outcome_v4_global_";
/// Exact version of the first-release outcome record.
pub const KAGEMUSHA_OPERATION_OUTCOME_RECORD_VERSION_V4: u16 = 4;
/// Maximum canonical bytes accepted for one persisted outcome record.
pub const KAGEMUSHA_OPERATION_OUTCOME_RECORD_MAX_BYTES_V4: usize = 16 * 1024;

const KAGEMUSHA_OPERATION_AUTHORITY_KEY_DOMAIN_V4: &[u8] =
    b"iroha:offline:kagemusha:operation-outcome-authority:v4\0";
const KAGEMUSHA_SIGNED_TRANSACTION_WIRE_HASH_DOMAIN_V4: &[u8] =
    b"iroha:offline:kagemusha:signed-transaction-wire:v4\0";

/// Canonical execution phase used by an operation's exact Kura locator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum KagemushaOperationExecutionPhaseV4 {
    /// Autonomous merge execution, ordered before ordinary carrier entrypoints.
    Merge,
    /// Ordinary carrier-block execution.
    Ordinary,
}

/// Lifecycle state stored while a block is executing and after it becomes terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum KagemushaOperationOutcomeStateV4 {
    /// Block-local attempt reservation or successfully committed economic claim.
    /// This state must never survive a committed block.
    Pending,
    /// The exact operation applied successfully; valid only at the global key.
    Applied,
    /// The exact carrier reached a deterministic rejection; valid only at an attempt key.
    Rejected,
}

/// Consensus-persisted identity and exact history locator for one operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaOperationOutcomeRecordV4 {
    /// Exact record format version.
    pub version: u16,
    /// Stable operation identifier supplied by the authorized device.
    pub operation_id: [u8; 32],
    /// Top-up or redemption operation kind.
    pub kind: KagemushaOperationKindV4,
    /// Fixed digest of the account whose device authorization signs the request.
    pub request_authority_digest: [u8; 32],
    /// Fixed digest of the outer account that signed the carrier transaction.
    pub outer_authority_digest: [u8; 32],
    /// Domain-separated digest of every canonical authorized request field.
    pub canonical_request_digest: [u8; 32],
    /// Domain-separated digest of the exact authorization-bearing signed-transaction wire.
    pub signed_transaction_wire_hash: [u8; 32],
    /// Payload-intent/replay hash of the external transaction entrypoint.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Global carrier height at which the operation became terminal.
    pub carrier_height: u64,
    /// Merge or ordinary execution phase.
    pub execution_phase: KagemushaOperationExecutionPhaseV4,
    /// Zero-based canonical index within the execution phase.
    pub phase_index: u64,
    /// Hash of the final complete transaction result, including batch receipts.
    pub result_hash: Option<HashOf<TransactionResult>>,
    /// Block-local or terminal outcome state.
    pub outcome: KagemushaOperationOutcomeStateV4,
}

impl KagemushaOperationOutcomeRecordV4 {
    fn validate_common(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), KagemushaOperationOutcomeErrorV4> {
        if self.version != KAGEMUSHA_OPERATION_OUTCOME_RECORD_VERSION_V4
            || self.operation_id == [0; 32]
            || self.operation_id != operation_id
            || self.request_authority_digest == [0; 32]
            || self.outer_authority_digest == [0; 32]
            || self.canonical_request_digest == [0; 32]
            || self.signed_transaction_wire_hash == [0; 32]
            || self.carrier_height == 0
            || matches!(self.outcome, KagemushaOperationOutcomeStateV4::Pending)
                != self.result_hash.is_none()
        {
            return Err(KagemushaOperationOutcomeErrorV4::InvalidRecord);
        }
        Ok(())
    }

    fn validate_attempt(
        &self,
        outer_authority: &AccountId,
        operation_id: [u8; 32],
    ) -> Result<(), KagemushaOperationOutcomeErrorV4> {
        self.validate_attempt_shape(operation_id)?;
        if self.outer_authority_digest != kagemusha_operation_authority_digest_v4(outer_authority)?
        {
            return Err(KagemushaOperationOutcomeErrorV4::InvalidRecord);
        }
        Ok(())
    }

    fn validate_attempt_shape(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), KagemushaOperationOutcomeErrorV4> {
        self.validate_common(operation_id)?;
        if self.outcome == KagemushaOperationOutcomeStateV4::Applied {
            return Err(KagemushaOperationOutcomeErrorV4::InvalidRecord);
        }
        Ok(())
    }

    fn validate_finality(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), KagemushaOperationOutcomeErrorV4> {
        self.validate_common(operation_id)?;
        if self.outcome == KagemushaOperationOutcomeStateV4::Rejected {
            return Err(KagemushaOperationOutcomeErrorV4::InvalidRecord);
        }
        Ok(())
    }

    fn matches_economic_identity(
        &self,
        kind: KagemushaOperationKindV4,
        request_authority_digest: [u8; 32],
        canonical_request_digest: [u8; 32],
    ) -> bool {
        self.kind == kind
            && self.request_authority_digest == request_authority_digest
            && self.canonical_request_digest == canonical_request_digest
    }

    fn has_same_economic_identity(&self, other: &Self) -> bool {
        self.operation_id == other.operation_id
            && self.matches_economic_identity(
                other.kind,
                other.request_authority_digest,
                other.canonical_request_digest,
            )
    }

    fn matches_carrier(
        &self,
        transaction: &SignedTransaction,
        carrier_kind: KagemushaOperationKindV4,
        request_authority: &AccountId,
        canonical_request_digest: [u8; 32],
    ) -> Result<bool, KagemushaOperationOutcomeErrorV4> {
        let request_authority_digest = kagemusha_operation_authority_digest_v4(request_authority)?;
        Ok(self.kind == carrier_kind
            && self.request_authority_digest == request_authority_digest
            && self.canonical_request_digest == canonical_request_digest
            && self.signed_transaction_wire_hash == signed_transaction_wire_hash_v4(transaction)?
            && self.entrypoint_hash == transaction.hash_as_entrypoint())
    }
}

/// Failure while reserving, persisting, or reading an operation outcome.
#[derive(Debug, Error)]
pub enum KagemushaOperationOutcomeErrorV4 {
    /// A canonical state key could not be constructed.
    #[error("invalid Kagemusha operation outcome state key")]
    InvalidStateKey,
    /// Canonical record encoding failed.
    #[error("failed to encode Kagemusha operation outcome: {0}")]
    Encode(#[source] norito::Error),
    /// Canonical record decoding failed.
    #[error("failed to decode Kagemusha operation outcome: {0}")]
    Decode(#[source] norito::Error),
    /// A stored record exceeded its fixed protocol limit.
    #[error("Kagemusha operation outcome exceeds its canonical byte limit")]
    RecordTooLarge,
    /// A stored record does not bind its state key or lifecycle shape.
    #[error("Kagemusha operation outcome record is inconsistent")]
    InvalidRecord,
    /// Two final results attempted to claim the same reserved record.
    #[error("Kagemusha operation outcome was finalized more than once")]
    DuplicateFinalization,
    /// An execution-phase locator could not be represented as `u64`.
    #[error("Kagemusha operation execution-phase index overflowed")]
    PhaseIndexOverflow,
    /// The reservation batch was used with another block height or execution phase.
    #[error("Kagemusha operation reservation batch has the wrong execution context")]
    ReservationContextMismatch,
    /// Two reservation or result segments claimed the same execution-phase index.
    #[error("Kagemusha operation execution-phase index is duplicated")]
    DuplicatePhaseIndex,
    /// A pending outcome from outside the active block-local batch was encountered.
    #[error("stale Kagemusha operation pending outcome is present in consensus state")]
    StalePending,
    /// Entrypoints and results did not have one-to-one cardinality.
    #[error("Kagemusha operation outcome inputs have mismatched cardinality")]
    ResultCardinalityMismatch,
    /// Finalization input differed from the exact entrypoints reserved for this batch.
    #[error("Kagemusha operation finalization input differs from its reservation batch")]
    ExecutionInputMismatch,
    /// A reservation disappeared or changed before finalization.
    #[error("Kagemusha operation block-local reservation is missing or inconsistent")]
    MissingReservation,
    /// A successful result did not carry a transaction-local fresh economic claim.
    #[error("successful Kagemusha operation has no fresh economic finality claim")]
    MissingFinalityClaim,
    /// A global economic claim conflicts with its attempt or result.
    #[error("Kagemusha operation economic finality claim is inconsistent")]
    FinalityClaimMismatch,
}

/// Exact execution slot in which a canonical operation carrier may run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaOperationExecutionLocatorV4 {
    execution_phase: KagemushaOperationExecutionPhaseV4,
    phase_index: u64,
}

impl KagemushaOperationExecutionLocatorV4 {
    /// Construct an execution locator assigned by the canonical block scheduler.
    pub(crate) const fn new(
        execution_phase: KagemushaOperationExecutionPhaseV4,
        phase_index: u64,
    ) -> Self {
        Self {
            execution_phase,
            phase_index,
        }
    }
}

/// First-carrier reservations and complete entrypoint coverage for one execution phase.
#[derive(Debug)]
pub(crate) struct KagemushaOperationReservationBatchV4 {
    carrier_height: u64,
    execution_phase: KagemushaOperationExecutionPhaseV4,
    entrypoint_hashes: std::collections::BTreeMap<u64, HashOf<TransactionEntrypoint>>,
    reservations: std::collections::BTreeMap<StatePath, KagemushaOperationOutcomeRecordV4>,
}

impl KagemushaOperationReservationBatchV4 {
    /// Start an empty block-local reservation batch.
    pub(crate) fn new(
        carrier_height: u64,
        execution_phase: KagemushaOperationExecutionPhaseV4,
    ) -> Self {
        Self {
            carrier_height,
            execution_phase,
            entrypoint_hashes: std::collections::BTreeMap::new(),
            reservations: std::collections::BTreeMap::new(),
        }
    }
}

/// One contiguous execution segment supplied for outcome finalization.
pub(crate) struct KagemushaOperationResultSegmentV4<'a, Results> {
    phase_index_base: u64,
    entrypoints: &'a [TransactionEntrypoint],
    results: Results,
}

impl<'a, Results> KagemushaOperationResultSegmentV4<'a, Results> {
    /// Bind an exact entrypoint slice to its result sequence.
    pub(crate) const fn new(
        phase_index_base: u64,
        entrypoints: &'a [TransactionEntrypoint],
        results: Results,
    ) -> Self {
        Self {
            phase_index_base,
            entrypoints,
            results,
        }
    }
}

/// Failure while consuming the one-shot signed-carrier binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum KagemushaOperationCarrierBindingErrorV4 {
    /// No canonical direct carrier was installed for the current transaction.
    #[error("the current transaction has no Kagemusha operation carrier binding")]
    MissingBinding,
    /// The executing instruction is a different operation kind.
    #[error("the Kagemusha operation kind differs from the signed carrier")]
    KindMismatch,
    /// The executing instruction uses a different operation identifier.
    #[error("the Kagemusha operation id differs from the signed carrier")]
    OperationIdMismatch,
    /// The complete request differs from the signed carrier.
    #[error("the Kagemusha operation request differs from the signed carrier")]
    RequestMismatch,
    /// The executing request failed canonical validation or encoding.
    #[error("the executing Kagemusha operation request is invalid")]
    InvalidRequest,
    /// Nested or repeated execution attempted to consume the binding again.
    #[error("the Kagemusha operation carrier binding was already consumed")]
    AlreadyConsumed,
}

/// Transaction-local proof that execution came from one exact signed singleton carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaOperationCarrierBindingV4 {
    kind: KagemushaOperationKindV4,
    operation_id: [u8; 32],
    canonical_request_digest: [u8; 32],
    consumed: bool,
}

impl KagemushaOperationCarrierBindingV4 {
    fn from_transaction(
        transaction: &SignedTransaction,
    ) -> Result<Option<Self>, KagemushaOperationCarrierErrorV4> {
        classify_kagemusha_operation_transaction_v4(transaction).map(|carrier| {
            carrier.map(|carrier| Self {
                kind: carrier.kind(),
                operation_id: carrier.operation_id(),
                canonical_request_digest: carrier.canonical_request_digest(),
                consumed: false,
            })
        })
    }

    pub(crate) fn consume(
        &mut self,
        request: KagemushaOperationRequestV4<'_>,
    ) -> Result<(), KagemushaOperationCarrierBindingErrorV4> {
        let digest = request
            .canonical_request_digest()
            .map_err(|_| KagemushaOperationCarrierBindingErrorV4::InvalidRequest)?;
        if self.kind != request.kind() {
            return Err(KagemushaOperationCarrierBindingErrorV4::KindMismatch);
        }
        if self.operation_id != request.operation_id() {
            return Err(KagemushaOperationCarrierBindingErrorV4::OperationIdMismatch);
        }
        if self.canonical_request_digest != digest {
            return Err(KagemushaOperationCarrierBindingErrorV4::RequestMismatch);
        }
        if self.consumed {
            return Err(KagemushaOperationCarrierBindingErrorV4::AlreadyConsumed);
        }
        self.consumed = true;
        Ok(())
    }
}

/// Derive the optional one-shot execution binding from a signed transaction.
pub(crate) fn signed_kagemusha_operation_carrier_binding_v4(
    transaction: &SignedTransaction,
) -> Result<Option<KagemushaOperationCarrierBindingV4>, ValidationFail> {
    KagemushaOperationCarrierBindingV4::from_transaction(transaction).map_err(|error| {
        ValidationFail::NotPermitted(format!("invalid Kagemusha operation carrier: {error}"))
    })
}

/// Hash the complete canonical V1 signed-transaction wire for outcome evidence.
///
/// Unlike [`SignedTransaction::hash`], this digest commits to the primary
/// signature and every authorization proof. The explicit domain and byte
/// length prevent this exact-byte identity from being confused with another
/// protocol hash.
///
/// # Errors
///
/// Returns an encoding or length failure when the complete transaction wire
/// cannot be represented canonically.
pub fn signed_transaction_wire_hash_v4(
    transaction: &SignedTransaction,
) -> Result<[u8; 32], KagemushaOperationOutcomeErrorV4> {
    let bytes = transaction
        .encode_wire_v1()
        .map_err(KagemushaOperationOutcomeErrorV4::Encode)?;
    let byte_len =
        u64::try_from(bytes.len()).map_err(|_| KagemushaOperationOutcomeErrorV4::RecordTooLarge)?;
    Ok(Hash::new_from_chunks(&[
        KAGEMUSHA_SIGNED_TRANSACTION_WIRE_HASH_DOMAIN_V4,
        &byte_len.to_le_bytes(),
        &bytes,
    ])
    .into())
}

/// Hash one complete canonical account identity for fixed-size outcome records.
///
/// # Errors
///
/// Returns an encoding or length failure when the account identity cannot be
/// represented canonically.
pub fn kagemusha_operation_authority_digest_v4(
    authority: &AccountId,
) -> Result<[u8; 32], KagemushaOperationOutcomeErrorV4> {
    let bytes =
        norito::encode_canonical(authority).map_err(KagemushaOperationOutcomeErrorV4::Encode)?;
    let byte_len =
        u64::try_from(bytes.len()).map_err(|_| KagemushaOperationOutcomeErrorV4::RecordTooLarge)?;
    Ok(Hash::new_from_chunks(&[
        KAGEMUSHA_OPERATION_AUTHORITY_KEY_DOMAIN_V4,
        &byte_len.to_le_bytes(),
        &bytes,
    ])
    .into())
}

/// Construct the exact per-submitter attempt key for one operation id.
pub fn kagemusha_operation_outcome_state_key_v4(
    outer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<StatePath, KagemushaOperationOutcomeErrorV4> {
    let authority_digest = kagemusha_operation_authority_digest_v4(outer_authority)?;
    kagemusha_operation_outcome_state_key_from_authority_digest_v4(authority_digest, operation_id)
}

/// Construct an attempt key from an already authenticated outer-authority digest.
///
/// # Errors
///
/// Returns an invalid-key failure for a zero authority digest, zero operation
/// identifier, or state-path encoding failure.
pub fn kagemusha_operation_outcome_state_key_from_authority_digest_v4(
    authority_digest: [u8; 32],
    operation_id: [u8; 32],
) -> Result<StatePath, KagemushaOperationOutcomeErrorV4> {
    if authority_digest == [0; 32] || operation_id == [0; 32] {
        return Err(KagemushaOperationOutcomeErrorV4::InvalidStateKey);
    }
    StatePath::from_str(&format!(
        "{KAGEMUSHA_OPERATION_OUTCOME_STATE_PREFIX_V4}{}_{}",
        hex::encode(authority_digest),
        hex::encode(operation_id)
    ))
    .map_err(|_| KagemushaOperationOutcomeErrorV4::InvalidStateKey)
}

/// Construct the globally unique economic-finality key for one operation id.
pub fn kagemusha_operation_finality_state_key_v4(
    operation_id: [u8; 32],
) -> Result<StatePath, KagemushaOperationOutcomeErrorV4> {
    if operation_id == [0; 32] {
        return Err(KagemushaOperationOutcomeErrorV4::InvalidStateKey);
    }
    StatePath::from_str(&format!(
        "{KAGEMUSHA_OPERATION_FINALITY_STATE_PREFIX_V4}{}",
        hex::encode(operation_id)
    ))
    .map_err(|_| KagemushaOperationOutcomeErrorV4::InvalidStateKey)
}

fn decode_outcome_record_v4(
    payload: &[u8],
    outer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<KagemushaOperationOutcomeRecordV4, KagemushaOperationOutcomeErrorV4> {
    if payload.len() > KAGEMUSHA_OPERATION_OUTCOME_RECORD_MAX_BYTES_V4 {
        return Err(KagemushaOperationOutcomeErrorV4::RecordTooLarge);
    }
    let record: KagemushaOperationOutcomeRecordV4 =
        norito::decode_canonical(payload).map_err(KagemushaOperationOutcomeErrorV4::Decode)?;
    record.validate_attempt(outer_authority, operation_id)?;
    Ok(record)
}

fn encode_outcome_record_v4(
    record: &KagemushaOperationOutcomeRecordV4,
) -> Result<Vec<u8>, KagemushaOperationOutcomeErrorV4> {
    record.validate_attempt_shape(record.operation_id)?;
    let payload =
        norito::encode_canonical(record).map_err(KagemushaOperationOutcomeErrorV4::Encode)?;
    if payload.len() > KAGEMUSHA_OPERATION_OUTCOME_RECORD_MAX_BYTES_V4 {
        return Err(KagemushaOperationOutcomeErrorV4::RecordTooLarge);
    }
    Ok(payload)
}

fn decode_finality_record_v4(
    payload: &[u8],
    operation_id: [u8; 32],
) -> Result<KagemushaOperationOutcomeRecordV4, KagemushaOperationOutcomeErrorV4> {
    if payload.len() > KAGEMUSHA_OPERATION_OUTCOME_RECORD_MAX_BYTES_V4 {
        return Err(KagemushaOperationOutcomeErrorV4::RecordTooLarge);
    }
    let record: KagemushaOperationOutcomeRecordV4 =
        norito::decode_canonical(payload).map_err(KagemushaOperationOutcomeErrorV4::Decode)?;
    record.validate_finality(operation_id)?;
    Ok(record)
}

fn encode_finality_record_v4(
    record: &KagemushaOperationOutcomeRecordV4,
) -> Result<Vec<u8>, KagemushaOperationOutcomeErrorV4> {
    record.validate_finality(record.operation_id)?;
    let payload =
        norito::encode_canonical(record).map_err(KagemushaOperationOutcomeErrorV4::Encode)?;
    if payload.len() > KAGEMUSHA_OPERATION_OUTCOME_RECORD_MAX_BYTES_V4 {
        return Err(KagemushaOperationOutcomeErrorV4::RecordTooLarge);
    }
    Ok(payload)
}

/// Load the globally applied record for one operation id.
///
/// # Errors
///
/// Returns a key, decoding, size, or consistency failure for malformed
/// consensus state. A pending claim is rejected because it must never survive
/// block finalization.
pub fn kagemusha_operation_finality_v4(
    world: &impl WorldReadOnly,
    operation_id: [u8; 32],
) -> Result<Option<KagemushaOperationOutcomeRecordV4>, KagemushaOperationOutcomeErrorV4> {
    let key = kagemusha_operation_finality_state_key_v4(operation_id)?;
    let Some(payload) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let record = decode_finality_record_v4(payload, operation_id)?;
    if record.outcome != KagemushaOperationOutcomeStateV4::Applied {
        return Err(KagemushaOperationOutcomeErrorV4::InvalidRecord);
    }
    Ok(Some(record))
}

/// Load the latest rejected attempt for one submitter and operation id.
///
/// # Errors
///
/// Returns a key, decoding, size, or consistency failure for malformed
/// consensus state. A pending attempt is rejected because it must never survive
/// block finalization.
pub fn kagemusha_operation_attempt_v4(
    world: &impl WorldReadOnly,
    outer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<Option<KagemushaOperationOutcomeRecordV4>, KagemushaOperationOutcomeErrorV4> {
    let key = kagemusha_operation_outcome_state_key_v4(outer_authority, operation_id)?;
    let Some(payload) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let record = decode_outcome_record_v4(payload, outer_authority, operation_id)?;
    if record.outcome != KagemushaOperationOutcomeStateV4::Rejected {
        return Err(KagemushaOperationOutcomeErrorV4::InvalidRecord);
    }
    Ok(Some(record))
}

/// Load global economic finality or the submitter's latest rejected attempt.
///
/// A globally applied record always wins, regardless of which outer authority
/// asks. This keeps economic finality globally keyed by `operation_id` while a
/// rejected copied carrier remains isolated to its own submitter.
///
/// # Errors
///
/// Returns a key, decoding, size, or consistency failure for malformed
/// consensus state. A pending record is also rejected because it must never be
/// visible after block commit.
pub fn kagemusha_operation_outcome_v4(
    world: &impl WorldReadOnly,
    outer_authority: &AccountId,
    operation_id: [u8; 32],
) -> Result<Option<KagemushaOperationOutcomeRecordV4>, KagemushaOperationOutcomeErrorV4> {
    if let Some(finality) = kagemusha_operation_finality_v4(world, operation_id)? {
        return Ok(Some(finality));
    }
    kagemusha_operation_attempt_v4(world, outer_authority, operation_id)
}

/// Reserve the first canonical carrier for each per-submitter attempt key.
///
/// Later carriers with the same key remain non-owning: their distinct phase
/// index makes stateful admission reject them while the first carrier keeps the
/// sole attempt record. Different submitters may independently attempt the same
/// global operation. A rejected attempt may be replaced only by the same
/// logical request, while an applied global record prevents new reservations.
/// Any pending record not owned by `batch` is stale consensus state.
pub(crate) fn reserve_kagemusha_operation_outcomes_v4(
    state_block: &mut StateBlock<'_>,
    batch: &mut KagemushaOperationReservationBatchV4,
    phase_index_base: u64,
    entrypoints: &[TransactionEntrypoint],
) -> Result<(), KagemushaOperationOutcomeErrorV4> {
    if batch.carrier_height != state_block._curr_block.height().get() {
        return Err(KagemushaOperationOutcomeErrorV4::ReservationContextMismatch);
    }

    let mut planned_entrypoints = std::collections::BTreeMap::new();
    let mut planned_reservations = std::collections::BTreeMap::new();
    let mut planned_payloads = std::collections::BTreeMap::new();
    for (offset, entrypoint) in entrypoints.iter().enumerate() {
        let offset = u64::try_from(offset)
            .map_err(|_| KagemushaOperationOutcomeErrorV4::PhaseIndexOverflow)?;
        let phase_index = phase_index_base
            .checked_add(offset)
            .ok_or(KagemushaOperationOutcomeErrorV4::PhaseIndexOverflow)?;
        if batch.entrypoint_hashes.contains_key(&phase_index)
            || planned_entrypoints
                .insert(phase_index, entrypoint.hash())
                .is_some()
        {
            return Err(KagemushaOperationOutcomeErrorV4::DuplicatePhaseIndex);
        }

        let carrier = match classify_kagemusha_operation_entrypoint_v4(entrypoint) {
            Ok(Some(carrier)) => carrier,
            Ok(None) | Err(_) => continue,
        };
        let TransactionEntrypoint::External(transaction) = entrypoint else {
            return Err(KagemushaOperationOutcomeErrorV4::ExecutionInputMismatch);
        };
        let key = kagemusha_operation_outcome_state_key_v4(
            transaction.authority(),
            carrier.operation_id(),
        )?;

        // The first carrier in canonical phase order owns the reservation. A
        // later occurrence remains in `entrypoint_hashes`, but never replaces
        // that owner or receives a second finalization.
        if batch.reservations.contains_key(&key) || planned_reservations.contains_key(&key) {
            continue;
        }

        let finality_key = kagemusha_operation_finality_state_key_v4(carrier.operation_id())?;
        let finality_payload = state_block.world.smart_contract_state.get(&finality_key);
        crate::sumeragi::witness::record_read_kagemusha_operation_outcome_v4(
            &finality_key,
            finality_payload.map(Vec::as_slice),
        );
        if let Some(payload) = finality_payload {
            let finality = decode_finality_record_v4(payload, carrier.operation_id())?;
            match finality.outcome {
                KagemushaOperationOutcomeStateV4::Applied => continue,
                KagemushaOperationOutcomeStateV4::Pending => {
                    let owned_by_batch = batch
                        .reservations
                        .values()
                        .any(|reservation| reservation == &finality);
                    if !owned_by_batch {
                        return Err(KagemushaOperationOutcomeErrorV4::StalePending);
                    }
                }
                KagemushaOperationOutcomeStateV4::Rejected => {
                    return Err(KagemushaOperationOutcomeErrorV4::InvalidRecord);
                }
            }
        }

        let record = KagemushaOperationOutcomeRecordV4 {
            version: KAGEMUSHA_OPERATION_OUTCOME_RECORD_VERSION_V4,
            operation_id: carrier.operation_id(),
            kind: carrier.kind(),
            request_authority_digest: kagemusha_operation_authority_digest_v4(
                &carrier.request().authorization().authority,
            )?,
            outer_authority_digest: kagemusha_operation_authority_digest_v4(
                transaction.authority(),
            )?,
            canonical_request_digest: carrier.canonical_request_digest(),
            signed_transaction_wire_hash: signed_transaction_wire_hash_v4(transaction)?,
            entrypoint_hash: entrypoint.hash(),
            carrier_height: batch.carrier_height,
            execution_phase: batch.execution_phase,
            phase_index,
            result_hash: None,
            outcome: KagemushaOperationOutcomeStateV4::Pending,
        };

        let current_payload = state_block.world.smart_contract_state.get(&key);
        crate::sumeragi::witness::record_read_kagemusha_operation_outcome_v4(
            &key,
            current_payload.map(Vec::as_slice),
        );
        if let Some(payload) = current_payload {
            let current =
                decode_outcome_record_v4(payload, transaction.authority(), carrier.operation_id())?;
            if current.outcome == KagemushaOperationOutcomeStateV4::Pending {
                return Err(KagemushaOperationOutcomeErrorV4::StalePending);
            }
            if !current.has_same_economic_identity(&record) {
                // A terminal attempt belongs to the request that created it. A
                // different request using the same submitter and operation id
                // receives no reservation and is rejected locally by execution
                // validation; it must not invalidate the containing batch.
                continue;
            }
        }
        planned_payloads.insert(key.clone(), encode_outcome_record_v4(&record)?);
        planned_reservations.insert(key, record);
    }

    batch.entrypoint_hashes.extend(planned_entrypoints);
    batch.reservations.extend(planned_reservations);
    for (key, payload) in planned_payloads {
        state_block.world.smart_contract_state.insert(key, payload);
    }
    Ok(())
}

/// Stage the global claim owned by a freshly applied economic operation.
///
/// This must be called from the fresh branch of the Kagemusha instruction only
/// after every economic, hardware, and replay write has succeeded. Because the
/// claim lives in the same [`StateTransaction`], any later transaction failure
/// rolls it back together with the economic effects.
///
/// # Errors
///
/// Returns an error unless the current execution slot owns an exact pending
/// attempt and the global operation id remains unclaimed.
pub(crate) fn stage_kagemusha_operation_finality_claim_v4(
    state_transaction: &mut StateTransaction<'_, '_>,
    outer_authority: &AccountId,
    request: KagemushaOperationRequestV4<'_>,
) -> Result<(), KagemushaOperationOutcomeErrorV4> {
    let operation_id = request.operation_id();
    let attempt_key = kagemusha_operation_outcome_state_key_v4(outer_authority, operation_id)?;
    let attempt_payload = state_transaction
        .world
        .smart_contract_state
        .get(&attempt_key)
        .ok_or(KagemushaOperationOutcomeErrorV4::MissingReservation)?;
    crate::sumeragi::witness::record_read_kagemusha_operation_outcome_v4(
        &attempt_key,
        Some(attempt_payload.as_slice()),
    );
    let attempt = decode_outcome_record_v4(attempt_payload, outer_authority, operation_id)?;
    let locator = state_transaction
        .kagemusha_operation_execution_locator
        .ok_or(KagemushaOperationOutcomeErrorV4::ReservationContextMismatch)?;
    let canonical_request_digest = request
        .canonical_request_digest()
        .map_err(|_| KagemushaOperationOutcomeErrorV4::InvalidRecord)?;
    let request_authority_digest =
        kagemusha_operation_authority_digest_v4(&request.authorization().authority)?;
    if attempt.outcome != KagemushaOperationOutcomeStateV4::Pending
        || attempt.carrier_height != state_transaction.block_height()
        || attempt.execution_phase != locator.execution_phase
        || attempt.phase_index != locator.phase_index
        || !attempt.matches_economic_identity(
            request.kind(),
            request_authority_digest,
            canonical_request_digest,
        )
    {
        return Err(KagemushaOperationOutcomeErrorV4::ReservationContextMismatch);
    }

    let finality_key = kagemusha_operation_finality_state_key_v4(operation_id)?;
    let finality_payload = state_transaction
        .world
        .smart_contract_state
        .get(&finality_key);
    crate::sumeragi::witness::record_read_kagemusha_operation_outcome_v4(
        &finality_key,
        finality_payload.map(Vec::as_slice),
    );
    if finality_payload.is_some() {
        return Err(KagemushaOperationOutcomeErrorV4::FinalityClaimMismatch);
    }
    let claim_payload = encode_finality_record_v4(&attempt)?;
    crate::sumeragi::witness::record_write_kagemusha_operation_outcome_v4(
        &finality_key,
        &claim_payload,
    );
    state_transaction
        .world
        .smart_contract_state
        .insert(finality_key, claim_payload);
    Ok(())
}

/// Require an exact block-local reservation before executing a carrier.
pub(crate) fn validate_kagemusha_operation_reservation_v4(
    transaction: &SignedTransaction,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), ValidationFail> {
    let Some(carrier) = classify_kagemusha_operation_transaction_v4(transaction)
        .map_err(|error| ValidationFail::NotPermitted(error.to_string()))?
    else {
        return Ok(());
    };
    let finality_key = kagemusha_operation_finality_state_key_v4(carrier.operation_id())
        .map_err(|error| ValidationFail::InternalError(error.to_string()))?;
    let finality_payload = state_transaction
        .world
        .smart_contract_state
        .get(&finality_key);
    crate::sumeragi::witness::record_read_kagemusha_operation_outcome_v4(
        &finality_key,
        finality_payload.map(Vec::as_slice),
    );
    let pending_finality = finality_payload
        .map(|payload| decode_finality_record_v4(payload, carrier.operation_id()))
        .transpose()
        .map_err(|error| ValidationFail::InternalError(error.to_string()))?;
    let request_authority_digest =
        kagemusha_operation_authority_digest_v4(&carrier.request().authorization().authority)
            .map_err(|error| ValidationFail::InternalError(error.to_string()))?;
    if pending_finality
        .as_ref()
        .is_some_and(|record| record.outcome == KagemushaOperationOutcomeStateV4::Applied)
    {
        return Err(ValidationFail::NotPermitted(
            "Kagemusha operation id is already economically final".to_owned(),
        ));
    }
    if pending_finality.as_ref().is_some_and(|record| {
        !record.matches_economic_identity(
            carrier.kind(),
            request_authority_digest,
            carrier.canonical_request_digest(),
        )
    }) {
        return Err(ValidationFail::NotPermitted(
            "Kagemusha operation id is claimed by another request".to_owned(),
        ));
    }
    let key =
        kagemusha_operation_outcome_state_key_v4(transaction.authority(), carrier.operation_id())
            .map_err(|error| ValidationFail::InternalError(error.to_string()))?;
    let payload = state_transaction
        .world
        .smart_contract_state
        .get(&key)
        .ok_or_else(|| {
            ValidationFail::NotPermitted(
                "Kagemusha operation has no canonical block-local reservation".to_owned(),
            )
        })?;
    let record = decode_outcome_record_v4(payload, transaction.authority(), carrier.operation_id())
        .map_err(|error| ValidationFail::InternalError(error.to_string()))?;
    if record.outcome != KagemushaOperationOutcomeStateV4::Pending {
        return Err(ValidationFail::NotPermitted(
            "Kagemusha operation id is already terminal".to_owned(),
        ));
    }
    let locator = state_transaction
        .kagemusha_operation_execution_locator
        .ok_or_else(|| {
            ValidationFail::NotPermitted(
                "Kagemusha operation has no canonical execution-slot binding".to_owned(),
            )
        })?;
    if record.carrier_height != state_transaction.block_height()
        || record.execution_phase != locator.execution_phase
        || record.phase_index != locator.phase_index
    {
        return Err(ValidationFail::NotPermitted(
            "Kagemusha operation is reserved for another execution slot".to_owned(),
        ));
    }
    if let Some(finality) = pending_finality
        && (finality.carrier_height != state_transaction.block_height()
            || (finality.execution_phase, finality.phase_index)
                > (locator.execution_phase, locator.phase_index))
    {
        return Err(ValidationFail::NotPermitted(
            "Kagemusha operation carries a stale or future economic claim".to_owned(),
        ));
    }
    if !record
        .matches_carrier(
            transaction,
            carrier.kind(),
            &carrier.request().authorization().authority,
            carrier.canonical_request_digest(),
        )
        .map_err(|error| ValidationFail::InternalError(error.to_string()))?
    {
        return Err(ValidationFail::NotPermitted(
            "Kagemusha operation id is already reserved by another carrier".to_owned(),
        ));
    }
    Ok(())
}

/// Finalize every reservation owned by `batch` from its exact indexed result.
///
/// Every reserved entrypoint and every result segment must match one-to-one.
/// Missing, reordered, duplicated, or mutated inputs abort instead of leaving a
/// pending record commit-visible. Reservations are consumed in canonical phase
/// order rather than state-key order. A successful result becomes globally
/// applied only when the transaction's fresh economic branch staged the exact
/// claim; a later same-operation no-op may observe that earlier application but
/// can never replace its owner.
pub(crate) fn finalize_kagemusha_operation_outcomes_v4<'a, Results>(
    state_block: &mut StateBlock<'_>,
    batch: KagemushaOperationReservationBatchV4,
    segments: impl IntoIterator<Item = KagemushaOperationResultSegmentV4<'a, Results>>,
) -> Result<(), KagemushaOperationOutcomeErrorV4>
where
    Results: IntoIterator + 'a,
    Results::IntoIter: ExactSizeIterator,
    Results::Item: std::ops::Deref<Target = TransactionResult>,
{
    if batch.carrier_height != state_block._curr_block.height().get() {
        return Err(KagemushaOperationOutcomeErrorV4::ReservationContextMismatch);
    }

    let mut observed = std::collections::BTreeMap::new();
    for segment in segments {
        let results = segment.results.into_iter();
        if segment.entrypoints.len() != results.len() {
            return Err(KagemushaOperationOutcomeErrorV4::ResultCardinalityMismatch);
        }
        for (offset, (entrypoint, result)) in segment.entrypoints.iter().zip(results).enumerate() {
            let offset = u64::try_from(offset)
                .map_err(|_| KagemushaOperationOutcomeErrorV4::PhaseIndexOverflow)?;
            let phase_index = segment
                .phase_index_base
                .checked_add(offset)
                .ok_or(KagemushaOperationOutcomeErrorV4::PhaseIndexOverflow)?;
            if observed.insert(phase_index, (entrypoint, result)).is_some() {
                return Err(KagemushaOperationOutcomeErrorV4::DuplicatePhaseIndex);
            }
        }
    }
    if observed.len() != batch.entrypoint_hashes.len()
        || batch.entrypoint_hashes.iter().any(|(phase_index, hash)| {
            observed
                .get(phase_index)
                .is_none_or(|(entrypoint, _)| entrypoint.hash() != *hash)
        })
    {
        return Err(KagemushaOperationOutcomeErrorV4::ExecutionInputMismatch);
    }

    let mut reservations: Vec<_> = batch.reservations.into_iter().collect();
    reservations.sort_unstable_by_key(|(_, record)| record.phase_index);

    let mut attempt_updates = std::collections::BTreeMap::new();
    let mut finality_records = std::collections::BTreeMap::new();
    let mut finality_updates = std::collections::BTreeMap::new();
    for (key, expected) in reservations {
        let (entrypoint, result) = observed
            .get(&expected.phase_index)
            .ok_or(KagemushaOperationOutcomeErrorV4::ExecutionInputMismatch)?;
        let result = std::ops::Deref::deref(result);
        let carrier = classify_kagemusha_operation_entrypoint_v4(entrypoint)
            .map_err(|_| KagemushaOperationOutcomeErrorV4::ExecutionInputMismatch)?
            .ok_or(KagemushaOperationOutcomeErrorV4::ExecutionInputMismatch)?;
        let TransactionEntrypoint::External(transaction) = entrypoint else {
            return Err(KagemushaOperationOutcomeErrorV4::ExecutionInputMismatch);
        };
        let observed_key = kagemusha_operation_outcome_state_key_v4(
            transaction.authority(),
            carrier.operation_id(),
        )?;
        if observed_key != key
            || expected.execution_phase != batch.execution_phase
            || !expected.matches_carrier(
                transaction,
                carrier.kind(),
                &carrier.request().authorization().authority,
                carrier.canonical_request_digest(),
            )?
        {
            return Err(KagemushaOperationOutcomeErrorV4::ExecutionInputMismatch);
        }
        let payload = state_block
            .world
            .smart_contract_state
            .get(&key)
            .ok_or(KagemushaOperationOutcomeErrorV4::MissingReservation)?;
        let mut attempt =
            decode_outcome_record_v4(payload, transaction.authority(), carrier.operation_id())?;
        if attempt != expected || attempt.outcome != KagemushaOperationOutcomeStateV4::Pending {
            return Err(KagemushaOperationOutcomeErrorV4::MissingReservation);
        }

        let finality_key = kagemusha_operation_finality_state_key_v4(expected.operation_id)?;
        if !finality_records.contains_key(&finality_key) {
            let finality_payload = state_block.world.smart_contract_state.get(&finality_key);
            crate::sumeragi::witness::record_read_kagemusha_operation_outcome_v4(
                &finality_key,
                finality_payload.map(Vec::as_slice),
            );
            let finality = finality_payload
                .map(|payload| decode_finality_record_v4(payload, expected.operation_id))
                .transpose()?;
            finality_records.insert(finality_key.clone(), finality);
        }
        let finality = finality_records
            .get_mut(&finality_key)
            .ok_or(KagemushaOperationOutcomeErrorV4::FinalityClaimMismatch)?;

        if result.is_ok() {
            let claimed = finality
                .as_mut()
                .ok_or(KagemushaOperationOutcomeErrorV4::MissingFinalityClaim)?;
            match claimed.outcome {
                KagemushaOperationOutcomeStateV4::Pending => {
                    if &*claimed != &expected {
                        return Err(KagemushaOperationOutcomeErrorV4::FinalityClaimMismatch);
                    }
                    claimed.result_hash = Some(result.hash());
                    claimed.outcome = KagemushaOperationOutcomeStateV4::Applied;
                    if finality_updates
                        .insert(finality_key, encode_finality_record_v4(claimed)?)
                        .is_some()
                    {
                        return Err(KagemushaOperationOutcomeErrorV4::DuplicateFinalization);
                    }
                }
                KagemushaOperationOutcomeStateV4::Applied => {
                    let claim_position = (
                        claimed.carrier_height,
                        claimed.execution_phase,
                        claimed.phase_index,
                    );
                    let attempt_position = (
                        expected.carrier_height,
                        expected.execution_phase,
                        expected.phase_index,
                    );
                    if !claimed.has_same_economic_identity(&expected)
                        || claim_position >= attempt_position
                    {
                        return Err(KagemushaOperationOutcomeErrorV4::FinalityClaimMismatch);
                    }
                }
                KagemushaOperationOutcomeStateV4::Rejected => {
                    return Err(KagemushaOperationOutcomeErrorV4::InvalidRecord);
                }
            }
            if attempt_updates.insert(key, None).is_some() {
                return Err(KagemushaOperationOutcomeErrorV4::DuplicateFinalization);
            }
        } else {
            if finality.as_ref().is_some_and(|claim| claim == &expected) {
                return Err(KagemushaOperationOutcomeErrorV4::FinalityClaimMismatch);
            }
            attempt.result_hash = Some(result.hash());
            attempt.outcome = KagemushaOperationOutcomeStateV4::Rejected;
            if attempt_updates
                .insert(key, Some(encode_outcome_record_v4(&attempt)?))
                .is_some()
            {
                return Err(KagemushaOperationOutcomeErrorV4::DuplicateFinalization);
            }
        }
    }
    if finality_records.values().any(|record| {
        record
            .as_ref()
            .is_some_and(|record| record.outcome == KagemushaOperationOutcomeStateV4::Pending)
    }) {
        return Err(KagemushaOperationOutcomeErrorV4::FinalityClaimMismatch);
    }

    for (key, payload) in attempt_updates {
        match payload {
            Some(payload) => {
                crate::sumeragi::witness::record_write_kagemusha_operation_outcome_v4(
                    &key, &payload,
                );
                state_block.world.smart_contract_state.insert(key, payload);
            }
            None => {
                crate::sumeragi::witness::record_write_kagemusha_operation_outcome_v4(&key, &[]);
                state_block.world.smart_contract_state.remove(key);
            }
        }
    }
    for (key, payload) in finality_updates {
        crate::sumeragi::witness::record_write_kagemusha_operation_outcome_v4(&key, &payload);
        state_block.world.smart_contract_state.insert(key, payload);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::{NonZeroU32, NonZeroU64};

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::{AccountId, MultisigMember, MultisigPolicy},
        asset::{AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::DomainId,
        events::execute_trigger::ExecuteTriggerEventFilter,
        isi::{Log, Register, offline::TopUpKagemushaRecursiveV4},
        level::Level,
        offline::{
            KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2,
            KagemushaAndroidKeyMintHardwareAssertionV1, KagemushaDeviceSignatureV2,
            KagemushaOnlineHardwareAssertionV1, KagemushaRecursiveSpendArtifactBindingV4,
            KagemushaRecursiveSpendTopUpRequestV4, KagemushaRequestAuthorizationV2,
            KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
            KagemushaTopUpShieldEvidenceV2,
        },
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        transaction::{ExecutionStep, FeePaymentIntent, TransactionBuilder},
        trigger::{
            DataTriggerSequence, TimeTriggerEntrypoint, Trigger,
            prelude::{Action, Repeats},
        },
    };
    use iroha_executor_data_model::isi::multisig::MultisigPropose;

    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn test_network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"Kagemusha operation outcome tests",
        )))
    }

    fn top_up_request(operation_id: [u8; 32]) -> KagemushaRecursiveSpendTopUpRequestV4 {
        top_up_request_with_note_commitment(operation_id, [0x61; 32])
    }

    fn top_up_request_with_note_commitment(
        operation_id: [u8; 32],
        note_commitment: [u8; 32],
    ) -> KagemushaRecursiveSpendTopUpRequestV4 {
        let request_key = KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519)
            .expect("derive request authority");
        let authority = AccountId::new(request_key.public_key().clone());
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("offline", "universal").expect("fixture domain"),
            "outcome".parse().expect("fixture asset name"),
        );
        let amount = KagemushaScaledAmountV2 {
            atomic_units: 7,
            scale: 0,
        };
        let issued_at_ms = 1;
        let mut request = KagemushaRecursiveSpendTopUpRequestV4 {
            version: 4,
            asset: AssetId::new(definition.clone(), authority.clone()),
            amount,
            current_note: KagemushaSpendableNoteDescriptorV2 {
                network_id: test_network_id(),
                asset: definition.clone(),
                note_commitment,
                spend_nullifier: [0x62; 32],
                amount,
            },
            shield_evidence: KagemushaTopUpShieldEvidenceV2 {
                initial_root: [0x65; 32],
                finalized_root: [0x66; 32],
                leaf_index: 0,
                proof: {
                    let backend = "halo2/ipa";
                    let mut attachment = ProofAttachment::new_ref(
                        backend.into(),
                        ProofBox::new(backend.to_owned(), vec![0x67]),
                        VerifyingKeyId::new(backend, "kagemusha-topup-shield-v2"),
                    );
                    attachment.vk_commitment = Some([0x68; 32]);
                    attachment
                },
            },
            artifact_binding: KagemushaRecursiveSpendArtifactBindingV4 {
                version: 4,
                generation: "outcome-test".to_owned(),
                manifest_sha256: [0x69; 32],
            },
            operation_id,
            authorization: KagemushaRequestAuthorizationV2 {
                authority,
                device_id: "outcome-test-device".to_owned(),
                asset_definition_id: definition,
                operation_id,
                issued_at_ms,
                expires_at_ms: issued_at_ms + KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2,
                nonce: [0x63; 32],
                payload_digest: [0x64; 32],
                registration_hash: Hash::new([0x6A; 32]).into(),
                hardware_assertion: KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
                    KagemushaAndroidKeyMintHardwareAssertionV1 {
                        signature: KagemushaDeviceSignatureV2::from_raw_bytes(&[1_u8; 64])
                            .expect("placeholder signature"),
                    },
                ),
            },
        };
        request.authorization.payload_digest = request
            .unsigned_payload_digest()
            .expect("derive request payload digest");
        let signing_bytes = request
            .authorization
            .signing_bytes()
            .expect("encode request signing bytes");
        use p256::ecdsa::signature::Signer as _;
        let hardware_key =
            p256::ecdsa::SigningKey::from_slice(&[1_u8; 32]).expect("fixture P-256 key");
        let signature: p256::ecdsa::Signature = hardware_key.sign(&signing_bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        request.authorization.set_hardware_signature(
            KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_slice())
                .expect("canonical hardware signature"),
        );
        request
            .authorization
            .verify_hardware_signature(
                hardware_key
                    .verifying_key()
                    .to_encoded_point(false)
                    .as_bytes(),
            )
            .expect("fixture hardware signature must verify");
        request
    }

    fn top_up_carrier(
        outer_key: &KeyPair,
        request: KagemushaRecursiveSpendTopUpRequestV4,
        nonce: u32,
    ) -> SignedTransaction {
        instruction_carrier(
            outer_key,
            InstructionBox::from(TopUpKagemushaRecursiveV4::new(request)),
            nonce,
        )
    }

    fn instruction_carrier(
        outer_key: &KeyPair,
        instruction: InstructionBox,
        nonce: u32,
    ) -> SignedTransaction {
        let mut builder = TransactionBuilder::new(
            test_network_id(),
            AccountId::new(outer_key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_nonce(NonZeroU32::new(nonce).expect("non-zero fixture nonce"));
        builder
            .with_instructions([instruction])
            .sign(outer_key.private_key())
    }

    fn trigger_registration_with(
        authority: AccountId,
        trigger_name: &str,
        instructions: Vec<InstructionBox>,
    ) -> InstructionBox {
        let trigger_id = trigger_name.parse().expect("valid fixture trigger id");
        let action = Action::new(
            instructions,
            Repeats::Indefinitely,
            authority,
            ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
        )
        .expect("valid fixture trigger action");
        InstructionBox::from(Register::trigger(Trigger::new(trigger_id, action)))
    }

    fn test_state() -> State {
        State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn stage_top_up_finality_claim(
        block: &mut StateBlock<'_>,
        outer_authority: &AccountId,
        request: &KagemushaRecursiveSpendTopUpRequestV4,
        phase_index: u64,
    ) {
        let mut transaction = block.transaction();
        transaction.bind_kagemusha_operation_execution_locator_v4(Some(
            KagemushaOperationExecutionLocatorV4::new(
                KagemushaOperationExecutionPhaseV4::Ordinary,
                phase_index,
            ),
        ));
        stage_kagemusha_operation_finality_claim_v4(
            &mut transaction,
            outer_authority,
            KagemushaOperationRequestV4::TopUp(request),
        )
        .expect("fresh economic execution must stage its exact finality claim");
        transaction.apply();
    }

    fn rejected_result(message: &str) -> TransactionResult {
        TransactionResult::new(Err(
            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(message.to_owned()),
            ),
        ))
    }

    fn outer_keys_in_reverse_state_key_order(operation_id: [u8; 32]) -> (KeyPair, KeyPair) {
        let mut first = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
            .expect("derive first outer authority");
        let mut second = KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519)
            .expect("derive second outer authority");
        let first_key = kagemusha_operation_outcome_state_key_v4(
            &AccountId::new(first.public_key().clone()),
            operation_id,
        )
        .expect("first attempt key");
        let second_key = kagemusha_operation_outcome_state_key_v4(
            &AccountId::new(second.public_key().clone()),
            operation_id,
        )
        .expect("second attempt key");
        if first_key < second_key {
            std::mem::swap(&mut first, &mut second);
        }
        (first, second)
    }

    #[test]
    fn complete_classifier_rejects_deferred_and_non_external_carriers() {
        let operation_id = [0xB2; 32];
        let outer_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(outer_key.public_key().clone());
        let operation =
            InstructionBox::from(TopUpKagemushaRecursiveV4::new(top_up_request(operation_id)));

        let direct = instruction_carrier(&outer_key, operation.clone(), 1);
        assert!(
            classify_kagemusha_operation_transaction_v4(&direct)
                .expect("classify direct carrier")
                .is_some(),
            "the direct singleton external carrier must remain canonical"
        );

        let trigger = trigger_registration_with(
            authority.clone(),
            "kagemusha_in_trigger",
            vec![operation.clone()],
        );
        let proposal = InstructionBox::from(MultisigPropose::new(
            authority.clone(),
            vec![operation.clone()],
            None,
        ));
        let nested_trigger = trigger_registration_with(
            authority.clone(),
            "kagemusha_deep_trigger",
            vec![proposal.clone()],
        );
        let nested_proposal = InstructionBox::from(MultisigPropose::new(
            authority.clone(),
            vec![nested_trigger],
            None,
        ));

        for (nonce, instruction) in [trigger, proposal.clone(), nested_proposal]
            .into_iter()
            .enumerate()
        {
            let nonce = u32::try_from(nonce + 2).expect("small fixture nonce");
            let transaction = instruction_carrier(&outer_key, instruction, nonce);
            assert!(
                classify_direct_kagemusha_operation_transaction_v4(&transaction)
                    .expect("the direct classifier sees an ordinary wrapper")
                    .is_none(),
                "the fixture must exercise Core's executor-aware traversal"
            );
            assert!(matches!(
                classify_kagemusha_operation_transaction_v4(&transaction),
                Err(KagemushaOperationCarrierErrorV4::NonCanonicalExecutable)
            ));
        }

        for (trigger_name, instruction) in [
            ("kagemusha_time_direct", operation),
            ("kagemusha_time_nested", proposal),
        ] {
            let entrypoint = TransactionEntrypoint::Time(TimeTriggerEntrypoint {
                id: trigger_name.parse().expect("valid time-trigger id"),
                instructions: ExecutionStep(vec![instruction].into()),
                authority: authority.clone(),
            });
            assert!(matches!(
                classify_kagemusha_operation_entrypoint_v4(&entrypoint),
                Err(KagemushaOperationCarrierErrorV4::NonExternalEntrypoint)
            ));
        }
    }

    #[test]
    fn attempt_key_is_composite_while_finality_key_is_global() {
        let first = AccountId::new(
            KeyPair::random_with_algorithm(Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let second = AccountId::new(
            KeyPair::random_with_algorithm(Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let operation_id = [0xA5; 32];
        assert_ne!(
            kagemusha_operation_outcome_state_key_v4(&first, operation_id).expect("first key"),
            kagemusha_operation_outcome_state_key_v4(&second, operation_id).expect("second key")
        );
        let global_key =
            kagemusha_operation_finality_state_key_v4(operation_id).expect("global key");
        assert_ne!(
            global_key,
            kagemusha_operation_outcome_state_key_v4(&first, operation_id)
                .expect("first attempt key")
        );
        assert_ne!(
            global_key,
            kagemusha_operation_finality_state_key_v4([0xA6; 32]).expect("different operation key")
        );
        assert!(
            kagemusha_operation_outcome_state_key_v4(&first, [0; 32]).is_err(),
            "zero operation ids must not enter consensus state"
        );
        assert!(
            kagemusha_operation_finality_state_key_v4([0; 32]).is_err(),
            "zero operation ids must not enter global consensus state"
        );
    }

    #[test]
    fn signed_wire_hash_distinguishes_authorization_proofs_for_one_intent() {
        let signer_a = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let signer_b = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let policy = MultisigPolicy::new(
            1,
            vec![
                MultisigMember::new(signer_a.public_key().clone(), 1).expect("member A"),
                MultisigMember::new(signer_b.public_key().clone(), 1).expect("member B"),
            ],
        )
        .expect("one-of-two policy");
        let builder = TransactionBuilder::new(
            test_network_id(),
            AccountId::new_multisig(policy),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "same intent".to_owned())]);
        let first = builder.clone().sign_multisig([signer_a.private_key()]);
        let second = builder.sign_multisig([signer_b.private_key()]);

        assert_eq!(first.hash(), second.hash());
        assert_eq!(first.hash_as_entrypoint(), second.hash_as_entrypoint());
        assert_ne!(
            signed_transaction_wire_hash_v4(&first).expect("first wire hash"),
            signed_transaction_wire_hash_v4(&second).expect("second wire hash")
        );
    }

    #[test]
    fn variable_size_outer_authority_cannot_expand_outcome_record() {
        let signers = (1_u16..=512)
            .map(|index| {
                let mut seed = [0_u8; 32];
                seed[..2].copy_from_slice(&index.to_le_bytes());
                KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
                    .expect("derive unique multisig member")
            })
            .collect::<Vec<_>>();
        let policy = MultisigPolicy::new(
            1,
            signers
                .iter()
                .map(|signer| {
                    MultisigMember::new(signer.public_key().clone(), 1)
                        .expect("valid multisig member")
                })
                .collect(),
        )
        .expect("large threshold-one policy remains a valid account identity");
        let outer_authority = AccountId::new_multisig(policy);
        assert!(
            norito::encode_canonical(&outer_authority)
                .expect("encode large account identity")
                .len()
                > KAGEMUSHA_OPERATION_OUTCOME_RECORD_MAX_BYTES_V4,
            "the fixture must exceed the old variable-size outcome-record ceiling"
        );

        let operation_id = [0xB1; 32];
        let request = top_up_request(operation_id);
        let transaction = TransactionBuilder::new(
            test_network_id(),
            outer_authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([TopUpKagemushaRecursiveV4::new(request)])
        .sign_multisig([signers[0].private_key()]);
        let entrypoints = vec![TransactionEntrypoint::External(transaction)];
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(&mut block, &mut batch, 0, &entrypoints)
            .expect("fixed authority digests keep reservation encoding bounded");

        let key = kagemusha_operation_outcome_state_key_v4(&outer_authority, operation_id)
            .expect("derive large-authority attempt key");
        let payload = block
            .world
            .smart_contract_state
            .get(&key)
            .expect("pending outcome record exists");
        assert!(payload.len() < KAGEMUSHA_OPERATION_OUTCOME_RECORD_MAX_BYTES_V4);
        let record = decode_outcome_record_v4(payload, &outer_authority, operation_id)
            .expect("fixed-size outcome record binds the full authority digest");
        assert_eq!(
            record.outer_authority_digest,
            kagemusha_operation_authority_digest_v4(&outer_authority)
                .expect("hash large outer authority")
        );
    }

    #[test]
    fn reservation_is_position_bound_and_first_carrier_wins() {
        let operation_id = [0xA6; 32];
        let outer_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let first = top_up_carrier(&outer_key, top_up_request(operation_id), 1);
        let second = top_up_carrier(&outer_key, top_up_request(operation_id), 2);
        let entrypoints = vec![
            TransactionEntrypoint::External(first.clone()),
            TransactionEntrypoint::External(second.clone()),
        ];
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(&mut block, &mut batch, 0, &entrypoints)
            .expect("reserve first carrier");
        assert_eq!(batch.reservations.len(), 1);

        let mut first_state_tx = block.transaction();
        first_state_tx.bind_kagemusha_operation_execution_locator_v4(Some(
            KagemushaOperationExecutionLocatorV4::new(
                KagemushaOperationExecutionPhaseV4::Ordinary,
                0,
            ),
        ));
        validate_kagemusha_operation_reservation_v4(&first, &first_state_tx)
            .expect("first carrier owns its exact slot");
        drop(first_state_tx);

        let mut second_state_tx = block.transaction();
        second_state_tx.bind_kagemusha_operation_execution_locator_v4(Some(
            KagemushaOperationExecutionLocatorV4::new(
                KagemushaOperationExecutionPhaseV4::Ordinary,
                1,
            ),
        ));
        let error = validate_kagemusha_operation_reservation_v4(&second, &second_state_tx)
            .expect_err("later duplicate must not consume the first reservation");
        assert!(error.to_string().contains("another execution slot"));
    }

    #[test]
    fn reservation_rejects_pending_not_owned_by_active_batch() {
        let operation_id = [0xA7; 32];
        let outer_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let transaction = top_up_carrier(&outer_key, top_up_request(operation_id), 1);
        let entrypoints = vec![TransactionEntrypoint::External(transaction)];
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut first_batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(&mut block, &mut first_batch, 0, &entrypoints)
            .expect("create pending reservation");
        let mut unrelated_batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        assert!(matches!(
            reserve_kagemusha_operation_outcomes_v4(
                &mut block,
                &mut unrelated_batch,
                1,
                &entrypoints,
            ),
            Err(KagemushaOperationOutcomeErrorV4::StalePending)
        ));
    }

    #[test]
    fn public_executor_rejects_carrier_without_block_reservation() {
        let outer_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let transaction = top_up_carrier(&outer_key, top_up_request([0xA9; 32]), 1);
        let authority = transaction.authority().clone();
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut state_transaction = block.transaction();
        let error = crate::executor::Executor::Initial
            .execute_transaction(
                &mut state_transaction,
                &authority,
                transaction,
                &mut crate::smartcontracts::ivm::cache::IvmCache::new(),
            )
            .expect_err("direct Executor use must not bypass outcome reservation");
        assert!(
            error
                .to_string()
                .contains("no canonical block-local reservation")
        );
    }

    #[test]
    fn finalization_requires_exact_cardinality_and_completes_every_reservation() {
        let operation_id = [0xA8; 32];
        let outer_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let request = top_up_request(operation_id);
        let transaction = top_up_carrier(&outer_key, request.clone(), 1);
        let outer_authority = transaction.authority().clone();
        let entrypoints = vec![TransactionEntrypoint::External(transaction.clone())];
        let results = vec![TransactionResult::new(Ok(DataTriggerSequence::default()))];
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(&mut block, &mut batch, 0, &entrypoints)
            .expect("reserve carrier");
        stage_top_up_finality_claim(&mut block, &outer_authority, &request, 0);
        finalize_kagemusha_operation_outcomes_v4(
            &mut block,
            batch,
            [KagemushaOperationResultSegmentV4::new(
                0,
                &entrypoints,
                results.iter(),
            )],
        )
        .expect("finalize exact result");
        let outcome = kagemusha_operation_outcome_v4(&block.world, &outer_authority, operation_id)
            .expect("decode outcome")
            .expect("outcome exists");
        assert_eq!(outcome.outcome, KagemushaOperationOutcomeStateV4::Applied);
        assert_eq!(
            outcome.signed_transaction_wire_hash,
            signed_transaction_wire_hash_v4(&transaction).expect("wire hash")
        );
        assert!(
            kagemusha_operation_attempt_v4(&block.world, &outer_authority, operation_id)
                .expect("attempt lookup")
                .is_none(),
            "a globally applied operation must not retain a pending attempt"
        );

        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(&mut block, &mut batch, 0, &entrypoints)
            .expect("reserve carrier for cardinality check");
        assert!(matches!(
            finalize_kagemusha_operation_outcomes_v4(
                &mut block,
                batch,
                [KagemushaOperationResultSegmentV4::new(
                    0,
                    &entrypoints,
                    std::iter::empty::<&TransactionResult>(),
                )],
            ),
            Err(KagemushaOperationOutcomeErrorV4::ResultCardinalityMismatch)
        ));
    }

    #[test]
    fn successful_result_without_fresh_claim_fails_closed() {
        let operation_id = [0xAB; 32];
        let outer_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let transaction = top_up_carrier(&outer_key, top_up_request(operation_id), 1);
        let entrypoints = vec![TransactionEntrypoint::External(transaction)];
        let results = vec![TransactionResult::new(Ok(DataTriggerSequence::default()))];
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(&mut block, &mut batch, 0, &entrypoints)
            .expect("reserve carrier");

        assert!(matches!(
            finalize_kagemusha_operation_outcomes_v4(
                &mut block,
                batch,
                [KagemushaOperationResultSegmentV4::new(
                    0,
                    &entrypoints,
                    &results,
                )],
            ),
            Err(KagemushaOperationOutcomeErrorV4::MissingFinalityClaim)
        ));
    }

    #[test]
    fn rejected_copied_carrier_cannot_poison_later_fresh_claim() {
        let operation_id = [0xAC; 32];
        let request = top_up_request(operation_id);
        let (first_outer, second_outer) = outer_keys_in_reverse_state_key_order(operation_id);
        let first = top_up_carrier(&first_outer, request.clone(), 1);
        let second = top_up_carrier(&second_outer, request.clone(), 2);
        let first_authority = first.authority().clone();
        let second_authority = second.authority().clone();
        let first_key = kagemusha_operation_outcome_state_key_v4(&first_authority, operation_id)
            .expect("first attempt key");
        let second_key = kagemusha_operation_outcome_state_key_v4(&second_authority, operation_id)
            .expect("second attempt key");
        assert!(
            first_key > second_key,
            "fixture must oppose state-key order and phase order"
        );
        let entrypoints = vec![
            TransactionEntrypoint::External(first),
            TransactionEntrypoint::External(second),
        ];
        let results = vec![
            rejected_result("copied carrier is not authorized"),
            TransactionResult::new(Ok(DataTriggerSequence::default())),
        ];
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(&mut block, &mut batch, 0, &entrypoints)
            .expect("reserve both outer-authority attempts");
        stage_top_up_finality_claim(&mut block, &second_authority, &request, 1);

        finalize_kagemusha_operation_outcomes_v4(
            &mut block,
            batch,
            [KagemushaOperationResultSegmentV4::new(
                0,
                &entrypoints,
                &results,
            )],
        )
        .expect("phase-ordered finalization must promote only the fresh claim");

        let rejected = kagemusha_operation_attempt_v4(&block.world, &first_authority, operation_id)
            .expect("rejected attempt lookup")
            .expect("rejected attempt remains observable");
        assert_eq!(rejected.outcome, KagemushaOperationOutcomeStateV4::Rejected);
        let applied = kagemusha_operation_finality_v4(&block.world, operation_id)
            .expect("finality lookup")
            .expect("fresh claim becomes final");
        assert_eq!(
            applied.outer_authority_digest,
            kagemusha_operation_authority_digest_v4(&second_authority)
                .expect("hash second outer authority")
        );
        assert_eq!(applied.phase_index, 1);
        let combined = kagemusha_operation_outcome_v4(&block.world, &first_authority, operation_id)
            .expect("combined lookup")
            .expect("global finality wins over a rejected attempt");
        assert_eq!(combined.outcome, KagemushaOperationOutcomeStateV4::Applied);
        assert_eq!(
            combined.outer_authority_digest,
            kagemusha_operation_authority_digest_v4(&second_authority)
                .expect("hash second outer authority")
        );
    }

    #[test]
    fn later_successful_noop_cannot_reown_global_finality() {
        let operation_id = [0xAD; 32];
        let request = top_up_request(operation_id);
        let (first_outer, second_outer) = outer_keys_in_reverse_state_key_order(operation_id);
        let first = top_up_carrier(&first_outer, request.clone(), 1);
        let second = top_up_carrier(&second_outer, request.clone(), 2);
        let first_authority = first.authority().clone();
        let second_authority = second.authority().clone();
        let entrypoints = vec![
            TransactionEntrypoint::External(first),
            TransactionEntrypoint::External(second),
        ];
        let results = vec![
            TransactionResult::new(Ok(DataTriggerSequence::default())),
            TransactionResult::new(Ok(DataTriggerSequence::default())),
        ];
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(&mut block, &mut batch, 0, &entrypoints)
            .expect("reserve distinct outer-authority attempts");
        stage_top_up_finality_claim(&mut block, &first_authority, &request, 0);

        finalize_kagemusha_operation_outcomes_v4(
            &mut block,
            batch,
            [KagemushaOperationResultSegmentV4::new(
                0,
                &entrypoints,
                &results,
            )],
        )
        .expect("later exact replay may succeed without another claim");

        let applied = kagemusha_operation_finality_v4(&block.world, operation_id)
            .expect("finality lookup")
            .expect("first fresh claim becomes final");
        assert_eq!(
            applied.outer_authority_digest,
            kagemusha_operation_authority_digest_v4(&first_authority)
                .expect("hash first outer authority")
        );
        assert_eq!(applied.phase_index, 0);
        assert!(
            kagemusha_operation_attempt_v4(&block.world, &second_authority, operation_id)
                .expect("later attempt lookup")
                .is_none(),
            "the later successful no-op must consume its pending attempt"
        );
    }

    #[test]
    fn matching_retry_replaces_rejected_attempt_and_can_apply() {
        let operation_id = [0xAE; 32];
        let request = top_up_request(operation_id);
        let outer_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let first = top_up_carrier(&outer_key, request.clone(), 1);
        let outer_authority = first.authority().clone();
        let first_entrypoints = vec![TransactionEntrypoint::External(first)];
        let first_results = vec![rejected_result("first attempt rejected")];
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut first_batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(
            &mut block,
            &mut first_batch,
            0,
            &first_entrypoints,
        )
        .expect("reserve first attempt");
        finalize_kagemusha_operation_outcomes_v4(
            &mut block,
            first_batch,
            [KagemushaOperationResultSegmentV4::new(
                0,
                &first_entrypoints,
                &first_results,
            )],
        )
        .expect("persist rejected first attempt");
        assert!(
            kagemusha_operation_finality_v4(&block.world, operation_id)
                .expect("finality lookup")
                .is_none(),
            "a rejected attempt must not claim economic finality"
        );

        let retry = top_up_carrier(&outer_key, request.clone(), 2);
        let retry_entrypoints = vec![TransactionEntrypoint::External(retry)];
        let retry_results = vec![TransactionResult::new(Ok(DataTriggerSequence::default()))];
        let mut retry_batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(
            &mut block,
            &mut retry_batch,
            1,
            &retry_entrypoints,
        )
        .expect("matching retry may replace the rejected attempt");
        stage_top_up_finality_claim(&mut block, &outer_authority, &request, 1);
        finalize_kagemusha_operation_outcomes_v4(
            &mut block,
            retry_batch,
            [KagemushaOperationResultSegmentV4::new(
                1,
                &retry_entrypoints,
                &retry_results,
            )],
        )
        .expect("matching fresh retry may apply");

        let applied = kagemusha_operation_finality_v4(&block.world, operation_id)
            .expect("finality lookup")
            .expect("retry becomes globally final");
        assert_eq!(
            applied.outer_authority_digest,
            kagemusha_operation_authority_digest_v4(&outer_authority)
                .expect("hash outer authority")
        );
        assert_eq!(applied.phase_index, 1);
        assert!(
            kagemusha_operation_attempt_v4(&block.world, &outer_authority, operation_id)
                .expect("attempt lookup")
                .is_none(),
            "successful retry consumes its rejected-attempt slot"
        );
    }

    #[test]
    fn conflicting_retry_preserves_rejected_attempt_and_rejects_only_its_transaction() {
        for execution_phase in [
            KagemushaOperationExecutionPhaseV4::Ordinary,
            KagemushaOperationExecutionPhaseV4::Merge,
        ] {
            let operation_id = [0xAF; 32];
            let outer_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let first_request = top_up_request(operation_id);
            let first = top_up_carrier(&outer_key, first_request, 1);
            let outer_authority = first.authority().clone();
            let first_entrypoints = vec![TransactionEntrypoint::External(first)];
            let first_results = vec![rejected_result("first attempt rejected")];
            let state = test_state();
            let header = BlockHeader::new(
                NonZeroU64::new(1).expect("non-zero height"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let mut first_batch = KagemushaOperationReservationBatchV4::new(1, execution_phase);
            reserve_kagemusha_operation_outcomes_v4(
                &mut block,
                &mut first_batch,
                0,
                &first_entrypoints,
            )
            .expect("reserve first request");
            finalize_kagemusha_operation_outcomes_v4(
                &mut block,
                first_batch,
                [KagemushaOperationResultSegmentV4::new(
                    0,
                    &first_entrypoints,
                    &first_results,
                )],
            )
            .expect("persist rejected first request");

            let attempt_key =
                kagemusha_operation_outcome_state_key_v4(&outer_authority, operation_id)
                    .expect("attempt key");
            let rejected_payload = block
                .world
                .smart_contract_state
                .get(&attempt_key)
                .expect("rejected attempt exists")
                .clone();

            let conflicting_request = top_up_request_with_note_commitment(operation_id, [0x70; 32]);
            let conflicting = top_up_carrier(&outer_key, conflicting_request, 2);
            let conflicting_entrypoints =
                vec![TransactionEntrypoint::External(conflicting.clone())];
            let conflicting_results = vec![rejected_result("operation id is already terminal")];
            let mut conflicting_batch =
                KagemushaOperationReservationBatchV4::new(1, execution_phase);
            reserve_kagemusha_operation_outcomes_v4(
                &mut block,
                &mut conflicting_batch,
                1,
                &conflicting_entrypoints,
            )
            .expect("a conflicting request must not invalidate its execution batch");
            assert!(
                conflicting_batch.reservations.is_empty(),
                "the conflicting request must not overwrite or own the rejected attempt"
            );
            assert_eq!(
                block.world.smart_contract_state.get(&attempt_key),
                Some(&rejected_payload),
                "reservation planning must preserve the original rejected evidence"
            );

            let mut state_transaction = block.transaction();
            state_transaction.bind_kagemusha_operation_execution_locator_v4(Some(
                KagemushaOperationExecutionLocatorV4::new(execution_phase, 1),
            ));
            let error =
                validate_kagemusha_operation_reservation_v4(&conflicting, &state_transaction)
                    .expect_err("the conflicting transaction must be rejected at execution");
            assert!(
                error.to_string().contains("already terminal"),
                "unexpected transaction-local rejection: {error}"
            );
            drop(state_transaction);

            finalize_kagemusha_operation_outcomes_v4(
                &mut block,
                conflicting_batch,
                [KagemushaOperationResultSegmentV4::new(
                    1,
                    &conflicting_entrypoints,
                    &conflicting_results,
                )],
            )
            .expect("an unreserved rejection must not invalidate finalization");
            assert_eq!(
                block.world.smart_contract_state.get(&attempt_key),
                Some(&rejected_payload),
                "batch finalization must preserve the original rejected evidence"
            );
        }
    }

    #[test]
    fn finalization_rejects_a_disappeared_reservation() {
        let operation_id = [0xAA; 32];
        let outer_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let transaction = top_up_carrier(&outer_key, top_up_request(operation_id), 1);
        let authority = transaction.authority().clone();
        let entrypoints = vec![TransactionEntrypoint::External(transaction)];
        let results = vec![TransactionResult::new(Ok(DataTriggerSequence::default()))];
        let state = test_state();
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut batch = KagemushaOperationReservationBatchV4::new(
            1,
            KagemushaOperationExecutionPhaseV4::Ordinary,
        );
        reserve_kagemusha_operation_outcomes_v4(&mut block, &mut batch, 0, &entrypoints)
            .expect("reserve carrier");
        let key = kagemusha_operation_outcome_state_key_v4(&authority, operation_id)
            .expect("outcome key");
        block.world.smart_contract_state.remove(key);

        assert!(matches!(
            finalize_kagemusha_operation_outcomes_v4(
                &mut block,
                batch,
                [KagemushaOperationResultSegmentV4::new(
                    0,
                    &entrypoints,
                    &results,
                )],
            ),
            Err(KagemushaOperationOutcomeErrorV4::MissingReservation)
        ));
    }
}
