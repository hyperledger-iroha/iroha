//! Production boundary for the executable Sumeragi v2 reducer.
//!
//! The reducer crate intentionally has no codec, cryptography, filesystem, or
//! networking dependencies.  This module is the narrow adapter which binds it
//! to the canonical data-model wire types and the crash-safe safety WAL.  WAL
//! effects are handled synchronously: a complete frame is encoded, appended,
//! flushed, and synchronised, and only then is the exact persistence identifier
//! acknowledged to the reducer.  Consequently a caller can never observe a
//! signing, broadcast, view-change, or apply effect which was causally ordered
//! after an unacknowledged safety write.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use super::v2_core as reducer;
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};
use norito::codec::{Decode, Encode};
use thiserror::Error;

use super::{
    safety_wal::{SafetyWal, SafetyWalError},
    serviced_candidate_store::{
        LeaderWireRecoveryAuthority, ProducerContinuationAddress, ProducerContinuationHandoffToken,
        ProducerContinuationIdentity, ProducerContinuationRecord, ProducerContinuationReservation,
        ProducerContinuationSourceClass, ProducerContinuationStatus,
        ProducerContinuationTerminalToken, SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
        ServicedCandidateKey, ServicedCandidateStore, serviced_candidate_stage_for_kind_code,
    },
    v2_body_store::{DurableBodyReceipt, ValidatedBodyReceipt},
};

// The wire admission limit and dependency-free reducer bound are one protocol
// constant. A drift would admit a context that the verified state machine
// cannot represent, so make it a compile-time error rather than an adapter
// runtime surprise.
const _: [(); wire::MAX_VALIDATORS_PER_HEIGHT] = [(); reducer::MAX_VOTING_ROSTER_LEN];
use crate::kura::KuraV2CommitReceipt;

const AGGREGATE_TOKEN_PREFIX: &[u8] = b"sumeragi-v2:verified-aggregate\0";
const MAX_DEFERRED_INPUTS: usize = 1024;
const MAX_DEFERRED_PROGRESS_INPUTS: usize = wire::MAX_VALIDATORS_PER_HEIGHT * 2 + 3;
const MAX_INGRESS_SEMANTIC_KEYS: usize = 1024;
// Scheduler priority is physical ownership evidence, not part of the logical
// reducer occurrence. Keep the legacy key field at one canonical value so
// existing snapshot layout remains unchanged while Normal/Progress rerouting
// coalesces to the same service identity.
const ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS: u8 = u8::MAX;

/// Maximum adapter effects returned by one serialized runtime invocation.
///
/// A reducer transition without persistence already has this exact source
/// bound. Persistence is acknowledged synchronously, but the record-specific
/// budgets below prove that replacing the sole `Persist` effect with its
/// causal `Persisted` continuation produces at most five effects. Keeping the
/// adapter bound equal to the reducer bound therefore matches the executor's
/// retained-batch contract without inflating either queue.
const MAX_ADAPTER_EFFECTS_PER_MACRO_STEP: usize = reducer::MAX_EFFECTS_PER_STEP;

/// Largest record-specific `Persist -> Persisted` flattened batch.
///
/// The witness is locally formed `InstallTimeout`: one local TimeoutVote
/// broadcast precedes `Persist`, then the acknowledgement can emit the TC
/// broadcast, `EnterView`, one protected-body fetch, and one reconstructed
/// locked Commit signature. Thus `2 - 1 + 4 = 5`.
const MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP: usize = 5;

// Every persistence-flattened batch must fit the executor's already verified
// source-transition capacity. A future record shape which breaks this
// relation fails at compile time as well as at the runtime checks below.
const _: () =
    assert!(MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);

/// WAL-record class used to select the exact adapter macro-step budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PersistenceMacroStepClass {
    /// Locally validated proposal intent.
    ProposalIntent,
    /// Local Prepare-vote intent.
    PrepareIntent,
    /// Newly observed highest Prepare certificate.
    ObservePrepare,
    /// Atomic lock and local Commit-vote intent.
    LockAndCommit,
    /// Local timeout-vote intent.
    TimeoutIntent,
    /// Installed timeout certificate.
    InstallTimeout,
    /// Durable Commit certificate decision.
    Decision,
}

impl PersistenceMacroStepClass {
    /// Classify every safety-WAL record; a new record cannot silently inherit a
    /// budget because the exhaustive match must be deliberately extended.
    fn from_record(record: &reducer::WalRecord) -> Self {
        match record {
            reducer::WalRecord::ProposalIntent(_) => Self::ProposalIntent,
            reducer::WalRecord::PrepareIntent(_) => Self::PrepareIntent,
            reducer::WalRecord::ObservePrepare(_) => Self::ObservePrepare,
            reducer::WalRecord::LockAndCommit { .. } => Self::LockAndCommit,
            reducer::WalRecord::TimeoutIntent(_) => Self::TimeoutIntent,
            reducer::WalRecord::InstallTimeout(_) => Self::InstallTimeout,
            reducer::WalRecord::Decision(_) => Self::Decision,
        }
    }

    /// Return the exact reviewed upper bounds for the source transition and
    /// its persistence acknowledgement continuation.
    fn budget(self) -> PersistenceMacroStepBudget {
        match self {
            // LocalProposalReady emits only Persist; Persisted emits Sign.
            Self::ProposalIntent => PersistenceMacroStepBudget::new(1, 1),
            // Signed Proposal may prefix the PrepareIntent Persist with its
            // Proposal broadcast; Persisted emits one Prepare Sign.
            Self::PrepareIntent => PersistenceMacroStepBudget::new(2, 1),
            // Signed Prepare can prefix ObservePrepare with the vote and QC
            // broadcasts plus one fetch. Its None continuation can emit at
            // most one already queued, still-authorized signature.
            Self::ObservePrepare => PersistenceMacroStepBudget::new(4, 1),
            // Signed Prepare can prefix LockAndCommit with vote and QC
            // broadcasts; Persisted emits one Commit Sign.
            Self::LockAndCommit => PersistenceMacroStepBudget::new(3, 1),
            // TimeoutElapsed emits only Persist; Persisted emits Sign.
            Self::TimeoutIntent => PersistenceMacroStepBudget::new(1, 1),
            // Signed TimeoutVote can prefix Persist with its vote broadcast;
            // Persisted can emit TC broadcast, EnterView, fetch, and Sign.
            Self::InstallTimeout => PersistenceMacroStepBudget::new(2, 4),
            // Signed CommitVote can prefix Persist with its vote broadcast;
            // Persisted can emit the CommitQC broadcast and one body/apply
            // stage. Decision invalidates every queued pre-decision signer.
            Self::Decision => PersistenceMacroStepBudget::new(2, 2),
        }
    }

    /// Canonical class inventory for exhaustive bound tests.
    #[cfg(test)]
    const ALL: [Self; 7] = [
        Self::ProposalIntent,
        Self::PrepareIntent,
        Self::ObservePrepare,
        Self::LockAndCommit,
        Self::TimeoutIntent,
        Self::InstallTimeout,
        Self::Decision,
    ];
}

/// Reviewed source/continuation lengths for one WAL-record class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PersistenceMacroStepBudget {
    /// Maximum effects in the reducer transition containing `Persist`.
    initial_effects: usize,
    /// Maximum effects emitted by the matching `Persisted` transition.
    continuation_effects: usize,
}

impl PersistenceMacroStepBudget {
    /// Construct one compile-time record-specific budget.
    const fn new(initial_effects: usize, continuation_effects: usize) -> Self {
        Self {
            initial_effects,
            continuation_effects,
        }
    }

    /// Maximum returned effects after replacing the sole `Persist` effect with
    /// the acknowledgement continuation.
    const fn flattened_effects(self) -> usize {
        self.initial_effects - 1 + self.continuation_effects
    }
}

/// Node-local fingerprints exported through the compact v2 status record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AdapterFingerprints {
    /// Hash of the node's consensus identity.
    pub node: Hash,
    /// Hash identifying the running build.
    pub build: Hash,
    /// Hash of all consensus-relevant configuration.
    pub config: Hash,
}

/// Read-only reducer facts needed by the bounded local proposal assembler.
///
/// The reducer remains the sole owner of lock and view state. Candidate code
/// receives only this snapshot and cannot mutate safety state or manufacture a
/// proposal justification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocalProposalDirective {
    tag: reducer::EventTag,
    leader: wire::ValidatorIndex,
    locked_round: Option<wire::ConsensusRound>,
    locked_subject: Option<wire::BlockSubject>,
    decided_subject: Option<wire::BlockSubject>,
}

impl LocalProposalDirective {
    /// Build an exact directive fixture without exposing reducer-owned fields
    /// in production builds.
    #[cfg(test)]
    pub(crate) const fn for_test(
        tag: reducer::EventTag,
        leader: wire::ValidatorIndex,
        locked_round: Option<wire::ConsensusRound>,
        locked_subject: Option<wire::BlockSubject>,
        decided_subject: Option<wire::BlockSubject>,
    ) -> Self {
        Self {
            tag,
            leader,
            locked_round,
            locked_subject,
            decided_subject,
        }
    }

    /// Exact height/view/generation which owns candidate work.
    pub(crate) const fn tag(self) -> reducer::EventTag {
        self.tag
    }

    /// Frozen-roster validator expected to propose in this view.
    pub(crate) const fn leader(self) -> wire::ValidatorIndex {
        self.leader
    }

    /// Subject whose exact immutable body must remain recoverable while locked.
    pub(crate) const fn locked_subject(self) -> Option<wire::BlockSubject> {
        self.locked_subject
    }

    /// Exact round/subject pair protected by the active durable lock.
    pub(crate) fn locked_body(self) -> Option<(wire::ConsensusRound, wire::BlockSubject)> {
        self.locked_round.zip(self.locked_subject)
    }

    /// Subject already decided at this height, if application is pending.
    pub(crate) const fn decided_subject(self) -> Option<wire::BlockSubject> {
        self.decided_subject
    }
}

/// Structurally and cryptographically verified immutable context for one
/// height.
///
/// The constructor verifies every roster proof of possession up front. A
/// non-genesis context additionally requires the exact durable parent
/// artifact and verifies both its CommitQC and the parent QC carried by the
/// new context under the previous frozen roster.
#[derive(Clone)]
pub(crate) struct VerifiedHeightContext {
    context: wire::HeightContext,
    proofs_of_possession: Vec<Vec<u8>>,
    parent_verification: Option<ParentVerificationContext>,
}

/// Frozen parent-roster material retained solely to authenticate the
/// parent CommitQC carried by a view-zero proposal.
#[derive(Clone)]
struct ParentVerificationContext {
    context: wire::HeightContext,
    proofs_of_possession: Vec<Vec<u8>>,
}

impl VerifiedHeightContext {
    /// Verify a genesis height context against its configured BLS roster.
    pub(crate) fn genesis(
        context: wire::HeightContext,
        proofs_of_possession: Vec<Vec<u8>>,
    ) -> Result<Self, AdapterError> {
        context.validate()?;
        if context.height != 1
            || context.parent_commit_qc.is_some()
            || context.snapshot_bootstrap.is_some()
        {
            return Err(AdapterError::InvalidGenesisContext);
        }
        verify_roster_proofs(&context, &proofs_of_possession)?;
        verify_next_epoch_snapshot_proofs(&context)?;
        Ok(Self {
            context,
            proofs_of_possession,
            parent_verification: None,
        })
    }

    /// Verify the complete first context authenticated by an audited snapshot payload.
    pub(crate) fn snapshot_bootstrap(
        record: &wire::SnapshotV2BootstrapRecord,
    ) -> Result<Self, AdapterError> {
        record.validate()?;
        if record.context.height <= 1
            || record.context.parent_commit_qc.is_some()
            || record.context.snapshot_bootstrap.is_none()
        {
            return Err(AdapterError::InvalidSnapshotBootstrapContext);
        }
        verify_roster_proofs(&record.context, &record.validator_set_pops)?;
        verify_next_epoch_snapshot_proofs(&record.context)?;
        Ok(Self {
            context: record.context.clone(),
            proofs_of_possession: record.validator_set_pops.clone(),
            parent_verification: None,
        })
    }

    /// Verify a successor context from a durable parent artifact.
    pub(crate) fn successor(
        context: wire::HeightContext,
        proofs_of_possession: Vec<Vec<u8>>,
        parent_artifact: &wire::finality::V2FinalityArtifact,
        parent_receipt: &KuraV2CommitReceipt,
        parent_proofs_of_possession: &[Vec<u8>],
    ) -> Result<Self, AdapterError> {
        context.validate()?;
        parent_artifact.validate()?;
        verify_next_epoch_snapshot_proofs(&context)?;
        if context.snapshot_bootstrap.is_some() {
            return Err(AdapterError::ParentContextMismatch);
        }
        if parent_artifact.validator_set_pops != parent_proofs_of_possession {
            return Err(AdapterError::ParentContextMismatch);
        }
        verify_roster_proofs(&parent_artifact.height_context, parent_proofs_of_possession)?;
        verify_quorum_certificate(
            &parent_artifact.height_context,
            &parent_artifact.commit_qc,
            parent_proofs_of_possession,
        )?;

        let parent_qc = context
            .parent_commit_qc
            .as_ref()
            .ok_or(AdapterError::ParentContextMismatch)?;
        let expected_height = parent_artifact
            .height
            .checked_add(1)
            .ok_or(AdapterError::ParentContextMismatch)?;
        if context.height != expected_height
            || context.chain_id != parent_artifact.height_context.chain_id
            || context.mode != parent_artifact.height_context.mode
            || context.da_layout != parent_artifact.height_context.da_layout
            || context.execution_policy_hash != parent_artifact.height_context.execution_policy_hash
            || parent_qc.subject != parent_artifact.subject
            || parent_receipt.height() != parent_artifact.height
            || parent_receipt.context_id() != parent_artifact.context_id()
            || parent_receipt.block_hash() != parent_artifact.block_hash
            || parent_receipt.subject() != parent_artifact.subject
            || parent_receipt.certificate() != parent_artifact.commit_qc.as_ref()
            || parent_receipt.artifact_hash() != HashOf::new(parent_artifact)
        {
            return Err(AdapterError::ParentContextMismatch);
        }
        if let Some(snapshot) = &parent_artifact.height_context.next_epoch_snapshot {
            if context.epoch != snapshot.epoch
                || context.epoch_end_height != snapshot.epoch_end_height
                || context.mode != snapshot.mode
                || context.roster != snapshot.roster
                || context.quorum != snapshot.quorum
                || context.leader_seed != snapshot.leader_seed
                || proofs_of_possession.as_slice() != snapshot.validator_set_pops.as_slice()
            {
                return Err(AdapterError::EpochTransitionMismatch);
            }
        } else if context.epoch != parent_artifact.height_context.epoch
            || context.epoch_end_height != parent_artifact.height_context.epoch_end_height
            || context.roster != parent_artifact.height_context.roster
            || context.quorum != parent_artifact.height_context.quorum
            || context.leader_seed != parent_artifact.height_context.leader_seed
            || proofs_of_possession.as_slice() != parent_artifact.validator_set_pops.as_slice()
        {
            return Err(AdapterError::EpochTransitionMismatch);
        }
        verify_quorum_certificate(
            &parent_artifact.height_context,
            parent_qc,
            parent_proofs_of_possession,
        )?;
        verify_roster_proofs(&context, &proofs_of_possession)?;
        Ok(Self {
            context,
            proofs_of_possession,
            parent_verification: Some(ParentVerificationContext {
                context: parent_artifact.height_context.clone(),
                proofs_of_possession: parent_proofs_of_possession.to_vec(),
            }),
        })
    }

    /// Borrow the exact frozen wire context.
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        &self.context
    }

    /// Borrow proofs of possession in the exact frozen-roster order.
    pub(crate) fn proofs_of_possession(&self) -> &[Vec<u8>] {
        &self.proofs_of_possession
    }
}

/// A canonical message whose safety intent is already durable and may be signed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SignRequest {
    /// Leader proposal with an empty signature field.
    Proposal(wire::Proposal),
    /// Prepare or Commit vote with an empty signature field.
    Vote(wire::Vote),
    /// Timeout vote with an empty signature field.
    TimeoutVote(wire::TimeoutVote),
}

impl SignRequest {
    /// Return the canonical bytes authorized by this durable signing request.
    pub(crate) fn signature_preimage(&self) -> Vec<u8> {
        match self {
            Self::Proposal(proposal) => proposal.signature_preimage(),
            Self::Vote(vote) => vote.signature_preimage(),
            Self::TimeoutVote(vote) => vote.signature_preimage(),
        }
    }

    /// Return the exact block subject owned by proposal or phase-vote work.
    ///
    /// Timeout votes carry only an optional high-QC report and do not own that
    /// certificate's body pipeline, so they deliberately return `None`.
    pub(crate) const fn subject(&self) -> Option<wire::BlockSubject> {
        match self {
            Self::Proposal(proposal) => Some(proposal.subject),
            Self::Vote(vote) => Some(vote.subject),
            Self::TimeoutVote(_) => None,
        }
    }

    /// Return the exact proposal/body origin owned by proposal or phase-vote work.
    ///
    /// As with [`Self::subject`], a timeout vote is view-progress work rather
    /// than ownership of the body named by its optional high QC.
    pub(crate) const fn body_round(&self) -> Option<wire::ConsensusRound> {
        match self {
            Self::Proposal(proposal) => Some(proposal.round),
            Self::Vote(vote) => Some(vote.proposal_round),
            Self::TimeoutVote(_) => None,
        }
    }
}

/// Return whether a proposal still satisfies the safe-value rule for one lock.
///
/// The exact locked subject remains safe in a later justified view. A different
/// subject is safe only when the immediately preceding timeout certificate
/// carries a strictly higher PrepareQC for that same subject.
pub(crate) fn proposal_is_safe_for_lock(
    proposal: &wire::Proposal,
    locked_round: wire::ConsensusRound,
    locked_subject: wire::BlockSubject,
) -> bool {
    if proposal.round.context_id != locked_round.context_id
        || proposal.round.height != locked_round.height
        || proposal.round.view < locked_round.view
    {
        return false;
    }
    if proposal.subject == locked_subject {
        return true;
    }
    let wire::ProposalJustification::Timeout(timeout) = &proposal.justification else {
        return false;
    };
    timeout.highest_prepare_qc.as_ref().is_some_and(|highest| {
        highest.phase == wire::GlobalPhase::Prepare
            && highest.round.context_id == locked_round.context_id
            && highest.round.height == locked_round.height
            && highest.round.view > locked_round.view
            && highest.subject == proposal.subject
            && timeout
                .timeout_certificate
                .highest_prepare_qc()
                .is_some_and(|selected| selected == highest)
    })
}

/// Effects delivered by the production adapter to asynchronous services.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AdapterEffect {
    /// Sign a canonical vote or timeout vote after its WAL intent is durable.
    Sign {
        /// Reducer incarnation tag to return with the signature completion.
        tag: reducer::EventTag,
        /// Canonical unsigned message.
        request: SignRequest,
    },
    /// Broadcast one explicitly versioned canonical v2 envelope.
    Broadcast(wire::ConsensusMessageV2),
    /// Fetch a body from ordinary or certified sources.
    FetchBody {
        /// Reducer incarnation tag for the completion.
        tag: reducer::EventTag,
        /// Proposal round.
        round: wire::ConsensusRound,
        /// Exact requested subject.
        subject: wire::BlockSubject,
        /// Manifest when the proposal supplied one.
        manifest: Option<wire::PayloadManifest>,
        /// Certified validator sources, empty for an uncertified proposal fetch.
        certified_sources: Vec<PeerId>,
        /// Full QC authorizing a certified request, absent for an uncertified
        /// leader-proposal fetch.
        certificate: Option<wire::QuorumCertificate>,
    },
    /// Durably store an already reconstructed exact body.
    StoreBody {
        /// Reducer incarnation tag for the completion.
        tag: reducer::EventTag,
        /// Proposal round.
        round: wire::ConsensusRound,
        /// Exact stored subject.
        subject: wire::BlockSubject,
    },
    /// Run deterministic validation over a durably stored exact body.
    ValidateBody {
        /// Reducer incarnation tag for the completion.
        tag: reducer::EventTag,
        /// Proposal round.
        round: wire::ConsensusRound,
        /// Exact subject to validate.
        subject: wire::BlockSubject,
    },
    /// Apply a decision only after its CommitQC decision record is durable.
    Apply {
        /// Reducer incarnation tag for the application completion.
        tag: reducer::EventTag,
        /// Exact finalized subject.
        subject: wire::BlockSubject,
        /// Canonical CommitQC authorizing application.
        certificate: wire::QuorumCertificate,
    },
    /// Reset lifecycle ownership after a persisted timeout certificate advances
    /// the view or supersedes the current view with a higher-generation lock.
    EnterView {
        /// New reducer incarnation tag.
        tag: reducer::EventTag,
        /// Canonical certificate authorizing the new view.
        certificate: wire::TimeoutCertificate,
        /// Exact post-install durable lock whose body pipeline must survive the transition.
        protected_body: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    },
    /// Report first-release equivocation metadata for operator visibility.
    ReportEquivocation {
        /// Offending voting validator.
        offender: PeerId,
        /// Round containing the conflict.
        round: wire::ConsensusRound,
        /// Conflicting message class.
        kind: reducer::EquivocationKind,
    },
    /// Report a deterministic validation failure for a certified body.
    ReportInvalidCertifiedBody {
        /// Rejected subject.
        subject: wire::BlockSubject,
        /// PrepareQC whose signers certified validity and availability.
        certificate: wire::QuorumCertificate,
    },
}

/// Result of one serialized reducer input after all synchronous WAL work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdapterOutcome {
    disposition: reducer::StepDisposition,
    effects: Vec<AdapterEffect>,
    deferred_admission_ordinal: Option<u128>,
    producer_handoff: Option<ProducerContinuationHandoffToken>,
}

/// Post-finality cleanup result for a reducer height already durable in Kura.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FinalizedV2Height {
    wal_retirement_warning: Option<String>,
}

impl FinalizedV2Height {
    /// Cleanup diagnostic after Kura already made the decision durable.
    ///
    /// A retained WAL is safe and replayable; it must be retried or reported,
    /// but it cannot turn a durably finalized height back into an unfinalized
    /// one.
    pub(crate) fn wal_retirement_warning(&self) -> Option<&str> {
        self.wal_retirement_warning.as_deref()
    }
}

/// Canonical consensus input whose structure and cryptography were verified.
///
/// The tuple field is private so networking code cannot manufacture the token
/// without passing [`SumeragiV2Adapter::authenticate`].
#[derive(Clone)]
pub(crate) struct AuthenticatedConsensusMessage(wire::ConsensusMessageV2);

impl AuthenticatedConsensusMessage {
    /// Borrow the cryptographically authenticated consensus payload.
    pub(crate) const fn payload(&self) -> &wire::ConsensusMessageV2Payload {
        &self.0.payload
    }

    /// Borrow the complete authenticated envelope for exact process-local
    /// ownership association. The private constructor remains the only way to
    /// mint this token.
    pub(crate) const fn wire_envelope(&self) -> &wire::ConsensusMessageV2 {
        &self.0
    }

    /// Return whether two authenticated tokens contain the exact same
    /// deterministic wire envelope.
    ///
    /// The runtime uses this only after independently authenticating the
    /// arriving envelope. Coalescing therefore cannot turn equality with an
    /// already-authenticated value into an authentication bypass.
    pub(crate) fn same_wire_envelope(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    /// Return whether this authenticated token contains the supplied exact
    /// deterministic wire envelope.
    ///
    /// Runtime backpressure may use this comparison only to decide whether an
    /// already-owned retransmission is worth authenticating.  Admission still
    /// receives a fresh [`AuthenticatedConsensusMessage`] before it coalesces
    /// the retransmission.
    pub(crate) fn matches_wire_envelope(&self, message: &wire::ConsensusMessageV2) -> bool {
        self.0 == *message
    }

    /// Canonical bytes of the exact authenticated envelope retained by this
    /// process-local token.
    pub(crate) fn canonical_wire_bytes(&self) -> Vec<u8> {
        self.0.encode()
    }

    /// Clone the exact authenticated envelope for fair-ingress unit fixtures.
    #[cfg(test)]
    pub(crate) fn wire_envelope_for_test(&self) -> wire::ConsensusMessageV2 {
        self.0.clone()
    }

    /// Construct an authenticated token for scheduling-boundary unit tests.
    #[cfg(test)]
    pub(crate) fn for_test(message: wire::ConsensusMessageV2) -> Self {
        Self(message)
    }
}

/// Full trusted evidence retained while a body-pipeline completion waits in
/// the adapter's Busy-deferred lane.
///
/// Reducer events intentionally contain only the consensus fields they
/// consume.  Queue ownership is stricter: an asynchronous retry may coalesce
/// only when every manifest and non-forgeable receipt is byte-for-byte equal
/// to the already-owned completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BodyPipelineCompletionEvidence {
    /// Locally assembled proposal bytes crossed both durable boundaries.
    LocalProposalReady {
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    },
    /// Canonical body reconstruction completed with this exact manifest.
    BodyAvailable { manifest: wire::PayloadManifest },
    /// Canonical body storage completed with this exact durable receipt.
    BodyStored {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    },
    /// Deterministic validation succeeded with this exact validated receipt.
    ValidationSucceeded {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    },
    /// Deterministic validation rejected this exact body.
    ValidationFailed {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    },
}

/// Decision-time disposition for one exact local-proposal completion owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecisionLocalProposalDisposition {
    /// The completion is fully bound to the installed Decision and current reducer tag.
    Retain,
    /// The evidence is valid but its stale tag requires ordinary body-pipeline recovery.
    RetireForRecovery,
    /// The queued trusted evidence conflicts with the installed Decision.
    Conflict,
}

/// Classify one queued local-proposal completion against a durable Decision.
///
/// `None` means the completion does not belong to the selected durable body.
/// An exact Decision owner is retainable only when both non-forgeable receipts
/// bind the full manifest and Decision execution commitment, and its tag is the
/// current reducer tag for the certificate's strict same round. The immutable
/// block header may have been constructed in an earlier view, but the manifest,
/// Vote, and CommitQC for this reproposal all name one exact round.
pub(crate) fn classify_decided_local_proposal(
    tag: reducer::EventTag,
    manifest: &wire::PayloadManifest,
    durable_receipt: &DurableBodyReceipt,
    validated_receipt: &ValidatedBodyReceipt,
    decision_tag: reducer::EventTag,
    decision_body_round: wire::ConsensusRound,
    decision_subject: wire::BlockSubject,
    decision_commitment: wire::ExecutionCommitment,
) -> Option<DecisionLocalProposalDisposition> {
    if manifest.round != decision_body_round || manifest.subject != decision_subject {
        return None;
    }
    if durable_receipt.context_id() != decision_body_round.context_id
        || durable_receipt.round() != manifest.round
        || durable_receipt.subject() != decision_subject
        || durable_receipt.manifest_hash() != HashOf::new(manifest)
        || validated_receipt.durable() != durable_receipt
        || validated_receipt.execution_commitment() != decision_commitment
    {
        return Some(DecisionLocalProposalDisposition::Conflict);
    }
    Some(
        if tag == decision_tag
            && tag.height() == decision_body_round.height
            && tag.view() == manifest.round.view
        {
            DecisionLocalProposalDisposition::Retain
        } else {
            DecisionLocalProposalDisposition::RetireForRecovery
        },
    )
}

#[derive(Clone, Debug)]
struct DeferredInput {
    admission_ordinal: u128,
    admission_capability: DeferredAdmissionCapability,
    event: reducer::Event,
    completion_evidence: Option<BodyPipelineCompletionEvidence>,
    retag_authenticated_ingress: bool,
    priority: DeferredPriority,
    protected_progress: bool,
    admission: Option<IngressAdmission>,
    authenticated_wire_identity: Option<Arc<[u8]>>,
    admitted_at: Instant,
    eligible_skips: u64,
}

struct DeferPolicyOutcome {
    outcome: AdapterOutcome,
}

impl PartialEq for DeferredInput {
    fn eq(&self, other: &Self) -> bool {
        self.event == other.event
            && self.completion_evidence == other.completion_evidence
            && self.retag_authenticated_ingress == other.retag_authenticated_ingress
            && self.priority == other.priority
            && self.protected_progress == other.protected_progress
            && self.authenticated_wire_identity == other.authenticated_wire_identity
    }
}

impl Eq for DeferredInput {}

/// Actor-owned source of process-local deferred admission ordinals.
///
/// The source is deliberately shared across height adapters. Replacing an
/// adapter therefore cannot alias a stale deferred capability by restarting
/// the sequence. Values are opaque, never serialized, and have no consensus
/// meaning.
#[derive(Clone, Debug)]
pub(crate) struct DeferredAdmissionOrdinalSource {
    state: Arc<Mutex<DeferredAdmissionOrdinalState>>,
    identity: Arc<()>,
}

#[derive(Debug)]
struct DeferredAdmissionOrdinalState {
    next: u128,
}

impl DeferredAdmissionOrdinalSource {
    /// Construct an actor-global source whose first successful admission uses
    /// `first`.
    ///
    /// Callers must retain and reuse this source across replacement height
    /// adapters. The runtime actor must inject the same source into every
    /// replacement adapter; there is deliberately no process-global fallback.
    pub(crate) fn new(first: u128) -> Self {
        Self {
            state: Arc::new(Mutex::new(DeferredAdmissionOrdinalState { next: first })),
            identity: Arc::new(()),
        }
    }

    fn mint(&self) -> Result<DeferredAdmissionCapability, AdapterError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| AdapterError::DeferredAdmissionOrdinalSourceUnavailable)?;
        // Reserve the next ordinal before returning the current one. `u128::MAX`
        // is never issued, so every successful capability has a distinct
        // representable next value and exhaustion cannot wrap to a stale owner.
        let next = state
            .next
            .checked_add(1)
            .ok_or(AdapterError::DeferredAdmissionOrdinalExhausted)?;
        let ordinal = state.next;
        state.next = next;
        Ok(DeferredAdmissionCapability {
            ordinal,
            source_identity: Arc::clone(&self.identity),
            adapter_service_claimed: Arc::new(AtomicBool::new(false)),
            runtime_handoff_claimed: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            unbound_fixture: false,
        })
    }

    #[cfg(test)]
    fn next_for_test(&self) -> u128 {
        self.state
            .lock()
            .expect("test deferred ordinal source remains available")
            .next
    }
}

#[derive(Clone, Debug)]
struct DeferredAdmissionCapability {
    ordinal: u128,
    source_identity: Arc<()>,
    adapter_service_claimed: Arc<AtomicBool>,
    runtime_handoff_claimed: Arc<AtomicBool>,
    #[cfg(test)]
    unbound_fixture: bool,
}

impl PartialEq for DeferredAdmissionCapability {
    fn eq(&self, other: &Self) -> bool {
        self.ordinal == other.ordinal
            && Arc::ptr_eq(&self.source_identity, &other.source_identity)
            && Arc::ptr_eq(
                &self.adapter_service_claimed,
                &other.adapter_service_claimed,
            )
            && Arc::ptr_eq(
                &self.runtime_handoff_claimed,
                &other.runtime_handoff_claimed,
            )
            && {
                #[cfg(test)]
                {
                    self.unbound_fixture == other.unbound_fixture
                }
                #[cfg(not(test))]
                {
                    true
                }
            }
    }
}

impl Eq for DeferredAdmissionCapability {}

impl DeferredAdmissionCapability {
    fn pending() -> Self {
        Self {
            ordinal: 0,
            source_identity: Arc::new(()),
            adapter_service_claimed: Arc::new(AtomicBool::new(false)),
            runtime_handoff_claimed: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            unbound_fixture: false,
        }
    }

    #[cfg(test)]
    fn for_test(ordinal: u128) -> Self {
        Self {
            ordinal,
            source_identity: Arc::new(()),
            adapter_service_claimed: Arc::new(AtomicBool::new(false)),
            runtime_handoff_claimed: Arc::new(AtomicBool::new(false)),
            unbound_fixture: true,
        }
    }

    fn claim_adapter_service_once(&self) -> bool {
        self.adapter_service_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn adapter_service_is_claimed(&self) -> bool {
        self.adapter_service_claimed.load(Ordering::Acquire)
    }

    fn claim_runtime_handoff_once(&self) -> bool {
        self.runtime_handoff_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn runtime_handoff_is_claimed(&self) -> bool {
        self.runtime_handoff_claimed.load(Ordering::Acquire)
    }
}

/// Three bounded classes in the adapter-owned Busy-deferred lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeferredPriority {
    /// Trusted completions and local timer events which untrusted traffic
    /// must not displace.
    Completion,
    /// Validated QCs/TCs, TimeoutVote messages, and exact locked-round Commit
    /// reconstruction.
    Progress,
    /// Proposals and individual control votes.
    Normal,
}

impl DeferredPriority {
    const fn code(self) -> u8 {
        match self {
            Self::Completion => 1,
            Self::Progress => 2,
            Self::Normal => 3,
        }
    }
}

/// Typed reducer-event discriminant retained by a deferred service token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeferredEventKind {
    /// Complete safety-WAL replay.
    ResumeAfterReplay,
    /// Locally built body is durable and valid.
    LocalProposalReady,
    /// Authenticated proposal.
    ProposalReceived,
    /// Authenticated Prepare or Commit vote.
    VoteReceived,
    /// Verified PrepareQC or CommitQC.
    QuorumCertificateReceived,
    /// Authenticated timeout vote.
    TimeoutVoteReceived,
    /// Verified timeout certificate.
    TimeoutCertificateReceived,
    /// Absolute round timeout.
    TimeoutElapsed,
    /// Periodic retransmission timeout.
    RetransmitElapsed,
    /// Reconstructed body completion.
    BodyAvailable,
    /// Durable body-store completion.
    BodyStored,
    /// Deterministic validation completion.
    ValidationCompleted,
    /// Safety-WAL persistence acknowledgement.
    Persisted,
    /// Safety-WAL persistence failure.
    PersistenceFailed,
    /// Local signing completion.
    Signed,
    /// Local application completion.
    ApplicationCompleted,
}

impl DeferredEventKind {
    const fn code(self) -> u8 {
        match self {
            Self::LocalProposalReady => 0,
            Self::ProposalReceived => 1,
            Self::VoteReceived => 2,
            Self::QuorumCertificateReceived => 3,
            Self::TimeoutVoteReceived => 4,
            Self::TimeoutCertificateReceived => 5,
            Self::TimeoutElapsed => 6,
            Self::RetransmitElapsed => 7,
            Self::BodyAvailable => 8,
            Self::BodyStored => 9,
            Self::ValidationCompleted => 10,
            Self::Persisted => 11,
            Self::PersistenceFailed => reducer::EVENT_PERSISTENCE_FAILED,
            Self::Signed => 13,
            Self::ApplicationCompleted => 14,
            Self::ResumeAfterReplay => 15,
        }
    }
}

/// Exact local retagging relation for one selected deferred occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeferredRetagRelation {
    /// The asynchronous completion or local event retained its original tag.
    Unchanged,
    /// An authenticated network event was rebound to the current reducer tag.
    AuthenticatedIngress {
        /// Tag retained while the authenticated event waited.
        from: reducer::EventTag,
        /// Current reducer tag used for this retry.
        to: reducer::EventTag,
    },
}

/// Per-class deferred queue lengths around one exact service selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeferredQueueLengths {
    /// Completion-lane owners.
    pub(crate) completion: u64,
    /// Progress-lane owners.
    pub(crate) progress: u64,
    /// Normal-lane owners.
    pub(crate) normal: u64,
}

impl DeferredQueueLengths {
    fn total(self) -> u64 {
        self.checked_total()
            .expect("bounded deferred queue totals fit u64")
    }

    fn checked_total(self) -> Option<u64> {
        self.completion
            .checked_add(self.progress)?
            .checked_add(self.normal)
    }

    const fn for_priority(self, priority: DeferredPriority) -> u64 {
        match priority {
            DeferredPriority::Completion => self.completion,
            DeferredPriority::Progress => self.progress,
            DeferredPriority::Normal => self.normal,
        }
    }
}

/// Exact process-local owner discharged by one Busy-deferred service turn.
///
/// The full typed events remain process-local and make semantic identity
/// lossless. `projection_hash` is a deterministic integrity projection over
/// every externally inspected field and every fixed event/evidence field; it
/// is not a wire capability and must never be serialized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeferredServiceEvidence {
    /// Actor-global ordinal minted only when this owner first entered a queue.
    pub(crate) admission_ordinal: u128,
    /// Exact selected queue class.
    pub(crate) priority: DeferredPriority,
    /// Typed reducer-event discriminant.
    pub(crate) event_kind: DeferredEventKind,
    /// Original tag retained at admission.
    pub(crate) original_tag: reducer::EventTag,
    /// Effective tag dispatched to the reducer.
    pub(crate) effective_tag: reducer::EventTag,
    /// Exact authenticated-retag relation.
    pub(crate) retag: DeferredRetagRelation,
    /// Whether the selected owner is protected locked-round progress.
    pub(crate) protected_progress: bool,
    /// Selected owner's accumulated eligible service debt.
    pub(crate) eligible_skips_before: u64,
    /// Service retires the selected owner's debt.
    pub(crate) eligible_skips_after: u64,
    /// All class lengths before selection.
    pub(crate) queue_lengths_before: DeferredQueueLengths,
    /// All class lengths after selection.
    pub(crate) queue_lengths_after: DeferredQueueLengths,
    /// Redundant exact total before selection.
    pub(crate) total_len_before: u64,
    /// Redundant exact total after selection.
    pub(crate) total_len_after: u64,
    /// Three-class service cursor before selection.
    pub(crate) service_cursor_before: DeferredPriority,
    /// Three-class service cursor after selection.
    pub(crate) service_cursor_after: DeferredPriority,
    /// Hash over the complete immutable process-local projection.
    pub(crate) projection_hash: Hash,
    original_event: reducer::Event,
    effective_event: reducer::Event,
    completion_evidence: Option<BodyPipelineCompletionEvidence>,
    original_admission: Option<IngressAdmission>,
    effective_admission: Option<IngressAdmission>,
    authenticated_wire_identity: Option<Arc<[u8]>>,
    admission_capability: DeferredAdmissionCapability,
}

impl DeferredServiceEvidence {
    /// Construct one internally consistent Completion-lane token for scheduler
    /// shell tests which use a fake driver rather than a real adapter.
    #[cfg(test)]
    pub(crate) fn completion_for_test(
        source: &DeferredAdmissionOrdinalSource,
        tag: reducer::EventTag,
        completion_len_before: u64,
        service_cursor_before: DeferredPriority,
    ) -> Self {
        assert!(completion_len_before != 0);
        let admission_capability = source
            .mint()
            .expect("test deferred ordinal remains available");
        let admission_ordinal = admission_capability.ordinal;
        let event = reducer::Event::TimeoutElapsed { tag };
        let queue_lengths_before = DeferredQueueLengths {
            completion: completion_len_before,
            progress: 0,
            normal: 0,
        };
        let queue_lengths_after = DeferredQueueLengths {
            completion: completion_len_before - 1,
            progress: 0,
            normal: 0,
        };
        let mut cursor = service_cursor_before;
        for _ in 0..3 {
            let selected = cursor;
            cursor = cursor.next();
            if selected == DeferredPriority::Completion {
                break;
            }
        }
        let mut evidence = Self {
            admission_ordinal,
            priority: DeferredPriority::Completion,
            event_kind: DeferredEventKind::TimeoutElapsed,
            original_tag: tag,
            effective_tag: tag,
            retag: DeferredRetagRelation::Unchanged,
            protected_progress: false,
            eligible_skips_before: 0,
            eligible_skips_after: 0,
            queue_lengths_before,
            queue_lengths_after,
            total_len_before: queue_lengths_before.total(),
            total_len_after: queue_lengths_after.total(),
            service_cursor_before,
            service_cursor_after: cursor,
            projection_hash: Hash::new([]),
            original_event: event.clone(),
            effective_event: event,
            completion_evidence: None,
            original_admission: None,
            effective_admission: None,
            authenticated_wire_identity: None,
            admission_capability,
        };
        evidence.projection_hash = deferred_service_projection_hash(&evidence);
        assert!(evidence.validate_exact());
        evidence
    }

    /// Return whether every redundant field and rank transition still matches
    /// the exact selected occurrence.
    pub(crate) fn validate_exact(&self) -> bool {
        if self.admission_capability.ordinal != self.admission_ordinal
            || self.event_kind != deferred_event_kind(&self.original_event)
            || self.event_kind != deferred_event_kind(&self.effective_event)
            || self.original_tag != deferred_event_tag(&self.original_event)
            || self.effective_tag != deferred_event_tag(&self.effective_event)
            || self.eligible_skips_after != 0
            || Some(self.total_len_before) != self.queue_lengths_before.checked_total()
            || Some(self.total_len_after) != self.queue_lengths_after.checked_total()
            || self.total_len_after.checked_add(1) != Some(self.total_len_before)
            || self
                .queue_lengths_before
                .for_priority(self.priority)
                .checked_sub(1)
                != Some(self.queue_lengths_after.for_priority(self.priority))
        {
            return false;
        }
        if self.is_authenticated_ingress() != self.authenticated_wire_identity.is_some() {
            return false;
        }
        if let Some(identity) = &self.authenticated_wire_identity {
            let mut cursor = identity.as_ref();
            let Ok(message) = wire::ConsensusMessageV2::decode(&mut cursor) else {
                return false;
            };
            if !cursor.is_empty()
                || !matches!(
                    (&message.payload, self.event_kind),
                    (
                        wire::ConsensusMessageV2Payload::Proposal(_),
                        DeferredEventKind::ProposalReceived
                    ) | (
                        wire::ConsensusMessageV2Payload::Vote(_),
                        DeferredEventKind::VoteReceived
                    ) | (
                        wire::ConsensusMessageV2Payload::QuorumCertificate(_),
                        DeferredEventKind::QuorumCertificateReceived
                    ) | (
                        wire::ConsensusMessageV2Payload::TimeoutVote(_),
                        DeferredEventKind::TimeoutVoteReceived
                    ) | (
                        wire::ConsensusMessageV2Payload::TimeoutCertificate(_),
                        DeferredEventKind::TimeoutCertificateReceived
                    )
                )
            {
                return false;
            }
        }
        for priority in [
            DeferredPriority::Completion,
            DeferredPriority::Progress,
            DeferredPriority::Normal,
        ] {
            if priority != self.priority
                && self.queue_lengths_before.for_priority(priority)
                    != self.queue_lengths_after.for_priority(priority)
            {
                return false;
            }
        }
        let mut cursor = self.service_cursor_before;
        let mut expected_selection = None;
        let mut expected_after = cursor;
        for _ in 0..3 {
            let candidate = cursor;
            cursor = cursor.next();
            if self.queue_lengths_before.for_priority(candidate) != 0 {
                expected_selection = Some(candidate);
                expected_after = cursor;
                break;
            }
        }
        if expected_selection != Some(self.priority) || expected_after != self.service_cursor_after
        {
            return false;
        }
        let retag_is_exact = match self.retag {
            DeferredRetagRelation::Unchanged => {
                self.original_tag == self.effective_tag
                    && self.original_event == self.effective_event
                    && self.original_admission == self.effective_admission
            }
            DeferredRetagRelation::AuthenticatedIngress { from, to } => {
                from == self.original_tag
                    && to == self.effective_tag
                    && self.original_event.clone().retag_authenticated_ingress(to)
                        == self.effective_event
                    && self.original_admission.map(|mut admission| {
                        admission.generation = to.generation();
                        admission
                    }) == self.effective_admission
            }
        };
        retag_is_exact && self.projection_hash == deferred_service_projection_hash(self)
    }

    /// Return whether this token owns the supplied exact reducer event.
    pub(crate) fn matches_effective_event(&self, event: &reducer::Event) -> bool {
        self.validate_exact() && self.effective_event == *event
    }

    /// Return whether the adapter claimed this owner before reducer dispatch.
    pub(crate) fn adapter_service_is_claimed(&self) -> bool {
        self.admission_capability.adapter_service_is_claimed()
    }

    /// Atomically consume the adapter-to-runtime handoff once. Cloned or
    /// replayed tokens retain the same process-local capability and fail after
    /// the first successful claim.
    pub(crate) fn claim_runtime_handoff_once(&self) -> bool {
        self.validate_exact()
            && self.adapter_service_is_claimed()
            && self.admission_capability.claim_runtime_handoff_once()
    }

    /// Return whether both production seams consumed this exact occurrence.
    pub(crate) fn service_handoff_is_complete(&self) -> bool {
        self.adapter_service_is_claimed() && self.admission_capability.runtime_handoff_is_claimed()
    }

    /// Whether this deferred occurrence originated at authenticated network
    /// ingress and therefore requires the runtime's matching fair-ingress
    /// carrier until service completes.
    pub(crate) const fn is_authenticated_ingress(&self) -> bool {
        matches!(
            self.retag,
            DeferredRetagRelation::AuthenticatedIngress { .. }
        )
    }

    /// Whether this token retains the exact canonical authenticated envelope
    /// carried by the serialized runtime owner.
    pub(crate) fn matches_authenticated_runtime_bytes(&self, canonical_bytes: &[u8]) -> bool {
        self.validate_exact()
            && self
                .authenticated_wire_identity
                .as_deref()
                .is_some_and(|identity| identity == canonical_bytes)
    }

    #[cfg(test)]
    pub(crate) fn claim_adapter_service_for_test(&self) -> bool {
        self.validate_exact() && self.admission_capability.claim_adapter_service_once()
    }

    /// Return whether this occurrence was minted by the supplied actor-owned
    /// source rather than another runtime actor with an overlapping ordinal.
    pub(crate) fn belongs_to(&self, source: &DeferredAdmissionOrdinalSource) -> bool {
        let exact = Arc::ptr_eq(&self.admission_capability.source_identity, &source.identity);
        #[cfg(test)]
        {
            exact || self.admission_capability.unbound_fixture
        }
        #[cfg(not(test))]
        {
            exact
        }
    }
}

struct DeferredServiceSelection {
    input: DeferredInput,
    evidence: DeferredServiceEvidence,
}

fn deferred_event_kind(event: &reducer::Event) -> DeferredEventKind {
    match event {
        reducer::Event::ResumeAfterReplay { .. } => DeferredEventKind::ResumeAfterReplay,
        reducer::Event::LocalProposalReady { .. } => DeferredEventKind::LocalProposalReady,
        reducer::Event::ProposalReceived { .. } => DeferredEventKind::ProposalReceived,
        reducer::Event::VoteReceived { .. } => DeferredEventKind::VoteReceived,
        reducer::Event::QuorumCertificateReceived { .. } => {
            DeferredEventKind::QuorumCertificateReceived
        }
        reducer::Event::TimeoutVoteReceived { .. } => DeferredEventKind::TimeoutVoteReceived,
        reducer::Event::TimeoutCertificateReceived { .. } => {
            DeferredEventKind::TimeoutCertificateReceived
        }
        reducer::Event::TimeoutElapsed { .. } => DeferredEventKind::TimeoutElapsed,
        reducer::Event::RetransmitElapsed { .. } => DeferredEventKind::RetransmitElapsed,
        reducer::Event::BodyAvailable { .. } => DeferredEventKind::BodyAvailable,
        reducer::Event::BodyStored { .. } => DeferredEventKind::BodyStored,
        reducer::Event::ValidationCompleted { .. } => DeferredEventKind::ValidationCompleted,
        reducer::Event::Persisted { .. } => DeferredEventKind::Persisted,
        reducer::Event::PersistenceFailed { .. } => DeferredEventKind::PersistenceFailed,
        reducer::Event::Signed { .. } => DeferredEventKind::Signed,
        reducer::Event::ApplicationCompleted { .. } => DeferredEventKind::ApplicationCompleted,
    }
}

fn deferred_event_tag(event: &reducer::Event) -> reducer::EventTag {
    match event {
        reducer::Event::ResumeAfterReplay { tag }
        | reducer::Event::LocalProposalReady { tag, .. }
        | reducer::Event::ProposalReceived { tag, .. }
        | reducer::Event::VoteReceived { tag, .. }
        | reducer::Event::QuorumCertificateReceived { tag, .. }
        | reducer::Event::TimeoutVoteReceived { tag, .. }
        | reducer::Event::TimeoutCertificateReceived { tag, .. }
        | reducer::Event::TimeoutElapsed { tag }
        | reducer::Event::RetransmitElapsed { tag }
        | reducer::Event::BodyAvailable { tag, .. }
        | reducer::Event::BodyStored { tag, .. }
        | reducer::Event::ValidationCompleted { tag, .. }
        | reducer::Event::Persisted { tag, .. }
        | reducer::Event::PersistenceFailed { tag, .. }
        | reducer::Event::Signed { tag, .. }
        | reducer::Event::ApplicationCompleted { tag, .. } => *tag,
    }
}

fn append_deferred_projection_field(projection: &mut Vec<u8>, field: &[u8]) {
    let len = u64::try_from(field.len()).expect("bounded deferred projection field fits u64");
    projection.extend_from_slice(&len.to_le_bytes());
    projection.extend_from_slice(field);
}

fn append_deferred_projection_u64(projection: &mut Vec<u8>, value: u64) {
    append_deferred_projection_field(projection, &value.to_le_bytes());
}

fn append_deferred_projection_tag(projection: &mut Vec<u8>, tag: reducer::EventTag) {
    append_deferred_projection_u64(projection, tag.height());
    append_deferred_projection_u64(projection, tag.view());
    append_deferred_projection_u64(projection, tag.generation().get());
}

fn append_deferred_projection_round(projection: &mut Vec<u8>, round: reducer::Round) {
    append_deferred_projection_u64(projection, round.height());
    append_deferred_projection_u64(projection, round.view());
}

fn append_deferred_projection_phase(projection: &mut Vec<u8>, phase: reducer::Phase) {
    projection.push(match phase {
        reducer::Phase::Prepare => 1,
        reducer::Phase::Commit => 2,
    });
}

fn append_deferred_projection_signature(
    projection: &mut Vec<u8>,
    signature: &reducer::OpaqueSignature,
) {
    append_deferred_projection_field(projection, signature.as_bytes());
}

fn append_deferred_projection_certificate(
    projection: &mut Vec<u8>,
    certificate: &reducer::QuorumCertificate,
) {
    let reference = certificate.reference();
    append_deferred_projection_field(projection, reference.context_id().as_bytes());
    append_deferred_projection_round(projection, reference.round());
    append_deferred_projection_round(projection, reference.proposal_round());
    append_deferred_projection_phase(projection, reference.phase());
    append_deferred_projection_field(projection, reference.subject().as_bytes());
    append_deferred_projection_u64(
        projection,
        u64::try_from(certificate.signatures().len())
            .expect("bounded certificate signer count fits u64"),
    );
    for share in certificate.signatures() {
        append_deferred_projection_field(projection, share.signer().as_bytes());
        append_deferred_projection_signature(projection, share.signature());
    }
}

fn append_deferred_projection_manifest(
    projection: &mut Vec<u8>,
    manifest: &reducer::PayloadManifest,
) {
    append_deferred_projection_field(projection, manifest.subject().as_bytes());
    append_deferred_projection_field(projection, manifest.payload_hash().as_bytes());
    append_deferred_projection_field(projection, manifest.chunk_root().as_bytes());
    append_deferred_projection_u64(projection, manifest.byte_len());
    append_deferred_projection_field(projection, &manifest.chunk_count().to_le_bytes());
}

fn append_deferred_projection_timeout_certificate(
    projection: &mut Vec<u8>,
    certificate: &reducer::TimeoutCertificate,
) {
    append_deferred_projection_field(projection, certificate.context_id().as_bytes());
    append_deferred_projection_round(projection, certificate.round());
    append_deferred_projection_u64(
        projection,
        u64::try_from(certificate.groups().len()).expect("bounded timeout group count fits u64"),
    );
    for group in certificate.groups() {
        match group.highest_prepare() {
            Some(highest_prepare) => {
                projection.push(1);
                append_deferred_projection_certificate(projection, highest_prepare);
            }
            None => projection.push(0),
        }
        append_deferred_projection_u64(
            projection,
            u64::try_from(group.signatures().len()).expect("bounded timeout signer count fits u64"),
        );
        for share in group.signatures() {
            append_deferred_projection_field(projection, share.signer().as_bytes());
            append_deferred_projection_signature(projection, share.signature());
        }
    }
}

/// Append the semantic identity of a certified occurrence without projecting
/// the replaceable quorum subset or aggregate-signature carrier.
fn append_serviced_candidate_certificate(
    projection: &mut Vec<u8>,
    certificate: &reducer::QuorumCertificate,
) {
    let reference = certificate.reference();
    append_deferred_projection_field(projection, reference.context_id().as_bytes());
    append_deferred_projection_round(projection, reference.round());
    append_deferred_projection_round(projection, reference.proposal_round());
    append_deferred_projection_phase(projection, reference.phase());
    append_deferred_projection_field(projection, reference.subject().as_bytes());
}

/// Append the semantic timeout occurrence selected by the certified round and
/// highest Prepare reference. Signer grouping and aggregate bytes are
/// authenticated carriers, not additional logical owners.
fn append_serviced_candidate_timeout_certificate(
    projection: &mut Vec<u8>,
    certificate: &reducer::TimeoutCertificate,
) {
    append_deferred_projection_field(projection, certificate.context_id().as_bytes());
    append_deferred_projection_round(projection, certificate.round());
    match certificate.highest_prepare() {
        Some(highest_prepare) => {
            projection.push(1);
            append_serviced_candidate_certificate(projection, highest_prepare);
        }
        None => projection.push(0),
    }
}

fn append_deferred_projection_event(projection: &mut Vec<u8>, event: &reducer::Event) {
    projection.push(deferred_event_kind(event).code());
    append_deferred_projection_tag(projection, deferred_event_tag(event));
    match event {
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. } => {}
        reducer::Event::LocalProposalReady { manifest, .. } => {
            append_deferred_projection_manifest(projection, manifest);
        }
        reducer::Event::ProposalReceived { proposal, .. } => {
            let body = proposal.proposal();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_field(projection, body.proposer().as_bytes());
            append_deferred_projection_manifest(projection, body.manifest());
            match body.justification() {
                reducer::ProposalJustification::ParentCommit(reference) => {
                    projection.push(1);
                    match reference {
                        Some(reference) => {
                            projection.push(1);
                            append_deferred_projection_field(
                                projection,
                                reference.context_id().as_bytes(),
                            );
                            append_deferred_projection_round(projection, reference.round());
                            append_deferred_projection_round(
                                projection,
                                reference.proposal_round(),
                            );
                            append_deferred_projection_phase(projection, reference.phase());
                            append_deferred_projection_field(
                                projection,
                                reference.subject().as_bytes(),
                            );
                        }
                        None => projection.push(0),
                    }
                }
                reducer::ProposalJustification::Timeout(certificate) => {
                    projection.push(2);
                    append_deferred_projection_timeout_certificate(projection, certificate);
                }
            }
            append_deferred_projection_signature(projection, proposal.signature());
        }
        reducer::Event::VoteReceived { vote, .. } => {
            let body = vote.vote();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_round(projection, body.proposal_round());
            append_deferred_projection_phase(projection, body.phase());
            append_deferred_projection_field(projection, body.subject().as_bytes());
            append_deferred_projection_field(projection, body.signer().as_bytes());
            append_deferred_projection_signature(projection, vote.signature());
        }
        reducer::Event::QuorumCertificateReceived { certificate, .. } => {
            append_deferred_projection_certificate(projection, certificate);
        }
        reducer::Event::TimeoutVoteReceived { vote, .. } => {
            let body = vote.vote();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_field(projection, body.signer().as_bytes());
            match body.highest_prepare() {
                Some(highest_prepare) => {
                    projection.push(1);
                    append_deferred_projection_certificate(projection, highest_prepare);
                }
                None => projection.push(0),
            }
            append_deferred_projection_signature(projection, vote.signature());
        }
        reducer::Event::TimeoutCertificateReceived { certificate, .. } => {
            append_deferred_projection_timeout_certificate(projection, certificate);
        }
        reducer::Event::BodyAvailable { round, subject, .. }
        | reducer::Event::BodyStored { round, subject, .. } => {
            append_deferred_projection_round(projection, *round);
            append_deferred_projection_field(projection, subject.as_bytes());
        }
        reducer::Event::ValidationCompleted {
            round,
            subject,
            valid,
            ..
        } => {
            append_deferred_projection_round(projection, *round);
            append_deferred_projection_field(projection, subject.as_bytes());
            projection.push(u8::from(*valid));
        }
        reducer::Event::Persisted { id, .. } | reducer::Event::PersistenceFailed { id, .. } => {
            append_deferred_projection_u64(projection, id.get());
        }
        reducer::Event::Signed { signature, .. } => {
            append_deferred_projection_signature(projection, signature);
        }
        reducer::Event::ApplicationCompleted { subject, .. } => {
            append_deferred_projection_field(projection, subject.as_bytes());
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServicedCandidatePolicy {
    /// An already durable occurrence is consumed without re-entering the reducer.
    Suppress,
}

/// Closed adapter-event projection used by the serviced-identity bound.
///
/// These are exactly the reducer input classes which may retain a transient
/// or durable-terminal service record. The formal model projects its more
/// detailed command stages onto this same eleven-class carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum ServicedCandidateStage {
    LocalProposalReady,
    ProposalReceived,
    VoteReceived,
    QuorumCertificateReceived,
    TimeoutVoteReceived,
    TimeoutCertificateReceived,
    TimeoutElapsed,
    BodyAvailable,
    BodyStored,
    ValidationCompleted,
    ApplicationCompleted,
}

impl ServicedCandidateStage {
    const ALL: [Self; 11] = [
        Self::LocalProposalReady,
        Self::ProposalReceived,
        Self::VoteReceived,
        Self::QuorumCertificateReceived,
        Self::TimeoutVoteReceived,
        Self::TimeoutCertificateReceived,
        Self::TimeoutElapsed,
        Self::BodyAvailable,
        Self::BodyStored,
        Self::ValidationCompleted,
        Self::ApplicationCompleted,
    ];
    const COUNT: usize = Self::ALL.len();

    const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::LocalProposalReady),
            1 => Some(Self::ProposalReceived),
            2 => Some(Self::VoteReceived),
            3 => Some(Self::QuorumCertificateReceived),
            4 => Some(Self::TimeoutVoteReceived),
            5 => Some(Self::TimeoutCertificateReceived),
            6 => Some(Self::TimeoutElapsed),
            7 => Some(Self::BodyAvailable),
            8 => Some(Self::BodyStored),
            9 => Some(Self::ValidationCompleted),
            10 => Some(Self::ApplicationCompleted),
            _ => None,
        }
    }
}

/// Physical source which makes a volatile producer parent replayable after a
/// same-height crash.
///
/// This is an internal proof/refinement classifier, not a wire field or
/// configuration knob. Active continuation hashes are never persisted. Only
/// the classes backed by an independently owned local durable source may
/// reserve a continuation; conditional transport and pre-store body results
/// deliberately remain continuation-free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProducerParentReplaySource {
    /// Authenticated ingress is useful only under the explicit responsive-peer
    /// retransmission assumption. A receiver cannot infer that assumption
    /// from a signature, so this class never authorizes a local continuation.
    ConditionalResponsiveTransport,
    /// A reconstructed body has not crossed the durable store boundary yet.
    /// Its manifest is not a reconstruction source for the body bytes.
    VolatileBodyReconstruction,
    /// Exact manifest and non-forgeable receipt retained by the body pipeline.
    DurableBodyPipeline,
    /// Durable reducer view/intent deterministically recreates this local root.
    SafetyWal,
    /// Durable Decision recreates Apply until the matching completion arrives.
    DurableDecision,
}

const fn producer_parent_replay_source_for_stage(
    stage: ServicedCandidateStage,
) -> ProducerParentReplaySource {
    match stage {
        ServicedCandidateStage::ProposalReceived
        | ServicedCandidateStage::VoteReceived
        | ServicedCandidateStage::QuorumCertificateReceived
        | ServicedCandidateStage::TimeoutVoteReceived
        | ServicedCandidateStage::TimeoutCertificateReceived => {
            ProducerParentReplaySource::ConditionalResponsiveTransport
        }
        ServicedCandidateStage::BodyAvailable => {
            ProducerParentReplaySource::VolatileBodyReconstruction
        }
        ServicedCandidateStage::LocalProposalReady
        | ServicedCandidateStage::BodyStored
        | ServicedCandidateStage::ValidationCompleted => {
            ProducerParentReplaySource::DurableBodyPipeline
        }
        ServicedCandidateStage::TimeoutElapsed => ProducerParentReplaySource::SafetyWal,
        ServicedCandidateStage::ApplicationCompleted => ProducerParentReplaySource::DurableDecision,
    }
}

const fn producer_parent_is_locally_reconstructible(stage: ServicedCandidateStage) -> bool {
    matches!(
        producer_parent_replay_source_for_stage(stage),
        ProducerParentReplaySource::DurableBodyPipeline
            | ProducerParentReplaySource::SafetyWal
            | ProducerParentReplaySource::DurableDecision
    )
}

fn producer_parent_has_exact_local_replay_binding(
    event: &reducer::Event,
    completion_evidence: Option<&BodyPipelineCompletionEvidence>,
    durable_decision: bool,
) -> bool {
    let Some(stage) = serviced_candidate_stage(event) else {
        return true;
    };
    match producer_parent_replay_source_for_stage(stage) {
        ProducerParentReplaySource::ConditionalResponsiveTransport => false,
        ProducerParentReplaySource::VolatileBodyReconstruction => false,
        ProducerParentReplaySource::DurableBodyPipeline => matches!(
            (event, completion_evidence),
            (
                reducer::Event::LocalProposalReady { .. },
                Some(BodyPipelineCompletionEvidence::LocalProposalReady { .. })
            ) | (
                reducer::Event::BodyStored { .. },
                Some(BodyPipelineCompletionEvidence::BodyStored { .. })
            ) | (
                reducer::Event::ValidationCompleted { valid: true, .. },
                Some(BodyPipelineCompletionEvidence::ValidationSucceeded { .. })
            ) | (
                reducer::Event::ValidationCompleted { valid: false, .. },
                Some(BodyPipelineCompletionEvidence::ValidationFailed { .. })
            )
        ),
        ProducerParentReplaySource::SafetyWal => {
            matches!(event, reducer::Event::TimeoutElapsed { .. })
        }
        ProducerParentReplaySource::DurableDecision => {
            durable_decision && matches!(event, reducer::Event::ApplicationCompleted { .. })
        }
    }
}

fn serviced_candidate_stage(event: &reducer::Event) -> Option<ServicedCandidateStage> {
    let stage = serviced_candidate_stage_for_kind_code(deferred_event_kind(event).code())?;
    ServicedCandidateStage::from_code(stage)
}

fn serviced_candidate_policy(event: &reducer::Event) -> Option<ServicedCandidatePolicy> {
    serviced_candidate_stage(event).map(|_| ServicedCandidatePolicy::Suppress)
}

fn is_authenticated_ingress_event(event: &reducer::Event) -> bool {
    matches!(
        event,
        reducer::Event::ProposalReceived { .. }
            | reducer::Event::VoteReceived { .. }
            | reducer::Event::QuorumCertificateReceived { .. }
            | reducer::Event::TimeoutVoteReceived { .. }
            | reducer::Event::TimeoutCertificateReceived { .. }
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServicedCandidateRecordKind {
    /// Same-process memory which prevents an applied identity from re-entering
    /// after an equal-rank owner temporarily covers the same reducer stage.
    Transient,
    /// Restart-stable memory for an exact internal lifecycle which drained
    /// after its asynchronous owner disappeared.
    DurableTerminal,
}

/// Exact process-local lifecycle owner supplied by the serialized runtime.
///
/// This carrier is deliberately not serialized. The causal key and immutable
/// admission ordinal are sufficient to coalesce retries while the process is
/// alive, but not to reconstruct a command after restart.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectedProducerLifecycle {
    causal_lifecycle_key: Hash,
    admission_ordinal: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ProducerReservationChange {
    Unchanged,
    Inserted,
    ClaimedDormant,
    ReplacedTerminal {
        process_previous: ProducerContinuationRecord,
        durable_previous: Option<ProducerContinuationRecord>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProducerReservationToken {
    address: ProducerContinuationAddress,
    change: ProducerReservationChange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingProducerHandoff {
    token: ProducerContinuationHandoffToken,
    service_view: wire::View,
    durable_store_terminal: bool,
    durable_terminal_evidence: bool,
    durable_previous: Option<ProducerContinuationRecord>,
}

/// Exact evidence consumed when a runtime-owned producer reservation retires.
///
/// A concrete successor is acknowledged only after the runtime has installed
/// the returned non-empty effect batch in its ownership sidecar. A durable
/// terminal is accepted only when the adapter retained exact terminal evidence
/// for the same opaque token before returning from the reducer transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProducerContinuationHandoffEvidence {
    /// An inherited or fresh causal successor is now physically owned.
    ConcreteSuccessor,
    /// The drained producer reached a process-local last consumer without an
    /// independently durable terminal. Same-height restart must reopen it.
    VolatileTerminal,
    /// Exact durable terminal evidence replaces the drained producer.
    DurableTerminal,
}

/// Classify the only dispositions which consume a serviced-identity slot.
///
/// Authenticated junk, stale/policy rejection, and ordinary duplicates remain
/// marker-free. An applied authenticated occurrence is not junk: retaining its
/// route-neutral identity for the rest of this process generation closes the
/// A -> B -> A replenishment episode while a same-height restart still clears
/// it and permits volatile quorum/pipeline reconstruction.
fn serviced_candidate_record_kind(
    event: &reducer::Event,
    disposition: reducer::StepDisposition,
) -> Option<ServicedCandidateRecordKind> {
    if disposition == reducer::StepDisposition::Applied {
        Some(ServicedCandidateRecordKind::Transient)
    } else if !is_authenticated_ingress_event(event)
        && disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    {
        Some(ServicedCandidateRecordKind::DurableTerminal)
    } else {
        None
    }
}

/// Append a route-neutral event projection which deliberately excludes the
/// process-local reducer generation and consumer-episode tag. The immutable
/// height context and semantic source view are projected by the caller.
fn append_serviced_candidate_event(projection: &mut Vec<u8>, event: &reducer::Event) {
    projection.push(deferred_event_kind(event).code());
    match event {
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. } => {}
        reducer::Event::LocalProposalReady { manifest, .. } => {
            append_deferred_projection_manifest(projection, manifest);
        }
        reducer::Event::ProposalReceived { proposal, .. } => {
            let body = proposal.proposal();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_field(projection, body.proposer().as_bytes());
            append_deferred_projection_manifest(projection, body.manifest());
            match body.justification() {
                reducer::ProposalJustification::ParentCommit(reference) => {
                    projection.push(1);
                    match reference {
                        Some(reference) => {
                            projection.push(1);
                            append_deferred_projection_field(
                                projection,
                                reference.context_id().as_bytes(),
                            );
                            append_deferred_projection_round(projection, reference.round());
                            append_deferred_projection_round(
                                projection,
                                reference.proposal_round(),
                            );
                            append_deferred_projection_phase(projection, reference.phase());
                            append_deferred_projection_field(
                                projection,
                                reference.subject().as_bytes(),
                            );
                        }
                        None => projection.push(0),
                    }
                }
                reducer::ProposalJustification::Timeout(certificate) => {
                    projection.push(2);
                    append_serviced_candidate_timeout_certificate(projection, certificate);
                }
            }
        }
        reducer::Event::VoteReceived { vote, .. } => {
            let body = vote.vote();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_round(projection, body.proposal_round());
            append_deferred_projection_phase(projection, body.phase());
            append_deferred_projection_field(projection, body.subject().as_bytes());
            append_deferred_projection_field(projection, body.signer().as_bytes());
        }
        reducer::Event::QuorumCertificateReceived { certificate, .. } => {
            append_serviced_candidate_certificate(projection, certificate);
        }
        reducer::Event::TimeoutVoteReceived { vote, .. } => {
            let body = vote.vote();
            append_deferred_projection_field(projection, body.context_id().as_bytes());
            append_deferred_projection_round(projection, body.round());
            append_deferred_projection_field(projection, body.signer().as_bytes());
            match body.highest_prepare() {
                Some(highest_prepare) => {
                    projection.push(1);
                    append_serviced_candidate_certificate(projection, highest_prepare);
                }
                None => projection.push(0),
            }
        }
        reducer::Event::TimeoutCertificateReceived { certificate, .. } => {
            append_serviced_candidate_timeout_certificate(projection, certificate);
        }
        reducer::Event::BodyAvailable { round, subject, .. }
        | reducer::Event::BodyStored { round, subject, .. } => {
            append_deferred_projection_round(projection, *round);
            append_deferred_projection_field(projection, subject.as_bytes());
        }
        reducer::Event::ValidationCompleted {
            round,
            subject,
            valid,
            ..
        } => {
            append_deferred_projection_round(projection, *round);
            append_deferred_projection_field(projection, subject.as_bytes());
            projection.push(u8::from(*valid));
        }
        reducer::Event::Persisted { id, .. } | reducer::Event::PersistenceFailed { id, .. } => {
            append_deferred_projection_u64(projection, id.get());
        }
        reducer::Event::Signed { signature, .. } => {
            append_deferred_projection_signature(projection, signature);
        }
        reducer::Event::ApplicationCompleted { subject, .. } => {
            append_deferred_projection_field(projection, subject.as_bytes());
        }
    }
}

fn serviced_candidate_event_fields(event: &reducer::Event) -> (wire::View, Option<[u8; 32]>, u8) {
    let tag_view = deferred_event_tag(event).view();
    match event {
        reducer::Event::LocalProposalReady { manifest, .. } => {
            (tag_view, Some(*manifest.subject().as_bytes()), 0)
        }
        reducer::Event::ProposalReceived { proposal, .. } => {
            let proposal = proposal.proposal();
            (
                proposal.round().view(),
                Some(*proposal.manifest().subject().as_bytes()),
                0,
            )
        }
        reducer::Event::VoteReceived { vote, .. } => {
            let vote = vote.vote();
            (
                vote.round().view(),
                Some(*vote.subject().as_bytes()),
                match vote.phase() {
                    reducer::Phase::Prepare => 1,
                    reducer::Phase::Commit => 2,
                },
            )
        }
        reducer::Event::QuorumCertificateReceived { certificate, .. } => (
            certificate.round().view(),
            Some(*certificate.subject().as_bytes()),
            match certificate.phase() {
                reducer::Phase::Prepare => 1,
                reducer::Phase::Commit => 2,
            },
        ),
        reducer::Event::TimeoutVoteReceived { vote, .. } => {
            let vote = vote.vote();
            (
                vote.round().view(),
                vote.highest_prepare()
                    .map(|certificate| *certificate.subject().as_bytes()),
                3,
            )
        }
        reducer::Event::TimeoutCertificateReceived { certificate, .. } => (
            certificate.round().view(),
            certificate
                .highest_prepare()
                .map(|highest| *highest.subject().as_bytes()),
            3,
        ),
        reducer::Event::BodyAvailable { round, subject, .. }
        | reducer::Event::BodyStored { round, subject, .. }
        | reducer::Event::ValidationCompleted { round, subject, .. } => {
            (round.view(), Some(*subject.as_bytes()), 0)
        }
        reducer::Event::ApplicationCompleted { subject, .. } => {
            (tag_view, Some(*subject.as_bytes()), 2)
        }
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. }
        | reducer::Event::Persisted { .. }
        | reducer::Event::PersistenceFailed { .. }
        | reducer::Event::Signed { .. } => (tag_view, None, 0),
    }
}

fn append_deferred_projection_receipt(projection: &mut Vec<u8>, receipt: &DurableBodyReceipt) {
    append_deferred_projection_field(projection, &receipt.context_id().encode());
    append_deferred_projection_field(projection, &receipt.round().encode());
    append_deferred_projection_field(projection, &receipt.subject().encode());
    append_deferred_projection_field(projection, receipt.manifest_hash().as_ref());
    append_deferred_projection_field(projection, receipt.frame_hash().as_ref());
}

fn append_deferred_projection_completion_evidence(
    projection: &mut Vec<u8>,
    evidence: Option<&BodyPipelineCompletionEvidence>,
) {
    let Some(evidence) = evidence else {
        projection.push(0);
        return;
    };
    match evidence {
        BodyPipelineCompletionEvidence::LocalProposalReady {
            manifest,
            durable_receipt,
            validated_receipt,
        } => {
            projection.push(1);
            append_deferred_projection_field(projection, &manifest.encode());
            append_deferred_projection_receipt(projection, durable_receipt);
            append_deferred_projection_field(
                projection,
                &validated_receipt.execution_commitment().encode(),
            );
        }
        BodyPipelineCompletionEvidence::BodyAvailable { manifest } => {
            projection.push(2);
            append_deferred_projection_field(projection, &manifest.encode());
        }
        BodyPipelineCompletionEvidence::BodyStored {
            round,
            subject,
            receipt,
        } => {
            projection.push(3);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &subject.encode());
            append_deferred_projection_receipt(projection, receipt);
        }
        BodyPipelineCompletionEvidence::ValidationSucceeded {
            round,
            subject,
            receipt,
        } => {
            projection.push(4);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &subject.encode());
            append_deferred_projection_receipt(projection, receipt.durable());
            append_deferred_projection_field(projection, &receipt.execution_commitment().encode());
        }
        BodyPipelineCompletionEvidence::ValidationFailed { round, subject } => {
            projection.push(5);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &subject.encode());
        }
    }
}

fn append_deferred_projection_admission(
    projection: &mut Vec<u8>,
    admission: Option<IngressAdmission>,
) {
    let Some(admission) = admission else {
        projection.push(0);
        return;
    };
    projection.push(1);
    match admission.key {
        IngressSemanticKey::Proposal { round, proposer } => {
            projection.push(1);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &proposer.encode());
        }
        IngressSemanticKey::Vote {
            round,
            phase,
            signer,
        } => {
            projection.push(2);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &phase.encode());
            append_deferred_projection_field(projection, &signer.encode());
        }
        IngressSemanticKey::TimeoutVote { round, signer } => {
            projection.push(3);
            append_deferred_projection_field(projection, &round.encode());
            append_deferred_projection_field(projection, &signer.encode());
        }
    }
    match admission.fingerprint {
        IngressFingerprint::Proposal(hash) => {
            projection.push(1);
            append_deferred_projection_field(projection, hash.as_ref());
        }
        IngressFingerprint::Vote(proposal_round, subject, commitment) => {
            projection.push(2);
            append_deferred_projection_field(projection, &proposal_round.encode());
            append_deferred_projection_field(projection, &subject.encode());
            append_deferred_projection_field(projection, &commitment.encode());
        }
        IngressFingerprint::TimeoutVote(reference) => {
            projection.push(3);
            append_deferred_projection_field(projection, &reference.encode());
        }
    }
    append_deferred_projection_u64(projection, admission.generation.get());
    projection.push(u8::from(admission.inserted_equivocation));
    projection.push(u8::from(admission.locked_commit_progress));
}

fn deferred_service_projection_hash(evidence: &DeferredServiceEvidence) -> Hash {
    let mut projection = Vec::new();
    append_deferred_projection_field(&mut projection, &evidence.admission_ordinal.to_le_bytes());
    projection.push(evidence.priority.code());
    projection.push(evidence.event_kind.code());
    append_deferred_projection_tag(&mut projection, evidence.original_tag);
    append_deferred_projection_tag(&mut projection, evidence.effective_tag);
    match evidence.retag {
        DeferredRetagRelation::Unchanged => projection.push(0),
        DeferredRetagRelation::AuthenticatedIngress { from, to } => {
            projection.push(1);
            append_deferred_projection_tag(&mut projection, from);
            append_deferred_projection_tag(&mut projection, to);
        }
    }
    projection.push(u8::from(evidence.protected_progress));
    append_deferred_projection_u64(&mut projection, evidence.eligible_skips_before);
    append_deferred_projection_u64(&mut projection, evidence.eligible_skips_after);
    for lengths in [evidence.queue_lengths_before, evidence.queue_lengths_after] {
        append_deferred_projection_u64(&mut projection, lengths.completion);
        append_deferred_projection_u64(&mut projection, lengths.progress);
        append_deferred_projection_u64(&mut projection, lengths.normal);
    }
    append_deferred_projection_u64(&mut projection, evidence.total_len_before);
    append_deferred_projection_u64(&mut projection, evidence.total_len_after);
    projection.push(evidence.service_cursor_before.code());
    projection.push(evidence.service_cursor_after.code());
    append_deferred_projection_event(&mut projection, &evidence.original_event);
    append_deferred_projection_event(&mut projection, &evidence.effective_event);
    append_deferred_projection_completion_evidence(
        &mut projection,
        evidence.completion_evidence.as_ref(),
    );
    append_deferred_projection_admission(&mut projection, evidence.original_admission);
    append_deferred_projection_admission(&mut projection, evidence.effective_admission);
    match &evidence.authenticated_wire_identity {
        None => projection.push(0),
        Some(identity) => {
            projection.push(1);
            append_deferred_projection_field(&mut projection, identity);
        }
    }
    Hash::new(projection)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeferredProgressClass {
    LockedCommitVote,
    TimeoutVote,
    PrepareCertificate,
    CommitCertificate,
    TimeoutCertificate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeferredProgressOwner {
    LockedCommitVote(reducer::ValidatorId),
    TimeoutVote(reducer::ValidatorId),
    PrepareCertificate,
    CommitCertificate,
    TimeoutCertificate,
}

impl DeferredProgressOwner {
    const fn class(self) -> DeferredProgressClass {
        match self {
            Self::LockedCommitVote(_) => DeferredProgressClass::LockedCommitVote,
            Self::TimeoutVote(_) => DeferredProgressClass::TimeoutVote,
            Self::PrepareCertificate => DeferredProgressClass::PrepareCertificate,
            Self::CommitCertificate => DeferredProgressClass::CommitCertificate,
            Self::TimeoutCertificate => DeferredProgressClass::TimeoutCertificate,
        }
    }
}

fn deferred_progress_owner(input: &DeferredInput) -> Option<DeferredProgressOwner> {
    if input.protected_progress {
        return match &input.event {
            reducer::Event::VoteReceived { vote, .. }
                if vote.vote().phase() == reducer::Phase::Commit =>
            {
                Some(DeferredProgressOwner::LockedCommitVote(
                    vote.vote().signer(),
                ))
            }
            _ => None,
        };
    }
    match &input.event {
        reducer::Event::TimeoutVoteReceived { vote, .. } => {
            Some(DeferredProgressOwner::TimeoutVote(vote.vote().signer()))
        }
        reducer::Event::QuorumCertificateReceived { certificate, .. } => {
            Some(match certificate.phase() {
                reducer::Phase::Prepare => DeferredProgressOwner::PrepareCertificate,
                reducer::Phase::Commit => DeferredProgressOwner::CommitCertificate,
            })
        }
        reducer::Event::TimeoutCertificateReceived { .. } => {
            Some(DeferredProgressOwner::TimeoutCertificate)
        }
        _ => None,
    }
}

fn deferred_progress_class(input: &DeferredInput) -> Option<DeferredProgressClass> {
    deferred_progress_owner(input).map(DeferredProgressOwner::class)
}

const fn deferred_progress_capacity(roster_len: usize) -> usize {
    let required = roster_len.saturating_mul(2).saturating_add(3);
    if required < MAX_DEFERRED_PROGRESS_INPUTS {
        required
    } else {
        MAX_DEFERRED_PROGRESS_INPUTS
    }
}

const fn semantic_ingress_capacity(roster_len: usize) -> usize {
    MAX_INGRESS_SEMANTIC_KEYS.saturating_add(roster_len.saturating_mul(2))
}

/// Maximum distinct service stages which one immutable lifecycle can cross.
///
/// This is mechanically derived from the closed reducer-event projection
/// accepted by [`serviced_candidate_policy`], not duplicated as a magic
/// number. It is neither a wire field nor a deployment knob.
const _: () = assert!(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE == ServicedCandidateStage::COUNT);
const _: () = assert!(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE == 11);

/// Dormant restart/historical roots in the reviewed lifecycle geometry.
///
/// The formal model uses the same fixed `AsyncDormantDurableLifecycleCapacity`.
const CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY: usize = 8;

/// Existing runtime/effect capacities which bound active candidate roots.
///
/// Production passes the already validated Sumeragi v2 queue configuration
/// into adapter construction. This internal value is not serialized and does
/// not add a configuration surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ServicedCandidateCapacityGeometry {
    runtime_command_capacity: usize,
    effect_work_capacity: usize,
}

impl ServicedCandidateCapacityGeometry {
    /// Bind the existing runtime command and effect-work capacities.
    pub(crate) const fn new(runtime_command_capacity: usize, effect_work_capacity: usize) -> Self {
        Self {
            runtime_command_capacity,
            effect_work_capacity,
        }
    }
}

// Standalone adapter fixtures are paired with the existing 1024-command and
// 1024-effect test defaults. Production construction always supplies the
// validated height configuration explicitly through the runner.
const DEFAULT_SERVICED_CANDIDATE_CAPACITY_GEOMETRY: ServicedCandidateCapacityGeometry =
    ServicedCandidateCapacityGeometry::new(MAX_DEFERRED_INPUTS, MAX_DEFERRED_INPUTS);

const fn candidate_lifecycle_capacity(
    roster_len: usize,
    geometry: ServicedCandidateCapacityGeometry,
) -> usize {
    let serviced = semantic_ingress_capacity(roster_len)
        .saturating_add(MAX_DEFERRED_INPUTS)
        .saturating_add(MAX_DEFERRED_INPUTS)
        .saturating_add(deferred_progress_capacity(roster_len));
    let active = geometry
        .runtime_command_capacity
        // One root plus at most three causal continuations per runtime owner.
        .saturating_add(geometry.runtime_command_capacity.saturating_mul(3))
        .saturating_add(geometry.effect_work_capacity)
        .saturating_add(CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY);
    serviced
        .saturating_add(active)
        // The due timeout clock owns one disjoint lifecycle reservation.
        .saturating_add(1)
}

/// Maximum same-view serviced identities retained by one adapter generation.
///
/// This is the complete reviewed lifecycle geometry: service queues, active
/// runtime roots, their bounded three-child causal continuations, effect work,
/// dormant durable replay, and the disjoint timeout clock. Multiplying by the
/// exact eleven-class reducer-event projection also covers a retained service
/// marker while the same causal lifecycle remains active.
const fn serviced_candidate_capacity_with_geometry(
    roster_len: usize,
    geometry: ServicedCandidateCapacityGeometry,
) -> usize {
    candidate_lifecycle_capacity(roster_len, geometry)
        .saturating_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
}

#[cfg(test)]
const fn serviced_candidate_capacity(roster_len: usize) -> usize {
    serviced_candidate_capacity_with_geometry(
        roster_len,
        DEFAULT_SERVICED_CANDIDATE_CAPACITY_GEOMETRY,
    )
}

/// Completion variant staged directly in the Busy-deferred lane by seam tests.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeferredBodyPipelineStageForTest {
    /// Durable storage completed.
    BodyStored,
    /// Deterministic validation succeeded.
    ValidationSucceeded,
    /// Deterministic validation failed.
    ValidationFailed,
    /// Local proposal construction completed.
    LocalProposalReady,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeferredBodyPipelineCompletionStage {
    LocalProposalReady,
    BodyAvailable,
    BodyStored,
    Validation,
}

fn deferred_body_pipeline_completion_stage(
    input: &DeferredInput,
    tag: reducer::EventTag,
    round: reducer::Round,
    subject: reducer::Subject,
) -> Option<DeferredBodyPipelineCompletionStage> {
    match &input.event {
        reducer::Event::LocalProposalReady {
            tag: queued_tag,
            manifest,
        } if *queued_tag == tag
            && tag.height() == round.height()
            && tag.view() == round.view()
            && manifest.subject() == subject =>
        {
            Some(DeferredBodyPipelineCompletionStage::LocalProposalReady)
        }
        reducer::Event::BodyAvailable {
            tag: queued_tag,
            round: queued_round,
            subject: queued_subject,
        } if *queued_tag == tag && *queued_round == round && *queued_subject == subject => {
            Some(DeferredBodyPipelineCompletionStage::BodyAvailable)
        }
        reducer::Event::BodyStored {
            tag: queued_tag,
            round: queued_round,
            subject: queued_subject,
        } if *queued_tag == tag && *queued_round == round && *queued_subject == subject => {
            Some(DeferredBodyPipelineCompletionStage::BodyStored)
        }
        reducer::Event::ValidationCompleted {
            tag: queued_tag,
            round: queued_round,
            subject: queued_subject,
            ..
        } if *queued_tag == tag && *queued_round == round && *queued_subject == subject => {
            Some(DeferredBodyPipelineCompletionStage::Validation)
        }
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::LocalProposalReady { .. }
        | reducer::Event::ProposalReceived { .. }
        | reducer::Event::VoteReceived { .. }
        | reducer::Event::QuorumCertificateReceived { .. }
        | reducer::Event::TimeoutVoteReceived { .. }
        | reducer::Event::TimeoutCertificateReceived { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. }
        | reducer::Event::BodyAvailable { .. }
        | reducer::Event::BodyStored { .. }
        | reducer::Event::ValidationCompleted { .. }
        | reducer::Event::Persisted { .. }
        | reducer::Event::PersistenceFailed { .. }
        | reducer::Event::Signed { .. }
        | reducer::Event::ApplicationCompleted { .. } => None,
    }
}

fn classify_deferred_decided_local_proposal(
    input: &DeferredInput,
    decision_tag: reducer::EventTag,
    decision_round: wire::ConsensusRound,
    decision_subject: wire::BlockSubject,
    decision_commitment: wire::ExecutionCommitment,
) -> Option<DecisionLocalProposalDisposition> {
    let reducer::Event::LocalProposalReady {
        manifest: core_manifest,
        tag,
    } = &input.event
    else {
        return None;
    };
    let Some(BodyPipelineCompletionEvidence::LocalProposalReady {
        manifest,
        durable_receipt,
        validated_receipt,
    }) = input.completion_evidence.as_ref()
    else {
        return (core_manifest.subject()
            == reducer::Subject::new(Hash::new(decision_subject.encode()).into()))
        .then_some(DecisionLocalProposalDisposition::Conflict);
    };
    let disposition = classify_decided_local_proposal(
        *tag,
        manifest,
        durable_receipt,
        validated_receipt,
        decision_tag,
        decision_round,
        decision_subject,
        decision_commitment,
    );
    let core_matches_evidence = core_manifest.subject()
        == reducer::Subject::new(Hash::new(manifest.subject.encode()).into())
        && core_manifest.payload_hash()
            == reducer::Digest::new(*manifest.subject.payload_hash.as_ref())
        && core_manifest.chunk_root() == reducer::Digest::new(*manifest.chunk_root.as_ref())
        && core_manifest.byte_len() == manifest.payload_size_bytes
        && usize::try_from(core_manifest.chunk_count()).ok() == Some(manifest.chunk_hashes.len());
    if !core_matches_evidence {
        return disposition
            .is_some()
            .then_some(DecisionLocalProposalDisposition::Conflict);
    }
    disposition
}

impl DeferredPriority {
    const fn next(self) -> Self {
        match self {
            Self::Completion => Self::Progress,
            Self::Progress => Self::Normal,
            Self::Normal => Self::Completion,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum IngressSemanticKey {
    Proposal {
        round: wire::ConsensusRound,
        proposer: wire::ValidatorIndex,
    },
    Vote {
        round: wire::ConsensusRound,
        phase: wire::GlobalPhase,
        signer: wire::ValidatorIndex,
    },
    TimeoutVote {
        round: wire::ConsensusRound,
        signer: wire::ValidatorIndex,
    },
}

impl IngressSemanticKey {
    fn round(self) -> wire::ConsensusRound {
        match self {
            Self::Proposal { round, .. }
            | Self::Vote { round, .. }
            | Self::TimeoutVote { round, .. } => round,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IngressFingerprint {
    Proposal(Hash),
    Vote(
        wire::ConsensusRound,
        wire::BlockSubject,
        wire::ExecutionCommitment,
    ),
    TimeoutVote(Option<wire::QuorumCertificateRef>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IngressEquivocationRecord {
    fingerprint: IngressFingerprint,
    equivocation_reported: bool,
    capacity_bypass: bool,
    admitted_at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IngressDeliveryRecord {
    fingerprint: IngressFingerprint,
    generation: reducer::Generation,
    locked_commit_progress: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IngressAdmission {
    key: IngressSemanticKey,
    fingerprint: IngressFingerprint,
    generation: reducer::Generation,
    inserted_equivocation: bool,
    locked_commit_progress: bool,
}

impl AdapterOutcome {
    /// Return whether the reducer applied or deliberately ignored the input.
    pub(crate) const fn disposition(&self) -> reducer::StepDisposition {
        self.disposition
    }

    /// Borrow the effects now safe for asynchronous execution.
    #[cfg(test)]
    pub(crate) fn effects(&self) -> &[AdapterEffect] {
        &self.effects
    }

    /// Consume the outcome and return its asynchronous effects.
    pub(crate) fn into_effects(self) -> Vec<AdapterEffect> {
        self.effects
    }

    /// Exact producer reservation which the serialized runtime must
    /// acknowledge only after installing its replacement owner.
    pub(crate) const fn producer_handoff(&self) -> Option<ProducerContinuationHandoffToken> {
        self.producer_handoff
    }

    /// Actor-global owner retained when this exact input crossed into the
    /// adapter's Busy-deferred queue.
    pub(crate) const fn deferred_admission_ordinal(&self) -> Option<u128> {
        self.deferred_admission_ordinal
    }

    /// Whether Busy backpressure retained no adapter-owned occurrence and the
    /// serialized runtime must keep the exact physical command in its FIFO.
    pub(crate) const fn requires_runtime_retry(&self) -> bool {
        matches!(
            self.disposition,
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        ) && self.deferred_admission_ordinal.is_none()
    }
}

/// Signature aggregation boundary used when the reducer forms a local QC or TC.
pub(crate) trait SignatureAggregator: Send + Sync {
    /// Aggregate the canonical signer-ordered BLS signature shares.
    fn aggregate(&self, signatures: &[&[u8]]) -> Result<Vec<u8>, String>;
}

#[derive(Debug, Default)]
struct BlsNormalSignatureAggregator;

impl SignatureAggregator for BlsNormalSignatureAggregator {
    fn aggregate(&self, signatures: &[&[u8]]) -> Result<Vec<u8>, String> {
        #[cfg(feature = "bls")]
        {
            iroha_crypto::bls_normal_aggregate_signatures(signatures)
                .map_err(|error| error.to_string())
        }
        #[cfg(not(feature = "bls"))]
        {
            let _ = signatures;
            Err("the iroha_core `bls` feature is required by Sumeragi v2".to_owned())
        }
    }
}

/// Fatal or structurally invalid adapter input.
#[derive(Debug, Error)]
pub(crate) enum AdapterError {
    /// Canonical wire validation rejected an input.
    #[error("invalid Sumeragi v2 wire value: {0}")]
    WireValidation(#[from] wire::ValidationError),
    /// The executable reducer rejected a transition.
    #[error("Sumeragi v2 reducer rejected a transition: {0}")]
    Reducer(#[from] reducer::ReducerError),
    /// Frozen context conversion failed.
    #[error("invalid executable Sumeragi v2 height context: {0}")]
    HeightContext(#[from] reducer::HeightContextError),
    /// Genesis verification was requested for a non-genesis context.
    #[error("Sumeragi v2 genesis context must be height 1 with no parent CommitQC")]
    InvalidGenesisContext,
    /// Snapshot bootstrap verification was requested for a normal genesis/successor context.
    #[error("Sumeragi v2 snapshot bootstrap context must be an anchored post-snapshot height")]
    InvalidSnapshotBootstrapContext,
    /// Successor context is not anchored to the supplied durable parent.
    #[error("Sumeragi v2 height context does not match its durable parent artifact")]
    ParentContextMismatch,
    /// Successor election inputs changed outside a certified epoch boundary or
    /// differ from the finalized next-epoch snapshot.
    #[error("Sumeragi v2 successor context violates the certified epoch transition")]
    EpochTransitionMismatch,
    /// Safety WAL I/O or integrity checking failed.
    #[error(transparent)]
    SafetyWal(#[from] SafetyWalError),
    /// The adjacent serviced-candidate snapshot failed validation or publication.
    #[error("Sumeragi v2 serviced-candidate store failed: {0}")]
    ServicedCandidateStore(String),
    /// Kura finality artifact failed structural validation.
    #[error("invalid Sumeragi v2 Kura finality artifact: {0}")]
    FinalityArtifact(#[from] wire::finality::V2FinalityValidationError),
    /// Kura's typed receipt or artifact differs from the reducer's exact decision.
    #[error("Sumeragi v2 Kura finality receipt does not match the applied reducer decision")]
    DurableCommitMismatch,
    /// Body-store receipt differs from the exact manifest, round, or subject.
    #[error("Sumeragi v2 durable body receipt does not match the reducer work item")]
    DurableBodyMismatch,
    /// The runner attempted to publish a successor before its live pacemaker
    /// clocks crossed the one-shot post-startup boundary.
    #[error("Sumeragi v2 successor activation requires armed live pacemaker clocks")]
    SuccessorClocksNotArmed,
    /// A complete WAL payload could not be decoded.
    #[error("invalid Sumeragi v2 safety WAL payload: {0}")]
    WalDecode(String),
    /// A WAL frame sequence did not match the reducer persistence identifier.
    #[error(
        "Sumeragi v2 WAL/reducer sequence mismatch: frame {frame_sequence}, persistence id {persistence_id}"
    )]
    WalSequenceMismatch {
        /// Zero-based file frame sequence.
        frame_sequence: u64,
        /// One-based reducer persistence identifier.
        persistence_id: u64,
    },
    /// A signer index was outside the frozen roster.
    #[error("Sumeragi v2 validator index {0} is outside the frozen roster")]
    ValidatorIndexOutOfRange(u32),
    /// A reducer validator token was not present in the adapter mapping.
    #[error("unknown executable Sumeragi v2 validator token {0}")]
    UnknownValidator(reducer::ValidatorId),
    /// A digest could not be expanded to its canonical block subject.
    #[error("unknown executable Sumeragi v2 subject {0}")]
    UnknownSubject(reducer::Subject),
    /// Two different wire subjects produced the same adapter digest.
    #[error("Sumeragi v2 block-subject digest collision")]
    SubjectCollision,
    /// A reducer manifest had no canonical wire representation.
    #[error("missing canonical Sumeragi v2 payload manifest")]
    MissingManifest,
    /// One round and subject were associated with two different manifests.
    #[error("conflicting canonical Sumeragi v2 payload manifests for one round and subject")]
    ConflictingManifest,
    /// A certificate reference could not be expanded to the full canonical QC.
    #[error("missing canonical Sumeragi v2 quorum certificate")]
    MissingCertificate,
    /// No fsynced deterministic execution result exists for a signable vote or QC.
    #[error("missing validated Sumeragi v2 execution commitment")]
    MissingExecutionCommitment,
    /// One immutable subject was bound to different execution results.
    #[error("conflicting Sumeragi v2 execution commitments for one immutable subject")]
    ConflictingExecutionCommitment,
    /// A proposal justification was structurally inconsistent.
    #[error("inconsistent Sumeragi v2 proposal justification")]
    InvalidProposalJustification,
    /// BLS aggregation failed for a locally formed certificate.
    #[error("failed to aggregate Sumeragi v2 signatures: {0}")]
    SignatureAggregation(String),
    /// Authenticated ingress rejected a signature, key, or proof of possession.
    #[error("Sumeragi v2 authenticated ingress rejected cryptography: {0}")]
    Cryptography(String),
    /// Proofs of possession were not aligned with the frozen voting roster.
    #[error(
        "Sumeragi v2 proof-of-possession count {actual} does not match roster length {expected}"
    )]
    ProofOfPossessionCount {
        /// Frozen voting-roster length.
        expected: usize,
        /// Supplied proof count.
        actual: usize,
    },
    /// A transport-only canonical payload was incorrectly routed to the reducer.
    #[error("Sumeragi v2 transport payload is not a reducer input")]
    TransportPayload,
    /// Trusted completion ownership exceeded the bounded deferred lane.
    #[error("Sumeragi v2 deferred completion lane exceeded its bounded capacity")]
    DeferredCompletionCapacityExceeded,
    /// The actor-global deferred admission ordinal cannot advance without
    /// wrapping and potentially aliasing a stale owner.
    #[error("Sumeragi v2 deferred admission ordinal space is exhausted")]
    DeferredAdmissionOrdinalExhausted,
    /// The actor-global deferred ordinal source was poisoned by a failed local
    /// owner and can no longer mint trustworthy capabilities.
    #[error("Sumeragi v2 deferred admission ordinal source is unavailable")]
    DeferredAdmissionOrdinalSourceUnavailable,
    /// Exact deferred service debt could not advance without wrapping.
    #[error("Sumeragi v2 deferred service debt overflowed")]
    DeferredServiceDebtOverflow,
    /// One adapter invocation violated the reviewed reducer/continuation
    /// composition contract. This is an internal source-refinement failure,
    /// never recoverable input backpressure.
    #[error(
        "Sumeragi v2 adapter macro-step exceeded its reviewed shape: initial {initial_effects}/{maximum_initial_effects}, Persist {persist_effects}/1, continuation {continuation_effects}/{maximum_continuation_effects}, flattened maximum {maximum_flattened_effects}, nested Persist {continuation_contains_persist}"
    )]
    AdapterMacroStepBoundExceeded {
        /// Effects emitted by the source reducer transition.
        initial_effects: usize,
        /// Record-specific maximum source-transition effects.
        maximum_initial_effects: usize,
        /// Number of `Persist` effects in the source transition.
        persist_effects: usize,
        /// Effects emitted by the synchronous `Persisted` continuation.
        continuation_effects: usize,
        /// Record-specific maximum continuation effects.
        maximum_continuation_effects: usize,
        /// Record-specific maximum flattened effects.
        maximum_flattened_effects: usize,
        /// Whether the acknowledgement attempted a second persistence hop.
        continuation_contains_persist: bool,
    },
    /// The adapter reported deferred work as serviceable, but the reducer
    /// still rejected that exact transition as Busy. Requeueing here would
    /// create a non-decreasing serialized-runtime spin.
    #[error("Sumeragi v2 deferred service violated its open-fence contract and is fail-closed")]
    DeferredServiceContractViolation,
    /// The selected deferred occurrence did not retain the exact actor source,
    /// semantic projection, or single-use capability.
    #[error("Sumeragi v2 deferred service ownership token is invalid or already consumed")]
    DeferredServiceOwnershipViolation,
    /// The serialized runtime lost, altered, or misattached the fair-ingress
    /// carrier for an authenticated adapter command.
    #[error("Sumeragi v2 authenticated runtime ingress ownership is invalid")]
    RuntimeIngressOwnershipViolation,
    /// The reducer is permanently closed after a durability failure.
    #[error("Sumeragi v2 adapter is fail-closed after a durability failure")]
    FailClosed,
    /// The caller attempted network ingress before recovery completed.
    #[error("Sumeragi v2 network ingress is closed until WAL replay completes")]
    ReplayNotComplete,
}

/// Production wrapper around the sole executable Sumeragi v2 reducer.
pub(crate) struct SumeragiV2Adapter {
    wire_context: wire::HeightContext,
    proofs_of_possession: Vec<Vec<u8>>,
    parent_verification: Option<ParentVerificationContext>,
    reducer: reducer::Reducer,
    wal: SafetyWal,
    serviced_candidate_store: ServicedCandidateStore,
    /// Process-generation coalescing markers. This superset includes durable
    /// terminal retirements restored from the adjacent snapshot and volatile
    /// successful services whose reducer state must be rebuilt after restart.
    serviced_candidates: BTreeMap<ServicedCandidateKey, wire::View>,
    /// Restart-stable subset of `serviced_candidates`.
    ///
    /// Only a drained, terminally discarded lifecycle enters this map.
    /// Ordinary successful proposal/vote/body service can leave quorum or
    /// pipeline state only in memory, so persisting its marker would suppress
    /// the retransmission needed to reconstruct that state after a crash.
    durable_serviced_candidates: BTreeMap<ServicedCandidateKey, wire::View>,
    /// Source-derived bound frozen with the adjacent durable store.
    serviced_candidate_capacity: usize,
    /// Process-local exact producer ownership, including live reservations.
    producer_continuations: BTreeMap<ProducerContinuationAddress, ProducerContinuationRecord>,
    /// Restart-safe producer lifecycle metadata published in the same atomic
    /// snapshot. Active records preserve their exact slot and ordinal, then
    /// reopen as `Reserved`; only terminal records suppress replay.
    durable_producer_continuations:
        BTreeMap<ProducerContinuationAddress, ProducerContinuationRecord>,
    /// Active records restored before this process reconstructed their exact
    /// runtime owner. The first matching retry must reuse the persisted causal
    /// key and first-admission ordinal; this set is only the process-local
    /// unclaimed marker and never authorizes identity replacement.
    restored_dormant_producer_continuations: BTreeSet<ProducerContinuationAddress>,
    /// Largest validated admission ordinal present in the snapshot as opened.
    ///
    /// Reclamation may immediately remove an older terminal record, so the
    /// runner must seed its actor-global source from this immutable opening
    /// watermark rather than recomputing it from the post-replay table.
    restored_producer_continuation_ordinal_high_watermark: Option<u128>,
    /// Number of bounded lifecycle slots frozen from runtime capacity geometry.
    producer_continuation_lifecycle_capacity: u64,
    /// Runtime-selected lifecycle being serviced by the next adapter step.
    selected_producer_lifecycle: Option<SelectedProducerLifecycle>,
    /// Busy-deferred adapter ordinal to its complete speculative reservation.
    deferred_producer_continuations: BTreeMap<u128, ProducerReservationToken>,
    /// Exact reservations returned across the runtime ownership cut but not
    /// yet acknowledged by a concrete successor or durable terminal.
    pending_producer_handoffs: BTreeMap<ProducerContinuationAddress, PendingProducerHandoff>,
    serviced_candidates_decision_reclaimed: bool,
    registry: WireRegistry,
    fingerprints: AdapterFingerprints,
    aggregator: Box<dyn SignatureAggregator>,
    active_subject: Option<(reducer::Round, reducer::Subject)>,
    pending_persistence_id: Option<u64>,
    ingress_equivocations: BTreeMap<IngressSemanticKey, IngressEquivocationRecord>,
    ingress_deliveries: BTreeMap<IngressSemanticKey, IngressDeliveryRecord>,
    deferred_completions: VecDeque<DeferredInput>,
    deferred_progress_inputs: VecDeque<DeferredInput>,
    deferred_inputs: VecDeque<DeferredInput>,
    deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    next_deferred_priority: DeferredPriority,
    ignore_counts: BTreeMap<reducer::IgnoreReason, u64>,
    last_progress: Option<(
        reducer::Generation,
        reducer::Round,
        wire::SumeragiV2ProgressTransition,
    )>,
    replay_complete: bool,
    fail_closed: bool,
}

fn commit_qc_status(
    certificate: &wire::QuorumCertificate,
    context: &wire::HeightContext,
) -> Result<wire::SumeragiV2CommitQcStatus, AdapterError> {
    if certificate.phase != wire::GlobalPhase::Commit
        || certificate.round.context_id != context.id()
        || certificate.round.height != context.height
    {
        return Err(AdapterError::DurableCommitMismatch);
    }
    certificate.validate(context)?;
    let signer_count = u32::try_from(certificate.signers.len())
        .map_err(|_| wire::ValidationError::TooManySigners)?;
    let signed_power = certificate.signers.iter().try_fold(
        0_u64,
        |total, signer| -> Result<u64, AdapterError> {
            let index = usize::try_from(*signer)
                .map_err(|_| AdapterError::ValidatorIndexOutOfRange(*signer))?;
            let power = context
                .roster
                .get(index)
                .ok_or(AdapterError::ValidatorIndexOutOfRange(*signer))?
                .power;
            total
                .checked_add(power)
                .ok_or_else(|| wire::ValidationError::VotingPowerOverflow.into())
        },
    )?;
    let validator_count =
        u32::try_from(context.roster.len()).map_err(|_| wire::ValidationError::RosterTooLarge)?;
    Ok(wire::SumeragiV2CommitQcStatus {
        certificate: certificate.as_ref(),
        validator_count,
        signer_count,
        min_signers: context.quorum.min_signers,
        signed_power,
        total_power: context.quorum.total_power,
    })
}

impl SumeragiV2Adapter {
    /// Open the safety WAL, replay every complete frame, and resume durable work.
    ///
    /// Network ingress is never exposed before replay has completed.  The
    /// returned startup effects may re-sign an already durable intent or fetch
    /// and apply an already durable decision.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
            deferred_admission_ordinals,
        )
    }

    /// Open using the already validated runtime/effect ownership geometry.
    ///
    /// The production runner uses this constructor so a configured command
    /// queue larger than the standalone fixture default cannot exhaust the
    /// serviced-identity table while its legitimate lifecycles remain active.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_with_capacity_geometry(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        capacity_geometry: ServicedCandidateCapacityGeometry,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication_with_capacity(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
            true,
            capacity_geometry,
            deferred_admission_ordinals,
        )
    }

    /// Open and replay the adapter without publishing its initial reducer
    /// status.
    ///
    /// The serialized runner uses this only while a finalized predecessor owns
    /// a `Running` successor handoff. It must publish a status snapshot after
    /// every remaining startup constructor succeeds, live clocks are armed,
    /// and authenticated ingress is open. All ordinary callers use [`Self::open`].
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_deferred_status(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
            false,
            deferred_admission_ordinals,
        )
    }

    /// Open with deferred status publication and the validated queue geometry.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_deferred_status_with_capacity_geometry(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        capacity_geometry: ServicedCandidateCapacityGeometry,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication_with_capacity(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
            false,
            capacity_geometry,
            deferred_admission_ordinals,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn open_with_aggregator(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        aggregator: Box<dyn SignatureAggregator>,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            aggregator,
            true,
            deferred_admission_ordinals,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn open_with_aggregator_and_publication(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        aggregator: Box<dyn SignatureAggregator>,
        publish_initial_status: bool,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator_and_publication_with_capacity(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            aggregator,
            publish_initial_status,
            DEFAULT_SERVICED_CANDIDATE_CAPACITY_GEOMETRY,
            deferred_admission_ordinals,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn open_with_aggregator_and_publication_with_capacity(
        wal_path: impl Into<PathBuf>,
        verified_context: VerifiedHeightContext,
        local_validator: Option<wire::ValidatorIndex>,
        generation: reducer::Generation,
        consensus_key_hash: [u8; 32],
        fingerprints: AdapterFingerprints,
        aggregator: Box<dyn SignatureAggregator>,
        publish_initial_status: bool,
        capacity_geometry: ServicedCandidateCapacityGeometry,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        let VerifiedHeightContext {
            context: wire_context,
            proofs_of_possession,
            parent_verification,
        } = verified_context;
        let wal_path = wal_path.into();
        let mut registry = WireRegistry::new(&wire_context)?;
        let context = registry.core_context(&wire_context)?;
        let local_validator = local_validator
            .map(|index| registry.validator_id(index))
            .transpose()?;
        let chain_hash: [u8; 32] = Hash::new(wire_context.chain_id.encode()).into();
        let serviced_candidate_owner: [u8; 32] = fingerprints.node.into();
        let candidate_lifecycle_capacity =
            candidate_lifecycle_capacity(wire_context.roster.len(), capacity_geometry);
        let serviced_candidate_capacity = candidate_lifecycle_capacity
            .checked_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
            .ok_or_else(|| {
                AdapterError::ServicedCandidateStore(
                    "serviced-candidate lifecycle-stage capacity overflowed".to_owned(),
                )
            })?;
        let producer_continuation_lifecycle_capacity = u64::try_from(candidate_lifecycle_capacity)
            .map_err(|_| {
                AdapterError::ServicedCandidateStore(
                    "producer-continuation lifecycle capacity is not representable".to_owned(),
                )
            })?;
        let (serviced_candidate_store, restored_serviced_candidates) =
            ServicedCandidateStore::open(
                &wal_path,
                wire_context.id(),
                wire_context.height,
                serviced_candidate_owner,
                candidate_lifecycle_capacity,
            )
            .map_err(AdapterError::ServicedCandidateStore)?;
        let wal = SafetyWal::open(
            wal_path,
            wire::PROTOCOL_VERSION,
            chain_hash,
            consensus_key_hash,
        )?;

        let entries = wal
            .recovered_records()
            .iter()
            .map(|record| {
                registry.decode_wal_entry(
                    record.sequence,
                    &record.payload,
                    parent_verification.as_ref(),
                    &proofs_of_possession,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let reducer = reducer::Reducer::recover(context, local_validator, generation, entries)?;
        if let Some(decision) = reducer.durable_state().decision() {
            // A stable certificate reference intentionally excludes the exact
            // signer quorum and aggregate. Reconstruct from the durable core
            // value rather than consulting the reference-resolution cache: a
            // later WAL record may carry another valid certificate for the
            // same reference, but it cannot authenticate the exact Decision
            // which replay retained.
            let certificate = registry.qc_to_wire(decision, aggregator.as_ref())?;
            // WAL framing detects torn or accidentally corrupted bytes, but it is not an
            // authority proof. Reauthenticate the exact replayed CommitQC before the reducer may
            // emit its recovery Apply effect. This also rejects a locally rewritten, perfectly
            // checksummed WAL whose QC was never signed by the frozen quorum.
            verify_quorum_certificate(&wire_context, &certificate, &proofs_of_possession)?;
        }
        // Recovery must expose the body/application pipeline in its very first
        // status snapshot. A durable decision owns the pipeline in preference
        // to the (possibly still retained) Prepare lock; otherwise the lock is
        // the active body which must remain recoverable while lifecycle views
        // advance before the exact body is re-proposed in a later round.
        let active_subject = reducer
            .durable_state()
            .decision()
            .or_else(|| reducer.durable_state().locked())
            .map(|certificate| (certificate.proposal_round(), certificate.subject()));
        let restored_records = restored_serviced_candidates.records;
        let restored_producer_continuations = restored_serviced_candidates.producer_continuations;
        let restored_dormant_producer_continuations = restored_producer_continuations
            .iter()
            .filter_map(|(address, record)| {
                (record.status() != ProducerContinuationStatus::Terminal).then_some(*address)
            })
            .collect();
        let restored_producer_continuation_ordinal_high_watermark = restored_producer_continuations
            .values()
            .map(|record| record.identity().admission_ordinal())
            .max();
        let mut adapter = Self {
            wire_context,
            proofs_of_possession,
            parent_verification,
            reducer,
            wal,
            serviced_candidate_store,
            serviced_candidates: restored_records.clone(),
            durable_serviced_candidates: restored_records,
            serviced_candidate_capacity,
            producer_continuations: restored_producer_continuations.clone(),
            durable_producer_continuations: restored_producer_continuations,
            restored_dormant_producer_continuations,
            restored_producer_continuation_ordinal_high_watermark,
            producer_continuation_lifecycle_capacity,
            selected_producer_lifecycle: None,
            deferred_producer_continuations: BTreeMap::new(),
            pending_producer_handoffs: BTreeMap::new(),
            serviced_candidates_decision_reclaimed: restored_serviced_candidates.decision_reclaimed,
            registry,
            fingerprints,
            aggregator,
            active_subject,
            pending_persistence_id: None,
            ingress_equivocations: BTreeMap::new(),
            ingress_deliveries: BTreeMap::new(),
            deferred_completions: VecDeque::new(),
            deferred_progress_inputs: VecDeque::new(),
            deferred_inputs: VecDeque::new(),
            deferred_admission_ordinals,
            next_deferred_priority: DeferredPriority::Completion,
            ignore_counts: BTreeMap::new(),
            last_progress: None,
            replay_complete: false,
            fail_closed: false,
        };
        adapter.reclaim_serviced_candidates()?;
        let replay_tag = adapter.reducer.current_tag();
        let replay_event = reducer::Event::ResumeAfterReplay { tag: replay_tag };
        let replay = adapter.reducer.step(replay_event.clone())?;
        adapter.record_reducer_outcome(&replay_event, replay.disposition(), replay.effects());
        let startup = replay.into_effects();
        let startup = adapter.drive_effects(startup)?;
        adapter.replay_complete = true;
        if publish_initial_status {
            adapter.publish_status()?;
        }
        Ok((adapter, startup))
    }

    /// Return the tag which must accompany a new asynchronous operation.
    pub(crate) const fn current_tag(&self) -> reducer::EventTag {
        self.reducer.current_tag()
    }

    /// Actor-global ordinal source shared with every replacement height
    /// adapter owned by this runtime actor.
    pub(crate) const fn deferred_admission_ordinal_source(
        &self,
    ) -> &DeferredAdmissionOrdinalSource {
        &self.deferred_admission_ordinals
    }

    /// Largest producer lifecycle ordinal validated while opening this height.
    ///
    /// This remains the opening value even if strict-view or Decision
    /// reclamation removes the corresponding tombstone before the serialized
    /// runtime is constructed.
    pub(crate) const fn restored_producer_continuation_ordinal_high_watermark(
        &self,
    ) -> Option<u128> {
        self.restored_producer_continuation_ordinal_high_watermark
    }

    /// Resolve one restart-dormant deterministic runtime root by its exact
    /// persisted causal key. Every stage in one lifecycle must agree on the
    /// immutable first-admission ordinal.
    pub(crate) fn dormant_producer_lifecycle(
        &self,
        causal_lifecycle_key: &Hash,
    ) -> super::v2_runtime::RuntimeDormantProducerLifecycle {
        use super::v2_runtime::RuntimeDormantProducerLifecycle as Dormant;

        let mut admission_ordinal = None;
        for address in &self.restored_dormant_producer_continuations {
            let Some(record) = self.producer_continuations.get(address) else {
                return Dormant::Conflict;
            };
            if record.identity().causal_lifecycle_key() != *causal_lifecycle_key {
                continue;
            }
            if record.status() != ProducerContinuationStatus::Reserved
                || self.durable_producer_continuations.get(address) != Some(record)
            {
                return Dormant::Conflict;
            }
            let candidate = record.identity().admission_ordinal();
            match admission_ordinal {
                Some(existing) if existing != candidate => return Dormant::Conflict,
                Some(_) => {}
                None => admission_ordinal = Some(candidate),
            }
        }
        admission_ordinal.map_or(Dormant::Absent, |admission_ordinal| Dormant::Exact {
            admission_ordinal,
        })
    }

    /// Return the restart-dormant Local stages which already reserve a
    /// completion-FIFO position.
    ///
    /// Timeout replay remains a non-FIFO clock root. Authenticated transport
    /// and pre-store reconstructed-body work retain their separate physical
    /// owners, so neither class consumes a local FIFO reservation here.
    pub(crate) fn dormant_local_fifo_reservations(
        &self,
    ) -> Result<Vec<super::v2_runtime::RuntimeDormantLocalFifoReservation>, String> {
        let expected_dormant = self
            .producer_continuations
            .iter()
            .filter_map(|(address, record)| {
                (record.status() != ProducerContinuationStatus::Terminal).then_some(*address)
            })
            .collect::<BTreeSet<_>>();
        if expected_dormant != self.restored_dormant_producer_continuations {
            return Err(
                "restart-dormant producer index disagreed with active snapshot records".to_owned(),
            );
        }

        let mut lifecycle_ordinals = BTreeMap::<Hash, u128>::new();
        let mut reservations = BTreeSet::new();
        for address in &self.restored_dormant_producer_continuations {
            let record = self
                .producer_continuations
                .get(address)
                .ok_or_else(|| "restart-dormant producer record was missing".to_owned())?;
            if record.status() != ProducerContinuationStatus::Reserved
                || record.identity().address() != *address
                || self.durable_producer_continuations.get(address) != Some(record)
            {
                return Err(
                    "restart-dormant producer record was not exact durable Reserved metadata"
                        .to_owned(),
                );
            }
            let identity = record.identity();
            let lifecycle_key = identity.causal_lifecycle_key();
            let admission_ordinal = identity.admission_ordinal();
            match lifecycle_ordinals.insert(lifecycle_key, admission_ordinal) {
                Some(existing) if existing != admission_ordinal => {
                    return Err(
                        "restart-dormant producer lifecycle changed its immutable ordinal"
                            .to_owned(),
                    );
                }
                Some(_) | None => {}
            }
            let stage = ServicedCandidateStage::from_code(identity.stage()).ok_or_else(|| {
                "restart-dormant producer carried an unknown service stage".to_owned()
            })?;
            let expected_source = producer_parent_replay_source_for_stage(stage);
            let source_exact = matches!(
                (expected_source, record.source_class()),
                (
                    ProducerParentReplaySource::ConditionalResponsiveTransport,
                    ProducerContinuationSourceClass::ConditionalTransport
                ) | (
                    ProducerParentReplaySource::VolatileBodyReconstruction,
                    ProducerContinuationSourceClass::VolatileBody
                ) | (
                    ProducerParentReplaySource::DurableBodyPipeline
                        | ProducerParentReplaySource::SafetyWal
                        | ProducerParentReplaySource::DurableDecision,
                    ProducerContinuationSourceClass::Local
                )
            );
            if !source_exact {
                return Err("restart-dormant producer changed its physical replay class".to_owned());
            }
            if matches!(
                stage,
                ServicedCandidateStage::LocalProposalReady
                    | ServicedCandidateStage::BodyStored
                    | ServicedCandidateStage::ValidationCompleted
                    | ServicedCandidateStage::ApplicationCompleted
            ) && !reservations.insert(
                super::v2_runtime::RuntimeDormantLocalFifoReservation::completion(
                    lifecycle_key,
                    admission_ordinal,
                    identity.stage(),
                ),
            ) {
                return Err(
                    "restart-dormant Local producer duplicated one FIFO reservation".to_owned(),
                );
            }
        }
        Ok(reservations.into_iter().collect())
    }

    /// Snapshot the exact reducer-owned facts which constrain local proposal
    /// construction. Proposal justification remains internal to the reducer.
    pub(crate) fn local_proposal_directive(&self) -> Result<LocalProposalDirective, AdapterError> {
        let durable = self.reducer.durable_state();
        let view = durable.current_view();
        let leader = self
            .registry
            .validator_index(self.reducer.context().leader(view))?;
        let locked = durable.locked();
        let locked_round =
            locked.map(|certificate| self.registry.round_to_wire(certificate.round()));
        let locked_subject = locked
            .map(|certificate| self.registry.subject(certificate.subject()))
            .transpose()?;
        let decided_subject = durable
            .decision()
            .map(|certificate| self.registry.subject(certificate.subject()))
            .transpose()?;
        Ok(LocalProposalDirective {
            tag: self.reducer.current_tag(),
            leader,
            locked_round,
            locked_subject,
            decided_subject,
        })
    }

    /// Return the exact Decision key reconstructed from complete WAL frames.
    ///
    /// Startup uses this before ingress opens to bind an interrupted canonical
    /// Kura tip to the reducer Decision and the exact durable body marker. A
    /// missing value means WAL replay contains no durable CommitQC decision.
    pub(crate) fn replayed_decision_key(
        &self,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        AdapterError,
    > {
        self.reducer
            .durable_state()
            .decision()
            .map(|certificate| {
                Ok((
                    self.registry.round_to_wire(certificate.round()),
                    self.registry.round_to_wire(certificate.proposal_round()),
                    self.registry.subject(certificate.subject())?,
                    self.registry.execution_commitment(
                        certificate.proposal_round(),
                        certificate.subject(),
                    )?,
                ))
            })
            .transpose()
    }

    /// Return whether WAL replay completed and authenticated ingress may open.
    pub(crate) const fn ingress_ready(&self) -> bool {
        self.replay_complete && !self.fail_closed
    }

    /// Return whether application completed and no unfinished safety write,
    /// signature, or adapter-owned deferred input remains before height
    /// rollover.
    pub(crate) fn ready_to_finish(&self) -> bool {
        self.ingress_ready()
            && self.deferred_completions.is_empty()
            && self.deferred_progress_inputs.is_empty()
            && self.deferred_inputs.is_empty()
            && self.reducer.ready_to_finish()
    }

    /// Verify a canonical consensus message against this adapter's frozen
    /// roster and prevalidated proofs of possession.
    pub(crate) fn authenticate(
        &self,
        message: wire::ConsensusMessageV2,
    ) -> Result<AuthenticatedConsensusMessage, AdapterError> {
        self.ensure_ingress()?;
        verify_authenticated_message(
            &self.wire_context,
            self.parent_verification.as_ref(),
            &message,
            &self.proofs_of_possession,
        )?;
        let authenticated = AuthenticatedConsensusMessage(message);
        self.ensure_authenticated_manifest_compatible(&authenticated)?;
        self.ensure_authenticated_execution_commitments_compatible(&authenticated)?;
        Ok(authenticated)
    }

    /// Return whether authenticated ingress belongs to the active lock's
    /// reserved progress path.
    ///
    /// QCs and TCs have their own progress classification at the runtime
    /// boundary. This predicate is deliberately narrower: only an exact
    /// Commit vote for an undecided durable lock may bypass normal ingress
    /// capacity.
    pub(crate) fn authenticated_ingress_is_progress(
        &self,
        message: &AuthenticatedConsensusMessage,
    ) -> bool {
        self.wire_ingress_may_use_progress(message.payload())
    }

    /// Return the tag of an exact Commit/Prepare QC already owned by the
    /// adapter's Busy-deferred progress lane.
    ///
    /// This comparison is intentionally exact, including canonical signer
    /// order and aggregate signature. Runtime admission may use the result as
    /// a capacity hint, but it must independently authenticate the arriving
    /// envelope before coalescing it with this owner.
    pub(crate) fn deferred_quorum_certificate_owner_tag(
        &self,
        candidate: &wire::QuorumCertificate,
    ) -> Option<reducer::EventTag> {
        self.deferred_quorum_certificate_owner(candidate)
            .map(|(tag, _)| tag)
    }

    /// Return the tag and actor-global admission ordinal of an exact
    /// Commit/Prepare QC already owned by the Busy-deferred progress lane.
    ///
    /// The ordinal is an opaque process-local association key. It lets the
    /// serialized runtime merge later authenticated-source routes into the
    /// exact deferred occurrence without exposing or reconstructing the
    /// adapter's reducer event.
    pub(crate) fn deferred_quorum_certificate_owner(
        &self,
        candidate: &wire::QuorumCertificate,
    ) -> Option<(reducer::EventTag, u128)> {
        self.deferred_progress_inputs.iter().find_map(|input| {
            let reducer::Event::QuorumCertificateReceived { tag, certificate } = &input.event
            else {
                return None;
            };
            self.registry
                .reducer_qc_matches_wire(certificate, candidate)
                .then_some((*tag, input.admission_ordinal))
        })
    }

    /// Return the tag and actor-global admission ordinal of an exact canonical
    /// authenticated envelope already owned by a Busy-deferred lane.
    ///
    /// The ordinal is an opaque process-local association key. It lets the
    /// serialized runtime merge later authenticated-source routes into the
    /// exact deferred occurrence without exposing or reconstructing the
    /// adapter's reducer event. This raw-byte comparison is only a capacity
    /// hint; runtime admission repeats it after authenticating the candidate.
    pub(crate) fn deferred_authenticated_message_owner(
        &self,
        candidate: &wire::ConsensusMessageV2,
    ) -> Option<(reducer::EventTag, u128)> {
        if let wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) = &candidate.payload
        {
            return self.deferred_quorum_certificate_owner(certificate);
        }
        let encoded = candidate.encode();
        self.deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .find_map(|input| {
                input
                    .authenticated_wire_identity
                    .as_deref()
                    .is_some_and(|owned| owned == encoded.as_slice())
                    .then_some((deferred_event_tag(&input.event), input.admission_ordinal))
            })
    }

    /// Exact actor-global ordinals currently retained by authenticated
    /// Busy-deferred inputs across all service classes.
    ///
    /// The serialized runtime uses this snapshot only to retire carriers for
    /// inputs which a legitimate adapter transition superseded and to reject
    /// any newly active deferred input that lacks its original carrier.
    pub(crate) fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
        self.deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .filter(|input| input.retag_authenticated_ingress)
            .map(|input| input.admission_ordinal)
            .collect()
    }

    /// Exact actor-global ordinals retained by every Busy-deferred input.
    /// Runtime lifecycle ownership uses this complete set; the authenticated
    /// subset above remains the separate fair-ingress carrier authority.
    pub(crate) fn all_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
        self.deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .map(|input| input.admission_ordinal)
            .collect()
    }

    /// Return whether a wire payload may use the active lock's progress lane.
    ///
    /// This is only a pre-authentication capacity hint. Callers must still
    /// authenticate the envelope and use [`Self::authenticated_ingress_is_progress`]
    /// as the security gate before enqueueing it as progress traffic.
    pub(crate) fn wire_ingress_may_use_progress(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        matches!(
            payload,
            wire::ConsensusMessageV2Payload::Vote(vote)
                if self.is_exact_locked_commit_vote(vote)
        )
    }

    /// Return the body identity whose direct vote lacks a locally validated
    /// execution commitment.
    ///
    /// This is a non-authenticating dequeue hint, but it still applies every
    /// cheap structural check before retaining fair-ingress ownership.
    /// Malformed votes and invalid or conflicting commitments return `None`
    /// so the mutating admission seam can reject them instead of allowing a
    /// far-future malformed occurrence to pin one source lane.
    pub(crate) fn wire_ingress_missing_execution_commitment(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> Option<(wire::ConsensusRound, wire::BlockSubject)> {
        let wire::ConsensusMessageV2Payload::Vote(vote) = payload else {
            return None;
        };
        if vote.validate(&self.wire_context).is_err() {
            return None;
        }
        matches!(
            self.ensure_vote_execution_commitment_bound(
                vote.proposal_round,
                vote.subject,
                vote.execution_commitment,
            ),
            Err(AdapterError::MissingExecutionCommitment)
        )
        .then_some((vote.proposal_round, vote.subject))
    }

    fn ensure_authenticated_manifest_compatible(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> Result<(), AdapterError> {
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = authenticated.payload() else {
            return Ok(());
        };
        if self.registry.manifest_conflicts(&proposal.manifest) {
            return Err(AdapterError::ConflictingManifest);
        }
        Ok(())
    }

    fn ensure_authenticated_execution_commitments_compatible(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> Result<(), AdapterError> {
        let mut observed = Vec::new();
        match authenticated.payload() {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => match &proposal.justification {
                wire::ProposalJustification::ParentCommit(parent) => {
                    if let Some(certificate) = &parent.certificate {
                        self.ensure_qc_execution_commitment_compatible(certificate, &mut observed)?;
                    }
                }
                wire::ProposalJustification::Timeout(timeout) => {
                    self.ensure_tc_execution_commitments_compatible(
                        &timeout.timeout_certificate,
                        &mut observed,
                    )?;
                    if let Some(certificate) = &timeout.highest_prepare_qc {
                        self.ensure_qc_execution_commitment_compatible(certificate, &mut observed)?;
                    }
                }
            },
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                self.ensure_vote_execution_commitment_bound(
                    vote.proposal_round,
                    vote.subject,
                    vote.execution_commitment,
                )?;
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                self.ensure_qc_execution_commitment_compatible(certificate, &mut observed)?;
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                if let Some(certificate) = &vote.highest_prepare_qc {
                    self.ensure_qc_execution_commitment_compatible(certificate, &mut observed)?;
                }
            }
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                self.ensure_tc_execution_commitments_compatible(certificate, &mut observed)?;
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
                self.ensure_qc_execution_commitment_compatible(
                    &request.certificate,
                    &mut observed,
                )?;
            }
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
                self.ensure_qc_execution_commitment_compatible(
                    &response.certificate,
                    &mut observed,
                )?;
            }
            wire::ConsensusMessageV2Payload::PayloadManifest(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::VrfCommit(_)
            | wire::ConsensusMessageV2Payload::VrfReveal(_) => {}
        }
        Ok(())
    }

    fn ensure_vote_execution_commitment_bound(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        commitment: wire::ExecutionCommitment,
    ) -> Result<(), AdapterError> {
        commitment.validate()?;
        let core_round = reducer::Round::new(round.height, round.view);
        let core_subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        if self
            .registry
            .subjects
            .get(&core_subject)
            .is_some_and(|registered| *registered != subject)
        {
            return Err(AdapterError::SubjectCollision);
        }
        if self.registry.execution_commitments.iter().any(
            |((_, registered_subject), registered)| {
                *registered_subject == core_subject && *registered != commitment
            },
        ) {
            return Err(AdapterError::ConflictingExecutionCommitment);
        }
        match self
            .registry
            .execution_commitments
            .get(&(core_round, core_subject))
        {
            Some(registered) if *registered == commitment => Ok(()),
            Some(_) => Err(AdapterError::ConflictingExecutionCommitment),
            None => Err(AdapterError::MissingExecutionCommitment),
        }
    }

    fn ensure_tc_execution_commitments_compatible(
        &self,
        certificate: &wire::TimeoutCertificate,
        observed: &mut Vec<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    ) -> Result<(), AdapterError> {
        for group in &certificate.groups {
            if let Some(highest) = &group.highest_prepare_qc {
                self.ensure_qc_execution_commitment_compatible(highest, observed)?;
            }
        }
        Ok(())
    }

    fn ensure_qc_execution_commitment_compatible(
        &self,
        certificate: &wire::QuorumCertificate,
        observed: &mut Vec<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    ) -> Result<(), AdapterError> {
        self.ensure_execution_commitment_compatible(
            certificate.proposal_round,
            certificate.subject,
            certificate.execution_commitment,
            observed,
        )
    }

    fn ensure_execution_commitment_compatible(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        commitment: wire::ExecutionCommitment,
        observed: &mut Vec<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    ) -> Result<(), AdapterError> {
        commitment.validate()?;
        let core_subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        if self
            .registry
            .subjects
            .get(&core_subject)
            .is_some_and(|registered| *registered != subject)
        {
            return Err(AdapterError::SubjectCollision);
        }
        if self.registry.execution_commitments.iter().any(
            |((_, registered_subject), registered)| {
                *registered_subject == core_subject && *registered != commitment
            },
        ) || observed.iter().any(|(_, registered_subject, registered)| {
            *registered_subject == subject && *registered != commitment
        }) {
            return Err(AdapterError::ConflictingExecutionCommitment);
        }
        observed.push((round, subject, commitment));
        Ok(())
    }

    fn is_exact_locked_commit_vote(&self, vote: &wire::Vote) -> bool {
        if vote.phase != wire::GlobalPhase::Commit {
            return false;
        }
        let durable = self.reducer.durable_state();
        // A retained Prepare lock stops being a reconstruction witness once a
        // CommitQC is durable. Post-decision votes therefore use ordinary,
        // height-long duplicate admission and cannot repeatedly consume the
        // generation-scoped protected lane while application is pending.
        if durable.decision().is_some() {
            return false;
        }
        let Some(locked) = durable.locked() else {
            return false;
        };
        vote.proposal_round.height == locked.round().height()
            && vote.proposal_round.view == locked.round().view()
            && vote.round == vote.proposal_round
            && self
                .registry
                .subject(locked.subject())
                .is_ok_and(|subject| subject == vote.subject)
            && self
                .registry
                .execution_commitment(locked.round(), locked.subject())
                .is_ok_and(|commitment| commitment == vote.execution_commitment)
    }

    fn deferred_owns_ingress(
        &self,
        key: IngressSemanticKey,
        fingerprint: IngressFingerprint,
    ) -> bool {
        // A certified EnterView may advance the reducer generation before its executor returns
        // control to the deferred runner. The old tagged event remains the sole owner during
        // that boundary, so an exact retransmission must coalesce instead of claiming the new
        // generation's protected slot.
        self.deferred_completions
            .iter()
            .chain(&self.deferred_progress_inputs)
            .chain(&self.deferred_inputs)
            .any(|input| {
                input.admission.is_some_and(|admission| {
                    admission.key == key && admission.fingerprint == fingerprint
                })
            })
    }

    #[allow(clippy::too_many_lines)]
    fn admit_authenticated_payload(
        &mut self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> Result<(Option<AdapterOutcome>, Option<IngressAdmission>), AdapterError> {
        let current_tag = self.reducer.current_tag();
        let current_view = current_tag.view();
        let generation = current_tag.generation();
        self.prune_ingress_records();
        let retained_vote_views = u64::try_from(self.wire_context.roster.len()).unwrap_or(u64::MAX);
        let oldest_retained_view = current_view.saturating_sub(retained_vote_views);
        // Retain arbitrary individual Commit/Prepare vote keys for one complete
        // leader rotation. Older CommitQCs remain admissible without
        // restriction. Exact durable locked-round CommitVotes are the sole old
        // individual-vote exception while the height is undecided: timeout
        // installation clears their volatile reducer pool, while replay keeps
        // retransmitting the durable Commit intent. Their single round/subject
        // cannot exhaust this table.
        let locked_commit_progress = match payload {
            wire::ConsensusMessageV2Payload::Vote(vote) => self.is_exact_locked_commit_vote(vote),
            _ => false,
        };
        let unsafe_proposal = if let wire::ConsensusMessageV2Payload::Proposal(proposal) = payload
            && let Some(locked) = self.reducer.durable_state().locked()
        {
            let locked_round = self.registry.round_to_wire(locked.round());
            let locked_subject = self.registry.subject(locked.subject())?;
            !proposal_is_safe_for_lock(proposal, locked_round, locked_subject)
        } else {
            false
        };
        let (key, fingerprint, round, signer, kind) = match payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                if proposal.round.view != current_view {
                    return Ok((
                        Some(Self::ignored_outcome(reducer::IgnoreReason::IrrelevantView)),
                        None,
                    ));
                }
                (
                    IngressSemanticKey::Proposal {
                        round: proposal.round,
                        proposer: proposal.proposer,
                    },
                    IngressFingerprint::Proposal(Hash::new(proposal.signature_preimage())),
                    proposal.round,
                    proposal.proposer,
                    reducer::EquivocationKind::Proposal,
                )
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                if vote.round.view > current_view
                    || (vote.round.view < oldest_retained_view && !locked_commit_progress)
                {
                    return Ok((
                        Some(Self::ignored_outcome(reducer::IgnoreReason::IrrelevantView)),
                        None,
                    ));
                }
                (
                    IngressSemanticKey::Vote {
                        round: vote.round,
                        phase: vote.phase,
                        signer: vote.signer,
                    },
                    IngressFingerprint::Vote(
                        vote.proposal_round,
                        vote.subject,
                        vote.execution_commitment,
                    ),
                    vote.round,
                    vote.signer,
                    reducer::EquivocationKind::Vote,
                )
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                if vote.round.view != current_view {
                    return Ok((
                        Some(Self::ignored_outcome(reducer::IgnoreReason::IrrelevantView)),
                        None,
                    ));
                }
                (
                    IngressSemanticKey::TimeoutVote {
                        round: vote.round,
                        signer: vote.signer,
                    },
                    IngressFingerprint::TimeoutVote(
                        vote.highest_prepare_qc
                            .as_ref()
                            .map(wire::QuorumCertificate::as_ref),
                    ),
                    vote.round,
                    vote.signer,
                    reducer::EquivocationKind::Timeout,
                )
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(_)
            | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::PayloadManifest(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::VrfCommit(_)
            | wire::ConsensusMessageV2Payload::VrfReveal(_) => {
                return Ok((None, None));
            }
        };
        let deferred_owner = self.deferred_owns_ingress(key, fingerprint);

        if let Some(record) = self.ingress_equivocations.get_mut(&key) {
            if record.fingerprint == fingerprint {
                if deferred_owner
                    || self.ingress_deliveries.get(&key).is_some_and(|delivered| {
                        debug_assert_eq!(delivered.fingerprint, fingerprint);
                        !locked_commit_progress
                            || (delivered.locked_commit_progress
                                && delivered.generation == generation)
                    })
                {
                    return Ok((
                        Some(Self::ignored_outcome(reducer::IgnoreReason::Duplicate)),
                        None,
                    ));
                }
                let admission = IngressAdmission {
                    key,
                    fingerprint,
                    generation,
                    inserted_equivocation: false,
                    locked_commit_progress,
                };
                if unsafe_proposal {
                    self.record_ingress_delivery(admission);
                    return Ok((
                        Some(Self::ignored_outcome(reducer::IgnoreReason::UnsafeProposal)),
                        None,
                    ));
                }
                return Ok((None, Some(admission)));
            }
            if record.equivocation_reported {
                return Ok((
                    Some(Self::ignored_outcome(reducer::IgnoreReason::Duplicate)),
                    None,
                ));
            }
            record.equivocation_reported = true;
            let offender = self
                .wire_context
                .roster
                .get(usize::try_from(signer).unwrap_or(usize::MAX))
                .map(|entry| entry.validator.clone())
                .ok_or(AdapterError::ValidatorIndexOutOfRange(signer))?;
            return Ok((
                Some(AdapterOutcome {
                    disposition: reducer::StepDisposition::Applied,
                    effects: vec![AdapterEffect::ReportEquivocation {
                        offender,
                        round,
                        kind,
                    }],
                    deferred_admission_ordinal: None,
                    producer_handoff: None,
                }),
                None,
            ));
        }

        let capacity_bypass = self.ingress_equivocations.len() >= MAX_INGRESS_SEMANTIC_KEYS;
        let protected_capacity_bypass =
            locked_commit_progress || matches!(key, IngressSemanticKey::TimeoutVote { .. });
        if capacity_bypass && !protected_capacity_bypass {
            // This is bounded backpressure for ordinary semantic traffic. QCs
            // and TCs do not consume this table. The at-most-roster-sized exact
            // locked Commit and current-view TimeoutVote sets bypass ordinary
            // capacity and use their independent reserved progress partitions.
            return Ok((
                Some(Self::ignored_outcome(reducer::IgnoreReason::Busy)),
                None,
            ));
        }
        self.ingress_equivocations.insert(
            key,
            IngressEquivocationRecord {
                fingerprint,
                equivocation_reported: false,
                capacity_bypass,
                admitted_at: Instant::now(),
            },
        );
        let admission = IngressAdmission {
            key,
            fingerprint,
            generation,
            inserted_equivocation: true,
            locked_commit_progress,
        };
        if unsafe_proposal {
            self.record_ingress_delivery(admission);
            return Ok((
                Some(Self::ignored_outcome(reducer::IgnoreReason::UnsafeProposal)),
                None,
            ));
        }
        Ok((None, Some(admission)))
    }

    fn prune_ingress_records(&mut self) {
        let current_view = self.reducer.current_tag().view();
        let current_height = self.wire_context.height;
        let retained_vote_views = u64::try_from(self.wire_context.roster.len()).unwrap_or(u64::MAX);
        let oldest_retained_view = current_view.saturating_sub(retained_vote_views);
        let durable_lock = self.reducer.durable_state().locked().and_then(|locked| {
            Some((
                self.registry.round_to_wire(locked.round()),
                self.registry.subject(locked.subject()).ok()?,
                self.registry
                    .execution_commitment(locked.round(), locked.subject())
                    .ok()?,
            ))
        });
        let matches_current_lock = |key: IngressSemanticKey, fingerprint: IngressFingerprint| {
            matches!(
                (key, fingerprint, durable_lock),
                (
                    IngressSemanticKey::Vote {
                        round,
                        phase: wire::GlobalPhase::Commit,
                        ..
                    },
                    IngressFingerprint::Vote(
                        proposal_round,
                        subject,
                        execution_commitment,
                    ),
                    Some((locked_round, locked_subject, locked_execution_commitment))
                ) if proposal_round == locked_round
                    && round.height == locked_round.height
                    && subject == locked_subject
                    && execution_commitment == locked_execution_commitment
            )
        };
        let matches_current_timeout = |key: IngressSemanticKey| {
            matches!(
                key,
                IngressSemanticKey::TimeoutVote { round, .. }
                    if round.height == current_height && round.view == current_view
            )
        };
        self.ingress_equivocations.retain(|key, record| {
            if record.capacity_bypass {
                matches_current_lock(*key, record.fingerprint) || matches_current_timeout(*key)
            } else {
                key.round().view >= oldest_retained_view
                    || matches_current_lock(*key, record.fingerprint)
            }
        });
        let equivocations = &self.ingress_equivocations;
        self.ingress_deliveries.retain(|key, delivery| {
            equivocations
                .get(key)
                .is_some_and(|record| record.fingerprint == delivery.fingerprint)
        });
    }

    fn ignored_outcome(reason: reducer::IgnoreReason) -> AdapterOutcome {
        AdapterOutcome {
            disposition: reducer::StepDisposition::Ignored(reason),
            effects: Vec::new(),
            deferred_admission_ordinal: None,
            producer_handoff: None,
        }
    }

    /// Feed a signature-checked and structurally verified canonical message.
    fn receive_verified(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        message.validate_version()?;
        let authenticated_wire_identity = Arc::<[u8]>::from(message.encode());
        let (outcome, admission) = self.admit_authenticated_payload(&message.payload)?;
        if let Some(outcome) = outcome {
            self.record_disposition(outcome.disposition());
            self.publish_status()?;
            return Ok(outcome);
        }
        let result =
            self.receive_admitted_payload(message.payload, admission, authenticated_wire_identity);
        if result.is_err()
            && let Some(admission) = admission
            && admission.inserted_equivocation
            && self
                .ingress_equivocations
                .get(&admission.key)
                .is_some_and(|record| record.fingerprint == admission.fingerprint)
        {
            self.ingress_equivocations.remove(&admission.key);
        }
        result
    }

    fn receive_admitted_payload(
        &mut self,
        payload: wire::ConsensusMessageV2Payload,
        admission: Option<IngressAdmission>,
        authenticated_wire_identity: Arc<[u8]>,
    ) -> Result<AdapterOutcome, AdapterError> {
        // Conversion is intentionally staged. A malformed value or a subject
        // collision must not leave attacker-controlled registry entries behind.
        // Semantic admission above bounds values which the reducer may defer.
        let mut registry = self.registry.clone();
        let tag = self.reducer.current_tag();
        match payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                let proposal = registry.proposal_to_core(&proposal, &self.wire_context)?;
                let round = proposal.proposal().round();
                let subject = proposal.proposal().manifest().subject();
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::ProposalReceived { tag, proposal },
                    Some((round, subject)),
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                let vote = registry.vote_to_core(&vote, &self.wire_context)?;
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::VoteReceived { tag, vote },
                    None,
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                let certificate = registry.qc_to_core(&certificate, &self.wire_context)?;
                let active_subject = Some((certificate.proposal_round(), certificate.subject()));
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::QuorumCertificateReceived { tag, certificate },
                    active_subject,
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                let vote = registry.timeout_vote_to_core(&vote, &self.wire_context)?;
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::TimeoutVoteReceived { tag, vote },
                    None,
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                let certificate = registry.tc_to_core(&certificate, &self.wire_context)?;
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::TimeoutCertificateReceived { tag, certificate },
                    None,
                    admission,
                    authenticated_wire_identity,
                );
            }
            wire::ConsensusMessageV2Payload::PayloadManifest(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::VrfCommit(_)
            | wire::ConsensusMessageV2Payload::VrfReveal(_) => {
                return Err(AdapterError::TransportPayload);
            }
        }
    }

    fn dispatch_staged_authenticated_ingress(
        &mut self,
        registry: WireRegistry,
        event: reducer::Event,
        active_subject: Option<(reducer::Round, reducer::Subject)>,
        admission: Option<IngressAdmission>,
        authenticated_wire_identity: Arc<[u8]>,
    ) -> Result<AdapterOutcome, AdapterError> {
        let previous_registry = core::mem::replace(&mut self.registry, registry);
        let previous_active_subject = self.active_subject;
        if let Some(active_subject) = active_subject {
            self.active_subject = Some(active_subject);
        }
        let result = self.step_authenticated_ingress_with_ownership(
            event,
            admission,
            Some(authenticated_wire_identity),
        );
        if result.is_err() {
            // A reducer failure after conversion may have partially consumed an
            // authenticated transition. Keep its registry expansion aligned
            // with reducer state and require WAL replay before further ingress.
            self.fail_closed = true;
            return result.map(|result| result.outcome);
        }
        let retain = result.as_ref().is_ok_and(|result| {
            result.outcome.disposition() == reducer::StepDisposition::Applied
                || (result.outcome.disposition()
                    == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
                    && result.outcome.deferred_admission_ordinal().is_some())
        });
        if !retain {
            self.registry = previous_registry;
            self.active_subject = previous_active_subject;
            self.publish_status()?;
        }
        result.map(|result| result.outcome)
    }

    /// Pass an authenticated canonical envelope to the reducer.
    pub(crate) fn receive_authenticated(
        &mut self,
        message: AuthenticatedConsensusMessage,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.receive_verified(message.0)
    }

    /// Notify the reducer that its one constant round timer expired.
    pub(crate) fn timeout_elapsed(
        &mut self,
        tag: reducer::EventTag,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        self.step(reducer::Event::TimeoutElapsed { tag })
    }

    /// Retry any missing proposal or certified body after the derived
    /// retransmission interval.
    pub(crate) fn retransmit_elapsed(
        &mut self,
        tag: reducer::EventTag,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        self.step(reducer::Event::RetransmitElapsed { tag })
    }

    /// Submit a locally assembled, durably stored, deterministically validated body.
    ///
    /// While the height is undecided, only the expected leader can take this
    /// transition: the reducer first persists its proposal intent and only
    /// then exposes signing. If an exact matching Decision became durable
    /// while assembly was completing, the trusted manifest and execution
    /// commitment instead transfer directly to decided-body application and
    /// the reducer emits `Apply` without creating proposal-only work.
    pub(crate) fn local_proposal_ready(
        &mut self,
        tag: reducer::EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: &DurableBodyReceipt,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        if durable_receipt.context_id() != self.wire_context.id()
            || durable_receipt.round() != manifest.round
            || durable_receipt.subject() != manifest.subject
            || durable_receipt.manifest_hash() != HashOf::new(&manifest)
            || validated_receipt.durable() != durable_receipt
        {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let completion_evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable_receipt.clone(),
            validated_receipt: validated_receipt.clone(),
        };
        // Manifest conversion registers body identity, while execution
        // commitment registration binds the deterministic validation result.
        // Stage both mutations so a conflict cannot install half of the trust
        // boundary and influence a later completion.
        let mut staged_registry = self.registry.clone();
        let core_manifest = staged_registry.manifest_to_core(&manifest, &self.wire_context)?;
        let round = staged_registry.round_to_core(manifest.round, &self.wire_context)?;
        let subject = core_manifest.subject();
        staged_registry.register_execution_commitment(
            round,
            subject,
            validated_receipt.execution_commitment(),
        )?;
        self.registry = staged_registry;
        self.active_subject = Some((round, subject));
        self.step_with_completion_evidence(
            reducer::Event::LocalProposalReady {
                tag,
                manifest: core_manifest,
            },
            Some(completion_evidence),
        )
    }

    /// Restore a body-store validation marker into the replayed wire registry.
    ///
    /// Proposal intent persistence deliberately precedes signing. On restart,
    /// the safety WAL reconstructs that intent while the exact execution
    /// commitment remains in the independently fsynced body store. Reassociating
    /// those same-round durable records before dispatching startup effects lets
    /// the replayed proposal continue directly into its Prepare vote.
    pub(crate) fn recover_validated_body(
        &mut self,
        manifest: &wire::PayloadManifest,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<(), AdapterError> {
        self.ensure_ingress()?;
        let durable_receipt = validated_receipt.durable();
        if durable_receipt.context_id() != self.wire_context.id()
            || durable_receipt.round() != manifest.round
            || durable_receipt.subject() != manifest.subject
            || durable_receipt.manifest_hash() != HashOf::new(manifest)
        {
            return Err(AdapterError::DurableBodyMismatch);
        }
        validated_receipt.execution_commitment().validate()?;

        // Stage registry expansion so any mismatch leaves replayed authority
        // unchanged and causes startup to fail closed at the caller.
        let mut registry = self.registry.clone();
        let core_manifest = registry.manifest_to_core(manifest, &self.wire_context)?;
        let round = registry.round_to_core(manifest.round, &self.wire_context)?;
        registry.register_execution_commitment(
            round,
            core_manifest.subject(),
            validated_receipt.execution_commitment(),
        )?;
        self.registry = registry;
        Ok(())
    }

    /// Complete a body reconstruction requested by [`AdapterEffect::FetchBody`].
    pub(crate) fn body_available(
        &mut self,
        tag: reducer::EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        let completion_evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        let round = self
            .registry
            .round_to_core(manifest.round, &self.wire_context)?;
        let subject = self.registry.register_subject(manifest.subject)?;
        self.rollback_deferred_conflicting_proposal(round, subject, &manifest);
        let core_manifest = self
            .registry
            .manifest_to_core(&manifest, &self.wire_context)?;
        if core_manifest.subject() != subject {
            return Err(AdapterError::DurableBodyMismatch);
        }
        self.step_with_completion_evidence(
            reducer::Event::BodyAvailable {
                tag,
                round,
                subject,
            },
            Some(completion_evidence),
        )
    }

    /// Retag one Busy-deferred body completion for the reducer incarnation installed by a TC.
    ///
    /// Only lifecycle ownership changes; the manifest proposal round and
    /// subject remain exact.
    pub(crate) fn rebind_deferred_body_available(
        &mut self,
        previous: reducer::EventTag,
        rebound: reducer::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> usize {
        let round = reducer::Round::new(manifest.round.height, manifest.round.view);
        let subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
        let mut rebound_count = 0usize;
        for input in &mut self.deferred_completions {
            if let reducer::Event::BodyAvailable {
                tag,
                round: queued_round,
                subject: queued_subject,
            } = &mut input.event
                && *tag == previous
                && *queued_round == round
                && *queued_subject == subject
            {
                *tag = rebound;
                rebound_count = rebound_count.saturating_add(1);
            }
        }
        rebound_count
    }

    /// Retire one Busy-deferred body completion whose exact pipeline was superseded.
    pub(crate) fn retire_deferred_body_available(
        &mut self,
        tag: reducer::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> usize {
        let round = reducer::Round::new(manifest.round.height, manifest.round.view);
        let subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
        let before = self.deferred_completions.len();
        self.deferred_completions.retain(|input| {
            !matches!(
                &input.event,
                reducer::Event::BodyAvailable {
                    tag: queued_tag,
                    round: queued_round,
                    subject: queued_subject,
                } if *queued_tag == tag && *queued_round == round && *queued_subject == subject
            )
        });
        let retired = before.saturating_sub(self.deferred_completions.len());
        self.retire_unowned_deferred_producer_continuations();
        retired
    }

    /// Count every Busy-deferred completion stage for one exact body pipeline.
    pub(crate) fn deferred_body_pipeline_completion_counts(
        &self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> super::v2_runtime::RetiredBodyPipelineCompletions {
        let round = reducer::Round::new(round.height, round.view);
        let subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        let mut counts = super::v2_runtime::RetiredBodyPipelineCompletions::default();
        for input in self
            .deferred_completions
            .iter()
            .chain(&self.deferred_inputs)
        {
            match deferred_body_pipeline_completion_stage(input, tag, round, subject) {
                Some(DeferredBodyPipelineCompletionStage::LocalProposalReady) => {
                    counts.record_local_proposal();
                }
                Some(DeferredBodyPipelineCompletionStage::BodyAvailable) => {
                    counts.record_body_available();
                }
                Some(DeferredBodyPipelineCompletionStage::BodyStored) => {
                    counts.record_body_stored();
                }
                Some(DeferredBodyPipelineCompletionStage::Validation) => {
                    counts.record_validation();
                }
                None => {}
            }
        }
        counts
    }

    /// Retire every Busy-deferred completion stage for one exact body pipeline.
    pub(crate) fn retire_deferred_body_pipeline_completions(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> super::v2_runtime::RetiredBodyPipelineCompletions {
        let round = reducer::Round::new(round.height, round.view);
        let subject = reducer::Subject::new(Hash::new(subject.encode()).into());
        let mut retired = super::v2_runtime::RetiredBodyPipelineCompletions::default();
        let mut retire = |queue: &mut VecDeque<DeferredInput>| {
            queue.retain(|input| {
                match deferred_body_pipeline_completion_stage(input, tag, round, subject) {
                    Some(DeferredBodyPipelineCompletionStage::LocalProposalReady) => {
                        retired.record_local_proposal();
                        false
                    }
                    Some(DeferredBodyPipelineCompletionStage::BodyAvailable) => {
                        retired.record_body_available();
                        false
                    }
                    Some(DeferredBodyPipelineCompletionStage::BodyStored) => {
                        retired.record_body_stored();
                        false
                    }
                    Some(DeferredBodyPipelineCompletionStage::Validation) => {
                        retired.record_validation();
                        false
                    }
                    None => true,
                }
            });
        };
        retire(&mut self.deferred_completions);
        retire(&mut self.deferred_inputs);
        self.retire_unowned_deferred_producer_continuations();
        retired
    }

    /// Count logical and exact completion owners in the Busy-deferred lane.
    ///
    /// A logical owner occupies the same tag/stage/round/subject slot. An
    /// exact owner additionally retains evidence equal to `candidate`.
    pub(crate) fn deferred_body_pipeline_completion_ownership(
        &self,
        tag: reducer::EventTag,
        candidate: &BodyPipelineCompletionEvidence,
    ) -> (usize, usize) {
        let (wire_round, wire_subject) = match candidate {
            BodyPipelineCompletionEvidence::LocalProposalReady { manifest, .. }
            | BodyPipelineCompletionEvidence::BodyAvailable { manifest } => {
                (manifest.round, manifest.subject)
            }
            BodyPipelineCompletionEvidence::BodyStored { round, subject, .. }
            | BodyPipelineCompletionEvidence::ValidationSucceeded { round, subject, .. }
            | BodyPipelineCompletionEvidence::ValidationFailed { round, subject } => {
                (*round, *subject)
            }
        };
        let round = reducer::Round::new(wire_round.height, wire_round.view);
        let subject = reducer::Subject::new(Hash::new(wire_subject.encode()).into());
        self.deferred_completions
            .iter()
            .chain(&self.deferred_inputs)
            .fold((0usize, 0usize), |(owners, exact), input| {
                let owns_slot = match (&input.event, candidate) {
                    (
                        reducer::Event::LocalProposalReady {
                            tag: queued_tag,
                            manifest,
                        },
                        BodyPipelineCompletionEvidence::LocalProposalReady { .. },
                    ) => {
                        *queued_tag == tag
                            && tag.height() == round.height()
                            && tag.view() == round.view()
                            && manifest.subject() == subject
                    }
                    (
                        reducer::Event::BodyAvailable {
                            tag: queued_tag,
                            round: queued_round,
                            subject: queued_subject,
                        },
                        BodyPipelineCompletionEvidence::BodyAvailable { .. },
                    )
                    | (
                        reducer::Event::BodyStored {
                            tag: queued_tag,
                            round: queued_round,
                            subject: queued_subject,
                        },
                        BodyPipelineCompletionEvidence::BodyStored { .. },
                    ) => *queued_tag == tag && *queued_round == round && *queued_subject == subject,
                    (
                        reducer::Event::ValidationCompleted {
                            tag: queued_tag,
                            round: queued_round,
                            subject: queued_subject,
                            ..
                        },
                        BodyPipelineCompletionEvidence::ValidationSucceeded { .. }
                        | BodyPipelineCompletionEvidence::ValidationFailed { .. },
                    ) => *queued_tag == tag && *queued_round == round && *queued_subject == subject,
                    _ => false,
                };
                if !owns_slot {
                    return (owners, exact);
                }
                (
                    owners.saturating_add(1),
                    exact.saturating_add(usize::from(
                        input.completion_evidence.as_ref() == Some(candidate),
                    )),
                )
            })
    }

    /// Classify exact decided `LocalProposalReady` owners without mutating any
    /// Busy-deferred lane.
    pub(crate) fn deferred_decided_local_proposal_counts(
        &self,
        decision_tag: reducer::EventTag,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) -> super::v2_runtime::DecisionLocalProposalCounts {
        let mut counts = super::v2_runtime::DecisionLocalProposalCounts::default();
        for input in self
            .deferred_completions
            .iter()
            .chain(&self.deferred_inputs)
        {
            if let Some(disposition) = classify_deferred_decided_local_proposal(
                input,
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            ) {
                counts.record(disposition);
            }
        }
        counts
    }

    /// Retire Busy-deferred proposal work after one exact decision is installed.
    ///
    /// All authenticated proposals and nonmatching local completions for the
    /// decided height are terminal. Body recovery, validation, and application
    /// completions remain owned because the decision may still need them before
    /// application. A unique current-tag completion whose full receipts match
    /// the Decision remains in place for direct application. Stale exact
    /// completions are retired so the durable reconstruction path can re-enter
    /// the reducer.
    pub(crate) fn retire_deferred_proposal_work_after_decision(
        &mut self,
        decision_tag: reducer::EventTag,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) {
        let core_round = reducer::Round::new(decision_round.height, decision_round.view);
        let core_subject = reducer::Subject::new(Hash::new(decision_subject.encode()).into());
        let retire = |queue: &mut VecDeque<DeferredInput>| {
            queue.retain(|input| {
                let remove = match &input.event {
                    reducer::Event::ProposalReceived { proposal, .. }
                        if proposal.proposal().round().height() == decision_round.height =>
                    {
                        true
                    }
                    reducer::Event::LocalProposalReady { tag, .. } => input
                        .completion_evidence
                        .as_ref()
                        .and_then(|evidence| match evidence {
                            BodyPipelineCompletionEvidence::LocalProposalReady {
                                manifest, ..
                            } => Some(manifest.round.height),
                            BodyPipelineCompletionEvidence::BodyAvailable { .. }
                            | BodyPipelineCompletionEvidence::BodyStored { .. }
                            | BodyPipelineCompletionEvidence::ValidationSucceeded { .. }
                            | BodyPipelineCompletionEvidence::ValidationFailed { .. } => None,
                        })
                        .is_some_and(|height| height == decision_round.height)
                        .then(|| {
                            !matches!(
                                classify_deferred_decided_local_proposal(
                                    input,
                                    decision_tag,
                                    decision_round,
                                    decision_subject,
                                    decision_commitment,
                                ),
                                Some(DecisionLocalProposalDisposition::Retain)
                            )
                        })
                        .unwrap_or(tag.height() == decision_round.height),
                    reducer::Event::ResumeAfterReplay { .. }
                    | reducer::Event::ProposalReceived { .. }
                    | reducer::Event::VoteReceived { .. }
                    | reducer::Event::QuorumCertificateReceived { .. }
                    | reducer::Event::TimeoutVoteReceived { .. }
                    | reducer::Event::TimeoutCertificateReceived { .. }
                    | reducer::Event::TimeoutElapsed { .. }
                    | reducer::Event::RetransmitElapsed { .. }
                    | reducer::Event::BodyAvailable { .. }
                    | reducer::Event::BodyStored { .. }
                    | reducer::Event::ValidationCompleted { .. }
                    | reducer::Event::Persisted { .. }
                    | reducer::Event::PersistenceFailed { .. }
                    | reducer::Event::Signed { .. }
                    | reducer::Event::ApplicationCompleted { .. } => false,
                };
                !remove
            });
        };
        retire(&mut self.deferred_completions);
        retire(&mut self.deferred_progress_inputs);
        retire(&mut self.deferred_inputs);
        if self.active_subject.is_some_and(|(round, subject)| {
            round.height() == decision_round.height
                && (round != core_round || subject != core_subject)
        }) {
            self.active_subject = None;
        }
        self.retire_unowned_deferred_producer_continuations();
    }

    /// Retire deferred proposals made unsafe by an installed durable lock.
    ///
    /// The locked subject may remain queued in a later justified view. A
    /// different subject survives only when the proposal carries a strictly
    /// higher matching PrepareQC.
    pub(crate) fn retire_deferred_unsafe_proposals_for_lock(
        &mut self,
        locked_round: wire::ConsensusRound,
        locked_subject: wire::BlockSubject,
    ) -> usize {
        let locked_round = reducer::Round::new(locked_round.height, locked_round.view);
        let locked_subject = reducer::Subject::new(Hash::new(locked_subject.encode()).into());
        let before = self.deferred_inputs.len();
        self.deferred_inputs.retain(|input| {
            let reducer::Event::ProposalReceived { proposal, .. } = &input.event else {
                return true;
            };
            let proposal = proposal.proposal();
            if proposal.context_id() != self.reducer.context().id()
                || proposal.round().height() != locked_round.height()
            {
                return true;
            }
            if proposal.round().view() < locked_round.view() {
                return false;
            }
            if proposal.manifest().subject() == locked_subject {
                return true;
            }
            let reducer::ProposalJustification::Timeout(certificate) = proposal.justification()
            else {
                return false;
            };
            certificate.highest_prepare().is_some_and(|highest| {
                highest.phase() == reducer::Phase::Prepare
                    && highest.subject() == proposal.manifest().subject()
                    && highest.round().view() > locked_round.view()
            })
        });
        let retired = before.saturating_sub(self.deferred_inputs.len());
        self.active_subject = Some((locked_round, locked_subject));
        self.retire_unowned_deferred_producer_continuations();
        retired
    }

    /// Stage one exact completion at the adapter boundary for runtime/executor seam tests.
    #[cfg(test)]
    pub(crate) fn defer_body_available_for_test(
        &mut self,
        tag: reducer::EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<(), AdapterError> {
        let core_manifest = self
            .registry
            .manifest_to_core(manifest, &self.wire_context)?;
        let admission_capability = self.mint_deferred_admission_ordinal()?;
        let admission_ordinal = admission_capability.ordinal;
        self.deferred_completions.push_back(DeferredInput {
            admission_ordinal,
            admission_capability,
            event: reducer::Event::BodyAvailable {
                tag,
                round: reducer::Round::new(manifest.round.height, manifest.round.view),
                subject: core_manifest.subject(),
            },
            completion_evidence: Some(BodyPipelineCompletionEvidence::BodyAvailable {
                manifest: manifest.clone(),
            }),
            retag_authenticated_ingress: false,
            priority: DeferredPriority::Completion,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
        Ok(())
    }

    /// Stage one authenticated proposal in the Busy-deferred lane for seam tests.
    #[cfg(test)]
    pub(crate) fn defer_authenticated_proposal_for_test(
        &mut self,
        tag: reducer::EventTag,
        proposal: &wire::Proposal,
    ) -> Result<(), AdapterError> {
        let authenticated_wire_identity = Arc::<[u8]>::from(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            ))
            .encode(),
        );
        let proposal = self
            .registry
            .proposal_to_core(proposal, &self.wire_context)?;
        let round = proposal.proposal().round();
        let subject = proposal.proposal().manifest().subject();
        self.active_subject = Some((round, subject));
        let admission_capability = self.mint_deferred_admission_ordinal()?;
        let admission_ordinal = admission_capability.ordinal;
        self.deferred_inputs.push_back(DeferredInput {
            admission_ordinal,
            admission_capability,
            event: reducer::Event::ProposalReceived { tag, proposal },
            completion_evidence: None,
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Normal,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: Some(authenticated_wire_identity),
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
        Ok(())
    }

    /// Stage one non-fetch body completion in the Busy-deferred lane for seam tests.
    #[cfg(test)]
    pub(crate) fn defer_body_pipeline_stage_for_test(
        &mut self,
        tag: reducer::EventTag,
        manifest: &wire::PayloadManifest,
        stage: DeferredBodyPipelineStageForTest,
    ) -> Result<(), AdapterError> {
        let core_manifest = self
            .registry
            .manifest_to_core(manifest, &self.wire_context)?;
        let round = reducer::Round::new(manifest.round.height, manifest.round.view);
        let subject = core_manifest.subject();
        let durable_receipt = DurableBodyReceipt::for_test(
            self.wire_context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(manifest),
        );
        let validated_receipt = ValidatedBodyReceipt::for_test(durable_receipt.clone());
        let completion_evidence = match stage {
            DeferredBodyPipelineStageForTest::BodyStored => {
                BodyPipelineCompletionEvidence::BodyStored {
                    round: manifest.round,
                    subject: manifest.subject,
                    receipt: durable_receipt,
                }
            }
            DeferredBodyPipelineStageForTest::ValidationSucceeded => {
                BodyPipelineCompletionEvidence::ValidationSucceeded {
                    round: manifest.round,
                    subject: manifest.subject,
                    receipt: validated_receipt,
                }
            }
            DeferredBodyPipelineStageForTest::ValidationFailed => {
                BodyPipelineCompletionEvidence::ValidationFailed {
                    round: manifest.round,
                    subject: manifest.subject,
                }
            }
            DeferredBodyPipelineStageForTest::LocalProposalReady => {
                BodyPipelineCompletionEvidence::LocalProposalReady {
                    manifest: manifest.clone(),
                    durable_receipt,
                    validated_receipt,
                }
            }
        };
        let event = match stage {
            DeferredBodyPipelineStageForTest::BodyStored => reducer::Event::BodyStored {
                tag,
                round,
                subject,
            },
            DeferredBodyPipelineStageForTest::ValidationSucceeded => {
                reducer::Event::ValidationCompleted {
                    tag,
                    round,
                    subject,
                    valid: true,
                }
            }
            DeferredBodyPipelineStageForTest::ValidationFailed => {
                reducer::Event::ValidationCompleted {
                    tag,
                    round,
                    subject,
                    valid: false,
                }
            }
            DeferredBodyPipelineStageForTest::LocalProposalReady => {
                reducer::Event::LocalProposalReady {
                    tag,
                    manifest: core_manifest,
                }
            }
        };
        let admission_capability = self.mint_deferred_admission_ordinal()?;
        let admission_ordinal = admission_capability.ordinal;
        self.deferred_completions.push_back(DeferredInput {
            admission_ordinal,
            admission_capability,
            event,
            completion_evidence: Some(completion_evidence),
            retag_authenticated_ingress: false,
            priority: DeferredPriority::Completion,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
        Ok(())
    }

    fn rollback_deferred_conflicting_proposal(
        &mut self,
        round: reducer::Round,
        subject: reducer::Subject,
        canonical: &wire::PayloadManifest,
    ) -> bool {
        // Busy authenticated ingress deliberately retains its staged registry
        // expansion. A canonical body completion may overtake that deferred
        // proposal, but may roll back only the exact proposal-owned manifest;
        // independently verified justification, QC, and subject material stays
        // registered for subsequent progress.
        let key = (round, subject);
        let Some(registered_manifest) = self.registry.manifests.get(&key).cloned() else {
            return false;
        };
        if registered_manifest == *canonical {
            return false;
        }
        let Some(registered_proposal) = self.registry.proposals.get(&key).cloned() else {
            return false;
        };
        if registered_proposal.round != canonical.round
            || registered_proposal.subject != canonical.subject
            || registered_proposal.manifest != registered_manifest
        {
            return false;
        }
        let admission_key = IngressSemanticKey::Proposal {
            round: registered_proposal.round,
            proposer: registered_proposal.proposer,
        };
        let expected_fingerprint =
            IngressFingerprint::Proposal(Hash::new(registered_proposal.signature_preimage()));
        let Some(registered_equivocation) = self.ingress_equivocations.get(&admission_key).copied()
        else {
            return false;
        };
        if registered_equivocation.fingerprint != expected_fingerprint {
            return false;
        }
        let owns_conflict = |input: &DeferredInput| {
            Self::deferred_input_owns_registered_proposal(
                input,
                round,
                subject,
                &registered_proposal,
            )
        };
        if !self.deferred_inputs.iter().any(owns_conflict) {
            return false;
        }

        self.deferred_inputs.retain(|input| !owns_conflict(input));
        self.retire_unowned_deferred_producer_continuations();
        let removed_proposal = self.registry.proposals.remove(&key);
        let removed_manifest = self.registry.manifests.remove(&key);
        let removed_equivocation = self.ingress_equivocations.remove(&admission_key);
        self.ingress_deliveries.remove(&admission_key);
        debug_assert_eq!(removed_proposal, Some(registered_proposal));
        debug_assert_eq!(removed_manifest, Some(registered_manifest));
        debug_assert_eq!(removed_equivocation, Some(registered_equivocation));
        true
    }

    fn deferred_input_owns_registered_proposal(
        input: &DeferredInput,
        round: reducer::Round,
        subject: reducer::Subject,
        registered: &wire::Proposal,
    ) -> bool {
        if !input.retag_authenticated_ingress || input.priority != DeferredPriority::Normal {
            return false;
        }
        let reducer::Event::ProposalReceived { proposal, .. } = &input.event else {
            return false;
        };
        let core = proposal.proposal();
        let core_manifest = core.manifest();
        core.context_id() == context_id(registered.round.context_id)
            && core.round() == round
            && core.proposer() == validator_token(registered.proposer)
            && core_manifest.subject() == subject
            && core_manifest.payload_hash()
                == reducer::Digest::new(*registered.manifest.subject.payload_hash.as_ref())
            && core_manifest.chunk_root()
                == reducer::Digest::new(*registered.manifest.chunk_root.as_ref())
            && core_manifest.byte_len() == registered.manifest.payload_size_bytes
            && u32::try_from(registered.manifest.chunk_hashes.len()).ok()
                == Some(core_manifest.chunk_count())
            && proposal.signature().as_bytes() == registered.signature
    }

    /// Acknowledge durable storage requested by [`AdapterEffect::StoreBody`].
    pub(crate) fn body_stored(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &DurableBodyReceipt,
    ) -> Result<AdapterOutcome, AdapterError> {
        if receipt.context_id() != self.wire_context.id()
            || receipt.round() != round
            || receipt.subject() != subject
        {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let round = self.registry.round_to_core(round, &self.wire_context)?;
        let subject = self.registry.register_subject(subject)?;
        let manifest = self
            .registry
            .manifests
            .get(&(round, subject))
            .ok_or(AdapterError::MissingManifest)?;
        if receipt.manifest_hash() != HashOf::new(manifest) {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let completion_evidence = BodyPipelineCompletionEvidence::BodyStored {
            round: receipt.round(),
            subject: receipt.subject(),
            receipt: receipt.clone(),
        };
        self.step_with_completion_evidence(
            reducer::Event::BodyStored {
                tag,
                round,
                subject,
            },
            Some(completion_evidence),
        )
    }

    /// Return successful deterministic validation requested by
    /// [`AdapterEffect::ValidateBody`].
    pub(crate) fn validation_succeeded(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &ValidatedBodyReceipt,
    ) -> Result<AdapterOutcome, AdapterError> {
        if receipt.durable().context_id() != self.wire_context.id()
            || receipt.durable().round() != round
            || receipt.durable().subject() != subject
        {
            return Err(AdapterError::DurableBodyMismatch);
        }
        let round = self.registry.round_to_core(round, &self.wire_context)?;
        let subject = self.registry.register_subject(subject)?;
        let manifest = self
            .registry
            .manifests
            .get(&(round, subject))
            .ok_or(AdapterError::MissingManifest)?;
        if receipt.durable().manifest_hash() != HashOf::new(manifest) {
            return Err(AdapterError::DurableBodyMismatch);
        }
        self.registry.register_execution_commitment(
            round,
            subject,
            receipt.execution_commitment(),
        )?;
        let completion_evidence = BodyPipelineCompletionEvidence::ValidationSucceeded {
            round: receipt.durable().round(),
            subject: receipt.durable().subject(),
            receipt: receipt.clone(),
        };
        self.step_with_completion_evidence(
            reducer::Event::ValidationCompleted {
                tag,
                round,
                subject,
                valid: true,
            },
            Some(completion_evidence),
        )
    }

    /// Report deterministic rejection of a durable body. A rejection cannot
    /// authorize a vote, so it requires no success receipt.
    pub(crate) fn validation_failed(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<AdapterOutcome, AdapterError> {
        let completion_evidence =
            BodyPipelineCompletionEvidence::ValidationFailed { round, subject };
        let round = self.registry.round_to_core(round, &self.wire_context)?;
        let subject = self.registry.register_subject(subject)?;
        self.step_with_completion_evidence(
            reducer::Event::ValidationCompleted {
                tag,
                round,
                subject,
                valid: false,
            },
            Some(completion_evidence),
        )
    }

    /// Return the opaque signature requested by [`AdapterEffect::Sign`].
    pub(crate) fn signature_completed(
        &mut self,
        tag: reducer::EventTag,
        signature: Vec<u8>,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.step(reducer::Event::Signed {
            tag,
            signature: reducer::OpaqueSignature::new(signature),
        })
    }

    /// Acknowledge successful application of the exact tagged decision.
    ///
    /// The reducer validates the current `(height, view, generation)` tag and
    /// refuses a completion for an undecided, unvalidated, stale, or different
    /// subject.
    pub(crate) fn application_completed(
        &mut self,
        tag: reducer::EventTag,
        subject: wire::BlockSubject,
    ) -> Result<AdapterOutcome, AdapterError> {
        let subject = self.registry.register_subject(subject)?;
        self.step(reducer::Event::ApplicationCompleted { tag, subject })
    }

    /// Decide whether an exact internal callback still needs a serialized
    /// runtime admission.
    ///
    /// The projection uses cloned registry/reducer state, so malformed, stale,
    /// monotone-complete, and durably tombstoned callbacks consume neither an
    /// admission ordinal nor a physical FIFO slot. Authenticated wire ingress
    /// deliberately bypasses this seam and remains governed by canonical
    /// authentication and semantic-delivery ownership.
    pub(crate) fn preflight_runtime_command_admission(
        &self,
        tag: reducer::EventTag,
        command: &super::v2_runtime::AdapterCommand,
    ) -> super::v2_runtime::RuntimeCommandAdmissionPreflight {
        use super::v2_runtime::{AdapterCommand, RuntimeCommandAdmissionPreflight as Preflight};

        if matches!(command, AdapterCommand::Authenticated(_)) {
            return Preflight::Admit;
        }
        if self.fail_closed || !self.replay_complete {
            return Preflight::Reject;
        }

        let projected = (|| -> Result<_, AdapterError> {
            let mut registry = self.registry.clone();
            let (event, completion_evidence) = match command {
                AdapterCommand::Authenticated(_) => unreachable!("handled above"),
                AdapterCommand::LocalProposalReady {
                    manifest,
                    durable_receipt,
                    validated_receipt,
                } => {
                    if durable_receipt.context_id() != self.wire_context.id()
                        || durable_receipt.round() != manifest.round
                        || durable_receipt.subject() != manifest.subject
                        || durable_receipt.manifest_hash() != HashOf::new(manifest)
                        || validated_receipt.durable() != durable_receipt
                    {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    let core_manifest = registry.manifest_to_core(manifest, &self.wire_context)?;
                    let round = registry.round_to_core(manifest.round, &self.wire_context)?;
                    registry.register_execution_commitment(
                        round,
                        core_manifest.subject(),
                        validated_receipt.execution_commitment(),
                    )?;
                    (
                        reducer::Event::LocalProposalReady {
                            tag,
                            manifest: core_manifest,
                        },
                        Some(BodyPipelineCompletionEvidence::LocalProposalReady {
                            manifest: manifest.clone(),
                            durable_receipt: durable_receipt.clone(),
                            validated_receipt: validated_receipt.clone(),
                        }),
                    )
                }
                AdapterCommand::BodyAvailable { manifest } => {
                    let round = registry.round_to_core(manifest.round, &self.wire_context)?;
                    let subject = registry.register_subject(manifest.subject)?;
                    let core_manifest = registry.manifest_to_core(manifest, &self.wire_context)?;
                    if core_manifest.subject() != subject {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    (
                        reducer::Event::BodyAvailable {
                            tag,
                            round,
                            subject,
                        },
                        Some(BodyPipelineCompletionEvidence::BodyAvailable {
                            manifest: manifest.clone(),
                        }),
                    )
                }
                AdapterCommand::BodyStored {
                    round,
                    subject,
                    receipt,
                } => {
                    if receipt.context_id() != self.wire_context.id()
                        || receipt.round() != *round
                        || receipt.subject() != *subject
                    {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    let core_round = registry.round_to_core(*round, &self.wire_context)?;
                    let core_subject = registry.register_subject(*subject)?;
                    let manifest = registry
                        .manifests
                        .get(&(core_round, core_subject))
                        .ok_or(AdapterError::MissingManifest)?;
                    if receipt.manifest_hash() != HashOf::new(manifest) {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    (
                        reducer::Event::BodyStored {
                            tag,
                            round: core_round,
                            subject: core_subject,
                        },
                        Some(BodyPipelineCompletionEvidence::BodyStored {
                            round: *round,
                            subject: *subject,
                            receipt: receipt.clone(),
                        }),
                    )
                }
                AdapterCommand::ValidationSucceeded {
                    round,
                    subject,
                    receipt,
                } => {
                    if receipt.durable().context_id() != self.wire_context.id()
                        || receipt.durable().round() != *round
                        || receipt.durable().subject() != *subject
                    {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    let core_round = registry.round_to_core(*round, &self.wire_context)?;
                    let core_subject = registry.register_subject(*subject)?;
                    let manifest = registry
                        .manifests
                        .get(&(core_round, core_subject))
                        .ok_or(AdapterError::MissingManifest)?;
                    if receipt.durable().manifest_hash() != HashOf::new(manifest) {
                        return Err(AdapterError::DurableBodyMismatch);
                    }
                    registry.register_execution_commitment(
                        core_round,
                        core_subject,
                        receipt.execution_commitment(),
                    )?;
                    (
                        reducer::Event::ValidationCompleted {
                            tag,
                            round: core_round,
                            subject: core_subject,
                            valid: true,
                        },
                        Some(BodyPipelineCompletionEvidence::ValidationSucceeded {
                            round: *round,
                            subject: *subject,
                            receipt: receipt.clone(),
                        }),
                    )
                }
                AdapterCommand::ValidationFailed { round, subject } => {
                    let core_round = registry.round_to_core(*round, &self.wire_context)?;
                    let core_subject = registry.register_subject(*subject)?;
                    (
                        reducer::Event::ValidationCompleted {
                            tag,
                            round: core_round,
                            subject: core_subject,
                            valid: false,
                        },
                        Some(BodyPipelineCompletionEvidence::ValidationFailed {
                            round: *round,
                            subject: *subject,
                        }),
                    )
                }
                AdapterCommand::SignatureCompleted(signature) => (
                    reducer::Event::Signed {
                        tag,
                        signature: reducer::OpaqueSignature::new(signature.clone()),
                    },
                    None,
                ),
                AdapterCommand::ApplicationCompleted(subject) => (
                    reducer::Event::ApplicationCompleted {
                        tag,
                        subject: registry.register_subject(*subject)?,
                    },
                    None,
                ),
            };
            Ok((event, completion_evidence))
        })();
        let Ok((event, completion_evidence)) = projected else {
            return Preflight::Reject;
        };

        // Internal completions retain their originating reducer incarnation.
        // A delayed completion from an obsolete incarnation is a harmless
        // stutter, but it must be discarded before allocating a new runtime
        // ordinal. The exact payload was validated above first so a malformed
        // internal callback cannot hide behind a stale tag.
        if tag != self.reducer.current_tag() {
            return Preflight::Coalesce;
        }

        let serviced_candidate = self.serviced_candidate(
            &event,
            DeferredPriority::Completion,
            completion_evidence.as_ref(),
            None,
        );
        if let Some((key, _, _)) = serviced_candidate {
            if self.serviced_candidates.contains_key(&key) {
                return Preflight::Coalesce;
            }
            let matching = self
                .producer_continuations
                .iter()
                .filter(|(_, record)| record.identity().candidate() == key)
                .collect::<Vec<_>>();
            match matching.len() {
                0 => {}
                1 => {
                    let (address, record) = matching[0];
                    if record.status() != ProducerContinuationStatus::Reserved
                        || !self
                            .restored_dormant_producer_continuations
                            .contains(address)
                        || self.durable_producer_continuations.get(address) != Some(record)
                    {
                        return Preflight::Coalesce;
                    }
                    let identity = record.identity();
                    // `ServicedCandidateKey` is deliberately route/priority
                    // neutral. This branch is nevertheless class-exact:
                    // only internal completion commands reach this
                    // preflight, and the serialized runtime rejects
                    // `ReuseDormant` in Normal or Progress before allocating
                    // a FIFO ordinal. Authenticated traffic retains the
                    // separate leader-wire lifecycle gate above.
                    return Preflight::ReuseDormant {
                        causal_lifecycle_key: identity.causal_lifecycle_key(),
                        admission_ordinal: identity.admission_ordinal(),
                        producer_stage: identity.stage(),
                    };
                }
                _ => return Preflight::Reject,
            }
        }

        // The reducer's persistence/signing fences intentionally report Busy
        // before dispatching an event to its phase handler. Consult the
        // phase-specific monotone facts first so an exact callback which has
        // already handed ownership to its successor cannot be admitted again
        // merely because unrelated durable work is now fenced.
        let phase_fact = match &event {
            reducer::Event::LocalProposalReady { manifest, .. } => {
                let round = reducer::Round::new(tag.height(), tag.view());
                let classify_proposal = |proposal: &reducer::Proposal| {
                    (proposal.round() == round).then(|| {
                        if proposal.manifest() == manifest {
                            Preflight::Coalesce
                        } else {
                            Preflight::Reject
                        }
                    })
                };
                self.reducer
                    .pending_persistence_record()
                    .and_then(|record| match record {
                        reducer::WalRecord::ProposalIntent(proposal) => classify_proposal(proposal),
                        _ => None,
                    })
                    .or_else(|| {
                        self.reducer
                            .durable_state()
                            .proposal_intent(round)
                            .and_then(classify_proposal)
                    })
                    .or_else(|| {
                        self.reducer
                            .awaiting_signature()
                            .and_then(|signable| match signable {
                                reducer::SignableMessage::Proposal(proposal) => {
                                    classify_proposal(proposal)
                                }
                                reducer::SignableMessage::Vote(_)
                                | reducer::SignableMessage::TimeoutVote(_) => None,
                            })
                    })
                    .or_else(|| {
                        self.reducer
                            .durable_state()
                            .decision()
                            .and_then(|decision| {
                                let exact_decided_body = decision.proposal_round() == round
                                    && decision.subject() == manifest.subject();
                                (exact_decided_body
                                    && (self.reducer.applied_subject() == Some(decision.subject())
                                        || self.reducer.body_state(round, decision.subject())
                                            == reducer::BodyState::Validated))
                                    .then_some(Preflight::Coalesce)
                            })
                    })
            }
            reducer::Event::BodyAvailable { round, subject, .. } => {
                (self.reducer.body_state(*round, *subject) != reducer::BodyState::Missing)
                    .then_some(Preflight::Coalesce)
            }
            reducer::Event::BodyStored { round, subject, .. } => {
                match self.reducer.body_state(*round, *subject) {
                    reducer::BodyState::Missing | reducer::BodyState::Available => None,
                    reducer::BodyState::Durable
                    | reducer::BodyState::Validated
                    | reducer::BodyState::Invalid => Some(Preflight::Coalesce),
                }
            }
            reducer::Event::ValidationCompleted {
                round,
                subject,
                valid,
                ..
            } => match (self.reducer.body_state(*round, *subject), *valid) {
                (reducer::BodyState::Missing | reducer::BodyState::Durable, _) => None,
                (reducer::BodyState::Validated, true)
                | (reducer::BodyState::Invalid, false)
                | (reducer::BodyState::Available, _) => Some(Preflight::Coalesce),
                (reducer::BodyState::Validated, false) | (reducer::BodyState::Invalid, true) => {
                    Some(Preflight::Reject)
                }
            },
            reducer::Event::Signed { .. } => self
                .reducer
                .awaiting_signature()
                .is_none()
                .then_some(Preflight::Coalesce),
            reducer::Event::ApplicationCompleted { subject, .. } => {
                self.reducer.applied_subject().map(|applied| {
                    if applied == *subject {
                        Preflight::Coalesce
                    } else {
                        Preflight::Reject
                    }
                })
            }
            reducer::Event::ResumeAfterReplay { .. }
            | reducer::Event::ProposalReceived { .. }
            | reducer::Event::VoteReceived { .. }
            | reducer::Event::QuorumCertificateReceived { .. }
            | reducer::Event::TimeoutVoteReceived { .. }
            | reducer::Event::TimeoutCertificateReceived { .. }
            | reducer::Event::TimeoutElapsed { .. }
            | reducer::Event::RetransmitElapsed { .. }
            | reducer::Event::Persisted { .. }
            | reducer::Event::PersistenceFailed { .. } => None,
        };
        if let Some(preflight) = phase_fact {
            return preflight;
        }

        let mut projected_reducer = self.reducer.clone();
        match projected_reducer.step(event) {
            Ok(outcome) => match outcome.disposition() {
                reducer::StepDisposition::Applied
                | reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
                | reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork) => {
                    Preflight::Admit
                }
                reducer::StepDisposition::Ignored(_) => Preflight::Coalesce,
            },
            Err(_) => Preflight::Reject,
        }
    }

    /// Consume an applied height after Kura has durably associated the exact
    /// canonical block and CommitQC artifact.
    ///
    /// This is the only production path which retires the height safety WAL.
    /// It compares the non-forgeable Kura receipt, the persisted artifact, and
    /// the reducer's cryptographically verified decision before consuming the
    /// reducer, then attempts to remove and directory-sync the obsolete WAL.
    /// Once the typed Kura receipt matches, cleanup failure is reported on the
    /// finalized result rather than misreporting the durable decision as lost.
    pub(crate) fn finish_height(
        mut self,
        kura_receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<FinalizedV2Height, AdapterError> {
        self.ensure_ingress()?;
        artifact
            .verify()
            .map_err(|error| AdapterError::Cryptography(error.to_string()))?;
        let core_decision = self
            .reducer
            .durable_state()
            .decision()
            .cloned()
            .ok_or(AdapterError::DurableCommitMismatch)?;
        let wire_decision = self
            .registry
            .qc_to_wire(&core_decision, self.aggregator.as_ref())?;
        let wire_subject = self.registry.subject(core_decision.subject())?;

        if artifact.height_context != self.wire_context
            || artifact.validator_set_pops != self.proofs_of_possession
            || artifact.subject != wire_subject
            || artifact.commit_qc != wire_decision
            || kura_receipt.height() != self.wire_context.height
            || kura_receipt.context_id() != self.wire_context.id()
            || kura_receipt.block_hash() != wire_subject.block_hash
            || kura_receipt.subject() != wire_subject
            || kura_receipt.certificate() != wire_decision.as_ref()
            || kura_receipt.artifact_hash() != HashOf::new(artifact)
        {
            return Err(AdapterError::DurableCommitMismatch);
        }

        let reducer_receipt = reducer::DurableCommitReceipt::from_trusted_storage(
            context_id(self.wire_context.id()),
            self.wire_context.height,
            core_decision.subject(),
            core_decision.reference(),
        );
        let closed = self.reducer.finish_height(reducer_receipt)?;
        let retirement = reducer::WalRetirementAuthorization::from_finalized_height(&closed);
        if !retirement.matches_finalized_height(&closed) {
            return Err(AdapterError::DurableCommitMismatch);
        }
        let serviced_candidate_warning = self.serviced_candidate_store.retire().err();
        let safety_wal_warning = self
            .wal
            .retire(retirement)
            .err()
            .map(|error| error.to_string());
        let wal_retirement_warning = match (safety_wal_warning, serviced_candidate_warning) {
            (None, None) => None,
            (Some(wal), None) => Some(wal),
            (None, Some(candidates)) => Some(candidates),
            (Some(wal), Some(candidates)) => {
                Some(format!("{wal}; serviced-candidate cleanup: {candidates}"))
            }
        };
        Ok(FinalizedV2Height {
            wal_retirement_warning,
        })
    }

    /// Build the compact canonical status payload from durable reducer state.
    pub(crate) fn status(&mut self) -> Result<wire::SumeragiV2Status, AdapterError> {
        let durable = self.reducer.durable_state();
        let view = durable.current_view();
        let leader = self
            .registry
            .validator_index(self.reducer.context().leader(view))?;
        let locked_prepare_qc = durable
            .locked()
            .map(|certificate| {
                self.registry
                    .qc_to_wire(certificate, self.aggregator.as_ref())
                    .map(|certificate| certificate.as_ref())
            })
            .transpose()?;
        let highest_prepare_qc = durable
            .highest_prepare()
            .map(|certificate| {
                self.registry
                    .qc_to_wire(certificate, self.aggregator.as_ref())
                    .map(|certificate| certificate.as_ref())
            })
            .transpose()?;
        let last_timeout_certificate = durable
            .last_timeout()
            .map(|certificate| {
                self.registry
                    .tc_to_wire(certificate, self.aggregator.as_ref())
                    .map(|certificate| certificate.as_ref())
            })
            .transpose()?;
        let decision = durable.decision().cloned();
        let (last_committed_height, last_committed_subject, last_commit_qc) =
            if let Some(certificate) = &decision {
                let certificate = self
                    .registry
                    .qc_to_wire(certificate, self.aggregator.as_ref())?;
                (
                    certificate.round.height,
                    Some(certificate.subject),
                    Some(commit_qc_status(&certificate, &self.wire_context)?),
                )
            } else if let Some(parent) = &self.wire_context.parent_commit_qc {
                let verification = self
                    .parent_verification
                    .as_ref()
                    .ok_or(AdapterError::ParentContextMismatch)?;
                let summary = commit_qc_status(parent, &verification.context)?;
                (parent.round.height, Some(parent.subject), Some(summary))
            } else if let Some(anchor) = &self.wire_context.snapshot_bootstrap {
                (anchor.snapshot_height, None, None)
            } else {
                (0, None, None)
            };
        let validator_count = u32::try_from(self.wire_context.roster.len())
            .map_err(|_| wire::ValidationError::RosterTooLarge)?;
        let height_context = wire::SumeragiV2HeightContextStatus {
            epoch: self.wire_context.epoch,
            epoch_end_height: self.wire_context.epoch_end_height,
            mode: self.wire_context.mode,
            epoch_seed: self.wire_context.leader_seed,
            validator_count,
            quorum: self.wire_context.quorum,
        };

        let (phase, body_state) = if let Some(decision) = &decision {
            if self.reducer.applied_subject() == Some(decision.subject()) {
                (
                    wire::SumeragiV2StatusPhase::PendingApply,
                    wire::SumeragiV2BodyState::Applied,
                )
            } else {
                (
                    wire::SumeragiV2StatusPhase::PendingApply,
                    wire::SumeragiV2BodyState::PendingApply,
                )
            }
        } else if let Some((round, subject)) = self.active_subject {
            match self.reducer.body_state(round, subject) {
                reducer::BodyState::Missing => (
                    wire::SumeragiV2StatusPhase::ReconstructingPayload,
                    wire::SumeragiV2BodyState::Reconstructing,
                ),
                reducer::BodyState::Available => (
                    wire::SumeragiV2StatusPhase::ReconstructingPayload,
                    wire::SumeragiV2BodyState::Reconstructing,
                ),
                reducer::BodyState::Durable => (
                    wire::SumeragiV2StatusPhase::ValidatingPayload,
                    wire::SumeragiV2BodyState::Stored,
                ),
                reducer::BodyState::Validated => {
                    if durable.locked().is_some() {
                        (
                            wire::SumeragiV2StatusPhase::Commit,
                            wire::SumeragiV2BodyState::Validated,
                        )
                    } else {
                        (
                            wire::SumeragiV2StatusPhase::Prepare,
                            wire::SumeragiV2BodyState::Validated,
                        )
                    }
                }
                reducer::BodyState::Invalid => (
                    wire::SumeragiV2StatusPhase::AwaitingProposal,
                    wire::SumeragiV2BodyState::Missing,
                ),
            }
        } else {
            (
                wire::SumeragiV2StatusPhase::AwaitingProposal,
                wire::SumeragiV2BodyState::Missing,
            )
        };

        #[cfg(not(test))]
        let output_guard_restart_required =
            super::output_guard::process_consensus_output_guard().restart_required();
        #[cfg(test)]
        let output_guard_restart_required = false;

        let liveness = self.liveness_status()?;

        Ok(wire::SumeragiV2Status {
            protocol_version: wire::PROTOCOL_VERSION,
            node_fingerprint: self.fingerprints.node,
            build_fingerprint: self.fingerprints.build,
            config_fingerprint: self.fingerprints.config,
            restart_required: self.fail_closed || output_guard_restart_required,
            height_context_id: self.wire_context.id(),
            height: self.wire_context.height,
            view,
            phase,
            leader,
            locked_prepare_qc,
            highest_prepare_qc,
            last_timeout_certificate,
            body_state,
            pending_persistence_id: self.pending_persistence_id,
            last_committed_height,
            last_committed_subject,
            height_context,
            last_commit_qc,
            liveness,
        })
    }

    /// Record and snapshot the runner-owned live-successor boundary.
    ///
    /// The marker lives in the adapter rather than only in the global status
    /// registry. Consequently a later ignored input or retransmission cannot
    /// restore the older replay marker and erase the activation witness.
    pub(crate) fn successor_activation_status(
        &mut self,
    ) -> Result<wire::SumeragiV2Status, AdapterError> {
        let round = reducer::Round::new(
            self.reducer.context().height(),
            self.reducer.current_tag().view(),
        );
        self.last_progress = Some((
            self.reducer.generation(),
            round,
            wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
        ));
        self.status()
    }

    fn liveness_status(&mut self) -> Result<wire::SumeragiV2LivenessStatus, AdapterError> {
        let min_signers = u32::try_from(self.reducer.context().minimum_signer_count())
            .map_err(|_| wire::ValidationError::RosterTooLarge)?;
        let total_power = self.reducer.context().total_voting_power().get();
        let mut prepare_quorums = Vec::new();
        let mut commit_quorums = Vec::new();
        for snapshot in self.reducer.vote_pool_snapshots() {
            let quorum = wire::SumeragiV2VoteQuorumStatus {
                round: self.registry.round_to_wire(snapshot.round),
                proposal_round: self.registry.round_to_wire(snapshot.proposal_round),
                subject: self.registry.subject(snapshot.subject)?,
                execution_commitment: self
                    .registry
                    .execution_commitment(snapshot.proposal_round, snapshot.subject)?,
                signer_count: u32::try_from(snapshot.signers.len())
                    .map_err(|_| wire::ValidationError::TooManySigners)?,
                signed_power: snapshot.signed_power.get(),
                min_signers,
                total_power,
            };
            match snapshot.phase {
                reducer::Phase::Prepare => prepare_quorums.push(quorum),
                reducer::Phase::Commit => commit_quorums.push(quorum),
            }
        }
        let timeout_quorums = self
            .reducer
            .timeout_pool_snapshots()
            .into_iter()
            .map(|snapshot| {
                Ok(wire::SumeragiV2TimeoutQuorumStatus {
                    round: self.registry.round_to_wire(snapshot.round),
                    signer_count: u32::try_from(snapshot.signers.len())
                        .map_err(|_| wire::ValidationError::TooManySigners)?,
                    signed_power: snapshot.signed_power.get(),
                    min_signers,
                    total_power,
                    certificate_formed: snapshot.certificate_formed,
                })
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;

        let outbound_intents = self.outbound_intent_statuses()?;
        let work = self.local_work_status();
        let queues = self.adapter_queue_statuses();
        let last_progress = self.last_progress.map(|(generation, round, transition)| {
            wire::SumeragiV2ProgressTransitionStatus {
                generation: generation.get(),
                round: self.registry.round_to_wire(round),
                transition,
                age_ms: 0,
            }
        });
        let ignore_counts = ALL_IGNORE_REASONS
            .into_iter()
            .map(|(core, wire)| wire::SumeragiV2IgnoreCount {
                reason: wire,
                count: self.ignore_counts.get(&core).copied().unwrap_or_default(),
            })
            .collect();

        Ok(wire::SumeragiV2LivenessStatus {
            generation: self.reducer.generation().get(),
            prepare_quorums,
            commit_quorums,
            timeout_quorums,
            outbound_intents,
            work,
            queues,
            last_progress,
            no_progress_age_ms: 0,
            blocker: None,
            ignore_counts,
        })
    }

    fn outbound_intent_statuses(
        &self,
    ) -> Result<Vec<wire::SumeragiV2OutboundIntentStatus>, AdapterError> {
        let mut intents = BTreeMap::<
            wire::SumeragiV2OutboundIntentKind,
            wire::SumeragiV2OutboundIntentStatus,
        >::new();
        if let Some(record) = self.reducer.pending_persistence_record() {
            let intent = self.intent_from_wal_record(
                record,
                wire::SumeragiV2OutboundIntentStage::PendingPersistence,
            )?;
            Self::retain_intent(&mut intents, intent);
        }
        if let Some(signable) = self.reducer.awaiting_signature() {
            let intent = self.intent_from_signable(
                signable,
                wire::SumeragiV2OutboundIntentStage::PendingSignature,
            )?;
            Self::retain_intent(&mut intents, intent);
        }
        for signable in self.reducer.queued_signatures() {
            let intent =
                self.intent_from_signable(signable, wire::SumeragiV2OutboundIntentStage::Queued)?;
            Self::retain_intent(&mut intents, intent);
        }
        for message in self.reducer.outbound_messages() {
            if let Some(intent) =
                self.intent_from_message(message, wire::SumeragiV2OutboundIntentStage::Sent)?
            {
                Self::retain_intent(&mut intents, intent);
            }
        }
        Ok(intents.into_values().collect())
    }

    fn retain_intent(
        intents: &mut BTreeMap<
            wire::SumeragiV2OutboundIntentKind,
            wire::SumeragiV2OutboundIntentStatus,
        >,
        candidate: wire::SumeragiV2OutboundIntentStatus,
    ) {
        let candidate_rank = outbound_stage_rank(candidate.stage);
        match intents.get_mut(&candidate.kind) {
            Some(current)
                if candidate_rank < outbound_stage_rank(current.stage)
                    || (candidate_rank == outbound_stage_rank(current.stage)
                        && candidate.round.view > current.round.view) =>
            {
                *current = candidate;
            }
            Some(_) => {}
            None => {
                intents.insert(candidate.kind, candidate);
            }
        }
    }

    fn intent_from_wal_record(
        &self,
        record: &reducer::WalRecord,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        match record {
            reducer::WalRecord::ProposalIntent(proposal) => {
                self.intent_for_proposal(proposal, stage)
            }
            reducer::WalRecord::PrepareIntent(vote) => self.intent_for_vote(vote, stage),
            reducer::WalRecord::ObservePrepare(certificate) => {
                self.intent_for_certificate(certificate, stage)
            }
            reducer::WalRecord::LockAndCommit { vote, .. } => self.intent_for_vote(vote, stage),
            reducer::WalRecord::TimeoutIntent(vote) => {
                Ok(self.intent_for_timeout_vote(vote, stage))
            }
            reducer::WalRecord::InstallTimeout(certificate) => {
                Ok(self.intent_for_timeout_certificate(certificate, stage))
            }
            reducer::WalRecord::Decision(certificate) => {
                self.intent_for_certificate(certificate, stage)
            }
        }
    }

    fn intent_from_signable(
        &self,
        signable: &reducer::SignableMessage,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        match signable {
            reducer::SignableMessage::Proposal(proposal) => {
                self.intent_for_proposal(proposal, stage)
            }
            reducer::SignableMessage::Vote(vote) => self.intent_for_vote(vote, stage),
            reducer::SignableMessage::TimeoutVote(vote) => {
                Ok(self.intent_for_timeout_vote(vote, stage))
            }
        }
    }

    fn intent_from_message(
        &self,
        message: &reducer::ConsensusMessageV2,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<Option<wire::SumeragiV2OutboundIntentStatus>, AdapterError> {
        let intent = match message {
            reducer::ConsensusMessageV2::Proposal(proposal) => {
                self.intent_for_proposal(proposal.proposal(), stage)?
            }
            reducer::ConsensusMessageV2::Vote(vote) => self.intent_for_vote(&vote.vote(), stage)?,
            reducer::ConsensusMessageV2::QuorumCertificate(certificate) => {
                self.intent_for_certificate(certificate, stage)?
            }
            reducer::ConsensusMessageV2::TimeoutVote(vote) => {
                self.intent_for_timeout_vote(&vote.vote(), stage)
            }
            reducer::ConsensusMessageV2::TimeoutCertificate(certificate) => {
                self.intent_for_timeout_certificate(certificate, stage)
            }
            reducer::ConsensusMessageV2::BodyRequest(_)
            | reducer::ConsensusMessageV2::BodyChunk(_) => return Ok(None),
        };
        Ok(Some(intent))
    }

    fn intent_for_proposal(
        &self,
        proposal: &reducer::Proposal,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        Ok(wire::SumeragiV2OutboundIntentStatus {
            kind: wire::SumeragiV2OutboundIntentKind::Proposal,
            round: self.registry.round_to_wire(proposal.round()),
            proposal_round: Some(self.registry.round_to_wire(proposal.round())),
            subject: Some(self.registry.subject(proposal.manifest().subject())?),
            execution_commitment: None,
            stage,
        })
    }

    fn intent_for_vote(
        &self,
        vote: &reducer::Vote,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        Ok(wire::SumeragiV2OutboundIntentStatus {
            kind: match vote.phase() {
                reducer::Phase::Prepare => wire::SumeragiV2OutboundIntentKind::PrepareVote,
                reducer::Phase::Commit => wire::SumeragiV2OutboundIntentKind::CommitVote,
            },
            round: self.registry.round_to_wire(vote.round()),
            proposal_round: Some(self.registry.round_to_wire(vote.proposal_round())),
            subject: Some(self.registry.subject(vote.subject())?),
            execution_commitment: Some(
                self.registry
                    .execution_commitment(vote.proposal_round(), vote.subject())?,
            ),
            stage,
        })
    }

    fn intent_for_certificate(
        &self,
        certificate: &reducer::QuorumCertificate,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> Result<wire::SumeragiV2OutboundIntentStatus, AdapterError> {
        Ok(wire::SumeragiV2OutboundIntentStatus {
            kind: match certificate.phase() {
                reducer::Phase::Prepare => wire::SumeragiV2OutboundIntentKind::PrepareQc,
                reducer::Phase::Commit => wire::SumeragiV2OutboundIntentKind::CommitQc,
            },
            round: self.registry.round_to_wire(certificate.round()),
            proposal_round: Some(self.registry.round_to_wire(certificate.proposal_round())),
            subject: Some(self.registry.subject(certificate.subject())?),
            execution_commitment: Some(
                self.registry
                    .execution_commitment(certificate.proposal_round(), certificate.subject())?,
            ),
            stage,
        })
    }

    fn intent_for_timeout_vote(
        &self,
        vote: &reducer::TimeoutVote,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> wire::SumeragiV2OutboundIntentStatus {
        wire::SumeragiV2OutboundIntentStatus {
            kind: wire::SumeragiV2OutboundIntentKind::TimeoutVote,
            round: self.registry.round_to_wire(vote.round()),
            proposal_round: None,
            subject: None,
            execution_commitment: None,
            stage,
        }
    }

    fn intent_for_timeout_certificate(
        &self,
        certificate: &reducer::TimeoutCertificate,
        stage: wire::SumeragiV2OutboundIntentStage,
    ) -> wire::SumeragiV2OutboundIntentStatus {
        wire::SumeragiV2OutboundIntentStatus {
            kind: wire::SumeragiV2OutboundIntentKind::TimeoutCertificate,
            round: self.registry.round_to_wire(certificate.round()),
            proposal_round: None,
            subject: None,
            execution_commitment: None,
            stage,
        }
    }

    fn local_work_status(&self) -> wire::SumeragiV2WorkStatus {
        use wire::SumeragiV2LocalWorkStage::{Complete, Idle, Queued};

        let durable = self.reducer.durable_state();
        let decision = durable.decision();
        let applied = decision.is_some_and(|certificate| {
            self.reducer.applied_subject() == Some(certificate.subject())
        });
        let mut work = wire::SumeragiV2WorkStatus {
            candidate: if self.active_subject.is_some() {
                Complete
            } else {
                Idle
            },
            application: if decision.is_some() {
                if applied { Complete } else { Queued }
            } else {
                Idle
            },
            successor_height: if applied { Queued } else { Idle },
            ..wire::SumeragiV2WorkStatus::default()
        };
        if let Some((round, subject)) = self.active_subject {
            match self.reducer.body_state(round, subject) {
                reducer::BodyState::Missing => work.body_recovery = Queued,
                reducer::BodyState::Available => {
                    work.body_recovery = Complete;
                    work.body_store = Queued;
                }
                reducer::BodyState::Durable => {
                    work.body_recovery = Complete;
                    work.body_store = Complete;
                    work.validation = Queued;
                }
                reducer::BodyState::Validated | reducer::BodyState::Invalid => {
                    work.body_recovery = Complete;
                    work.body_store = Complete;
                    work.validation = Complete;
                }
            }
        }
        work
    }

    fn adapter_queue_statuses(&self) -> Vec<wire::SumeragiV2QueueStatus> {
        let now = Instant::now();
        let ingress_oldest = self
            .ingress_equivocations
            .values()
            .map(|record| record.admitted_at)
            .min();
        let progress_capacity = deferred_progress_capacity(self.wire_context.roster.len());
        vec![
            queue_status(
                wire::SumeragiV2QueueKind::Ingress,
                self.ingress_equivocations.len(),
                semantic_ingress_capacity(self.wire_context.roster.len()),
                ingress_oldest.map(|oldest| now.saturating_duration_since(oldest)),
                0,
            ),
            deferred_queue_status(
                wire::SumeragiV2QueueKind::DeferredNormal,
                &self.deferred_inputs,
                MAX_DEFERRED_INPUTS,
                now,
            ),
            deferred_queue_status(
                wire::SumeragiV2QueueKind::DeferredProgress,
                &self.deferred_progress_inputs,
                progress_capacity,
                now,
            ),
            deferred_queue_status(
                wire::SumeragiV2QueueKind::DeferredCompletion,
                &self.deferred_completions,
                MAX_DEFERRED_INPUTS,
                now,
            ),
        ]
    }

    fn serviced_candidate(
        &self,
        event: &reducer::Event,
        _priority: DeferredPriority,
        completion_evidence: Option<&BodyPipelineCompletionEvidence>,
        authenticated_wire_identity: Option<&[u8]>,
    ) -> Option<(ServicedCandidateKey, wire::View, ServicedCandidatePolicy)> {
        let policy = serviced_candidate_policy(event)?;
        let service_view = self.reducer.current_tag().view();
        let (source_view, target, phase) = serviced_candidate_event_fields(event);
        let leader = self.wire_context.leader(source_view);
        let owner: [u8; 32] = self.fingerprints.node.into();
        let mut projection = Vec::new();
        append_deferred_projection_field(&mut projection, &self.wire_context.id().encode());
        append_deferred_projection_u64(&mut projection, self.wire_context.height);
        append_deferred_projection_field(&mut projection, &owner);
        append_deferred_projection_field(&mut projection, &leader.encode());
        append_deferred_projection_u64(&mut projection, source_view);
        match target {
            Some(target) => {
                projection.push(1);
                append_deferred_projection_field(&mut projection, &target);
            }
            None => projection.push(0),
        }
        projection.push(phase);
        append_serviced_candidate_event(&mut projection, event);
        append_deferred_projection_completion_evidence(&mut projection, completion_evidence);
        // Authenticated network inputs have one semantic reducer occurrence
        // even when another valid signature, quorum subset, nested aggregate,
        // or canonical envelope carries them. The exact raw carrier remains
        // bound to deferred ownership and is revalidated immediately before
        // reducer service; after successful service it must not create a new
        // logical tombstone. Local completion evidence remains exact below.
        let carrier_identity = if matches!(
            event,
            reducer::Event::ProposalReceived { .. }
                | reducer::Event::VoteReceived { .. }
                | reducer::Event::QuorumCertificateReceived { .. }
                | reducer::Event::TimeoutVoteReceived { .. }
                | reducer::Event::TimeoutCertificateReceived { .. }
        ) {
            None
        } else {
            authenticated_wire_identity
        };
        match carrier_identity {
            Some(identity) => {
                projection.push(1);
                append_deferred_projection_field(&mut projection, identity);
            }
            None => projection.push(0),
        }
        let evidence: [u8; 32] = Hash::new(projection).into();
        Some((
            ServicedCandidateKey::new(
                self.wire_context.id(),
                self.wire_context.height,
                owner,
                leader,
                source_view,
                target,
                phase,
                ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
                deferred_event_kind(event).code(),
                evidence,
            ),
            service_view,
            policy,
        ))
    }

    fn fail_serviced_candidate_store(&mut self, reason: String) -> AdapterError {
        self.fail_closed = true;
        AdapterError::ServicedCandidateStore(reason)
    }

    /// Bind the immutable lifecycle selected by the serialized runtime to the
    /// next adapter transition.
    ///
    /// The runtime has already validated this carrier against its scheduler
    /// sidecar. Keeping the seam explicit prevents direct adapter tests and
    /// startup replay from accidentally minting production ownership.
    pub(crate) fn bind_selected_producer_lifecycle(
        &mut self,
        causal_lifecycle_key: Hash,
        admission_ordinal: u128,
    ) -> Result<(), AdapterError> {
        if admission_ordinal == 0 || self.selected_producer_lifecycle.is_some() {
            return Err(self.fail_serviced_candidate_store(
                "selected producer lifecycle was zero or already bound".to_owned(),
            ));
        }
        self.selected_producer_lifecycle = Some(SelectedProducerLifecycle {
            causal_lifecycle_key,
            admission_ordinal,
        });
        Ok(())
    }

    /// Clear the one-transition runtime binding.
    pub(crate) fn clear_selected_producer_lifecycle(&mut self) {
        self.selected_producer_lifecycle = None;
    }

    fn producer_lifecycle_slot(
        &self,
        candidate: ServicedCandidateKey,
        selected: &SelectedProducerLifecycle,
    ) -> Result<u64, String> {
        let mut existing_slot = None;
        for record in self.producer_continuations.values().filter(|record| {
            let identity = record.identity();
            identity.admission_ordinal() == selected.admission_ordinal
                && identity.causal_lifecycle_key() == selected.causal_lifecycle_key
        }) {
            let slot = record.identity().address().lifecycle_slot();
            if existing_slot
                .replace(slot)
                .is_some_and(|existing| existing != slot)
            {
                return Err("one producer lifecycle occupied multiple bounded slots".to_owned());
            }
        }
        if let Some(slot) = existing_slot {
            return Ok(slot);
        }

        (1..=self.producer_continuation_lifecycle_capacity)
            .find(|slot| {
                self.producer_continuations
                    .values()
                    .filter(|record| record.identity().address().lifecycle_slot() == *slot)
                    .all(|record| {
                        let identity = record.identity();
                        record.status() == ProducerContinuationStatus::Terminal
                            && identity.admission_ordinal() < selected.admission_ordinal
                            && identity.candidate().source_view() < candidate.source_view()
                    })
            })
            .ok_or_else(|| "bounded producer lifecycle slots are exhausted".to_owned())
    }

    /// Reserve the exact selected lifecycle-stage address before reducer
    /// service can retire its source.
    fn reserve_selected_producer_continuation(
        &mut self,
        candidate: Option<(ServicedCandidateKey, wire::View, ServicedCandidatePolicy)>,
    ) -> Result<Option<ProducerReservationToken>, AdapterError> {
        let (Some((candidate, _, _)), Some(selected)) =
            (candidate, self.selected_producer_lifecycle.clone())
        else {
            return Ok(None);
        };
        let existing = self
            .producer_continuations
            .iter()
            .filter(|(_, record)| record.identity().candidate() == candidate)
            .map(|(address, _)| *address)
            .collect::<Vec<_>>();
        match existing.as_slice() {
            [address] => {
                let record = self.producer_continuations[address].clone();
                if self.durable_producer_continuations.get(address) != Some(&record) {
                    return Err(self.fail_serviced_candidate_store(
                        "active producer identity was not present in durable admission metadata"
                            .to_owned(),
                    ));
                }
                if record.status() != ProducerContinuationStatus::Reserved {
                    return Err(self.fail_serviced_candidate_store(
                        "a terminal producer identity reached live reservation".to_owned(),
                    ));
                }
                let identity = record.identity();
                if identity.admission_ordinal() != selected.admission_ordinal
                    || identity.causal_lifecycle_key() != selected.causal_lifecycle_key
                {
                    return Err(self.fail_serviced_candidate_store(
                        "replayed producer lifecycle changed its immutable key or ordinal"
                            .to_owned(),
                    ));
                }
                let change = if self.restored_dormant_producer_continuations.remove(address) {
                    ProducerReservationChange::ClaimedDormant
                } else {
                    ProducerReservationChange::Unchanged
                };
                if matches!(change, ProducerReservationChange::ClaimedDormant)
                    && (self
                        .deferred_producer_continuations
                        .values()
                        .any(|reservation| reservation.address == *address)
                        || self.pending_producer_handoffs.contains_key(address))
                {
                    self.restored_dormant_producer_continuations
                        .insert(*address);
                    return Err(self.fail_serviced_candidate_store(
                        "restart-dormant producer already had a live process alias".to_owned(),
                    ));
                }
                return Ok(Some(ProducerReservationToken {
                    address: *address,
                    change,
                }));
            }
            [] => {}
            _ => {
                return Err(self.fail_serviced_candidate_store(
                    "one logical producer candidate occupied multiple bounded addresses".to_owned(),
                ));
            }
        }
        let lifecycle_slot = self
            .producer_lifecycle_slot(candidate, &selected)
            .map_err(|reason| self.fail_serviced_candidate_store(reason))?;
        let identity = ProducerContinuationIdentity::new(
            candidate,
            selected.causal_lifecycle_key,
            lifecycle_slot,
            selected.admission_ordinal,
        )
        .map_err(|reason| self.fail_serviced_candidate_store(reason))?;
        let address = identity.address();
        let record = ProducerContinuationRecord::new(
            identity,
            ProducerContinuationStatus::Reserved,
            Vec::new(),
        )
        .map_err(|reason| self.fail_serviced_candidate_store(reason))?;
        let process_previous = self.producer_continuations.get(&address).cloned();
        let reservation = self
            .serviced_candidate_store
            .reserve_producer_continuation(&mut self.producer_continuations, record);
        let reservation = match reservation {
            Ok(reservation) => reservation,
            Err(reason) => return Err(self.fail_serviced_candidate_store(reason)),
        };
        let durable_previous = self
            .durable_producer_continuations
            .insert(address, self.producer_continuations[&address].clone());
        let change = match reservation {
            ProducerContinuationReservation::Inserted => ProducerReservationChange::Inserted,
            ProducerContinuationReservation::Coalesced => ProducerReservationChange::Unchanged,
            ProducerContinuationReservation::ReplacedTerminal => {
                ProducerReservationChange::ReplacedTerminal {
                    process_previous: process_previous.ok_or_else(|| {
                        self.fail_serviced_candidate_store(
                            "terminal replacement omitted its process incumbent".to_owned(),
                        )
                    })?,
                    durable_previous: durable_previous.clone(),
                }
            }
        };
        if let Err(reason) = self
            .serviced_candidate_store
            .persist_with_producer_continuations(
                &self.durable_serviced_candidates,
                &self.durable_producer_continuations,
                self.serviced_candidates_decision_reclaimed,
            )
        {
            match durable_previous {
                Some(previous) => {
                    self.durable_producer_continuations
                        .insert(address, previous);
                }
                None => {
                    self.durable_producer_continuations.remove(&address);
                }
            }
            match &change {
                ProducerReservationChange::Unchanged => {}
                ProducerReservationChange::Inserted => {
                    self.producer_continuations.remove(&address);
                }
                ProducerReservationChange::ClaimedDormant => {
                    self.restored_dormant_producer_continuations.insert(address);
                }
                ProducerReservationChange::ReplacedTerminal {
                    process_previous, ..
                } => {
                    self.producer_continuations
                        .insert(address, process_previous.clone());
                }
            }
            return Err(self.fail_serviced_candidate_store(reason));
        }
        Ok(Some(ProducerReservationToken { address, change }))
    }

    fn persist_producer_lifecycles(&mut self) -> Result<(), AdapterError> {
        self.serviced_candidate_store
            .persist_with_producer_continuations(
                &self.durable_serviced_candidates,
                &self.durable_producer_continuations,
                self.serviced_candidates_decision_reclaimed,
            )
            .map_err(|reason| self.fail_serviced_candidate_store(reason))
    }

    fn rollback_producer_reservation(
        &mut self,
        token: Option<ProducerReservationToken>,
    ) -> Result<(), AdapterError> {
        let Some(token) = token else {
            return Ok(());
        };
        self.pending_producer_handoffs.remove(&token.address);
        match token.change {
            ProducerReservationChange::Unchanged => return Ok(()),
            ProducerReservationChange::Inserted => {
                self.producer_continuations.remove(&token.address);
                self.durable_producer_continuations.remove(&token.address);
            }
            ProducerReservationChange::ClaimedDormant => {
                self.restored_dormant_producer_continuations
                    .insert(token.address);
            }
            ProducerReservationChange::ReplacedTerminal {
                process_previous,
                durable_previous,
            } => {
                self.producer_continuations
                    .insert(token.address, process_previous);
                match durable_previous {
                    Some(previous) => {
                        self.durable_producer_continuations
                            .insert(token.address, previous);
                    }
                    None => {
                        self.durable_producer_continuations.remove(&token.address);
                    }
                }
            }
        }
        self.persist_producer_lifecycles()
    }

    fn release_unrecorded_producer(
        &mut self,
        token: Option<ProducerReservationToken>,
    ) -> Result<(), AdapterError> {
        let Some(token) = token else {
            return Ok(());
        };
        self.pending_producer_handoffs.remove(&token.address);
        match token.change {
            ProducerReservationChange::Unchanged
            | ProducerReservationChange::Inserted
            | ProducerReservationChange::ClaimedDormant => {
                self.producer_continuations.remove(&token.address);
                self.durable_producer_continuations.remove(&token.address);
                self.restored_dormant_producer_continuations
                    .remove(&token.address);
            }
            ProducerReservationChange::ReplacedTerminal {
                process_previous,
                durable_previous,
            } => {
                self.producer_continuations
                    .insert(token.address, process_previous);
                match durable_previous {
                    Some(previous) => {
                        self.durable_producer_continuations
                            .insert(token.address, previous);
                    }
                    None => {
                        self.durable_producer_continuations.remove(&token.address);
                    }
                }
            }
        }
        self.persist_producer_lifecycles()
    }

    fn terminalize_producer_continuation(
        &mut self,
        address: Option<ProducerContinuationAddress>,
    ) -> Result<Option<ProducerContinuationRecord>, AdapterError> {
        let Some(address) = address else {
            return Ok(None);
        };
        let Some(previous) = self.producer_continuations.get(&address).cloned() else {
            return Err(self.fail_serviced_candidate_store(
                "selected producer reservation disappeared before terminalization".to_owned(),
            ));
        };
        if previous.status() == ProducerContinuationStatus::Terminal {
            return Ok(Some(previous));
        }
        let terminal = ProducerContinuationRecord::new(
            previous.identity(),
            ProducerContinuationStatus::Terminal,
            Vec::new(),
        )
        .map_err(|reason| self.fail_serviced_candidate_store(reason))?;
        self.producer_continuations.insert(address, terminal);
        Ok(Some(previous))
    }

    /// Close producer reservations whose adapter-owned Busy occurrence was
    /// retired by an exact state-changing corridor update.
    ///
    /// Queue-retirement APIs intentionally keep their existing infallible
    /// signatures. Any impossible missing/corrupt reservation latches the
    /// adapter fail-closed through `terminalize_producer_continuation`; the
    /// next ingress or executor turn then reports that state.
    fn retire_unowned_deferred_producer_continuations(&mut self) {
        let active = self.all_deferred_admission_ordinals();
        let retired = self
            .deferred_producer_continuations
            .keys()
            .filter(|ordinal| !active.contains(ordinal))
            .copied()
            .collect::<Vec<_>>();
        for ordinal in retired {
            let Some(reservation) = self.deferred_producer_continuations.remove(&ordinal) else {
                continue;
            };
            // A strict view advance or Decision removed the deferred source
            // before service. It is a goal/exited lifecycle, not a synthetic
            // successor acknowledgement. Restore any older durable incumbent
            // and otherwise reopen the bounded address for reconstructed work.
            if self
                .release_goal_reached_producer(Some(reservation))
                .is_err()
            {
                break;
            }
        }
    }

    /// Release a speculative active record when the same macro-step reached a
    /// durable goal (Decision or strict view advance) before a producer
    /// continuation was needed. If the active reservation temporarily
    /// replaced an older durable terminal at the same bounded address, restore
    /// that exact restart-safe incumbent in process memory.
    fn release_goal_reached_producer(
        &mut self,
        reservation: Option<ProducerReservationToken>,
    ) -> Result<(), AdapterError> {
        self.release_unrecorded_producer(reservation)
    }

    /// Reserve bounded serviced-identity capacity before mutating the reducer.
    ///
    /// The fast path needs no speculative reducer step. Only a theoretically
    /// full table projects the deterministic transition on a clone, allowing
    /// ignored Byzantine/stale traffic to remain marker-free while refusing a
    /// genuinely consuming transition before its physical owner is released.
    fn ensure_serviced_candidate_capacity_before_step(
        &mut self,
        event: &reducer::Event,
        candidate: Option<(ServicedCandidateKey, wire::View, ServicedCandidatePolicy)>,
    ) -> Result<(), AdapterError> {
        let Some((key, _, _)) = candidate else {
            return Ok(());
        };
        if self.serviced_candidates.contains_key(&key) {
            return Ok(());
        }
        let capacity = self.serviced_candidate_capacity;
        if self.serviced_candidates.len() < capacity {
            return Ok(());
        }

        let mut projected = self.reducer.clone();
        let disposition = projected.step(event.clone())?.disposition();
        if serviced_candidate_record_kind(event, disposition).is_none() {
            return Ok(());
        }
        Err(self.fail_serviced_candidate_store(format!(
            "derived serviced-candidate capacity {capacity} is exhausted before semantic service"
        )))
    }

    /// Mark one non-Busy reducer occurrence before its final owner is returned
    /// to the caller or serialized runtime.
    ///
    /// Applied occurrences retain a process-generation marker through the
    /// same-view episode. Only an exact internal callback drained after its
    /// asynchronous item disappeared also receives the restart-stable marker.
    fn record_serviced_candidate(
        &mut self,
        candidate: Option<(ServicedCandidateKey, wire::View, ServicedCandidatePolicy)>,
        durable_terminal_retirement: bool,
        durable_terminal_evidence: bool,
        producer_reservation: Option<ProducerReservationToken>,
    ) -> Result<Option<ProducerContinuationHandoffToken>, AdapterError> {
        let Some((key, service_view, _)) = candidate else {
            self.release_unrecorded_producer(producer_reservation)?;
            return Ok(None);
        };
        if self.reducer.durable_state().decision().is_some() {
            // Decision closes this height's candidate-service episode. The
            // reducer's durable Decision owns all remaining application and
            // replay progress, so no post-Decision occurrence may recreate a
            // tombstone reclaimed by that same macro-step.
            self.release_goal_reached_producer(producer_reservation)?;
            return Ok(None);
        }
        if service_view < self.reducer.current_tag().view() {
            // The same macro-step durably advanced the view and reclaimed the
            // completed old-view epoch before this owner reached its return
            // seam. Recreating that obsolete key would undo strict-view
            // reclamation.
            self.release_goal_reached_producer(producer_reservation)?;
            return Ok(None);
        }
        let capacity = self.serviced_candidate_capacity;
        let process_marker_exists = self.serviced_candidates.contains_key(&key);
        if !process_marker_exists && self.serviced_candidates.len() >= capacity {
            return Err(self.fail_serviced_candidate_store(format!(
                "derived serviced-candidate capacity {capacity} is exhausted"
            )));
        }
        if !process_marker_exists {
            assert_eq!(self.serviced_candidates.insert(key, service_view), None);
        }
        let Some(reservation) = producer_reservation else {
            if !durable_terminal_retirement || self.durable_serviced_candidates.contains_key(&key) {
                return Ok(None);
            }
            if self.durable_serviced_candidates.len() >= capacity {
                if !process_marker_exists {
                    self.serviced_candidates.remove(&key);
                }
                return Err(self.fail_serviced_candidate_store(format!(
                    "derived durable serviced-candidate capacity {capacity} is exhausted"
                )));
            }
            assert_eq!(
                self.durable_serviced_candidates.insert(key, service_view),
                None
            );
            if let Err(reason) = self
                .serviced_candidate_store
                .persist_with_producer_continuations(
                    &self.durable_serviced_candidates,
                    &self.durable_producer_continuations,
                    self.serviced_candidates_decision_reclaimed,
                )
            {
                self.durable_serviced_candidates.remove(&key);
                if !process_marker_exists {
                    self.serviced_candidates.remove(&key);
                }
                return Err(self.fail_serviced_candidate_store(reason));
            }
            return Ok(None);
        };
        let address = reservation.address;
        let token = self
            .producer_continuations
            .get(&address)
            .and_then(ProducerContinuationRecord::handoff_token)
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "selected producer reservation was not live at the runtime handoff".to_owned(),
                )
            })?;
        let pending = PendingProducerHandoff {
            token,
            service_view,
            durable_store_terminal: durable_terminal_retirement,
            durable_terminal_evidence,
            durable_previous: match reservation.change {
                ProducerReservationChange::ReplacedTerminal {
                    durable_previous, ..
                } => durable_previous,
                ProducerReservationChange::Unchanged
                | ProducerReservationChange::Inserted
                | ProducerReservationChange::ClaimedDormant => None,
            },
        };
        match self.pending_producer_handoffs.get(&address) {
            Some(existing) if *existing != pending => {
                return Err(self.fail_serviced_candidate_store(
                    "an exact producer handoff changed its terminal policy".to_owned(),
                ));
            }
            Some(_) => {}
            None => {
                self.pending_producer_handoffs.insert(address, pending);
            }
        }
        Ok(Some(token))
    }

    /// Classify the exact replacement evidence retained for one pending handoff.
    ///
    /// A non-empty effect batch is a concrete causal successor. An empty batch
    /// is restart-stable only when the source retained independent durable
    /// terminal evidence; every other empty last consumer is explicitly
    /// volatile and reopens after same-height restart.
    pub(crate) fn producer_handoff_evidence(
        &self,
        token: ProducerContinuationHandoffToken,
        has_concrete_successor: bool,
    ) -> Result<ProducerContinuationHandoffEvidence, AdapterError> {
        let pending = self
            .pending_producer_handoffs
            .get(&token.address())
            .ok_or(AdapterError::RuntimeIngressOwnershipViolation)?;
        if pending.token != token {
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        Ok(if has_concrete_successor {
            ProducerContinuationHandoffEvidence::ConcreteSuccessor
        } else if pending.durable_terminal_evidence {
            ProducerContinuationHandoffEvidence::DurableTerminal
        } else {
            ProducerContinuationHandoffEvidence::VolatileTerminal
        })
    }

    /// Consume one exact runtime handoff after its replacement owner exists.
    ///
    /// The opaque token is checked against both the live continuation record
    /// and the pending service metadata. Durable publication commits the
    /// service tombstone and producer terminal in one source-sealed snapshot.
    pub(crate) fn acknowledge_producer_handoff(
        &mut self,
        token: ProducerContinuationHandoffToken,
        evidence: ProducerContinuationHandoffEvidence,
    ) -> Result<ProducerContinuationTerminalToken, AdapterError> {
        let address = token.address();
        let pending = self
            .pending_producer_handoffs
            .get(&address)
            .cloned()
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "producer handoff acknowledgement had no pending reservation".to_owned(),
                )
            })?;
        let record = self
            .producer_continuations
            .get(&address)
            .cloned()
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "producer handoff acknowledgement lost its reservation".to_owned(),
                )
            })?;
        if pending.token != token || !token.matches_reserved(&record) {
            return Err(self.fail_serviced_candidate_store(
                "producer handoff acknowledgement changed exact identity".to_owned(),
            ));
        }
        if evidence == ProducerContinuationHandoffEvidence::DurableTerminal
            && !pending.durable_terminal_evidence
        {
            return Err(self.fail_serviced_candidate_store(
                "producer handoff claimed terminal evidence not retained by its source".to_owned(),
            ));
        }
        if evidence == ProducerContinuationHandoffEvidence::VolatileTerminal
            && pending.durable_terminal_evidence
        {
            return Err(self.fail_serviced_candidate_store(
                "producer handoff weakened retained durable terminal evidence".to_owned(),
            ));
        }
        let previous = self
            .terminalize_producer_continuation(Some(address))?
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "producer handoff terminalization returned no incumbent".to_owned(),
                )
            })?;
        let terminal = self
            .producer_continuations
            .get(&address)
            .cloned()
            .ok_or_else(|| {
                self.fail_serviced_candidate_store(
                    "producer handoff terminal disappeared after terminalization".to_owned(),
                )
            })?;
        if pending.durable_store_terminal {
            let key = token.identity().candidate();
            let previous_service = self
                .durable_serviced_candidates
                .insert(key, pending.service_view);
            let previous_durable = self
                .durable_producer_continuations
                .insert(address, terminal.clone());
            if let Err(reason) = self
                .serviced_candidate_store
                .persist_with_producer_continuations(
                    &self.durable_serviced_candidates,
                    &self.durable_producer_continuations,
                    self.serviced_candidates_decision_reclaimed,
                )
            {
                match previous_service {
                    Some(view) => {
                        self.durable_serviced_candidates.insert(key, view);
                    }
                    None => {
                        self.durable_serviced_candidates.remove(&key);
                    }
                }
                match previous_durable {
                    Some(record) => {
                        self.durable_producer_continuations.insert(address, record);
                    }
                    None => {
                        self.durable_producer_continuations.remove(&address);
                    }
                }
                self.producer_continuations.insert(address, previous);
                return Err(self.fail_serviced_candidate_store(reason));
            }
        } else {
            match pending.durable_previous.clone() {
                Some(previous) => {
                    self.durable_producer_continuations
                        .insert(address, previous);
                }
                None => {
                    self.durable_producer_continuations.remove(&address);
                }
            }
            if let Err(error) = self.persist_producer_lifecycles() {
                self.durable_producer_continuations.insert(address, record);
                self.producer_continuations.insert(address, previous);
                return Err(error);
            }
        }
        self.pending_producer_handoffs.remove(&address);
        self.restored_dormant_producer_continuations
            .remove(&address);
        terminal.terminal_token().ok_or_else(|| {
            self.fail_serviced_candidate_store(
                "producer handoff did not produce an exact terminal token".to_owned(),
            )
        })
    }

    /// Return the safety-WAL replay cut used to reconcile generic ingress.
    ///
    /// The adapter is opened before the adjacent leader-wire store. Its
    /// current view is therefore backed by replayed timeout-certificate state,
    /// and its Decision bit is backed by the durable Decision record. The
    /// opaque capability lets the generic store retire only records whose
    /// protocol episode is already impossible to re-enter.
    pub(crate) fn leader_wire_recovery_authority(
        &self,
    ) -> Result<LeaderWireRecoveryAuthority, AdapterError> {
        self.ensure_ingress()?;
        let owner: [u8; 32] = self.fingerprints.node.into();
        Ok(LeaderWireRecoveryAuthority::from_replayed_adapter(
            self.wire_context.id(),
            self.wire_context.height,
            owner,
            self.reducer.current_tag().view(),
            self.reducer.durable_state().decision().is_some(),
        ))
    }

    /// Read-only restart-stable producer terminals restored from the adjacent
    /// serviced-candidate snapshot.
    pub(crate) fn durable_producer_terminal_tokens(
        &self,
    ) -> Vec<ProducerContinuationTerminalToken> {
        self.durable_producer_continuations
            .values()
            .filter_map(ProducerContinuationRecord::terminal_token)
            .collect()
    }

    /// Reclaim only epochs made obsolete by a strict certified view advance
    /// or by the first durable Decision in this height.
    fn reclaim_serviced_candidates(&mut self) -> Result<(), AdapterError> {
        let current_view = self.reducer.current_tag().view();
        let decision_durable = self.reducer.durable_state().decision().is_some();
        if self.serviced_candidates_decision_reclaimed && !decision_durable {
            return Err(self.fail_serviced_candidate_store(
                "snapshot claims durable-Decision reclamation before a durable Decision".to_owned(),
            ));
        }
        self.serviced_candidates
            .retain(|_, service_view| *service_view >= current_view);
        let previous_durable_len = self.durable_serviced_candidates.len();
        let previous_durable_producer_len = self.durable_producer_continuations.len();
        self.durable_serviced_candidates
            .retain(|_, service_view| *service_view >= current_view);
        if !decision_durable {
            // A strict certified view advance is itself the durable reason an
            // older lifecycle cannot re-enter. Remove its paired producer
            // tombstone whenever the exact service tombstone is reclaimed so
            // every non-Decision snapshot keeps the two tables atomic.
            self.durable_producer_continuations.retain(|_, record| {
                if record.status() == ProducerContinuationStatus::Terminal {
                    self.durable_serviced_candidates
                        .contains_key(&record.identity().candidate())
                } else {
                    record.identity().candidate().source_view() >= current_view
                }
            });
        }
        let mut durable_changed = self.durable_serviced_candidates.len() != previous_durable_len
            || self.durable_producer_continuations.len() != previous_durable_producer_len;
        if decision_durable && !self.serviced_candidates_decision_reclaimed {
            self.serviced_candidates.clear();
            self.durable_serviced_candidates.clear();
            self.producer_continuations.clear();
            self.durable_producer_continuations.clear();
            self.restored_dormant_producer_continuations.clear();
            self.deferred_producer_continuations.clear();
            self.pending_producer_handoffs.clear();
            self.serviced_candidates_decision_reclaimed = true;
            durable_changed = true;
        }
        let retired_dormant = self
            .restored_dormant_producer_continuations
            .iter()
            .filter(|address| !self.durable_producer_continuations.contains_key(address))
            .copied()
            .collect::<Vec<_>>();
        for address in retired_dormant {
            self.restored_dormant_producer_continuations
                .remove(&address);
            self.producer_continuations.remove(&address);
        }
        debug_assert!(
            self.durable_serviced_candidates
                .keys()
                .all(|key| self.serviced_candidates.contains_key(key))
        );
        if durable_changed
            && let Err(reason) = self
                .serviced_candidate_store
                .persist_with_producer_continuations(
                    &self.durable_serviced_candidates,
                    &self.durable_producer_continuations,
                    self.serviced_candidates_decision_reclaimed,
                )
        {
            return Err(self.fail_serviced_candidate_store(reason));
        }
        Ok(())
    }

    #[cfg(test)]
    fn serviced_candidate_count_for_test(&self) -> usize {
        self.serviced_candidates.len()
    }

    #[cfg(test)]
    fn serviced_candidate_store_path_for_test(&self) -> &std::path::Path {
        self.serviced_candidate_store.path_for_test()
    }

    #[cfg(test)]
    fn serviced_candidate_views_for_test(&self) -> BTreeSet<wire::View> {
        self.serviced_candidates.values().copied().collect()
    }

    fn ensure_ingress(&self) -> Result<(), AdapterError> {
        if self.fail_closed {
            Err(AdapterError::FailClosed)
        } else if !self.replay_complete {
            Err(AdapterError::ReplayNotComplete)
        } else {
            Ok(())
        }
    }

    fn step(&mut self, event: reducer::Event) -> Result<AdapterOutcome, AdapterError> {
        self.step_with_completion_evidence(event, None)
    }

    fn step_with_completion_evidence(
        &mut self,
        event: reducer::Event,
        completion_evidence: Option<BodyPipelineCompletionEvidence>,
    ) -> Result<AdapterOutcome, AdapterError> {
        let priority = match &event {
            reducer::Event::ResumeAfterReplay { .. }
            | reducer::Event::LocalProposalReady { .. }
            | reducer::Event::BodyAvailable { .. }
            | reducer::Event::BodyStored { .. }
            | reducer::Event::ValidationCompleted { .. }
            | reducer::Event::Persisted { .. }
            | reducer::Event::PersistenceFailed { .. }
            | reducer::Event::Signed { .. }
            | reducer::Event::ApplicationCompleted { .. }
            | reducer::Event::TimeoutElapsed { .. }
            | reducer::Event::RetransmitElapsed { .. } => DeferredPriority::Completion,
            reducer::Event::TimeoutVoteReceived { .. } => DeferredPriority::Progress,
            reducer::Event::ProposalReceived { .. }
            | reducer::Event::VoteReceived { .. }
            | reducer::Event::QuorumCertificateReceived { .. }
            | reducer::Event::TimeoutCertificateReceived { .. } => DeferredPriority::Normal,
        };
        self.step_with_defer_policy(event, false, priority, None, completion_evidence, None)
            .map(|result| result.outcome)
    }

    #[cfg(test)]
    fn step_authenticated_ingress(
        &mut self,
        event: reducer::Event,
        admission: Option<IngressAdmission>,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.step_authenticated_ingress_with_ownership(event, admission, None)
            .map(|result| result.outcome)
    }

    fn step_authenticated_ingress_with_ownership(
        &mut self,
        event: reducer::Event,
        admission: Option<IngressAdmission>,
        authenticated_wire_identity: Option<Arc<[u8]>>,
    ) -> Result<DeferPolicyOutcome, AdapterError> {
        let priority = if matches!(
            &event,
            reducer::Event::QuorumCertificateReceived { .. }
                | reducer::Event::TimeoutVoteReceived { .. }
                | reducer::Event::TimeoutCertificateReceived { .. }
        ) || admission.is_some_and(|admission| admission.locked_commit_progress)
        {
            DeferredPriority::Progress
        } else {
            DeferredPriority::Normal
        };
        self.step_with_defer_policy(
            event,
            true,
            priority,
            admission,
            None,
            authenticated_wire_identity,
        )
    }

    fn step_with_defer_policy(
        &mut self,
        event: reducer::Event,
        retag_authenticated_ingress: bool,
        priority: DeferredPriority,
        admission: Option<IngressAdmission>,
        completion_evidence: Option<BodyPipelineCompletionEvidence>,
        authenticated_wire_identity: Option<Arc<[u8]>>,
    ) -> Result<DeferPolicyOutcome, AdapterError> {
        self.ensure_ingress()?;
        let queued = event.clone();
        let serviced_candidate = self.serviced_candidate(
            &queued,
            priority,
            completion_evidence.as_ref(),
            authenticated_wire_identity.as_deref(),
        );
        if serviced_candidate.is_some_and(|(key, _, policy)| {
            policy == ServicedCandidatePolicy::Suppress
                && self.serviced_candidates.contains_key(&key)
        }) {
            if let Some(admission) = admission {
                self.record_ingress_delivery(admission);
            }
            let disposition = reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate);
            self.record_disposition(disposition);
            self.publish_status()?;
            self.log_body_progress(&queued, disposition, 0);
            return Ok(DeferPolicyOutcome {
                outcome: AdapterOutcome {
                    disposition,
                    effects: Vec::new(),
                    deferred_admission_ordinal: None,
                    producer_handoff: None,
                },
            });
        }
        let producer_stage = serviced_candidate_stage(&queued);
        let locally_reconstructible_producer =
            producer_stage.is_some_and(producer_parent_is_locally_reconstructible);
        if self.selected_producer_lifecycle.is_some()
            && serviced_candidate.is_some()
            && locally_reconstructible_producer
            && !producer_parent_has_exact_local_replay_binding(
                &queued,
                completion_evidence.as_ref(),
                self.reducer.durable_state().decision().is_some(),
            )
        {
            return Err(self.fail_serviced_candidate_store(
                "selected producer kind had no exact replayable parent binding".to_owned(),
            ));
        }
        // Every selected exact producer class reserves its immutable lifecycle
        // before the reducer step. Conditional transport and volatile-body
        // parents are reopened through the durable generic ingress token;
        // only the Local class additionally requires an immediate local replay
        // binding at this boundary.
        let producer_candidate = if producer_stage.is_some() {
            serviced_candidate
        } else {
            None
        };
        let producer_reservation =
            self.reserve_selected_producer_continuation(producer_candidate)?;
        if let Err(error) =
            self.ensure_serviced_candidate_capacity_before_step(&queued, serviced_candidate)
        {
            self.rollback_producer_reservation(producer_reservation)?;
            return Err(error);
        }
        let outcome = match self.reducer.step(event) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.rollback_producer_reservation(producer_reservation)?;
                return Err(error.into());
            }
        };
        let disposition = outcome.disposition();
        self.record_reducer_outcome(&queued, disposition, outcome.effects());
        if disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            self.log_body_progress(&queued, disposition, 0);
            let deferred_admission_ordinal = match self.enqueue_deferred(
                queued,
                retag_authenticated_ingress,
                priority,
                admission,
                completion_evidence,
                authenticated_wire_identity,
            ) {
                Ok(ordinal) => ordinal,
                Err(error) => {
                    self.rollback_producer_reservation(producer_reservation)?;
                    return Err(error);
                }
            };
            match (deferred_admission_ordinal, producer_reservation) {
                (Some(ordinal), Some(reservation)) => {
                    match self.deferred_producer_continuations.get(&ordinal) {
                        Some(existing) if existing.address != reservation.address => {
                            // `enqueue_deferred` coalesced with an older exact
                            // owner. Keep that owner's address and undo only
                            // this speculative reservation.
                            self.rollback_producer_reservation(Some(reservation))?;
                        }
                        Some(_) => {}
                        None => {
                            self.deferred_producer_continuations
                                .insert(ordinal, reservation);
                        }
                    }
                }
                (None, reservation) => self.rollback_producer_reservation(reservation)?,
                (Some(_), None) => {}
            }
            if deferred_admission_ordinal.is_some()
                && let Some(admission) = admission
            {
                self.record_ingress_delivery(admission);
            }
            self.publish_status()?;
            return Ok(DeferPolicyOutcome {
                outcome: AdapterOutcome {
                    disposition,
                    effects: Vec::new(),
                    deferred_admission_ordinal,
                    producer_handoff: None,
                },
            });
        }
        // Busy is the reducer's only retryable disposition. Every applied or
        // safely ignored authenticated input has crossed its consumer
        // boundary, so retain the delivery record and coalesce an exact
        // retransmission before conversion. A Commit ignored before its exact
        // lock is durable records an ordinary delivery; once that lock is
        // installed, `locked_commit_progress` changes the consumer epoch and
        // admits the same authenticated vote once in the current generation.
        // Later pool resets remain generation scoped.
        // One adapter invocation returns exactly one reducer macro-step. Busy-
        // deferred inputs remain adapter-owned and the serialized runtime
        // schedules them explicitly after this batch reaches the executor.
        // Concatenating them here would erase the reducer transition boundary
        // and could exceed the executor's retained-batch capacity.
        let effects = match self.drive_effects(outcome.into_effects()) {
            Ok(effects) => effects,
            Err(error) => {
                self.release_unrecorded_producer(producer_reservation)?;
                return Err(error);
            }
        };
        let record_kind = serviced_candidate_record_kind(&queued, disposition);
        let serviced_candidate = record_kind.and(serviced_candidate);
        let durable_terminal_retirement =
            record_kind == Some(ServicedCandidateRecordKind::DurableTerminal);
        let durable_terminal_evidence =
            durable_terminal_retirement || completion_evidence.is_some();
        let producer_handoff = if record_kind.is_some() {
            self.record_serviced_candidate(
                serviced_candidate,
                durable_terminal_retirement,
                durable_terminal_evidence,
                producer_reservation,
            )?
        } else {
            self.release_unrecorded_producer(producer_reservation)?;
            None
        };
        if let Some(admission) = admission {
            self.record_ingress_delivery(admission);
        }
        self.publish_status()?;
        self.log_body_progress(&queued, disposition, effects.len());
        Ok(DeferPolicyOutcome {
            outcome: AdapterOutcome {
                disposition,
                effects,
                deferred_admission_ordinal: None,
                producer_handoff,
            },
        })
    }

    fn record_ingress_delivery(&mut self, admission: IngressAdmission) {
        self.ingress_deliveries.insert(
            admission.key,
            IngressDeliveryRecord {
                fingerprint: admission.fingerprint,
                generation: admission.generation,
                locked_commit_progress: admission.locked_commit_progress,
            },
        );
    }

    fn record_disposition(&mut self, disposition: reducer::StepDisposition) {
        if let reducer::StepDisposition::Ignored(reason) = disposition {
            let count = self.ignore_counts.entry(reason).or_default();
            *count = count.saturating_add(1);
        }
    }

    fn record_reducer_outcome(
        &mut self,
        event: &reducer::Event,
        disposition: reducer::StepDisposition,
        effects: &[reducer::Effect],
    ) {
        self.record_disposition(disposition);
        if disposition != reducer::StepDisposition::Applied
            || effects
                .iter()
                .any(|effect| matches!(effect, reducer::Effect::ReportEquivocation { .. }))
        {
            return;
        }
        let current = reducer::Round::new(
            self.reducer.context().height(),
            self.reducer.current_tag().view(),
        );
        let progress = match event {
            reducer::Event::ResumeAfterReplay { .. } => Some((
                wire::SumeragiV2ProgressTransition::RecoveryReplayed,
                current,
            )),
            reducer::Event::LocalProposalReady { tag, .. } => Some((
                if effects
                    .iter()
                    .any(|effect| matches!(effect, reducer::Effect::Apply { .. }))
                {
                    wire::SumeragiV2ProgressTransition::BodyValidated
                } else {
                    wire::SumeragiV2ProgressTransition::ProposalAdmitted
                },
                reducer::Round::new(tag.height(), tag.view()),
            )),
            reducer::Event::ProposalReceived { proposal, .. } => Some((
                wire::SumeragiV2ProgressTransition::ProposalAdmitted,
                proposal.proposal().round(),
            )),
            reducer::Event::VoteReceived { vote, .. } => Some((
                match vote.vote().phase() {
                    reducer::Phase::Prepare => {
                        wire::SumeragiV2ProgressTransition::PrepareVoteAdmitted
                    }
                    reducer::Phase::Commit => {
                        wire::SumeragiV2ProgressTransition::CommitVoteAdmitted
                    }
                },
                vote.vote().round(),
            )),
            reducer::Event::QuorumCertificateReceived { certificate, .. } => Some((
                match certificate.phase() {
                    reducer::Phase::Prepare => wire::SumeragiV2ProgressTransition::PrepareQuorum,
                    reducer::Phase::Commit => wire::SumeragiV2ProgressTransition::CommitQuorum,
                },
                certificate.round(),
            )),
            reducer::Event::TimeoutVoteReceived { vote, .. } => Some((
                wire::SumeragiV2ProgressTransition::TimeoutVoteAdmitted,
                vote.vote().round(),
            )),
            reducer::Event::BodyAvailable { round, .. } => {
                Some((wire::SumeragiV2ProgressTransition::BodyAvailable, *round))
            }
            reducer::Event::BodyStored { round, .. } => {
                Some((wire::SumeragiV2ProgressTransition::BodyStored, *round))
            }
            reducer::Event::ValidationCompleted {
                round, valid: true, ..
            } => Some((wire::SumeragiV2ProgressTransition::BodyValidated, *round)),
            reducer::Event::ApplicationCompleted { .. } => {
                Some((wire::SumeragiV2ProgressTransition::Applied, current))
            }
            reducer::Event::Persisted { .. }
                if effects
                    .iter()
                    .any(|effect| matches!(effect, reducer::Effect::EnterView { .. })) =>
            {
                Some((
                    wire::SumeragiV2ProgressTransition::TimeoutCertificateInstalled,
                    current,
                ))
            }
            reducer::Event::Persisted { .. }
                if self.reducer.durable_state().decision().is_some() =>
            {
                Some((
                    wire::SumeragiV2ProgressTransition::DecisionPersisted,
                    self.reducer
                        .durable_state()
                        .decision()
                        .expect("guarded durable decision")
                        .round(),
                ))
            }
            reducer::Event::Persisted { .. }
                if self.reducer.durable_state().locked().is_some()
                    && effects.iter().any(|effect| {
                        matches!(
                            effect,
                            reducer::Effect::Sign {
                                message: reducer::SignableMessage::Vote(vote),
                                ..
                            } if vote.phase() == reducer::Phase::Commit
                        )
                    }) =>
            {
                Some((
                    wire::SumeragiV2ProgressTransition::LockInstalled,
                    self.reducer
                        .durable_state()
                        .locked()
                        .expect("guarded durable lock")
                        .round(),
                ))
            }
            reducer::Event::Signed { .. } => effects.iter().find_map(|effect| match effect {
                reducer::Effect::Broadcast(reducer::ConsensusMessageV2::Vote(vote)) => Some((
                    match vote.vote().phase() {
                        reducer::Phase::Prepare => {
                            wire::SumeragiV2ProgressTransition::PrepareVoteAdmitted
                        }
                        reducer::Phase::Commit => {
                            wire::SumeragiV2ProgressTransition::CommitVoteAdmitted
                        }
                    },
                    vote.vote().round(),
                )),
                reducer::Effect::Broadcast(reducer::ConsensusMessageV2::TimeoutVote(vote)) => {
                    Some((
                        wire::SumeragiV2ProgressTransition::TimeoutVoteAdmitted,
                        vote.vote().round(),
                    ))
                }
                _ => None,
            }),
            reducer::Event::TimeoutCertificateReceived { .. }
            | reducer::Event::TimeoutElapsed { .. }
            | reducer::Event::RetransmitElapsed { .. }
            | reducer::Event::Persisted { .. }
            | reducer::Event::ValidationCompleted { valid: false, .. }
            | reducer::Event::PersistenceFailed { .. } => None,
        };
        if let Some((transition, round)) = progress {
            self.last_progress = Some((self.reducer.generation(), round, transition));
        }
    }

    fn log_body_progress(
        &self,
        event: &reducer::Event,
        disposition: reducer::StepDisposition,
        effect_count: usize,
    ) {
        let (stage, round, subject, valid) = match event {
            reducer::Event::ProposalReceived { proposal, .. } => {
                let proposal = proposal.proposal();
                (
                    "proposal_received",
                    proposal.round(),
                    proposal.manifest().subject(),
                    None,
                )
            }
            reducer::Event::BodyAvailable { round, subject, .. } => {
                ("body_available", *round, *subject, None)
            }
            reducer::Event::BodyStored { round, subject, .. } => {
                ("body_stored", *round, *subject, None)
            }
            reducer::Event::ValidationCompleted {
                round,
                subject,
                valid,
                ..
            } => ("validation_completed", *round, *subject, Some(*valid)),
            _ => return,
        };
        let current_tag = self.current_tag();
        iroha_logger::debug!(
            stage,
            round_height = round.height(),
            round_view = round.view(),
            subject = ?subject,
            ?valid,
            ?disposition,
            effect_count,
            current_height = current_tag.height(),
            current_view = current_tag.view(),
            deferred_completions = self.deferred_completions.len(),
            deferred_progress = self.deferred_progress_inputs.len(),
            deferred_normal = self.deferred_inputs.len(),
            "processed Sumeragi v2 body-progress reducer input"
        );
    }

    fn enqueue_deferred(
        &mut self,
        event: reducer::Event,
        retag_authenticated_ingress: bool,
        priority: DeferredPriority,
        admission: Option<IngressAdmission>,
        completion_evidence: Option<BodyPipelineCompletionEvidence>,
        authenticated_wire_identity: Option<Arc<[u8]>>,
    ) -> Result<Option<u128>, AdapterError> {
        if retag_authenticated_ingress && authenticated_wire_identity.is_none() {
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        let protected_progress =
            admission.is_some_and(|admission| admission.locked_commit_progress);
        let mut input = DeferredInput {
            admission_ordinal: 0,
            admission_capability: DeferredAdmissionCapability::pending(),
            event,
            completion_evidence,
            retag_authenticated_ingress,
            priority,
            protected_progress,
            admission,
            authenticated_wire_identity,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        };
        let progress_capacity = deferred_progress_capacity(self.wire_context.roster.len());
        let duplicate_ordinal = match priority {
            DeferredPriority::Completion => self
                .deferred_completions
                .iter()
                .find(|queued| *queued == &input),
            DeferredPriority::Progress => self
                .deferred_progress_inputs
                .iter()
                .find(|queued| *queued == &input),
            DeferredPriority::Normal => {
                self.deferred_inputs.iter().find(|queued| *queued == &input)
            }
        }
        .map(|queued| queued.admission_ordinal);
        if let Some(ordinal) = duplicate_ordinal {
            return Ok(Some(ordinal));
        }
        match priority {
            DeferredPriority::Completion => {
                // Adapter completions and local timer events are trusted;
                // untrusted network traffic cannot consume this reserved lane.
                // `contains` above bounds repeated retransmit ticks for one tag,
                // while the one-shot absolute timeout is never dropped merely
                // because the normal deferred lane is full.
                if self.deferred_completions.len() >= MAX_DEFERRED_INPUTS {
                    return Err(AdapterError::DeferredCompletionCapacityExceeded);
                }
            }
            DeferredPriority::Progress => {
                // The progress lane is partitioned before admission: one slot
                // per frozen validator is reserved independently for exact
                // locked-round Commit reconstruction and TimeoutVote messages,
                // plus one slot for each PrepareQC, CommitQC, and TC class.
                // Exact duplicates coalesce above; a distinct item for an
                // already-owned signer/class retries after fair service rather
                // than displacing admitted progress.
                let Some(owner) = deferred_progress_owner(&input) else {
                    return Ok(None);
                };
                let class = owner.class();
                let class_capacity = match class {
                    DeferredProgressClass::LockedCommitVote
                    | DeferredProgressClass::TimeoutVote => self.wire_context.roster.len(),
                    DeferredProgressClass::PrepareCertificate
                    | DeferredProgressClass::CommitCertificate
                    | DeferredProgressClass::TimeoutCertificate => 1,
                };
                if self.deferred_progress_inputs.iter().any(|queued| {
                    deferred_progress_owner(queued)
                        .is_some_and(|queued_owner| queued_owner == owner)
                }) {
                    return Ok(None);
                }
                let class_len = self
                    .deferred_progress_inputs
                    .iter()
                    .filter(|queued| deferred_progress_class(queued) == Some(class))
                    .count();
                if class_len >= class_capacity
                    || self.deferred_progress_inputs.len() >= progress_capacity
                {
                    return Ok(None);
                }
            }
            DeferredPriority::Normal => {
                if self.deferred_inputs.len() >= MAX_DEFERRED_INPUTS {
                    return Ok(None);
                }
            }
        }
        input.admission_capability = self.mint_deferred_admission_ordinal()?;
        input.admission_ordinal = input.admission_capability.ordinal;
        let admission_ordinal = input.admission_ordinal;
        match priority {
            DeferredPriority::Completion => self.deferred_completions.push_back(input),
            DeferredPriority::Progress => self.deferred_progress_inputs.push_back(input),
            DeferredPriority::Normal => self.deferred_inputs.push_back(input),
        }
        Ok(Some(admission_ordinal))
    }

    fn mint_deferred_admission_ordinal(
        &mut self,
    ) -> Result<DeferredAdmissionCapability, AdapterError> {
        match self.deferred_admission_ordinals.mint() {
            Ok(ordinal) => Ok(ordinal),
            Err(error) => {
                self.fail_closed = true;
                Err(error)
            }
        }
    }

    /// Return whether one adapter-owned Busy-deferred input can cross the
    /// reducer boundary now.
    ///
    /// The serialized runtime gives this finite debt its own scheduling turn.
    /// Pending WAL or signing work must instead be cleared by its matching
    /// completion command, so reporting deferred work as ready while either
    /// fence is active would spin and starve that completion.
    pub(crate) fn deferred_work_is_serviceable(&self) -> bool {
        !self.fail_closed
            && self.replay_complete
            && self.reducer.pending_persistence_record().is_none()
            && self.reducer.awaiting_signature().is_none()
            && (!self.deferred_completions.is_empty()
                || !self.deferred_progress_inputs.is_empty()
                || !self.deferred_inputs.is_empty())
    }

    /// Return whether one exact runtime completion opens the active signing
    /// fence which currently makes older adapter-owned debt unserviceable.
    ///
    /// Safety-WAL acknowledgement is synchronous at this boundary, so signing
    /// is the only externally completed reducer fence. The preflight clone
    /// proves the callback still applies to the current reducer incarnation;
    /// the production effect executor has already verified the signature
    /// against its exact pending [`SignRequest`] and transferred that task's
    /// lifecycle owner. The runtime separately rejects an independently
    /// minted `SignatureCompleted` root from the dependency bypass. Stale and
    /// otherwise nonmatching completions remain ordinary FIFO work. A `true`
    /// result promises that dispatch consumes this signing fence rather than
    /// returning retryable or deferred work; the runtime fails closed if that
    /// contract is violated.
    pub(crate) fn completion_unblocks_deferred_fence(
        &self,
        tag: reducer::EventTag,
        command: &super::v2_runtime::AdapterCommand,
    ) -> bool {
        use super::v2_runtime::{
            AdapterCommand, RuntimeCommandAdmissionPreflight as AdmissionPreflight,
        };

        !self.fail_closed
            && self.replay_complete
            && tag == self.reducer.current_tag()
            && self.reducer.pending_persistence_record().is_none()
            && self.reducer.awaiting_signature().is_some()
            && matches!(command, AdapterCommand::SignatureCompleted(_))
            && self.preflight_runtime_command_admission(tag, command) == AdmissionPreflight::Admit
    }

    /// Return whether this exact queued command is forced to report `Busy` by
    /// the same active signing fence opened by
    /// [`Self::completion_unblocks_deferred_fence`].
    ///
    /// This is deliberately a proof, not a broad command-class hint. Internal
    /// callbacks must survive reducer preflight, while authenticated ingress
    /// must have a fresh semantic-admission path and convert against the
    /// current registry. A duplicate, equivocation report, unsafe proposal,
    /// capacity terminal, stale view, malformed conversion, or independent
    /// signature callback therefore remains an ordinary ordered owner.
    pub(crate) fn command_is_blocked_by_deferred_fence(
        &self,
        tag: reducer::EventTag,
        command: &super::v2_runtime::AdapterCommand,
    ) -> bool {
        use super::v2_runtime::{
            AdapterCommand, RuntimeCommandAdmissionPreflight as AdmissionPreflight,
        };

        if self.fail_closed
            || !self.replay_complete
            || tag != self.reducer.current_tag()
            || self.reducer.pending_persistence_record().is_some()
            || self.reducer.awaiting_signature().is_none()
        {
            return false;
        }
        match command {
            AdapterCommand::SignatureCompleted(_) => false,
            AdapterCommand::Authenticated(authenticated) => {
                self.authenticated_command_reaches_fenced_reducer(authenticated)
            }
            AdapterCommand::LocalProposalReady { .. }
            | AdapterCommand::BodyAvailable { .. }
            | AdapterCommand::BodyStored { .. }
            | AdapterCommand::ValidationSucceeded { .. }
            | AdapterCommand::ValidationFailed { .. }
            | AdapterCommand::ApplicationCompleted(_) => {
                self.preflight_runtime_command_admission(tag, command) == AdmissionPreflight::Admit
            }
        }
    }

    /// Conservatively prove that authenticated ingress reaches `Reducer::step`
    /// in the current adapter state. Once this returns `true`, the active
    /// signing fence makes the non-`Signed` reducer event unconditionally
    /// `Busy` before any phase handler can run.
    fn authenticated_command_reaches_fenced_reducer(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> bool {
        let message = &authenticated.0;
        if message.validate_version().is_err() {
            return false;
        }
        let current_view = self.reducer.current_tag().view();
        let retained_vote_views = u64::try_from(self.wire_context.roster.len()).unwrap_or(u64::MAX);
        let oldest_retained_view = current_view.saturating_sub(retained_vote_views);
        let payload = &message.payload;
        let locked_commit_progress = match payload {
            wire::ConsensusMessageV2Payload::Vote(vote) => self.is_exact_locked_commit_vote(vote),
            _ => false,
        };
        let unsafe_proposal = if let wire::ConsensusMessageV2Payload::Proposal(proposal) = payload
            && let Some(locked) = self.reducer.durable_state().locked()
        {
            let Ok(locked_subject) = self.registry.subject(locked.subject()) else {
                return false;
            };
            !proposal_is_safe_for_lock(
                proposal,
                self.registry.round_to_wire(locked.round()),
                locked_subject,
            )
        } else {
            false
        };
        if unsafe_proposal {
            return false;
        }

        let semantic_key = match payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                if proposal.round.view != current_view {
                    return false;
                }
                Some(IngressSemanticKey::Proposal {
                    round: proposal.round,
                    proposer: proposal.proposer,
                })
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                if vote.round.view > current_view
                    || (vote.round.view < oldest_retained_view && !locked_commit_progress)
                {
                    return false;
                }
                Some(IngressSemanticKey::Vote {
                    round: vote.round,
                    phase: vote.phase,
                    signer: vote.signer,
                })
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                if vote.round.view != current_view {
                    return false;
                }
                Some(IngressSemanticKey::TimeoutVote {
                    round: vote.round,
                    signer: vote.signer,
                })
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(_)
            | wire::ConsensusMessageV2Payload::TimeoutCertificate(_) => None,
            wire::ConsensusMessageV2Payload::PayloadManifest(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::VrfCommit(_)
            | wire::ConsensusMessageV2Payload::VrfReveal(_) => return false,
        };
        if let Some(key) = semantic_key {
            // Any existing semantic record can terminate as a duplicate or an
            // equivocation report before reaching the reducer. Be conservative
            // even when normal pruning would make a stale record removable.
            if self.ingress_equivocations.contains_key(&key) {
                return false;
            }
            let capacity_bypass = self.ingress_equivocations.len() >= MAX_INGRESS_SEMANTIC_KEYS;
            let protected_capacity_bypass =
                locked_commit_progress || matches!(key, IngressSemanticKey::TimeoutVote { .. });
            if capacity_bypass && !protected_capacity_bypass {
                return false;
            }
        }

        // Authentication has already verified the envelope signature. Repeat
        // the registry conversion on a clone so conflicting identities or
        // commitments cannot be mislabeled as reducer-fenced work.
        let mut registry = self.registry.clone();
        match payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => registry
                .proposal_to_core(proposal, &self.wire_context)
                .is_ok(),
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                registry.vote_to_core(vote, &self.wire_context).is_ok()
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                registry.qc_to_core(certificate, &self.wire_context).is_ok()
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => registry
                .timeout_vote_to_core(vote, &self.wire_context)
                .is_ok(),
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                registry.tc_to_core(certificate, &self.wire_context).is_ok()
            }
            wire::ConsensusMessageV2Payload::PayloadManifest(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::VrfCommit(_)
            | wire::ConsensusMessageV2Payload::VrfReveal(_) => false,
        }
    }

    /// Service at most one adapter-owned Busy-deferred reducer transition.
    ///
    /// Returning one macro-step preserves the executor's fixed retained-batch
    /// bound. Repeated serialized runtime turns decrease the finite deferred
    /// rank, while `pop_deferred_next` keeps the three classes round-robin.
    #[cfg(test)]
    pub(crate) fn drain_deferred(&mut self) -> Result<Vec<AdapterEffect>, AdapterError> {
        self.drain_deferred_with_evidence()
            .map(|selection| selection.map_or_else(Vec::new, |(effects, _)| effects))
    }

    /// Service one deferred transition and return its exact process-local
    /// ownership token with the resulting effects.
    ///
    /// `None` means no owner was serviceable. Production runtime code treats a
    /// `None` after observing [`Self::deferred_work_is_serviceable`] as a
    /// fail-closed source-fidelity violation.
    pub(crate) fn drain_deferred_with_evidence(
        &mut self,
    ) -> Result<Option<(Vec<AdapterEffect>, DeferredServiceEvidence)>, AdapterError> {
        let eligible = self.all_deferred_admission_ordinals();
        self.drain_deferred_with_evidence_for_ordinals(&eligible)
    }

    /// Service one deferred transition from the exact lifecycle-minimal set
    /// selected by the serialized runtime.
    ///
    /// The adapter still owns class rotation within that set.  Passing the set
    /// across the runtime/adapter seam prevents a later Completion, Progress,
    /// or Normal occurrence from overtaking an older causal lifecycle merely
    /// because it occupies the cursor's next class.
    pub(crate) fn drain_deferred_with_evidence_for_ordinals(
        &mut self,
        eligible: &BTreeSet<u128>,
    ) -> Result<Option<(Vec<AdapterEffect>, DeferredServiceEvidence)>, AdapterError> {
        let Some((effects, evidence, producer_handoff)) =
            self.drain_deferred_with_handoff_for_ordinals(eligible)?
        else {
            return Ok(None);
        };
        if let Some(token) = producer_handoff {
            let handoff_evidence = self.producer_handoff_evidence(token, !effects.is_empty())?;
            self.acknowledge_producer_handoff(token, handoff_evidence)?;
        }
        Ok(Some((effects, evidence)))
    }

    /// Production deferred-service seam retaining an exact producer token
    /// until the serialized runtime installs the returned successor owner.
    pub(crate) fn drain_deferred_with_handoff_for_ordinals(
        &mut self,
        eligible: &BTreeSet<u128>,
    ) -> Result<
        Option<(
            Vec<AdapterEffect>,
            DeferredServiceEvidence,
            Option<ProducerContinuationHandoffToken>,
        )>,
        AdapterError,
    > {
        self.ensure_ingress()?;
        if !self.deferred_work_is_serviceable() {
            return Ok(None);
        }
        let active = self.all_deferred_admission_ordinals();
        if eligible.is_empty() || !eligible.is_subset(&active) {
            self.fail_closed = true;
            return Err(AdapterError::DeferredServiceOwnershipViolation);
        }
        let Some(selection) = self.pop_deferred_next_eligible(eligible)? else {
            return Ok(None);
        };
        if !selection.evidence.validate_exact()
            || !selection
                .evidence
                .matches_effective_event(&selection.input.event)
            || !selection
                .evidence
                .belongs_to(&self.deferred_admission_ordinals)
            || !self.deferred_authenticated_event_matches_wire(&selection.evidence)
            || !selection
                .evidence
                .admission_capability
                .claim_adapter_service_once()
        {
            self.fail_closed = true;
            return Err(AdapterError::DeferredServiceOwnershipViolation);
        }
        let deferred_ordinal = selection.evidence.admission_ordinal;
        let producer_continuation = self
            .deferred_producer_continuations
            .get(&deferred_ordinal)
            .cloned();
        let input = selection.input;
        let serviced_candidate = self.serviced_candidate(
            &input.event,
            input.priority,
            input.completion_evidence.as_ref(),
            input.authenticated_wire_identity.as_deref(),
        );
        if serviced_candidate.is_some_and(|(key, _, policy)| {
            policy == ServicedCandidatePolicy::Suppress
                && self.serviced_candidates.contains_key(&key)
        }) {
            if let Some(admission) = input.admission {
                self.record_ingress_delivery(admission);
            }
            let disposition = reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate);
            self.record_disposition(disposition);
            self.deferred_producer_continuations
                .remove(&deferred_ordinal);
            self.release_unrecorded_producer(producer_continuation)?;
            self.publish_status()?;
            self.log_body_progress(&input.event, disposition, 0);
            return Ok(Some((Vec::new(), selection.evidence, None)));
        }
        if let Err(error) =
            self.ensure_serviced_candidate_capacity_before_step(&input.event, serviced_candidate)
        {
            self.retain_failed_serviced_deferred_owner(input);
            return Err(error);
        }
        let event = input.event.clone();
        let observed_event = event.clone();
        let outcome = self.reducer.step(event)?;
        let disposition = outcome.disposition();
        self.record_reducer_outcome(&observed_event, disposition, outcome.effects());
        if disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            // `Busy` has exactly the two fences excluded by
            // `deferred_work_is_serviceable`. A future reducer change must
            // extend that predicate deliberately; silently requeueing would
            // return `Advanced([])` forever without decreasing queue rank.
            return Err(self.fail_deferred_service_contract());
        }
        let effects = self.drive_effects(outcome.into_effects())?;
        let record_kind = serviced_candidate_record_kind(&observed_event, disposition);
        let serviced_candidate = record_kind.and(serviced_candidate);
        let durable_terminal_retirement =
            record_kind == Some(ServicedCandidateRecordKind::DurableTerminal);
        let durable_terminal_evidence =
            durable_terminal_retirement || input.completion_evidence.is_some();
        let producer_handoff = if record_kind.is_some() {
            self.record_serviced_candidate(
                serviced_candidate,
                durable_terminal_retirement,
                durable_terminal_evidence,
                producer_continuation,
            )
        } else {
            self.release_unrecorded_producer(producer_continuation)
                .map(|()| None)
        };
        let producer_handoff = match producer_handoff {
            Ok(token) => token,
            Err(error) => {
                self.retain_failed_serviced_deferred_owner(input);
                return Err(error);
            }
        };
        self.deferred_producer_continuations
            .remove(&deferred_ordinal);
        if let Some(admission) = input.admission {
            self.record_ingress_delivery(admission);
        }
        self.publish_status()?;
        self.log_body_progress(&observed_event, disposition, effects.len());
        Ok(Some((effects, selection.evidence, producer_handoff)))
    }

    fn retain_failed_serviced_deferred_owner(&mut self, input: DeferredInput) {
        match input.priority {
            DeferredPriority::Completion => self.deferred_completions.push_front(input),
            DeferredPriority::Progress => self.deferred_progress_inputs.push_front(input),
            DeferredPriority::Normal => self.deferred_inputs.push_front(input),
        }
    }

    fn deferred_authenticated_event_matches_wire(
        &self,
        evidence: &DeferredServiceEvidence,
    ) -> bool {
        let Some(identity) = evidence.authenticated_wire_identity.as_deref() else {
            return !evidence.is_authenticated_ingress();
        };
        let message = match &evidence.original_event {
            reducer::Event::ProposalReceived { proposal, .. } => {
                reducer::ConsensusMessageV2::Proposal(proposal.clone())
            }
            reducer::Event::VoteReceived { vote, .. } => {
                reducer::ConsensusMessageV2::Vote(vote.clone())
            }
            reducer::Event::QuorumCertificateReceived { certificate, .. } => {
                reducer::ConsensusMessageV2::QuorumCertificate(certificate.clone())
            }
            reducer::Event::TimeoutVoteReceived { vote, .. } => {
                reducer::ConsensusMessageV2::TimeoutVote(vote.clone())
            }
            reducer::Event::TimeoutCertificateReceived { certificate, .. } => {
                reducer::ConsensusMessageV2::TimeoutCertificate(certificate.clone())
            }
            _ => return false,
        };
        let mut registry = self.registry.clone();
        registry
            .message_to_wire(message, self.aggregator.as_ref())
            .is_ok_and(|message| message.encode().as_slice() == identity)
    }

    /// Fail closed when the deferred-service predicate and reducer Busy
    /// contract disagree.
    fn fail_deferred_service_contract(&mut self) -> AdapterError {
        self.fail_closed = true;
        AdapterError::DeferredServiceContractViolation
    }

    fn deferred_queue_lengths(&self) -> DeferredQueueLengths {
        DeferredQueueLengths {
            completion: u64::try_from(self.deferred_completions.len())
                .expect("bounded completion queue length fits u64"),
            progress: u64::try_from(self.deferred_progress_inputs.len())
                .expect("bounded progress queue length fits u64"),
            normal: u64::try_from(self.deferred_inputs.len())
                .expect("bounded normal queue length fits u64"),
        }
    }

    #[cfg(test)]
    fn pop_deferred_next(&mut self) -> Result<Option<DeferredServiceSelection>, AdapterError> {
        let eligible = self.all_deferred_admission_ordinals();
        self.pop_deferred_next_eligible(&eligible)
    }

    fn pop_deferred_next_eligible(
        &mut self,
        eligible: &BTreeSet<u128>,
    ) -> Result<Option<DeferredServiceSelection>, AdapterError> {
        let queue_lengths_before = self.deferred_queue_lengths();
        let service_cursor_before = self.next_deferred_priority;
        for _ in 0..3 {
            let priority = self.next_deferred_priority;
            self.next_deferred_priority = self.next_deferred_priority.next();
            let selected = match priority {
                DeferredPriority::Completion => self
                    .deferred_completions
                    .iter()
                    .position(|input| eligible.contains(&input.admission_ordinal))
                    .and_then(|position| self.deferred_completions.remove(position)),
                DeferredPriority::Progress => self
                    .deferred_progress_inputs
                    .iter()
                    .position(|input| eligible.contains(&input.admission_ordinal))
                    .and_then(|position| self.deferred_progress_inputs.remove(position)),
                DeferredPriority::Normal => self
                    .deferred_inputs
                    .iter()
                    .position(|input| eligible.contains(&input.admission_ordinal))
                    .and_then(|position| self.deferred_inputs.remove(position)),
            };
            let Some(selected) = selected else {
                continue;
            };
            for skipped_priority in [
                DeferredPriority::Completion,
                DeferredPriority::Progress,
                DeferredPriority::Normal,
            ] {
                if skipped_priority == priority {
                    continue;
                }
                let oldest = match skipped_priority {
                    DeferredPriority::Completion => self
                        .deferred_completions
                        .iter_mut()
                        .find(|input| eligible.contains(&input.admission_ordinal)),
                    DeferredPriority::Progress => self
                        .deferred_progress_inputs
                        .iter_mut()
                        .find(|input| eligible.contains(&input.admission_ordinal)),
                    DeferredPriority::Normal => self
                        .deferred_inputs
                        .iter_mut()
                        .find(|input| eligible.contains(&input.admission_ordinal)),
                };
                let Some(oldest) = oldest else {
                    continue;
                };
                let Some(next_debt) = oldest.eligible_skips.checked_add(1) else {
                    self.fail_closed = true;
                    return Err(AdapterError::DeferredServiceDebtOverflow);
                };
                oldest.eligible_skips = next_debt;
            }
            let mut input = selected;
            let original_event = input.event.clone();
            let original_admission = input.admission;
            let original_tag = deferred_event_tag(&original_event);
            let retag = if input.retag_authenticated_ingress {
                let current_tag = self.reducer.current_tag();
                input.event = input.event.retag_authenticated_ingress(current_tag);
                if let Some(admission) = &mut input.admission {
                    admission.generation = current_tag.generation();
                }
                DeferredRetagRelation::AuthenticatedIngress {
                    from: original_tag,
                    to: current_tag,
                }
            } else {
                DeferredRetagRelation::Unchanged
            };
            let queue_lengths_after = self.deferred_queue_lengths();
            let mut evidence = DeferredServiceEvidence {
                admission_ordinal: input.admission_ordinal,
                priority,
                event_kind: deferred_event_kind(&original_event),
                original_tag,
                effective_tag: deferred_event_tag(&input.event),
                retag,
                protected_progress: input.protected_progress,
                eligible_skips_before: input.eligible_skips,
                eligible_skips_after: 0,
                queue_lengths_before,
                queue_lengths_after,
                total_len_before: queue_lengths_before.total(),
                total_len_after: queue_lengths_after.total(),
                service_cursor_before,
                service_cursor_after: self.next_deferred_priority,
                projection_hash: Hash::new([]),
                original_event,
                effective_event: input.event.clone(),
                completion_evidence: input.completion_evidence.clone(),
                original_admission,
                effective_admission: input.admission,
                authenticated_wire_identity: input.authenticated_wire_identity.clone(),
                admission_capability: input.admission_capability.clone(),
            };
            evidence.projection_hash = deferred_service_projection_hash(&evidence);
            return Ok(Some(DeferredServiceSelection { input, evidence }));
        }
        Ok(None)
    }

    fn publish_status(&mut self) -> Result<(), AdapterError> {
        let status = self.status()?;
        super::status::set_v2_status(status);
        Ok(())
    }

    /// Permanently close the adapter after an internal macro-step shape
    /// violates the reviewed reducer/continuation contract.
    fn fail_macro_step(&mut self, error: AdapterError) -> AdapterError {
        debug_assert!(matches!(
            &error,
            AdapterError::AdapterMacroStepBoundExceeded { .. }
        ));
        self.fail_closed = true;
        error
    }

    fn drive_effects(
        &mut self,
        effects: Vec<reducer::Effect>,
    ) -> Result<Vec<AdapterEffect>, AdapterError> {
        let initial_effects = effects.len();
        let mut persist_effects = 0usize;
        let mut persistence_class = None;
        for effect in &effects {
            if let reducer::Effect::Persist { entry, .. } = effect {
                persist_effects = persist_effects.saturating_add(1);
                persistence_class = Some(PersistenceMacroStepClass::from_record(entry.record()));
            }
        }
        let persistence_budget = persistence_class.map(PersistenceMacroStepClass::budget);
        let maximum_initial_effects = persistence_budget
            .map_or(MAX_ADAPTER_EFFECTS_PER_MACRO_STEP, |budget| {
                budget.initial_effects
            });
        let maximum_continuation_effects =
            persistence_budget.map_or(0, |budget| budget.continuation_effects);
        let maximum_flattened_effects = persistence_budget.map_or(
            MAX_ADAPTER_EFFECTS_PER_MACRO_STEP,
            PersistenceMacroStepBudget::flattened_effects,
        );
        if persist_effects > 1 || initial_effects > maximum_initial_effects {
            let error = AdapterError::AdapterMacroStepBoundExceeded {
                initial_effects,
                maximum_initial_effects,
                persist_effects,
                continuation_effects: 0,
                maximum_continuation_effects,
                maximum_flattened_effects,
                continuation_contains_persist: false,
            };
            return Err(self.fail_macro_step(error));
        }

        let mut pending = VecDeque::from(effects);
        let mut ready = Vec::new();
        let mut observed_continuation_effects = 0usize;
        let mut continuation_contains_persist = false;
        while let Some(effect) = pending.pop_front() {
            match effect {
                reducer::Effect::Persist { tag, entry } => {
                    let id = entry.id();
                    self.pending_persistence_id = Some(id.get());
                    if self.replay_complete {
                        if let Err(error) = self.publish_status() {
                            self.fail_closed = true;
                            return Err(error);
                        }
                    }
                    let payload = match self
                        .registry
                        .encode_wal_entry(&entry, self.aggregator.as_ref())
                    {
                        Ok(payload) => payload,
                        Err(error) => {
                            self.fail_closed = true;
                            return Err(error);
                        }
                    };
                    let sequence = match self.wal.append(&payload) {
                        Ok(sequence) => sequence,
                        Err(error) => {
                            self.fail_closed = true;
                            let _ = self
                                .reducer
                                .step(reducer::Event::PersistenceFailed { tag, id });
                            return Err(error.into());
                        }
                    };
                    if sequence.checked_add(1) != Some(id.get()) {
                        self.fail_closed = true;
                        return Err(AdapterError::WalSequenceMismatch {
                            frame_sequence: sequence,
                            persistence_id: id.get(),
                        });
                    }
                    self.pending_persistence_id = None;
                    let persisted = reducer::Event::Persisted { tag, id };
                    let continuation = match self.reducer.step(persisted.clone()) {
                        Ok(continuation) => continuation,
                        Err(error) => {
                            // The physical WAL is now ahead of memory. Only a
                            // clean reopen/replay may reconcile that state.
                            self.fail_closed = true;
                            return Err(error.into());
                        }
                    };
                    self.prune_ingress_records();
                    self.reclaim_serviced_candidates()?;
                    self.record_reducer_outcome(
                        &persisted,
                        continuation.disposition(),
                        continuation.effects(),
                    );
                    let continuation = continuation.into_effects();
                    observed_continuation_effects = continuation.len();
                    continuation_contains_persist = continuation
                        .iter()
                        .any(|effect| matches!(effect, reducer::Effect::Persist { .. }));
                    let flattened_effects = initial_effects
                        .saturating_sub(1)
                        .saturating_add(observed_continuation_effects);
                    if observed_continuation_effects > maximum_continuation_effects
                        || continuation_contains_persist
                        || flattened_effects > maximum_flattened_effects
                        || flattened_effects > MAX_ADAPTER_EFFECTS_PER_MACRO_STEP
                    {
                        let error = AdapterError::AdapterMacroStepBoundExceeded {
                            initial_effects,
                            maximum_initial_effects,
                            persist_effects,
                            continuation_effects: observed_continuation_effects,
                            maximum_continuation_effects,
                            maximum_flattened_effects,
                            continuation_contains_persist,
                        };
                        return Err(self.fail_macro_step(error));
                    }
                    reducer::prepend_causal_continuation(&mut pending, continuation);
                }
                effect => match self.convert_effect(effect) {
                    Ok(effect) => ready.push(effect),
                    Err(error) => {
                        self.fail_closed = true;
                        return Err(error);
                    }
                },
            }
        }
        if ready.len() > MAX_ADAPTER_EFFECTS_PER_MACRO_STEP {
            let error = AdapterError::AdapterMacroStepBoundExceeded {
                initial_effects,
                maximum_initial_effects,
                persist_effects,
                continuation_effects: observed_continuation_effects,
                maximum_continuation_effects,
                maximum_flattened_effects,
                continuation_contains_persist,
            };
            return Err(self.fail_macro_step(error));
        }
        Ok(ready)
    }

    fn convert_effect(&mut self, effect: reducer::Effect) -> Result<AdapterEffect, AdapterError> {
        match effect {
            reducer::Effect::Persist { .. } => {
                unreachable!("persistence effects are consumed by drive_effects")
            }
            reducer::Effect::FetchBody {
                tag,
                round,
                subject,
                manifest,
                certified_sources,
                certificate,
            } => Ok(AdapterEffect::FetchBody {
                tag,
                round: self.registry.round_to_wire(round),
                subject: self.registry.subject(subject)?,
                manifest: manifest
                    .map(|manifest| self.registry.manifest_to_wire(round, &manifest))
                    .transpose()?,
                certified_sources: certified_sources
                    .into_iter()
                    .map(|validator| self.registry.peer(validator))
                    .collect::<Result<_, _>>()?,
                certificate: certificate
                    .map(|certificate| {
                        self.registry
                            .qc_to_wire(&certificate, self.aggregator.as_ref())
                    })
                    .transpose()?,
            }),
            reducer::Effect::StoreBody {
                tag,
                round,
                subject,
            } => Ok(AdapterEffect::StoreBody {
                tag,
                round: self.registry.round_to_wire(round),
                subject: self.registry.subject(subject)?,
            }),
            reducer::Effect::ValidateBody {
                tag,
                round,
                subject,
            } => Ok(AdapterEffect::ValidateBody {
                tag,
                round: self.registry.round_to_wire(round),
                subject: self.registry.subject(subject)?,
            }),
            reducer::Effect::Sign { tag, message } => {
                let request = match message {
                    reducer::SignableMessage::Proposal(proposal) => SignRequest::Proposal(
                        self.registry
                            .unsigned_proposal_to_wire(&proposal, self.aggregator.as_ref())?,
                    ),
                    reducer::SignableMessage::Vote(vote) => {
                        SignRequest::Vote(self.registry.unsigned_vote_to_wire(vote)?)
                    }
                    reducer::SignableMessage::TimeoutVote(vote) => SignRequest::TimeoutVote(
                        self.registry
                            .unsigned_timeout_vote_to_wire(&vote, self.aggregator.as_ref())?,
                    ),
                };
                Ok(AdapterEffect::Sign { tag, request })
            }
            reducer::Effect::Broadcast(message) => Ok(AdapterEffect::Broadcast(
                self.registry
                    .message_to_wire(message, self.aggregator.as_ref())?,
            )),
            reducer::Effect::Apply {
                tag,
                subject,
                certificate,
            } => Ok(AdapterEffect::Apply {
                tag,
                subject: self.registry.subject(subject)?,
                certificate: self
                    .registry
                    .qc_to_wire(&certificate, self.aggregator.as_ref())?,
            }),
            reducer::Effect::EnterView {
                tag,
                certificate,
                protected_lock,
            } => {
                // Consume the lock selected by the reducer transition itself.
                // Converting the full QC through the registry also proves that
                // its exact execution commitment is bound before the executor
                // receives the reduced round/subject body identity.
                let protected_body = protected_lock
                    .as_ref()
                    .map(|locked| {
                        self.registry
                            .qc_to_wire(locked, self.aggregator.as_ref())
                            .map(|locked| (locked.round, locked.subject))
                    })
                    .transpose()?;
                self.active_subject = protected_lock
                    .as_ref()
                    .map(|locked| (locked.round(), locked.subject()));
                Ok(AdapterEffect::EnterView {
                    tag,
                    certificate: self
                        .registry
                        .tc_to_wire(&certificate, self.aggregator.as_ref())?,
                    protected_body,
                })
            }
            reducer::Effect::ReportEquivocation { evidence } => {
                // TODO: Carry the complete authenticated conflicting message pair
                // through `AdapterEffect` and persist it before enabling evidence
                // penalties. First-release live handling is deliberately logging-only.
                Ok(AdapterEffect::ReportEquivocation {
                    offender: self.registry.peer(evidence.offender())?,
                    round: self.registry.round_to_wire(evidence.round()),
                    kind: evidence.kind(),
                })
            }
            reducer::Effect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => Ok(AdapterEffect::ReportInvalidCertifiedBody {
                subject: self.registry.subject(subject)?,
                certificate: self
                    .registry
                    .qc_to_wire(&certificate, self.aggregator.as_ref())?,
            }),
        }
    }
}

const ALL_IGNORE_REASONS: [(reducer::IgnoreReason, wire::SumeragiV2IgnoreReason); 12] = [
    (
        reducer::IgnoreReason::WrongHeight,
        wire::SumeragiV2IgnoreReason::WrongHeight,
    ),
    (
        reducer::IgnoreReason::WrongView,
        wire::SumeragiV2IgnoreReason::WrongView,
    ),
    (
        reducer::IgnoreReason::StaleGeneration,
        wire::SumeragiV2IgnoreReason::StaleGeneration,
    ),
    (
        reducer::IgnoreReason::Busy,
        wire::SumeragiV2IgnoreReason::Busy,
    ),
    (
        reducer::IgnoreReason::Duplicate,
        wire::SumeragiV2IgnoreReason::Duplicate,
    ),
    (
        reducer::IgnoreReason::NoMatchingWork,
        wire::SumeragiV2IgnoreReason::NoMatchingWork,
    ),
    (
        reducer::IgnoreReason::Observer,
        wire::SumeragiV2IgnoreReason::Observer,
    ),
    (
        reducer::IgnoreReason::ViewClosed,
        wire::SumeragiV2IgnoreReason::ViewClosed,
    ),
    (
        reducer::IgnoreReason::AlreadyDecided,
        wire::SumeragiV2IgnoreReason::AlreadyDecided,
    ),
    (
        reducer::IgnoreReason::RecoveryPending,
        wire::SumeragiV2IgnoreReason::RecoveryPending,
    ),
    (
        reducer::IgnoreReason::IrrelevantView,
        wire::SumeragiV2IgnoreReason::IrrelevantView,
    ),
    (
        reducer::IgnoreReason::UnsafeProposal,
        wire::SumeragiV2IgnoreReason::UnsafeProposal,
    ),
];

const fn outbound_stage_rank(stage: wire::SumeragiV2OutboundIntentStage) -> u8 {
    match stage {
        wire::SumeragiV2OutboundIntentStage::PendingPersistence => 0,
        wire::SumeragiV2OutboundIntentStage::PendingSignature => 1,
        wire::SumeragiV2OutboundIntentStage::Queued => 2,
        wire::SumeragiV2OutboundIntentStage::Sent => 3,
    }
}

fn bounded_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn duration_ms(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn queue_status(
    queue: wire::SumeragiV2QueueKind,
    depth: usize,
    capacity: usize,
    oldest_age: Option<std::time::Duration>,
    service_debt: u64,
) -> wire::SumeragiV2QueueStatus {
    wire::SumeragiV2QueueStatus {
        queue,
        depth: bounded_u32(depth),
        capacity: bounded_u32(capacity),
        oldest_age_ms: oldest_age.map(duration_ms),
        service_debt,
    }
}

fn deferred_queue_status(
    queue: wire::SumeragiV2QueueKind,
    inputs: &VecDeque<DeferredInput>,
    capacity: usize,
    now: Instant,
) -> wire::SumeragiV2QueueStatus {
    let oldest_age = inputs
        .iter()
        .map(|input| input.admitted_at)
        .min()
        .map(|oldest| now.saturating_duration_since(oldest));
    let service_debt = inputs
        .iter()
        .map(|input| input.eligible_skips)
        .max()
        .unwrap_or_default();
    queue_status(queue, inputs.len(), capacity, oldest_age, service_debt)
}

#[cfg(test)]
fn progress_rank(event: &reducer::Event) -> u8 {
    match event {
        reducer::Event::QuorumCertificateReceived { certificate, .. }
            if certificate.phase() == reducer::Phase::Commit =>
        {
            3
        }
        reducer::Event::TimeoutCertificateReceived { .. } => 2,
        reducer::Event::QuorumCertificateReceived { .. } => 1,
        reducer::Event::ResumeAfterReplay { .. }
        | reducer::Event::LocalProposalReady { .. }
        | reducer::Event::ProposalReceived { .. }
        | reducer::Event::VoteReceived { .. }
        | reducer::Event::TimeoutVoteReceived { .. }
        | reducer::Event::TimeoutElapsed { .. }
        | reducer::Event::RetransmitElapsed { .. }
        | reducer::Event::BodyAvailable { .. }
        | reducer::Event::BodyStored { .. }
        | reducer::Event::ValidationCompleted { .. }
        | reducer::Event::Persisted { .. }
        | reducer::Event::PersistenceFailed { .. }
        | reducer::Event::Signed { .. }
        | reducer::Event::ApplicationCompleted { .. } => 0,
    }
}

#[derive(Clone, Debug, Decode, Encode)]
struct WalEnvelopeV2 {
    protocol_version: u16,
    persistence_id: u64,
    record: WalRecordV2,
}

#[derive(Clone, Debug, Decode, Encode)]
enum WalRecordV2 {
    ProposalIntent(wire::Proposal),
    PrepareIntent(wire::Vote),
    ObservePrepare(wire::QuorumCertificate),
    LockAndCommit {
        prepare: wire::QuorumCertificate,
        vote: wire::Vote,
    },
    TimeoutIntent(wire::TimeoutVote),
    InstallTimeout(wire::TimeoutCertificate),
    Decision(wire::QuorumCertificate),
}

#[derive(Clone, Default)]
struct WireRegistry {
    wire_context: Option<wire::HeightContext>,
    context_id: Option<wire::HeightContextId>,
    peers: Vec<PeerId>,
    validators: BTreeMap<reducer::ValidatorId, wire::ValidatorIndex>,
    subjects: BTreeMap<reducer::Subject, wire::BlockSubject>,
    manifests: BTreeMap<(reducer::Round, reducer::Subject), wire::PayloadManifest>,
    execution_commitments: BTreeMap<(reducer::Round, reducer::Subject), wire::ExecutionCommitment>,
    certificates: BTreeMap<reducer::CertificateRef, wire::QuorumCertificate>,
    proposals: BTreeMap<(reducer::Round, reducer::Subject), wire::Proposal>,
}

impl WireRegistry {
    fn new(context: &wire::HeightContext) -> Result<Self, AdapterError> {
        let mut registry = Self {
            wire_context: Some(context.clone()),
            context_id: Some(context.id()),
            peers: context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect(),
            ..Self::default()
        };
        for index in 0..context.roster.len() {
            let index = u32::try_from(index).map_err(|_| wire::ValidationError::RosterTooLarge)?;
            registry.validators.insert(validator_token(index), index);
        }
        Ok(registry)
    }

    fn core_context(
        &mut self,
        context: &wire::HeightContext,
    ) -> Result<reducer::HeightContext, AdapterError> {
        let parent_commit = context
            .parent_commit_qc
            .as_ref()
            .map(|certificate| self.register_parent_qc(certificate))
            .transpose()?;
        let roster = context
            .roster
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let index =
                    u32::try_from(index).map_err(|_| wire::ValidationError::RosterTooLarge)?;
                Ok(reducer::Validator::new(
                    validator_token(index),
                    reducer::VotingPower::new(entry.power),
                ))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let mode = match context.mode {
            wire::ConsensusMode::Permissioned => reducer::VotingMode::Permissioned,
            wire::ConsensusMode::Npos => reducer::VotingMode::Npos,
        };
        let leader_height_seed = Hash::new((context.leader_seed, context.height).encode());
        let context_id = context_id(
            self.context_id
                .expect("registry is constructed with a height context"),
        );
        let chain_id = reducer::ChainId::new(Hash::new(context.chain_id.encode()).into());
        let nexus_hash = reducer::Digest::new(*context.nexus_amx_context_hash.as_ref());
        let execution_policy_hash = reducer::Digest::new(*context.execution_policy_hash.as_ref());
        let da_hash = reducer::Digest::new(Hash::new(context.da_layout.encode()).into());
        let leader_seed = reducer::Digest::new(leader_height_seed.into());
        if context.snapshot_bootstrap.is_some() {
            reducer::HeightContext::new_snapshot_bootstrap(
                context_id,
                chain_id,
                context.height,
                context.epoch,
                roster,
                mode,
                nexus_hash,
                execution_policy_hash,
                da_hash,
                leader_seed,
            )
        } else {
            reducer::HeightContext::new(
                context_id,
                chain_id,
                context.height,
                parent_commit,
                context.epoch,
                roster,
                mode,
                nexus_hash,
                execution_policy_hash,
                da_hash,
                leader_seed,
            )
        }
        .map_err(Into::into)
    }

    fn validator_id(
        &self,
        index: wire::ValidatorIndex,
    ) -> Result<reducer::ValidatorId, AdapterError> {
        if usize::try_from(index)
            .ok()
            .is_some_and(|index| index < self.peers.len())
        {
            Ok(validator_token(index))
        } else {
            Err(AdapterError::ValidatorIndexOutOfRange(index))
        }
    }

    fn validator_index(
        &self,
        validator: reducer::ValidatorId,
    ) -> Result<wire::ValidatorIndex, AdapterError> {
        self.validators
            .get(&validator)
            .copied()
            .ok_or(AdapterError::UnknownValidator(validator))
    }

    fn peer(&self, validator: reducer::ValidatorId) -> Result<PeerId, AdapterError> {
        let index = self.validator_index(validator)?;
        self.peers
            .get(usize::try_from(index).unwrap_or(usize::MAX))
            .cloned()
            .ok_or(AdapterError::ValidatorIndexOutOfRange(index))
    }

    fn register_subject(
        &mut self,
        subject: wire::BlockSubject,
    ) -> Result<reducer::Subject, AdapterError> {
        let digest = reducer::Subject::new(Hash::new(subject.encode()).into());
        match self.subjects.get(&digest) {
            Some(existing) if *existing != subject => Err(AdapterError::SubjectCollision),
            Some(_) => Ok(digest),
            None => {
                self.subjects.insert(digest, subject);
                Ok(digest)
            }
        }
    }

    fn manifest_conflicts(&self, incoming: &wire::PayloadManifest) -> bool {
        let round = reducer::Round::new(incoming.round.height, incoming.round.view);
        let subject = reducer::Subject::new(Hash::new(incoming.subject.encode()).into());
        self.manifests
            .get(&(round, subject))
            .is_some_and(|registered| registered != incoming)
    }

    fn subject(&self, subject: reducer::Subject) -> Result<wire::BlockSubject, AdapterError> {
        self.subjects
            .get(&subject)
            .copied()
            .ok_or(AdapterError::UnknownSubject(subject))
    }

    fn register_execution_commitment(
        &mut self,
        round: reducer::Round,
        subject: reducer::Subject,
        commitment: wire::ExecutionCommitment,
    ) -> Result<(), AdapterError> {
        commitment.validate()?;
        if self
            .execution_commitments
            .iter()
            .any(|((_, registered_subject), registered)| {
                *registered_subject == subject && *registered != commitment
            })
        {
            return Err(AdapterError::ConflictingExecutionCommitment);
        }
        match self.execution_commitments.get(&(round, subject)) {
            Some(_) => Ok(()),
            None => {
                self.execution_commitments
                    .insert((round, subject), commitment);
                Ok(())
            }
        }
    }

    fn execution_commitment(
        &self,
        round: reducer::Round,
        subject: reducer::Subject,
    ) -> Result<wire::ExecutionCommitment, AdapterError> {
        self.execution_commitments
            .get(&(round, subject))
            .copied()
            .ok_or(AdapterError::MissingExecutionCommitment)
    }

    fn round_to_core(
        &self,
        round: wire::ConsensusRound,
        context: &wire::HeightContext,
    ) -> Result<reducer::Round, AdapterError> {
        if Some(round.context_id) != self.context_id || round.height != context.height {
            return Err(wire::ValidationError::WrongHeightContext.into());
        }
        Ok(reducer::Round::new(round.height, round.view))
    }

    fn round_to_wire(&self, round: reducer::Round) -> wire::ConsensusRound {
        wire::ConsensusRound {
            context_id: self
                .context_id
                .expect("registry is constructed with a height context"),
            height: round.height(),
            view: round.view(),
        }
    }

    fn phase_to_core(phase: wire::GlobalPhase) -> reducer::Phase {
        match phase {
            wire::GlobalPhase::Prepare => reducer::Phase::Prepare,
            wire::GlobalPhase::Commit => reducer::Phase::Commit,
        }
    }

    fn phase_to_wire(phase: reducer::Phase) -> wire::GlobalPhase {
        match phase {
            reducer::Phase::Prepare => wire::GlobalPhase::Prepare,
            reducer::Phase::Commit => wire::GlobalPhase::Commit,
        }
    }

    fn vote_to_core(
        &mut self,
        vote: &wire::Vote,
        context: &wire::HeightContext,
    ) -> Result<reducer::SignedVote, AdapterError> {
        if vote.round != vote.proposal_round {
            return Err(AdapterError::WireValidation(
                wire::ValidationError::InvalidProposalRound,
            ));
        }
        let round = self.round_to_core(vote.round, context)?;
        let proposal_round = self.round_to_core(vote.proposal_round, context)?;
        let subject = self.register_subject(vote.subject)?;
        self.register_execution_commitment(proposal_round, subject, vote.execution_commitment)?;
        let signer = self.validator_id(vote.signer)?;
        Ok(reducer::SignedVote::new(
            reducer::Vote::new_with_proposal_round(
                context_id(vote.round.context_id),
                round,
                proposal_round,
                Self::phase_to_core(vote.phase),
                subject,
                signer,
            ),
            reducer::OpaqueSignature::new(vote.signature.clone()),
        ))
    }

    fn unsigned_vote_to_wire(&self, vote: reducer::Vote) -> Result<wire::Vote, AdapterError> {
        Ok(wire::Vote {
            round: self.round_to_wire(vote.round()),
            proposal_round: self.round_to_wire(vote.proposal_round()),
            phase: Self::phase_to_wire(vote.phase()),
            subject: self.subject(vote.subject())?,
            execution_commitment: self
                .execution_commitment(vote.proposal_round(), vote.subject())?,
            signer: self.validator_index(vote.signer())?,
            signature: Vec::new(),
        })
    }

    fn signed_vote_to_wire(&self, vote: &reducer::SignedVote) -> Result<wire::Vote, AdapterError> {
        let mut wire = self.unsigned_vote_to_wire(vote.vote())?;
        wire.signature = vote.signature().as_bytes().to_vec();
        Ok(wire)
    }

    fn qc_reference_to_core(
        &mut self,
        reference: &wire::QuorumCertificateRef,
    ) -> Result<reducer::CertificateRef, AdapterError> {
        let Some(wire_context_id) = self.context_id else {
            return Err(AdapterError::WireValidation(
                wire::ValidationError::WrongHeightContext,
            ));
        };
        self.qc_reference_to_core_for_context(reference, wire_context_id)
    }

    fn qc_reference_to_core_for_context(
        &mut self,
        reference: &wire::QuorumCertificateRef,
        expected_context_id: wire::HeightContextId,
    ) -> Result<reducer::CertificateRef, AdapterError> {
        if reference.round.context_id != expected_context_id
            || reference.proposal_round.context_id != expected_context_id
            || reference.proposal_round.height != reference.round.height
        {
            return Err(AdapterError::WireValidation(
                wire::ValidationError::WrongHeightContext,
            ));
        }
        if reference.proposal_round != reference.round {
            return Err(AdapterError::WireValidation(
                wire::ValidationError::InvalidProposalRound,
            ));
        }
        let round = reducer::Round::new(reference.round.height, reference.round.view);
        let proposal_round = reducer::Round::new(
            reference.proposal_round.height,
            reference.proposal_round.view,
        );
        let subject = self.register_subject(reference.subject)?;
        self.register_execution_commitment(
            proposal_round,
            subject,
            reference.execution_commitment,
        )?;
        Ok(reducer::CertificateRef::new_with_proposal_round(
            context_id(reference.round.context_id),
            round,
            proposal_round,
            Self::phase_to_core(reference.phase),
            subject,
        ))
    }

    /// Register the predecessor CommitQC frozen into a successor context.
    ///
    /// This is the sole certificate conversion that does not target the active
    /// registry context. The certificate remains bound to the predecessor and
    /// must name the same committed decision as the successor's immutable
    /// parent anchor; ordinary QCs continue through [`Self::qc_reference_to_core`].
    fn register_parent_qc(
        &mut self,
        certificate: &wire::QuorumCertificate,
    ) -> Result<reducer::CertificateRef, AdapterError> {
        let frozen = self
            .wire_context
            .as_ref()
            .and_then(|context| context.parent_commit_qc.as_ref())
            .map(wire::QuorumCertificate::as_ref)
            .ok_or(AdapterError::ParentContextMismatch)?;
        let reference = certificate.as_ref();
        if !reference.same_commit_decision(frozen) {
            return Err(AdapterError::ParentContextMismatch);
        }
        let core = self.qc_reference_to_core_for_context(&reference, frozen.round.context_id)?;
        self.certificates.insert(core, certificate.clone());
        Ok(core)
    }

    fn qc_to_core(
        &mut self,
        certificate: &wire::QuorumCertificate,
        context: &wire::HeightContext,
    ) -> Result<reducer::QuorumCertificate, AdapterError> {
        certificate.validate(context)?;
        let reference = self.qc_reference_to_core(&certificate.as_ref())?;
        let aggregate = aggregate_token(&certificate.aggregate_signature);
        let signatures = certificate
            .signers
            .iter()
            .map(|index| {
                Ok(reducer::SignatureShare::new(
                    self.validator_id(*index)?,
                    aggregate.clone(),
                ))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let core = reducer::QuorumCertificate::new(reference, signatures);
        self.certificates.insert(reference, certificate.clone());
        Ok(core)
    }

    fn qc_to_wire(
        &mut self,
        certificate: &reducer::QuorumCertificate,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::QuorumCertificate, AdapterError> {
        let signers = certificate
            .signatures()
            .iter()
            .map(|share| self.validator_index(share.signer()))
            .collect::<Result<Vec<_>, _>>()?;
        let aggregate_signature = aggregate_core_shares(certificate.signatures(), aggregator)?;
        let wire = wire::QuorumCertificate {
            round: self.round_to_wire(certificate.round()),
            proposal_round: self.round_to_wire(certificate.proposal_round()),
            phase: Self::phase_to_wire(certificate.phase()),
            subject: self.subject(certificate.subject())?,
            execution_commitment: self
                .execution_commitment(certificate.proposal_round(), certificate.subject())?,
            signers,
            aggregate_signature,
        };
        self.certificates
            .insert(certificate.reference(), wire.clone());
        Ok(wire)
    }

    /// Return whether a reducer QC retains this exact authenticated wire QC.
    ///
    /// Network QCs store the aggregate signature as the same opaque token on
    /// every reducer signature share. Comparing that token as well as the
    /// canonical signer order prevents a different certificate for the same
    /// round, phase, and subject from borrowing an existing deferred owner.
    fn reducer_qc_matches_wire(
        &self,
        queued: &reducer::QuorumCertificate,
        candidate: &wire::QuorumCertificate,
    ) -> bool {
        if self.round_to_wire(queued.round()) != candidate.round
            || self.round_to_wire(queued.proposal_round()) != candidate.proposal_round
            || Self::phase_to_wire(queued.phase()) != candidate.phase
            || !self
                .subject(queued.subject())
                .is_ok_and(|subject| subject == candidate.subject)
            || !self
                .execution_commitment(queued.proposal_round(), queued.subject())
                .is_ok_and(|commitment| commitment == candidate.execution_commitment)
            || queued.signatures().len() != candidate.signers.len()
        {
            return false;
        }
        let aggregate = aggregate_token(&candidate.aggregate_signature);
        queued
            .signatures()
            .iter()
            .zip(&candidate.signers)
            .all(|(share, signer)| {
                self.validator_index(share.signer())
                    .is_ok_and(|index| index == *signer)
                    && share.signature() == &aggregate
            })
    }

    fn timeout_vote_to_core(
        &mut self,
        vote: &wire::TimeoutVote,
        context: &wire::HeightContext,
    ) -> Result<reducer::SignedTimeoutVote, AdapterError> {
        vote.validate(context)?;
        let round = self.round_to_core(vote.round, context)?;
        let highest = vote
            .highest_prepare_qc
            .as_ref()
            .map(|certificate| self.qc_to_core(certificate, context))
            .transpose()?;
        Ok(reducer::SignedTimeoutVote::new(
            reducer::TimeoutVote::new(
                context_id(vote.round.context_id),
                round,
                self.validator_id(vote.signer)?,
                highest,
            ),
            reducer::OpaqueSignature::new(vote.signature.clone()),
        ))
    }

    fn unsigned_timeout_vote_to_wire(
        &mut self,
        vote: &reducer::TimeoutVote,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::TimeoutVote, AdapterError> {
        let highest_prepare_qc = vote
            .highest_prepare()
            .map(|certificate| self.qc_to_wire(certificate, aggregator))
            .transpose()?;
        Ok(wire::TimeoutVote {
            round: self.round_to_wire(vote.round()),
            highest_prepare_qc,
            signer: self.validator_index(vote.signer())?,
            signature: Vec::new(),
        })
    }

    fn signed_timeout_vote_to_wire(
        &mut self,
        vote: &reducer::SignedTimeoutVote,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::TimeoutVote, AdapterError> {
        let mut wire = self.unsigned_timeout_vote_to_wire(&vote.vote(), aggregator)?;
        wire.signature = vote.signature().as_bytes().to_vec();
        Ok(wire)
    }

    fn tc_to_core(
        &mut self,
        certificate: &wire::TimeoutCertificate,
        context: &wire::HeightContext,
    ) -> Result<reducer::TimeoutCertificate, AdapterError> {
        certificate.validate(context)?;
        let round = self.round_to_core(certificate.round, context)?;
        let groups = certificate
            .groups
            .iter()
            .map(|group| {
                let high = group
                    .highest_prepare_qc
                    .as_ref()
                    .map(|certificate| self.qc_to_core(certificate, context))
                    .transpose()?;
                let aggregate = aggregate_token(&group.aggregate_signature);
                let signatures = group
                    .signers
                    .iter()
                    .map(|index| {
                        Ok(reducer::SignatureShare::new(
                            self.validator_id(*index)?,
                            aggregate.clone(),
                        ))
                    })
                    .collect::<Result<Vec<_>, AdapterError>>()?;
                Ok(reducer::TimeoutSignatureGroup::new(high, signatures))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let core = reducer::TimeoutCertificate::new(
            context_id(certificate.round.context_id),
            round,
            groups,
        );
        Ok(core)
    }

    fn tc_to_wire(
        &mut self,
        certificate: &reducer::TimeoutCertificate,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::TimeoutCertificate, AdapterError> {
        let groups = certificate
            .groups()
            .iter()
            .map(|group| {
                let highest_prepare_qc = group
                    .highest_prepare()
                    .map(|certificate| self.qc_to_wire(certificate, aggregator))
                    .transpose()?;
                let signers = group
                    .signatures()
                    .iter()
                    .map(|share| self.validator_index(share.signer()))
                    .collect::<Result<Vec<_>, AdapterError>>()?;
                let aggregate_signature = aggregate_core_shares(group.signatures(), aggregator)?;
                Ok(wire::TimeoutVoteGroup {
                    highest_prepare_qc,
                    signers,
                    aggregate_signature,
                })
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let wire = wire::TimeoutCertificate {
            round: self.round_to_wire(certificate.round()),
            groups,
        };
        Ok(wire)
    }

    fn manifest_to_core(
        &mut self,
        manifest: &wire::PayloadManifest,
        context: &wire::HeightContext,
    ) -> Result<reducer::PayloadManifest, AdapterError> {
        manifest.validate(context)?;
        let round = self.round_to_core(manifest.round, context)?;
        let subject = self.register_subject(manifest.subject)?;
        let chunk_count = u32::try_from(manifest.chunk_hashes.len())
            .map_err(|_| wire::ValidationError::ChunkCountTooLarge)?;
        if self
            .manifests
            .get(&(round, subject))
            .is_some_and(|existing| existing != manifest)
        {
            return Err(AdapterError::ConflictingManifest);
        }
        self.manifests.insert((round, subject), manifest.clone());
        Ok(reducer::PayloadManifest::new(
            subject,
            reducer::Digest::new(*manifest.subject.payload_hash.as_ref()),
            reducer::Digest::new(*manifest.chunk_root.as_ref()),
            manifest.payload_size_bytes,
            chunk_count,
        ))
    }

    fn manifest_to_wire(
        &self,
        round: reducer::Round,
        manifest: &reducer::PayloadManifest,
    ) -> Result<wire::PayloadManifest, AdapterError> {
        self.manifests
            .get(&(round, manifest.subject()))
            .cloned()
            .ok_or(AdapterError::MissingManifest)
    }

    fn proposal_to_core(
        &mut self,
        proposal: &wire::Proposal,
        context: &wire::HeightContext,
    ) -> Result<reducer::SignedProposal, AdapterError> {
        let core_proposal = self.proposal_body_to_core(proposal, context)?;
        Ok(reducer::SignedProposal::new(
            core_proposal,
            reducer::OpaqueSignature::new(proposal.signature.clone()),
        ))
    }

    fn proposal_body_to_core(
        &mut self,
        proposal: &wire::Proposal,
        context: &wire::HeightContext,
    ) -> Result<reducer::Proposal, AdapterError> {
        let round = self.round_to_core(proposal.round, context)?;
        if proposal.manifest.round != proposal.round
            || proposal.manifest.subject != proposal.subject
        {
            return Err(AdapterError::InvalidProposalJustification);
        }
        let manifest = self.manifest_to_core(&proposal.manifest, context)?;
        let justification = self.justification_to_core(&proposal.justification, context)?;
        let core_proposal = reducer::Proposal::new(
            context_id(proposal.round.context_id),
            round,
            self.validator_id(proposal.proposer)?,
            manifest,
            justification,
        );
        let subject = core_proposal.manifest().subject();
        // Replay may contain the same semantic proposal intent more than once.
        // The reducer identity intentionally reduces full certificates to
        // stable references, while the leader signature covers the exact wire
        // justification. Preserve the first durable envelope so a later
        // same-reference certificate variant cannot retarget re-signing.
        self.proposals
            .entry((round, subject))
            .or_insert_with(|| proposal.clone());
        Ok(core_proposal)
    }

    fn justification_to_core(
        &mut self,
        justification: &wire::ProposalJustification,
        context: &wire::HeightContext,
    ) -> Result<reducer::ProposalJustification, AdapterError> {
        match justification {
            wire::ProposalJustification::ParentCommit(parent) => {
                let reference = parent
                    .certificate
                    .as_ref()
                    .map(|certificate| self.register_parent_qc(certificate))
                    .transpose()?;
                Ok(reducer::ProposalJustification::ParentCommit(reference))
            }
            wire::ProposalJustification::Timeout(timeout) => {
                let selected = timeout.timeout_certificate.highest_prepare_qc();
                if selected != timeout.highest_prepare_qc.as_ref() {
                    return Err(AdapterError::InvalidProposalJustification);
                }
                let certificate = self.tc_to_core(&timeout.timeout_certificate, context)?;
                Ok(reducer::ProposalJustification::Timeout(certificate))
            }
        }
    }

    fn justification_to_wire(
        &mut self,
        justification: &reducer::ProposalJustification,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::ProposalJustification, AdapterError> {
        match justification {
            reducer::ProposalJustification::ParentCommit(reference) => {
                let certificate = reference
                    .map(|reference| {
                        self.certificates
                            .get(&reference)
                            .cloned()
                            .ok_or(AdapterError::MissingCertificate)
                    })
                    .transpose()?;
                Ok(wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate },
                ))
            }
            reducer::ProposalJustification::Timeout(certificate) => {
                let timeout_certificate = self.tc_to_wire(certificate, aggregator)?;
                let highest_prepare_qc = certificate
                    .highest_prepare()
                    .map(|certificate| self.qc_to_wire(certificate, aggregator))
                    .transpose()?;
                Ok(wire::ProposalJustification::Timeout(
                    wire::TimeoutJustification {
                        timeout_certificate,
                        highest_prepare_qc,
                    },
                ))
            }
        }
    }

    fn unsigned_proposal_to_wire(
        &mut self,
        proposal: &reducer::Proposal,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::Proposal, AdapterError> {
        let key = (proposal.round(), proposal.manifest().subject());
        if let Some(cached) = self.proposals.get(&key) {
            let mut cached = cached.clone();
            cached.signature.clear();
            return Ok(cached);
        }
        let manifest = self.manifest_to_wire(proposal.round(), proposal.manifest())?;
        let wire = wire::Proposal {
            round: self.round_to_wire(proposal.round()),
            proposer: self.validator_index(proposal.proposer())?,
            subject: self.subject(proposal.manifest().subject())?,
            manifest,
            justification: self.justification_to_wire(proposal.justification(), aggregator)?,
            signature: Vec::new(),
        };
        self.proposals.insert(key, wire.clone());
        Ok(wire)
    }

    fn signed_proposal_to_wire(
        &mut self,
        proposal: &reducer::SignedProposal,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::Proposal, AdapterError> {
        let mut wire = self.unsigned_proposal_to_wire(proposal.proposal(), aggregator)?;
        wire.signature = proposal.signature().as_bytes().to_vec();
        self.proposals.insert(
            (
                proposal.proposal().round(),
                proposal.proposal().manifest().subject(),
            ),
            wire.clone(),
        );
        Ok(wire)
    }

    fn message_to_wire(
        &mut self,
        message: reducer::ConsensusMessageV2,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::ConsensusMessageV2, AdapterError> {
        let payload = match message {
            reducer::ConsensusMessageV2::Proposal(proposal) => {
                wire::ConsensusMessageV2Payload::Proposal(
                    self.signed_proposal_to_wire(&proposal, aggregator)?,
                )
            }
            reducer::ConsensusMessageV2::Vote(vote) => {
                wire::ConsensusMessageV2Payload::Vote(self.signed_vote_to_wire(&vote)?)
            }
            reducer::ConsensusMessageV2::QuorumCertificate(certificate) => {
                wire::ConsensusMessageV2Payload::QuorumCertificate(
                    self.qc_to_wire(&certificate, aggregator)?,
                )
            }
            reducer::ConsensusMessageV2::TimeoutVote(vote) => {
                wire::ConsensusMessageV2Payload::TimeoutVote(
                    self.signed_timeout_vote_to_wire(&vote, aggregator)?,
                )
            }
            reducer::ConsensusMessageV2::TimeoutCertificate(certificate) => {
                wire::ConsensusMessageV2Payload::TimeoutCertificate(
                    self.tc_to_wire(&certificate, aggregator)?,
                )
            }
            reducer::ConsensusMessageV2::BodyRequest(_)
            | reducer::ConsensusMessageV2::BodyChunk(_) => {
                return Err(AdapterError::TransportPayload);
            }
        };
        Ok(wire::ConsensusMessageV2::new(payload))
    }

    fn encode_wal_entry(
        &mut self,
        entry: &reducer::WalEntry,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<Vec<u8>, AdapterError> {
        let record = match entry.record() {
            reducer::WalRecord::ProposalIntent(proposal) => {
                WalRecordV2::ProposalIntent(self.unsigned_proposal_to_wire(proposal, aggregator)?)
            }
            reducer::WalRecord::PrepareIntent(vote) => {
                WalRecordV2::PrepareIntent(self.unsigned_vote_to_wire(*vote)?)
            }
            reducer::WalRecord::ObservePrepare(certificate) => {
                WalRecordV2::ObservePrepare(self.qc_to_wire(certificate, aggregator)?)
            }
            reducer::WalRecord::LockAndCommit { prepare, vote } => WalRecordV2::LockAndCommit {
                prepare: self.qc_to_wire(prepare, aggregator)?,
                vote: self.unsigned_vote_to_wire(*vote)?,
            },
            reducer::WalRecord::TimeoutIntent(vote) => {
                WalRecordV2::TimeoutIntent(self.unsigned_timeout_vote_to_wire(vote, aggregator)?)
            }
            reducer::WalRecord::InstallTimeout(certificate) => {
                WalRecordV2::InstallTimeout(self.tc_to_wire(certificate, aggregator)?)
            }
            reducer::WalRecord::Decision(certificate) => {
                WalRecordV2::Decision(self.qc_to_wire(certificate, aggregator)?)
            }
        };
        Ok(WalEnvelopeV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            persistence_id: entry.id().get(),
            record,
        }
        .encode())
    }

    fn decode_wal_entry(
        &mut self,
        sequence: u64,
        payload: &[u8],
        parent_verification: Option<&ParentVerificationContext>,
        proofs_of_possession: &[Vec<u8>],
    ) -> Result<reducer::WalEntry, AdapterError> {
        let mut input = payload;
        let envelope = WalEnvelopeV2::decode(&mut input)
            .map_err(|error| AdapterError::WalDecode(error.to_string()))?;
        if !input.is_empty() {
            return Err(AdapterError::WalDecode(
                "trailing bytes after complete record".to_owned(),
            ));
        }
        if envelope.protocol_version != wire::PROTOCOL_VERSION {
            return Err(AdapterError::WalDecode(format!(
                "unsupported protocol version {}",
                envelope.protocol_version
            )));
        }
        if sequence.checked_add(1) != Some(envelope.persistence_id) {
            return Err(AdapterError::WalSequenceMismatch {
                frame_sequence: sequence,
                persistence_id: envelope.persistence_id,
            });
        }
        let wire_context = self
            .wire_context
            .as_ref()
            .expect("registry is constructed with a height context");
        verify_wal_record_authority(
            wire_context,
            parent_verification,
            &envelope.record,
            proofs_of_possession,
        )?;
        // The registry is already bound to this immutable context identifier;
        // reducer replay performs the remaining height and safety checks.
        let wire_context_id = self
            .context_id
            .expect("registry is constructed with a height context");
        let context_height = match &envelope.record {
            WalRecordV2::ProposalIntent(proposal) => proposal.round.height,
            WalRecordV2::PrepareIntent(vote) | WalRecordV2::LockAndCommit { vote, .. } => {
                vote.round.height
            }
            WalRecordV2::ObservePrepare(certificate) | WalRecordV2::Decision(certificate) => {
                certificate.round.height
            }
            WalRecordV2::TimeoutIntent(vote) => vote.round.height,
            WalRecordV2::InstallTimeout(certificate) => certificate.round.height,
        };
        if context_height == 0 {
            return Err(AdapterError::WalDecode("zero consensus height".to_owned()));
        }
        let round = |wire_round: wire::ConsensusRound| {
            if wire_round.context_id != wire_context_id || wire_round.height != context_height {
                Err(AdapterError::WireValidation(
                    wire::ValidationError::WrongHeightContext,
                ))
            } else {
                Ok(reducer::Round::new(wire_round.height, wire_round.view))
            }
        };
        let record = match envelope.record {
            WalRecordV2::ProposalIntent(proposal) => {
                let context = self
                    .wire_context
                    .clone()
                    .expect("registry is constructed with a height context");
                reducer::WalRecord::ProposalIntent(self.proposal_body_to_core(&proposal, &context)?)
            }
            WalRecordV2::PrepareIntent(vote) => {
                vote.execution_commitment.validate()?;
                let core_round = round(vote.round)?;
                let proposal_round = round(vote.proposal_round)?;
                let subject = self.register_subject(vote.subject)?;
                self.register_execution_commitment(
                    proposal_round,
                    subject,
                    vote.execution_commitment,
                )?;
                reducer::WalRecord::PrepareIntent(reducer::Vote::new_with_proposal_round(
                    context_id(wire_context_id),
                    core_round,
                    proposal_round,
                    Self::phase_to_core(vote.phase),
                    subject,
                    self.validator_id(vote.signer)?,
                ))
            }
            WalRecordV2::ObservePrepare(certificate) => {
                reducer::WalRecord::ObservePrepare(self.qc_to_core_unchecked(&certificate)?)
            }
            WalRecordV2::LockAndCommit { prepare, vote } => {
                vote.execution_commitment.validate()?;
                let core_round = round(vote.round)?;
                let proposal_round = round(vote.proposal_round)?;
                let subject = self.register_subject(vote.subject)?;
                self.register_execution_commitment(
                    proposal_round,
                    subject,
                    vote.execution_commitment,
                )?;
                reducer::WalRecord::LockAndCommit {
                    prepare: self.qc_to_core_unchecked(&prepare)?,
                    vote: reducer::Vote::new_with_proposal_round(
                        context_id(wire_context_id),
                        core_round,
                        proposal_round,
                        Self::phase_to_core(vote.phase),
                        subject,
                        self.validator_id(vote.signer)?,
                    ),
                }
            }
            WalRecordV2::TimeoutIntent(vote) => {
                let core_round = round(vote.round)?;
                let high = vote
                    .highest_prepare_qc
                    .as_ref()
                    .map(|certificate| self.qc_to_core_unchecked(certificate))
                    .transpose()?;
                reducer::WalRecord::TimeoutIntent(reducer::TimeoutVote::new(
                    context_id(wire_context_id),
                    core_round,
                    self.validator_id(vote.signer)?,
                    high,
                ))
            }
            WalRecordV2::InstallTimeout(certificate) => {
                reducer::WalRecord::InstallTimeout(self.tc_to_core_unchecked(&certificate)?)
            }
            WalRecordV2::Decision(certificate) => {
                reducer::WalRecord::Decision(self.qc_to_core_unchecked(&certificate)?)
            }
        };
        Ok(reducer::WalEntry::new(
            reducer::PersistenceId::new(envelope.persistence_id),
            record,
        ))
    }

    fn qc_to_core_unchecked(
        &mut self,
        certificate: &wire::QuorumCertificate,
    ) -> Result<reducer::QuorumCertificate, AdapterError> {
        let reference = self.qc_reference_to_core(&certificate.as_ref())?;
        let aggregate = aggregate_token(&certificate.aggregate_signature);
        let signatures = certificate
            .signers
            .iter()
            .map(|index| {
                Ok(reducer::SignatureShare::new(
                    self.validator_id(*index)?,
                    aggregate.clone(),
                ))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        let core = reducer::QuorumCertificate::new(reference, signatures);
        self.certificates.insert(reference, certificate.clone());
        Ok(core)
    }

    fn tc_to_core_unchecked(
        &mut self,
        certificate: &wire::TimeoutCertificate,
    ) -> Result<reducer::TimeoutCertificate, AdapterError> {
        let round = reducer::Round::new(certificate.round.height, certificate.round.view);
        let groups = certificate
            .groups
            .iter()
            .map(|group| {
                let highest = group
                    .highest_prepare_qc
                    .as_ref()
                    .map(|certificate| self.qc_to_core_unchecked(certificate))
                    .transpose()?;
                let aggregate = aggregate_token(&group.aggregate_signature);
                let signatures = group
                    .signers
                    .iter()
                    .map(|index| {
                        Ok(reducer::SignatureShare::new(
                            self.validator_id(*index)?,
                            aggregate.clone(),
                        ))
                    })
                    .collect::<Result<Vec<_>, AdapterError>>()?;
                Ok(reducer::TimeoutSignatureGroup::new(highest, signatures))
            })
            .collect::<Result<Vec<_>, AdapterError>>()?;
        Ok(reducer::TimeoutCertificate::new(
            context_id(certificate.round.context_id),
            round,
            groups,
        ))
    }
}

fn verify_proposal_justification_authority(
    context: &wire::HeightContext,
    parent_verification: Option<&ParentVerificationContext>,
    justification: &wire::ProposalJustification,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    match justification {
        wire::ProposalJustification::ParentCommit(parent) => {
            match (&parent.certificate, parent_verification) {
                (None, None) if context.height == 1 || context.snapshot_bootstrap.is_some() => {
                    Ok(())
                }
                (Some(certificate), Some(parent_verification)) => verify_quorum_certificate(
                    &parent_verification.context,
                    certificate,
                    &parent_verification.proofs_of_possession,
                ),
                (None, None) | (None, Some(_)) | (Some(_), None) => {
                    Err(AdapterError::ParentContextMismatch)
                }
            }
        }
        wire::ProposalJustification::Timeout(timeout) => {
            verify_timeout_certificate(
                context,
                &timeout.timeout_certificate,
                proofs_of_possession,
            )?;
            if let Some(highest) = &timeout.highest_prepare_qc {
                verify_quorum_certificate(context, highest, proofs_of_possession)?;
            }
            Ok(())
        }
    }
}

/// Reauthenticate every external authority proof embedded in one durable WAL
/// record before reducer replay may consume it.
///
/// WAL frame checksums and identity hashes detect corruption and accidental
/// key/configuration drift; they are not signatures by a remote quorum. Local
/// unsigned intents therefore remain inside the trusted-storage boundary, but
/// every carried QC and TC must still verify under its frozen roster. Requiring
/// empty intent-signature fields also prevents ignored wire bytes from aliasing
/// the same durable core intent.
fn verify_wal_record_authority(
    context: &wire::HeightContext,
    parent_verification: Option<&ParentVerificationContext>,
    record: &WalRecordV2,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    let require_unsigned = |signature: &[u8], kind: &str| {
        if signature.is_empty() {
            Ok(())
        } else {
            Err(AdapterError::WalDecode(format!(
                "{kind} must not carry signature bytes"
            )))
        }
    };
    match record {
        WalRecordV2::ProposalIntent(proposal) => {
            require_unsigned(&proposal.signature, "ProposalIntent")?;
            verify_proposal_justification_authority(
                context,
                parent_verification,
                &proposal.justification,
                proofs_of_possession,
            )
        }
        WalRecordV2::PrepareIntent(vote) => require_unsigned(&vote.signature, "PrepareIntent"),
        WalRecordV2::ObservePrepare(certificate) => {
            verify_quorum_certificate(context, certificate, proofs_of_possession)
        }
        WalRecordV2::LockAndCommit { prepare, vote } => {
            require_unsigned(&vote.signature, "LockAndCommit vote")?;
            verify_quorum_certificate(context, prepare, proofs_of_possession)
        }
        WalRecordV2::TimeoutIntent(vote) => {
            require_unsigned(&vote.signature, "TimeoutIntent")?;
            if let Some(highest) = &vote.highest_prepare_qc {
                verify_quorum_certificate(context, highest, proofs_of_possession)?;
            }
            Ok(())
        }
        WalRecordV2::InstallTimeout(certificate) => {
            verify_timeout_certificate(context, certificate, proofs_of_possession)
        }
        WalRecordV2::Decision(certificate) => {
            verify_quorum_certificate(context, certificate, proofs_of_possession)
        }
    }
}

fn verify_authenticated_message(
    context: &wire::HeightContext,
    parent_verification: Option<&ParentVerificationContext>,
    message: &wire::ConsensusMessageV2,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    message.validate_version()?;
    // `SumeragiV2Adapter` can only be built from `VerifiedHeightContext`,
    // which has already validated the immutable context, every BLS key, and
    // the complete aligned PoP vector. Do not rescan the boundary snapshot for
    // every hostile ingress message.

    match &message.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => {
            proposal.validate(context)?;
            verify_individual_signature(
                context,
                proposal.proposer,
                &proposal.signature,
                &proposal.signature_preimage(),
            )?;
            verify_proposal_justification_authority(
                context,
                parent_verification,
                &proposal.justification,
                proofs_of_possession,
            )
        }
        wire::ConsensusMessageV2Payload::Vote(vote) => {
            vote.validate(context)?;
            verify_individual_signature(
                context,
                vote.signer,
                &vote.signature,
                &vote.signature_preimage(),
            )
        }
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
            verify_quorum_certificate(context, certificate, proofs_of_possession)
        }
        wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
            vote.validate(context)?;
            if let Some(highest) = &vote.highest_prepare_qc {
                verify_quorum_certificate(context, highest, proofs_of_possession)?;
            }
            verify_individual_signature(
                context,
                vote.signer,
                &vote.signature,
                &vote.signature_preimage(),
            )
        }
        wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
            verify_timeout_certificate(context, certificate, proofs_of_possession)
        }
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_) => Err(AdapterError::TransportPayload),
    }
}

fn verify_roster_proofs(
    context: &wire::HeightContext,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    wire::finality::verify_validator_roster_pops(context, proofs_of_possession).map_err(|error| {
        match error {
            wire::finality::V2QuorumCertificateVerificationError::ProofOfPossessionCount {
                expected,
                actual,
            } => AdapterError::ProofOfPossessionCount { expected, actual },
            other => AdapterError::Cryptography(other.to_string()),
        }
    })
}

fn verify_next_epoch_snapshot_proofs(context: &wire::HeightContext) -> Result<(), AdapterError> {
    let Some(snapshot) = &context.next_epoch_snapshot else {
        return Ok(());
    };
    wire::finality::verify_validator_power_roster_pops(
        &snapshot.roster,
        &snapshot.validator_set_pops,
    )
    .map_err(|error| AdapterError::Cryptography(error.to_string()))
}

fn verify_individual_signature(
    context: &wire::HeightContext,
    signer: wire::ValidatorIndex,
    signature: &[u8],
    preimage: &[u8],
) -> Result<(), AdapterError> {
    let index = usize::try_from(signer)
        .ok()
        .filter(|index| *index < context.roster.len())
        .ok_or(AdapterError::ValidatorIndexOutOfRange(signer))?;
    let signature = Signature::try_from_bytes(signature)
        .map_err(|error| AdapterError::Cryptography(error.to_string()))?;
    signature
        .verify(context.roster[index].validator.public_key(), preimage)
        .map_err(|error| AdapterError::Cryptography(error.to_string()))
}

fn verify_quorum_certificate(
    context: &wire::HeightContext,
    certificate: &wire::QuorumCertificate,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    wire::finality::verify_quorum_certificate_with_validator_pops(
        context,
        certificate,
        proofs_of_possession,
    )
    .map_err(|error| match error {
        wire::finality::V2QuorumCertificateVerificationError::InvalidCertificate(error) => {
            AdapterError::WireValidation(error)
        }
        wire::finality::V2QuorumCertificateVerificationError::ProofOfPossessionCount {
            expected,
            actual,
        } => AdapterError::ProofOfPossessionCount { expected, actual },
        other => AdapterError::Cryptography(other.to_string()),
    })
}

/// Verify one certificate against immutable historical context authority.
///
/// This deliberately reuses the exact production roster-PoP and aggregate
/// verifier used by live reducer ingress; block sync does not maintain a
/// second certificate-validation implementation.
pub(crate) fn verify_historical_quorum_certificate(
    context: &wire::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    certificate: &wire::QuorumCertificate,
) -> Result<(), AdapterError> {
    context.validate()?;
    verify_roster_proofs(context, proofs_of_possession)?;
    verify_quorum_certificate(context, certificate, proofs_of_possession)
}

fn verify_timeout_certificate(
    context: &wire::HeightContext,
    certificate: &wire::TimeoutCertificate,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    certificate.validate(context)?;
    for group in &certificate.groups {
        if let Some(highest) = &group.highest_prepare_qc {
            verify_quorum_certificate(context, highest, proofs_of_possession)?;
        }
        let signer = group
            .signers
            .first()
            .copied()
            .ok_or(wire::ValidationError::EmptyTimeoutGroup)?;
        let preimage = wire::TimeoutVote {
            round: certificate.round,
            highest_prepare_qc: group.highest_prepare_qc.clone(),
            signer,
            signature: Vec::new(),
        }
        .signature_preimage();
        verify_aggregate_signature(
            context,
            &group.signers,
            &group.aggregate_signature,
            &preimage,
            proofs_of_possession,
        )?;
    }
    Ok(())
}

fn verify_aggregate_signature(
    context: &wire::HeightContext,
    signers: &[wire::ValidatorIndex],
    aggregate_signature: &[u8],
    preimage: &[u8],
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    let mut public_keys = Vec::with_capacity(signers.len());
    let mut pops = Vec::with_capacity(signers.len());
    for signer in signers {
        let index = usize::try_from(*signer)
            .ok()
            .filter(|index| *index < context.roster.len() && *index < proofs_of_possession.len())
            .ok_or(AdapterError::ValidatorIndexOutOfRange(*signer))?;
        public_keys.push(context.roster[index].validator.public_key());
        pops.push(proofs_of_possession[index].as_slice());
    }
    #[cfg(feature = "bls")]
    {
        iroha_crypto::bls_normal_verify_preaggregated_same_message(
            preimage,
            aggregate_signature,
            &public_keys,
            &pops,
        )
        .map_err(|error| AdapterError::Cryptography(error.to_string()))
    }
    #[cfg(not(feature = "bls"))]
    {
        let _ = (public_keys, pops, aggregate_signature, preimage);
        Err(AdapterError::Cryptography(
            "the iroha_core `bls` feature is required by Sumeragi v2".to_owned(),
        ))
    }
}

fn validator_token(index: wire::ValidatorIndex) -> reducer::ValidatorId {
    let mut bytes = [0_u8; 32];
    bytes[28..].copy_from_slice(&index.to_be_bytes());
    reducer::ValidatorId::new(bytes)
}

fn context_id(id: wire::HeightContextId) -> reducer::ContextId {
    reducer::ContextId::new(*id.0.as_ref())
}

fn aggregate_token(signature: &[u8]) -> reducer::OpaqueSignature {
    let mut token = Vec::with_capacity(AGGREGATE_TOKEN_PREFIX.len() + signature.len());
    token.extend_from_slice(AGGREGATE_TOKEN_PREFIX);
    token.extend_from_slice(signature);
    reducer::OpaqueSignature::new(token)
}

fn aggregate_core_shares(
    shares: &[reducer::SignatureShare],
    aggregator: &dyn SignatureAggregator,
) -> Result<Vec<u8>, AdapterError> {
    let signatures = shares
        .iter()
        .map(|share| share.signature().as_bytes())
        .collect::<Vec<_>>();
    if let Some(first) = signatures.first()
        && let Some(aggregate) = first.strip_prefix(AGGREGATE_TOKEN_PREFIX)
    {
        if signatures
            .iter()
            .all(|signature| signature.strip_prefix(AGGREGATE_TOKEN_PREFIX) == Some(aggregate))
        {
            return Ok(aggregate.to_vec());
        }
        return Err(AdapterError::SignatureAggregation(
            "verified aggregate tokens disagree within one certificate".to_owned(),
        ));
    }
    if signatures
        .iter()
        .any(|signature| signature.starts_with(AGGREGATE_TOKEN_PREFIX))
    {
        return Err(AdapterError::SignatureAggregation(
            "verified aggregate tokens cannot be mixed with signature shares".to_owned(),
        ));
    }
    aggregator
        .aggregate(&signatures)
        .map_err(AdapterError::SignatureAggregation)
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Write as _, time::Duration};

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use tempfile::TempDir;

    use super::super::serviced_candidate_store::ProducerContinuationSourceClass;
    use super::*;

    #[derive(Debug)]
    struct TestAggregator;

    impl SignatureAggregator for TestAggregator {
        fn aggregate(&self, signatures: &[&[u8]]) -> Result<Vec<u8>, String> {
            let mut aggregate = Vec::new();
            for signature in signatures {
                aggregate.extend_from_slice(
                    &u32::try_from(signature.len())
                        .map_err(|error| error.to_string())?
                        .to_le_bytes(),
                );
                aggregate.extend_from_slice(signature);
            }
            Ok(aggregate)
        }
    }

    fn peer(seed: u8) -> PeerId {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("deterministic peer key");
        PeerId::new(key.public_key().clone())
    }

    fn context() -> wire::HeightContext {
        let mut roster = (1_u8..=4)
            .map(|seed| wire::ValidatorPower {
                validator: peer(seed),
                power: 1,
            })
            .collect::<Vec<_>>();
        roster.sort();
        wire::HeightContext {
            chain_id: "sumeragi-v2-adapter-test".into(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1024 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0xA5; 32],
        }
    }

    fn verified_genesis(context: wire::HeightContext) -> VerifiedHeightContext {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS-normal key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        assert!(
            keys.iter()
                .zip(&context.roster)
                .all(|(key, entry)| key.public_key() == entry.validator.public_key())
        );
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("BLS proof of possession")
            })
            .collect();
        VerifiedHeightContext::genesis(context, proofs).expect("verified genesis context")
    }

    #[test]
    fn deferred_adapter_activation_marker_survives_a_no_progress_publication() {
        let _guard = crate::sumeragi::status::rbc_status_test_guard();
        crate::sumeragi::status::clear_v2_status();
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let (mut adapter, startup) = SumeragiV2Adapter::open_deferred_status(
            directory.path().join("deferred-status.wal"),
            verified_genesis(context.clone()),
            None,
            reducer::Generation::new(context.height),
            [0xA6; 32],
            AdapterFingerprints {
                node: Hash::new(b"deferred node"),
                build: Hash::new(b"deferred build"),
                config: Hash::new(b"deferred config"),
            },
            DeferredAdmissionOrdinalSource::new(1),
        )
        .expect("open replayed adapter without status publication");

        assert!(startup.is_empty());
        assert!(
            crate::sumeragi::status::v2_status().is_none(),
            "successor replay must remain invisible while its remaining constructors are fallible"
        );
        let prepared = adapter
            .successor_activation_status()
            .expect("prepare reducer-owned activation snapshot");
        assert_eq!(prepared.height, context.height);
        assert!(matches!(
            prepared.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                ..
            })
        ));
        assert!(
            crate::sumeragi::status::v2_status().is_none(),
            "preparing a snapshot is not publication"
        );
        crate::sumeragi::status::set_v2_status(prepared);

        let stale_tag = reducer::EventTag::new(
            context.height,
            0,
            reducer::Generation::new(context.height.saturating_sub(1)),
        );
        let ignored = adapter
            .retransmit_elapsed(stale_tag)
            .expect("publish an ignored post-activation retransmission");
        assert_eq!(
            ignored.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::StaleGeneration)
        );
        let republished = crate::sumeragi::status::v2_status().expect("republished status");
        assert!(matches!(
            republished.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                ..
            })
        ));
        crate::sumeragi::status::clear_v2_status();
    }

    #[test]
    fn executable_leader_rotation_matches_the_canonical_wire_context() {
        let wire_context = context();
        let mut registry = WireRegistry::new(&wire_context).expect("wire registry");
        let core_context = registry
            .core_context(&wire_context)
            .expect("executable context");

        for view in 0..=100 {
            let wire_leader = wire_context.leader(view);
            assert_eq!(
                registry
                    .validator_index(core_context.leader(view))
                    .expect("core leader maps to wire roster"),
                wire_leader,
                "leader mismatch in view {view}"
            );
        }
    }

    #[test]
    fn successor_core_context_preserves_the_parent_certificate_binding() {
        let parent_context = context();
        let parent_round = wire::ConsensusRound {
            context_id: parent_context.id(),
            height: parent_context.height,
            view: 3,
        };
        let parent_qc = wire::QuorumCertificate {
            round: parent_round,
            proposal_round: parent_round,
            phase: wire::GlobalPhase::Commit,
            subject: subject(0x6d),
            execution_commitment: execution_commitment(0x6d),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x6d; 48],
        };
        let mut successor = parent_context.clone();
        successor.height += 1;
        successor.parent_commit_qc = Some(parent_qc);
        successor.validate().expect("structural successor context");
        let successor_id = successor.id();

        let mut registry = WireRegistry::new(&successor).expect("successor wire registry");
        let core_context = registry
            .core_context(&successor)
            .expect("parent-bound successor context");
        let core_parent = core_context
            .parent_commit()
            .expect("successor retains its parent CommitQC");

        assert_eq!(core_parent.context_id(), context_id(parent_context.id()));
        assert_ne!(core_parent.context_id(), context_id(successor_id));
        assert_eq!(core_parent.round().height(), parent_context.height);
        assert_eq!(core_parent.proposal_round().view(), parent_round.view);

        let parent_reference = successor
            .parent_commit_qc
            .as_ref()
            .expect("successor parent CommitQC")
            .as_ref();
        assert!(matches!(
            registry.qc_reference_to_core(&parent_reference),
            Err(AdapterError::WireValidation(
                wire::ValidationError::WrongHeightContext
            ))
        ));
    }

    #[cfg(feature = "bls")]
    fn authenticated_context() -> (wire::HeightContext, Vec<KeyPair>, Vec<Vec<u8>>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS-normal key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("BLS proof of possession")
            })
            .collect::<Vec<_>>();
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            chain_id: "sumeragi-v2-auth-test".into(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 3,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"authenticated nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1024 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0x5A; 32],
        };
        (context, keys, pops)
    }

    #[cfg(feature = "bls")]
    fn authenticate_qc(certificate: &mut wire::QuorumCertificate, keys: &[KeyPair]) {
        let signer = certificate
            .signers
            .first()
            .copied()
            .expect("fixture certificate has signers");
        let preimage = wire::Vote {
            round: certificate.round,
            proposal_round: certificate.proposal_round,
            phase: certificate.phase,
            subject: certificate.subject,
            execution_commitment: certificate.execution_commitment,
            signer,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = certificate
            .signers
            .iter()
            .map(|signer| {
                let index = usize::try_from(*signer).expect("small fixture signer index");
                Signature::new(keys[index].private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate fixture certificate");
    }

    #[cfg(feature = "bls")]
    #[test]
    fn height_context_rejects_missing_and_rogue_proofs_of_possession() {
        let (context, _keys, mut proofs) = authenticated_context();
        assert!(matches!(
            VerifiedHeightContext::genesis(context.clone(), proofs[..3].to_vec()),
            Err(AdapterError::ProofOfPossessionCount {
                expected: 4,
                actual: 3
            })
        ));
        proofs.swap(0, 1);
        assert!(matches!(
            VerifiedHeightContext::genesis(context, proofs),
            Err(AdapterError::Cryptography(_))
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn aggregate_verification_rejects_signer_without_aligned_pop() {
        let (context, _keys, proofs) = authenticated_context();
        let signer = u32::try_from(context.roster.len() - 1).expect("small fixture roster");

        assert!(matches!(
            verify_aggregate_signature(
                &context,
                &[signer],
                &[],
                b"missing aligned proof of possession",
                &proofs[..proofs.len() - 1],
            ),
            Err(AdapterError::ValidatorIndexOutOfRange(index)) if index == signer
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn boundary_context_rejects_missing_invalid_and_foreign_future_pops_before_voting() {
        let (mut context, _keys, proofs) = authenticated_context();
        context.epoch_end_height = context.height;
        context.next_epoch_snapshot = Some(wire::finality::FinalizedNextEpochSnapshot {
            epoch: context.epoch + 1,
            epoch_end_height: context.height + 10,
            mode: context.mode,
            roster: context.roster.clone(),
            validator_set_pops: proofs.clone(),
            quorum: context.quorum,
            leader_seed: [0x6A; 32],
        });
        VerifiedHeightContext::genesis(context.clone(), proofs.clone())
            .expect("valid future PoPs are admitted before voting");

        let mut missing = context.clone();
        missing
            .next_epoch_snapshot
            .as_mut()
            .expect("boundary snapshot")
            .validator_set_pops
            .pop();
        assert!(matches!(
            VerifiedHeightContext::genesis(missing, proofs.clone()),
            Err(AdapterError::WireValidation(
                wire::ValidationError::NextEpochProofOfPossessionCount
            ))
        ));

        let foreign_key =
            KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::BlsNormal).expect("foreign BLS key");
        let foreign_pop =
            iroha_crypto::bls_normal_pop_prove(foreign_key.private_key()).expect("foreign PoP");
        let mut foreign = context.clone();
        foreign
            .next_epoch_snapshot
            .as_mut()
            .expect("boundary snapshot")
            .validator_set_pops[0] = foreign_pop;
        assert!(matches!(
            VerifiedHeightContext::genesis(foreign, proofs.clone()),
            Err(AdapterError::Cryptography(_))
        ));

        let mut corrupted = context;
        corrupted
            .next_epoch_snapshot
            .as_mut()
            .expect("boundary snapshot")
            .validator_set_pops[0][0] ^= 0x80;
        assert!(matches!(
            VerifiedHeightContext::genesis(corrupted, proofs),
            Err(AdapterError::Cryptography(_))
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn successor_context_requires_the_durable_cryptographic_parent() {
        let (parent_context, keys, proofs) = authenticated_context();
        let parent_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent block")),
            payload_hash: Hash::new(b"parent payload"),
        };
        let round = wire::ConsensusRound {
            context_id: parent_context.id(),
            height: parent_context.height,
            view: 0,
        };
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x21),
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let parent_qc = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x21),
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate parent CommitQC"),
        };
        let artifact = wire::finality::V2FinalityArtifact::new(
            parent_context.clone(),
            parent_subject,
            parent_qc.clone(),
            proofs.clone(),
        );
        artifact.validate().expect("valid parent artifact");
        let receipt = KuraV2CommitReceipt::for_test(&artifact);
        let mut successor = parent_context.clone();
        successor.height = 2;
        successor.parent_commit_qc = Some(parent_qc.clone());

        let verified_successor = VerifiedHeightContext::successor(
            successor.clone(),
            proofs.clone(),
            &artifact,
            &receipt,
            &proofs,
        )
        .expect("durable verified parent anchors successor");

        let mut substituted_execution_policy = successor.clone();
        substituted_execution_policy.execution_policy_hash =
            Hash::new(b"substituted successor execution policy");
        assert!(matches!(
            VerifiedHeightContext::successor(
                substituted_execution_policy,
                proofs.clone(),
                &artifact,
                &receipt,
                &proofs,
            ),
            Err(AdapterError::ParentContextMismatch)
        ));

        let mut substituted_successor_pops = proofs.clone();
        substituted_successor_pops.swap(0, 1);
        assert!(matches!(
            VerifiedHeightContext::successor(
                successor.clone(),
                substituted_successor_pops,
                &artifact,
                &receipt,
                &proofs,
            ),
            Err(AdapterError::EpochTransitionMismatch)
        ));

        let mut substituted_parent_artifact = artifact.clone();
        substituted_parent_artifact.validator_set_pops.swap(0, 1);
        let substituted_receipt = KuraV2CommitReceipt::for_test(&substituted_parent_artifact);
        assert!(matches!(
            VerifiedHeightContext::successor(
                successor.clone(),
                proofs.clone(),
                &substituted_parent_artifact,
                &substituted_receipt,
                &proofs,
            ),
            Err(AdapterError::ParentContextMismatch)
        ));

        // The same parent decision can acquire a valid CommitQC in another
        // view. Semantic proposal admission accepts it, but the authentication
        // boundary must still verify that alternate certificate under the
        // retained parent roster rather than trusting the leader signature.
        let alternate_round = wire::ConsensusRound {
            view: round.view + 1,
            ..round
        };
        let alternate_preimage = wire::Vote {
            round: alternate_round,
            proposal_round: alternate_round,
            phase: wire::GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x21),
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let alternate_shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &alternate_preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let alternate_refs = alternate_shares
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let alternate_parent_qc = wire::QuorumCertificate {
            round: alternate_round,
            proposal_round: alternate_round,
            phase: wire::GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x21),
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&alternate_refs)
                .expect("aggregate alternate parent CommitQC"),
        };
        let proposal_round = wire::ConsensusRound {
            context_id: successor.id(),
            height: successor.height,
            view: 0,
        };
        let proposal_subject = subject(0x72);
        let proposal_body = b"parent-auth-body".to_vec();
        let manifest = wire::PayloadManifest::derive(
            &successor,
            proposal_round,
            proposal_subject,
            u64::try_from(proposal_body.len()).expect("fixture body length fits u64"),
            &[proposal_body],
        )
        .expect("valid successor manifest");
        let proposer = successor.leader(0);
        let mut proposal = wire::Proposal {
            round: proposal_round,
            proposer,
            subject: proposal_subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification {
                    certificate: Some(alternate_parent_qc),
                },
            ),
            signature: Vec::new(),
        };
        proposal.signature = Signature::new(
            keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
            &proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("successor-safety.wal"),
            verified_successor.clone(),
            None,
            reducer::Generation::new(2),
            [0x62; 32],
            fingerprints(),
            Box::new(TestAggregator),
            DeferredAdmissionOrdinalSource::new(1),
        )
        .expect("open successor adapter");
        assert!(startup.is_empty());
        let authenticated = adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
            ))
            .expect("alternate-view parent CommitQC is cryptographically verified");

        let mut alternate_registry = adapter.registry.clone();
        alternate_registry
            .proposal_to_core(&proposal, &successor)
            .expect("alternate-view parent CommitQC retains the durable parent decision");

        let foreign_parent_subject = subject(0x73);
        let foreign_parent_commitment = execution_commitment(0x73);
        let foreign_preimage = wire::Vote {
            round: alternate_round,
            proposal_round: alternate_round,
            phase: wire::GlobalPhase::Commit,
            subject: foreign_parent_subject,
            execution_commitment: foreign_parent_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let foreign_shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &foreign_preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let foreign_refs = foreign_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let foreign_parent_qc = wire::QuorumCertificate {
            round: alternate_round,
            proposal_round: alternate_round,
            phase: wire::GlobalPhase::Commit,
            subject: foreign_parent_subject,
            execution_commitment: foreign_parent_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&foreign_refs)
                .expect("aggregate foreign parent CommitQC"),
        };
        let mut retargeted_proposal = proposal.clone();
        let wire::ProposalJustification::ParentCommit(parent) =
            &mut retargeted_proposal.justification
        else {
            unreachable!("fixture carries a parent certificate")
        };
        parent.certificate = Some(foreign_parent_qc);
        retargeted_proposal.signature = Signature::new(
            keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
            &retargeted_proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        let retargeted_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(retargeted_proposal),
        );
        assert!(matches!(
            adapter.authenticate(retargeted_message.clone()),
            Err(AdapterError::WireValidation(
                wire::ValidationError::InvalidProposalJustification
            ))
        ));
        let registry_before_retargeting = adapter.registry.clone();
        // Exercise the staged conversion defense directly. Production ingress
        // reaches this only after `authenticate`, whose structural check above
        // already rejects the retargeting; the inner conversion must still be
        // fail-closed if a caller violates that private precondition.
        assert!(matches!(
            adapter.receive_verified(retargeted_message),
            Err(AdapterError::ParentContextMismatch)
        ));
        assert_registry_eq(&adapter.registry, &registry_before_retargeting);
        assert!(adapter.ingress_equivocations.is_empty());
        assert!(adapter.ingress_deliveries.is_empty());

        let admitted = adapter
            .receive_authenticated(authenticated)
            .expect("parent CommitQC remains bound to the predecessor during conversion");
        assert!(matches!(
            admitted.effects(),
            [AdapterEffect::FetchBody { manifest: Some(manifest), .. }]
                if manifest.round == proposal.round && manifest.subject == proposal.subject
        ));
        assert!(matches!(
            verify_authenticated_message(
                &successor,
                None,
                &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                    proposal.clone(),
                )),
                &proofs,
            ),
            Err(AdapterError::ParentContextMismatch)
        ));

        if let wire::ProposalJustification::ParentCommit(parent) = &mut proposal.justification {
            parent
                .certificate
                .as_mut()
                .expect("alternate parent certificate")
                .aggregate_signature[0] ^= 0x20;
        } else {
            unreachable!("fixture carries a parent certificate")
        }
        proposal.signature = Signature::new(
            keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
            &proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal),
            )),
            Err(AdapterError::Cryptography(_))
        ));

        successor
            .parent_commit_qc
            .as_mut()
            .expect("parent QC")
            .aggregate_signature[0] ^= 0x80;
        assert!(matches!(
            VerifiedHeightContext::successor(
                successor,
                proofs.clone(),
                &artifact,
                &receipt,
                &proofs,
            ),
            Err(AdapterError::Cryptography(_))
        ));

        let mut different_artifact = artifact.clone();
        different_artifact.commit_qc.aggregate_signature[0] ^= 0x40;
        let wrong_receipt = KuraV2CommitReceipt::for_test(&different_artifact);
        let mut successor = parent_context;
        successor.height = 2;
        successor.parent_commit_qc = Some(parent_qc);
        assert!(matches!(
            VerifiedHeightContext::successor(
                successor,
                proofs.clone(),
                &artifact,
                &wrong_receipt,
                &proofs,
            ),
            Err(AdapterError::ParentContextMismatch)
        ));
    }

    fn fingerprints() -> AdapterFingerprints {
        AdapterFingerprints {
            node: Hash::new(b"node"),
            build: Hash::new(b"build"),
            config: Hash::new(b"config"),
        }
    }

    fn subject(byte: u8) -> wire::BlockSubject {
        wire::BlockSubject {
            parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new([byte, 0]))),
            block_hash: HashOf::from_untyped_unchecked(Hash::new([byte, 1])),
            payload_hash: Hash::new([byte, 2]),
        }
    }

    fn execution_commitment(byte: u8) -> wire::ExecutionCommitment {
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new([byte, 3]),
            Hash::new([byte, 4]),
            Hash::new([byte, 5]),
            1,
            Hash::new([byte, 6]),
        )
    }

    #[test]
    fn commit_qc_status_reports_exact_frozen_signer_power() {
        let mut context = context();
        context.mode = wire::ConsensusMode::Npos;
        for (index, validator) in context.roster.iter_mut().enumerate() {
            validator.power = u64::try_from(index + 1).expect("fixture power fits u64");
        }
        context.quorum =
            wire::DualQuorum::from_roster(&context.roster).expect("weighted fixture quorum");
        let certificate = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            },
            proposal_round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            },
            phase: wire::GlobalPhase::Commit,
            subject: subject(0x31),
            execution_commitment: execution_commitment(0x31),
            signers: vec![0, 2, 3],
            aggregate_signature: vec![0xA5; 48],
        };

        let summary = commit_qc_status(&certificate, &context).expect("valid CommitQC summary");

        assert_eq!(summary.certificate, certificate.as_ref());
        assert_eq!(summary.validator_count, 4);
        assert_eq!(summary.signer_count, 3);
        assert_eq!(summary.min_signers, 3);
        assert_eq!(summary.signed_power, 8);
        assert_eq!(summary.total_power, 10);
    }

    #[test]
    fn vote_body_ownership_uses_the_authenticated_proposal_origin() {
        let context = context();
        let proposal_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 1,
        };
        let finality_round = wire::ConsensusRound {
            view: 3,
            ..proposal_round
        };
        let request = SignRequest::Vote(wire::Vote {
            round: finality_round,
            proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: subject(0x30),
            execution_commitment: execution_commitment(0x30),
            signer: 0,
            signature: Vec::new(),
        });

        assert_eq!(request.body_round(), Some(proposal_round));
    }

    #[test]
    fn locked_subject_reproposal_and_strict_higher_prepare_are_safe() {
        let context = context();
        let locked_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 1,
        };
        let locked_subject = subject(0x32);
        let exact_manifest = wire::PayloadManifest::derive(
            &context,
            locked_round,
            locked_subject,
            5,
            &[b"chunk".to_vec()],
        )
        .expect("exact locked manifest");
        let exact = wire::Proposal {
            round: locked_round,
            proposer: context.leader(locked_round.view),
            subject: locked_subject,
            manifest: exact_manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        assert!(proposal_is_safe_for_lock(
            &exact,
            locked_round,
            locked_subject
        ));

        let later_round = wire::ConsensusRound {
            view: locked_round.view + 1,
            ..locked_round
        };
        let later = wire::Proposal {
            round: later_round,
            proposer: context.leader(later_round.view),
            manifest: wire::PayloadManifest::derive(
                &context,
                later_round,
                locked_subject,
                5,
                &[b"chunk".to_vec()],
            )
            .expect("later same-subject manifest"),
            ..exact
        };
        assert!(proposal_is_safe_for_lock(
            &later,
            locked_round,
            locked_subject
        ));

        let prepared_subject = subject(0x33);
        let prepared_round = wire::ConsensusRound {
            view: locked_round.view + 1,
            ..locked_round
        };
        let proposal_round = wire::ConsensusRound {
            view: prepared_round.view + 1,
            ..prepared_round
        };
        let highest_prepare = wire::QuorumCertificate {
            round: prepared_round,
            proposal_round: prepared_round,
            phase: wire::GlobalPhase::Prepare,
            subject: prepared_subject,
            execution_commitment: execution_commitment(0x33),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x33; 48],
        };
        let prepared_proposal = wire::Proposal {
            round: proposal_round,
            proposer: context.leader(proposal_round.view),
            subject: prepared_subject,
            manifest: wire::PayloadManifest::derive(
                &context,
                proposal_round,
                prepared_subject,
                5,
                &[b"chunk".to_vec()],
            )
            .expect("prepared-subject manifest"),
            justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
                timeout_certificate: wire::TimeoutCertificate {
                    round: prepared_round,
                    groups: vec![wire::TimeoutVoteGroup {
                        highest_prepare_qc: Some(highest_prepare.clone()),
                        signers: vec![0, 1, 2],
                        aggregate_signature: vec![0x34; 48],
                    }],
                },
                highest_prepare_qc: Some(highest_prepare),
            }),
            signature: vec![0x35; 48],
        };
        assert!(proposal_is_safe_for_lock(
            &prepared_proposal,
            locked_round,
            locked_subject
        ));
        let mut registry = WireRegistry::new(&context).expect("wire registry");
        registry
            .justification_to_core(&prepared_proposal.justification, &context)
            .expect("matching strict-higher PrepareQC authorizes the proposal subject");

        let mut missing_repeated_high = prepared_proposal.clone();
        let wire::ProposalJustification::Timeout(timeout) =
            &mut missing_repeated_high.justification
        else {
            unreachable!("prepared fixture carries a timeout")
        };
        timeout.highest_prepare_qc = None;
        assert!(
            !proposal_is_safe_for_lock(&missing_repeated_high, locked_round, locked_subject),
            "safe-value admission must reject a TC-selected high omitted by the proposal"
        );
        let mut missing_registry = WireRegistry::new(&context).expect("wire registry");
        assert!(matches!(
            missing_registry.justification_to_core(&missing_repeated_high.justification, &context),
            Err(AdapterError::InvalidProposalJustification)
        ));
        assert!(
            missing_registry.subjects.is_empty()
                && missing_registry.execution_commitments.is_empty()
                && missing_registry.certificates.is_empty(),
            "the omitted repeated-QC gate must reject before registry mutation"
        );

        let mut invented_repeated_high = prepared_proposal.clone();
        let wire::ProposalJustification::Timeout(timeout) =
            &mut invented_repeated_high.justification
        else {
            unreachable!("prepared fixture carries a timeout")
        };
        timeout.timeout_certificate.groups[0].highest_prepare_qc = None;
        assert!(
            !proposal_is_safe_for_lock(&invented_repeated_high, locked_round, locked_subject),
            "safe-value admission must reject a repeated high absent from the TC"
        );
        let mut invented_registry = WireRegistry::new(&context).expect("wire registry");
        assert!(matches!(
            invented_registry
                .justification_to_core(&invented_repeated_high.justification, &context),
            Err(AdapterError::InvalidProposalJustification)
        ));
        assert!(
            invented_registry.subjects.is_empty()
                && invented_registry.execution_commitments.is_empty()
                && invented_registry.certificates.is_empty(),
            "the invented repeated-QC gate must reject before registry mutation"
        );

        let mut alternate_evidence = prepared_proposal.clone();
        let wire::ProposalJustification::Timeout(timeout) = &mut alternate_evidence.justification
        else {
            unreachable!("prepared fixture carries a timeout")
        };
        let tc_selected = timeout
            .timeout_certificate
            .highest_prepare_qc()
            .expect("prepared fixture TC carries a high QC")
            .clone();
        let repeated = timeout
            .highest_prepare_qc
            .as_mut()
            .expect("prepared fixture repeats the high QC");
        repeated.signers = vec![0, 1, 3];
        repeated.aggregate_signature = vec![0x36; 48];
        assert_eq!(repeated.as_ref(), tc_selected.as_ref());
        assert_ne!(repeated, &tc_selected);
        assert!(
            !proposal_is_safe_for_lock(&alternate_evidence, locked_round, locked_subject),
            "safe-value admission must reject same-reference alternate evidence"
        );
        let mut alternate_registry = WireRegistry::new(&context).expect("wire registry");
        assert!(matches!(
            alternate_registry.justification_to_core(&alternate_evidence.justification, &context),
            Err(AdapterError::InvalidProposalJustification)
        ));
        assert!(
            alternate_registry.subjects.is_empty()
                && alternate_registry.execution_commitments.is_empty()
                && alternate_registry.certificates.is_empty(),
            "the exact repeated-QC gate must reject before registry mutation"
        );

        let mut equal_rank = prepared_proposal.clone();
        let wire::ProposalJustification::Timeout(timeout) = &mut equal_rank.justification else {
            unreachable!("prepared fixture carries a timeout")
        };
        let selected = timeout
            .highest_prepare_qc
            .as_mut()
            .expect("prepared fixture carries a high QC");
        selected.round = locked_round;
        selected.proposal_round = locked_round;
        timeout.timeout_certificate.groups[0].highest_prepare_qc = Some(selected.clone());
        assert!(
            !proposal_is_safe_for_lock(&equal_rank, locked_round, locked_subject),
            "an equal-rank PrepareQC cannot release a different lock subject"
        );
    }

    fn proposal(
        context: &wire::HeightContext,
        proposer: wire::ValidatorIndex,
        subject: wire::BlockSubject,
    ) -> wire::ConsensusMessageV2 {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let manifest =
            wire::PayloadManifest::derive(context, round, subject, 5, &[b"chunk".to_vec()])
                .expect("valid fixture manifest");
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
            round,
            proposer,
            subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: vec![0x91],
        }))
    }

    fn authenticated_wire_identity(payload: wire::ConsensusMessageV2Payload) -> Arc<[u8]> {
        Arc::from(wire::ConsensusMessageV2::new(payload).encode())
    }

    fn durable_body_receipt(
        adapter: &SumeragiV2Adapter,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> DurableBodyReceipt {
        let manifest = adapter
            .registry
            .manifests
            .values()
            .find(|manifest| manifest.round == round && manifest.subject == subject)
            .expect("registered proposal manifest");
        DurableBodyReceipt::for_test(
            adapter.wire_context.id(),
            round,
            subject,
            HashOf::new(manifest),
        )
    }

    fn validated_receipts_for_manifest(
        context: &wire::HeightContext,
        manifest: &wire::PayloadManifest,
    ) -> (DurableBodyReceipt, ValidatedBodyReceipt) {
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        (durable, validated)
    }

    fn deferred_admission_ordinals() -> DeferredAdmissionOrdinalSource {
        DeferredAdmissionOrdinalSource::new(1)
    }

    struct ProcessOnlyProducerReplacement {
        address: ProducerContinuationAddress,
        incumbent: ProducerContinuationRecord,
        candidate: (ServicedCandidateKey, wire::View, ServicedCandidatePolicy),
        reservation: ProducerReservationToken,
    }

    fn reserve_process_only_producer_replacement(
        adapter: &mut SumeragiV2Adapter,
        marker: u8,
    ) -> ProcessOnlyProducerReplacement {
        let event = reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        };
        let candidate = adapter
            .serviced_candidate(&event, DeferredPriority::Completion, None, None)
            .expect("timeout has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"process-only predecessor"), 1)
            .expect("bind process-only predecessor");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("reserve process-only predecessor")
            .expect("tracked predecessor reserves");
        let handoff = adapter
            .record_serviced_candidate(Some(candidate), false, false, Some(reservation))
            .expect("drain process-only predecessor")
            .expect("drained predecessor retains its exact reservation");
        let address = handoff.address();
        adapter
            .acknowledge_producer_handoff(
                handoff,
                ProducerContinuationHandoffEvidence::VolatileTerminal,
            )
            .expect("terminalize process-only predecessor");
        let incumbent = adapter.producer_continuations[&address].clone();
        assert_eq!(incumbent.status(), ProducerContinuationStatus::Terminal);
        assert!(
            !adapter
                .durable_producer_continuations
                .contains_key(&address),
            "volatile predecessor must not have restart-stable state"
        );

        let replacement_key = ServicedCandidateKey::new(
            adapter.wire_context.id(),
            adapter.wire_context.height,
            adapter.fingerprints.node.into(),
            adapter.wire_context.leader(1),
            1,
            Some([marker; 32]),
            0,
            ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
            DeferredEventKind::TimeoutElapsed.code(),
            [marker; 32],
        );
        let candidate = (replacement_key, 1, ServicedCandidatePolicy::Suppress);
        adapter.clear_selected_producer_lifecycle();
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"newer replacement"), 2)
            .expect("bind newer replacement");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("replace process-only terminal")
            .expect("tracked replacement reserves");
        assert_eq!(reservation.address, address);
        let ProducerReservationChange::ReplacedTerminal {
            process_previous,
            durable_previous,
        } = &reservation.change
        else {
            panic!("newer lifecycle must replace the process-only terminal");
        };
        assert_eq!(process_previous, &incumbent);
        assert!(
            durable_previous.is_none(),
            "replacement must retain the absence of durable predecessor state"
        );

        ProcessOnlyProducerReplacement {
            address,
            incumbent,
            candidate,
            reservation,
        }
    }

    fn assert_process_only_predecessor_absent_after_restart(directory: &TempDir) {
        let (restarted, startup) = open_test(directory).expect("restart adapter");
        assert!(startup.is_empty());
        assert!(
            restarted.producer_continuations.is_empty()
                && restarted.durable_producer_continuations.is_empty()
                && restarted.restored_dormant_producer_continuations.is_empty(),
            "a process-only predecessor must not be synthesized during restart"
        );
    }

    fn open_test(
        directory: &TempDir,
    ) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), AdapterError> {
        SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(1),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
    }

    fn open_test_with_capacity_geometry(
        directory: &TempDir,
        capacity_geometry: ServicedCandidateCapacityGeometry,
    ) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), AdapterError> {
        SumeragiV2Adapter::open_with_aggregator_and_publication_with_capacity(
            directory.path().join("capacity-safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(1),
            [0x12; 32],
            fingerprints(),
            Box::new(TestAggregator),
            true,
            capacity_geometry,
            deferred_admission_ordinals(),
        )
    }

    fn assert_registry_eq(actual: &WireRegistry, expected: &WireRegistry) {
        assert_eq!(actual.wire_context, expected.wire_context);
        assert_eq!(actual.context_id, expected.context_id);
        assert_eq!(actual.peers, expected.peers);
        assert_eq!(actual.validators, expected.validators);
        assert_eq!(actual.subjects, expected.subjects);
        assert_eq!(actual.manifests, expected.manifests);
        assert_eq!(actual.execution_commitments, expected.execution_commitments);
        assert_eq!(actual.certificates, expected.certificates);
        assert_eq!(actual.proposals, expected.proposals);
    }

    fn open_test_as_leader(
        directory: &TempDir,
    ) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), AdapterError> {
        let context = context();
        let leader = context.leader(0);
        SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("leader-safety.wal"),
            verified_genesis(context),
            Some(leader),
            reducer::Generation::new(1),
            [0x22; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
    }

    fn unowned_body_event(adapter: &SumeragiV2Adapter, marker: u8) -> reducer::Event {
        reducer::Event::BodyAvailable {
            tag: adapter.current_tag(),
            round: reducer::Round::new(adapter.wire_context.height, adapter.current_tag().view()),
            subject: reducer::Subject::repeat(marker),
        }
    }

    fn durably_retire_unowned_body_event(adapter: &mut SumeragiV2Adapter, marker: u8) {
        let event = unowned_body_event(adapter, marker);
        assert!(
            adapter
                .enqueue_deferred(event, false, DeferredPriority::Completion, None, None, None,)
                .expect("retain the terminal candidate under exact deferred ownership")
                .is_some()
        );
        assert!(
            adapter
                .drain_deferred()
                .expect("durably retire the terminal candidate")
                .is_empty()
        );
    }

    #[test]
    fn direct_internal_discard_tombstones_a_b_a_and_survives_restart() {
        let directory = TempDir::new().expect("temporary directory");
        let a_marker = 0x31;
        let b_marker = 0x32;
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let initial = adapter.serviced_candidate_count_for_test();
            let a = unowned_body_event(&adapter, a_marker);
            let b = unowned_body_event(&adapter, b_marker);
            assert_ne!(a, b);
            assert_ne!(
                adapter
                    .step(a.clone())
                    .expect("service candidate A")
                    .disposition(),
                reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
            );
            assert_ne!(
                adapter
                    .step(b)
                    .expect("service equal-rank replacement B")
                    .disposition(),
                reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
            );
            assert_eq!(adapter.serviced_candidate_count_for_test(), initial + 2);
            assert_eq!(adapter.durable_serviced_candidates.len(), initial + 2);
            assert_eq!(
                adapter
                    .step(a)
                    .expect("coalesce resurrected candidate A")
                    .disposition(),
                reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
            );
            assert_eq!(adapter.serviced_candidate_count_for_test(), initial + 2);
        }

        let context = context();
        let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context.clone()),
            Some(0),
            reducer::Generation::new(2),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("reopen with exact direct-discard terminal records");
        assert!(startup.is_empty());
        let retained = restarted.serviced_candidate_count_for_test();
        assert_eq!(
            retained, 2,
            "direct and deferred internal NoMatchingWork discards are restart-stable"
        );
        let restarted_a = unowned_body_event(&restarted, a_marker);
        assert_eq!(
            restarted
                .step(restarted_a)
                .expect("coalesce A after process generation changes")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert_eq!(restarted.serviced_candidate_count_for_test(), retained);
    }

    #[test]
    fn nonquorum_vote_retransmission_rebuilds_volatile_pool_after_restart() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let vote =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: subject(0x35),
                execution_commitment: execution_commitment(0x35),
                signer: 1,
                signature: vec![0x35],
            }));
        let replacement =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: subject(0x35),
                execution_commitment: execution_commitment(0x35),
                signer: 2,
                signature: vec![0x36],
            }));
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            assert_eq!(
                adapter
                    .receive_authenticated(AuthenticatedConsensusMessage::for_test(vote.clone()))
                    .expect("admit one nonquorum Prepare vote")
                    .disposition(),
                reducer::StepDisposition::Applied
            );
            assert_eq!(adapter.serviced_candidate_count_for_test(), 1);
            assert!(
                adapter.durable_serviced_candidates.is_empty(),
                "a volatile quorum contribution is process-local, never a restart tombstone"
            );
            let first_key = IngressSemanticKey::Vote {
                round,
                phase: wire::GlobalPhase::Prepare,
                signer: 1,
            };
            adapter.ingress_deliveries.remove(&first_key);
            adapter.ingress_equivocations.remove(&first_key);
            assert_eq!(
                adapter
                    .receive_authenticated(AuthenticatedConsensusMessage::for_test(replacement,))
                    .expect("service equal-rank candidate B")
                    .disposition(),
                reducer::StepDisposition::Applied
            );
            assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
            let replacement_key = IngressSemanticKey::Vote {
                round,
                phase: wire::GlobalPhase::Prepare,
                signer: 2,
            };
            adapter.ingress_deliveries.remove(&replacement_key);
            adapter.ingress_equivocations.remove(&replacement_key);
            assert_eq!(
                adapter
                    .receive_authenticated(AuthenticatedConsensusMessage::for_test(vote.clone()))
                    .expect("coalesce candidate A after equal-rank replacement B")
                    .disposition(),
                reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate),
                "same-generation A -> B -> A service must not resurrect A"
            );
            assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
            assert!(
                adapter
                    .status()
                    .expect("one-vote status")
                    .liveness
                    .prepare_quorums
                    .iter()
                    .any(|quorum| quorum.round == round && quorum.signer_count == 2)
            );
        }

        let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context),
            Some(0),
            reducer::Generation::new(2),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("restart after losing the volatile vote pool");
        assert!(startup.is_empty());
        assert_eq!(restarted.serviced_candidate_count_for_test(), 0);
        assert!(
            restarted
                .status()
                .expect("empty post-restart pool")
                .liveness
                .prepare_quorums
                .is_empty()
        );
        assert_eq!(
            restarted
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(vote))
                .expect("retransmission reconstructs the lost vote owner")
                .disposition(),
            reducer::StepDisposition::Applied
        );
        assert!(
            restarted
                .status()
                .expect("rebuilt vote pool")
                .liveness
                .prepare_quorums
                .iter()
                .any(|quorum| quorum.round == round && quorum.signer_count == 1)
        );
    }

    #[test]
    fn deferred_discard_tombstones_before_owner_release_and_restart() {
        let directory = TempDir::new().expect("temporary directory");
        let marker = 0x33;
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let initial = adapter.serviced_candidate_count_for_test();
            let discarded = unowned_body_event(&adapter, marker);
            assert!(
                adapter
                    .enqueue_deferred(
                        discarded.clone(),
                        false,
                        DeferredPriority::Completion,
                        None,
                        None,
                        None,
                    )
                    .expect("retain the candidate under deferred ownership")
                    .is_some()
            );
            assert_eq!(adapter.deferred_completions.len(), 1);

            let effects = adapter
                .drain_deferred()
                .expect("service the nondispatchable candidate exactly once");
            assert!(effects.is_empty());
            assert!(adapter.deferred_completions.is_empty());
            assert_eq!(
                adapter.serviced_candidate_count_for_test(),
                initial + 1,
                "the terminal discard must be durable before the deferred owner is released"
            );
            assert_eq!(
                adapter
                    .step(discarded)
                    .expect("coalesce retransmission after deferred drain")
                    .disposition(),
                reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
            );
            assert_eq!(adapter.serviced_candidate_count_for_test(), initial + 1);
        }

        let context = context();
        let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context),
            Some(0),
            reducer::Generation::new(2),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("restore the terminal candidate tombstone");
        assert!(startup.is_empty());
        let retained = restarted.serviced_candidate_count_for_test();
        let retransmitted = unowned_body_event(&restarted, marker);
        assert_eq!(
            restarted
                .step(retransmitted)
                .expect("coalesce retransmission after same-height restart")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert_eq!(restarted.serviced_candidate_count_for_test(), retained);
    }

    #[test]
    fn serviced_candidate_write_failure_is_fail_closed_and_retains_deferred_owner() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        durably_retire_unowned_body_event(&mut adapter, 0x40);
        let event = unowned_body_event(&adapter, 0x41);
        assert!(
            adapter
                .enqueue_deferred(event, false, DeferredPriority::Completion, None, None, None,)
                .expect("retain candidate in deferred ownership")
                .is_some()
        );
        let path = adapter
            .serviced_candidate_store_path_for_test()
            .to_path_buf();
        std::fs::remove_file(&path).expect("remove published snapshot");
        std::fs::create_dir(&path).expect("replace snapshot target with a directory");
        let retained = adapter.deferred_completions.len();
        assert!(matches!(
            adapter.drain_deferred(),
            Err(AdapterError::ServicedCandidateStore(_))
        ));
        assert!(adapter.fail_closed);
        assert_eq!(
            adapter.deferred_completions.len(),
            retained,
            "failed publication retains the selected owner before fail-stop"
        );
    }

    #[test]
    fn restored_producer_reuses_runtime_key_and_ordinal_and_does_not_resurrect() {
        let directory = TempDir::new().expect("temporary directory");
        let causal_key;
        {
            let (adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let started_at = Instant::now();
            let lifecycle_ordinals =
                super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0);
            let (mut runtime, startup) =
                super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
                    adapter,
                    startup,
                    started_at,
                    Duration::from_secs(4),
                    super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
                    lifecycle_ordinals,
                )
                .expect("construct the original serialized runtime");
            assert!(startup.is_empty());
            runtime
                .arm_live_clocks(started_at)
                .expect("arm the original runtime");
            let owner = runtime
                .frozen_timeout_owner_for_test(started_at + Duration::from_secs(4))
                .expect("freeze the deterministic original timeout owner");
            causal_key = owner.causal_origin().lifecycle_key;
            assert_eq!(owner.lifecycle_ordinal(), 1);
            let mut adapter = runtime.into_driver();
            let event = reducer::Event::TimeoutElapsed {
                tag: adapter.current_tag(),
            };
            let candidate = adapter
                .serviced_candidate(&event, DeferredPriority::Completion, None, None)
                .expect("timeout has a producer stage");
            adapter
                .bind_selected_producer_lifecycle(causal_key, owner.lifecycle_ordinal())
                .expect("bind selected source");
            let reservation = adapter
                .reserve_selected_producer_continuation(Some(candidate))
                .expect("reserve before source retirement")
                .expect("tracked candidate reserves an address");
            let address = reservation.address;
            assert_eq!(
                adapter.producer_continuations[&address].status(),
                ProducerContinuationStatus::Reserved
            );
            assert_eq!(
                adapter.durable_producer_continuations.get(&address),
                adapter.producer_continuations.get(&address),
                "reservation is synchronized before its source can retire"
            );
        }

        let (restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(2),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("restart with exact active admission metadata");
        assert!(startup.is_empty());
        let restored = restarted
            .producer_continuations
            .values()
            .next()
            .expect("active producer metadata reopens");
        assert_eq!(restored.status(), ProducerContinuationStatus::Reserved);
        assert_eq!(restored.identity().admission_ordinal(), 1);
        let restored_address = restored.identity().address();
        assert_eq!(restored_address.lifecycle_slot(), 1);
        assert_eq!(
            restarted.restored_producer_continuation_ordinal_high_watermark(),
            Some(1)
        );
        assert!(
            restarted
                .restored_dormant_producer_continuations
                .contains(&restored_address)
        );
        assert!(
            restarted
                .dormant_local_fifo_reservations()
                .expect("validate restored timeout metadata")
                .is_empty(),
            "a restart-dormant timeout remains a non-FIFO clock root"
        );

        let lifecycle_ordinals =
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(1);
        let started_at = Instant::now();
        let (mut runtime, startup) =
            super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
                restarted,
                startup,
                started_at,
                Duration::from_secs(4),
                super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
                lifecycle_ordinals,
            )
            .expect("construct the restarted serialized runtime");
        assert!(startup.is_empty());
        assert_eq!(
            runtime.remaining_completion_capacity(),
            6,
            "the non-FIFO timeout root cannot consume completion capacity"
        );
        runtime
            .arm_live_clocks(started_at)
            .expect("arm the restarted runtime");
        let step = runtime
            .step(started_at + Duration::from_secs(4))
            .expect("replayed timeout reuses and crosses the exact runtime handoff");
        let super::super::v2_runtime::RuntimeStep::Advanced(effects) = step else {
            panic!("the exact replayed timeout must advance");
        };
        assert!(!effects.is_empty(), "timeout retains a concrete successor");
        let scheduler = runtime
            .take_last_scheduler_ownership()
            .expect("timeout publishes exact scheduler ownership");
        assert_eq!(
            scheduler.selected,
            super::super::v2_runtime::RuntimeSelectedOwnerKind::Timeout
        );
        let effect_ownership = runtime
            .take_effect_ownership(effects.len())
            .expect("take the concrete successor ownership");
        assert!(
            effect_ownership
                .iter()
                .all(|ownership| ownership.owner().lifecycle_ordinal() == 1),
            "every concrete successor retains the original owner 1"
        );
        let retained = runtime
            .driver()
            .producer_continuations
            .get(&restored_address)
            .expect("runtime acknowledgement retains its process-local terminal");
        assert_eq!(
            retained.identity().admission_ordinal(),
            1,
            "restart cannot replace the immutable first-admission ordinal"
        );
        assert_eq!(
            retained.identity().causal_lifecycle_key(),
            effect_ownership[0].owner().causal_origin().lifecycle_key
        );
        assert_eq!(
            retained.identity().causal_lifecycle_key(),
            causal_key,
            "the exact retry retains its persisted causal identity"
        );
        assert_eq!(retained.status(), ProducerContinuationStatus::Terminal);
        assert!(
            !runtime
                .driver()
                .durable_producer_continuations
                .contains_key(&restored_address),
            "a concrete volatile successor removes the dormant restart record"
        );
        assert!(
            !runtime
                .driver()
                .restored_dormant_producer_continuations
                .contains(&restored_address)
        );

        drop(runtime.into_driver());
        let (restarted_again, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(3),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("restart after the runtime handoff");
        assert!(
            matches!(
                startup.as_slice(),
                [AdapterEffect::Sign {
                    request: SignRequest::TimeoutVote(_),
                    ..
                }]
            ),
            "restart reconstructs the durable exact successor instead of the drained timeout stage"
        );
        assert!(
            restarted_again.producer_continuations.is_empty()
                && restarted_again.durable_producer_continuations.is_empty()
                && restarted_again
                    .restored_dormant_producer_continuations
                    .is_empty(),
            "the drained logical request cannot be recreated at its old stage"
        );
    }

    #[test]
    fn live_producer_owner_cannot_replace_immutable_identity() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let event = reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        };
        let candidate = adapter
            .serviced_candidate(&event, DeferredPriority::Completion, None, None)
            .expect("timeout has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"live producer owner"), 1)
            .expect("bind first live owner");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("reserve first live owner")
            .expect("tracked source reserves an address");
        let address = reservation.address;
        let original = adapter.producer_continuations[&address].clone();
        assert!(
            adapter.restored_dormant_producer_continuations.is_empty(),
            "same-process reservations are never restart-dormant"
        );

        adapter.clear_selected_producer_lifecycle();
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"forged equal-rank owner"), 1)
            .expect("bind a distinct equal-rank owner");
        assert!(matches!(
            adapter.reserve_selected_producer_continuation(Some(candidate)),
            Err(AdapterError::ServicedCandidateStore(_))
        ));
        assert_eq!(adapter.producer_continuations[&address], original);
        assert_eq!(
            adapter.durable_producer_continuations.get(&address),
            Some(&original),
            "rejected live replacement changes no durable alias"
        );
    }

    #[test]
    fn restored_producer_rejects_a_mismatched_replay_identity_without_mutation() {
        let directory = TempDir::new().expect("temporary directory");
        let candidate;
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let event = reducer::Event::TimeoutElapsed {
                tag: adapter.current_tag(),
            };
            candidate = adapter
                .serviced_candidate(&event, DeferredPriority::Completion, None, None)
                .expect("timeout has a producer stage");
            adapter
                .bind_selected_producer_lifecycle(Hash::new(b"stored producer owner"), 1)
                .expect("bind stored owner");
            adapter
                .reserve_selected_producer_continuation(Some(candidate))
                .expect("persist stored owner")
                .expect("tracked source reserves an address");
        }

        let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(2),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("restore dormant producer");
        assert!(startup.is_empty());
        let (address, original) = restarted
            .producer_continuations
            .iter()
            .next()
            .map(|(address, record)| (*address, record.clone()))
            .expect("restored producer exists");
        restarted
            .bind_selected_producer_lifecycle(Hash::new(b"replayed producer owner"), 2)
            .expect("bind replay owner");

        assert!(matches!(
            restarted.reserve_selected_producer_continuation(Some(candidate)),
            Err(AdapterError::ServicedCandidateStore(_))
        ));
        assert_eq!(restarted.producer_continuations[&address], original);
        assert_eq!(
            restarted.durable_producer_continuations.get(&address),
            Some(&original)
        );
        assert!(
            restarted
                .restored_dormant_producer_continuations
                .contains(&address),
            "a rejected identity replacement cannot claim the dormant alias"
        );
    }

    #[test]
    fn conditional_transport_service_reserves_and_coalesces_a_producer_lifecycle() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let leader = adapter.wire_context.leader(adapter.current_tag().view());
        let message = proposal(&adapter.wire_context, leader, subject(0x6D));
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"conditional transport source"), 17)
            .expect("bind exact transport owner");
        let outcome = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(message))
            .expect("service authenticated proposal");
        let handoff = outcome
            .producer_handoff()
            .expect("transport service retains a producer handoff");
        assert_eq!(
            handoff.source_class(),
            ProducerContinuationSourceClass::ConditionalTransport
        );
        assert_eq!(handoff.identity().admission_ordinal(), 17);
        let address = handoff.identity().address();
        assert_eq!(
            adapter.producer_continuations[&address].status(),
            ProducerContinuationStatus::Reserved
        );
        adapter
            .acknowledge_producer_handoff(
                handoff,
                ProducerContinuationHandoffEvidence::ConcreteSuccessor,
            )
            .expect("physical runtime successor acknowledges transport service");
        assert_eq!(
            adapter.producer_continuations[&address].status(),
            ProducerContinuationStatus::Terminal
        );
        assert!(
            !adapter
                .durable_producer_continuations
                .contains_key(&address),
            "volatile transport completion cannot become a restart-stable terminal"
        );
    }

    #[test]
    fn retired_empty_handoff_terminalizes_once_and_exact_replay_coalesces() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let event = reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        };
        let candidate = adapter
            .serviced_candidate(&event, DeferredPriority::Completion, None, None)
            .expect("timeout has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"retired empty handoff"), 18)
            .expect("bind exact local owner");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("reserve before source retirement")
            .expect("tracked source reserves an address");
        let handoff = adapter
            .record_serviced_candidate(Some(candidate), false, false, Some(reservation))
            .expect("drain source without a concrete successor")
            .expect("drained source retains its exact reservation");
        let address = handoff.identity().address();
        assert_eq!(
            adapter
                .producer_handoff_evidence(handoff, false)
                .expect("classify empty handoff"),
            ProducerContinuationHandoffEvidence::VolatileTerminal
        );
        let terminal = adapter
            .acknowledge_producer_handoff(
                handoff,
                ProducerContinuationHandoffEvidence::VolatileTerminal,
            )
            .expect("retired empty handoff terminalizes");
        assert_eq!(terminal.identity(), handoff.identity());
        assert_eq!(
            adapter.producer_continuations[&address].status(),
            ProducerContinuationStatus::Terminal
        );
        assert_eq!(adapter.producer_continuations.len(), 1);
        assert!(
            !adapter
                .durable_producer_continuations
                .contains_key(&address),
            "process-local retirement must not be upgraded to restart-stable evidence"
        );

        let replay = adapter
            .step(event)
            .expect("coalesce exact retransmission after drain");
        assert_eq!(
            replay.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert!(
            replay.producer_handoff().is_none(),
            "the drained identity cannot mint a second producer lifecycle"
        );
        assert_eq!(adapter.producer_continuations.len(), 1);
        assert_eq!(
            adapter.producer_continuations[&address].status(),
            ProducerContinuationStatus::Terminal,
            "exact replay cannot resurrect the retired old stage"
        );
    }

    #[test]
    fn every_producer_stage_has_an_explicit_replay_parent_contract() {
        let classified = ServicedCandidateStage::ALL
            .map(|stage| (stage, producer_parent_replay_source_for_stage(stage)));
        assert_eq!(classified.len(), SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE);
        assert_eq!(
            classified,
            [
                (
                    ServicedCandidateStage::LocalProposalReady,
                    ProducerParentReplaySource::DurableBodyPipeline,
                ),
                (
                    ServicedCandidateStage::ProposalReceived,
                    ProducerParentReplaySource::ConditionalResponsiveTransport,
                ),
                (
                    ServicedCandidateStage::VoteReceived,
                    ProducerParentReplaySource::ConditionalResponsiveTransport,
                ),
                (
                    ServicedCandidateStage::QuorumCertificateReceived,
                    ProducerParentReplaySource::ConditionalResponsiveTransport,
                ),
                (
                    ServicedCandidateStage::TimeoutVoteReceived,
                    ProducerParentReplaySource::ConditionalResponsiveTransport,
                ),
                (
                    ServicedCandidateStage::TimeoutCertificateReceived,
                    ProducerParentReplaySource::ConditionalResponsiveTransport,
                ),
                (
                    ServicedCandidateStage::TimeoutElapsed,
                    ProducerParentReplaySource::SafetyWal,
                ),
                (
                    ServicedCandidateStage::BodyAvailable,
                    ProducerParentReplaySource::VolatileBodyReconstruction,
                ),
                (
                    ServicedCandidateStage::BodyStored,
                    ProducerParentReplaySource::DurableBodyPipeline,
                ),
                (
                    ServicedCandidateStage::ValidationCompleted,
                    ProducerParentReplaySource::DurableBodyPipeline,
                ),
                (
                    ServicedCandidateStage::ApplicationCompleted,
                    ProducerParentReplaySource::DurableDecision,
                ),
            ]
        );
        for stage in ServicedCandidateStage::ALL {
            let expected = matches!(
                stage,
                ServicedCandidateStage::LocalProposalReady
                    | ServicedCandidateStage::TimeoutElapsed
                    | ServicedCandidateStage::BodyStored
                    | ServicedCandidateStage::ValidationCompleted
                    | ServicedCandidateStage::ApplicationCompleted
            );
            assert_eq!(
                producer_parent_is_locally_reconstructible(stage),
                expected,
                "only an independently durable local parent may reserve"
            );
        }
    }

    #[test]
    fn speculative_producer_rollback_restores_free_and_terminal_slots() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let event = reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        };
        let first = adapter
            .serviced_candidate(&event, DeferredPriority::Completion, None, None)
            .expect("timeout has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"first source"), 1)
            .expect("bind first source");
        let inserted = adapter
            .reserve_selected_producer_continuation(Some(first))
            .expect("reserve free slot")
            .expect("tracked source reserves");
        let address = inserted.address;
        assert_eq!(inserted.change, ProducerReservationChange::Inserted);
        let original = adapter.producer_continuations[&address].clone();
        adapter.clear_selected_producer_lifecycle();
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"first source"), 1)
            .expect("bind the exact physical retry");
        let coalesced = adapter
            .reserve_selected_producer_continuation(Some(first))
            .expect("coalesce the same logical request")
            .expect("tracked retry retains its original address");
        assert_eq!(coalesced.address, address);
        assert_eq!(coalesced.change, ProducerReservationChange::Unchanged);
        assert_eq!(adapter.producer_continuations[&address], original);
        adapter
            .rollback_producer_reservation(Some(coalesced))
            .expect("roll back coalesced reservation");
        adapter
            .rollback_producer_reservation(Some(inserted))
            .expect("roll back inserted reservation");
        assert!(!adapter.producer_continuations.contains_key(&address));

        adapter.clear_selected_producer_lifecycle();
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"first source"), 1)
            .expect("rebind first source");
        let inserted = adapter
            .reserve_selected_producer_continuation(Some(first))
            .expect("reserve first owner")
            .expect("tracked source reserves");
        adapter
            .terminalize_producer_continuation(Some(inserted.address))
            .expect("terminalize incumbent");
        let terminal = adapter.producer_continuations[&inserted.address].clone();

        let replacement_key = ServicedCandidateKey::new(
            adapter.wire_context.id(),
            adapter.wire_context.height,
            adapter.fingerprints.node.into(),
            adapter.wire_context.leader(1),
            1,
            Some([0x47; 32]),
            0,
            ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
            DeferredEventKind::TimeoutElapsed.code(),
            [0x47; 32],
        );
        let replacement = (replacement_key, 1, ServicedCandidatePolicy::Suppress);
        adapter.clear_selected_producer_lifecycle();
        adapter
            .bind_selected_producer_lifecycle(Hash::new(b"replacement source"), 2)
            .expect("bind replacement source");
        let replaced = adapter
            .reserve_selected_producer_continuation(Some(replacement))
            .expect("replace terminal slot")
            .expect("tracked replacement reserves");
        assert!(matches!(
            replaced.change,
            ProducerReservationChange::ReplacedTerminal { .. }
        ));
        adapter
            .rollback_producer_reservation(Some(replaced))
            .expect("roll back terminal replacement");
        assert_eq!(adapter.producer_continuations[&address], terminal);
    }

    #[test]
    fn process_only_producer_replacement_rollback_stays_volatile_across_restart() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let replacement = reserve_process_only_producer_replacement(&mut adapter, 0x48);

        adapter
            .rollback_producer_reservation(Some(replacement.reservation))
            .expect("roll back process-only terminal replacement");
        assert_eq!(
            adapter.producer_continuations[&replacement.address],
            replacement.incumbent
        );
        assert!(
            !adapter
                .durable_producer_continuations
                .contains_key(&replacement.address),
            "rollback cannot publish the process-only predecessor"
        );

        drop(adapter);
        assert_process_only_predecessor_absent_after_restart(&directory);
    }

    #[test]
    fn process_only_producer_replacement_release_stays_volatile_across_restart() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let replacement = reserve_process_only_producer_replacement(&mut adapter, 0x49);

        adapter
            .release_unrecorded_producer(Some(replacement.reservation))
            .expect("release process-only terminal replacement");
        assert_eq!(
            adapter.producer_continuations[&replacement.address],
            replacement.incumbent
        );
        assert!(
            !adapter
                .durable_producer_continuations
                .contains_key(&replacement.address),
            "release cannot publish the process-only predecessor"
        );

        drop(adapter);
        assert_process_only_predecessor_absent_after_restart(&directory);
    }

    #[test]
    fn process_only_producer_replacement_handoff_does_not_resurrect_predecessor() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let replacement = reserve_process_only_producer_replacement(&mut adapter, 0x4A);

        let handoff = adapter
            .record_serviced_candidate(
                Some(replacement.candidate),
                false,
                false,
                Some(replacement.reservation),
            )
            .expect("stage volatile replacement handoff")
            .expect("replacement retains an exact handoff");
        adapter
            .acknowledge_producer_handoff(
                handoff,
                ProducerContinuationHandoffEvidence::VolatileTerminal,
            )
            .expect("acknowledge volatile replacement handoff");
        assert_eq!(
            adapter.producer_continuations[&replacement.address].status(),
            ProducerContinuationStatus::Terminal
        );
        assert!(
            !adapter
                .durable_producer_continuations
                .contains_key(&replacement.address),
            "non-durable acknowledgement cannot resurrect the process-only predecessor"
        );

        drop(adapter);
        assert_process_only_predecessor_absent_after_restart(&directory);
    }

    #[test]
    fn retiring_busy_local_parent_releases_unacknowledged_producer_owner() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: tag.view(),
        };
        let subject = subject(0x4A);
        let manifest = wire::PayloadManifest::derive(
            &adapter.wire_context,
            round,
            subject,
            5,
            &[b"producer-retirement-body".to_vec()],
        )
        .expect("derive body manifest");
        adapter
            .defer_body_pipeline_stage_for_test(
                tag,
                &manifest,
                DeferredBodyPipelineStageForTest::BodyStored,
            )
            .expect("stage exact local parent");
        let (admission_ordinal, candidate) = {
            let input = adapter
                .deferred_completions
                .back()
                .expect("deferred local parent");
            (
                input.admission_ordinal,
                adapter
                    .serviced_candidate(
                        &input.event,
                        input.priority,
                        input.completion_evidence.as_ref(),
                        input.authenticated_wire_identity.as_deref(),
                    )
                    .expect("body-store completion has a serviced identity"),
            )
        };
        adapter
            .bind_selected_producer_lifecycle(
                Hash::new(b"retired busy local parent"),
                admission_ordinal,
            )
            .expect("bind exact lifecycle");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("reserve before adapter ownership")
            .expect("local durable parent reserves");
        let address = reservation.address;
        adapter
            .deferred_producer_continuations
            .insert(admission_ordinal, reservation);

        adapter.retire_deferred_body_pipeline_completions(tag, round, subject);

        assert!(adapter.deferred_completions.is_empty());
        assert!(
            !adapter
                .deferred_producer_continuations
                .contains_key(&admission_ordinal)
        );
        assert!(
            !adapter.producer_continuations.contains_key(&address),
            "goal-reaching retirement cannot manufacture successor acknowledgement"
        );
    }

    #[test]
    fn terminal_producer_tombstone_survives_restart_blocks_aba_and_advances_shared_source() {
        let directory = TempDir::new().expect("temporary directory");
        let causal_key = Hash::new(b"terminal producer parent");
        let address;
        let terminal;
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let event = reducer::Event::TimeoutElapsed {
                tag: adapter.current_tag(),
            };
            let candidate = adapter
                .serviced_candidate(&event, DeferredPriority::Completion, None, None)
                .expect("timeout has a producer stage");
            adapter
                .bind_selected_producer_lifecycle(causal_key.clone(), 41)
                .expect("bind selected source");
            address = adapter
                .reserve_selected_producer_continuation(Some(candidate))
                .expect("reserve producer")
                .expect("tracked candidate reserves an address")
                .address;
            adapter
                .terminalize_producer_continuation(Some(address))
                .expect("terminalize after source retirement");
            terminal = adapter.producer_continuations[&address].clone();
            adapter
                .durable_producer_continuations
                .insert(address, terminal.clone());
            let terminal_candidate = terminal.identity().candidate();
            adapter
                .durable_serviced_candidates
                .insert(terminal_candidate, terminal_candidate.source_view());
            adapter
                .serviced_candidate_store
                .persist_with_producer_continuations(
                    &adapter.durable_serviced_candidates,
                    &adapter.durable_producer_continuations,
                    adapter.serviced_candidates_decision_reclaimed,
                )
                .expect("publish terminal high-watermark");
        }

        let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(2),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("restore terminal producer high-watermark");
        assert!(startup.is_empty());
        assert_eq!(restarted.producer_continuations[&address], terminal);
        let restored_high_watermark = restarted
            .restored_producer_continuation_ordinal_high_watermark()
            .expect("restored producer tombstone carries an ordinal");
        assert_eq!(restored_high_watermark, 41);
        let serve_high_watermark = 7;
        assert!(restored_high_watermark > serve_high_watermark);
        let lifecycle_ordinals =
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
                serve_high_watermark,
            );
        lifecycle_ordinals
            .advance_past(restored_high_watermark)
            .expect("fold producer high-watermark into actor source");
        let first_runtime_owner = lifecycle_ordinals
            .reserve_one()
            .expect("mint first post-restart runtime owner");
        let first_serve_owner = lifecycle_ordinals
            .reserve_one()
            .expect("mint first post-restart Serve owner");
        assert_eq!(first_runtime_owner, restored_high_watermark + 1);
        assert!(first_serve_owner > first_runtime_owner);
        assert!(
            restarted
                .serviced_candidate_store
                .reserve_producer_continuation(
                    &mut restarted.producer_continuations,
                    ProducerContinuationRecord::new(
                        terminal.identity(),
                        ProducerContinuationStatus::Reserved,
                        Vec::new(),
                    )
                    .expect("construct stale ABA retry"),
                )
                .is_err(),
            "a drained logical stage cannot resurrect through its old identity"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn serviced_candidate_reclaim_failure_fail_stops_then_replay_reclaims() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let snapshot_path;
        let stale_snapshot;
        let marker = 0x42;
        {
            let (mut adapter, startup) =
                open_test_as_leader(&directory).expect("open leader adapter");
            assert!(startup.is_empty());
            durably_retire_unowned_body_event(&mut adapter, marker);
            let pre_decision_timeout = reducer::Event::TimeoutElapsed {
                tag: adapter.current_tag(),
            };
            let pre_decision_producer = adapter
                .serviced_candidate(
                    &pre_decision_timeout,
                    DeferredPriority::Completion,
                    None,
                    None,
                )
                .expect("timeout has a producer identity");
            adapter
                .bind_selected_producer_lifecycle(Hash::new(b"pre-Decision producer tombstone"), 1)
                .expect("bind pre-Decision producer");
            let pre_decision_reservation = adapter
                .reserve_selected_producer_continuation(Some(pre_decision_producer))
                .expect("reserve pre-Decision producer")
                .expect("tracked timeout reserves");
            let handoff = adapter
                .record_serviced_candidate(
                    Some(pre_decision_producer),
                    true,
                    true,
                    Some(pre_decision_reservation),
                )
                .expect("stage paired pre-Decision producer tombstone")
                .expect("pre-Decision producer has a runtime handoff");
            adapter
                .acknowledge_producer_handoff(
                    handoff,
                    ProducerContinuationHandoffEvidence::DurableTerminal,
                )
                .expect("publish paired pre-Decision producer tombstone");
            adapter.clear_selected_producer_lifecycle();
            assert!(!adapter.producer_continuations.is_empty());
            assert!(!adapter.durable_producer_continuations.is_empty());
            assert!(adapter.serviced_candidate_count_for_test() > 0);
            snapshot_path = adapter
                .serviced_candidate_store_path_for_test()
                .to_path_buf();
            stale_snapshot =
                std::fs::read(&snapshot_path).expect("retain the pre-Decision snapshot");

            let decided_subject = subject(0x43);
            let leader = adapter.wire_context.leader(0);
            let proposal = proposal(&adapter.wire_context, leader, decided_subject);
            let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
                unreachable!("proposal helper returns a proposal")
            };
            let manifest = proposal.manifest;
            let (_, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
            let decision = wire::QuorumCertificate {
                round: manifest.round,
                proposal_round: manifest.round,
                phase: wire::GlobalPhase::Commit,
                subject: decided_subject,
                execution_commitment: validated.execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x43; 96],
            };

            std::fs::remove_file(&snapshot_path).expect("remove the published snapshot");
            std::fs::create_dir(&snapshot_path)
                .expect("replace the reclaim target with a directory");
            let wal_records_before = adapter.wal.recovered_records().len();
            assert!(matches!(
                adapter.receive_authenticated(AuthenticatedConsensusMessage::for_test(
                    wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::QuorumCertificate(decision),
                    ),
                )),
                Err(AdapterError::ServicedCandidateStore(_))
            ));
            assert!(adapter.fail_closed);
            assert!(
                adapter.wal.recovered_records().len() > wal_records_before,
                "the safety WAL advances before adjacent tombstone reclamation"
            );
            assert!(
                adapter.reducer.durable_state().decision().is_some(),
                "the failed adjacent snapshot publication cannot roll back the durable Decision"
            );
        }

        std::fs::remove_dir(&snapshot_path).expect("remove the injected reclaim obstacle");
        std::fs::write(&snapshot_path, &stale_snapshot)
            .expect("restore the last durable pre-Decision snapshot");
        let leader = context.leader(0);
        let (restarted, _startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("leader-safety.wal"),
            verified_genesis(context.clone()),
            Some(leader),
            reducer::Generation::new(2),
            [0x22; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("replay the durable Decision and reclaim the stale snapshot");
        assert!(restarted.reducer.durable_state().decision().is_some());
        assert!(restarted.serviced_candidates_decision_reclaimed);
        assert_eq!(restarted.serviced_candidate_count_for_test(), 0);
        assert!(restarted.producer_continuations.is_empty());
        assert!(restarted.durable_producer_continuations.is_empty());
        assert_ne!(
            std::fs::read(&snapshot_path).expect("read replay-reclaimed snapshot"),
            stale_snapshot,
            "replay must durably replace the stale pre-Decision snapshot"
        );
        drop(restarted);

        let (mut replayed_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("leader-safety.wal"),
            verified_genesis(context),
            Some(leader),
            reducer::Generation::new(3),
            [0x22; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("restore the replay-reclaimed snapshot on a second restart");
        assert!(replayed_again.reducer.durable_state().decision().is_some());
        assert!(replayed_again.serviced_candidates_decision_reclaimed);
        assert_eq!(replayed_again.serviced_candidate_count_for_test(), 0);
        assert!(replayed_again.producer_continuations.is_empty());
        assert!(replayed_again.durable_producer_continuations.is_empty());
        let decision_subject = replayed_again
            .reducer
            .durable_state()
            .decision()
            .expect("replayed Decision exists")
            .subject();
        let completion = reducer::Event::ApplicationCompleted {
            tag: replayed_again.current_tag(),
            subject: decision_subject,
        };
        let completion_candidate = replayed_again
            .serviced_candidate(&completion, DeferredPriority::Completion, None, None)
            .expect("application completion has a service identity");
        replayed_again
            .bind_selected_producer_lifecycle(Hash::new(b"post-Decision completion"), 1)
            .expect("bind post-Decision completion");
        let completion_reservation = replayed_again
            .reserve_selected_producer_continuation(Some(completion_candidate))
            .expect("reserve post-Decision completion")
            .expect("tracked completion reserves");
        let completion_handoff = replayed_again
            .record_serviced_candidate(
                Some(completion_candidate),
                true,
                true,
                Some(completion_reservation.clone()),
            )
            .expect("Decision retires process-local completion");
        assert!(completion_handoff.is_none());
        assert!(
            !replayed_again
                .durable_producer_continuations
                .contains_key(&completion_reservation.address),
            "Decision early return cannot publish an orphan producer terminal"
        );
        assert!(
            !replayed_again
                .producer_continuations
                .contains_key(&completion_reservation.address),
            "Decision early return cannot retain a process-local producer ghost"
        );
        replayed_again.clear_selected_producer_lifecycle();
        let post_replay = unowned_body_event(&replayed_again, marker);
        replayed_again
            .step(post_replay)
            .expect("post-Decision candidate handling remains fail-safe");
        assert_eq!(
            replayed_again.serviced_candidate_count_for_test(),
            0,
            "replay reclamation prevents the old candidate epoch from resurrecting"
        );
    }

    #[test]
    fn serviced_candidate_snapshot_is_bound_to_the_local_validator_owner() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let owner_a_wal = directory.path().join("owner-a.wal");
        let owner_a_snapshot;
        {
            let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
                &owner_a_wal,
                verified_genesis(context.clone()),
                Some(0),
                reducer::Generation::new(1),
                [0xA1; 32],
                fingerprints(),
                Box::new(TestAggregator),
                deferred_admission_ordinals(),
            )
            .expect("open owner-A adapter");
            assert!(startup.is_empty());
            durably_retire_unowned_body_event(&mut adapter, 0xA1);
            owner_a_snapshot = adapter
                .serviced_candidate_store_path_for_test()
                .to_path_buf();
        }

        let owner_b_wal = directory.path().join("owner-b.wal");
        let owner_b_snapshot = directory.path().join("owner-b.wal.serviced-candidates");
        std::fs::copy(&owner_a_snapshot, &owner_b_snapshot)
            .expect("transplant owner-A sidecar onto owner-B path");
        let mut owner_b_fingerprints = fingerprints();
        owner_b_fingerprints.node = Hash::new(b"owner-b node");
        assert!(matches!(
            SumeragiV2Adapter::open_with_aggregator(
                owner_b_wal,
                verified_genesis(context),
                Some(1),
                reducer::Generation::new(1),
                [0xB2; 32],
                owner_b_fingerprints,
                Box::new(TestAggregator),
                deferred_admission_ordinals(),
            ),
            Err(AdapterError::ServicedCandidateStore(_))
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn aggregate_carrier_and_priority_variants_coalesce_to_one_semantic_candidate() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let signer_subsets = [
            vec![0, 1, 2],
            vec![0, 1, 3],
            vec![0, 2, 3],
            vec![1, 2, 3],
            vec![0, 1, 2, 3],
        ];
        let marker_count = adapter.serviced_candidate_count_for_test();
        let mut qc_key = None;
        for (variant, signers) in signer_subsets.iter().enumerate() {
            let marker = u8::try_from(variant).expect("small carrier variant");
            let certificate = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: subject(0xC1),
                execution_commitment: execution_commitment(0xC1),
                signers: signers.clone(),
                aggregate_signature: vec![0xC0 | marker; 96],
            };
            let carrier = wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone()),
            )
            .encode();
            let certificate = adapter
                .registry
                .qc_to_core(&certificate, &adapter.wire_context)
                .expect("convert valid same-reference QC carrier");
            let candidate = adapter
                .serviced_candidate(
                    &reducer::Event::QuorumCertificateReceived { tag, certificate },
                    if variant % 2 == 0 {
                        DeferredPriority::Normal
                    } else {
                        DeferredPriority::Progress
                    },
                    None,
                    Some(&carrier),
                )
                .expect("QC has a service identity");
            assert_eq!(
                candidate.0.class(),
                ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
                "scheduler priority is excluded from the logical key"
            );
            match qc_key {
                Some(expected) => assert_eq!(
                    candidate.0, expected,
                    "valid quorum subset and aggregate replacement is not a new QC owner"
                ),
                None => qc_key = Some(candidate.0),
            }
            adapter
                .record_serviced_candidate(Some(candidate), false, false, None)
                .expect("coalesce QC carrier variant");
        }
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            marker_count + 1,
            "all valid QC carrier variants share one transient identity"
        );

        let mut tc_key = None;
        for (variant, signers) in signer_subsets.iter().enumerate() {
            let marker = u8::try_from(variant).expect("small carrier variant");
            let certificate = wire::TimeoutCertificate {
                round,
                groups: vec![wire::TimeoutVoteGroup {
                    highest_prepare_qc: None,
                    signers: signers.clone(),
                    aggregate_signature: vec![0xD0 | marker; 96],
                }],
            };
            let carrier = wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate.clone()),
            )
            .encode();
            let certificate = adapter
                .registry
                .tc_to_core(&certificate, &adapter.wire_context)
                .expect("convert valid same-reference TC carrier");
            let candidate = adapter
                .serviced_candidate(
                    &reducer::Event::TimeoutCertificateReceived { tag, certificate },
                    if variant % 2 == 0 {
                        DeferredPriority::Normal
                    } else {
                        DeferredPriority::Progress
                    },
                    None,
                    Some(&carrier),
                )
                .expect("TC has a service identity");
            match tc_key {
                Some(expected) => assert_eq!(
                    candidate.0, expected,
                    "valid timeout quorum subset and aggregate replacement is not a new owner"
                ),
                None => tc_key = Some(candidate.0),
            }
            adapter
                .record_serviced_candidate(Some(candidate), false, false, None)
                .expect("coalesce TC carrier variant");
        }
        assert_ne!(qc_key, tc_key);
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            marker_count + 2
        );

        let mut timeout_vote_key = None;
        for (variant, signers) in signer_subsets.iter().enumerate() {
            let marker = u8::try_from(variant).expect("small carrier variant");
            let highest_prepare = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: subject(0xC2),
                execution_commitment: execution_commitment(0xC2),
                signers: signers.clone(),
                aggregate_signature: vec![0xE0 | marker; 96],
            };
            let vote = wire::TimeoutVote {
                round,
                highest_prepare_qc: Some(highest_prepare),
                signer: 0,
                signature: vec![0x70 | marker; 96],
            };
            let carrier = wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(vote.clone()),
            )
            .encode();
            let vote = adapter
                .registry
                .timeout_vote_to_core(&vote, &adapter.wire_context)
                .expect("convert TimeoutVote with alternate high-QC carrier");
            let candidate = adapter
                .serviced_candidate(
                    &reducer::Event::TimeoutVoteReceived { tag, vote },
                    if variant % 2 == 0 {
                        DeferredPriority::Normal
                    } else {
                        DeferredPriority::Progress
                    },
                    None,
                    Some(&carrier),
                )
                .expect("TimeoutVote has a service identity");
            match timeout_vote_key {
                Some(expected) => assert_eq!(
                    candidate.0, expected,
                    "nested high-QC signer and signature variants are one TimeoutVote owner"
                ),
                None => timeout_vote_key = Some(candidate.0),
            }
            adapter
                .record_serviced_candidate(Some(candidate), false, false, None)
                .expect("coalesce nested TimeoutVote carrier variant");
        }
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            marker_count + 3
        );

        let proposal_round = wire::ConsensusRound { view: 1, ..round };
        let proposal_subject = subject(0xC3);
        let manifest = wire::PayloadManifest::derive(
            &adapter.wire_context,
            proposal_round,
            proposal_subject,
            5,
            &[b"chunk".to_vec()],
        )
        .expect("derive proposal manifest");
        let mut proposal_key = None;
        for (variant, signers) in signer_subsets.iter().enumerate() {
            let marker = u8::try_from(variant).expect("small carrier variant");
            let certificate = wire::TimeoutCertificate {
                round,
                groups: vec![wire::TimeoutVoteGroup {
                    highest_prepare_qc: None,
                    signers: signers.clone(),
                    aggregate_signature: vec![0x50 | marker; 96],
                }],
            };
            let proposal = wire::Proposal {
                round: proposal_round,
                proposer: adapter.wire_context.leader(proposal_round.view),
                subject: proposal_subject,
                manifest: manifest.clone(),
                justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
                    timeout_certificate: certificate,
                    highest_prepare_qc: None,
                }),
                signature: vec![0x60 | marker; 96],
            };
            let carrier = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            ))
            .encode();
            let proposal = adapter
                .registry
                .proposal_to_core(&proposal, &adapter.wire_context)
                .expect("convert proposal with alternate TC carrier");
            let candidate = adapter
                .serviced_candidate(
                    &reducer::Event::ProposalReceived { tag, proposal },
                    if variant % 2 == 0 {
                        DeferredPriority::Normal
                    } else {
                        DeferredPriority::Progress
                    },
                    None,
                    Some(&carrier),
                )
                .expect("proposal has a service identity");
            match proposal_key {
                Some(expected) => assert_eq!(
                    candidate.0, expected,
                    "nested TC and proposal-signature variants are one proposal owner"
                ),
                None => proposal_key = Some(candidate.0),
            }
            adapter
                .record_serviced_candidate(Some(candidate), false, false, None)
                .expect("coalesce nested proposal carrier variant");
        }
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            marker_count + 4
        );

        let mut vote_key = None;
        for variant in 0_u8..5 {
            let vote = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: subject(0xC4),
                execution_commitment: execution_commitment(0xC4),
                signer: 1,
                signature: vec![0x20 | variant; 96],
            };
            let carrier =
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote.clone()))
                    .encode();
            let vote = adapter
                .registry
                .vote_to_core(&vote, &adapter.wire_context)
                .expect("convert alternate vote signature carrier");
            let candidate = adapter
                .serviced_candidate(
                    &reducer::Event::VoteReceived { tag, vote },
                    if variant % 2 == 0 {
                        DeferredPriority::Normal
                    } else {
                        DeferredPriority::Progress
                    },
                    None,
                    Some(&carrier),
                )
                .expect("vote has a service identity");
            match vote_key {
                Some(expected) => assert_eq!(
                    candidate.0, expected,
                    "authenticated signature replacements are one vote owner"
                ),
                None => vote_key = Some(candidate.0),
            }
            adapter
                .record_serviced_candidate(Some(candidate), false, false, None)
                .expect("coalesce vote carrier variant");
        }
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            marker_count + 5
        );
    }

    #[test]
    fn serviced_candidate_capacity_exhaustion_never_evicts_an_old_owner() {
        let directory = TempDir::new().expect("temporary directory");
        let geometry = ServicedCandidateCapacityGeometry::new(7, 3);
        let (mut adapter, startup) = open_test_with_capacity_geometry(&directory, geometry)
            .expect("open adapter with non-default production geometry");
        assert!(startup.is_empty());
        let capacity =
            serviced_candidate_capacity_with_geometry(adapter.wire_context.roster.len(), geometry);
        assert_eq!(adapter.serviced_candidate_capacity, capacity);
        assert_ne!(
            capacity,
            serviced_candidate_capacity(adapter.wire_context.roster.len()),
            "the configured runtime/effect geometry must replace the fixture default"
        );
        adapter.serviced_candidates.clear();
        for index in 0..capacity {
            let mut evidence = [0_u8; 32];
            evidence[..8].copy_from_slice(
                &u64::try_from(index)
                    .expect("bounded capacity index fits u64")
                    .to_le_bytes(),
            );
            let source_view = u64::try_from(index).expect("bounded source view fits u64");
            assert_eq!(
                adapter.serviced_candidates.insert(
                    ServicedCandidateKey::new(
                        adapter.wire_context.id(),
                        adapter.wire_context.height,
                        adapter.fingerprints.node.into(),
                        adapter.wire_context.leader(source_view),
                        source_view,
                        None,
                        0,
                        DeferredPriority::Normal.code(),
                        u8::MAX,
                        evidence,
                    ),
                    adapter.current_tag().view(),
                ),
                None
            );
        }
        let retained = adapter.serviced_candidates.clone();
        let reducer_before = adapter.reducer.clone();
        let overflow = unowned_body_event(&adapter, 0x42);
        assert!(matches!(
            adapter.step(overflow),
            Err(AdapterError::ServicedCandidateStore(reason))
                if reason.contains("capacity")
        ));
        assert!(adapter.fail_closed);
        assert_eq!(
            adapter.serviced_candidates, retained,
            "capacity exhaustion cannot evict a prior tombstone"
        );
        assert_eq!(
            adapter.reducer, reducer_before,
            "capacity must be reserved before the consuming reducer transition"
        );
    }

    #[test]
    fn persistence_macro_step_budgets_have_exact_five_effect_maximum() {
        let expected = [
            (
                PersistenceMacroStepClass::ProposalIntent,
                PersistenceMacroStepBudget::new(1, 1),
            ),
            (
                PersistenceMacroStepClass::PrepareIntent,
                PersistenceMacroStepBudget::new(2, 1),
            ),
            (
                PersistenceMacroStepClass::ObservePrepare,
                PersistenceMacroStepBudget::new(4, 1),
            ),
            (
                PersistenceMacroStepClass::LockAndCommit,
                PersistenceMacroStepBudget::new(3, 1),
            ),
            (
                PersistenceMacroStepClass::TimeoutIntent,
                PersistenceMacroStepBudget::new(1, 1),
            ),
            (
                PersistenceMacroStepClass::InstallTimeout,
                PersistenceMacroStepBudget::new(2, 4),
            ),
            (
                PersistenceMacroStepClass::Decision,
                PersistenceMacroStepBudget::new(2, 2),
            ),
        ];
        assert_eq!(
            PersistenceMacroStepClass::ALL,
            expected.map(|(class, _)| class),
            "the exhaustive WAL class inventory must remain source ordered"
        );
        for (class, budget) in expected {
            assert_eq!(class.budget(), budget);
            assert!(budget.initial_effects >= 1);
            assert!(budget.continuation_effects <= reducer::MAX_EFFECTS_PER_STEP);
            assert!(budget.flattened_effects() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
        }
        assert_eq!(
            PersistenceMacroStepClass::ALL
                .into_iter()
                .map(|class| class.budget().flattened_effects())
                .max(),
            Some(MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP)
        );
        assert_eq!(
            PersistenceMacroStepClass::InstallTimeout
                .budget()
                .flattened_effects(),
            MAX_FLATTENED_PERSISTENCE_EFFECTS_PER_MACRO_STEP,
            "local TC formation is the unique five-effect persistence witness"
        );
    }

    #[test]
    fn drive_effects_rejects_oversized_non_persisting_batch() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let effect = reducer::Effect::FetchBody {
            tag,
            round: reducer::Round::new(tag.height(), tag.view()),
            subject: reducer::Subject::default(),
            manifest: None,
            certified_sources: Vec::new(),
            certificate: None,
        };
        let oversized = vec![effect; MAX_ADAPTER_EFFECTS_PER_MACRO_STEP + 1];

        assert!(matches!(
            adapter.drive_effects(oversized),
            Err(AdapterError::AdapterMacroStepBoundExceeded {
                initial_effects,
                maximum_initial_effects,
                persist_effects: 0,
                continuation_effects: 0,
                continuation_contains_persist: false,
                ..
            }) if initial_effects == MAX_ADAPTER_EFFECTS_PER_MACRO_STEP + 1
                && maximum_initial_effects == MAX_ADAPTER_EFFECTS_PER_MACRO_STEP
        ));
        assert!(adapter.fail_closed);
        assert!(adapter.wal.recovered_records().is_empty());
    }

    #[test]
    fn drive_effects_rejects_record_specific_overbudget_before_wal_append() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let timeout = adapter
            .reducer
            .step(reducer::Event::TimeoutElapsed { tag })
            .expect("stage the sole TimeoutIntent Persist")
            .into_effects();
        assert!(matches!(
            timeout.as_slice(),
            [reducer::Effect::Persist { .. }]
        ));
        let unrelated = reducer::Effect::FetchBody {
            tag,
            round: reducer::Round::new(tag.height(), tag.view()),
            subject: reducer::Subject::default(),
            manifest: None,
            certified_sources: Vec::new(),
            certificate: None,
        };
        let mut overbudget = vec![unrelated];
        overbudget.extend(timeout);

        assert!(matches!(
            adapter.drive_effects(overbudget),
            Err(AdapterError::AdapterMacroStepBoundExceeded {
                initial_effects: 2,
                maximum_initial_effects: 1,
                persist_effects: 1,
                continuation_effects: 0,
                maximum_continuation_effects: 1,
                maximum_flattened_effects: 1,
                continuation_contains_persist: false,
            })
        ));
        assert!(adapter.fail_closed);
        assert!(adapter.wal.recovered_records().is_empty());
    }

    #[test]
    fn drive_effects_rejects_multiple_persist_owners_before_wal_append() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let mut timeout = adapter
            .reducer
            .step(reducer::Event::TimeoutElapsed { tag })
            .expect("stage the sole TimeoutIntent Persist")
            .into_effects();
        let persist = timeout.pop().expect("one Persist effect");
        assert!(matches!(&persist, reducer::Effect::Persist { .. }));

        assert!(matches!(
            adapter.drive_effects(vec![persist.clone(), persist]),
            Err(AdapterError::AdapterMacroStepBoundExceeded {
                persist_effects: 2,
                continuation_effects: 0,
                continuation_contains_persist: false,
                ..
            })
        ));
        assert!(adapter.fail_closed);
        assert!(adapter.wal.recovered_records().is_empty());
    }

    #[test]
    fn post_wal_oversized_continuation_fails_closed_and_replays_exact_record() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let wire_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let protected_subject = subject(0x6d);
        let prepare = wire::QuorumCertificate {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Prepare,
            subject: protected_subject,
            execution_commitment: execution_commitment(0x6d),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x6d; 96],
        };
        let timeout = wire::TimeoutCertificate {
            round: wire_round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x6e; 96],
            }],
        };
        let wire_context = adapter.wire_context.clone();
        let timeout = adapter
            .registry
            .tc_to_core(&timeout, &wire_context)
            .expect("convert the lock-promoting timeout certificate");
        let timeout_tag = adapter.current_tag();
        let pending_timeout = adapter
            .reducer
            .step(reducer::Event::TimeoutCertificateReceived {
                tag: timeout_tag,
                certificate: timeout,
            })
            .expect("stage the real InstallTimeout persistence");
        let mut pending_effects = pending_timeout.into_effects();
        let reducer::Effect::Persist { tag, entry } = pending_effects
            .pop()
            .expect("InstallTimeout has one Persist effect")
        else {
            panic!("InstallTimeout must stage persistence");
        };
        assert!(pending_effects.is_empty());

        // Keep the reducer's real lock-promoting continuation, but classify
        // and encode this adversarial boundary call as the smaller
        // TimeoutIntent class. The substitute is itself a valid first WAL
        // record with the exact pending persistence ID, so the continuation
        // guard is reached only after the append succeeds.
        let timeout_round = reducer::Round::new(wire_round.height, wire_round.view);
        let local_validator = adapter
            .reducer
            .local_validator()
            .expect("test adapter is a validator");
        let forged_entry = reducer::WalEntry::new(
            entry.id(),
            reducer::WalRecord::TimeoutIntent(reducer::TimeoutVote::new(
                adapter.reducer.context().id(),
                timeout_round,
                local_validator,
                None,
            )),
        );
        assert!(matches!(
            adapter.drive_effects(vec![reducer::Effect::Persist {
                tag,
                entry: forged_entry,
            }]),
            Err(AdapterError::AdapterMacroStepBoundExceeded {
                initial_effects: 1,
                maximum_initial_effects: 1,
                persist_effects: 1,
                continuation_effects: 2,
                maximum_continuation_effects: 1,
                maximum_flattened_effects: 1,
                continuation_contains_persist: false,
            })
        ));
        assert!(adapter.fail_closed);
        assert_eq!(adapter.wal.recovered_records().len(), 1);
        assert_eq!(adapter.wal.recovered_records()[0].sequence, 0);
        drop(adapter);

        let (recovered, first_startup) =
            open_test(&directory).expect("replay the one valid timeout intent");
        assert!(recovered.ingress_ready());
        assert!(!recovered.fail_closed);
        assert_eq!(recovered.wal.recovered_records().len(), 1);
        assert_eq!(recovered.reducer.durable_state().last_id().get(), 1);
        assert!(
            recovered
                .reducer
                .durable_state()
                .timeout_intent(timeout_round)
                .is_some()
        );
        assert!(first_startup.len() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
        assert!(matches!(
            first_startup.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(vote),
                ..
            }] if vote.round == wire_round
                && vote.highest_prepare_qc.is_none()
                && vote.signer == 0
                && vote.signature.is_empty()
        ));
        drop(recovered);

        let (recovered_again, second_startup) =
            open_test(&directory).expect("repeat deterministic timeout-intent replay");
        assert_eq!(second_startup, first_startup);
        assert!(second_startup.len() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
        assert_eq!(recovered_again.wal.recovered_records().len(), 1);
        assert!(recovered_again.ingress_ready());
        assert!(!recovered_again.fail_closed);
    }

    #[test]
    fn open_records_exactly_one_recovery_progress_transition() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        assert!(matches!(
            adapter.last_progress,
            Some((
                generation,
                round,
                wire::SumeragiV2ProgressTransition::RecoveryReplayed
            )) if generation == adapter.current_tag().generation()
                && round == reducer::Round::new(adapter.wire_context.height, 0)
        ));
        assert_eq!(
            adapter
                .ignore_counts
                .get(&reducer::IgnoreReason::Duplicate)
                .copied()
                .unwrap_or_default(),
            0,
            "opening must step ResumeAfterReplay once, not record a duplicate replay"
        );
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            0,
            "the replay control trigger cannot consume candidate-tombstone capacity"
        );
        for attempt in 0..3 {
            adapter
                .retransmit_elapsed(adapter.current_tag())
                .unwrap_or_else(|error| panic!("retransmit control attempt {attempt}: {error}"));
        }
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            0,
            "periodic retransmission triggers remain executable without becoming tombstones"
        );
        let status = adapter.status().expect("status after replay");
        assert!(matches!(
            status.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
                ..
            })
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn first_recovery_snapshot_tracks_the_durable_locked_body() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        let locked_subject = subject(0xCE);
        let wire_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let (_, keys, _) = authenticated_context();
        let mut wire_prepare = wire::QuorumCertificate {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Prepare,
            subject: locked_subject,
            execution_commitment: execution_commitment(0xCE),
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        authenticate_qc(&mut wire_prepare, &keys);
        let prepare = adapter
            .registry
            .qc_to_core(&wire_prepare, &adapter.wire_context)
            .expect("register durable PrepareQC");
        let round = prepare.round();
        let core_subject = prepare.subject();
        let local_validator = adapter
            .registry
            .validator_id(0)
            .expect("local fixture validator");
        let lock_entry = reducer::WalEntry::new(
            reducer::PersistenceId::new(1),
            reducer::WalRecord::LockAndCommit {
                prepare,
                vote: reducer::Vote::new(
                    adapter.reducer.context().id(),
                    round,
                    reducer::Phase::Commit,
                    core_subject,
                    local_validator,
                ),
            },
        );
        let encoded = adapter
            .registry
            .encode_wal_entry(&lock_entry, &TestAggregator)
            .expect("encode durable lock");
        assert_eq!(
            adapter.wal.append(&encoded).expect("append durable lock"),
            0
        );
        drop(adapter);

        let (mut recovered, startup) = open_test(&directory).expect("recover durable lock");
        assert!(matches!(
            startup.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::Vote(vote),
                ..
            }] if vote.phase == wire::GlobalPhase::Commit
                && vote.subject == locked_subject
        ));
        assert_eq!(recovered.active_subject, Some((round, core_subject)));
        let status = recovered.status().expect("first locked recovery snapshot");
        assert_eq!(
            status.liveness.work.candidate,
            wire::SumeragiV2LocalWorkStage::Complete
        );
        assert_eq!(
            status.liveness.work.body_recovery,
            wire::SumeragiV2LocalWorkStage::Queued
        );
        assert!(matches!(
            status.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
                ..
            })
        ));
    }

    #[test]
    fn persistence_is_fsynced_before_sign_is_exposed() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        assert!(adapter.ingress_ready());
        let proposer = adapter.status().expect("status").leader;
        let subject = subject(7);
        let proposal = proposal(&adapter.wire_context, proposer, subject);
        let fetch = adapter
            .receive_verified(proposal)
            .expect("accept proposal")
            .into_effects();
        let (tag, manifest) = match fetch.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };
        let round = manifest.round;
        let store = adapter
            .body_available(tag, manifest)
            .expect("body available")
            .into_effects();
        assert!(matches!(
            store.as_slice(),
            [AdapterEffect::StoreBody { .. }]
        ));
        let receipt = durable_body_receipt(&adapter, round, subject);
        let validate = adapter
            .body_stored(tag, round, subject, &receipt)
            .expect("body stored")
            .into_effects();
        assert!(matches!(
            validate.as_slice(),
            [AdapterEffect::ValidateBody { .. }]
        ));
        let validated = ValidatedBodyReceipt::for_test(receipt.clone());
        let sign = adapter
            .validation_succeeded(tag, round, subject, &validated)
            .expect("valid body")
            .into_effects();
        assert!(matches!(sign.as_slice(), [AdapterEffect::Sign { .. }]));
        assert_eq!(adapter.wal.recovered_records().len(), 1);
        assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
    }

    #[test]
    fn tc_promoted_lock_requires_same_subject_reproposal_before_commit() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let subject = subject(0x97);
        let manifest = wire::PayloadManifest::derive(
            &adapter.wire_context,
            round,
            subject,
            5,
            &[b"chunk".to_vec()],
        )
        .expect("valid certified-body manifest");
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &manifest);
        let execution_commitment = validated.execution_commitment();
        let prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signers: vec![1, 2, 3],
            aggregate_signature: vec![0x97; 96],
        };

        let timeout_tag = adapter.current_tag();
        let timeout_sign = adapter
            .timeout_elapsed(timeout_tag)
            .expect("persist a local timeout without the remote PrepareQC")
            .into_effects();
        assert!(matches!(
            timeout_sign.as_slice(),
            [AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(vote),
            }] if *tag == timeout_tag && vote.highest_prepare_qc.is_none()
        ));
        assert_eq!(adapter.wal.recovered_records().len(), 1);
        adapter
            .signature_completed(timeout_tag, vec![0xA7; 96])
            .expect("complete the timeout vote before installing the remote TC");

        let timeout = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare.clone()),
                signers: vec![1, 2, 3],
                aggregate_signature: vec![0xB7; 96],
            }],
        };
        let installed = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
            ))
            .expect("install the TC carrying a PrepareQC missed by this validator")
            .into_effects();
        assert_eq!(adapter.wal.recovered_records().len(), 2);
        assert!(
            installed
                .iter()
                .all(|effect| !matches!(effect, AdapterEffect::Sign { .. })),
            "the TC cannot expose Commit signing before local body validation"
        );
        let fetch_tag = match installed.as_slice() {
            [
                AdapterEffect::EnterView {
                    tag: enter_tag,
                    protected_body: Some((protected_round, protected_subject)),
                    ..
                },
                AdapterEffect::FetchBody {
                    tag,
                    round: fetched_round,
                    subject: fetched_subject,
                    certificate: Some(certificate),
                    ..
                },
            ] if enter_tag == tag
                && *protected_round == round
                && *protected_subject == subject
                && *fetched_round == round
                && *fetched_subject == subject
                && certificate.as_ref() == prepare.as_ref() =>
            {
                *tag
            }
            effects => panic!(
                "TC acknowledgement must expose EnterView before its exact body fetch: {effects:?}"
            ),
        };

        assert!(matches!(
            adapter
                .body_available(fetch_tag, manifest)
                .expect("recover the TC-protected body")
                .effects(),
            [AdapterEffect::StoreBody {
                tag,
                round: stored_round,
                subject: stored_subject,
            }] if *tag == fetch_tag
                && *stored_round == round
                && *stored_subject == subject
        ));
        assert!(matches!(
            adapter
                .body_stored(fetch_tag, round, subject, &durable)
                .expect("store the TC-protected body")
                .effects(),
            [AdapterEffect::ValidateBody {
                tag,
                round: validated_round,
                subject: validated_subject,
            }] if *tag == fetch_tag
                && *validated_round == round
                && *validated_subject == subject
        ));
        let validation = adapter
            .validation_succeeded(fetch_tag, round, subject, &validated)
            .expect("validate the TC-protected body without relabelling its origin")
            .into_effects();
        let current_round = wire::ConsensusRound {
            view: fetch_tag.view(),
            ..round
        };
        assert_eq!(
            current_round.view,
            round.view + 1,
            "the TC installs the successor proposal view"
        );
        assert!(
            validation.is_empty(),
            "validating an old-round lock cannot mint a split-round Commit vote: {validation:?}"
        );
        assert_eq!(
            adapter.wal.recovered_records().len(),
            2,
            "validation must not append LockAndCommit until the immutable body is re-proposed"
        );
        assert_eq!(adapter.reducer.durable_state().last_id().get(), 2);
        let core_current_round = reducer::Round::new(current_round.height, current_round.view);
        assert_eq!(
            adapter
                .reducer
                .durable_state()
                .commit_intent(core_current_round),
            None,
            "only a new same-round PrepareQC may authorize Commit in the successor view"
        );
        let status = adapter.status().expect("protected reproposal status");
        assert!(status.liveness.outbound_intents.iter().all(|intent| {
            !matches!(
                intent.kind,
                wire::SumeragiV2OutboundIntentKind::CommitVote
                    | wire::SumeragiV2OutboundIntentKind::CommitQc
            )
        }));
    }

    #[test]
    fn leader_without_owned_candidate_work_reports_missing_proposal_state() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader adapter");
        assert!(startup.is_empty());
        let status = adapter.status().expect("fresh leader status");
        let local = adapter
            .registry
            .validator_index(
                adapter
                    .reducer
                    .local_validator()
                    .expect("fixture has a local validator"),
            )
            .expect("map local validator");
        assert_eq!(status.leader, local, "fixture local validator is leader");
        assert_eq!(
            status.liveness.work.candidate,
            wire::SumeragiV2LocalWorkStage::Idle,
            "leadership alone is not ownership of candidate construction"
        );
        assert_eq!(status.phase, wire::SumeragiV2StatusPhase::AwaitingProposal);
    }

    #[test]
    fn one_round_and_subject_cannot_change_its_registered_manifest() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, _) = open_test(&directory).expect("open adapter");
        let proposer = adapter.status().expect("status").leader;
        let subject = subject(0x3D);
        let fetch = adapter
            .receive_verified(proposal(&adapter.wire_context, proposer, subject))
            .expect("accept proposal")
            .into_effects();
        let (tag, manifest) = match fetch.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };
        adapter
            .body_available(tag, manifest.clone())
            .expect("register exact manifest");
        let conflicting = wire::PayloadManifest::derive(
            &adapter.wire_context,
            manifest.round,
            manifest.subject,
            5,
            &[b"other".to_vec()],
        )
        .expect("structurally valid conflicting manifest");

        assert!(matches!(
            adapter.body_available(tag, conflicting),
            Err(AdapterError::ConflictingManifest)
        ));
    }

    #[test]
    fn authenticated_proposal_cannot_conflict_with_registered_canonical_manifest() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, _) = open_test(&directory).expect("open adapter");
        let context = adapter.wire_context.clone();
        let proposer = adapter.status().expect("status").leader;
        let subject = subject(0x3E);
        let canonical = proposal(&context, proposer, subject);
        let wire::ConsensusMessageV2Payload::Proposal(canonical_proposal) = &canonical.payload
        else {
            panic!("fixture is a proposal")
        };
        adapter
            .registry
            .manifest_to_core(&canonical_proposal.manifest, &context)
            .expect("register canonical body manifest before proposal arrival");

        let canonical = AuthenticatedConsensusMessage::for_test(canonical);
        adapter
            .ensure_authenticated_manifest_compatible(&canonical)
            .expect("the exact registered manifest remains admissible");

        let mut conflicting = proposal(&context, proposer, subject);
        let wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal) =
            &mut conflicting.payload
        else {
            panic!("fixture is a proposal")
        };
        conflicting_proposal.manifest = wire::PayloadManifest::derive(
            &context,
            conflicting_proposal.round,
            conflicting_proposal.subject,
            5,
            &[b"other".to_vec()],
        )
        .expect("structurally valid alternate manifest");
        let conflicting = AuthenticatedConsensusMessage::for_test(conflicting);
        assert!(matches!(
            adapter.ensure_authenticated_manifest_compatible(&conflicting),
            Err(AdapterError::ConflictingManifest)
        ));
        assert!(!adapter.fail_closed);
    }

    #[test]
    fn proposal_registry_preserves_the_first_exact_semantic_envelope() {
        let context = context();
        let mut registry = WireRegistry::new(&context).expect("registry");
        let wire::ConsensusMessageV2Payload::Proposal(first) =
            proposal(&context, context.leader(0), subject(0x40)).payload
        else {
            unreachable!("proposal fixture")
        };
        let mut later = first.clone();
        later.signature = vec![0x40; 96];

        registry
            .proposal_to_core(&first, &context)
            .expect("register first exact proposal envelope");
        registry
            .proposal_to_core(&later, &context)
            .expect("the same semantic proposal remains convertible");

        let key = (
            reducer::Round::new(first.round.height, first.round.view),
            reducer::Subject::new(Hash::new(first.subject.encode()).into()),
        );
        assert_eq!(
            registry.proposals.get(&key),
            Some(&first),
            "a later exact-envelope alias cannot retarget durable re-signing"
        );
    }

    #[test]
    fn canonical_body_rolls_back_exact_busy_deferred_conflicting_proposal() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, _) = open_test(&directory).expect("open adapter");
        let context = adapter.wire_context.clone();
        let proposer = adapter.status().expect("status").leader;
        let subject = subject(0x3F);
        let canonical = proposal(&context, proposer, subject);
        let wire::ConsensusMessageV2Payload::Proposal(canonical_proposal) = &canonical.payload
        else {
            panic!("fixture is a proposal")
        };
        let canonical_manifest = canonical_proposal.manifest.clone();
        let round = canonical_manifest.round;

        let mut conflicting = proposal(&context, proposer, subject);
        let conflicting_proposal = {
            let wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal) =
                &mut conflicting.payload
            else {
                panic!("fixture is a proposal")
            };
            conflicting_proposal.manifest = wire::PayloadManifest::derive(
                &context,
                conflicting_proposal.round,
                conflicting_proposal.subject,
                5,
                &[b"other".to_vec()],
            )
            .expect("structurally valid alternate manifest");
            conflicting_proposal.clone()
        };
        let conflicting_wire_identity = Arc::<[u8]>::from(conflicting.encode());
        let deferred = adapter
            .registry
            .proposal_to_core(&conflicting_proposal, &context)
            .expect("convert authenticated proposal before reducer reports Busy");
        let deferred_tag = adapter.current_tag();
        adapter.deferred_inputs.push_back(DeferredInput {
            admission_ordinal: 1,
            admission_capability: DeferredAdmissionCapability::for_test(1),
            event: reducer::Event::ProposalReceived {
                tag: deferred_tag,
                proposal: deferred,
            },
            completion_evidence: None,
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Normal,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: Some(conflicting_wire_identity),
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
        let admission_key = IngressSemanticKey::Proposal { round, proposer };
        adapter.ingress_equivocations.insert(
            admission_key,
            IngressEquivocationRecord {
                fingerprint: IngressFingerprint::Proposal(Hash::new(
                    conflicting_proposal.signature_preimage(),
                )),
                equivocation_reported: true,
                capacity_bypass: false,
                admitted_at: Instant::now(),
            },
        );
        adapter.ingress_deliveries.insert(
            admission_key,
            IngressDeliveryRecord {
                fingerprint: IngressFingerprint::Proposal(Hash::new(
                    conflicting_proposal.signature_preimage(),
                )),
                generation: deferred_tag.generation(),
                locked_commit_progress: false,
            },
        );

        let retained_qc = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: execution_commitment(0x3F),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x3F; 96],
        };
        adapter
            .registry
            .qc_to_core(&retained_qc, &context)
            .expect("register independently authenticated QC material");
        let retained_certificates = adapter.registry.certificates.clone();
        let retained_execution_commitments = adapter.registry.execution_commitments.clone();
        assert!(adapter.registry.manifest_conflicts(&canonical_manifest));

        let outcome = adapter
            .body_available(deferred_tag, canonical_manifest.clone())
            .expect("canonical body supersedes only its Busy-deferred proposal authority");
        assert_eq!(
            outcome.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
        );
        assert!(adapter.deferred_inputs.is_empty());
        assert!(!adapter.ingress_equivocations.contains_key(&admission_key));
        assert!(!adapter.ingress_deliveries.contains_key(&admission_key));
        assert!(adapter.registry.proposals.is_empty());
        assert_eq!(
            adapter.registry.manifests.values().next(),
            Some(&canonical_manifest)
        );
        assert_eq!(adapter.registry.certificates, retained_certificates);
        assert_eq!(
            adapter.registry.execution_commitments,
            retained_execution_commitments
        );
        assert!(!adapter.fail_closed);
    }

    #[test]
    fn forged_body_receipt_cannot_cross_the_prepare_durability_boundary() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, _) = open_test(&directory).expect("open adapter");
        let proposer = adapter.status().expect("status").leader;
        let proposed_subject = subject(31);
        let fetch = adapter
            .receive_verified(proposal(&adapter.wire_context, proposer, proposed_subject))
            .expect("accept proposal")
            .into_effects();
        let (tag, manifest) = match fetch.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };
        let round = manifest.round;
        adapter
            .body_available(tag, manifest)
            .expect("body available");
        let correct = durable_body_receipt(&adapter, round, proposed_subject);
        let forged = DurableBodyReceipt::for_test(
            adapter.wire_context.id(),
            round,
            subject(32),
            correct.manifest_hash(),
        );
        assert!(matches!(
            adapter.body_stored(tag, round, proposed_subject, &forged),
            Err(AdapterError::DurableBodyMismatch)
        ));
        assert!(matches!(
            adapter
                .body_stored(tag, round, proposed_subject, &correct)
                .expect("the real durable receipt remains usable")
                .effects(),
            [AdapterEffect::ValidateBody { .. }]
        ));
    }

    #[test]
    fn local_proposal_and_prepare_are_each_persisted_before_signing() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let subject = subject(8);
        let leader = adapter.wire_context.leader(0);
        let proposal = proposal(&adapter.wire_context, leader, subject);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let proposal_tag = adapter.current_tag();
        let sign = adapter
            .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
            .expect("submit local proposal")
            .into_effects();
        let tag = match sign.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Proposal(proposal),
                },
            ] => {
                assert!(proposal.signature.is_empty());
                *tag
            }
            effects => panic!("unexpected local proposal effects: {effects:?}"),
        };
        assert_eq!(adapter.wal.recovered_records().len(), 1);

        let effects = adapter
            .signature_completed(tag, vec![0xD1; 96])
            .expect("sign local proposal")
            .into_effects();
        assert!(matches!(
            effects.as_slice(),
            [
                AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::Proposal(_),
                    ..
                }),
                AdapterEffect::Sign {
                    request: SignRequest::Vote(_),
                    ..
                }
            ]
        ));
        assert_eq!(adapter.wal.recovered_records().len(), 2);
        assert_eq!(adapter.reducer.durable_state().last_id().get(), 2);
    }

    #[test]
    fn local_proposal_commitment_conflict_is_transactional() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let proposed_subject = subject(0x7b);
        let leader = adapter.wire_context.leader(0);
        let proposal = proposal(&adapter.wire_context, leader, proposed_subject);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let manifest = proposal.manifest;
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &manifest);
        let round = reducer::Round::new(manifest.round.height, manifest.round.view);
        let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
        let conflicting = execution_commitment(0x7c);
        assert_ne!(conflicting, validated.execution_commitment());
        adapter
            .registry
            .register_execution_commitment(round, core_subject, conflicting)
            .expect("pre-bind a conflicting authenticated commitment");

        let subjects_before = adapter.registry.subjects.clone();
        let manifests_before = adapter.registry.manifests.clone();
        let commitments_before = adapter.registry.execution_commitments.clone();
        let active_before = adapter.active_subject;
        let reducer_before = adapter.reducer.clone();
        let wal_len_before = adapter.wal.recovered_records().len();

        assert!(matches!(
            adapter.local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated,),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));
        assert_eq!(adapter.registry.subjects, subjects_before);
        assert_eq!(adapter.registry.manifests, manifests_before);
        assert_eq!(adapter.registry.execution_commitments, commitments_before);
        assert_eq!(adapter.active_subject, active_before);
        assert_eq!(adapter.reducer, reducer_before);
        assert_eq!(adapter.wal.recovered_records().len(), wal_len_before);
    }

    #[test]
    fn exact_local_completion_after_decision_reports_body_validated_progress() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let predecision_a = unowned_body_event(&adapter, 0x79);
        adapter
            .step(predecision_a)
            .expect("service pre-Decision candidate A");
        let predecision_b = unowned_body_event(&adapter, 0x7A);
        adapter
            .step(predecision_b)
            .expect("service pre-Decision candidate B");
        assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
        assert_eq!(adapter.durable_serviced_candidates.len(), 2);
        let decided_subject = subject(0x7d);
        let leader = adapter.wire_context.leader(0);
        let proposal = proposal(&adapter.wire_context, leader, decided_subject);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let manifest = proposal.manifest;
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &manifest);
        let decision = wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: decided_subject,
            execution_commitment: validated.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x7d; 96],
        };
        let decided = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    decision.clone(),
                )),
            ))
            .expect("install the exact durable Decision");
        assert!(matches!(
            decided.effects(),
            [AdapterEffect::FetchBody { .. }]
        ));
        assert!(adapter.serviced_candidates_decision_reclaimed);
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            0,
            "durable Decision reclaims the complete candidate-service epoch, including its triggering occurrence"
        );

        let applied = adapter
            .local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated)
            .expect("transfer trusted local validation to the Decision");
        let apply_tag = match applied.effects() {
            [
                AdapterEffect::Apply {
                    tag,
                    subject,
                    certificate,
                },
            ] if *subject == decided_subject && certificate == &decision => *tag,
            effects => panic!("unexpected exact Decision application effects: {effects:?}"),
        };
        assert!(matches!(
            adapter.status().expect("liveness snapshot").liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                round,
                transition: wire::SumeragiV2ProgressTransition::BodyValidated,
                ..
            }) if round == decision.round
        ));
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            0,
            "post-Decision application progress cannot resurrect candidate tombstones"
        );
        let completed = adapter
            .application_completed(apply_tag, decided_subject)
            .expect("retire the exact Decision application lifecycle");
        assert_eq!(completed.disposition(), reducer::StepDisposition::Applied);
        assert!(completed.effects().is_empty());
        for attempt in 0..3 {
            let retransmit = adapter
                .retransmit_elapsed(adapter.current_tag())
                .unwrap_or_else(|error| panic!("post-drain retransmission {attempt}: {error}"));
            assert!(
                retransmit.effects().is_empty(),
                "a drained exact Decision lifecycle cannot recreate physical Fetch/Store/Validate/Apply work: {:?}",
                retransmit.effects()
            );
        }
        assert!(adapter.deferred_completions.is_empty());
        assert!(adapter.durable_serviced_candidates.is_empty());
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            0,
            "monotone applied state, not a recycled dormant ordinal or tombstone, suppresses resurrection"
        );
    }

    #[test]
    fn busy_local_completion_during_decision_wal_reaches_apply_once() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let decided_subject = subject(0x7e);
        let leader = adapter.wire_context.leader(0);
        let proposal = proposal(&adapter.wire_context, leader, decided_subject);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let manifest = proposal.manifest;
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &manifest);
        let decision = wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: decided_subject,
            execution_commitment: validated.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x7e; 96],
        };
        let context = adapter.wire_context.clone();
        let certificate = adapter
            .registry
            .qc_to_core(&decision, &context)
            .expect("convert exact Decision certificate");
        let decision_tag = adapter.current_tag();
        let pending_decision = adapter
            .reducer
            .step(reducer::Event::QuorumCertificateReceived {
                tag: decision_tag,
                certificate,
            })
            .expect("stage Decision WAL persistence");
        assert!(matches!(
            pending_decision.effects(),
            [reducer::Effect::Persist { .. }]
        ));

        let busy = adapter
            .local_proposal_ready(decision_tag, manifest, &durable, &validated)
            .expect("Busy boundary retains the trusted local completion");
        assert_eq!(
            busy.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert!(busy.effects().is_empty());
        assert_eq!(adapter.deferred_completions.len(), 1);

        let decision_effects = adapter
            .drive_effects(pending_decision.into_effects())
            .expect("fsync and acknowledge the Decision WAL record");
        assert!(matches!(
            decision_effects.as_slice(),
            [AdapterEffect::FetchBody {
                subject,
                certificate: Some(certificate),
                ..
            }] if *subject == decided_subject && certificate == &decision
        ));
        let completion_effects = adapter
            .drain_deferred()
            .expect("fairly service the Busy-deferred completion");
        assert!(matches!(
            completion_effects.as_slice(),
            [AdapterEffect::Apply {
                subject,
                certificate,
                ..
            }] if *subject == decided_subject && certificate == &decision
        ));
        assert!(adapter.deferred_completions.is_empty());
        assert!(
            adapter
                .drain_deferred()
                .expect("completion cannot be applied twice")
                .is_empty()
        );
    }

    #[test]
    fn busy_deferred_input_blocks_terminal_readiness_until_serviced() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let decided_subject = subject(0x7f);
        let leader = adapter.wire_context.leader(0);
        let proposal = proposal(&adapter.wire_context, leader, decided_subject);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let manifest = proposal.manifest;
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &manifest);
        let decision = wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: decided_subject,
            execution_commitment: validated.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x7f; 96],
        };
        let context = adapter.wire_context.clone();
        let certificate = adapter
            .registry
            .qc_to_core(&decision, &context)
            .expect("convert exact Decision certificate");
        let decision_tag = adapter.current_tag();
        let pending_decision = adapter
            .reducer
            .step(reducer::Event::QuorumCertificateReceived {
                tag: decision_tag,
                certificate,
            })
            .expect("stage Decision WAL persistence");
        assert!(matches!(
            pending_decision.effects(),
            [reducer::Effect::Persist { .. }]
        ));

        let busy_completion = adapter
            .local_proposal_ready(decision_tag, manifest.clone(), &durable, &validated)
            .expect("retain the trusted completion across the Busy fence");
        assert_eq!(
            busy_completion.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        let terminal_vote = wire::Vote {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Prepare,
            subject: decided_subject,
            execution_commitment: validated.execution_commitment(),
            signer: 3,
            signature: vec![0x80; 96],
        };
        let busy_vote = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(terminal_vote)),
            ))
            .expect("retain authenticated ingress across the Busy fence");
        assert_eq!(
            busy_vote.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_completions.len(), 1);
        assert_eq!(adapter.deferred_inputs.len(), 1);

        let decision_effects = adapter
            .drive_effects(pending_decision.into_effects())
            .expect("fsync and acknowledge the Decision WAL record");
        assert!(matches!(
            decision_effects.as_slice(),
            [AdapterEffect::FetchBody { subject, .. }] if *subject == decided_subject
        ));
        let completion_effects = adapter
            .drain_deferred()
            .expect("service the retained completion first");
        assert!(matches!(
            completion_effects.as_slice(),
            [AdapterEffect::Apply { subject, .. }] if *subject == decided_subject
        ));
        assert!(adapter.deferred_completions.is_empty());
        assert_eq!(adapter.deferred_inputs.len(), 1);

        let applied = adapter
            .application_completed(decision_tag, decided_subject)
            .expect("acknowledge exact decision application");
        assert_eq!(applied.disposition(), reducer::StepDisposition::Applied);
        assert!(applied.effects().is_empty());
        assert!(adapter.reducer.ready_to_finish());
        assert!(adapter.deferred_work_is_serviceable());
        assert!(
            !adapter.ready_to_finish(),
            "adapter-owned Busy debt must block terminal height rollover"
        );

        assert!(
            adapter
                .drain_deferred()
                .expect("retire the authenticated terminal vote")
                .is_empty()
        );
        assert!(adapter.deferred_inputs.is_empty());
        assert!(adapter.ready_to_finish());
    }

    #[test]
    fn saturated_normal_lane_retains_exact_local_proposal_completion() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());

        let proposed_subject = subject(0x81);
        let leader = adapter.wire_context.leader(0);
        let proposal = proposal(&adapter.wire_context, leader, proposed_subject);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let manifest = proposal.manifest;
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &manifest);
        let evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable.clone(),
            validated_receipt: validated.clone(),
        };
        let proposal_tag = adapter.current_tag();
        let sign = adapter
            .local_proposal_ready(proposal_tag, manifest.clone(), &durable, &validated)
            .expect("persist the local proposal before signing")
            .into_effects();
        let sign_tag = match sign.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Proposal(_),
                },
            ] => *tag,
            effects => panic!("unexpected local proposal effects: {effects:?}"),
        };

        let deferred_vote = wire::Vote {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0x82),
            execution_commitment: execution_commitment(0x82),
            signer: 0,
            signature: vec![0x82; 96],
        };
        let busy = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(deferred_vote),
            ))
            .expect("defer normal ingress behind the proposal signature");
        assert_eq!(
            busy.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        let filler = adapter
            .deferred_inputs
            .front()
            .expect("normal ingress owns one deferred slot")
            .clone();
        assert_eq!(filler.priority, DeferredPriority::Normal);
        adapter.deferred_inputs = std::iter::repeat_n(filler, MAX_DEFERRED_INPUTS).collect();

        let first_retry = adapter
            .local_proposal_ready(proposal_tag, manifest.clone(), &durable, &validated)
            .expect("trusted local completion bypasses saturated normal ingress");
        assert_eq!(
            first_retry.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
        assert_eq!(
            adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
            (1, 1),
            "the full manifest and both receipts have exactly one completion owner"
        );
        assert!(matches!(
            adapter.deferred_completions.front(),
            Some(DeferredInput {
                event: reducer::Event::LocalProposalReady { .. },
                priority: DeferredPriority::Completion,
                ..
            })
        ));
        let first_completion_ordinal = adapter
            .deferred_completions
            .front()
            .expect("first completion retains an exact owner")
            .admission_ordinal;
        let next_ordinal_before_duplicate = adapter.deferred_admission_ordinals.next_for_test();

        let exact_retry = adapter
            .local_proposal_ready(proposal_tag, manifest, &durable, &validated)
            .expect("an exact retry coalesces with its existing owner");
        assert_eq!(
            exact_retry.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
        assert_eq!(
            adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
            (1, 1),
            "an exact retry cannot duplicate completion ownership"
        );
        assert_eq!(
            adapter
                .deferred_completions
                .front()
                .expect("duplicate retains the original owner")
                .admission_ordinal,
            first_completion_ordinal,
            "an exact duplicate must not mint or reset its admission ordinal"
        );
        assert_eq!(
            adapter.deferred_admission_ordinals.next_for_test(),
            next_ordinal_before_duplicate,
            "duplicate coalescing must not consume an actor ordinal"
        );

        let completed = adapter
            .signature_completed(sign_tag, vec![0x81; 96])
            .expect("signature completion drains the retained proposal retry")
            .into_effects();
        assert!(completed.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(_),
                ..
            })
        )));
        let prepare_sign_tag = completed
            .iter()
            .find_map(|effect| match effect {
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(vote),
                } if vote.phase == wire::GlobalPhase::Prepare
                    && vote.subject == proposed_subject =>
                {
                    Some(*tag)
                }
                _ => None,
            })
            .expect("proposal completion opens its serialized Prepare signature");
        assert_eq!(
            adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
            (1, 1),
            "the retry remains owned while the causally next signature is outstanding"
        );
        assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);

        let prepare_completed = adapter
            .signature_completed(prepare_sign_tag, vec![0x82; 96])
            .expect("Prepare signature releases all deferred reducer work")
            .into_effects();
        assert!(prepare_completed.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(vote),
                ..
            }) if vote.phase == wire::GlobalPhase::Prepare
                && vote.subject == proposed_subject
        )));
        assert_eq!(
            adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
            (1, 1),
            "signature completion cannot concatenate a second reducer macro-step"
        );
        assert!(adapter.deferred_work_is_serviceable());
        assert!(
            adapter
                .drain_deferred()
                .expect("service the retained local completion in its own turn")
                .is_empty()
        );
        assert_eq!(
            adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
            (0, 0),
            "one explicit deferred turn retires the sole completion owner"
        );
        assert!(adapter.deferred_inputs.len() <= MAX_DEFERRED_INPUTS);
        assert!(adapter.ingress_ready());
        assert!(!adapter.fail_closed);
    }

    #[test]
    fn replay_resigns_a_durable_proposal_before_prepare() {
        let directory = TempDir::new().expect("temporary directory");
        {
            let (mut adapter, _) = open_test_as_leader(&directory).expect("open leader");
            let subject = subject(10);
            let leader = adapter.wire_context.leader(0);
            let proposal = proposal(&adapter.wire_context, leader, subject);
            let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
                unreachable!("proposal helper returns a proposal")
            };
            let (durable, validated) =
                validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
            let proposal_tag = adapter.current_tag();
            let sign = adapter
                .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
                .expect("persist proposal intent");
            assert!(matches!(
                sign.effects(),
                [AdapterEffect::Sign {
                    request: SignRequest::Proposal(_),
                    ..
                }]
            ));
        }

        let (adapter, startup) = open_test_as_leader(&directory).expect("replay leader");
        assert!(adapter.ingress_ready());
        assert!(matches!(
            startup.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            }]
        ));
        assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
    }

    #[test]
    fn proposal_signed_callback_is_restart_scoped_before_control_delivery() {
        let directory = TempDir::new().expect("temporary directory");
        let proposal_signature = vec![0xD1; 96];
        {
            let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
            assert!(startup.is_empty());
            let proposed_subject = subject(0xA8);
            let proposal = proposal(
                &adapter.wire_context,
                adapter.wire_context.leader(0),
                proposed_subject,
            );
            let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
                unreachable!("proposal fixture")
            };
            let (durable, validated) =
                validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
            let sign = adapter
                .local_proposal_ready(
                    adapter.current_tag(),
                    proposal.manifest,
                    &durable,
                    &validated,
                )
                .expect("persist proposal intent before signing");
            let sign_tag = match sign.effects() {
                [
                    AdapterEffect::Sign {
                        tag,
                        request: SignRequest::Proposal(_),
                    },
                ] => *tag,
                effects => panic!("unexpected proposal sign effects: {effects:?}"),
            };
            let retained = adapter.serviced_candidate_count_for_test();
            let signed = adapter
                .signature_completed(sign_tag, proposal_signature.clone())
                .expect("complete proposal signature before simulated control loss");
            assert!(signed.effects().iter().any(|effect| matches!(
                effect,
                AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::Proposal(_),
                    ..
                })
            )));
            assert!(signed.effects().iter().any(|effect| matches!(
                effect,
                AdapterEffect::Sign {
                    request: SignRequest::Vote(vote),
                    ..
                } if vote.phase == wire::GlobalPhase::Prepare
            )));
            assert_eq!(
                adapter.serviced_candidate_count_for_test(),
                retained,
                "a Signed callback is not a durable candidate tombstone"
            );
            // Drop both returned controls: the WAL contains ProposalIntent and
            // PrepareIntent, while neither broadcast reached transport.
        }

        let context = context();
        let leader = context.leader(0);
        let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("leader-safety.wal"),
            verified_genesis(context),
            Some(leader),
            reducer::Generation::new(2),
            [0x22; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("recover proposal and Prepare intents");
        let proposal_tag = match startup.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Proposal(_),
                },
            ] => *tag,
            effects => panic!("unexpected recovered proposal frontier: {effects:?}"),
        };
        let retained = recovered.serviced_candidate_count_for_test();
        let replayed = recovered
            .signature_completed(proposal_tag, proposal_signature)
            .expect("new generation accepts the replay-issued proposal callback");
        assert!(replayed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(_),
                ..
            })
        )));
        let prepare_tag = replayed
            .effects()
            .iter()
            .find_map(|effect| match effect {
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(vote),
                } if vote.phase == wire::GlobalPhase::Prepare => Some(*tag),
                _ => None,
            })
            .expect("recovered proposal releases its durable Prepare signature");
        assert_eq!(recovered.serviced_candidate_count_for_test(), retained);
        let prepare_signature = vec![0xD2; 96];
        let prepared = recovered
            .signature_completed(prepare_tag, prepare_signature.clone())
            .expect("complete replayed Prepare signature");
        assert!(prepared.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(vote),
                ..
            }) if vote.phase == wire::GlobalPhase::Prepare
        )));
        assert_eq!(
            recovered
                .signature_completed(prepare_tag, prepare_signature)
                .expect("same-episode duplicate is reducer-idempotent")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
        );
    }

    #[test]
    fn vote_signed_callback_is_restart_scoped_before_control_delivery() {
        let directory = TempDir::new().expect("temporary directory");
        let vote_signature = vec![0xE1; 96];
        let prepared_subject = subject(0xA9);
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let proposer = adapter.status().expect("status").leader;
            let fetch = adapter
                .receive_verified(proposal(&adapter.wire_context, proposer, prepared_subject))
                .expect("accept remote proposal");
            let (tag, manifest) = match fetch.effects() {
                [
                    AdapterEffect::FetchBody {
                        tag,
                        manifest: Some(manifest),
                        ..
                    },
                ] => (*tag, manifest.clone()),
                effects => panic!("unexpected proposal effects: {effects:?}"),
            };
            let round = manifest.round;
            adapter
                .body_available(tag, manifest)
                .expect("make remote body available");
            let receipt = durable_body_receipt(&adapter, round, prepared_subject);
            adapter
                .body_stored(tag, round, prepared_subject, &receipt)
                .expect("acknowledge durable body");
            let validated = ValidatedBodyReceipt::for_test(receipt);
            let sign = adapter
                .validation_succeeded(tag, round, prepared_subject, &validated)
                .expect("persist Prepare intent");
            let sign_tag = match sign.effects() {
                [
                    AdapterEffect::Sign {
                        tag,
                        request: SignRequest::Vote(vote),
                    },
                ] if vote.phase == wire::GlobalPhase::Prepare => *tag,
                effects => panic!("unexpected Prepare sign effects: {effects:?}"),
            };
            let retained = adapter.serviced_candidate_count_for_test();
            let signed = adapter
                .signature_completed(sign_tag, vote_signature.clone())
                .expect("complete Prepare signature before simulated transport loss");
            assert!(signed.effects().iter().any(|effect| matches!(
                effect,
                AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::Vote(vote),
                    ..
                }) if vote.phase == wire::GlobalPhase::Prepare
            )));
            assert_eq!(adapter.serviced_candidate_count_for_test(), retained);
        }

        let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(2),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("recover durable Prepare intent");
        let sign_tag = match startup.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(vote),
                },
            ] if vote.phase == wire::GlobalPhase::Prepare && vote.subject == prepared_subject => {
                *tag
            }
            effects => panic!("unexpected recovered Prepare frontier: {effects:?}"),
        };
        let signed = recovered
            .signature_completed(sign_tag, vote_signature.clone())
            .expect("new generation accepts the replay-issued Prepare callback");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(vote),
                ..
            }) if vote.phase == wire::GlobalPhase::Prepare
                && vote.subject == prepared_subject
        )));
        assert_eq!(
            recovered
                .signature_completed(sign_tag, vote_signature)
                .expect("same-episode duplicate is reducer-idempotent")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
        );
    }

    #[test]
    fn timeout_signed_callback_is_restart_scoped_before_control_delivery() {
        let directory = TempDir::new().expect("temporary directory");
        let timeout_signature = vec![0xF1; 96];
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let sign = adapter
                .timeout_elapsed(adapter.current_tag())
                .expect("persist Timeout intent");
            let sign_tag = match sign.effects() {
                [
                    AdapterEffect::Sign {
                        tag,
                        request: SignRequest::TimeoutVote(_),
                    },
                ] => *tag,
                effects => panic!("unexpected Timeout sign effects: {effects:?}"),
            };
            let retained = adapter.serviced_candidate_count_for_test();
            let signed = adapter
                .signature_completed(sign_tag, timeout_signature.clone())
                .expect("complete Timeout signature before simulated transport loss");
            assert!(signed.effects().iter().any(|effect| matches!(
                effect,
                AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                    ..
                })
            )));
            assert_eq!(adapter.serviced_candidate_count_for_test(), retained);
        }

        let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(2),
            [0x11; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("recover durable Timeout intent");
        let sign_tag = match startup.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(_),
                },
            ] => *tag,
            effects => panic!("unexpected recovered Timeout frontier: {effects:?}"),
        };
        let signed = recovered
            .signature_completed(sign_tag, timeout_signature.clone())
            .expect("new generation accepts the replay-issued Timeout callback");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                ..
            })
        )));
        assert_eq!(
            recovered
                .signature_completed(sign_tag, timeout_signature)
                .expect("same-episode duplicate is reducer-idempotent")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
        );
    }

    #[test]
    fn deferred_adapter_replay_with_startup_effects_publishes_no_status() {
        let _guard = crate::sumeragi::status::rbc_status_test_guard();
        crate::sumeragi::status::clear_v2_status();
        let directory = TempDir::new().expect("temporary directory");
        {
            let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
            assert!(startup.is_empty());
            let proposal = proposal(
                &adapter.wire_context,
                adapter.wire_context.leader(0),
                subject(10),
            );
            let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
                unreachable!("proposal helper returns a proposal")
            };
            let (durable, validated) =
                validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
            let proposal_tag = adapter.current_tag();
            let sign = adapter
                .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
                .expect("persist proposal intent");
            assert!(matches!(
                sign.effects(),
                [AdapterEffect::Sign {
                    request: SignRequest::Proposal(_),
                    ..
                }]
            ));
        }

        crate::sumeragi::status::clear_v2_status();
        let context = context();
        let leader = context.leader(0);
        let (mut adapter, startup) = SumeragiV2Adapter::open_deferred_status(
            directory.path().join("leader-safety.wal"),
            verified_genesis(context),
            Some(leader),
            reducer::Generation::new(1),
            [0x22; 32],
            fingerprints(),
            deferred_admission_ordinals(),
        )
        .expect("replay leader without publishing status");
        assert!(matches!(
            startup.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            }]
        ));
        assert!(
            crate::sumeragi::status::v2_status().is_none(),
            "nonempty startup work must not publish the prepared successor"
        );
        let prepared = adapter
            .successor_activation_status()
            .expect("prepare reducer-owned activation snapshot");
        assert_eq!(prepared.height, 1);
        assert!(matches!(
            prepared.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                ..
            })
        ));
        assert!(
            crate::sumeragi::status::v2_status().is_none(),
            "snapshot construction must remain separate from publication"
        );
        crate::sumeragi::status::clear_v2_status();
    }

    include!("tests/v2_adapter_01_replay_and_registry.rs");
    #[test]
    #[allow(clippy::too_many_lines)]
    fn capacity_bypass_records_follow_current_lock_and_timeout_view() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        let install_lock = |adapter: &mut SumeragiV2Adapter, marker: u8| {
            let locked_subject = subject(marker);
            let locked_execution_commitment = execution_commitment(marker);
            let wire_round = wire::ConsensusRound {
                context_id: adapter.wire_context.id(),
                height: adapter.wire_context.height,
                view: 0,
            };
            let wire_prepare = wire::QuorumCertificate {
                round: wire_round,
                proposal_round: wire_round,
                phase: wire::GlobalPhase::Prepare,
                subject: locked_subject,
                execution_commitment: locked_execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![marker; 96],
            };
            let core_context = adapter.reducer.context().clone();
            let prepare = adapter
                .registry
                .qc_to_core(&wire_prepare, &adapter.wire_context)
                .expect("register lock certificate");
            let local_validator = adapter
                .registry
                .validator_id(0)
                .expect("local fixture validator");
            let vote = reducer::Vote::new(
                core_context.id(),
                prepare.round(),
                reducer::Phase::Commit,
                prepare.subject(),
                local_validator,
            );
            adapter.reducer = reducer::Reducer::recover(
                core_context,
                Some(local_validator),
                reducer::Generation::new(u64::from(marker)),
                [reducer::WalEntry::new(
                    reducer::PersistenceId::new(1),
                    reducer::WalRecord::LockAndCommit { prepare, vote },
                )],
            )
            .expect("recover durable lock fixture");
            (wire_round, locked_subject, locked_execution_commitment)
        };
        let admit_locked_roster =
            |adapter: &mut SumeragiV2Adapter,
             wire_round: wire::ConsensusRound,
             locked_subject: wire::BlockSubject,
             locked_execution_commitment: wire::ExecutionCommitment| {
                let roster_len = adapter.wire_context.roster.len();
                for signer in 0..roster_len {
                    let signer = u32::try_from(signer).expect("fixture signer index fits u32");
                    let payload = wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                        round: wire_round,
                        proposal_round: wire_round,
                        phase: wire::GlobalPhase::Commit,
                        subject: locked_subject,
                        execution_commitment: locked_execution_commitment,
                        signer,
                        signature: vec![u8::try_from(signer).expect("small fixture signer")],
                    });
                    let (outcome, admission) = adapter
                        .admit_authenticated_payload(&payload)
                        .expect("exact lock bypasses ordinary capacity");
                    assert!(outcome.is_none());
                    let admission = admission.expect("lock vote owns a capacity-bypass record");
                    assert!(
                        adapter
                            .ingress_equivocations
                            .get(&admission.key)
                            .expect("inserted lock admission")
                            .capacity_bypass
                    );
                    adapter.record_ingress_delivery(admission);
                }
            };
        let admit_timeout_roster =
            |adapter: &mut SumeragiV2Adapter, wire_round: wire::ConsensusRound| {
                let roster_len = adapter.wire_context.roster.len();
                for signer in 0..roster_len {
                    let signer = u32::try_from(signer).expect("fixture signer index fits u32");
                    let payload = wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                        round: wire_round,
                        highest_prepare_qc: None,
                        signer,
                        signature: vec![0xE0 ^ u8::try_from(signer).expect("small fixture signer")],
                    });
                    let (outcome, admission) = adapter
                        .admit_authenticated_payload(&payload)
                        .expect("current TimeoutVote bypasses ordinary capacity");
                    assert!(outcome.is_none());
                    let admission = admission.expect("TimeoutVote owns a capacity-bypass record");
                    assert!(
                        adapter
                            .ingress_equivocations
                            .get(&admission.key)
                            .expect("inserted TimeoutVote admission")
                            .capacity_bypass
                    );
                    adapter.record_ingress_delivery(admission);
                }
            };

        let first_lock = install_lock(&mut adapter, 0xDB);
        let ordinary_round = first_lock.0;
        for index in 0..MAX_INGRESS_SEMANTIC_KEYS {
            let proposer = u32::try_from(index).expect("semantic table bound fits u32");
            adapter.ingress_equivocations.insert(
                IngressSemanticKey::Proposal {
                    round: ordinary_round,
                    proposer,
                },
                IngressEquivocationRecord {
                    fingerprint: IngressFingerprint::Proposal(Hash::new(index.to_le_bytes())),
                    equivocation_reported: false,
                    capacity_bypass: false,
                    admitted_at: Instant::now(),
                },
            );
        }
        admit_locked_roster(&mut adapter, first_lock.0, first_lock.1, first_lock.2);
        let roster_len = adapter.wire_context.roster.len();
        admit_timeout_roster(&mut adapter, first_lock.0);
        assert_eq!(
            adapter.ingress_equivocations.len(),
            semantic_ingress_capacity(roster_len),
            "ordinary, exact-lock, and current TimeoutVote owners realize the complete live semantic bound"
        );
        let ingress = adapter
            .adapter_queue_statuses()
            .into_iter()
            .find(|queue| queue.queue == wire::SumeragiV2QueueKind::Ingress)
            .expect("ingress queue status");
        assert_eq!(
            usize::try_from(ingress.depth).unwrap(),
            semantic_ingress_capacity(roster_len)
        );
        assert_eq!(
            usize::try_from(ingress.capacity).unwrap(),
            semantic_ingress_capacity(roster_len)
        );
        assert_eq!(
            adapter
                .ingress_equivocations
                .values()
                .filter(|record| record.capacity_bypass)
                .count(),
            roster_len * 2
        );
        let same_view_equivocations = adapter.ingress_equivocations.clone();
        let same_view_deliveries = adapter.ingress_deliveries.clone();
        adapter.prune_ingress_records();
        assert_eq!(adapter.ingress_equivocations, same_view_equivocations);
        assert_eq!(adapter.ingress_deliveries, same_view_deliveries);

        // The following lock-replacement half isolates durable-lock retention;
        // view-advance retirement for these TimeoutVote owners is exercised by
        // `full_normal_deferred_lane_cannot_drop_absolute_timeout`.
        adapter
            .ingress_equivocations
            .retain(|key, _| !matches!(key, IngressSemanticKey::TimeoutVote { .. }));
        adapter
            .ingress_deliveries
            .retain(|key, _| !matches!(key, IngressSemanticKey::TimeoutVote { .. }));
        assert_eq!(
            adapter.ingress_equivocations.len(),
            MAX_INGRESS_SEMANTIC_KEYS + roster_len
        );

        let second_lock = install_lock(&mut adapter, 0xDC);
        adapter.prune_ingress_records();
        assert_eq!(
            adapter.ingress_equivocations.len(),
            MAX_INGRESS_SEMANTIC_KEYS
        );
        assert!(
            adapter
                .ingress_equivocations
                .values()
                .all(|record| !record.capacity_bypass)
        );
        assert!(adapter.ingress_deliveries.is_empty());

        admit_locked_roster(&mut adapter, second_lock.0, second_lock.1, second_lock.2);
        assert_eq!(
            adapter.ingress_equivocations.len(),
            MAX_INGRESS_SEMANTIC_KEYS + roster_len,
            "capacity-bypass records from successive locks cannot accumulate"
        );
        assert_eq!(
            adapter
                .ingress_equivocations
                .values()
                .filter(|record| record.capacity_bypass)
                .count(),
            roster_len
        );
        let ingress = adapter
            .adapter_queue_statuses()
            .into_iter()
            .find(|queue| queue.queue == wire::SumeragiV2QueueKind::Ingress)
            .expect("ingress queue status after lock advance");
        assert!(ingress.depth <= ingress.capacity);
    }

    include!("tests/v2_adapter_02_view_and_lock_progress.rs");
    #[test]
    fn prelock_current_commit_is_readmitted_with_priority_neutral_service_identity() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        let locked_subject = subject(0xBE);
        let locked_execution_commitment = execution_commitment(0xBE);
        let wire_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let remote_commit =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                round: wire_round,
                proposal_round: wire_round,
                phase: wire::GlobalPhase::Commit,
                subject: locked_subject,
                execution_commitment: locked_execution_commitment,
                signer: 1,
                signature: vec![0xBE],
            }));
        let authenticated = AuthenticatedConsensusMessage::for_test(remote_commit.clone());
        assert!(!adapter.authenticated_ingress_is_progress(&authenticated));
        let generation = adapter.current_tag().generation();
        let serviced_before = adapter.serviced_candidate_count_for_test();

        let premature = adapter
            .receive_authenticated(authenticated)
            .expect("deliver the current Commit before its Prepare lock is durable");
        assert_eq!(
            premature.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::IrrelevantView)
        );
        let key = IngressSemanticKey::Vote {
            round: wire_round,
            phase: wire::GlobalPhase::Commit,
            signer: 1,
        };
        let delivered = adapter
            .ingress_deliveries
            .get(&key)
            .expect("the pre-lock reducer delivery is recorded");
        assert_eq!(delivered.generation, generation);
        assert!(!delivered.locked_commit_progress);
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            serviced_before,
            "authenticated policy discards cannot allocate candidate markers"
        );
        assert!(adapter.durable_serviced_candidates.is_empty());
        let wire::ConsensusMessageV2Payload::Vote(normal_vote) = &remote_commit.payload else {
            unreachable!("fixture is a Commit vote")
        };
        let normal_vote = adapter
            .registry
            .vote_to_core(normal_vote, &adapter.wire_context)
            .expect("project the marker-free pre-lock occurrence");
        let normal_candidate = adapter
            .serviced_candidate(
                &reducer::Event::VoteReceived {
                    tag: adapter.current_tag(),
                    vote: normal_vote,
                },
                DeferredPriority::Normal,
                None,
                None,
            )
            .expect("pre-lock occurrence still has an exact rank identity")
            .0;
        adapter.ingress_deliveries.remove(&key);
        adapter.ingress_equivocations.remove(&key);
        let marker_count = adapter.serviced_candidate_count_for_test();
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                    remote_commit.clone(),
                ))
                .expect("marker-free policy discard remains reducer-idempotent")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::IrrelevantView)
        );
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            marker_count,
            "same-class policy replay cannot consume a tombstone"
        );

        let prepare = wire::QuorumCertificate {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Prepare,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xAE; 96],
        };
        let observed = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    prepare,
                )),
            ))
            .expect("observe the current PrepareQC");
        assert!(observed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::FetchBody {
                round,
                subject,
                certificate: Some(certificate),
                ..
            } if *round == wire_round
                && *subject == locked_subject
                && certificate.phase == wire::GlobalPhase::Prepare
        )));

        let manifest = wire::PayloadManifest::derive(
            &adapter.wire_context,
            wire_round,
            locked_subject,
            5,
            &[b"chunk".to_vec()],
        )
        .expect("derive the certified body manifest");
        let (durable, _) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
        let validated = ValidatedBodyReceipt::for_test_with_commitment(
            durable.clone(),
            locked_execution_commitment,
        );
        assert!(matches!(
            adapter
                .body_available(adapter.current_tag(), manifest.clone())
                .expect("make the certified body available")
                .effects(),
            [AdapterEffect::StoreBody { .. }]
        ));
        assert!(matches!(
            adapter
                .body_stored(adapter.current_tag(), wire_round, locked_subject, &durable,)
                .expect("acknowledge durable body storage")
                .effects(),
            [AdapterEffect::ValidateBody { .. }]
        ));
        let locked = adapter
            .validation_succeeded(
                adapter.current_tag(),
                wire_round,
                locked_subject,
                &validated,
            )
            .expect("persist the exact current LockAndCommit record");
        let commit_sign_tag = locked
            .effects()
            .iter()
            .find_map(|effect| match effect {
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(vote),
                } if vote.round == wire_round
                    && vote.phase == wire::GlobalPhase::Commit
                    && vote.subject == locked_subject =>
                {
                    Some(*tag)
                }
                _ => None,
            })
            .expect("durable lock acknowledgement authorizes the local Commit signature");
        assert_eq!(adapter.current_tag().generation(), generation);
        assert!(adapter.authenticated_ingress_is_progress(
            &AuthenticatedConsensusMessage::for_test(remote_commit.clone(),)
        ));

        let readmitted = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                remote_commit.clone(),
            ))
            .expect("re-admit the exact vote in the new lock consumer epoch");
        assert_eq!(
            readmitted.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_progress_inputs.len(), 1);
        assert!(adapter.deferred_progress_inputs[0].protected_progress);
        let progress_input = &adapter.deferred_progress_inputs[0];
        let progress_candidate = adapter
            .serviced_candidate(
                &progress_input.event,
                progress_input.priority,
                progress_input.completion_evidence.as_ref(),
                progress_input.authenticated_wire_identity.as_deref(),
            )
            .expect("exact-lock Commit has a route-neutral service identity")
            .0;
        assert_eq!(
            progress_candidate.class(),
            ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS
        );
        assert_eq!(
            normal_candidate, progress_candidate,
            "the same authenticated Commit occurrence must coalesce across Normal/Progress routing"
        );
        let delivered = adapter
            .ingress_deliveries
            .get(&key)
            .expect("the exact-lock consumer owns the re-admitted vote");
        assert_eq!(delivered.generation, generation);
        assert!(delivered.locked_commit_progress);
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                    remote_commit.clone(),
                ))
                .expect("coalesce behind exact deferred ownership")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );

        adapter
            .signature_completed(commit_sign_tag, vec![0xBF])
            .expect("self-admit the local Commit");
        assert_eq!(adapter.deferred_progress_inputs.len(), 1);
        adapter
            .drain_deferred()
            .expect("give the deferred remote owner its serialized runtime turn");
        assert!(adapter.deferred_progress_inputs.is_empty());
        assert!(
            adapter.serviced_candidates.contains_key(&normal_candidate),
            "the priority-neutral applied Commit remains coalesced for this process generation"
        );
        assert!(adapter.durable_serviced_candidates.is_empty());
        assert!(
            adapter
                .status()
                .expect("post-lock status")
                .liveness
                .commit_quorums
                .iter()
                .any(|quorum| quorum.round == wire_round
                    && quorum.subject == locked_subject
                    && quorum.execution_commitment == locked_execution_commitment
                    && quorum.signer_count == 2)
        );
        adapter.ingress_deliveries.remove(&key);
        adapter.ingress_equivocations.remove(&key);
        let marker_count = adapter.serviced_candidate_count_for_test();
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(remote_commit))
                .expect("monotone Commit-vote state suppresses replay after ingress reset")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), marker_count);
    }

    include!("tests/v2_adapter_03_tc_and_terminal_ingress.rs");
    #[test]
    fn deferred_zero_ordinal_is_exact_single_use_and_never_reminted() {
        let source = DeferredAdmissionOrdinalSource::new(0);
        let tag = reducer::EventTag::new(1, 0, reducer::Generation::new(1));
        let first = DeferredServiceEvidence::completion_for_test(
            &source,
            tag,
            1,
            DeferredPriority::Completion,
        );
        let second = DeferredServiceEvidence::completion_for_test(
            &source,
            tag,
            1,
            DeferredPriority::Completion,
        );

        assert_eq!(first.admission_ordinal, 0);
        assert_eq!(second.admission_ordinal, 1);
        assert!(first.validate_exact());
        assert!(first.belongs_to(&source));
        assert!(first.claim_adapter_service_for_test());
        assert!(!first.claim_adapter_service_for_test());
        assert!(first.claim_runtime_handoff_once());
        assert!(!first.claim_runtime_handoff_once());
        assert!(first.service_handoff_is_complete());
        assert!(second.claim_adapter_service_for_test());
        assert!(second.claim_runtime_handoff_once());
    }

    #[test]
    fn deferred_projection_distinguishes_authenticated_proposal_origins() {
        let context_id = reducer::ContextId::repeat(0xA1);
        let finality_round = reducer::Round::new(7, 4);
        let origin_a = reducer::Round::new(7, 1);
        let origin_b = reducer::Round::new(7, 2);
        let subject = reducer::Subject::repeat(0xA2);
        let signer = reducer::ValidatorId::repeat(0xA3);
        let tag = reducer::EventTag::new(7, 4, reducer::Generation::new(1));
        let signature = reducer::OpaqueSignature::new(vec![0xA4]);
        let project = |event: reducer::Event| {
            let mut projection = Vec::new();
            append_deferred_projection_event(&mut projection, &event);
            projection
        };

        let signed_vote = |proposal_round| {
            reducer::SignedVote::new(
                reducer::Vote::new_with_proposal_round(
                    context_id,
                    finality_round,
                    proposal_round,
                    reducer::Phase::Commit,
                    subject,
                    signer,
                ),
                signature.clone(),
            )
        };
        assert_ne!(
            project(reducer::Event::VoteReceived {
                tag,
                vote: signed_vote(origin_a),
            }),
            project(reducer::Event::VoteReceived {
                tag,
                vote: signed_vote(origin_b),
            })
        );

        let certificate = |proposal_round| {
            reducer::QuorumCertificate::new(
                reducer::CertificateRef::new_with_proposal_round(
                    context_id,
                    finality_round,
                    proposal_round,
                    reducer::Phase::Commit,
                    subject,
                ),
                vec![reducer::SignatureShare::new(signer, signature.clone())],
            )
        };
        assert_ne!(
            project(reducer::Event::QuorumCertificateReceived {
                tag,
                certificate: certificate(origin_a),
            }),
            project(reducer::Event::QuorumCertificateReceived {
                tag,
                certificate: certificate(origin_b),
            })
        );

        let proposal = |proposal_round| {
            reducer::SignedProposal::new(
                reducer::Proposal::new(
                    context_id,
                    reducer::Round::new(7, 0),
                    signer,
                    reducer::PayloadManifest::new(
                        subject,
                        reducer::Digest::repeat(0xA5),
                        reducer::Digest::repeat(0xA6),
                        1,
                        1,
                    ),
                    reducer::ProposalJustification::ParentCommit(Some(
                        reducer::CertificateRef::new_with_proposal_round(
                            context_id,
                            finality_round,
                            proposal_round,
                            reducer::Phase::Commit,
                            subject,
                        ),
                    )),
                ),
                signature.clone(),
            )
        };
        assert_ne!(
            project(reducer::Event::ProposalReceived {
                tag,
                proposal: proposal(origin_a),
            }),
            project(reducer::Event::ProposalReceived {
                tag,
                proposal: proposal(origin_b),
            })
        );
    }

    #[test]
    fn deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        adapter.deferred_admission_ordinals = DeferredAdmissionOrdinalSource::new(u128::MAX - 1);
        let context = adapter.wire_context.clone();
        let proposer = context.leader(0);
        let first = proposal(&context, proposer, subject(0xD1));
        let second = proposal(&context, proposer, subject(0xD2));
        let wire::ConsensusMessageV2Payload::Proposal(first) = first.payload else {
            unreachable!("proposal fixture")
        };
        let wire::ConsensusMessageV2Payload::Proposal(second) = second.payload else {
            unreachable!("proposal fixture")
        };
        let tag = adapter.current_tag();

        adapter
            .defer_body_available_for_test(tag, &first.manifest)
            .expect("last safely advanceable ordinal is admitted");
        assert_eq!(
            adapter
                .deferred_completions
                .front()
                .expect("first owner remains queued")
                .admission_ordinal,
            u128::MAX - 1
        );
        assert!(matches!(
            adapter.defer_body_available_for_test(tag, &second.manifest),
            Err(AdapterError::DeferredAdmissionOrdinalExhausted)
        ));
        assert!(adapter.fail_closed);
        assert_eq!(adapter.deferred_completions.len(), 1);
        assert_eq!(
            adapter.deferred_admission_ordinals.next_for_test(),
            u128::MAX,
            "exhaustion cannot wrap the actor source to a stale ordinal"
        );
    }

    #[test]
    fn deferred_actor_source_never_aliases_across_adapter_instances() {
        let first_directory = TempDir::new().expect("first temporary directory");
        let second_directory = TempDir::new().expect("second temporary directory");
        let source = DeferredAdmissionOrdinalSource::new(0);
        let open = |directory: &TempDir, source: DeferredAdmissionOrdinalSource| {
            SumeragiV2Adapter::open_with_aggregator(
                directory.path().join("shared-ordinal-safety.wal"),
                verified_genesis(context()),
                Some(0),
                reducer::Generation::new(1),
                [0xD3; 32],
                fingerprints(),
                Box::new(TestAggregator),
                source,
            )
        };
        let (mut first, first_startup) =
            open(&first_directory, source.clone()).expect("open first adapter instance");
        let (mut second, second_startup) =
            open(&second_directory, source.clone()).expect("open second adapter instance");
        assert!(first_startup.is_empty());
        assert!(second_startup.is_empty());
        let first_context = first.wire_context.clone();
        let second_context = second.wire_context.clone();
        let wire::ConsensusMessageV2Payload::Proposal(first_proposal) =
            proposal(&first_context, first_context.leader(0), subject(0xD4)).payload
        else {
            unreachable!("proposal fixture")
        };
        let wire::ConsensusMessageV2Payload::Proposal(second_proposal) =
            proposal(&second_context, second_context.leader(0), subject(0xD5)).payload
        else {
            unreachable!("proposal fixture")
        };
        let first_tag = first.current_tag();
        let second_tag = second.current_tag();
        first
            .defer_body_available_for_test(first_tag, &first_proposal.manifest)
            .expect("first adapter instance admits owner zero");
        second
            .defer_body_available_for_test(second_tag, &second_proposal.manifest)
            .expect("second adapter instance advances the same actor source");

        let first_owner = first
            .pop_deferred_next()
            .expect("first adapter instance rank remains valid")
            .expect("first adapter instance returns exact owner")
            .evidence;
        let second_owner = second
            .pop_deferred_next()
            .expect("second adapter instance rank remains valid")
            .expect("second adapter instance returns exact owner")
            .evidence;
        assert_eq!(first_owner.admission_ordinal, 0);
        assert_eq!(second_owner.admission_ordinal, 1);
        assert_ne!(
            first_owner.admission_ordinal,
            second_owner.admission_ordinal
        );
        assert!(first_owner.belongs_to(&source));
        assert!(second_owner.belongs_to(&source));
        assert!(first_owner.validate_exact());
        assert!(second_owner.validate_exact());
    }

    #[test]
    fn deferred_service_evidence_rejects_every_owner_and_rank_mutation() {
        let source = DeferredAdmissionOrdinalSource::new(0);
        let foreign = DeferredAdmissionOrdinalSource::new(0);
        let tag = reducer::EventTag::new(7, 2, reducer::Generation::new(3));
        let evidence =
            DeferredServiceEvidence::completion_for_test(&source, tag, 1, DeferredPriority::Normal);
        assert!(evidence.validate_exact());
        assert!(evidence.belongs_to(&source));
        assert!(!evidence.belongs_to(&foreign));

        let rejected = |mutated: DeferredServiceEvidence| {
            assert!(!mutated.validate_exact());
        };

        let mut mutated = evidence.clone();
        mutated.admission_ordinal = 1;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.priority = DeferredPriority::Progress;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.event_kind = DeferredEventKind::RetransmitElapsed;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.original_tag = reducer::EventTag::new(7, 3, reducer::Generation::new(3));
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.original_event = reducer::Event::RetransmitElapsed { tag };
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.protected_progress = true;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.eligible_skips_after = 1;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.service_cursor_after = DeferredPriority::Normal;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.service_cursor_before = DeferredPriority::Completion;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.queue_lengths_after.completion = 1;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.total_len_before = 2;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.retag = DeferredRetagRelation::AuthenticatedIngress { from: tag, to: tag };
        rejected(mutated);

        let mut mutated = evidence;
        mutated.projection_hash = Hash::new(b"wrong deferred projection");
        rejected(mutated);
    }

    #[test]
    fn deferred_authenticated_retry_retains_exact_original_and_effective_tags() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let context = adapter.wire_context.clone();
        let wire::ConsensusMessageV2Payload::Proposal(proposal) =
            proposal(&context, context.leader(0), subject(0xD6)).payload
        else {
            unreachable!("proposal fixture")
        };
        let authenticated_wire_identity = Arc::<[u8]>::from(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            ))
            .encode(),
        );
        let proposal = adapter
            .registry
            .proposal_to_core(&proposal, &context)
            .expect("convert authenticated proposal");
        let effective_tag = adapter.current_tag();
        let original_tag = reducer::EventTag::new(
            effective_tag.height(),
            effective_tag.view().saturating_add(1),
            effective_tag.generation(),
        );
        let admission_capability = adapter
            .mint_deferred_admission_ordinal()
            .expect("mint exact deferred owner");
        adapter.deferred_inputs.push_back(DeferredInput {
            admission_ordinal: admission_capability.ordinal,
            admission_capability,
            event: reducer::Event::ProposalReceived {
                tag: original_tag,
                proposal,
            },
            completion_evidence: None,
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Normal,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: Some(authenticated_wire_identity),
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
        adapter.next_deferred_priority = DeferredPriority::Normal;

        let selection = adapter
            .pop_deferred_next()
            .expect("authenticated retry rank remains valid")
            .expect("select exact authenticated retry");
        assert!(selection.evidence.validate_exact());
        assert_eq!(selection.evidence.original_tag, original_tag);
        assert_eq!(selection.evidence.effective_tag, effective_tag);
        assert_eq!(
            selection.evidence.retag,
            DeferredRetagRelation::AuthenticatedIngress {
                from: original_tag,
                to: effective_tag,
            }
        );
        assert!(
            selection
                .evidence
                .matches_effective_event(&selection.input.event)
        );
        assert!(adapter.deferred_authenticated_event_matches_wire(&selection.evidence));
    }

    #[test]
    fn authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let context = adapter.wire_context.clone();
        let wire::ConsensusMessageV2Payload::Proposal(first) =
            proposal(&context, context.leader(0), subject(0xD7)).payload
        else {
            unreachable!("proposal fixture")
        };
        let wire::ConsensusMessageV2Payload::Proposal(other) =
            proposal(&context, context.leader(0), subject(0xD8)).payload
        else {
            unreachable!("proposal fixture")
        };
        let first = adapter
            .registry
            .proposal_to_core(&first, &context)
            .expect("convert retained authenticated proposal");
        let event = reducer::Event::ProposalReceived {
            tag: adapter.current_tag(),
            proposal: first,
        };

        assert!(matches!(
            adapter.enqueue_deferred(
                event.clone(),
                true,
                DeferredPriority::Normal,
                None,
                None,
                None,
            ),
            Err(AdapterError::RuntimeIngressOwnershipViolation)
        ));
        assert!(adapter.deferred_inputs.is_empty());

        let admission_capability = adapter
            .mint_deferred_admission_ordinal()
            .expect("mint exact deferred owner");
        adapter.deferred_inputs.push_back(DeferredInput {
            admission_ordinal: admission_capability.ordinal,
            admission_capability,
            event,
            completion_evidence: None,
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Normal,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: Some(authenticated_wire_identity(
                wire::ConsensusMessageV2Payload::Proposal(other),
            )),
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });
        adapter.next_deferred_priority = DeferredPriority::Normal;
        let durable_before = adapter.reducer.durable_state().clone();

        assert!(matches!(
            adapter.drain_deferred_with_evidence(),
            Err(AdapterError::DeferredServiceOwnershipViolation)
        ));
        assert_eq!(adapter.reducer.durable_state(), &durable_before);
        assert!(adapter.fail_closed);
    }

    #[test]
    fn deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let current = adapter.current_tag();
        let stale = reducer::EventTag::new(
            current.height().saturating_add(1),
            current.view(),
            current.generation(),
        );
        let capability = adapter
            .mint_deferred_admission_ordinal()
            .expect("mint exact adapter capability");
        let input = |tag| DeferredInput {
            admission_ordinal: capability.ordinal,
            admission_capability: capability.clone(),
            event: reducer::Event::TimeoutElapsed { tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority: DeferredPriority::Completion,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        };
        adapter.deferred_completions.push_back(input(stale));
        adapter
            .deferred_completions
            .push_back(input(reducer::EventTag::new(
                stale.height().saturating_add(1),
                stale.view(),
                stale.generation(),
            )));

        let (_, first) = adapter
            .drain_deferred_with_evidence()
            .expect("first exact capability crosses the adapter")
            .expect("first deferred owner is serviceable");
        assert!(first.adapter_service_is_claimed());
        assert!(!first.service_handoff_is_complete());
        assert_eq!(
            adapter
                .ignore_counts
                .get(&reducer::IgnoreReason::WrongHeight)
                .copied(),
            Some(1)
        );
        assert!(matches!(
            adapter.drain_deferred_with_evidence(),
            Err(AdapterError::DeferredServiceOwnershipViolation)
        ));
        assert_eq!(
            adapter
                .ignore_counts
                .get(&reducer::IgnoreReason::WrongHeight)
                .copied(),
            Some(1),
            "the replay is rejected before a second reducer transition"
        );

        let foreign_directory = TempDir::new().expect("foreign temporary directory");
        let (mut foreign_adapter, foreign_startup) =
            open_test(&foreign_directory).expect("open foreign adapter");
        assert!(foreign_startup.is_empty());
        let foreign_source = DeferredAdmissionOrdinalSource::new(0);
        let foreign_capability = foreign_source.mint().expect("mint foreign capability");
        let foreign_tag = reducer::EventTag::new(
            foreign_adapter.current_tag().height().saturating_add(1),
            foreign_adapter.current_tag().view(),
            foreign_adapter.current_tag().generation(),
        );
        foreign_adapter
            .deferred_completions
            .push_back(DeferredInput {
                admission_ordinal: foreign_capability.ordinal,
                admission_capability: foreign_capability,
                event: reducer::Event::TimeoutElapsed { tag: foreign_tag },
                completion_evidence: None,
                retag_authenticated_ingress: false,
                priority: DeferredPriority::Completion,
                protected_progress: false,
                admission: None,
                authenticated_wire_identity: None,
                admitted_at: Instant::now(),
                eligible_skips: 0,
            });
        assert!(matches!(
            foreign_adapter.drain_deferred_with_evidence(),
            Err(AdapterError::DeferredServiceOwnershipViolation)
        ));
        assert!(foreign_adapter.ignore_counts.is_empty());
    }

    #[test]
    fn deferred_service_debt_counts_only_oldest_skipped_classes() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let input = |priority: DeferredPriority| DeferredInput {
            admission_ordinal: priority.code().into(),
            admission_capability: DeferredAdmissionCapability::for_test(priority.code().into()),
            event: reducer::Event::TimeoutElapsed { tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        };
        adapter
            .deferred_completions
            .push_back(input(DeferredPriority::Completion));
        adapter
            .deferred_completions
            .push_back(input(DeferredPriority::Completion));
        adapter
            .deferred_progress_inputs
            .push_back(input(DeferredPriority::Progress));
        adapter
            .deferred_progress_inputs
            .push_back(input(DeferredPriority::Progress));
        adapter
            .deferred_inputs
            .push_back(input(DeferredPriority::Normal));
        adapter
            .deferred_inputs
            .push_back(input(DeferredPriority::Normal));
        adapter.next_deferred_priority = DeferredPriority::Completion;

        let selected = adapter
            .pop_deferred_next()
            .expect("deferred service debt remains representable")
            .expect("completion receives its turn");
        assert_eq!(selected.evidence.priority, DeferredPriority::Completion);
        assert!(selected.evidence.validate_exact());
        assert_eq!(adapter.deferred_completions[0].eligible_skips, 0);
        assert_eq!(adapter.deferred_progress_inputs[0].eligible_skips, 1);
        assert_eq!(adapter.deferred_progress_inputs[1].eligible_skips, 0);
        assert_eq!(adapter.deferred_inputs[0].eligible_skips, 1);
        assert_eq!(adapter.deferred_inputs[1].eligible_skips, 0);
    }

    #[test]
    fn deferred_selector_services_only_the_runtime_lifecycle_minimum_set() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let input = |ordinal, priority| DeferredInput {
            admission_ordinal: ordinal,
            admission_capability: DeferredAdmissionCapability::for_test(ordinal),
            event: reducer::Event::TimeoutElapsed { tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        };
        adapter
            .deferred_completions
            .push_back(input(10, DeferredPriority::Completion));
        adapter
            .deferred_inputs
            .push_back(input(11, DeferredPriority::Normal));
        adapter
            .deferred_inputs
            .push_back(input(1, DeferredPriority::Normal));
        adapter.next_deferred_priority = DeferredPriority::Completion;

        let selection = adapter
            .pop_deferred_next_eligible(&BTreeSet::from([1]))
            .expect("lifecycle-filtered deferred selection remains exact")
            .expect("the runtime-minimal deferred owner is present");
        assert_eq!(selection.evidence.admission_ordinal, 1);
        assert_eq!(selection.evidence.priority, DeferredPriority::Normal);
        assert_eq!(adapter.deferred_completions[0].admission_ordinal, 10);
        assert_eq!(adapter.deferred_inputs[0].admission_ordinal, 11);
        assert_eq!(adapter.deferred_completions[0].eligible_skips, 0);
        assert_eq!(adapter.deferred_inputs[0].eligible_skips, 0);
    }

    #[test]
    fn deferred_service_debt_overflow_is_typed_and_fail_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let input = |ordinal, priority, eligible_skips| DeferredInput {
            admission_ordinal: ordinal,
            admission_capability: DeferredAdmissionCapability::for_test(ordinal),
            event: reducer::Event::TimeoutElapsed { tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips,
        };
        adapter
            .deferred_completions
            .push_back(input(1, DeferredPriority::Completion, 0));
        adapter
            .deferred_progress_inputs
            .push_back(input(2, DeferredPriority::Progress, u64::MAX));
        adapter.next_deferred_priority = DeferredPriority::Completion;

        assert!(matches!(
            adapter.pop_deferred_next(),
            Err(AdapterError::DeferredServiceDebtOverflow)
        ));
        assert!(adapter.fail_closed);
    }

    #[test]
    fn deferred_service_cursor_cycles_nonempty_classes() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let input = |priority: DeferredPriority| DeferredInput {
            admission_ordinal: priority.code().into(),
            admission_capability: DeferredAdmissionCapability::for_test(priority.code().into()),
            event: reducer::Event::TimeoutElapsed { tag },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        };
        for priority in [
            DeferredPriority::Completion,
            DeferredPriority::Progress,
            DeferredPriority::Normal,
        ] {
            let queue = match priority {
                DeferredPriority::Completion => &mut adapter.deferred_completions,
                DeferredPriority::Progress => &mut adapter.deferred_progress_inputs,
                DeferredPriority::Normal => &mut adapter.deferred_inputs,
            };
            queue.push_back(input(priority));
            queue.push_back(input(priority));
        }
        adapter.next_deferred_priority = DeferredPriority::Completion;

        let selected = (0..6)
            .map(|_| {
                let selection = adapter
                    .pop_deferred_next()
                    .expect("deferred service debt remains representable")
                    .expect("every nonempty class receives both turns");
                assert!(selection.evidence.validate_exact());
                selection.evidence.priority
            })
            .collect::<Vec<_>>();
        assert_eq!(
            selected,
            vec![
                DeferredPriority::Completion,
                DeferredPriority::Progress,
                DeferredPriority::Normal,
                DeferredPriority::Completion,
                DeferredPriority::Progress,
                DeferredPriority::Normal,
            ]
        );
        assert!(
            adapter
                .pop_deferred_next()
                .expect("empty rank remains valid")
                .is_none()
        );
    }

    #[test]
    fn deferred_dispatch_decreases_rank_by_exactly_one_macro_step_per_turn() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let current = adapter.current_tag();
        let stale = reducer::EventTag::new(
            current.height().saturating_add(1),
            current.view(),
            current.generation(),
        );
        let input = |priority: DeferredPriority| DeferredInput {
            admission_ordinal: priority.code().into(),
            admission_capability: DeferredAdmissionCapability::for_test(priority.code().into()),
            event: reducer::Event::TimeoutElapsed { tag: stale },
            completion_evidence: None,
            retag_authenticated_ingress: false,
            priority,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: None,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        };
        adapter
            .deferred_completions
            .push_back(input(DeferredPriority::Completion));
        adapter
            .deferred_progress_inputs
            .push_back(input(DeferredPriority::Progress));
        adapter
            .deferred_inputs
            .push_back(input(DeferredPriority::Normal));
        adapter.next_deferred_priority = DeferredPriority::Completion;

        for (turn, expected_lengths) in [
            (DeferredPriority::Completion, [0, 1, 1]),
            (DeferredPriority::Progress, [0, 0, 1]),
            (DeferredPriority::Normal, [0, 0, 0]),
        ] {
            assert!(adapter.deferred_work_is_serviceable());
            let before = adapter.deferred_completions.len()
                + adapter.deferred_progress_inputs.len()
                + adapter.deferred_inputs.len();
            assert!(
                adapter
                    .drain_deferred()
                    .expect("service one stale deferred transition")
                    .is_empty()
            );
            let after = adapter.deferred_completions.len()
                + adapter.deferred_progress_inputs.len()
                + adapter.deferred_inputs.len();
            assert_eq!(before - after, 1, "{turn:?} owns exactly one turn");
            assert_eq!(
                [
                    adapter.deferred_completions.len(),
                    adapter.deferred_progress_inputs.len(),
                    adapter.deferred_inputs.len(),
                ],
                expected_lengths,
                "the round-robin cursor selected {turn:?}"
            );
        }
        assert!(!adapter.deferred_work_is_serviceable());
    }

    #[test]
    fn deferred_service_contract_violation_is_terminal() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        assert!(matches!(
            adapter.fail_deferred_service_contract(),
            AdapterError::DeferredServiceContractViolation
        ));
        assert!(adapter.fail_closed);
        assert!(matches!(
            adapter.drain_deferred(),
            Err(AdapterError::FailClosed)
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn unowned_busy_certificates_roll_back_staged_registry_and_active_subject() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let timeout_sign = adapter
            .timeout_elapsed(tag)
            .expect("start a local timeout signature fence");
        assert!(matches!(
            timeout_sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));

        let wire_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let qc = |phase, marker| wire::QuorumCertificate {
            round: wire_round,
            proposal_round: wire_round,
            phase,
            subject: subject(marker),
            execution_commitment: execution_commitment(marker),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![marker; 96],
        };
        let deferred_qcs = [
            qc(wire::GlobalPhase::Prepare, 0xE0),
            qc(wire::GlobalPhase::Commit, 0xE1),
        ];
        for (ordinal, certificate) in deferred_qcs.into_iter().enumerate() {
            let certificate_wire_identity = authenticated_wire_identity(
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone()),
            );
            let certificate = adapter
                .registry
                .qc_to_core(&certificate, &adapter.wire_context)
                .expect("convert certificate lane fixture");
            adapter.deferred_progress_inputs.push_back(DeferredInput {
                admission_ordinal: u128::try_from(ordinal)
                    .expect("bounded fixture ordinal fits u128")
                    .saturating_add(1),
                admission_capability: DeferredAdmissionCapability::for_test(
                    u128::try_from(ordinal)
                        .expect("bounded fixture ordinal fits u128")
                        .saturating_add(1),
                ),
                event: reducer::Event::QuorumCertificateReceived { tag, certificate },
                completion_evidence: None,
                retag_authenticated_ingress: true,
                priority: DeferredPriority::Progress,
                protected_progress: false,
                admission: None,
                authenticated_wire_identity: Some(certificate_wire_identity),
                admitted_at: Instant::now(),
                eligible_skips: 0,
            });
        }
        let deferred_timeout = wire::TimeoutCertificate {
            round: wire_round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xE2; 96],
            }],
        };
        let deferred_timeout_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(deferred_timeout.clone()),
        );
        let deferred_timeout = adapter
            .registry
            .tc_to_core(&deferred_timeout, &adapter.wire_context)
            .expect("convert timeout-certificate lane fixture");
        adapter.deferred_progress_inputs.push_back(DeferredInput {
            admission_ordinal: 4,
            admission_capability: DeferredAdmissionCapability::for_test(4),
            event: reducer::Event::TimeoutCertificateReceived {
                tag,
                certificate: deferred_timeout,
            },
            completion_evidence: None,
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Progress,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: Some(deferred_timeout_wire_identity),
            admitted_at: Instant::now(),
            eligible_skips: 0,
        });

        let registry_before = adapter.registry.clone();
        let active_subject_before = adapter.active_subject;
        let deferred_before = adapter.deferred_progress_inputs.clone();
        for certificate in [
            qc(wire::GlobalPhase::Prepare, 0xE3),
            qc(wire::GlobalPhase::Commit, 0xE4),
        ] {
            let outcome = adapter
                .receive_verified(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
                ))
                .expect("apply certificate-class backpressure");
            assert_eq!(
                outcome.disposition(),
                reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
            );
            assert_eq!(adapter.deferred_progress_inputs, deferred_before);
            assert_registry_eq(&adapter.registry, &registry_before);
            assert_eq!(adapter.active_subject, active_subject_before);
        }

        let timeout_with_new_high_qc = wire::TimeoutCertificate {
            round: wire_round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(qc(wire::GlobalPhase::Prepare, 0xE5)),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xE5; 96],
            }],
        };
        let outcome = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_with_new_high_qc),
            ))
            .expect("apply timeout-certificate-class backpressure");
        assert_eq!(
            outcome.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_progress_inputs, deferred_before);
        assert_registry_eq(&adapter.registry, &registry_before);
        assert_eq!(adapter.active_subject, active_subject_before);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn unowned_busy_exact_locked_vote_rolls_back_and_remains_retryable() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        let locked_subject = subject(0xE6);
        let locked_execution_commitment = execution_commitment(0xE6);
        let wire_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let wire_prepare = wire::QuorumCertificate {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Prepare,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xE6; 96],
        };
        let core_context = adapter.reducer.context().clone();
        let prepare = adapter
            .registry
            .qc_to_core(&wire_prepare, &adapter.wire_context)
            .expect("register the durable PrepareQC");
        let round = prepare.round();
        let core_subject = prepare.subject();
        let local_validator = adapter
            .registry
            .validator_id(0)
            .expect("local fixture validator");
        let lock_entry = reducer::WalEntry::new(
            reducer::PersistenceId::new(1),
            reducer::WalRecord::LockAndCommit {
                prepare,
                vote: reducer::Vote::new(
                    core_context.id(),
                    round,
                    reducer::Phase::Commit,
                    core_subject,
                    local_validator,
                ),
            },
        );
        let encoded = adapter
            .registry
            .encode_wal_entry(&lock_entry, &TestAggregator)
            .expect("encode the durable lock");
        assert_eq!(
            adapter
                .wal
                .append(&encoded)
                .expect("append the durable lock"),
            0
        );
        adapter.reducer = reducer::Reducer::recover(
            core_context,
            Some(local_validator),
            reducer::Generation::new(1),
            [lock_entry],
        )
        .expect("recover the durable locked Commit intent");
        let replay_tag = adapter.reducer.current_tag();
        let replay = adapter
            .reducer
            .step(reducer::Event::ResumeAfterReplay { tag: replay_tag })
            .expect("resume the durable Commit intent");
        assert!(matches!(
            replay.effects(),
            [reducer::Effect::Sign {
                message: reducer::SignableMessage::Vote(vote),
                ..
            }] if vote.phase() == reducer::Phase::Commit
        ));

        let roster_len = adapter.wire_context.roster.len();
        let mut fillers = VecDeque::with_capacity(roster_len);
        for signer in 0..roster_len {
            let signer = u32::try_from(signer).expect("fixture signer fits u32");
            let wire_filler_vote = wire::Vote {
                round: wire_round,
                proposal_round: wire_round,
                phase: wire::GlobalPhase::Commit,
                subject: locked_subject,
                execution_commitment: locked_execution_commitment,
                signer,
                signature: vec![0xE7 ^ u8::try_from(signer).expect("fixture signer fits u8")],
            };
            let filler_wire_identity = authenticated_wire_identity(
                wire::ConsensusMessageV2Payload::Vote(wire_filler_vote.clone()),
            );
            let filler_vote = adapter
                .registry
                .vote_to_core(&wire_filler_vote, &adapter.wire_context)
                .expect("convert locked-vote capacity fixture");
            fillers.push_back(DeferredInput {
                admission_ordinal: u128::from(signer).saturating_add(1),
                admission_capability: DeferredAdmissionCapability::for_test(
                    u128::from(signer).saturating_add(1),
                ),
                event: reducer::Event::VoteReceived {
                    tag: replay_tag,
                    vote: filler_vote,
                },
                completion_evidence: None,
                retag_authenticated_ingress: true,
                priority: DeferredPriority::Progress,
                protected_progress: true,
                admission: None,
                authenticated_wire_identity: Some(filler_wire_identity),
                admitted_at: Instant::now(),
                eligible_skips: 0,
            });
        }
        adapter.deferred_progress_inputs = fillers;
        let retried_signer = u32::try_from(
            roster_len
                .checked_sub(1)
                .expect("fixture roster is non-empty"),
        )
        .expect("fixture signer fits u32");

        let locked_vote =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                round: wire_round,
                proposal_round: wire_round,
                phase: wire::GlobalPhase::Commit,
                subject: locked_subject,
                execution_commitment: locked_execution_commitment,
                signer: retried_signer,
                signature: vec![0xE8],
            }));
        let key = IngressSemanticKey::Vote {
            round: wire_round,
            phase: wire::GlobalPhase::Commit,
            signer: retried_signer,
        };
        let registry_before = adapter.registry.clone();
        let active_subject_before = adapter.active_subject;
        let deferred_before = adapter.deferred_progress_inputs.clone();
        let backpressured = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote.clone()))
            .expect("apply locked-vote-class backpressure");
        assert_eq!(
            backpressured.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert!(
            backpressured.requires_runtime_retry(),
            "a full lane retains no adapter owner and must re-expose the exact runtime command"
        );
        assert_eq!(adapter.deferred_progress_inputs, deferred_before);
        assert_registry_eq(&adapter.registry, &registry_before);
        assert_eq!(adapter.active_subject, active_subject_before);
        assert!(adapter.ingress_equivocations.contains_key(&key));
        assert!(
            !adapter.ingress_deliveries.contains_key(&key),
            "admission without locked-vote queue ownership must remain retryable"
        );

        adapter.deferred_progress_inputs.pop_back();
        let retried = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(locked_vote))
            .expect("retry after locked-vote ownership becomes available");
        assert_eq!(
            retried.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert!(!retried.requires_runtime_retry());
        assert_eq!(
            adapter.deferred_progress_inputs.len(),
            adapter.wire_context.roster.len()
        );
        assert!(adapter.ingress_deliveries.contains_key(&key));
        assert!(matches!(
            adapter.deferred_progress_inputs.back(),
            Some(DeferredInput {
                event: reducer::Event::VoteReceived { .. },
                admission: Some(_),
                protected_progress: true,
                ..
            })
        ));
    }

    #[test]
    fn deferred_progress_capacity_matches_partition_geometry() {
        assert_eq!(deferred_progress_capacity(0), 3);
        assert_eq!(deferred_progress_capacity(1), 5);
        assert_eq!(deferred_progress_capacity(4), 11);
        assert_eq!(
            deferred_progress_capacity(wire::MAX_VALIDATORS_PER_HEIGHT),
            MAX_DEFERRED_PROGRESS_INPUTS
        );
        assert_eq!(
            deferred_progress_capacity(wire::MAX_VALIDATORS_PER_HEIGHT.saturating_add(1)),
            MAX_DEFERRED_PROGRESS_INPUTS,
            "invalid oversized rosters cannot expand the static adapter bound"
        );
        assert_eq!(semantic_ingress_capacity(0), MAX_INGRESS_SEMANTIC_KEYS);
        assert_eq!(semantic_ingress_capacity(4), MAX_INGRESS_SEMANTIC_KEYS + 8);
        assert_eq!(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE, 11);
        assert_eq!(
            BTreeSet::from(ServicedCandidateStage::ALL.map(|stage| stage as u8)).len(),
            SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
            "the closed adapter-event projection has eleven distinct classes"
        );
        assert_eq!(
            serviced_candidate_capacity(4),
            (MAX_INGRESS_SEMANTIC_KEYS
                + 8
                + MAX_DEFERRED_INPUTS * 2
                + 11
                + MAX_DEFERRED_INPUTS * 4
                + MAX_DEFERRED_INPUTS
                + CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY
                + 1)
                * SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
            "serviced identities cover active causal/effect/clock owners as well as service queues"
        );
        for roster_len in [0, 1, 4, wire::MAX_VALIDATORS_PER_HEIGHT] {
            assert_eq!(
                serviced_candidate_capacity(roster_len),
                candidate_lifecycle_capacity(
                    roster_len,
                    DEFAULT_SERVICED_CANDIDATE_CAPACITY_GEOMETRY,
                )
                .saturating_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE),
                "the bound is the complete reviewed lifecycle geometry times the exact stage \
                 carrier for roster size {roster_len}"
            );
        }
        let configured = ServicedCandidateCapacityGeometry::new(4_096, 777);
        assert_eq!(
            candidate_lifecycle_capacity(4, configured),
            semantic_ingress_capacity(4)
                + MAX_DEFERRED_INPUTS * 2
                + deferred_progress_capacity(4)
                + 4_096 * 4
                + 777
                + CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY
                + 1,
            "runtime and effect ownership are derived from the supplied production configuration"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn deferred_progress_partition_owns_every_vote_and_certificate_class() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let roster_len = adapter.wire_context.roster.len();
        let tag = adapter.current_tag();
        let wire_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };

        for signer in 0..roster_len {
            let signer = u32::try_from(signer).expect("fixture signer fits u32");
            let marker = u8::try_from(signer).expect("fixture signer fits u8") | 0xA0;
            let locked_subject = subject(marker);
            let locked_commitment = execution_commitment(marker);
            let wire_vote = wire::Vote {
                round: wire_round,
                proposal_round: wire_round,
                phase: wire::GlobalPhase::Commit,
                subject: locked_subject,
                execution_commitment: locked_commitment,
                signer,
                signature: vec![marker],
            };
            let vote_wire_identity = authenticated_wire_identity(
                wire::ConsensusMessageV2Payload::Vote(wire_vote.clone()),
            );
            let vote = adapter
                .registry
                .vote_to_core(&wire_vote, &adapter.wire_context)
                .expect("convert locked Commit capacity fixture");
            let admission = IngressAdmission {
                key: IngressSemanticKey::Vote {
                    round: wire_round,
                    phase: wire::GlobalPhase::Commit,
                    signer,
                },
                fingerprint: IngressFingerprint::Vote(
                    wire_round,
                    locked_subject,
                    locked_commitment,
                ),
                generation: tag.generation(),
                inserted_equivocation: false,
                locked_commit_progress: true,
            };
            assert!(
                adapter
                    .enqueue_deferred(
                        reducer::Event::VoteReceived { tag, vote },
                        true,
                        DeferredPriority::Progress,
                        Some(admission),
                        None,
                        Some(vote_wire_identity),
                    )
                    .expect("admit one locked Commit owner per frozen validator")
                    .is_some()
            );

            let wire_timeout = wire::TimeoutVote {
                round: wire_round,
                highest_prepare_qc: None,
                signer,
                signature: vec![marker ^ 0x0F],
            };
            let timeout_wire_identity = authenticated_wire_identity(
                wire::ConsensusMessageV2Payload::TimeoutVote(wire_timeout.clone()),
            );
            let timeout = adapter
                .registry
                .timeout_vote_to_core(&wire_timeout, &adapter.wire_context)
                .expect("convert TimeoutVote capacity fixture");
            assert!(
                adapter
                    .enqueue_deferred(
                        reducer::Event::TimeoutVoteReceived { tag, vote: timeout },
                        true,
                        DeferredPriority::Progress,
                        None,
                        None,
                        Some(timeout_wire_identity),
                    )
                    .expect("admit one TimeoutVote owner per frozen validator")
                    .is_some()
            );
            if signer == 0 {
                let retained = adapter.deferred_progress_inputs.clone();
                let wire_distinct_same_signer = wire::TimeoutVote {
                    round: wire::ConsensusRound {
                        view: wire_round.view + 1,
                        ..wire_round
                    },
                    highest_prepare_qc: None,
                    signer,
                    signature: vec![marker ^ 0xF0],
                };
                let distinct_wire_identity = authenticated_wire_identity(
                    wire::ConsensusMessageV2Payload::TimeoutVote(wire_distinct_same_signer.clone()),
                );
                let distinct_same_signer = adapter
                    .registry
                    .timeout_vote_to_core(&wire_distinct_same_signer, &adapter.wire_context)
                    .expect("convert distinct same-signer TimeoutVote fixture");
                let distinct_same_signer = reducer::Event::TimeoutVoteReceived {
                    tag,
                    vote: distinct_same_signer,
                };
                assert!(
                    adapter
                        .enqueue_deferred(
                            distinct_same_signer.clone(),
                            true,
                            DeferredPriority::Progress,
                            None,
                            None,
                            Some(Arc::clone(&distinct_wire_identity)),
                        )
                        .expect("same signer cannot consume a second TimeoutVote slot")
                        .is_none(),
                    "TimeoutVote ownership must be signer-injective before the class is full"
                );
                assert_eq!(
                    adapter.deferred_progress_inputs, retained,
                    "later same-signer traffic must not displace admitted progress"
                );
                let core_signer = adapter
                    .registry
                    .validator_id(signer)
                    .expect("fixture signer belongs to the frozen roster");
                let owned_index = adapter
                    .deferred_progress_inputs
                    .iter()
                    .position(|queued| {
                        deferred_progress_owner(queued)
                            == Some(DeferredProgressOwner::TimeoutVote(core_signer))
                    })
                    .expect("original same-signer TimeoutVote owns one slot");
                adapter.deferred_progress_inputs.remove(owned_index);
                assert!(
                    adapter
                        .enqueue_deferred(
                            distinct_same_signer,
                            true,
                            DeferredPriority::Progress,
                            None,
                            None,
                            Some(distinct_wire_identity),
                        )
                        .expect("same signer retries after its prior owner is serviced")
                        .is_some()
                );
            }
        }

        for (phase, marker) in [
            (wire::GlobalPhase::Prepare, 0xB0),
            (wire::GlobalPhase::Commit, 0xB1),
        ] {
            let certificate = wire::QuorumCertificate {
                round: wire_round,
                proposal_round: wire_round,
                phase,
                subject: subject(marker),
                execution_commitment: execution_commitment(marker),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![marker; 96],
            };
            let certificate_wire_identity = authenticated_wire_identity(
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone()),
            );
            let certificate = adapter
                .registry
                .qc_to_core(&certificate, &adapter.wire_context)
                .expect("convert QC capacity fixture");
            assert!(
                adapter
                    .enqueue_deferred(
                        reducer::Event::QuorumCertificateReceived { tag, certificate },
                        true,
                        DeferredPriority::Progress,
                        None,
                        None,
                        Some(certificate_wire_identity),
                    )
                    .expect("admit the independent QC class owner")
                    .is_some()
            );
        }
        let timeout_certificate = wire::TimeoutCertificate {
            round: wire_round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xB2; 96],
            }],
        };
        let timeout_certificate_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate.clone()),
        );
        let timeout_certificate = adapter
            .registry
            .tc_to_core(&timeout_certificate, &adapter.wire_context)
            .expect("convert TC capacity fixture");
        assert!(
            adapter
                .enqueue_deferred(
                    reducer::Event::TimeoutCertificateReceived {
                        tag,
                        certificate: timeout_certificate,
                    },
                    true,
                    DeferredPriority::Progress,
                    None,
                    None,
                    Some(timeout_certificate_wire_identity),
                )
                .expect("admit the independent TC class owner")
                .is_some()
        );

        assert_eq!(
            adapter.deferred_progress_inputs.len(),
            deferred_progress_capacity(roster_len)
        );
        for (class, expected) in [
            (DeferredProgressClass::LockedCommitVote, roster_len),
            (DeferredProgressClass::TimeoutVote, roster_len),
            (DeferredProgressClass::PrepareCertificate, 1),
            (DeferredProgressClass::CommitCertificate, 1),
            (DeferredProgressClass::TimeoutCertificate, 1),
        ] {
            assert_eq!(
                adapter
                    .deferred_progress_inputs
                    .iter()
                    .filter(|input| deferred_progress_class(input) == Some(class))
                    .count(),
                expected,
                "each protected Progress class owns its exact partition"
            );
        }

        let retained = adapter.deferred_progress_inputs.clone();
        let later_round = wire::ConsensusRound {
            view: 1,
            ..wire_round
        };
        let overflow = wire::TimeoutVote {
            round: later_round,
            highest_prepare_qc: None,
            signer: 0,
            signature: vec![0xBF],
        };
        let overflow_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::TimeoutVote(overflow.clone()),
        );
        let overflow = adapter
            .registry
            .timeout_vote_to_core(&overflow, &adapter.wire_context)
            .expect("convert distinct TimeoutVote overflow fixture");
        assert!(
            adapter
                .enqueue_deferred(
                    reducer::Event::TimeoutVoteReceived {
                        tag,
                        vote: overflow,
                    },
                    true,
                    DeferredPriority::Progress,
                    None,
                    None,
                    Some(overflow_wire_identity),
                )
                .expect("a full TimeoutVote partition rejects without displacement")
                .is_none()
        );
        assert_eq!(adapter.deferred_progress_inputs, retained);
    }

    #[test]
    fn protected_locked_vote_uses_reserved_capacity_without_evicting_certificate_ownership() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let wire_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let wire_timeout = wire::TimeoutCertificate {
            round: wire_round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xCA; 96],
            }],
        };
        let timeout_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(wire_timeout.clone()),
        );
        let timeout = adapter
            .registry
            .tc_to_core(&wire_timeout, &adapter.wire_context)
            .expect("convert certificate lane fixture");
        let tag = adapter.current_tag();
        let certificate_input = DeferredInput {
            admission_ordinal: 1,
            admission_capability: DeferredAdmissionCapability::for_test(1),
            event: reducer::Event::TimeoutCertificateReceived {
                tag,
                certificate: timeout,
            },
            completion_evidence: None,
            retag_authenticated_ingress: true,
            priority: DeferredPriority::Progress,
            protected_progress: false,
            admission: None,
            authenticated_wire_identity: Some(timeout_wire_identity),
            admitted_at: Instant::now(),
            eligible_skips: 0,
        };
        adapter
            .deferred_progress_inputs
            .push_back(certificate_input.clone());
        assert!(
            adapter
                .deferred_progress_inputs
                .iter()
                .all(|input| progress_rank(&input.event) > 0)
        );
        let admitted_before = adapter.deferred_progress_inputs.clone();
        let wire_overflow_certificate = wire::TimeoutCertificate {
            round: wire::ConsensusRound {
                view: wire_round.view + 1,
                ..wire_round
            },
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xCB; 96],
            }],
        };
        let overflow_certificate_wire_identity = authenticated_wire_identity(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(wire_overflow_certificate.clone()),
        );
        let overflow_certificate = adapter
            .registry
            .tc_to_core(&wire_overflow_certificate, &adapter.wire_context)
            .expect("convert distinct certificate overflow fixture");
        assert!(
            adapter
                .enqueue_deferred(
                    reducer::Event::TimeoutCertificateReceived {
                        tag,
                        certificate: overflow_certificate,
                    },
                    true,
                    DeferredPriority::Progress,
                    None,
                    None,
                    Some(overflow_certificate_wire_identity),
                )
                .expect("ordinary certificate overflow is rejected before admission")
                .is_none()
        );
        assert_eq!(
            adapter.deferred_progress_inputs, admitted_before,
            "equal-rank traffic must never replace already admitted certificate ownership"
        );

        let locked_subject = subject(0xDA);
        let locked_execution_commitment = execution_commitment(0xDA);
        let wire_vote = wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: locked_subject,
            execution_commitment: locked_execution_commitment,
            signer: 1,
            signature: vec![0xDA],
        };
        let vote_wire_identity =
            authenticated_wire_identity(wire::ConsensusMessageV2Payload::Vote(wire_vote.clone()));
        let vote = adapter
            .registry
            .vote_to_core(&wire_vote, &adapter.wire_context)
            .expect("convert protected locked vote fixture");
        let admission = IngressAdmission {
            key: IngressSemanticKey::Vote {
                round: wire_round,
                phase: wire::GlobalPhase::Commit,
                signer: 1,
            },
            fingerprint: IngressFingerprint::Vote(
                wire_round,
                locked_subject,
                locked_execution_commitment,
            ),
            generation: tag.generation(),
            inserted_equivocation: false,
            locked_commit_progress: true,
        };
        let protected_event = reducer::Event::VoteReceived { tag, vote };
        assert_eq!(progress_rank(&protected_event), 0);

        assert!(
            adapter
                .enqueue_deferred(
                    protected_event,
                    true,
                    DeferredPriority::Progress,
                    Some(admission),
                    None,
                    Some(vote_wire_identity),
                )
                .expect("protected ownership uses its reserved locked-vote capacity")
                .is_some()
        );
        assert_eq!(adapter.deferred_progress_inputs.len(), 2);
        assert_eq!(
            adapter
                .deferred_progress_inputs
                .iter()
                .filter(|input| input.protected_progress)
                .count(),
            1
        );
        assert!(matches!(
            adapter.deferred_progress_inputs.back(),
            Some(DeferredInput {
                event: reducer::Event::VoteReceived { .. },
                admission: Some(_),
                protected_progress: true,
                ..
            })
        ));
    }

    fn saturate_ordinary_semantic_history(
        adapter: &mut SumeragiV2Adapter,
        round: wire::ConsensusRound,
    ) {
        for index in 0..MAX_INGRESS_SEMANTIC_KEYS {
            if adapter.ingress_equivocations.len() >= MAX_INGRESS_SEMANTIC_KEYS {
                break;
            }
            let proposer = u32::MAX
                .checked_sub(u32::try_from(index).expect("semantic index fits u32"))
                .expect("fixture proposer remains in range");
            adapter.ingress_equivocations.insert(
                IngressSemanticKey::Proposal { round, proposer },
                IngressEquivocationRecord {
                    fingerprint: IngressFingerprint::Proposal(Hash::new(index.to_le_bytes())),
                    equivocation_reported: false,
                    capacity_bypass: false,
                    admitted_at: Instant::now(),
                },
            );
        }
        assert_eq!(
            adapter.ingress_equivocations.len(),
            MAX_INGRESS_SEMANTIC_KEYS
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn busy_deferred_source_identity_coalesces_across_consumer_view_change() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let first_tag = adapter.current_tag();
        let first_round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: first_tag.view(),
        };
        saturate_ordinary_semantic_history(&mut adapter, first_round);

        let first_timeout = adapter
            .timeout_elapsed(first_tag)
            .expect("start the first local TimeoutVote signature fence");
        let first_sign_tag = match first_timeout.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(_),
                },
            ] => *tag,
            effects => panic!("unexpected first timeout effects: {effects:?}"),
        };

        let timeout_certificate = wire::TimeoutCertificate {
            round: first_round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xD7; 96],
            }],
        };
        let deferred_tc = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate),
            ))
            .expect("defer the TC behind the first signature fence");
        assert_eq!(
            deferred_tc.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );

        let old_timeout = wire::TimeoutVote {
            round: first_round,
            highest_prepare_qc: None,
            signer: 1,
            signature: vec![0xD8],
        };
        let old_key = IngressSemanticKey::TimeoutVote {
            round: first_round,
            signer: 1,
        };
        let deferred_old = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(old_timeout.clone()),
            ))
            .expect("defer the old-view TimeoutVote behind the TC");
        assert_eq!(
            deferred_old.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert!(
            adapter
                .ingress_equivocations
                .get(&old_key)
                .is_some_and(|record| record.capacity_bypass)
        );
        assert!(adapter.ingress_deliveries.contains_key(&old_key));
        assert_eq!(adapter.deferred_progress_inputs.len(), 2);
        let old_input = adapter
            .deferred_progress_inputs
            .back()
            .expect("the old-view TimeoutVote owns the later Busy slot");
        let original_candidate = adapter
            .serviced_candidate(
                &old_input.event,
                old_input.priority,
                old_input.completion_evidence.as_ref(),
                old_input.authenticated_wire_identity.as_deref(),
            )
            .expect("authenticated TimeoutVote has a service identity");
        assert_eq!(original_candidate.1, first_round.view);
        assert_eq!(original_candidate.0.source_view(), first_round.view);
        assert_eq!(
            original_candidate.0.leader(),
            adapter.wire_context.leader(first_round.view)
        );

        let duplicate_old = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(old_timeout),
            ))
            .expect("coalesce the exact deferred TimeoutVote");
        assert_eq!(
            duplicate_old.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert_eq!(adapter.deferred_progress_inputs.len(), 2);

        let signed = adapter
            .signature_completed(first_sign_tag, vec![0xD9; 96])
            .expect("complete the first signature before installing the deferred TC")
            .into_effects();
        assert!(
            signed
                .iter()
                .all(|effect| !matches!(effect, AdapterEffect::EnterView { .. }))
        );
        let enter_view = adapter
            .drain_deferred()
            .expect("install the deferred TC as a separate macro-step");
        assert!(enter_view.iter().any(|effect| matches!(
            effect,
            AdapterEffect::EnterView { tag, .. } if tag.view() == 1
        )));
        assert_eq!(adapter.current_tag().view(), 1);
        assert_eq!(
            adapter.deferred_progress_inputs.len(),
            1,
            "EnterView must leave the later old-view TimeoutVote owned until service"
        );
        let old_owner = adapter
            .registry
            .validator_id(1)
            .expect("fixture TimeoutVote signer belongs to the frozen roster");
        assert!(matches!(
            adapter.deferred_progress_inputs.front(),
            Some(DeferredInput {
                event: reducer::Event::TimeoutVoteReceived { vote, .. },
                ..
            }) if vote.vote().round().view() == 0
                && vote.vote().signer() == old_owner
        ));
        let old_input = adapter
            .deferred_progress_inputs
            .front()
            .expect("the old-view TimeoutVote remains owned");
        let retagged_event = old_input
            .event
            .clone()
            .retag_authenticated_ingress(adapter.current_tag());
        let retagged_candidate = adapter
            .serviced_candidate(
                &retagged_event,
                old_input.priority,
                old_input.completion_evidence.as_ref(),
                old_input.authenticated_wire_identity.as_deref(),
            )
            .expect("retagged TimeoutVote retains a service identity");
        assert_eq!(retagged_candidate.0, original_candidate.0);
        assert_eq!(retagged_candidate.0.source_view(), first_round.view);
        assert_eq!(retagged_candidate.1, adapter.current_tag().view());
        assert_ne!(
            retagged_candidate.0.leader(),
            adapter.wire_context.leader(retagged_candidate.1),
            "logical leader ownership derives from source view, not the consumer episode"
        );
        assert_ne!(
            original_candidate.1, retagged_candidate.1,
            "the consumer episode advanced while semantic source identity stayed fixed"
        );
        assert!(
            !adapter.ingress_equivocations.contains_key(&old_key)
                && !adapter.ingress_deliveries.contains_key(&old_key),
            "a capacity-bypass TimeoutVote record must retire when its view is no longer current"
        );

        let second_tag = adapter.current_tag();
        let second_timeout = adapter
            .timeout_elapsed(second_tag)
            .expect("start the current-view TimeoutVote signature fence");
        let second_sign_tag = match second_timeout.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(_),
                },
            ] => *tag,
            effects => panic!("unexpected second timeout effects: {effects:?}"),
        };
        let second_round = wire::ConsensusRound {
            view: second_tag.view(),
            ..first_round
        };
        let current_timeout = wire::TimeoutVote {
            round: second_round,
            highest_prepare_qc: None,
            signer: 1,
            signature: vec![0xDA],
        };
        let current_key = IngressSemanticKey::TimeoutVote {
            round: second_round,
            signer: 1,
        };
        let registry_before = adapter.registry.clone();
        let active_subject_before = adapter.active_subject;
        let deferred_before = adapter.deferred_progress_inputs.clone();
        for attempt in 0..2 {
            let blocked = adapter
                .receive_verified(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::TimeoutVote(current_timeout.clone()),
                ))
                .expect("same-owner TimeoutVote remains retryable before service");
            assert_eq!(
                blocked.disposition(),
                reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy),
                "pre-service attempt {attempt} must not be poisoned as a duplicate"
            );
            assert_eq!(adapter.deferred_progress_inputs, deferred_before);
            assert_registry_eq(&adapter.registry, &registry_before);
            assert_eq!(adapter.active_subject, active_subject_before);
            assert!(
                adapter
                    .ingress_equivocations
                    .get(&current_key)
                    .is_some_and(|record| record.capacity_bypass)
            );
            assert!(!adapter.ingress_deliveries.contains_key(&current_key));
        }

        adapter
            .signature_completed(second_sign_tag, vec![0xDB; 96])
            .expect("complete the current-view signature");
        assert!(
            adapter
                .drain_deferred()
                .expect("service the old owner in its own macro-step")
                .is_empty()
        );
        assert!(adapter.deferred_progress_inputs.is_empty());
        assert_eq!(
            adapter.serviced_candidates.get(&original_candidate.0),
            None,
            "retagged authenticated policy discard remains marker-free"
        );
        let retained_count = adapter.serviced_candidate_count_for_test();
        adapter
            .record_serviced_candidate(Some(retagged_candidate), false, false, None)
            .expect("an exact same-episode source occurrence coalesces");
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            retained_count + 1,
            "a transient same-source projection remains owned until strict episode exit"
        );

        let applied = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(current_timeout.clone()),
            ))
            .expect("retry the current-view TimeoutVote after service");
        assert_eq!(applied.disposition(), reducer::StepDisposition::Applied);
        assert!(adapter.ingress_deliveries.contains_key(&current_key));
        let duplicate = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(current_timeout),
            ))
            .expect("coalesce the delivered current-view TimeoutVote");
        assert_eq!(
            duplicate.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
    }

    #[test]
    fn full_normal_deferred_lane_cannot_drop_absolute_timeout() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        // Leave the reducer waiting for a Prepare signature, then model a
        // saturated untrusted deferred lane. The absolute timeout is delivered
        // while that signature fence is active, exactly where it used to be
        // classified as normal traffic and silently discarded.
        let proposer = adapter.status().expect("status").leader;
        let proposed_subject = subject(0xD2);
        let fetch = adapter
            .receive_verified(proposal(&adapter.wire_context, proposer, proposed_subject))
            .expect("accept proposal")
            .into_effects();
        let (tag, manifest) = match fetch.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };
        let round = manifest.round;
        adapter
            .body_available(tag, manifest)
            .expect("body available");
        let receipt = durable_body_receipt(&adapter, round, proposed_subject);
        adapter
            .body_stored(tag, round, proposed_subject, &receipt)
            .expect("body stored");
        let validated = ValidatedBodyReceipt::for_test(receipt);
        let sign = adapter
            .validation_succeeded(tag, round, proposed_subject, &validated)
            .expect("body valid")
            .into_effects();
        let sign_tag = match sign.as_slice() {
            [AdapterEffect::Sign { tag, .. }] => *tag,
            effects => panic!("unexpected validation effects: {effects:?}"),
        };

        let normal_vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0xD3),
            execution_commitment: execution_commitment(0xD3),
            signer: 1,
            signature: vec![0xD3],
        };
        let deferred_vote = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(normal_vote.clone()),
            ))
            .expect("defer normal authenticated vote");
        assert_eq!(
            deferred_vote.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        let filler = adapter
            .deferred_inputs
            .front()
            .expect("normal vote is queued")
            .clone();
        assert_eq!(filler.priority, DeferredPriority::Normal);
        adapter.deferred_inputs = std::iter::repeat_n(filler, MAX_DEFERRED_INPUTS).collect();

        let mut backpressured_vote = normal_vote;
        backpressured_vote.signer = 2;
        backpressured_vote.signature = vec![0xD4];
        let backpressured_key = IngressSemanticKey::Vote {
            round,
            phase: wire::GlobalPhase::Prepare,
            signer: 2,
        };
        let backpressured = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(backpressured_vote.clone()),
            ))
            .expect("apply normal-lane backpressure");
        assert_eq!(
            backpressured.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert!(
            adapter
                .ingress_equivocations
                .contains_key(&backpressured_key)
        );
        assert!(
            !adapter.ingress_deliveries.contains_key(&backpressured_key),
            "admission without queue ownership must remain retryable"
        );

        adapter.deferred_inputs.pop_back();
        let retried = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(backpressured_vote),
            ))
            .expect("retry after reserved ownership becomes available");
        assert_eq!(
            retried.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert!(adapter.ingress_deliveries.contains_key(&backpressured_key));
        assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);

        // Saturate the ordinary semantic table as well. TimeoutVote owns an
        // independent signer-bounded semantic slot, so it must still reach the
        // protected Busy-deferred partition instead of being rejected before
        // the reducer boundary.
        saturate_ordinary_semantic_history(&mut adapter, round);

        let timeout_vote = wire::TimeoutVote {
            round,
            highest_prepare_qc: None,
            signer: 1,
            signature: vec![0xD5],
        };
        let timeout_key = IngressSemanticKey::TimeoutVote { round, signer: 1 };
        let deferred_timeout_vote = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(timeout_vote),
            ))
            .expect("defer TimeoutVote through its protected class");
        assert_eq!(
            deferred_timeout_vote.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
        assert!(
            adapter
                .ingress_equivocations
                .get(&timeout_key)
                .is_some_and(|record| record.capacity_bypass),
            "current-view TimeoutVote must bypass saturated ordinary semantic capacity"
        );
        assert!(adapter.ingress_deliveries.contains_key(&timeout_key));
        assert!(matches!(
            adapter.deferred_progress_inputs.back(),
            Some(DeferredInput {
                event: reducer::Event::TimeoutVoteReceived { .. },
                priority: DeferredPriority::Progress,
                protected_progress: false,
                ..
            })
        ));
        assert_eq!(
            deferred_progress_class(
                adapter
                    .deferred_progress_inputs
                    .back()
                    .expect("deferred TimeoutVote owns the progress lane")
            ),
            Some(DeferredProgressClass::TimeoutVote)
        );

        let timeout = adapter
            .timeout_elapsed(sign_tag)
            .expect("defer trusted absolute timeout");
        assert_eq!(
            timeout.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
        assert!(matches!(
            adapter.deferred_completions.front(),
            Some(DeferredInput {
                event: reducer::Event::TimeoutElapsed { .. },
                priority: DeferredPriority::Completion,
                ..
            })
        ));

        let completed = adapter
            .signature_completed(sign_tag, vec![0xD2; 96])
            .expect("complete outstanding Prepare signature")
            .into_effects();
        assert!(completed.iter().all(|effect| !matches!(
            effect,
            AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }
        )));
        let timeout_effects = adapter
            .drain_deferred()
            .expect("service the absolute timeout as one deferred macro-step");
        let timeout_sign_tag = timeout_effects
            .iter()
            .find_map(|effect| match effect {
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(_),
                } => Some(*tag),
                _ => None,
            })
            .expect("absolute timeout starts the durable local TimeoutVote signature");
        assert!(adapter.deferred_completions.is_empty());
        assert_eq!(
            adapter.deferred_progress_inputs.len(),
            1,
            "the remote TimeoutVote remains owned while the local TimeoutVote signature fences the reducer"
        );

        adapter
            .signature_completed(timeout_sign_tag, vec![0xD6; 96])
            .expect("complete the local TimeoutVote signature");
        adapter
            .drain_deferred()
            .expect("service protected progress in its own macro-step");
        assert!(adapter.deferred_progress_inputs.is_empty());
    }

    #[test]
    fn failed_ingress_conversion_rolls_back_registry_and_admission() {
        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let proposer = adapter.status().expect("status").leader;
        let proposed_subject = subject(0xE0);
        let valid = proposal(&adapter.wire_context, proposer, proposed_subject);
        let mut malformed = valid.clone();
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut malformed.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        proposal.justification = wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: wire::TimeoutCertificate {
                round: proposal.round,
                groups: Vec::new(),
            },
            highest_prepare_qc: None,
        });

        let subject_count = adapter.registry.subjects.len();
        let manifest_count = adapter.registry.manifests.len();
        assert!(adapter.receive_verified(malformed).is_err());
        assert_eq!(adapter.registry.subjects.len(), subject_count);
        assert_eq!(adapter.registry.manifests.len(), manifest_count);
        assert!(adapter.ingress_equivocations.is_empty());
        assert!(adapter.ingress_deliveries.is_empty());
        assert!(adapter.active_subject.is_none());

        // The failed conversion did not poison the semantic key; the valid
        // proposal for the same leader and round is still admitted.
        assert!(matches!(
            adapter
                .receive_verified(valid)
                .expect("valid retry")
                .effects(),
            [AdapterEffect::FetchBody { .. }]
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn authentication_rejects_valid_commitment_conflicts_without_mutating_adapter() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys, pops) = authenticated_context();
        let verified =
            VerifiedHeightContext::genesis(context.clone(), pops).expect("verified context");
        let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("commitment-auth-safety.wal"),
            verified,
            None,
            reducer::Generation::new(1),
            [0x83; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("open observing adapter");
        assert!(startup.is_empty());

        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let locally_validated_subject = subject(0x87);
        let locally_validated_manifest = wire::PayloadManifest::derive(
            &context,
            round,
            locally_validated_subject,
            5,
            &[b"local".to_vec()],
        )
        .expect("derive locally validated manifest");
        let (_, locally_validated_receipt) =
            validated_receipts_for_manifest(&context, &locally_validated_manifest);
        let locally_validated_commitment = locally_validated_receipt.execution_commitment();
        let wrong_unbound_commitment = execution_commitment(0x87);
        assert_ne!(wrong_unbound_commitment, locally_validated_commitment);
        let signed_vote = |execution_commitment| {
            let mut vote = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: locally_validated_subject,
                execution_commitment,
                signer: 0,
                signature: Vec::new(),
            };
            vote.signature = Signature::new(
                keys[usize::try_from(vote.signer).expect("small signer")].private_key(),
                &vote.signature_preimage(),
            )
            .payload()
            .to_vec();
            vote
        };
        let wrong_unbound_vote = signed_vote(wrong_unbound_commitment);
        let canonical_unbound_vote = signed_vote(locally_validated_commitment);
        let registry_before_unbound_votes = adapter.registry.clone();
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(wrong_unbound_vote.clone()),
            )),
            Err(AdapterError::MissingExecutionCommitment)
        ));
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(canonical_unbound_vote.clone()),
            )),
            Err(AdapterError::MissingExecutionCommitment)
        ));
        assert_registry_eq(&adapter.registry, &registry_before_unbound_votes);
        assert!(adapter.ingress_equivocations.is_empty());
        assert!(adapter.ingress_deliveries.is_empty());
        assert!(adapter.deferred_completions.is_empty());
        assert!(adapter.deferred_progress_inputs.is_empty());
        assert!(adapter.deferred_inputs.is_empty());
        assert!(adapter.ingress_ready());
        assert!(!adapter.fail_closed);

        adapter
            .recover_validated_body(&locally_validated_manifest, &locally_validated_receipt)
            .expect("local deterministic validation establishes canonical commitment authority");
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(wrong_unbound_vote),
            )),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));
        adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(canonical_unbound_vote),
            ))
            .expect("the same signed canonical vote is admissible after local validation");
        assert!(adapter.ingress_ready());
        assert!(!adapter.fail_closed);

        let bound_subject = subject(0x83);
        let canonical_commitment = execution_commitment(0x83);
        let conflicting_commitment = execution_commitment(0x84);
        let core_subject = adapter
            .registry
            .register_subject(bound_subject)
            .expect("register canonical subject");
        adapter
            .registry
            .register_execution_commitment(
                reducer::Round::new(round.height, round.view),
                core_subject,
                canonical_commitment,
            )
            .expect("bind canonical validated execution result");
        let retained_registry = adapter.registry.clone();
        let retained_equivocations = adapter.ingress_equivocations.clone();
        let retained_deliveries = adapter.ingress_deliveries.clone();
        let retained_queue_lengths = (
            adapter.deferred_completions.len(),
            adapter.deferred_progress_inputs.len(),
            adapter.deferred_inputs.len(),
        );

        let mut conflicting_vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: bound_subject,
            execution_commitment: conflicting_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        conflicting_vote.signature = Signature::new(
            keys[usize::try_from(conflicting_vote.signer).expect("small signer")].private_key(),
            &conflicting_vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(conflicting_vote.clone()),
            )),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));

        let mut conflicting_qc = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: bound_subject,
            execution_commitment: conflicting_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        authenticate_qc(&mut conflicting_qc, &keys);
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(conflicting_qc.clone()),
            )),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));

        let later_round = wire::ConsensusRound { view: 1, ..round };
        let mut cross_round_conflicting_vote = wire::Vote {
            round: later_round,
            proposal_round: later_round,
            signature: Vec::new(),
            ..conflicting_vote
        };
        cross_round_conflicting_vote.signature = Signature::new(
            keys[usize::try_from(cross_round_conflicting_vote.signer).expect("small signer index")]
                .private_key(),
            &cross_round_conflicting_vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        let cross_round_conflicting_payload =
            wire::ConsensusMessageV2Payload::Vote(cross_round_conflicting_vote.clone());
        assert_eq!(
            adapter.wire_ingress_missing_execution_commitment(&cross_round_conflicting_payload),
            None,
            "a same-subject cross-round conflict must drain instead of retaining fair-ingress ownership"
        );
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                cross_round_conflicting_payload,
            )),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));

        let mut cross_round_canonical_vote = wire::Vote {
            execution_commitment: canonical_commitment,
            signature: Vec::new(),
            ..cross_round_conflicting_vote
        };
        cross_round_canonical_vote.signature = Signature::new(
            keys[usize::try_from(cross_round_canonical_vote.signer).expect("small signer index")]
                .private_key(),
            &cross_round_canonical_vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        let cross_round_canonical_payload =
            wire::ConsensusMessageV2Payload::Vote(cross_round_canonical_vote);
        assert_eq!(
            adapter.wire_ingress_missing_execution_commitment(&cross_round_canonical_payload),
            Some((later_round, bound_subject)),
            "the same commitment on another round remains unbound until exact-round validation"
        );
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(cross_round_canonical_payload,)),
            Err(AdapterError::MissingExecutionCommitment)
        ));

        let mut cross_round_conflict = wire::QuorumCertificate {
            round: later_round,
            proposal_round: later_round,
            ..conflicting_qc.clone()
        };
        authenticate_qc(&mut cross_round_conflict, &keys);
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(cross_round_conflict),
            )),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));
        let mut cross_round_canonical = wire::QuorumCertificate {
            round: later_round,
            proposal_round: later_round,
            execution_commitment: canonical_commitment,
            ..conflicting_qc.clone()
        };
        authenticate_qc(&mut cross_round_canonical, &keys);
        adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(cross_round_canonical),
            ))
            .expect("an unchanged re-proposal authenticates the same deterministic execution");

        let timeout_round = wire::ConsensusRound { view: 1, ..round };
        let timeout_preimage = wire::TimeoutVote {
            round: timeout_round,
            highest_prepare_qc: Some(conflicting_qc.clone()),
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let timeout_shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &timeout_preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let timeout_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &timeout_shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate valid timeout signatures");
        let mut conflicting_timeout_vote = wire::TimeoutVote {
            round: timeout_round,
            highest_prepare_qc: Some(conflicting_qc.clone()),
            signer: 0,
            signature: Vec::new(),
        };
        conflicting_timeout_vote.signature = Signature::new(
            keys[usize::try_from(conflicting_timeout_vote.signer).expect("small signer")]
                .private_key(),
            &conflicting_timeout_vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(conflicting_timeout_vote),
            )),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));
        let conflicting_tc = wire::TimeoutCertificate {
            round: timeout_round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(conflicting_qc.clone()),
                signers: vec![0, 1, 2],
                aggregate_signature: timeout_signature,
            }],
        };
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(conflicting_tc.clone()),
            )),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));

        let proposal_round = wire::ConsensusRound { view: 2, ..round };
        let proposal_subject = bound_subject;
        let proposal_body = vec![0x83, 2];
        let proposal_manifest = wire::PayloadManifest::derive(
            &context,
            proposal_round,
            proposal_subject,
            u64::try_from(proposal_body.len()).expect("proposal body length"),
            &[proposal_body],
        )
        .expect("derive later-view proposal manifest");
        let proposer = context.leader(proposal_round.view);
        let mut conflicting_proposal = wire::Proposal {
            round: proposal_round,
            proposer,
            subject: proposal_subject,
            manifest: proposal_manifest,
            justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
                timeout_certificate: conflicting_tc,
                highest_prepare_qc: Some(conflicting_qc.clone()),
            }),
            signature: Vec::new(),
        };
        conflicting_proposal.signature = Signature::new(
            keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
            &conflicting_proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        let conflicting_proposal_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal),
        );
        // Exercise the read-only embedded-certificate compatibility walk
        // directly, then confirm ordinary ingress rejects the same
        // structurally valid proposal for its conflicting deterministic
        // execution result.
        let authenticated_conflicting_proposal =
            AuthenticatedConsensusMessage::for_test(conflicting_proposal_message.clone());
        assert!(matches!(
            adapter.ensure_authenticated_execution_commitments_compatible(
                &authenticated_conflicting_proposal,
            ),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));
        assert!(matches!(
            adapter.authenticate(conflicting_proposal_message),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));

        let unbound_subject = subject(0x85);
        let mut unbound_qc_a = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: unbound_subject,
            execution_commitment: execution_commitment(0x85),
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        authenticate_qc(&mut unbound_qc_a, &keys);
        let mut unbound_qc_b = wire::QuorumCertificate {
            execution_commitment: execution_commitment(0x86),
            ..unbound_qc_a.clone()
        };
        authenticate_qc(&mut unbound_qc_b, &keys);
        let timeout_group = |highest_prepare_qc: wire::QuorumCertificate,
                             signers: Vec<wire::ValidatorIndex>| {
            let preimage = wire::TimeoutVote {
                round: timeout_round,
                highest_prepare_qc: Some(highest_prepare_qc.clone()),
                signer: signers[0],
                signature: Vec::new(),
            }
            .signature_preimage();
            let shares = signers
                .iter()
                .map(|signer| {
                    Signature::new(
                        keys[usize::try_from(*signer).expect("small signer")].private_key(),
                        &preimage,
                    )
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(highest_prepare_qc),
                signers,
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                    &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                )
                .expect("aggregate valid disjoint timeout group"),
            }
        };
        let mut conflicting_groups = vec![
            timeout_group(unbound_qc_a, vec![0, 1]),
            timeout_group(unbound_qc_b, vec![2, 3]),
        ];
        conflicting_groups.sort_by_key(|group| {
            group
                .highest_prepare_qc
                .as_ref()
                .map(wire::QuorumCertificate::as_ref)
        });
        let within_envelope_conflict = wire::TimeoutCertificate {
            round: timeout_round,
            groups: conflicting_groups,
        };
        assert!(matches!(
            adapter.authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(within_envelope_conflict),
            )),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));
        assert!(
            !adapter
                .registry
                .execution_commitments
                .keys()
                .any(|(_, registered_subject)| *registered_subject
                    == reducer::Subject::new(Hash::new(unbound_subject.encode()).into())),
            "within-envelope checking cannot bind either attacker commitment"
        );
        assert!(adapter.ingress_ready());
        assert!(!adapter.fail_closed);

        // Transport adapters authenticate their outer request/response
        // identities separately. The same read-only compatibility walk still
        // covers every embedded certificate before a transport payload is
        // unwrapped into reducer ingress.
        let certified_request =
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyRequest(wire::CertifiedBodyRequest {
                    round,
                    subject: bound_subject,
                    certificate: conflicting_qc.clone(),
                    requester: context.roster[0].validator.clone(),
                    signature: vec![0x83; 96],
                }),
            ));
        assert!(matches!(
            adapter.ensure_authenticated_execution_commitments_compatible(&certified_request),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));
        let commit_response =
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                    wire::CommitCertificateResponse {
                        request_hash: HashOf::from_untyped_unchecked(Hash::new(
                            b"commitment-conflict-request",
                        )),
                        certificate: wire::QuorumCertificate {
                            phase: wire::GlobalPhase::Commit,
                            ..conflicting_qc
                        },
                        responder: context.roster[1].validator.clone(),
                        signature: vec![0x84; 96],
                    },
                ),
            ));
        assert!(matches!(
            adapter.ensure_authenticated_execution_commitments_compatible(&commit_response),
            Err(AdapterError::ConflictingExecutionCommitment)
        ));

        assert_registry_eq(&adapter.registry, &retained_registry);
        assert_eq!(adapter.ingress_equivocations, retained_equivocations);
        assert_eq!(adapter.ingress_deliveries, retained_deliveries);
        assert_eq!(
            (
                adapter.deferred_completions.len(),
                adapter.deferred_progress_inputs.len(),
                adapter.deferred_inputs.len(),
            ),
            retained_queue_lengths
        );
        assert!(adapter.ingress_ready());
        assert!(!adapter.fail_closed);

        let mut canonical_vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: bound_subject,
            execution_commitment: canonical_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        canonical_vote.signature = Signature::new(
            keys[usize::try_from(canonical_vote.signer).expect("small signer")].private_key(),
            &canonical_vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(canonical_vote),
            ))
            .expect("the exact canonical commitment remains authentically admissible");
        assert!(adapter.ingress_ready());
    }

    #[cfg(feature = "bls")]
    #[test]
    fn authenticated_ingress_verifies_individual_and_aggregate_bls() {
        let (context, keys, pops) = authenticated_context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = subject(12);
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: execution_commitment(12),
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(keys[0].private_key(), &vote.signature_preimage())
            .payload()
            .to_vec();
        verify_authenticated_message(
            &context,
            None,
            &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
            &pops,
        )
        .expect("verify individual vote");

        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: execution_commitment(12),
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: execution_commitment(12),
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&refs)
                .expect("aggregate BLS votes"),
        };
        verify_authenticated_message(
            &context,
            None,
            &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                certificate,
            )),
            &pops,
        )
        .expect("verify aggregate QC");
    }

    #[cfg(feature = "bls")]
    #[test]
    fn timeout_vote_installs_embedded_qc_before_forming_tc() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys, pops) = authenticated_context();
        let verified_context =
            VerifiedHeightContext::genesis(context.clone(), pops).expect("verify context");
        let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("timeout-safety.wal"),
            verified_context,
            None,
            reducer::Generation::new(1),
            [0x33; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("open observing adapter");
        assert!(startup.is_empty());

        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = subject(13);
        let prepare_preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: execution_commitment(13),
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let prepare_shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &prepare_preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let prepare_refs = prepare_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: execution_commitment(13),
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&prepare_refs)
                .expect("aggregate PrepareQC"),
        };
        let manifest =
            wire::PayloadManifest::derive(&context, round, subject, 5, &[b"chunk".to_vec()])
                .expect("valid protected-body manifest");
        let core_manifest = adapter
            .registry
            .manifest_to_core(&manifest, &context)
            .expect("register protected-body manifest");
        let core_round = reducer::Round::new(round.height, round.view);
        let core_subject = core_manifest.subject();
        let original_tag = adapter.current_tag();

        let mut all_effects = Vec::new();
        for signer in 0_u32..3 {
            if signer == 2 {
                adapter.deferred_completions.push_back(DeferredInput {
                    admission_ordinal: 1,
                    admission_capability: DeferredAdmissionCapability::for_test(1),
                    event: reducer::Event::BodyAvailable {
                        tag: original_tag,
                        round: core_round,
                        subject: core_subject,
                    },
                    completion_evidence: Some(BodyPipelineCompletionEvidence::BodyAvailable {
                        manifest: manifest.clone(),
                    }),
                    retag_authenticated_ingress: false,
                    priority: DeferredPriority::Completion,
                    protected_progress: false,
                    admission: None,
                    authenticated_wire_identity: None,
                    admitted_at: Instant::now(),
                    eligible_skips: 0,
                });
            }
            let mut timeout = wire::TimeoutVote {
                round,
                highest_prepare_qc: Some(prepare.clone()),
                signer,
                signature: Vec::new(),
            };
            timeout.signature = Signature::new(
                keys[usize::try_from(signer).expect("small signer")].private_key(),
                &timeout.signature_preimage(),
            )
            .payload()
            .to_vec();
            let authenticated = adapter
                .authenticate(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::TimeoutVote(timeout),
                ))
                .expect("authenticate self-contained timeout vote");
            all_effects.push(
                adapter
                    .receive_authenticated(authenticated)
                    .expect("ingest timeout vote")
                    .into_effects(),
            );
        }
        let final_effects = all_effects.pop().expect("three timeout outcomes");

        assert_eq!(adapter.reducer.durable_state().current_view(), 1);
        assert!(adapter.reducer.durable_state().highest_prepare().is_some());
        assert!(final_effects.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(_),
                ..
            })
        )));
        assert!(
            final_effects
                .iter()
                .any(|effect| matches!(effect, AdapterEffect::EnterView { .. }))
        );
        assert!(
            !final_effects
                .iter()
                .any(|effect| matches!(effect, AdapterEffect::StoreBody { .. })),
            "old-generation BodyAvailable must not cross EnterView before executor rebinding"
        );
        let (rebound_tag, protected_body) = final_effects
            .iter()
            .find_map(|effect| match effect {
                AdapterEffect::EnterView {
                    tag,
                    protected_body,
                    ..
                } => Some((*tag, *protected_body)),
                _ => None,
            })
            .expect("view installation effect");
        assert_eq!(protected_body, Some((round, subject)));
        assert!(matches!(
            adapter.deferred_completions.front(),
            Some(DeferredInput {
                event: reducer::Event::BodyAvailable { tag, round, subject },
                ..
            }) if *tag == original_tag && *round == core_round && *subject == core_subject
        ));
        assert_eq!(
            adapter.rebind_deferred_body_available(original_tag, rebound_tag, &manifest),
            1
        );
        assert!(matches!(
            adapter.deferred_completions.front(),
            Some(DeferredInput {
                event: reducer::Event::BodyAvailable { tag, .. },
                ..
            }) if *tag == rebound_tag
        ));
        assert_eq!(
            adapter.retire_deferred_body_available(rebound_tag, &manifest),
            1
        );
        assert!(adapter.deferred_completions.is_empty());
    }
}
