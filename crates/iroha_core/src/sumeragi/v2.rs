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
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
};

use iroha_crypto::{Algorithm, Hash, HashOf, Signature};
use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};
use iroha_sumeragi_core as reducer;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use super::{
    safety_wal::{SafetyWal, SafetyWalError},
    v2_body_store::{DurableBodyReceipt, ValidatedBodyReceipt},
};
use crate::kura::KuraV2CommitReceipt;

const AGGREGATE_TOKEN_PREFIX: &[u8] = b"sumeragi-v2:verified-aggregate\0";
const MAX_DEFERRED_INPUTS: usize = 1024;
const MAX_DEFERRED_PROGRESS_INPUTS: usize = 256;
const MAX_INGRESS_SEMANTIC_KEYS: usize = 1024;

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
    locked_subject: Option<wire::BlockSubject>,
    decided_subject: Option<wire::BlockSubject>,
}

impl LocalProposalDirective {
    /// Exact height/view/generation which owns candidate work.
    pub(crate) const fn tag(self) -> reducer::EventTag {
        self.tag
    }

    /// Frozen-roster validator expected to propose in this view.
    pub(crate) const fn leader(self) -> wire::ValidatorIndex {
        self.leader
    }

    /// Subject which must be re-proposed while the local lock remains active.
    pub(crate) const fn locked_subject(self) -> Option<wire::BlockSubject> {
        self.locked_subject
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
        if context.height != 1 || context.parent_commit_qc.is_some() {
            return Err(AdapterError::InvalidGenesisContext);
        }
        verify_roster_proofs(&context, &proofs_of_possession)?;
        Ok(Self {
            context,
            proofs_of_possession,
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
            || parent_qc.subject != parent_artifact.subject
            || parent_receipt.height() != parent_artifact.height
            || parent_receipt.context_id() != parent_artifact.context_id()
            || parent_receipt.block_hash() != parent_artifact.block_hash
            || parent_receipt.artifact_hash() != HashOf::new(parent_artifact)
        {
            return Err(AdapterError::ParentContextMismatch);
        }
        if let Some(snapshot) = &parent_artifact.height_context.next_epoch_snapshot {
            if context.epoch != snapshot.epoch
                || context.mode != snapshot.mode
                || context.roster != snapshot.roster
                || context.quorum != snapshot.quorum
                || context.leader_seed != snapshot.leader_seed
            {
                return Err(AdapterError::EpochTransitionMismatch);
            }
        } else if context.epoch != parent_artifact.height_context.epoch
            || context.epoch_end_height != parent_artifact.height_context.epoch_end_height
            || context.roster != parent_artifact.height_context.roster
            || context.quorum != parent_artifact.height_context.quorum
            || context.leader_seed != parent_artifact.height_context.leader_seed
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
    /// Reset the round timer after a persisted timeout certificate advances view.
    EnterView {
        /// New reducer incarnation tag.
        tag: reducer::EventTag,
        /// Canonical certificate authorizing the new view.
        certificate: wire::TimeoutCertificate,
    },
    /// Report authenticated equivocation to evidence handling.
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
}

/// A consumed reducer height whose exact decision is durable in Kura.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FinalizedV2Height {
    context: wire::HeightContext,
    decision: wire::QuorumCertificate,
    wal_retirement_warning: Option<String>,
}

impl FinalizedV2Height {
    /// Frozen wire context which governed the finalized height.
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        &self.context
    }

    /// Exact cryptographically verified CommitQC stored by Kura.
    pub(crate) const fn decision(&self) -> &wire::QuorumCertificate {
        &self.decision
    }

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
pub(crate) struct AuthenticatedConsensusMessage(wire::ConsensusMessageV2);

#[derive(Clone, Debug, PartialEq, Eq)]
struct DeferredInput {
    event: reducer::Event,
    retag_authenticated_ingress: bool,
    priority: DeferredPriority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeferredPriority {
    /// Trusted completions of operations already requested by the reducer.
    Completion,
    /// Validated QCs and TCs which can finalize or advance the protocol.
    Progress,
    /// Proposals and individual control votes.
    Normal,
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
    Vote(wire::BlockSubject),
    TimeoutVote(Option<wire::QuorumCertificateRef>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IngressAdmissionRecord {
    fingerprint: IngressFingerprint,
    equivocation_reported: bool,
}

impl AdapterOutcome {
    /// Return whether the reducer applied or deliberately ignored the input.
    pub(crate) const fn disposition(&self) -> reducer::StepDisposition {
        self.disposition
    }

    /// Borrow the effects now safe for asynchronous execution.
    pub(crate) fn effects(&self) -> &[AdapterEffect] {
        &self.effects
    }

    /// Consume the outcome and return its asynchronous effects.
    pub(crate) fn into_effects(self) -> Vec<AdapterEffect> {
        self.effects
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
    /// Kura finality artifact failed structural validation.
    #[error("invalid Sumeragi v2 Kura finality artifact: {0}")]
    FinalityArtifact(#[from] wire::finality::V2FinalityValidationError),
    /// Kura's typed receipt or artifact differs from the reducer's exact decision.
    #[error("Sumeragi v2 Kura finality receipt does not match the applied reducer decision")]
    DurableCommitMismatch,
    /// Body-store receipt differs from the exact manifest, round, or subject.
    #[error("Sumeragi v2 durable body receipt does not match the reducer work item")]
    DurableBodyMismatch,
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
    registry: WireRegistry,
    fingerprints: AdapterFingerprints,
    aggregator: Box<dyn SignatureAggregator>,
    active_subject: Option<(reducer::Round, reducer::Subject)>,
    pending_persistence_id: Option<u64>,
    ingress_admission: BTreeMap<IngressSemanticKey, IngressAdmissionRecord>,
    deferred_completions: VecDeque<DeferredInput>,
    deferred_progress_inputs: VecDeque<DeferredInput>,
    deferred_inputs: VecDeque<DeferredInput>,
    replay_complete: bool,
    fail_closed: bool,
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
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        Self::open_with_aggregator(
            wal_path,
            verified_context,
            local_validator,
            generation,
            consensus_key_hash,
            fingerprints,
            Box::<BlsNormalSignatureAggregator>::default(),
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
    ) -> Result<(Self, Vec<AdapterEffect>), AdapterError> {
        let VerifiedHeightContext {
            context: wire_context,
            proofs_of_possession,
            parent_verification,
        } = verified_context;
        let mut registry = WireRegistry::new(&wire_context)?;
        let context = registry.core_context(&wire_context)?;
        let local_validator = local_validator
            .map(|index| registry.validator_id(index))
            .transpose()?;
        let chain_hash: [u8; 32] = Hash::new(wire_context.chain_id.encode()).into();
        let wal = SafetyWal::open(
            wal_path,
            wire::PROTOCOL_VERSION,
            chain_hash,
            consensus_key_hash,
        )?;

        let entries = wal
            .recovered_records()
            .iter()
            .map(|record| registry.decode_wal_entry(record.sequence, &record.payload))
            .collect::<Result<Vec<_>, _>>()?;
        let reducer = reducer::Reducer::recover(context, local_validator, generation, entries)?;
        let mut adapter = Self {
            wire_context,
            proofs_of_possession,
            parent_verification,
            reducer,
            wal,
            registry,
            fingerprints,
            aggregator,
            active_subject: None,
            pending_persistence_id: None,
            ingress_admission: BTreeMap::new(),
            deferred_completions: VecDeque::new(),
            deferred_progress_inputs: VecDeque::new(),
            deferred_inputs: VecDeque::new(),
            replay_complete: false,
            fail_closed: false,
        };
        let startup = adapter.reducer.resume_after_replay();
        let startup = adapter.drive_effects(startup)?;
        adapter.replay_complete = true;
        adapter.publish_status()?;
        Ok((adapter, startup))
    }

    /// Return the tag which must accompany a new asynchronous operation.
    pub(crate) const fn current_tag(&self) -> reducer::EventTag {
        self.reducer.current_tag()
    }

    /// Snapshot the exact reducer-owned facts which constrain local proposal
    /// construction. Proposal justification remains internal to the reducer.
    pub(crate) fn local_proposal_directive(&self) -> Result<LocalProposalDirective, AdapterError> {
        let durable = self.reducer.durable_state();
        let view = durable.current_view();
        let leader = self
            .registry
            .validator_index(self.reducer.context().leader(view))?;
        let locked_subject = durable
            .locked()
            .map(|certificate| self.registry.subject(certificate.subject()))
            .transpose()?;
        let decided_subject = durable
            .decision()
            .map(|certificate| self.registry.subject(certificate.subject()))
            .transpose()?;
        Ok(LocalProposalDirective {
            tag: self.reducer.current_tag(),
            leader,
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
    ) -> Result<Option<(wire::ConsensusRound, wire::BlockSubject)>, AdapterError> {
        self.reducer
            .durable_state()
            .decision()
            .map(|certificate| {
                Ok((
                    self.registry.round_to_wire(certificate.round()),
                    self.registry.subject(certificate.subject())?,
                ))
            })
            .transpose()
    }

    /// Return whether WAL replay completed and authenticated ingress may open.
    pub(crate) const fn ingress_ready(&self) -> bool {
        self.replay_complete && !self.fail_closed
    }

    /// Return whether application completed and no unfinished safety write or
    /// signature remains before height rollover.
    pub(crate) fn ready_to_finish(&self) -> bool {
        self.ingress_ready() && self.reducer.ready_to_finish()
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
        Ok(AuthenticatedConsensusMessage(message))
    }

    #[allow(clippy::too_many_lines)]
    fn admit_authenticated_payload(
        &mut self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> Result<(Option<AdapterOutcome>, Option<IngressSemanticKey>), AdapterError> {
        let current_view = self.reducer.current_tag().view();
        // Retain individual Commit/Prepare vote keys for one complete leader
        // rotation. Older CommitQCs remain admissible without restriction, but
        // retaining arbitrary old individual-vote keys would let one Byzantine
        // signer exhaust the per-height admission table after a long partition.
        let retained_vote_views = u64::try_from(self.wire_context.roster.len()).unwrap_or(u64::MAX);
        let oldest_retained_view = current_view.saturating_sub(retained_vote_views);
        self.ingress_admission
            .retain(|key, _| key.round().view >= oldest_retained_view);
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
                if vote.round.view > current_view || vote.round.view < oldest_retained_view {
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
                    IngressFingerprint::Vote(vote.subject),
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
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
                return Ok((None, None));
            }
        };

        if let Some(record) = self.ingress_admission.get_mut(&key) {
            if record.fingerprint == fingerprint {
                return Ok((
                    Some(Self::ignored_outcome(reducer::IgnoreReason::Duplicate)),
                    None,
                ));
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
                }),
                None,
            ));
        }

        if self.ingress_admission.len() >= MAX_INGRESS_SEMANTIC_KEYS {
            // This is bounded backpressure for non-certificate traffic. QCs and
            // TCs bypass this table and use the reserved progress queue below.
            return Ok((
                Some(Self::ignored_outcome(reducer::IgnoreReason::Busy)),
                None,
            ));
        }
        self.ingress_admission.insert(
            key,
            IngressAdmissionRecord {
                fingerprint,
                equivocation_reported: false,
            },
        );
        Ok((None, Some(key)))
    }

    fn ignored_outcome(reason: reducer::IgnoreReason) -> AdapterOutcome {
        AdapterOutcome {
            disposition: reducer::StepDisposition::Ignored(reason),
            effects: Vec::new(),
        }
    }

    /// Feed a signature-checked and structurally verified canonical message.
    fn receive_verified(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        message.validate_version()?;
        let (outcome, inserted_admission) = self.admit_authenticated_payload(&message.payload)?;
        if let Some(outcome) = outcome {
            return Ok(outcome);
        }
        let result = self.receive_admitted_payload(message.payload);
        if result.is_err()
            && let Some(key) = inserted_admission
        {
            self.ingress_admission.remove(&key);
        }
        result
    }

    fn receive_admitted_payload(
        &mut self,
        payload: wire::ConsensusMessageV2Payload,
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
                );
            }
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                let vote = registry.vote_to_core(&vote, &self.wire_context)?;
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::VoteReceived { tag, vote },
                    None,
                );
            }
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                let certificate = registry.qc_to_core(&certificate, &self.wire_context)?;
                let active_subject = Some((certificate.round(), certificate.subject()));
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::QuorumCertificateReceived { tag, certificate },
                    active_subject,
                );
            }
            wire::ConsensusMessageV2Payload::TimeoutVote(vote) => {
                let (vote, highest) = registry.timeout_vote_to_core(&vote, &self.wire_context)?;
                self.registry = registry;
                let result = self.receive_verified_timeout_vote(tag, vote, highest);
                if result.is_err() {
                    self.fail_closed = true;
                }
                return result;
            }
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                let certificate = registry.tc_to_core(&certificate, &self.wire_context)?;
                return self.dispatch_staged_authenticated_ingress(
                    registry,
                    reducer::Event::TimeoutCertificateReceived { tag, certificate },
                    None,
                );
            }
            wire::ConsensusMessageV2Payload::PayloadManifest(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
                return Err(AdapterError::TransportPayload);
            }
        }
    }

    fn dispatch_staged_authenticated_ingress(
        &mut self,
        registry: WireRegistry,
        event: reducer::Event,
        active_subject: Option<(reducer::Round, reducer::Subject)>,
    ) -> Result<AdapterOutcome, AdapterError> {
        let previous_registry = core::mem::replace(&mut self.registry, registry);
        let previous_active_subject = self.active_subject;
        if let Some(active_subject) = active_subject {
            self.active_subject = Some(active_subject);
        }
        let result = self.step_authenticated_ingress(event);
        if result.is_err() {
            // A reducer failure after conversion may have partially consumed an
            // authenticated transition. Keep its registry expansion aligned
            // with reducer state and require WAL replay before further ingress.
            self.fail_closed = true;
            return result;
        }
        let retain = result.as_ref().is_ok_and(|outcome| {
            matches!(
                outcome.disposition(),
                reducer::StepDisposition::Applied
                    | reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
            )
        });
        if !retain {
            self.registry = previous_registry;
            self.active_subject = previous_active_subject;
        }
        result
    }

    fn receive_verified_timeout_vote(
        &mut self,
        tag: reducer::EventTag,
        vote: reducer::SignedTimeoutVote,
        highest: Option<reducer::QuorumCertificate>,
    ) -> Result<AdapterOutcome, AdapterError> {
        let mut effects = Vec::new();
        if let Some(certificate) = highest {
            effects.extend(
                self.step_authenticated_ingress(reducer::Event::QuorumCertificateReceived {
                    tag,
                    certificate,
                })?
                .into_effects(),
            );
        }
        let outcome =
            self.step_authenticated_ingress(reducer::Event::TimeoutVoteReceived { tag, vote })?;
        let disposition = outcome.disposition();
        effects.extend(outcome.into_effects());
        Ok(AdapterOutcome {
            disposition,
            effects,
        })
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
    /// Only the expected leader can take this transition.  The reducer first
    /// persists a proposal intent; the returned signing request is therefore
    /// safe to execute immediately.
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
        let core_manifest = self
            .registry
            .manifest_to_core(&manifest, &self.wire_context)?;
        let round = self
            .registry
            .round_to_core(manifest.round, &self.wire_context)?;
        let subject = core_manifest.subject();
        self.active_subject = Some((round, subject));
        self.step(reducer::Event::LocalProposalReady {
            tag,
            manifest: core_manifest,
        })
    }

    /// Complete a body reconstruction requested by [`AdapterEffect::FetchBody`].
    pub(crate) fn body_available(
        &mut self,
        tag: reducer::EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        let round = self
            .registry
            .round_to_core(manifest.round, &self.wire_context)?;
        let subject = self.registry.register_subject(manifest.subject)?;
        let core_manifest = self
            .registry
            .manifest_to_core(&manifest, &self.wire_context)?;
        if core_manifest.subject() != subject {
            return Err(AdapterError::DurableBodyMismatch);
        }
        self.step(reducer::Event::BodyAvailable {
            tag,
            round,
            subject,
        })
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
        self.step(reducer::Event::BodyStored {
            tag,
            round,
            subject,
        })
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
        self.step(reducer::Event::ValidationCompleted {
            tag,
            round,
            subject,
            valid: true,
        })
    }

    /// Report deterministic rejection of a durable body. A rejection cannot
    /// authorize a vote, so it requires no success receipt.
    pub(crate) fn validation_failed(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<AdapterOutcome, AdapterError> {
        let round = self.registry.round_to_core(round, &self.wire_context)?;
        let subject = self.registry.register_subject(subject)?;
        self.step(reducer::Event::ValidationCompleted {
            tag,
            round,
            subject,
            valid: false,
        })
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
        artifact.validate()?;
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
        let _closed = self.reducer.finish_height(reducer_receipt)?;
        let wal_retirement_warning = self.wal.retire().err().map(|error| error.to_string());
        Ok(FinalizedV2Height {
            context: self.wire_context,
            decision: wire_decision,
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
        let (last_committed_height, last_committed_subject) = if let Some(certificate) = &decision {
            (
                certificate.round().height(),
                Some(self.registry.subject(certificate.subject())?),
            )
        } else if let Some(parent) = &self.wire_context.parent_commit_qc {
            (parent.round.height, Some(parent.subject))
        } else {
            (0, None)
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

        Ok(wire::SumeragiV2Status {
            protocol_version: wire::PROTOCOL_VERSION,
            node_fingerprint: self.fingerprints.node,
            build_fingerprint: self.fingerprints.build,
            config_fingerprint: self.fingerprints.config,
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
        })
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
        let priority = match &event {
            reducer::Event::BodyAvailable { .. }
            | reducer::Event::BodyStored { .. }
            | reducer::Event::ValidationCompleted { .. }
            | reducer::Event::Persisted { .. }
            | reducer::Event::PersistenceFailed { .. }
            | reducer::Event::Signed { .. }
            | reducer::Event::ApplicationCompleted { .. } => DeferredPriority::Completion,
            reducer::Event::LocalProposalReady { .. }
            | reducer::Event::ProposalReceived { .. }
            | reducer::Event::VoteReceived { .. }
            | reducer::Event::QuorumCertificateReceived { .. }
            | reducer::Event::TimeoutVoteReceived { .. }
            | reducer::Event::TimeoutCertificateReceived { .. }
            | reducer::Event::TimeoutElapsed { .. }
            | reducer::Event::RetransmitElapsed { .. } => DeferredPriority::Normal,
        };
        self.step_with_defer_policy(event, false, priority)
    }

    fn step_authenticated_ingress(
        &mut self,
        event: reducer::Event,
    ) -> Result<AdapterOutcome, AdapterError> {
        let priority = if matches!(
            &event,
            reducer::Event::QuorumCertificateReceived { .. }
                | reducer::Event::TimeoutCertificateReceived { .. }
        ) {
            DeferredPriority::Progress
        } else {
            DeferredPriority::Normal
        };
        self.step_with_defer_policy(event, true, priority)
    }

    fn step_with_defer_policy(
        &mut self,
        event: reducer::Event,
        retag_authenticated_ingress: bool,
        priority: DeferredPriority,
    ) -> Result<AdapterOutcome, AdapterError> {
        self.ensure_ingress()?;
        let queued = event.clone();
        let outcome = self.reducer.step(event)?;
        let disposition = outcome.disposition();
        if disposition == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy) {
            self.enqueue_deferred(queued, retag_authenticated_ingress, priority);
            self.publish_status()?;
            return Ok(AdapterOutcome {
                disposition,
                effects: Vec::new(),
            });
        }
        let mut effects = self.drive_effects(outcome.into_effects())?;
        effects.extend(self.drain_deferred()?);
        self.publish_status()?;
        Ok(AdapterOutcome {
            disposition,
            effects,
        })
    }

    fn enqueue_deferred(
        &mut self,
        event: reducer::Event,
        retag_authenticated_ingress: bool,
        priority: DeferredPriority,
    ) {
        let input = DeferredInput {
            event,
            retag_authenticated_ingress,
            priority,
        };
        let queue = match priority {
            DeferredPriority::Completion => &mut self.deferred_completions,
            DeferredPriority::Progress => &mut self.deferred_progress_inputs,
            DeferredPriority::Normal => &mut self.deferred_inputs,
        };
        if queue.contains(&input) {
            return;
        }
        match priority {
            DeferredPriority::Completion => {
                // Adapter completions correspond to already outstanding work;
                // untrusted network traffic cannot consume this reserved lane.
                queue.push_back(input);
            }
            DeferredPriority::Progress => {
                if queue.len() >= MAX_DEFERRED_PROGRESS_INPUTS {
                    let incoming_rank = progress_rank(&input.event);
                    let replace = queue
                        .iter()
                        .position(|queued| progress_rank(&queued.event) <= incoming_rank);
                    let Some(replace) = replace else {
                        return;
                    };
                    queue.remove(replace);
                }
                queue.push_back(input);
            }
            DeferredPriority::Normal => {
                if queue.len() < MAX_DEFERRED_INPUTS {
                    queue.push_back(input);
                }
            }
        }
    }

    fn drain_deferred(&mut self) -> Result<Vec<AdapterEffect>, AdapterError> {
        let mut ready = Vec::new();
        while let Some(mut input) = self
            .deferred_completions
            .pop_front()
            .or_else(|| self.deferred_progress_inputs.pop_front())
            .or_else(|| self.deferred_inputs.pop_front())
        {
            if input.retag_authenticated_ingress {
                input.event = input
                    .event
                    .retag_authenticated_ingress(self.reducer.current_tag());
            }
            let retry = input.clone();
            let event = input.event;
            let outcome = self.reducer.step(event)?;
            if outcome.disposition()
                == reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
            {
                match retry.priority {
                    DeferredPriority::Completion => self.deferred_completions.push_front(retry),
                    DeferredPriority::Progress => {
                        self.deferred_progress_inputs.push_front(retry);
                    }
                    DeferredPriority::Normal => self.deferred_inputs.push_front(retry),
                }
                break;
            }
            ready.extend(self.drive_effects(outcome.into_effects())?);
        }
        Ok(ready)
    }

    fn publish_status(&mut self) -> Result<(), AdapterError> {
        let status = self.status()?;
        super::status::set_v2_status(status);
        Ok(())
    }

    fn drive_effects(
        &mut self,
        effects: Vec<reducer::Effect>,
    ) -> Result<Vec<AdapterEffect>, AdapterError> {
        let mut pending = VecDeque::from(effects);
        let mut ready = Vec::new();
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
                    let continuation =
                        match self.reducer.step(reducer::Event::Persisted { tag, id }) {
                            Ok(continuation) => continuation,
                            Err(error) => {
                                // The physical WAL is now ahead of memory. Only a
                                // clean reopen/replay may reconcile that state.
                                self.fail_closed = true;
                                return Err(error.into());
                            }
                        };
                    for effect in continuation.into_effects().into_iter().rev() {
                        pending.push_front(effect);
                    }
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
                    reducer::SignableMessage::TimeoutVote(vote) => {
                        SignRequest::TimeoutVote(self.registry.unsigned_timeout_vote_to_wire(vote)?)
                    }
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
            reducer::Effect::EnterView { tag, certificate } => Ok(AdapterEffect::EnterView {
                tag,
                certificate: self
                    .registry
                    .tc_to_wire(&certificate, self.aggregator.as_ref())?,
            }),
            reducer::Effect::ReportEquivocation {
                offender,
                round,
                kind,
            } => Ok(AdapterEffect::ReportEquivocation {
                offender: self.registry.peer(offender)?,
                round: self.registry.round_to_wire(round),
                kind,
            }),
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

fn progress_rank(event: &reducer::Event) -> u8 {
    match event {
        reducer::Event::QuorumCertificateReceived { certificate, .. }
            if certificate.phase() == reducer::Phase::Commit =>
        {
            3
        }
        reducer::Event::TimeoutCertificateReceived { .. } => 2,
        reducer::Event::QuorumCertificateReceived { .. } => 1,
        reducer::Event::LocalProposalReady { .. }
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
    certificates: BTreeMap<reducer::CertificateRef, wire::QuorumCertificate>,
    timeouts: BTreeMap<reducer::Round, wire::TimeoutCertificate>,
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
            .map(|certificate| self.qc_reference_to_core(&certificate.as_ref()))
            .transpose()?;
        if let Some(certificate) = &context.parent_commit_qc {
            self.register_qc(certificate)?;
        }
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
        reducer::HeightContext::new(
            context_id(context.id()),
            reducer::ChainId::new(Hash::new(context.chain_id.encode()).into()),
            context.height,
            parent_commit,
            context.epoch,
            roster,
            mode,
            reducer::Digest::new(*context.nexus_amx_context_hash.as_ref()),
            reducer::Digest::new(Hash::new(context.da_layout.encode()).into()),
            reducer::Digest::new(leader_height_seed.into()),
        )
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

    fn subject(&self, subject: reducer::Subject) -> Result<wire::BlockSubject, AdapterError> {
        self.subjects
            .get(&subject)
            .copied()
            .ok_or(AdapterError::UnknownSubject(subject))
    }

    fn round_to_core(
        &self,
        round: wire::ConsensusRound,
        context: &wire::HeightContext,
    ) -> Result<reducer::Round, AdapterError> {
        if round.context_id != context.id() || round.height != context.height {
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
        let round = self.round_to_core(vote.round, context)?;
        let subject = self.register_subject(vote.subject)?;
        let signer = self.validator_id(vote.signer)?;
        Ok(reducer::SignedVote::new(
            reducer::Vote::new(
                context_id(vote.round.context_id),
                round,
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
            phase: Self::phase_to_wire(vote.phase()),
            subject: self.subject(vote.subject())?,
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
        Ok(reducer::CertificateRef::new(
            context_id(reference.round.context_id),
            reducer::Round::new(reference.round.height, reference.round.view),
            Self::phase_to_core(reference.phase),
            self.register_subject(reference.subject)?,
        ))
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

    fn register_qc(
        &mut self,
        certificate: &wire::QuorumCertificate,
    ) -> Result<reducer::CertificateRef, AdapterError> {
        let reference = self.qc_reference_to_core(&certificate.as_ref())?;
        self.certificates.insert(reference, certificate.clone());
        Ok(reference)
    }

    fn qc_to_wire(
        &mut self,
        certificate: &reducer::QuorumCertificate,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::QuorumCertificate, AdapterError> {
        if let Some(cached) = self.certificates.get(&certificate.reference()) {
            return Ok(cached.clone());
        }
        let signers = certificate
            .signatures()
            .iter()
            .map(|share| self.validator_index(share.signer()))
            .collect::<Result<Vec<_>, _>>()?;
        let aggregate_signature = aggregate_core_shares(certificate.signatures(), aggregator)?;
        let wire = wire::QuorumCertificate {
            round: self.round_to_wire(certificate.round()),
            phase: Self::phase_to_wire(certificate.phase()),
            subject: self.subject(certificate.subject())?,
            signers,
            aggregate_signature,
        };
        self.certificates
            .insert(certificate.reference(), wire.clone());
        Ok(wire)
    }

    fn timeout_vote_to_core(
        &mut self,
        vote: &wire::TimeoutVote,
        context: &wire::HeightContext,
    ) -> Result<
        (
            reducer::SignedTimeoutVote,
            Option<reducer::QuorumCertificate>,
        ),
        AdapterError,
    > {
        vote.validate(context)?;
        let round = self.round_to_core(vote.round, context)?;
        let highest = vote
            .highest_prepare_qc
            .as_ref()
            .map(|certificate| self.qc_to_core(certificate, context))
            .transpose()?;
        Ok((
            reducer::SignedTimeoutVote::new(
                reducer::TimeoutVote::new(
                    context_id(vote.round.context_id),
                    round,
                    self.validator_id(vote.signer)?,
                    highest.as_ref().map(reducer::QuorumCertificate::reference),
                ),
                reducer::OpaqueSignature::new(vote.signature.clone()),
            ),
            highest,
        ))
    }

    fn unsigned_timeout_vote_to_wire(
        &mut self,
        vote: reducer::TimeoutVote,
    ) -> Result<wire::TimeoutVote, AdapterError> {
        let highest_prepare_qc = vote
            .highest_prepare()
            .map(|reference| {
                self.certificates
                    .get(&reference)
                    .cloned()
                    .ok_or(AdapterError::MissingCertificate)
            })
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
    ) -> Result<wire::TimeoutVote, AdapterError> {
        let mut wire = self.unsigned_timeout_vote_to_wire(vote.vote())?;
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
        self.timeouts.insert(round, certificate.clone());
        Ok(core)
    }

    fn tc_to_wire(
        &mut self,
        certificate: &reducer::TimeoutCertificate,
        aggregator: &dyn SignatureAggregator,
    ) -> Result<wire::TimeoutCertificate, AdapterError> {
        if let Some(cached) = self.timeouts.get(&certificate.round()) {
            return Ok(cached.clone());
        }
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
        self.timeouts.insert(certificate.round(), wire.clone());
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
        self.proposals.insert((round, subject), proposal.clone());
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
                    .map(|certificate| self.register_qc(certificate))
                    .transpose()?;
                Ok(reducer::ProposalJustification::ParentCommit(reference))
            }
            wire::ProposalJustification::Timeout(timeout) => {
                let certificate = self.tc_to_core(&timeout.timeout_certificate, context)?;
                let selected = timeout.timeout_certificate.highest_prepare_qc();
                if selected.map(wire::QuorumCertificate::as_ref)
                    != timeout
                        .highest_prepare_qc
                        .as_ref()
                        .map(wire::QuorumCertificate::as_ref)
                {
                    return Err(AdapterError::InvalidProposalJustification);
                }
                if let Some(highest) = &timeout.highest_prepare_qc {
                    self.qc_to_core(highest, context)?;
                }
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
                    self.signed_timeout_vote_to_wire(&vote)?,
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
                WalRecordV2::TimeoutIntent(self.unsigned_timeout_vote_to_wire(*vote)?)
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
                let core_round = round(vote.round)?;
                let subject = self.register_subject(vote.subject)?;
                reducer::WalRecord::PrepareIntent(reducer::Vote::new(
                    context_id(wire_context_id),
                    core_round,
                    Self::phase_to_core(vote.phase),
                    subject,
                    self.validator_id(vote.signer)?,
                ))
            }
            WalRecordV2::ObservePrepare(certificate) => {
                reducer::WalRecord::ObservePrepare(self.qc_to_core_unchecked(&certificate)?)
            }
            WalRecordV2::LockAndCommit { prepare, vote } => {
                let core_round = round(vote.round)?;
                let subject = self.register_subject(vote.subject)?;
                reducer::WalRecord::LockAndCommit {
                    prepare: self.qc_to_core_unchecked(&prepare)?,
                    vote: reducer::Vote::new(
                        context_id(wire_context_id),
                        core_round,
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
                    .map(|certificate| {
                        self.qc_to_core_unchecked(certificate)
                            .map(|certificate| certificate.reference())
                    })
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
        self.certificates.insert(reference, certificate.clone());
        Ok(reducer::QuorumCertificate::new(reference, signatures))
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
        self.timeouts.insert(round, certificate.clone());
        Ok(reducer::TimeoutCertificate::new(
            context_id(certificate.round.context_id),
            round,
            groups,
        ))
    }
}

fn verify_authenticated_message(
    context: &wire::HeightContext,
    parent_verification: Option<&ParentVerificationContext>,
    message: &wire::ConsensusMessageV2,
    proofs_of_possession: &[Vec<u8>],
) -> Result<(), AdapterError> {
    message.validate_version()?;
    context.validate()?;
    if proofs_of_possession.len() != context.roster.len() {
        return Err(AdapterError::ProofOfPossessionCount {
            expected: context.roster.len(),
            actual: proofs_of_possession.len(),
        });
    }
    validate_bls_roster(context)?;

    match &message.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => {
            proposal.validate(context)?;
            verify_individual_signature(
                context,
                proposal.proposer,
                &proposal.signature,
                &proposal.signature_preimage(),
            )?;
            match &proposal.justification {
                wire::ProposalJustification::ParentCommit(parent) => {
                    match (&parent.certificate, parent_verification) {
                        (None, None) if context.height == 1 => {}
                        (Some(certificate), Some(parent_verification)) => {
                            verify_quorum_certificate(
                                &parent_verification.context,
                                certificate,
                                &parent_verification.proofs_of_possession,
                            )?;
                        }
                        (None, None) | (None, Some(_)) | (Some(_), None) => {
                            return Err(AdapterError::ParentContextMismatch);
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
                }
            }
            Ok(())
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
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
            Err(AdapterError::TransportPayload)
        }
    }
}

fn validate_bls_roster(context: &wire::HeightContext) -> Result<(), AdapterError> {
    for entry in &context.roster {
        let algorithm = entry
            .validator
            .public_key()
            .try_algorithm()
            .map_err(|error| AdapterError::Cryptography(error.to_string()))?;
        if algorithm != Algorithm::BlsNormal {
            return Err(AdapterError::Cryptography(format!(
                "validator {} uses {algorithm:?}; expected BLS-normal",
                entry.validator
            )));
        }
    }
    Ok(())
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

/// Verify one certificate against an immutable context record reopened for
/// historical certified-body service.
///
/// This deliberately reuses the exact production roster-PoP and aggregate
/// verifier used by live reducer ingress; block sync does not maintain a
/// second certificate-validation implementation.
pub(crate) fn verify_persisted_quorum_certificate(
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
            .filter(|index| *index < context.roster.len())
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
    aggregator
        .aggregate(&signatures)
        .map_err(AdapterError::SignatureAggregation)
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Write as _};

    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use tempfile::TempDir;

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
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
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
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"authenticated nexus amx context"),
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
            phase: wire::GlobalPhase::Commit,
            subject: parent_subject,
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
            phase: wire::GlobalPhase::Commit,
            subject: parent_subject,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate parent CommitQC"),
        };
        let artifact = wire::finality::V2FinalityArtifact::new(
            parent_context.clone(),
            parent_subject,
            parent_qc.clone(),
            None,
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
            phase: wire::GlobalPhase::Commit,
            subject: parent_subject,
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
            phase: wire::GlobalPhase::Commit,
            subject: parent_subject,
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
        let (adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("successor-safety.wal"),
            verified_successor.clone(),
            None,
            reducer::Generation::new(2),
            [0x62; 32],
            fingerprints(),
            Box::new(TestAggregator),
        )
        .expect("open successor adapter");
        assert!(startup.is_empty());
        adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
            ))
            .expect("alternate-view parent CommitQC is cryptographically verified");
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
        )
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
        )
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
    fn replay_resigns_only_an_acknowledged_intent() {
        let directory = TempDir::new().expect("temporary directory");
        {
            let (mut adapter, _) = open_test(&directory).expect("open adapter");
            let proposer = adapter.status().expect("status").leader;
            let subject = subject(9);
            let proposal = proposal(&adapter.wire_context, proposer, subject);
            let effects = adapter
                .receive_verified(proposal)
                .expect("accept proposal")
                .into_effects();
            let (tag, manifest) = match effects.as_slice() {
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
            let receipt = durable_body_receipt(&adapter, round, subject);
            adapter
                .body_stored(tag, round, subject, &receipt)
                .expect("body stored");
            let validated = ValidatedBodyReceipt::for_test(receipt);
            let sign = adapter
                .validation_succeeded(tag, round, subject, &validated)
                .expect("body valid");
            assert!(matches!(sign.effects(), [AdapterEffect::Sign { .. }]));
        }

        let (adapter, startup) = open_test(&directory).expect("replay adapter");
        assert!(adapter.ingress_ready());
        assert!(matches!(startup.as_slice(), [AdapterEffect::Sign { .. }]));
        assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
    }

    #[test]
    fn replayed_decision_key_survives_incomplete_tail_and_rejects_key_drift() {
        let directory = TempDir::new().expect("temporary directory");
        let expected;
        {
            let (mut adapter, startup) = open_test(&directory).expect("open adapter");
            assert!(startup.is_empty());
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(b"pending Kura block")),
                payload_hash: Hash::new(b"pending exact body"),
            };
            let round = wire::ConsensusRound {
                context_id: adapter.wire_context.id(),
                height: adapter.wire_context.height,
                view: 0,
            };
            let decision = wire::QuorumCertificate {
                round,
                phase: wire::GlobalPhase::Commit,
                subject,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xD4; 48],
            };
            let record = WalEnvelopeV2 {
                protocol_version: wire::PROTOCOL_VERSION,
                persistence_id: 1,
                record: WalRecordV2::Decision(decision),
            }
            .encode();
            adapter
                .wal
                .append(&record)
                .expect("append acknowledged Decision record");
            expected = (round, subject);
        }
        OpenOptions::new()
            .append(true)
            .open(directory.path().join("safety.wal"))
            .expect("open WAL tail")
            .write_all(b"S2FR\x01\x00")
            .expect("model incomplete next frame");

        let (adapter, startup) = open_test(&directory).expect("replay durable Decision");
        assert!(matches!(
            startup.as_slice(),
            [AdapterEffect::FetchBody {
                certificate: Some(_),
                ..
            }]
        ));
        assert_eq!(
            adapter
                .replayed_decision_key()
                .expect("map replayed Decision"),
            Some(expected)
        );
        drop(adapter);

        assert!(matches!(
            SumeragiV2Adapter::open_with_aggregator(
                directory.path().join("safety.wal"),
                verified_genesis(context()),
                Some(0),
                reducer::Generation::new(1),
                [0x99; 32],
                fingerprints(),
                Box::new(TestAggregator),
            ),
            Err(AdapterError::SafetyWal(SafetyWalError::IdentityMismatch {
                field: "consensus key hash",
                ..
            }))
        ));
    }

    #[test]
    fn verified_aggregate_qc_roundtrips_without_reaggregation() {
        let context = context();
        let mut registry = WireRegistry::new(&context).expect("registry");
        let subject = subject(3);
        let certificate = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            },
            phase: wire::GlobalPhase::Prepare,
            subject,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xAA; 96],
        };
        let core = registry
            .qc_to_core(&certificate, &context)
            .expect("convert verified QC");
        let roundtrip = registry
            .qc_to_wire(&core, &TestAggregator)
            .expect("convert QC to wire");
        assert_eq!(roundtrip, certificate);
    }

    #[test]
    fn self_contained_grouped_timeout_certificate_roundtrips() {
        let context = context();
        let mut registry = WireRegistry::new(&context).expect("registry");
        let subject = subject(5);
        let prepare = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            },
            phase: wire::GlobalPhase::Prepare,
            subject,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xAB; 96],
        };
        let certificate = wire::TimeoutCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 3,
            },
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xBC; 96],
            }],
        };
        let core = registry
            .tc_to_core(&certificate, &context)
            .expect("convert verified TC");
        let roundtrip = registry
            .tc_to_wire(&core, &TestAggregator)
            .expect("convert TC to wire");
        assert_eq!(roundtrip, certificate);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn equivocation_flood_is_bounded_and_cannot_starve_commit_qc() {
        fn flood_subject(counter: u64) -> wire::BlockSubject {
            let mut bytes = [0_u8; 9];
            bytes[..8].copy_from_slice(&counter.to_le_bytes());
            bytes[8] = 0;
            let parent_block_hash = HashOf::from_untyped_unchecked(Hash::new(bytes));
            bytes[8] = 1;
            let block_hash = HashOf::from_untyped_unchecked(Hash::new(bytes));
            bytes[8] = 2;
            wire::BlockSubject {
                parent_block_hash: Some(parent_block_hash),
                block_hash,
                payload_hash: Hash::new(bytes),
            }
        }

        let directory = TempDir::new().expect("temporary directory");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());

        // Drive the local node to an outstanding Prepare signature. Authenticated
        // network inputs now exercise the adapter's deferred queues.
        let proposer = adapter.status().expect("status").leader;
        let decided_subject = subject(0xD0);
        let proposal = proposal(&adapter.wire_context, proposer, decided_subject);
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
        adapter
            .body_available(tag, manifest)
            .expect("body available");
        let receipt = durable_body_receipt(&adapter, round, decided_subject);
        adapter
            .body_stored(tag, round, decided_subject, &receipt)
            .expect("body stored");
        let validated = ValidatedBodyReceipt::for_test(receipt);
        let sign = adapter
            .validation_succeeded(tag, round, decided_subject, &validated)
            .expect("body valid")
            .into_effects();
        let sign_tag = match sign.as_slice() {
            [AdapterEffect::Sign { tag, .. }] => *tag,
            effects => panic!("unexpected validation effects: {effects:?}"),
        };

        let first_vote = wire::Vote {
            round,
            phase: wire::GlobalPhase::Prepare,
            subject: flood_subject(0),
            signer: 1,
            signature: vec![0x41],
        };
        let first = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(first_vote),
            ))
            .expect("defer first vote");
        assert_eq!(
            first.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_inputs.len(), 1);

        let mut evidence_reports = 0_usize;
        let flood_size = u64::try_from(MAX_DEFERRED_INPUTS).expect("queue bound fits u64") + 128;
        for counter in 1..=flood_size {
            let vote = wire::Vote {
                round,
                phase: wire::GlobalPhase::Prepare,
                subject: flood_subject(counter),
                signer: 1,
                signature: vec![0x42],
            };
            let outcome = adapter
                .receive_verified(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(vote),
                ))
                .expect("equivocation admission stays live");
            evidence_reports += outcome
                .effects()
                .iter()
                .filter(|effect| matches!(effect, AdapterEffect::ReportEquivocation { .. }))
                .count();
        }
        assert_eq!(evidence_reports, 1, "evidence is capped per semantic key");
        assert_eq!(adapter.deferred_inputs.len(), 1);
        assert_eq!(adapter.ingress_admission.len(), 2);
        assert!(adapter.registry.subjects.len() <= 2);

        // A valid CommitQC bypasses normal admission and receives a reserved,
        // higher-priority deferred slot even while the signer is outstanding.
        let commit_qc = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Commit,
            subject: decided_subject,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC0; 96],
        };
        let commit = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(commit_qc),
            ))
            .expect("defer CommitQC");
        assert_eq!(
            commit.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.deferred_progress_inputs.len(), 1);

        let completed = adapter
            .signature_completed(sign_tag, vec![0xD1; 96])
            .expect("complete outstanding Prepare signature")
            .into_effects();
        assert!(completed.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Apply { subject, .. } if *subject == decided_subject
        )));
        let decided_subject = adapter
            .registry
            .register_subject(decided_subject)
            .expect("subject");
        assert_eq!(
            adapter
                .reducer
                .durable_state()
                .decision()
                .map(reducer::QuorumCertificate::subject),
            Some(decided_subject)
        );
        assert!(adapter.deferred_progress_inputs.is_empty());
        assert!(adapter.deferred_inputs.is_empty());
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
        assert!(adapter.ingress_admission.is_empty());
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
            phase: wire::GlobalPhase::Prepare,
            subject,
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
            phase: wire::GlobalPhase::Prepare,
            subject,
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
            phase: wire::GlobalPhase::Prepare,
            subject,
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
            phase: wire::GlobalPhase::Prepare,
            subject,
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
            phase: wire::GlobalPhase::Prepare,
            subject,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&prepare_refs)
                .expect("aggregate PrepareQC"),
        };

        let mut all_effects = Vec::new();
        for signer in 0_u32..3 {
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
    }
}
