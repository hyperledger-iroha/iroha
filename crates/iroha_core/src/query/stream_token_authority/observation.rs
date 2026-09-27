//! One-use role-11 Check consumption over exact signed execution and current native authority.
//!
//! The original Reserve/Complete history, challenged Check, custody and permission checks all
//! use one applied State view. A decoded row, observer signature or submission acknowledgement
//! cannot construct the verified capability returned here.

use std::{sync::Arc, time::Duration};

use iroha_crypto::HashOf;
use iroha_data_model::{
    account::AccountId,
    isi::sorafs::MutateSorafsStreamTokenAuthority,
    sorafs::{
        capacity::ProviderId,
        stream_token_authority::{
            STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1, StreamTokenAuthorityActionV1 as Action,
            StreamTokenAuthorityRequestV1, StreamTokenCheckPhaseV1 as Phase, StreamTokenCheckV1,
            StreamTokenFinalityFloorV1, StreamTokenOutcomeV1, StreamTokenReviewedV1,
            validate_stream_token_check_claim_v1,
        },
    },
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use sorafs_manifest::signer::{
    custody::{SignerCustodyAnchorV1, SignerCustodyBindingV1},
    custody_control::SignerCustodyControlStateV1,
    protocol::SignerPurposeBindingV1,
    receipt::{SignerCompletedOperationV1, SignerOperationFinalizedAnchorV1},
    stream_token::stream_token_binding_digest_v1,
};

use super::{
    OperationHeadV1, OperationRecordV1, authenticate_stream_token_history_to_floor_v1,
    eligibility::{authorized, check_live_custody, checked_phase},
    read_head, read_slot,
};
use crate::{
    query::{
        signer_check::{
            BoundNativeCheckV1, NativeCheckErrorV1, NativeCheckFloorV1, NativeCheckRoundV1,
            NativeCustodyCheckPurposeV1, NativeCustodyCheckRefV1, authenticate_applied_check_v1,
            bind_signed_check_v1, validate_native_signatory_v1,
        },
        stream_token_custody::{read_active, read_stream_token_custody_control_at_v1},
    },
    state::{State, StateReadOnly, StateView, WorldReadOnly},
};
use mv::storage::StorageReadOnly;

/// Current native inputs captured together before issuing a fresh role-11 Check.
/// This is preparation data; only consuming the signed Check creates a verified capability.
pub struct StreamTokenAuthoritySnapshotV1 {
    /// Exact current custody control state.
    pub control: SignerCustodyControlStateV1,
    /// Current custody journal revision.
    pub control_revision: u64,
    /// Exact custody commitment at the captured finalized block.
    pub anchor: SignerCustodyAnchorV1,
    /// Registered provider owner, derived from this State view.
    pub operator: AccountId,
    /// Current per-provider audit and reservation head.
    pub head: OperationHeadV1,
    /// Requested immutable operation, when already admitted.
    pub operation: Option<OperationRecordV1>,
    /// Durable authenticated block and committee context preceding the Check.
    pub floor: StreamTokenFinalityFloorV1,
}

/// Capture bounded native custody and operation inputs from one finalized State view.
///
/// # Errors
/// Rejects foreign bindings, corrupt native indexes, unregistered providers or missing finality.
pub fn capture_stream_token_authority_v1(
    view: &StateView<'_>,
    binding: &SignerCustodyBindingV1,
    operation_id: [u8; 32],
) -> Result<StreamTokenAuthoritySnapshotV1, Error> {
    let SignerPurposeBindingV1::StreamToken { provider_id } = binding.purpose else {
        return Err(Error::Invalid);
    };
    let provider = ProviderId::new(provider_id);
    let height = u64::try_from(view.block_hashes().len()).map_err(|_| Error::Finality)?;
    let custody = read_stream_token_custody_control_at_v1(view, binding, height)
        .map_err(|_| Error::Authority)?
        .ok_or(Error::Authority)?;
    let finality = crate::query::signer_finality::verify_signer_finality_v1(
        view,
        height,
        custody.anchor.block_hash,
    )
    .map_err(|_| Error::Finality)?;
    let control = read_active(view.world(), provider)
        .map_err(|_| Error::Authority)?
        .ok_or(Error::Authority)?;
    if control.state != custody.state || control.index.digest != custody.anchor.state_digest {
        return Err(Error::Authority);
    }
    let operator = view
        .world()
        .provider_owners()
        .get(&provider)
        .cloned()
        .ok_or(Error::Authority)?;
    let head = read_head(view.world(), provider).map_err(|_| Error::Authority)?;
    let operation =
        read_slot(view.world(), provider, operation_id).map_err(|_| Error::Authority)?;
    Ok(StreamTokenAuthoritySnapshotV1 {
        control: custody.state,
        control_revision: control.index.revision,
        anchor: custody.anchor,
        operator,
        head,
        operation,
        floor: StreamTokenFinalityFloorV1 {
            height,
            block_hash: custody.anchor.block_hash,
            context_id: finality.context_id(),
        },
    })
}

/// Independent purpose, accounts, custody, operation and committee pins for one native Check.
/// These are preparation inputs, never decoded verification output.
pub struct StreamTokenCheckExpectedV1 {
    /// Exact provider-scoped key and policy binding.
    pub binding: SignerCustodyBindingV1,
    /// Registered single-signature observer with the provider Check permission.
    pub observer: AccountId,
    /// Registered provider owner with the provider Operate permission.
    pub expected_operator: AccountId,
    /// Exact native custody control revision retained before the attempt.
    pub control_revision: u64,
    /// Exact native custody control digest retained before the attempt.
    pub control_digest: [u8; 32],
    /// Original prepared body, custody and audit predecessor.
    pub reviewed: StreamTokenReviewedV1,
    /// Independently requested phase and original native operation.
    pub phase: Phase,
    /// Independently retained finalized height, block and committee context.
    pub floor: StreamTokenFinalityFloorV1,
}

/// Closed UTC uncertainty interval sampled after the native execution proof is consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamTokenEligibilityTimeIntervalV1 {
    /// Earliest possible current Unix time in milliseconds, strictly positive.
    pub earliest_unix_ms: u64,
    /// Latest possible current Unix time, below `u64::MAX` and not before the earliest time.
    pub latest_unix_ms: u64,
}

/// Payload-free failure of a consumed native stream-token Check attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StreamTokenObservationErrorV1 {
    /// Independent expectations, bounds or purpose are invalid.
    #[error("invalid stream-token Check expectations")]
    Invalid,
    /// The original monotonic observation interval expired.
    #[error("stream-token Check observation expired")]
    Expired,
    /// Operating-system challenge entropy was unavailable.
    #[error("stream-token Check entropy unavailable")]
    Entropy,
    /// The signed transaction differs from the exact prepared native Check.
    #[error("stream-token Check signed transaction mismatch")]
    Transaction,
    /// The exact signed Check has not applied to the retained State.
    #[error("stream-token Check is not applied")]
    NotApplied,
    /// Durable finality or committee continuity does not match the independent floor.
    #[error("stream-token Check finality unavailable")]
    Finality,
    /// Original native operation or challenged Check execution was not authenticated.
    #[error("stream-token Check execution proof rejected")]
    Execution,
    /// Current same-cut custody, permission or phase eligibility has changed.
    #[error("stream-token Check current authority rejected")]
    Authority,
    /// The independently sampled UTC interval is unavailable or malformed.
    #[error("stream-token Check eligibility clock unavailable")]
    Clock,
}
use StreamTokenObservationErrorV1 as Error;

impl From<NativeCheckErrorV1> for Error {
    fn from(error: NativeCheckErrorV1) -> Self {
        match error {
            NativeCheckErrorV1::Invalid => Self::Invalid,
            NativeCheckErrorV1::Expired => Self::Expired,
            NativeCheckErrorV1::Entropy => Self::Entropy,
            NativeCheckErrorV1::Transaction => Self::Transaction,
            NativeCheckErrorV1::NotApplied => Self::NotApplied,
            NativeCheckErrorV1::Finality => Self::Finality,
            NativeCheckErrorV1::Execution => Self::Execution,
        }
    }
}

fn native_floor(floor: StreamTokenFinalityFloorV1) -> NativeCheckFloorV1 {
    NativeCheckFloorV1 {
        height: floor.height,
        block_hash: floor.block_hash,
        context_id: floor.context_id,
    }
}

/// Move-only native challenge prepared before signing or submitting an observer transaction.
#[must_use = "sign the exact prepared Check within its original interval"]
pub struct PreparedStreamTokenCheckV1 {
    state: Arc<State>,
    expected: StreamTokenCheckExpectedV1,
    instruction: MutateSorafsStreamTokenAuthority,
    round: NativeCheckRoundV1,
}

/// Move-only signed Check with one unchanged observation deadline.
#[must_use = "submit the exact signed Check and consume its finalized result once"]
pub struct PendingStreamTokenCheckV1 {
    prepared: PreparedStreamTokenCheckV1,
    bound: BoundNativeCheckV1,
}

/// Native authority captured at the same applied cut as the verified Check and operation history.
/// It has no decoded constructor and is exposed only through a verified native Check.
pub struct StreamTokenCheckSnapshotV1 {
    control: SignerCustodyControlStateV1,
    anchor: SignerCustodyAnchorV1,
    head: OperationHeadV1,
    operation: Option<OperationRecordV1>,
    completed: Option<SignerCompletedOperationV1>,
}
impl StreamTokenCheckSnapshotV1 {
    /// Exact current native custody control.
    #[must_use]
    pub const fn control(&self) -> &SignerCustodyControlStateV1 {
        &self.control
    }
    /// Current applied block and custody digest.
    #[must_use]
    pub const fn anchor(&self) -> SignerCustodyAnchorV1 {
        self.anchor
    }
    /// Exact current operation journal head.
    #[must_use]
    pub const fn head(&self) -> &OperationHeadV1 {
        &self.head
    }
    /// Original operation's current immutable row; absent only for a Current phase.
    #[must_use]
    pub const fn operation(&self) -> Option<&OperationRecordV1> {
        self.operation.as_ref()
    }

    /// Native completion joined to its original execution and immutable journal commitment.
    #[must_use]
    pub const fn completed_operation(&self) -> Option<&SignerCompletedOperationV1> {
        self.completed.as_ref()
    }
}

/// Move-only verified native Check scoped to its original phase and monotonic lifetime.
/// This capability authenticates ledger authority; private receipt verification remains required.
#[must_use = "use the verified Check only for its original live phase"]
pub struct VerifiedStreamTokenCheckV1 {
    instruction: MutateSorafsStreamTokenAuthority,
    snapshot: StreamTokenCheckSnapshotV1,
    applied_floor: StreamTokenFinalityFloorV1,
    entry_hash: HashOf<TransactionEntrypoint>,
    canonical_external: Vec<u8>,
    check_block_hash: [u8; 32],
    time: StreamTokenEligibilityTimeIntervalV1,
    round: NativeCheckRoundV1,
}

fn validate_expected(expected: &StreamTokenCheckExpectedV1) -> Result<ProviderId, Error> {
    native_floor(expected.floor).validate()?;
    let SignerPurposeBindingV1::StreamToken { provider_id } = expected.binding.purpose else {
        return Err(Error::Invalid);
    };
    let reviewed = &expected.reviewed.request;
    if provider_id == [0; 32]
        || expected.control_revision == 0
        || expected.control_digest == [0; 32]
        || expected.observer == expected.expected_operator
        || validate_native_signatory_v1(&expected.observer).is_err()
        || validate_native_signatory_v1(&expected.expected_operator).is_err()
        || expected.observer == AccountId::new(expected.binding.public_key.clone())
        || reviewed.original_custody.control_state_digest != expected.control_digest
        || reviewed.binding_digest
            != stream_token_binding_digest_v1(&expected.binding).map_err(|_| Error::Invalid)?
    {
        return Err(Error::Invalid);
    }
    Ok(ProviderId::new(provider_id))
}

/// Prepare a purpose-native observation with fresh OS entropy and an unchanged local deadline.
///
/// # Errors
/// Rejects malformed or oversized expectations, non-independent accounts, an invalid duration,
/// unavailable challenge entropy, or expiry before returning the exact unsigned Check.
pub fn begin_stream_token_check_v1(
    state: Arc<State>,
    expected: StreamTokenCheckExpectedV1,
    max_elapsed: Duration,
) -> Result<PreparedStreamTokenCheckV1, Error> {
    let mut round = NativeCheckRoundV1::start(max_elapsed)?;
    let provider = validate_expected(&expected)?;
    let mut instruction = MutateSorafsStreamTokenAuthority {
        request: StreamTokenAuthorityRequestV1 {
            network_id: expected.binding.network_id,
            provider_id: provider,
            expected_control_revision: expected.control_revision,
            expected_control_digest: expected.control_digest,
            action: Action::Check(StreamTokenCheckV1 {
                challenge: [1; 32],
                expected_operator: expected.expected_operator.clone(),
                expected_observer: expected.observer.clone(),
                floor: expected.floor,
                reviewed: expected.reviewed,
                phase: expected.phase.clone(),
            }),
        },
    };
    validate_stream_token_check_claim_v1(
        &instruction.request,
        expected.binding.network_id,
        provider,
        expected.control_revision,
        expected.control_digest,
        &expected.expected_operator,
        &expected.observer,
        [1; 32],
        expected.floor,
        &expected.reviewed,
        &expected.phase,
    )
    .map_err(|_| Error::Invalid)?;
    if norito::canonical_frame_len(&instruction.request).map_err(|_| Error::Invalid)?
        > STREAM_TOKEN_AUTHORITY_REQUEST_MAX_BYTES_V1
    {
        return Err(Error::Invalid);
    }
    let Action::Check(check) = &mut instruction.request.action else {
        return Err(Error::Invalid);
    };
    check.challenge = round.issue_challenge()?;
    Ok(PreparedStreamTokenCheckV1 {
        state,
        expected,
        instruction,
        round,
    })
}

impl PreparedStreamTokenCheckV1 {
    /// Exact instruction, including its already-fixed fresh challenge.
    #[must_use]
    pub const fn instruction(&self) -> &MutateSorafsStreamTokenAuthority {
        &self.instruction
    }

    /// Check the original lifetime without renewing it.
    ///
    /// # Errors
    /// Returns expiry once the original monotonic deadline is reached.
    pub fn ensure_live(&self) -> Result<(), Error> {
        self.round.ensure_live().map_err(Into::into)
    }

    /// Bind one exact directly signed observer transaction and consume this prepared attempt.
    ///
    /// # Errors
    /// Rejects signature, authority, instruction, network, size or deadline substitution.
    pub fn bind_signed_transaction(
        mut self,
        signed: SignedTransaction,
    ) -> Result<PendingStreamTokenCheckV1, Error> {
        let bound = bind_signed_check_v1(
            &mut self.round,
            NativeCustodyCheckRefV1::StreamToken(&self.instruction),
            &self.expected.binding.chain_id,
            self.expected.binding.network_id,
            &self.expected.observer,
            native_floor(self.expected.floor),
            signed,
        )?;
        Ok(PendingStreamTokenCheckV1 {
            prepared: self,
            bound,
        })
    }
}

fn snapshot_at(
    view: &impl StateReadOnly,
    expected: &StreamTokenCheckExpectedV1,
    instruction: &MutateSorafsStreamTokenAuthority,
    now: u64,
) -> Result<StreamTokenCheckSnapshotV1, Error> {
    let provider = instruction.request.provider_id;
    let height = u64::try_from(view.block_hashes().len()).map_err(|_| Error::Authority)?;
    let control = read_active(view.world(), provider)
        .map_err(|_| Error::Authority)?
        .ok_or(Error::Authority)?;
    let snapshot = read_stream_token_custody_control_at_v1(view, &expected.binding, height)
        .map_err(|_| Error::Authority)?
        .ok_or(Error::Authority)?;
    if control.index.revision != expected.control_revision
        || control.index.digest != expected.control_digest
        || snapshot.anchor.state_digest != control.index.digest
        || snapshot.state != control.state
        || !authorized(
            view,
            &expected.observer,
            provider,
            &instruction.request.action,
        )
    {
        return Err(Error::Authority);
    }
    let Action::Check(check) = &instruction.request.action else {
        return Err(Error::Invalid);
    };
    check_live_custody(&control, check, &expected.observer, now).map_err(|_| Error::Authority)?;
    let head = read_head(view.world(), provider).map_err(|_| Error::Authority)?;
    let (reviewed, phase) =
        checked_phase(view, provider, check, &head, now).map_err(|_| Error::Authority)?;
    if reviewed != expected.reviewed || phase != expected.phase {
        return Err(Error::Authority);
    }
    let operation = read_slot(view.world(), provider, reviewed.request.operation_id)
        .map_err(|_| Error::Authority)?;
    let completed = match operation.as_ref() {
        Some(record) => completed_at(view, record)?,
        None => None,
    };
    Ok(StreamTokenCheckSnapshotV1 {
        control: snapshot.state,
        anchor: snapshot.anchor,
        head,
        operation,
        completed,
    })
}

fn completed_at(
    view: &impl StateReadOnly,
    record: &OperationRecordV1,
) -> Result<Option<SignerCompletedOperationV1>, Error> {
    let row = &record.operation;
    let StreamTokenOutcomeV1::Completed(complete) = row.operation.outcome else {
        return Ok(None);
    };
    let terminal = row.terminal_execution.as_ref().ok_or(Error::Execution)?;
    let offset = usize::try_from(terminal.height.checked_sub(1).ok_or(Error::Execution)?)
        .map_err(|_| Error::Execution)?;
    let block_hash = view.block_hashes().get(offset).ok_or(Error::Finality)?;
    Ok(Some(SignerCompletedOperationV1 {
        operation_id: complete.reviewed.request.operation_id,
        intent_digest: complete
            .reviewed
            .intent
            .digest()
            .map_err(|_| Error::Execution)?,
        original_custody: complete.reviewed.request.original_custody,
        reservation: complete.reservation,
        commitment: complete.commitment,
        signatures_digest: complete.signatures_digest,
        completed_at_unix_ms: terminal.recorded_at_unix_ms,
        anchor: SignerOperationFinalizedAnchorV1 {
            height: terminal.height,
            block_hash: *block_hash.as_ref(),
            operation_state_digest: super::record_digest(record).map_err(|_| Error::Execution)?,
        },
    }))
}

impl PendingStreamTokenCheckV1 {
    /// Exact signed transaction to submit or reconcile without constructing another Check.
    #[must_use]
    pub fn signed_transaction(&self) -> &SignedTransaction {
        self.bound.signed_transaction()
    }

    /// Authenticate Check execution, original operation history and current same-cut authority.
    ///
    /// Both UTC endpoints are sampled only after the bounded historical proof; neither sampling
    /// nor a later phase resets the original monotonic lifetime. Complete phases prove the actual
    /// signed Reserve and Complete entries and successful outputs from this node's Kura history.
    ///
    /// # Errors
    /// Consumes the pending attempt on any unavailable application/finality/history, current
    /// authority change, malformed time interval or expiration.
    pub fn verify_finalized(
        self,
        sample_time: impl FnOnce() -> Result<StreamTokenEligibilityTimeIntervalV1, Error>,
    ) -> Result<VerifiedStreamTokenCheckV1, Error> {
        let prepared = self.prepared;
        let cut = authenticate_applied_check_v1(
            &prepared.state,
            NativeCustodyCheckPurposeV1::StreamToken,
            self.bound,
            &prepared.round,
        )?;
        let view = cut.view();
        if !matches!(prepared.expected.phase, Phase::Current(_)) {
            let history = authenticate_stream_token_history_to_floor_v1(
                view,
                prepared.instruction.request.provider_id,
                prepared.expected.reviewed.request.operation_id,
                prepared.expected.floor,
            )
            .map_err(|_| Error::Execution)?;
            let claimed = match &prepared.expected.phase {
                Phase::BeforeProvider(row)
                | Phase::AfterProvider(row)
                | Phase::BeforeCommit(row)
                | Phase::AfterCommit(row)
                | Phase::BeforeRelease(row) => row,
                Phase::Current(_) => return Err(Error::Invalid),
            };
            if &history.current().operation != claimed
                || history.reserved().operation.operation.reviewed != prepared.expected.reviewed
            {
                return Err(Error::Execution);
            }
        }
        prepared.round.ensure_live()?;
        let time = sample_time().map_err(|_| Error::Clock)?;
        if time.earliest_unix_ms == 0
            || time.latest_unix_ms < time.earliest_unix_ms
            || time.latest_unix_ms == u64::MAX
        {
            return Err(Error::Clock);
        }
        let snapshot = snapshot_at(
            view,
            &prepared.expected,
            &prepared.instruction,
            time.earliest_unix_ms,
        )?;
        snapshot_at(
            view,
            &prepared.expected,
            &prepared.instruction,
            time.latest_unix_ms,
        )?;
        prepared.round.ensure_live()?;
        let floor = cut.applied_floor();
        let applied_floor = StreamTokenFinalityFloorV1 {
            height: floor.height,
            block_hash: floor.block_hash,
            context_id: floor.context_id,
        };
        let entry_hash = cut.entry_hash();
        let (canonical_external, check_block_hash) = cut.into_verified_entry();
        Ok(VerifiedStreamTokenCheckV1 {
            instruction: prepared.instruction,
            snapshot,
            applied_floor,
            entry_hash,
            canonical_external,
            check_block_hash,
            time,
            round: prepared.round,
        })
    }
}

impl VerifiedStreamTokenCheckV1 {
    /// Check that this capability still belongs to its original monotonic observation lifetime.
    ///
    /// # Errors
    /// Returns expiry without renewing the challenge or resampling its observation time.
    pub fn ensure_live(&self) -> Result<(), Error> {
        self.round.ensure_live().map_err(Into::into)
    }
    /// Exact authenticated native Check and phase.
    #[must_use]
    pub const fn instruction(&self) -> &MutateSorafsStreamTokenAuthority {
        &self.instruction
    }
    /// Native custody, journal and original operation at the authenticated applied cut.
    #[must_use]
    pub const fn snapshot(&self) -> &StreamTokenCheckSnapshotV1 {
        &self.snapshot
    }
    /// Exact final applied block and committee context.
    #[must_use]
    pub const fn applied_floor(&self) -> StreamTokenFinalityFloorV1 {
        self.applied_floor
    }
    /// Hash of the exact signed Check proved to have executed successfully.
    #[must_use]
    pub const fn entry_hash(&self) -> HashOf<TransactionEntrypoint> {
        self.entry_hash
    }
    /// Complete canonical External entry authenticated by the successful result proof.
    #[must_use]
    pub fn canonical_external(&self) -> &[u8] {
        &self.canonical_external
    }
    /// Exact finalized block containing the successful Check.
    #[must_use]
    pub const fn check_block_hash(&self) -> [u8; 32] {
        self.check_block_hash
    }
    /// Original earliest/latest UTC endpoints; their age cannot be refreshed by a caller.
    #[must_use]
    pub const fn time_interval(&self) -> StreamTokenEligibilityTimeIntervalV1 {
        self.time
    }
}

#[cfg(test)]
mod tests;
