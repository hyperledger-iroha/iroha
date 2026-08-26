use super::{
    CertificateRef, Committee, CommitteeRole, ConsensusMessageV2, DurableState, EventTag,
    Generation, HeightContext, MAX_VOTING_ROSTER_LEN, OpaqueSignature, PayloadManifest,
    PersistenceId, Phase, Proposal, ProposalJustification, Quorum, QuorumCertificate, QuorumError,
    ReplayError, Round, SignatureShare, SignedProposal, SignedTimeoutVote, SignedVote, Subject,
    TimeoutCertificate, TimeoutSignatureGroup, TimeoutVote, ValidatorId, Vote, VotingPower,
    WalEntry, WalRecord,
    refinement::{
        self, BoundaryCapabilityKey, CERTIFICATE_EVIDENCE_ABSENT, CERTIFICATE_EVIDENCE_FOREIGN,
        CERTIFICATE_EVIDENCE_INCOMING, CERTIFICATE_EVIDENCE_LOCAL, CONTINUATION_DECIDE,
        CONTINUATION_INSTALL_TIMEOUT, CONTINUATION_NONE, CONTINUATION_SIGN,
        CanonicalIdentityProjection, CertificateIdentityProjection, EFFECT_APPLY, EFFECT_BROADCAST,
        EFFECT_ENTER_VIEW, EFFECT_FETCH, EFFECT_PERSIST, EFFECT_REPORT, EFFECT_SIGN, EFFECT_STORE,
        EFFECT_VALIDATE, EVENT_BODY_AVAILABLE, EVENT_BODY_STORED, EVENT_PERSISTED,
        EVENT_PERSISTENCE_FAILED, EVENT_RESUME_AFTER_REPLAY, EVENT_SIGNED, EffectCapabilityKey,
        EffectTrace, EnterViewProjection, IDENTITY_DOMAIN_CONTEXT, IDENTITY_DOMAIN_SUBJECT,
        IDENTITY_KIND_CONSENSUS_CONTEXT, IDENTITY_KIND_CONSENSUS_SUBJECT,
        LockedCommitProgressWitnessProjection, PendingProjection,
        ProductionDurableIntentTraceProjection, REPLAY_EFFECT_COMMIT, REPLAY_EFFECT_DECISION,
        REPLAY_EFFECT_NONE, REPLAY_EFFECT_PREPARE, REPLAY_EFFECT_PROPOSAL, REPLAY_EFFECT_TIMEOUT,
        ReplayPlanProjection, SIGNED_MESSAGE_COMMIT, SIGNED_MESSAGE_NONE, SIGNED_MESSAGE_PREPARE,
        SIGNED_MESSAGE_PROPOSAL, SIGNED_MESSAGE_TIMEOUT, SafetyProjection, SubjectProjection,
        TagProjection, TimeoutIdentityProjection, TransitionProjection, ValidatorProjection,
        VolatileSummary, WAL_RECORD_DECISION, WAL_RECORD_INSTALL_TIMEOUT,
        WAL_RECORD_LOCK_AND_COMMIT, WAL_RECORD_OBSERVE_PREPARE, WAL_RECORD_PREPARE_INTENT,
        WAL_RECORD_PROPOSAL_INTENT, WAL_RECORD_TIMEOUT_INTENT,
        check_production_durable_intent_transition, locked_commit_progress_witness_is_valid,
    },
    types::timeout_vote_view_is_admissible,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    fmt,
};
/// Progress of an exact block body through the durable validation boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BodyState {
    /// No exact body has been reconstructed yet.
    #[default]
    Missing,
    /// The adapter has reconstructed the exact body in volatile memory.
    Available,
    /// The body store acknowledged durable storage.
    Durable,
    /// Deterministic validation succeeded for the durable body.
    Validated,
    /// Deterministic validation rejected the body.
    Invalid,
}
/// Locally signable consensus message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignableMessage {
    /// Proposal whose unique local intent is already durable.
    Proposal(Proposal),
    /// Prepare or Commit vote whose intent is already durable.
    Vote(Vote),
    /// Timeout vote whose intent is already durable.
    TimeoutVote(TimeoutVote),
}
/// Kind of authenticated equivocation observed by the reducer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquivocationKind {
    /// Two different vote subjects in the same phase and round.
    Vote,
    /// Two timeout votes with different high-QC references in one round.
    Timeout,
    /// Two different proposals from the expected leader in one round.
    Proposal,
}
/// Exact authenticated pair proving equivocation; carrying both signatures prevents a
/// downstream adapter from turning an unverifiable summary into slashing evidence.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EquivocationEvidence {
    /// Two different proposals signed by one round leader.
    Proposal {
        /// First authenticated proposal retained by the reducer.
        first: SignedProposal,
        /// Conflicting authenticated proposal.
        second: SignedProposal,
    },
    /// Two different subjects signed in one phase and round.
    Vote {
        /// First authenticated vote retained by the reducer.
        first: SignedVote,
        /// Conflicting authenticated vote.
        second: SignedVote,
    },
    /// Two different high-QC claims signed for one timeout round.
    Timeout {
        /// First authenticated timeout vote retained by the reducer.
        first: SignedTimeoutVote,
        /// Conflicting authenticated timeout vote.
        second: SignedTimeoutVote,
    },
}
impl EquivocationEvidence {
    /// Return the conflicting message class.
    #[must_use]
    pub const fn kind(&self) -> EquivocationKind {
        match self {
            Self::Proposal { .. } => EquivocationKind::Proposal,
            Self::Vote { .. } => EquivocationKind::Vote,
            Self::Timeout { .. } => EquivocationKind::Timeout,
        }
    }
    /// Return the offending validator.
    #[must_use]
    pub fn offender(&self) -> ValidatorId {
        match self {
            Self::Proposal { first, .. } => first.proposal().proposer(),
            Self::Vote { first, .. } => first.vote().signer(),
            Self::Timeout { first, .. } => first.vote().signer(),
        }
    }
    /// Return the round containing both artifacts.
    #[must_use]
    pub fn round(&self) -> Round {
        match self {
            Self::Proposal { first, .. } => first.proposal().round(),
            Self::Vote { first, .. } => first.vote().round(),
            Self::Timeout { first, .. } => first.vote().round(),
        }
    }
    fn is_conflict_in(&self, context: &HeightContext) -> bool {
        if self.round().height() != context.height()
            || context.validator(&self.offender()).is_none()
        {
            return false;
        }
        match self {
            Self::Proposal { first, second } => {
                let first = first.proposal();
                let second = second.proposal();
                first.context_id() == context.id()
                    && second.context_id() == context.id()
                    && first.round() == second.round()
                    && first.proposer() == second.proposer()
                    && first.manifest() != second.manifest()
            }
            Self::Vote { first, second } => {
                let first = first.vote();
                let second = second.vote();
                first.context_id() == context.id()
                    && second.context_id() == context.id()
                    && first.round() == second.round()
                    && first.phase() == second.phase()
                    && first.signer() == second.signer()
                    && (first.proposal_round() != second.proposal_round()
                        || first.subject() != second.subject())
            }
            Self::Timeout { first, second } => {
                let first = first.vote();
                let second = second.vote();
                first.context_id() == context.id()
                    && second.context_id() == context.id()
                    && first.round() == second.round()
                    && first.signer() == second.signer()
                    && first.highest_prepare() != second.highest_prepare()
            }
        }
    }
}
/// Side effect requested from an asynchronous production adapter.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Append one complete frame to the safety WAL and fsync it.
    Persist {
        /// Tag that must accompany the acknowledgement.
        tag: EventTag,
        /// Complete frame to append.
        entry: WalEntry,
    },
    /// Fetch an exact body from authenticated frozen-roster sources under a verified QC.
    FetchBody {
        /// Tag for all fetch completions.
        tag: EventTag,
        /// Round associated with the body.
        round: Round,
        /// Requested body identity.
        subject: Subject,
        /// Proposal manifest when one is available.
        manifest: Option<PayloadManifest>,
        /// Validators certified to have made the body available.
        certified_sources: Vec<ValidatorId>,
        /// Certificate authorizing a certified fetch; absent for an uncertified proposal.
        certificate: Option<QuorumCertificate>,
    },
    /// Store a reconstructed body durably before validation.
    StoreBody {
        /// Tag for the storage acknowledgement.
        tag: EventTag,
        /// Round associated with the body.
        round: Round,
        /// Body identity.
        subject: Subject,
    },
    /// Run deterministic validation against a durable exact body.
    ValidateBody {
        /// Tag for the validation completion.
        tag: EventTag,
        /// Round associated with the body.
        round: Round,
        /// Body identity.
        subject: Subject,
    },
    /// Sign a safety-relevant message whose intent is already durable.
    Sign {
        /// Tag for the signing completion.
        tag: EventTag,
        /// Canonical message to sign.
        message: SignableMessage,
    },
    /// Broadcast an authenticated protocol-v2 message to all voting validators.
    Broadcast(ConsensusMessageV2),
    /// Apply a durably decided exact block body.
    Apply {
        /// Tag that must accompany the application completion.
        tag: EventTag,
        /// Finalized subject.
        subject: Subject,
        /// Durable `CommitQC` authorizing application.
        certificate: QuorumCertificate,
    },
    /// Notify adapters that a persisted TC changed the current view.
    EnterView {
        /// New reducer tag; old asynchronous completions are now stale.
        tag: EventTag,
        /// Certificate authorizing the view change.
        certificate: TimeoutCertificate,
        /// Exact durable lock selected after installing the certificate. Carrying it across
        /// serialization prevents mutable rereads from rebinding body work.
        protected_lock: Option<QuorumCertificate>,
    },
    /// Report authenticated equivocation without changing safety state.
    ReportEquivocation {
        /// Exact pair of authenticated conflicting artifacts.
        evidence: EquivocationEvidence,
    },
    /// Report a deterministic validation failure for a certified subject.
    ReportInvalidCertifiedBody {
        /// Subject rejected by the deterministic validator.
        subject: Subject,
        /// `PrepareQC` whose signers certified the subject as valid.
        certificate: QuorumCertificate,
    },
}
/// Storage evidence that a decided block and exact `CommitQC` form one durable Kura height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DurableCommitReceipt {
    context_id: super::ContextId,
    height: u64,
    subject: Subject,
    certificate: CertificateRef,
}
impl DurableCommitReceipt {
    /// Construct a receipt at the trusted durable-storage boundary.
    #[must_use]
    pub const fn from_trusted_storage(
        context_id: super::ContextId,
        height: u64,
        subject: Subject,
        certificate: CertificateRef,
    ) -> Self {
        Self {
            context_id,
            height,
            subject,
            certificate,
        }
    }
}
/// Closed, finalized reducer output used to derive the next height context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalizedHeight {
    context: HeightContext,
    decision: QuorumCertificate,
}
impl FinalizedHeight {
    /// Return the context that was finalized.
    #[must_use]
    pub const fn context(&self) -> &HeightContext {
        &self.context
    }
    /// Return the exact durable `CommitQC` decision.
    #[must_use]
    pub const fn decision(&self) -> &QuorumCertificate {
        &self.decision
    }
}
/// Authenticated input or asynchronous adapter completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Resume effects authorized by complete safety-WAL replay. A recovered reducer accepts
    /// this once; its pending bit and full tag bind the lifecycle, height, view, and generation.
    ResumeAfterReplay {
        /// Exact tag of the recovered reducer incarnation.
        tag: EventTag,
    },
    /// Submit a locally built body that is already durable and validated.
    LocalProposalReady {
        /// Current reducer tag assigned by the local builder adapter.
        tag: EventTag,
        /// Manifest of the durably stored, deterministically valid body.
        manifest: PayloadManifest,
    },
    /// Receive a signature-checked proposal.
    ProposalReceived {
        /// Current reducer tag assigned by the adapter.
        tag: EventTag,
        /// Authenticated proposal.
        proposal: SignedProposal,
    },
    /// Receive a signature-checked Prepare or Commit vote.
    VoteReceived {
        /// Current reducer tag assigned by the adapter.
        tag: EventTag,
        /// Authenticated vote.
        vote: SignedVote,
    },
    /// Receive an adapter-verified quorum certificate.
    QuorumCertificateReceived {
        /// Current reducer tag assigned by the adapter.
        tag: EventTag,
        /// Verified certificate.
        certificate: QuorumCertificate,
    },
    /// Receive a signature-checked timeout vote.
    TimeoutVoteReceived {
        /// Current reducer tag assigned by the adapter.
        tag: EventTag,
        /// Authenticated timeout vote.
        vote: SignedTimeoutVote,
    },
    /// Receive an adapter-verified timeout certificate.
    TimeoutCertificateReceived {
        /// Current reducer tag assigned by the adapter.
        tag: EventTag,
        /// Verified timeout certificate.
        certificate: TimeoutCertificate,
    },
    /// Notify the reducer that the current round timer expired.
    TimeoutElapsed {
        /// Current reducer tag assigned by the timer adapter.
        tag: EventTag,
    },
    /// Repeat liveness-critical body acquisition after the retransmission interval.
    RetransmitElapsed {
        /// Current reducer tag assigned by the retransmission timer adapter.
        tag: EventTag,
    },
    /// Notify the reducer that an exact requested body was reconstructed.
    BodyAvailable {
        /// Tag returned by the fetch adapter.
        tag: EventTag,
        /// Round associated with the fetch.
        round: Round,
        /// Reconstructed body identity.
        subject: Subject,
    },
    /// Acknowledge durable storage of an exact body.
    BodyStored {
        /// Tag returned by the body store.
        tag: EventTag,
        /// Round associated with the store.
        round: Round,
        /// Stored body identity.
        subject: Subject,
    },
    /// Complete deterministic body validation.
    ValidationCompleted {
        /// Tag returned by the executor adapter.
        tag: EventTag,
        /// Round associated with validation.
        round: Round,
        /// Validated body identity.
        subject: Subject,
        /// Whether deterministic validation succeeded.
        valid: bool,
    },
    /// Acknowledge a complete, fsynced WAL frame.
    Persisted {
        /// Tag emitted with the persistence request.
        tag: EventTag,
        /// Acknowledged WAL identifier.
        id: PersistenceId,
    },
    /// Report a failed WAL append. The reducer remains fail-closed.
    PersistenceFailed {
        /// Tag emitted with the persistence request.
        tag: EventTag,
        /// Failed WAL identifier.
        id: PersistenceId,
    },
    /// Return an opaque signature for the sole outstanding signing request.
    Signed {
        /// Tag emitted with the signing request.
        tag: EventTag,
        /// Produced signature bytes.
        signature: OpaqueSignature,
    },
    /// Confirm that the exact decided body was successfully applied locally.
    ApplicationCompleted {
        /// Tag returned by the apply adapter.
        tag: EventTag,
        /// Applied decision subject.
        subject: Subject,
    },
}
impl Event {
    fn tag(&self) -> EventTag {
        match self {
            Self::ResumeAfterReplay { tag }
            | Self::LocalProposalReady { tag, .. }
            | Self::ProposalReceived { tag, .. }
            | Self::VoteReceived { tag, .. }
            | Self::QuorumCertificateReceived { tag, .. }
            | Self::TimeoutVoteReceived { tag, .. }
            | Self::TimeoutCertificateReceived { tag, .. }
            | Self::TimeoutElapsed { tag }
            | Self::RetransmitElapsed { tag }
            | Self::BodyAvailable { tag, .. }
            | Self::BodyStored { tag, .. }
            | Self::ValidationCompleted { tag, .. }
            | Self::Persisted { tag, .. }
            | Self::PersistenceFailed { tag, .. }
            | Self::Signed { tag, .. }
            | Self::ApplicationCompleted { tag, .. } => *tag,
        }
    }
    /// Retag authenticated network input after reducer backpressure. Async completions retain
    /// their original tag; only Proposal/Vote/QC/TC delivery may follow a queued view change,
    /// preserving an old-view `CommitQC` for its height.
    #[must_use]
    pub fn retag_authenticated_ingress(self, tag: EventTag) -> Self {
        match self {
            Self::ProposalReceived { proposal, .. } => Self::ProposalReceived { tag, proposal },
            Self::VoteReceived { vote, .. } => Self::VoteReceived { tag, vote },
            Self::QuorumCertificateReceived { certificate, .. } => {
                Self::QuorumCertificateReceived { tag, certificate }
            }
            Self::TimeoutVoteReceived { vote, .. } => Self::TimeoutVoteReceived { tag, vote },
            Self::TimeoutCertificateReceived { certificate, .. } => {
                Self::TimeoutCertificateReceived { tag, certificate }
            }
            event => event,
        }
    }
}
/// Reason an input was safely ignored without changing state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IgnoreReason {
    /// Input belongs to another height.
    WrongHeight,
    /// Input was tagged for a different current view.
    WrongView,
    /// Completion belongs to an old local incarnation or view generation.
    StaleGeneration,
    /// The reducer is waiting for persistence or signing; the adapter must retry.
    Busy,
    /// The message or completion has already been handled.
    Duplicate,
    /// No outstanding body operation matches the completion.
    NoMatchingWork,
    /// The node is an observer and therefore cannot vote or time out.
    Observer,
    /// A durable timeout intent closed this view to new Prepare/Commit votes.
    ViewClosed,
    /// A finalized decision makes this input irrelevant.
    AlreadyDecided,
    /// WAL replay completed, but its one resumption has not crossed the commit gate.
    RecoveryPending,
    /// The input's round or safe-value rank cannot affect local state.
    IrrelevantView,
    /// A durable lock makes the proposal's subject unsafe to prepare.
    UnsafeProposal,
}
/// Whether a reducer step changed state or intentionally ignored an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepDisposition {
    /// The input was applied.
    Applied,
    /// The input was safely ignored for the stated reason.
    Ignored(IgnoreReason),
}
/// Result of one serialized reducer transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepOutcome {
    disposition: StepDisposition,
    effects: Vec<Effect>,
}
impl StepOutcome {
    fn applied(effects: Vec<Effect>) -> Self {
        Self {
            disposition: StepDisposition::Applied,
            effects,
        }
    }
    fn ignored(reason: IgnoreReason) -> Self {
        Self {
            disposition: StepDisposition::Ignored(reason),
            effects: Vec::new(),
        }
    }
    /// Returns whether the input was applied or ignored.
    #[must_use]
    pub const fn disposition(&self) -> StepDisposition {
        self.disposition
    }
    /// Returns requested adapter effects in deterministic order.
    #[must_use]
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    /// Consumes the outcome and returns its adapter effects.
    #[must_use]
    pub fn into_effects(self) -> Vec<Effect> {
        self.effects
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct BodyWork {
    manifest: Option<PayloadManifest>,
    state: BodyState,
}
/// Exact in-memory WAL append and the continuation fenced behind its fsync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPersistence {
    entry: WalEntry,
    continuation: Continuation,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Continuation {
    None,
    Sign(SignableMessage),
    InstallTimeout {
        certificate: TimeoutCertificate,
        broadcast: bool,
    },
    Decide {
        certificate: QuorumCertificate,
        broadcast: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum OutboundControlClass {
    Proposal,
    PrepareVote,
    CommitVote,
    PrepareQc,
    CommitQc,
    TimeoutVote,
    TimeoutCertificate,
}
/// Exact partial vote quorum retained by the reducer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VotePoolSnapshot {
    /// Voting round.
    pub(crate) round: Round,
    /// Immutable proposal-body origin authenticated by the pool.
    pub(crate) proposal_round: Round,
    /// Voting phase.
    pub(crate) phase: Phase,
    /// Exact voted subject.
    pub(crate) subject: Subject,
    /// Canonically ordered distinct signers for this subject.
    pub(crate) signers: Vec<ValidatorId>,
    /// Voting power represented by `signers`.
    pub(crate) signed_power: VotingPower,
}
/// Exact partial timeout quorum retained by the reducer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TimeoutPoolSnapshot {
    /// Timed-out round.
    pub(crate) round: Round,
    /// Canonically ordered distinct timeout signers.
    pub(crate) signers: Vec<ValidatorId>,
    /// Voting power represented by `signers`.
    pub(crate) signed_power: VotingPower,
    /// Whether this pool already formed a timeout certificate.
    pub(crate) certificate_formed: bool,
}
/// The sole executable Sumeragi v2 consensus state machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reducer {
    context: HeightContext,
    local_validator: Option<ValidatorId>,
    generation: Generation,
    durable: DurableState,
    candidate: Option<Proposal>,
    candidate_signed: Option<SignedProposal>,
    body_work: BTreeMap<(Round, Subject), BodyWork>,
    pending_prepare: BTreeMap<CertificateRef, QuorumCertificate>,
    known_prepare: BTreeMap<CertificateRef, QuorumCertificate>,
    votes: BTreeMap<(Round, Phase, Round), BTreeMap<ValidatorId, SignedVote>>,
    timeout_votes: BTreeMap<Round, BTreeMap<ValidatorId, SignedTimeoutVote>>,
    formed_certificates: BTreeSet<CertificateRef>,
    formed_timeouts: BTreeSet<Round>,
    outbound_control: BTreeMap<OutboundControlClass, ConsensusMessageV2>,
    pending_persistence: Option<PendingPersistence>,
    awaiting_signature: Option<SignableMessage>,
    signature_queue: VecDeque<SignableMessage>,
    replay_resumed: bool,
    applied_subject: Option<Subject>,
    // Volatile, view-scoped admission for Set B body work and new votes.
    fallback_active: bool,
}
impl Reducer {
    /// Return immutable archive fanout authenticated by the frozen context. The carried QC,
    /// not signer-subset membership, authorizes the exact subject.
    fn frozen_archive_sources(&self) -> Vec<ValidatorId> {
        self.context
            .roster()
            .iter()
            .map(|validator| validator.id())
            .collect()
    }
    fn local_committee_role(&self) -> Option<CommitteeRole> {
        let local = self.local_validator?;
        let index = self
            .context
            .roster()
            .iter()
            .position(|validator| validator.id() == local)?;
        let index = u32::try_from(index).ok()?;
        Committee::project(&self.context, self.durable.current_view())
            .ok()?
            .role(index)
            .ok()
    }
    fn local_candidate_body_eligible(&self) -> bool {
        match self.local_committee_role() {
            Some(CommitteeRole::SetBValidator) => self.fallback_active,
            Some(
                CommitteeRole::Leader | CommitteeRole::SetAValidator | CommitteeRole::ProxyTail,
            ) => true,
            None => false,
        }
    }
    /// Return whether this validator may create a Prepare/Commit intent for
    /// the current candidate body.
    ///
    /// The serialized runtime uses this read-only projection when deciding
    /// whether exact, already-arrived locked-reproposal Prepare votes can
    /// productively precede a frozen timeout owner. Reducer admission and WAL
    /// persistence remain the authority for the eventual vote.
    pub(crate) fn local_candidate_body_is_eligible(&self) -> bool {
        self.local_candidate_body_eligible()
    }
    fn local_certified_candidate_body_eligible(&self) -> bool {
        self.local_validator.is_none() || self.local_candidate_body_eligible()
    }
    fn record_is_role_eligible(&self, record: &WalRecord) -> bool {
        match record {
            WalRecord::PrepareIntent(_) | WalRecord::LockAndCommit { .. } => {
                self.local_candidate_body_eligible()
            }
            WalRecord::ProposalIntent(_)
            | WalRecord::ObservePrepare(_)
            | WalRecord::TimeoutIntent(_)
            | WalRecord::InstallTimeout(_)
            | WalRecord::Decision(_) => true,
        }
    }
    /// Constructs a fresh reducer at view zero.
    /// # Errors
    /// Returns an error if the local validator is absent from the frozen voting roster.
    pub fn new(
        context: HeightContext,
        local_validator: Option<ValidatorId>,
        generation: Generation,
    ) -> Result<Self, ReducerError> {
        let durable = DurableState::new(&context);
        // A fresh reducer has no replay lifecycle transition to consume.
        Self::from_durable(context, local_validator, generation, durable, true)
    }
    /// Reconstructs a reducer from complete WAL frames.
    /// Until a matching [`Event::ResumeAfterReplay`] crosses [`Self::step`], the reducer
    /// accepts nothing else, keeping replay effects behind the production commit gate.
    /// # Errors
    /// Returns an error if replay fails or the local validator is absent from the roster.
    pub fn recover(
        context: HeightContext,
        local_validator: Option<ValidatorId>,
        generation: Generation,
        entries: impl IntoIterator<Item = WalEntry>,
    ) -> Result<Self, ReducerError> {
        let durable = DurableState::replay(&context, local_validator, entries)?;
        // Only successful WAL replay creates an unconsumed recovery event.
        Self::from_durable(context, local_validator, generation, durable, false)
    }
    fn from_durable(
        context: HeightContext,
        local_validator: Option<ValidatorId>,
        generation: Generation,
        durable: DurableState,
        replay_resumed: bool,
    ) -> Result<Self, ReducerError> {
        if local_validator.is_some_and(|id| context.validator(&id).is_none()) {
            return Err(ReducerError::LocalValidatorNotInRoster);
        }
        let retryable_prepare = durable.highest_prepare().filter(|certificate| {
            durable.decision().is_none()
                && certificate.round().view() == durable.current_view()
                && durable.timeout_intent(certificate.round()).is_none()
        });
        let mut body_work = BTreeMap::new();
        let mut pending_prepare = BTreeMap::new();
        if let Some(certificate) = retryable_prepare {
            // Replay restores an undecided open-current-view high PrepareQC as exact Missing-body
            // authority without constructor work. Retransmission derives FetchBody after replay;
            // closed/old highs, locks, and Decisions retain narrower recovery paths.
            body_work.insert(
                (certificate.round(), certificate.subject()),
                BodyWork {
                    manifest: None,
                    state: BodyState::Missing,
                },
            );
            pending_prepare.insert(certificate.reference(), certificate.clone());
        }
        let mut known_prepare = BTreeMap::new();
        if let Some(certificate) = durable.highest_prepare() {
            known_prepare.insert(certificate.reference(), certificate.clone());
        }
        if let Some(certificate) = durable.locked() {
            known_prepare.insert(certificate.reference(), certificate.clone());
        }
        let mut outbound_control = BTreeMap::new();
        if let Some(certificate) = durable.decision() {
            outbound_control.insert(
                OutboundControlClass::CommitQc,
                ConsensusMessageV2::QuorumCertificate(certificate.clone()),
            );
        } else {
            if let Some(certificate) = durable.highest_prepare() {
                outbound_control.insert(
                    OutboundControlClass::PrepareQc,
                    ConsensusMessageV2::QuorumCertificate(certificate.clone()),
                );
            }
            if let Some(certificate) = durable.last_timeout() {
                outbound_control.insert(
                    OutboundControlClass::TimeoutCertificate,
                    ConsensusMessageV2::TimeoutCertificate(certificate.clone()),
                );
            }
        }
        Ok(Self {
            context,
            local_validator,
            generation,
            durable,
            candidate: None,
            candidate_signed: None,
            body_work,
            pending_prepare,
            known_prepare,
            votes: BTreeMap::new(),
            timeout_votes: BTreeMap::new(),
            formed_certificates: BTreeSet::new(),
            formed_timeouts: BTreeSet::new(),
            outbound_control,
            pending_persistence: None,
            awaiting_signature: None,
            signature_queue: VecDeque::new(),
            replay_resumed,
            applied_subject: None,
            fallback_active: false,
        })
    }
    /// Returns the frozen height context.
    #[must_use]
    pub const fn context(&self) -> &HeightContext {
        &self.context
    }
    /// Returns the local validator, or `None` for an observer.
    #[must_use]
    pub const fn local_validator(&self) -> Option<ValidatorId> {
        self.local_validator
    }
    /// Return volatile evidence-pool cardinalities for boundedness tests.
    #[cfg(test)]
    pub(crate) fn volatile_evidence_counts(&self) -> (usize, usize, usize, usize) {
        (
            self.votes.len(),
            self.timeout_votes.len(),
            self.formed_certificates.len(),
            self.formed_timeouts.len(),
        )
    }
    #[cfg(test)]
    pub(crate) fn volatile_prepare_counts(&self) -> (usize, usize) {
        (self.pending_prepare.len(), self.known_prepare.len())
    }
    /// Returns the state reconstructed from acknowledged WAL frames.
    #[must_use]
    pub const fn durable_state(&self) -> &DurableState {
        &self.durable
    }
    /// Return the decision subject applied in this reducer incarnation, if any.
    #[must_use]
    pub const fn applied_subject(&self) -> Option<Subject> {
        self.applied_subject
    }
    /// Return whether a matching Kura receipt can consume the height without unfinished work.
    #[must_use]
    pub fn ready_to_finish(&self) -> bool {
        self.pending_persistence.is_none()
            && self.awaiting_signature.is_none()
            && self
                .durable
                .decision()
                .is_some_and(|decision| self.applied_subject == Some(decision.subject()))
    }
    /// Returns the current tag adapters must attach to new inputs.
    #[must_use]
    pub const fn current_tag(&self) -> EventTag {
        EventTag::new(
            self.context.height(),
            self.durable.current_view(),
            self.generation,
        )
    }
    /// Return the local generation owning volatile consumer state.
    #[must_use]
    pub(crate) const fn generation(&self) -> Generation {
        self.generation
    }
    /// Return exact subject-grouped partial Prepare and Commit pools.
    #[must_use]
    pub(crate) fn vote_pool_snapshots(&self) -> Vec<VotePoolSnapshot> {
        let mut grouped = BTreeMap::<(Round, Phase, Round, Subject), Vec<ValidatorId>>::new();
        for ((round, phase, proposal_round), votes) in &self.votes {
            for vote in votes.values() {
                grouped
                    .entry((*round, *phase, *proposal_round, vote.vote().subject()))
                    .or_default()
                    .push(vote.vote().signer());
            }
        }
        grouped
            .into_iter()
            .map(|((round, phase, proposal_round, subject), signers)| {
                let quorum = Quorum::calculate(&self.context, &signers)
                    .expect("reducer vote pools contain canonical roster signers");
                VotePoolSnapshot {
                    round,
                    proposal_round,
                    phase,
                    subject,
                    signers,
                    signed_power: quorum.voting_power(),
                }
            })
            .collect()
    }
    /// Return exact partial timeout pools and their formed-certificate state.
    #[must_use]
    pub(crate) fn timeout_pool_snapshots(&self) -> Vec<TimeoutPoolSnapshot> {
        self.timeout_votes
            .iter()
            .map(|(round, votes)| {
                let signers = votes.keys().copied().collect::<Vec<_>>();
                let quorum = Quorum::calculate(&self.context, &signers)
                    .expect("reducer timeout pools contain canonical roster signers");
                TimeoutPoolSnapshot {
                    round: *round,
                    signers,
                    signed_power: quorum.voting_power(),
                    certificate_formed: self.formed_timeouts.contains(round),
                }
            })
            .collect()
    }
    /// Iterate over signed or certified control intents retained for retransmission.
    pub fn outbound_messages(&self) -> impl Iterator<Item = &ConsensusMessageV2> {
        self.outbound_control.values()
    }
    /// Return the durable record currently fenced behind a WAL append.
    #[must_use]
    pub(crate) fn pending_persistence_record(&self) -> Option<&WalRecord> {
        self.pending_persistence
            .as_ref()
            .map(|pending| pending.entry.record())
    }
    /// Return the signable currently owned by the signer boundary.
    #[must_use]
    pub(crate) const fn awaiting_signature(&self) -> Option<&SignableMessage> {
        self.awaiting_signature.as_ref()
    }
    /// Iterate over durable signables waiting behind the active signer work.
    pub(crate) fn queued_signatures(&self) -> impl Iterator<Item = &SignableMessage> {
        self.signature_queue.iter()
    }
    /// Returns the body state for a round and subject.
    #[must_use]
    pub fn body_state(&self, round: Round, subject: Subject) -> BodyState {
        self.body_work
            .get(&(round, subject))
            .map_or(BodyState::Missing, |work| work.state)
    }
    /// Return the exact same-round body key authenticated by a Decision.
    fn decision_body_round(&self, decision: &QuorumCertificate) -> Round {
        decision.proposal_round()
    }
    /// Consume a fully applied reducer after Kura durably exposes the matching
    /// block and `CommitQC`. Consuming `self` prevents any further votes at the
    /// closed height.
    ///
    /// # Errors
    ///
    /// Returns an error unless application completed and every receipt field
    /// matches the exact durable decision.
    pub fn finish_height(
        self,
        receipt: DurableCommitReceipt,
    ) -> Result<FinalizedHeight, ReducerError> {
        if self.pending_persistence.is_some() || self.awaiting_signature.is_some() {
            return Err(ReducerError::HeightStillBusy);
        }
        let decision = self
            .durable
            .decision()
            .cloned()
            .ok_or(ReducerError::HeightNotApplied)?;
        if self.applied_subject != Some(decision.subject()) {
            return Err(ReducerError::HeightNotApplied);
        }
        if receipt.context_id != self.context.id()
            || receipt.height != self.context.height()
            || receipt.subject != decision.subject()
            || receipt.certificate != decision.reference()
        {
            return Err(ReducerError::DurableCommitReceiptMismatch);
        }
        Ok(FinalizedHeight {
            context: self.context,
            decision,
        })
    }
    fn on_resume_after_replay(&mut self) -> StepOutcome {
        if self.replay_resumed {
            return StepOutcome::ignored(IgnoreReason::Duplicate);
        }
        self.replay_resumed = true;
        if let Some(decision) = self.durable.decision().cloned() {
            return StepOutcome::applied(self.decision_effects(decision));
        }
        let round = Round::new(self.context.height(), self.durable.current_view());
        if let Some(timeout) = self.durable.timeout_intent(round) {
            self.signature_queue
                .push_back(SignableMessage::TimeoutVote(timeout));
        } else if let Some(proposal) = self
            .durable
            .proposal_intent(round)
            .filter(|proposal| Self::durable_proposal_is_active(&self.durable, proposal))
        {
            self.signature_queue
                .push_back(SignableMessage::Proposal(proposal.clone()));
        }
        if let Some(vote) = self
            .durable
            .prepare_intents()
            .find(|vote| Self::durable_vote_is_active(&self.durable, *vote))
        {
            self.signature_queue.push_back(SignableMessage::Vote(vote));
        }
        if let Some(vote) = self
            .durable
            .locked()
            .and_then(|locked| self.durable.commit_intent_for_lock(locked))
        {
            self.signature_queue.push_back(SignableMessage::Vote(vote));
        }
        StepOutcome::applied(self.drive_signature())
    }
    /// Applies one input to the serialized state machine.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed authenticated input, an invalid
    /// certificate, failed persistence, or an impossible durable-state change.
    pub fn step(&mut self, event: Event) -> Result<StepOutcome, ReducerError> {
        // The candidate transition stays private until the executable refinement gate accepts its
        // exact state/effect projection. Even errors pass through it as empty stutters, so no
        // `Reducer::step` exit bypasses the production kernel verified in `verus_proofs.rs`.
        let audit_event = event.clone();
        let mut next = self.clone();
        match next.step_in_place(event) {
            Ok(outcome) => {
                let transition = self.transition_projection(&audit_event, &next, outcome.effects());
                let Some(checked_refinement) = refinement::check(transition) else {
                    // Keep diagnostics derivable inside this dependency-free core. The adapter
                    // that observes the returned error owns logging and telemetry.
                    let _diagnostic = refinement::diagnose(transition);
                    return Err(ReducerError::RefinementViolation);
                };
                let durable_intent_trace = ProductionDurableIntentTraceProjection {
                    event_tag: transition.event_tag,
                    owner_tag_before: Self::tag_projection(self.current_tag()),
                    owner_tag_after: Self::tag_projection(next.current_tag()),
                    event_kind: transition.event_kind,
                    event_persistence_id: match &audit_event {
                        Event::Persisted { id, .. } | Event::PersistenceFailed { id, .. } => {
                            id.get()
                        }
                        _ => 0,
                    },
                    pending_before: transition.pending_before,
                    pending_after: next.pending_projection(),
                    boundary_claimed: transition.boundary_claimed,
                    boundary_granted: transition.boundary_granted,
                    effects: transition.effects,
                    durable_sequence_before: self.durable.last_id().get(),
                    durable_sequence_after: next.durable.last_id().get(),
                };
                let Some(checked_transition) =
                    check_production_durable_intent_transition(durable_intent_trace)
                else {
                    return Err(ReducerError::RefinementViolation);
                };
                if let Some(violation) = next.progress_witness_violation() {
                    return Err(ReducerError::ProgressWitnessViolation(violation));
                }
                let _authorized_transition = checked_transition.into_projection();
                let _authorized_refinement = checked_refinement.into_projection();
                *self = next;
                Ok(outcome)
            }
            Err(error) => {
                if !self.transition_refines(&audit_event, self, &[]) {
                    let transition = self.transition_projection(&audit_event, self, &[]);
                    let _diagnostic = refinement::diagnose(transition);
                    return Err(ReducerError::RefinementViolation);
                }
                if let Some(violation) = self.progress_witness_violation() {
                    return Err(ReducerError::ProgressWitnessViolation(violation));
                }
                Err(error)
            }
        }
    }
    /// Return the first missing reconstruction witness for durable progress.
    ///
    /// This check deliberately covers only reducer-owned state. Adapter queue
    /// ownership and asynchronous worker service are checked at their own
    /// admission boundaries; the reducer must retain enough state to recreate
    /// either operation on retransmission or recovery.
    pub(crate) fn progress_witness_violation(&self) -> Option<ProgressWitnessViolation> {
        if let Some(locked) = self.durable.locked()
            && self.durable.decision().is_none()
            && self.local_validator.is_some()
        {
            let commit_intent = self.durable.commit_intent_for_lock(locked);
            let progress_witness = locked_commit_progress_witness_is_valid(
                self.locked_commit_progress_witness_projection(locked, commit_intent),
            );
            if commit_intent.is_some() {
                if self.replay_resumed && !progress_witness {
                    return Some(ProgressWitnessViolation::LockedCommitOrphaned);
                }
            } else if self.replay_resumed
                && self.body_state(locked.round(), locked.subject()) == BodyState::Validated
                && !progress_witness
            {
                // A current-view validated lock owns its exact same-round Commit append. For a
                // historical TC-promoted lock, the installed predecessor timeout instead authorizes
                // unchanged body re-proposal; it never authorizes a new Commit for the closed round.
                return Some(ProgressWitnessViolation::LockedCommitOrphaned);
            }
        }
        if let Some(decision) = self.durable.decision()
            && self.applied_subject != Some(decision.subject())
            && self.replay_resumed
        {
            let body_round = self.decision_body_round(decision);
            let state = self.body_state(body_round, decision.subject());
            if state == BodyState::Invalid {
                return Some(ProgressWitnessViolation::DecidedBodyInvalid);
            }
            // Every non-invalid body stage has deterministic retransmission (fetch, store,
            // validate, or apply), so the exact stage is the reducer-owned reconstruction witness.
            if !self
                .body_work
                .contains_key(&(body_round, decision.subject()))
            {
                return Some(ProgressWitnessViolation::DecisionApplicationOrphaned);
            }
        }
        None
    }
    fn locked_commit_progress_witness_projection(
        &self,
        locked: &QuorumCertificate,
        commit_intent: Option<Vote>,
    ) -> LockedCommitProgressWitnessProjection {
        let current_round = Round::new(self.context.height(), self.durable.current_view());
        let signature_pending = commit_intent.is_some_and(|intent| {
            self.awaiting_signature.as_ref().is_some_and(
                |message| matches!(message, SignableMessage::Vote(vote) if *vote == intent),
            ) || self
                .signature_queue
                .iter()
                .any(|message| matches!(message, SignableMessage::Vote(vote) if *vote == intent))
        });
        let pooled = commit_intent.is_some_and(|intent| {
            self.votes
                .get(&(intent.round(), Phase::Commit, intent.proposal_round()))
                .and_then(|pool| pool.get(&intent.signer()))
                .is_some_and(|vote| vote.vote() == intent)
        });
        let timeout_intent = self.durable.timeout_intent(current_round);
        let installed_timeout = self.durable.last_timeout();
        LockedCommitProgressWitnessProjection {
            context_id: Self::context_identity_projection(self.context.id()),
            current_height: current_round.height(),
            current_view: current_round.view(),
            local_validator_present: self.local_validator.is_some(),
            local_validator: self.local_validator.unwrap_or_default(),
            locked_context_id: Self::context_identity_projection(locked.reference().context_id()),
            locked_height: locked.proposal_round().height(),
            locked_view: locked.proposal_round().view(),
            locked_subject: Self::subject_identity_projection(locked.subject()),
            commit_intent_present: commit_intent.is_some(),
            commit_context_id: commit_intent
                .map_or_else(CanonicalIdentityProjection::zero, |intent| {
                    Self::context_identity_projection(intent.context_id())
                }),
            commit_height: commit_intent.map_or(0, |intent| intent.round().height()),
            commit_view: commit_intent.map_or(0, |intent| intent.round().view()),
            commit_proposal_height: commit_intent
                .map_or(0, |intent| intent.proposal_round().height()),
            commit_proposal_view: commit_intent.map_or(0, |intent| intent.proposal_round().view()),
            commit_phase: commit_intent.map_or(0, |intent| Self::phase_code(intent.phase())),
            commit_subject: commit_intent
                .map_or_else(CanonicalIdentityProjection::zero, |intent| {
                    Self::subject_identity_projection(intent.subject())
                }),
            commit_signer: commit_intent.map_or_else(ValidatorId::default, Vote::signer),
            commit_signature_pending: signature_pending,
            commit_pooled: pooled,
            pending: self.pending_projection(),
            timeout_intent_present: timeout_intent.is_some(),
            timeout_intent_durable: timeout_intent.is_some(),
            timeout_context_id: timeout_intent
                .as_ref()
                .map_or_else(CanonicalIdentityProjection::zero, |vote| {
                    Self::context_identity_projection(vote.context_id())
                }),
            timeout_height: timeout_intent
                .as_ref()
                .map_or(0, |vote| vote.round().height()),
            timeout_view: timeout_intent
                .as_ref()
                .map_or(0, |vote| vote.round().view()),
            timeout_signer: timeout_intent
                .as_ref()
                .map_or_else(ValidatorId::default, |vote| vote.signer()),
            installed_timeout_present: installed_timeout.is_some(),
            installed_timeout_durable: installed_timeout.is_some(),
            installed_timeout_context_id: installed_timeout
                .map_or_else(CanonicalIdentityProjection::zero, |certificate| {
                    Self::context_identity_projection(certificate.context_id())
                }),
            installed_timeout_height: installed_timeout
                .map_or(0, |certificate| certificate.round().height()),
            installed_timeout_view: installed_timeout
                .map_or(0, |certificate| certificate.round().view()),
        }
    }
    fn step_in_place(&mut self, event: Event) -> Result<StepOutcome, ReducerError> {
        if let Some(reason) = self.reject_tag(event.tag()) {
            return Ok(StepOutcome::ignored(reason));
        }
        if !self.replay_resumed && !matches!(event, Event::ResumeAfterReplay { .. }) {
            return Ok(StepOutcome::ignored(IgnoreReason::RecoveryPending));
        }
        // A duplicate replay-resume event is a pure idempotent stutter even
        // if an effect emitted by the first event is still outstanding.
        let replay_duplicate =
            self.replay_resumed && matches!(event, Event::ResumeAfterReplay { .. });
        if self.pending_persistence.is_some()
            && !matches!(
                event,
                Event::Persisted { .. } | Event::PersistenceFailed { .. }
            )
            && !replay_duplicate
        {
            return Ok(StepOutcome::ignored(IgnoreReason::Busy));
        }
        if self.awaiting_signature.is_some()
            && !matches!(event, Event::Signed { .. })
            && !Self::certified_progress_bypasses_signature_fence(&event)
            && !replay_duplicate
        {
            return Ok(StepOutcome::ignored(IgnoreReason::Busy));
        }
        match event {
            Event::ResumeAfterReplay { .. } => Ok(self.on_resume_after_replay()),
            Event::LocalProposalReady { manifest, .. } => self.on_local_proposal_ready(manifest),
            Event::ProposalReceived { proposal, .. } => self.on_proposal(&proposal),
            Event::VoteReceived { vote, .. } => self.on_vote(vote),
            Event::QuorumCertificateReceived { certificate, .. } => {
                self.on_certificate(certificate, false)
            }
            Event::TimeoutVoteReceived { vote, .. } => self.on_timeout_vote(vote),
            Event::TimeoutCertificateReceived { certificate, .. } => {
                self.on_timeout_certificate(certificate, false)
            }
            Event::TimeoutElapsed { .. } => self.on_timeout_elapsed(),
            Event::RetransmitElapsed { .. } => self.on_retransmit_elapsed(),
            Event::BodyAvailable { round, subject, .. } => {
                Ok(self.on_body_available(round, subject))
            }
            Event::BodyStored { round, subject, .. } => Ok(self.on_body_stored(round, subject)),
            Event::ValidationCompleted {
                round,
                subject,
                valid,
                ..
            } => self.on_validation(round, subject, valid),
            Event::Persisted { id, .. } => self.on_persisted(id),
            Event::PersistenceFailed { id, .. } => self.on_persistence_failed(id),
            Event::Signed { signature, .. } => self.on_signed(signature),
            Event::ApplicationCompleted { subject, .. } => self.on_application_completed(subject),
        }
    }
    fn transition_refines(&self, event: &Event, after: &Self, effects: &[Effect]) -> bool {
        refinement::accepts(self.transition_projection(event, after, effects))
    }
    fn transition_projection<'a>(
        &'a self,
        event: &Event,
        after: &'a Self,
        effects: &[Effect],
    ) -> TransitionProjection<'a> {
        let trace = self.effect_trace(event, after, effects);
        let boundary_claimed = self.boundary_claim(event, after, effects);
        let boundary_granted = self.boundary_grant(event, after, effects, boundary_claimed);
        let installed_height = after.durable.height();
        let installed_view = after.durable.current_view();
        let timeout_evidence_after_outside_installed_window = Self::cardinality(
            after
                .timeout_votes
                .keys()
                .chain(after.formed_timeouts.iter())
                .filter(|round| {
                    round.height() != installed_height
                        || !timeout_vote_view_is_admissible(installed_view, round.view())
                })
                .count(),
        );
        TransitionProjection {
            before_state: self,
            after_state: after,
            durable_before: &self.durable,
            durable_after: &after.durable,
            safety_before: self.production_safety_projection(),
            safety_after: after.production_safety_projection(),
            context_before: &self.context,
            context_after: &after.context,
            local_before: Self::validator_projection(self.local_validator),
            local_after: Self::validator_projection(after.local_validator),
            event_tag: Self::tag_projection(event.tag()),
            height_before: self.context.height(),
            view_before: self.durable.current_view(),
            generation_before: self.generation.get(),
            generation_after: after.generation.get(),
            pending_state_before: self.pending_persistence.as_ref(),
            pending_state_after: after.pending_persistence.as_ref(),
            pending_before: self.pending_projection(),
            // `awaiting_before` is the executable signing-fence predicate,
            // rather than merely a snapshot of the optional task. A fully
            // verified TC or CommitQC may supersede that local task through
            // its own safety-WAL transition; every other event remains
            // fenced until the exact signature completion arrives.
            awaiting_before: self.awaiting_signature.is_some()
                && !Self::certified_progress_bypasses_signature_fence(event),
            replay_before: self.replay_resumed,
            application_before: Self::subject_projection(self.applied_subject),
            application_after: Self::subject_projection(after.applied_subject),
            event_kind: Self::event_kind(event),
            awaiting_message_kind: self
                .awaiting_signature
                .as_ref()
                .map_or(SIGNED_MESSAGE_NONE, Self::signable_message_kind),
            validator_count: Self::cardinality(self.context.roster().len()),
            volatile_before: self.volatile_summary(),
            volatile_after: after.volatile_summary(),
            timeout_votes_before: &self.timeout_votes,
            timeout_votes_after: &after.timeout_votes,
            formed_timeouts_before: &self.formed_timeouts,
            formed_timeouts_after: &after.formed_timeouts,
            timeout_evidence_after_outside_installed_window,
            timeout_control_before: self
                .outbound_control
                .get(&OutboundControlClass::TimeoutVote),
            timeout_control_after: after
                .outbound_control
                .get(&OutboundControlClass::TimeoutVote),
            boundary_claimed,
            boundary_granted,
            enter_view: self.enter_view_projection(after, effects),
            effects: trace,
        }
    }
    fn effect_trace(&self, event: &Event, after: &Self, effects: &[Effect]) -> EffectTrace {
        let mut trace = EffectTrace::empty();
        for effect in effects {
            let requested = Self::effect_capability(effect);
            let granted = self.granted_effect_capability(event, after, effect);
            if !trace.push(requested, granted) {
                // A non-canonical length is rejected by the verified kernel.
                trace.len = u8::try_from(refinement::MAX_EFFECTS_PER_STEP + 1)
                    .expect("the fixed effect bound fits in u8");
                break;
            }
        }
        trace
    }
    fn production_safety_projection(&self) -> SafetyProjection {
        let durable_context_id = self.durable.context_id();
        let expected_context_id = self.context.id();
        let durable_height = self.durable.height();
        let expected_height = self.context.height();
        let invalid_highest_prepare = self.durable.highest_prepare().is_some_and(|certificate| {
            certificate.phase() != Phase::Prepare
                || certificate.round().view() > self.durable.current_view()
                || certificate.validate(&self.context).is_err()
        });
        let invalid_lock = self.durable.locked().is_some_and(|locked| {
            locked.phase() != Phase::Prepare
                || locked.round().view() > self.durable.current_view()
                || locked.validate(&self.context).is_err()
                || self.durable.highest_prepare().is_none_or(|highest| {
                    highest.round().view() < locked.round().view()
                        || (highest.round().view() == locked.round().view()
                            && highest.subject() != locked.subject())
                })
        });
        let invalid_timeout = self.durable.last_timeout().is_some_and(|certificate| {
            certificate.validate(&self.context).is_err()
                || certificate.round().view().checked_add(1) != Some(self.durable.current_view())
        });
        let invalid_decision = self.durable.decision().is_some_and(|certificate| {
            certificate.phase() != Phase::Commit || certificate.validate(&self.context).is_err()
        });
        let invalid_pending_append = self.pending_persistence.as_ref().is_some_and(|pending| {
            let structurally_invalid = self.durable.next_id() != Ok(pending.entry.id())
                || !Self::continuation_matches_record(
                    pending.entry.record(),
                    &pending.continuation,
                );
            let mut expected = self.durable.clone();
            structurally_invalid
                || expected
                    .apply(&self.context, self.local_validator, &pending.entry)
                    .is_err()
        });
        let awaiting_unauthorized = usize::from(
            self.awaiting_signature
                .as_ref()
                .is_some_and(|message| !self.signable_is_durably_authorized(message)),
        );
        let queued_unauthorized = self
            .signature_queue
            .iter()
            .filter(|message| !self.signable_is_durably_authorized(message))
            .count();
        let invalid_application = self.applied_subject.is_some_and(|subject| {
            self.durable.decision().is_none_or(|decision| {
                decision.subject() != subject
                    || self.body_state(self.decision_body_round(decision), subject)
                        != BodyState::Validated
            })
        });
        SafetyProjection {
            durable_identity_mismatches: u64::from(
                durable_context_id != expected_context_id || durable_height != expected_height,
            ),
            asynchronous_fence_conflicts: u64::from(
                self.pending_persistence.is_some() && self.awaiting_signature.is_some(),
            ),
            invalid_highest_prepare: u64::from(invalid_highest_prepare),
            invalid_lock: u64::from(invalid_lock),
            invalid_timeout: u64::from(invalid_timeout),
            invalid_decision: u64::from(invalid_decision),
            invalid_pending_append: u64::from(invalid_pending_append),
            unauthorized_signables: Self::cardinality(
                awaiting_unauthorized.saturating_add(queued_unauthorized),
            ),
            invalid_application: u64::from(invalid_application),
        }
    }
    fn continuation_matches_record(record: &WalRecord, continuation: &Continuation) -> bool {
        match (record, continuation) {
            (
                WalRecord::ProposalIntent(record),
                Continuation::Sign(SignableMessage::Proposal(message)),
            ) => record == message,
            (
                WalRecord::PrepareIntent(record),
                Continuation::Sign(SignableMessage::Vote(message)),
            ) => record == message && message.phase() == Phase::Prepare,
            (
                WalRecord::LockAndCommit { vote: record, .. },
                Continuation::Sign(SignableMessage::Vote(message)),
            ) => record == message && message.phase() == Phase::Commit,
            (
                WalRecord::TimeoutIntent(record),
                Continuation::Sign(SignableMessage::TimeoutVote(message)),
            ) => record == message,
            (WalRecord::ObservePrepare(_), Continuation::None) => true,
            (
                WalRecord::InstallTimeout(record),
                Continuation::InstallTimeout { certificate, .. },
            ) => record == certificate,
            (WalRecord::Decision(record), Continuation::Decide { certificate, .. }) => {
                record == certificate
            }
            _ => false,
        }
    }
    fn signable_is_durably_authorized(&self, message: &SignableMessage) -> bool {
        Self::signable_is_durably_authorized_for(&self.durable, message)
    }
    fn signable_is_durably_authorized_for(
        durable: &DurableState,
        message: &SignableMessage,
    ) -> bool {
        match message {
            SignableMessage::Proposal(proposal) => {
                Self::durable_proposal_is_active(durable, proposal)
            }
            SignableMessage::Vote(vote) => Self::durable_vote_is_active(durable, *vote),
            SignableMessage::TimeoutVote(vote) => {
                Self::durable_timeout_vote_is_active(durable, vote)
            }
        }
    }
    fn durable_proposal_is_active(durable: &DurableState, proposal: &Proposal) -> bool {
        let exact_justification = match proposal.justification() {
            ProposalJustification::ParentCommit(_) => proposal.round().view() == 0,
            ProposalJustification::Timeout(certificate) => durable
                .is_exact_local_proposal_timeout_justification(
                    proposal.round().view(),
                    certificate,
                ),
        };
        durable.decision().is_none()
            && proposal.round() == Round::new(durable.height(), durable.current_view())
            && durable.timeout_intent(proposal.round()).is_none()
            && durable.proposal_intent(proposal.round()) == Some(proposal)
            && exact_justification
            && Self::proposal_is_safe_for_durable_lock(durable, proposal)
    }
    fn proposal_is_safe_for_durable_lock(durable: &DurableState, proposal: &Proposal) -> bool {
        let Some(locked) = durable.locked() else {
            return true;
        };
        let subject = proposal.manifest().subject();
        if locked.subject() == subject {
            return true;
        }
        let ProposalJustification::Timeout(certificate) = proposal.justification() else {
            return false;
        };
        certificate.highest_prepare().is_some_and(|highest| {
            highest.phase() == Phase::Prepare
                && highest.subject() == subject
                && highest.round().view() > locked.round().view()
        })
    }
    fn durable_vote_is_active(durable: &DurableState, vote: Vote) -> bool {
        if durable.decision().is_some() {
            return false;
        }
        match vote.phase() {
            Phase::Prepare => {
                vote.round() == Round::new(durable.height(), durable.current_view())
                    && durable.timeout_intent(vote.round()).is_none()
                    && durable.prepare_intent(vote.round()) == Some(vote)
            }
            Phase::Commit => durable.locked().is_some_and(|locked| {
                locked.round() == vote.proposal_round()
                    && locked.subject() == vote.subject()
                    && durable.commit_intent_for_lock(locked) == Some(vote)
            }),
        }
    }
    fn durable_timeout_vote_is_active(durable: &DurableState, vote: &TimeoutVote) -> bool {
        durable.decision().is_none()
            && vote.round() == Round::new(durable.height(), durable.current_view())
            && durable.timeout_intent(vote.round()) == Some(vote.clone())
    }
    fn outbound_control_is_active(
        context: &HeightContext,
        durable: &DurableState,
        message: &ConsensusMessageV2,
    ) -> bool {
        if let Some(decision) = durable.decision() {
            return matches!(
                message,
                ConsensusMessageV2::QuorumCertificate(certificate)
                    if certificate.phase() == Phase::Commit
                        && decision.reference() == certificate.reference()
                        && certificate.validate(context).is_ok()
            );
        }
        match message {
            ConsensusMessageV2::Proposal(proposal) => {
                Self::durable_proposal_is_active(durable, proposal.proposal())
            }
            ConsensusMessageV2::Vote(vote) => Self::durable_vote_is_active(durable, vote.vote()),
            ConsensusMessageV2::QuorumCertificate(certificate) => match certificate.phase() {
                Phase::Prepare => durable.highest_prepare().is_some_and(|highest| {
                    highest == certificate && certificate.validate(context).is_ok()
                }),
                Phase::Commit => durable.decision().is_some_and(|decision| {
                    decision.reference() == certificate.reference()
                        && certificate.validate(context).is_ok()
                }),
            },
            ConsensusMessageV2::TimeoutVote(vote) => {
                Self::durable_timeout_vote_is_active(durable, &vote.vote())
            }
            ConsensusMessageV2::TimeoutCertificate(certificate) => {
                durable.last_timeout() == Some(certificate)
            }
            ConsensusMessageV2::BodyRequest(_) | ConsensusMessageV2::BodyChunk(_) => false,
        }
    }
    fn prune_inactive_outbound_control(&mut self) {
        let context = &self.context;
        let durable = &self.durable;
        self.outbound_control
            .retain(|_, message| Self::outbound_control_is_active(context, durable, message));
    }
    fn queue_active_locked_commit_signature(&mut self) {
        let Some(locked) = self.durable.locked() else {
            return;
        };
        let Some(vote) = self.durable.commit_intent_for_lock(locked) else {
            return;
        };
        if !Self::durable_vote_is_active(&self.durable, vote) {
            return;
        }
        let message = SignableMessage::Vote(vote);
        if self.awaiting_signature.as_ref() == Some(&message)
            || self.signature_queue.iter().any(|queued| queued == &message)
        {
            return;
        }
        self.signature_queue.push_back(message);
    }
    /// Return whether authenticated certificate progress may supersede one
    /// outstanding local signature task.
    ///
    /// The adapter verifies both certificate variants before constructing
    /// their reducer events, and the reducer validates them again before any
    /// mutation. PrepareQCs deliberately remain behind the signing fence:
    /// only a TC, which installs a new reducer incarnation, or a CommitQC,
    /// which installs the terminal Decision, can retire the old task without
    /// weakening a local voting guard.
    pub(crate) fn certified_progress_bypasses_signature_fence(event: &Event) -> bool {
        matches!(event, Event::TimeoutCertificateReceived { .. })
            || matches!(
                event,
                Event::QuorumCertificateReceived { certificate, .. }
                    if certificate.phase() == Phase::Commit
            )
    }
    /// Park the sole in-flight signing intent before certified progress opens
    /// a new WAL boundary.
    ///
    /// The old adapter task remains externally cancellable until `EnterView`
    /// or Decision reconciliation consumes it. Reducer ownership moves back
    /// to the durable-intent queue first, so pending persistence and an
    /// awaiting signature never overlap. If a same-view TC upgrade leaves the
    /// intent authorized, acknowledgement reissues it under the new
    /// generation; otherwise the durable transition filters it out.
    fn park_awaiting_signature_for_certified_progress(&mut self) {
        let Some(message) = self.awaiting_signature.take() else {
            return;
        };
        if !self.signature_queue.iter().any(|queued| queued == &message) {
            self.signature_queue.push_front(message);
        }
    }
    fn tag_projection(tag: EventTag) -> TagProjection {
        TagProjection {
            height: tag.height(),
            view: tag.view(),
            generation: tag.generation().get(),
        }
    }
    fn validator_projection(validator: Option<ValidatorId>) -> ValidatorProjection {
        ValidatorProjection {
            present: validator.is_some(),
            id: validator.unwrap_or_default(),
        }
    }
    fn subject_projection(subject: Option<Subject>) -> SubjectProjection {
        SubjectProjection {
            present: subject.is_some(),
            subject: subject.unwrap_or_default(),
        }
    }
    fn pending_projection(&self) -> PendingProjection {
        self.pending_persistence
            .as_ref()
            .map_or_else(PendingProjection::default, |pending| {
                let (round, subject) = Self::wal_record_round_subject(pending.entry.record());
                let proposal_round = Self::wal_record_proposal_round(pending.entry.record());
                PendingProjection {
                    record_kind: Self::wal_record_kind(pending.entry.record()),
                    continuation: Self::continuation_kind(&pending.continuation),
                    persistence_id: pending.entry.id().get(),
                    context_id: Self::context_identity_projection(
                        pending.entry.record().context_id(),
                    ),
                    height: round.height(),
                    view: round.view(),
                    proposal_present: proposal_round.is_some(),
                    proposal_height: proposal_round.map_or(0, Round::height),
                    proposal_view: proposal_round.map_or(0, Round::view),
                    subject: Self::subject_identity_projection(subject),
                }
            })
    }
    fn context_identity_projection(context_id: super::ContextId) -> CanonicalIdentityProjection {
        CanonicalIdentityProjection::from_bytes(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_CONSENSUS_CONTEXT,
            *context_id.as_bytes(),
        )
    }
    fn subject_identity_projection(subject: Subject) -> CanonicalIdentityProjection {
        CanonicalIdentityProjection::from_bytes(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_CONSENSUS_SUBJECT,
            *subject.as_bytes(),
        )
    }
    fn certificate_evidence_class(
        certificate: Option<&QuorumCertificate>,
        local: Option<&QuorumCertificate>,
        incoming: Option<&QuorumCertificate>,
    ) -> u8 {
        let Some(certificate) = certificate else {
            return CERTIFICATE_EVIDENCE_ABSENT;
        };
        // LOCAL deliberately wins the equal-local/equal-incoming tie. Full
        // QuorumCertificate equality makes either label semantically harmless,
        // and one deterministic priority keeps every copied position identical.
        if local.is_some_and(|candidate| candidate == certificate) {
            CERTIFICATE_EVIDENCE_LOCAL
        } else if incoming.is_some_and(|candidate| candidate == certificate) {
            CERTIFICATE_EVIDENCE_INCOMING
        } else {
            CERTIFICATE_EVIDENCE_FOREIGN
        }
    }
    fn certificate_signer_projection(
        &self,
        certificate: &QuorumCertificate,
    ) -> Option<(u128, u64, u64, u64)> {
        let roster = self.context.roster();
        // HeightContext::new enforces this protocol bound. Repeat it here so a
        // future constructor or recovery change cannot make the u128 mapping
        // truncate or alias a signer set.
        if roster.len() > MAX_VOTING_ROSTER_LEN || MAX_VOTING_ROSTER_LEN > u128::BITS as usize {
            return None;
        }
        let quorum = certificate.validate(&self.context).ok()?;
        let mut signer_bitmap = 0u128;
        let mut signer_bitmap_count = 0u64;
        for signature in certificate.signatures() {
            let signer = signature.signer();
            let index = roster
                .binary_search_by_key(&signer, |validator| validator.id())
                .ok()?;
            if index >= MAX_VOTING_ROSTER_LEN {
                return None;
            }
            let shift = u32::try_from(index).ok()?;
            let bit = 1u128.checked_shl(shift)?;
            if signer_bitmap & bit != 0 {
                return None;
            }
            signer_bitmap |= bit;
            signer_bitmap_count = signer_bitmap_count.checked_add(1)?;
        }
        let signer_count = u64::try_from(quorum.signer_count()).ok()?;
        if signer_count == 0
            || signer_count > u64::try_from(MAX_VOTING_ROSTER_LEN).ok()?
            || signer_bitmap_count != signer_count
        {
            return None;
        }
        Some((
            signer_bitmap,
            signer_bitmap_count,
            signer_count,
            quorum.voting_power().get(),
        ))
    }
    fn certificate_identity_projection(
        &self,
        certificate: Option<&QuorumCertificate>,
        local: Option<&QuorumCertificate>,
        incoming: Option<&QuorumCertificate>,
    ) -> CertificateIdentityProjection {
        let Some(certificate) = certificate else {
            return CertificateIdentityProjection::default();
        };
        let evidence_class = Self::certificate_evidence_class(Some(certificate), local, incoming);
        let signer_projection = self.certificate_signer_projection(certificate);
        let (signer_bitmap, signer_bitmap_count, signer_count, voting_power) =
            signer_projection.unwrap_or((0, 0, 0, 0));
        CertificateIdentityProjection {
            present: true,
            context_id: Self::context_identity_projection(certificate.reference().context_id()),
            height: certificate.round().height(),
            view: certificate.round().view(),
            phase: Self::phase_code(certificate.phase()),
            subject: Self::subject_identity_projection(certificate.subject()),
            signer_bitmap,
            signer_bitmap_count,
            signer_count,
            voting_power,
            evidence_class: if signer_projection.is_some() {
                evidence_class
            } else {
                CERTIFICATE_EVIDENCE_FOREIGN
            },
        }
    }
    fn timeout_identity_projection(
        &self,
        certificate: Option<&TimeoutCertificate>,
        local: Option<&QuorumCertificate>,
        incoming: Option<&QuorumCertificate>,
    ) -> TimeoutIdentityProjection {
        certificate.map_or_else(TimeoutIdentityProjection::default, |certificate| {
            TimeoutIdentityProjection {
                present: true,
                context_id: Self::context_identity_projection(certificate.context_id()),
                height: certificate.round().height(),
                view: certificate.round().view(),
                highest_prepare: self.certificate_identity_projection(
                    certificate.highest_prepare(),
                    local,
                    incoming,
                ),
            }
        })
    }
    fn enter_view_projection(&self, after: &Self, effects: &[Effect]) -> EnterViewProjection {
        let pending_record_timeout = self.pending_persistence.as_ref().and_then(|pending| {
            if let WalRecord::InstallTimeout(certificate) = pending.entry.record() {
                Some(certificate)
            } else {
                None
            }
        });
        let pending_continuation_timeout = self.pending_persistence.as_ref().and_then(|pending| {
            if let Continuation::InstallTimeout { certificate, .. } = &pending.continuation {
                Some(certificate)
            } else {
                None
            }
        });
        let enter_count = u64::try_from(
            effects
                .iter()
                .filter(|effect| matches!(effect, Effect::EnterView { .. }))
                .count(),
        )
        .unwrap_or(u64::MAX);
        let fetch_count = u64::try_from(
            effects
                .iter()
                .filter(|effect| matches!(effect, Effect::FetchBody { .. }))
                .count(),
        )
        .unwrap_or(u64::MAX);
        let mut enter_index = u8::MAX;
        let mut following_fetch_index = u8::MAX;
        let mut effect_timeout = None;
        let mut effect_protected_lock = None;
        let mut following_fetch_lock = None;
        if let Some((
            index,
            Effect::EnterView {
                certificate,
                protected_lock,
                ..
            },
        )) = effects
            .iter()
            .enumerate()
            .find(|(_, effect)| matches!(effect, Effect::EnterView { .. }))
        {
            enter_index = u8::try_from(index).unwrap_or(u8::MAX);
            effect_timeout = Some(certificate);
            effect_protected_lock = protected_lock.as_ref();
            if let Some(Effect::FetchBody {
                round,
                subject,
                certificate: Some(certificate),
                ..
            }) = effects.get(index.saturating_add(1))
                && *round == certificate.round()
                && *subject == certificate.subject()
            {
                following_fetch_index = u8::try_from(index + 1).unwrap_or(u8::MAX);
                following_fetch_lock = Some(certificate);
            }
        }
        let pending = self.pending_projection();
        let local_lock = self.durable.locked();
        let local_highest = self.durable.highest_prepare();
        let incoming_lock = pending_record_timeout.and_then(TimeoutCertificate::highest_prepare);
        let prepare_control_slot_present_after = after
            .outbound_control
            .contains_key(&OutboundControlClass::PrepareQc);
        let retained_prepare_qc_after = after
            .outbound_control
            .get(&OutboundControlClass::PrepareQc)
            .and_then(|message| match message {
                ConsensusMessageV2::QuorumCertificate(certificate)
                    if certificate.phase() == Phase::Prepare =>
                {
                    Some(certificate)
                }
                _ => None,
            });
        EnterViewProjection {
            active: enter_count != 0,
            context_id: Self::context_identity_projection(self.context.id()),
            before_tag: Self::tag_projection(self.current_tag()),
            after_tag: Self::tag_projection(after.current_tag()),
            pending_record_kind: pending.record_kind,
            pending_continuation: pending.continuation,
            pending_record_timeout: self.timeout_identity_projection(
                pending_record_timeout,
                local_lock,
                incoming_lock,
            ),
            pending_continuation_timeout: self.timeout_identity_projection(
                pending_continuation_timeout,
                local_lock,
                incoming_lock,
            ),
            durable_timeout_after: self.timeout_identity_projection(
                after.durable.last_timeout(),
                local_lock,
                incoming_lock,
            ),
            effect_timeout: self.timeout_identity_projection(
                effect_timeout,
                local_lock,
                incoming_lock,
            ),
            local_lock_before: self.certificate_identity_projection(
                local_lock,
                local_lock,
                incoming_lock,
            ),
            local_highest_before: self.certificate_identity_projection(
                local_highest,
                local_highest,
                incoming_lock,
            ),
            incoming_highest_for_control: self.certificate_identity_projection(
                incoming_lock,
                local_highest,
                incoming_lock,
            ),
            durable_lock_after: self.certificate_identity_projection(
                after.durable.locked(),
                local_lock,
                incoming_lock,
            ),
            durable_highest_after: self.certificate_identity_projection(
                after.durable.highest_prepare(),
                local_highest,
                incoming_lock,
            ),
            prepare_control_slot_present_after,
            retained_prepare_qc_after: self.certificate_identity_projection(
                retained_prepare_qc_after,
                local_highest,
                incoming_lock,
            ),
            effect_protected_lock: self.certificate_identity_projection(
                effect_protected_lock,
                local_lock,
                incoming_lock,
            ),
            following_fetch_lock: self.certificate_identity_projection(
                following_fetch_lock,
                local_lock,
                incoming_lock,
            ),
            enter_count,
            fetch_count,
            enter_index,
            following_fetch_index,
        }
    }
    fn wal_record_round_subject(record: &WalRecord) -> (Round, Subject) {
        match record {
            WalRecord::ProposalIntent(proposal) => {
                (proposal.round(), proposal.manifest().subject())
            }
            WalRecord::PrepareIntent(vote) => (vote.round(), vote.subject()),
            WalRecord::ObservePrepare(certificate) | WalRecord::Decision(certificate) => {
                (certificate.round(), certificate.subject())
            }
            WalRecord::LockAndCommit { vote, .. } => (vote.round(), vote.subject()),
            WalRecord::TimeoutIntent(vote) => (vote.round(), Subject::default()),
            WalRecord::InstallTimeout(certificate) => {
                let subject = certificate
                    .highest_prepare()
                    .map_or_else(Subject::default, QuorumCertificate::subject);
                (certificate.round(), subject)
            }
        }
    }
    fn wal_record_proposal_round(record: &WalRecord) -> Option<Round> {
        match record {
            WalRecord::ProposalIntent(proposal) => Some(proposal.round()),
            WalRecord::PrepareIntent(vote) | WalRecord::LockAndCommit { vote, .. } => {
                Some(vote.proposal_round())
            }
            WalRecord::ObservePrepare(certificate) | WalRecord::Decision(certificate) => {
                Some(certificate.proposal_round())
            }
            WalRecord::TimeoutIntent(_) | WalRecord::InstallTimeout(_) => None,
        }
    }
    const fn continuation_kind(continuation: &Continuation) -> u8 {
        match continuation {
            Continuation::None => CONTINUATION_NONE,
            Continuation::Sign(_) => CONTINUATION_SIGN,
            Continuation::InstallTimeout { .. } => CONTINUATION_INSTALL_TIMEOUT,
            Continuation::Decide { .. } => CONTINUATION_DECIDE,
        }
    }
    fn wal_record_auxiliary_certificate(record: &WalRecord) -> Option<CertificateRef> {
        match record {
            WalRecord::ProposalIntent(proposal) => match proposal.justification() {
                ProposalJustification::ParentCommit(certificate) => *certificate,
                ProposalJustification::Timeout(certificate) => certificate
                    .highest_prepare()
                    .map(QuorumCertificate::reference),
            },
            WalRecord::LockAndCommit { prepare, .. } => Some(prepare.reference()),
            WalRecord::TimeoutIntent(vote) => vote.highest_prepare_ref(),
            WalRecord::InstallTimeout(certificate) => certificate
                .highest_prepare()
                .map(QuorumCertificate::reference),
            WalRecord::PrepareIntent(_) | WalRecord::ObservePrepare(_) | WalRecord::Decision(_) => {
                None
            }
        }
    }
    fn boundary_for_pending(
        kind: u8,
        pending: &PendingPersistence,
        tag: EventTag,
    ) -> BoundaryCapabilityKey {
        let (_, subject) = Self::wal_record_round_subject(pending.entry.record());
        let proposal_round = Self::wal_record_proposal_round(pending.entry.record());
        let mut boundary = BoundaryCapabilityKey {
            kind,
            record_kind: Self::wal_record_kind(pending.entry.record()),
            continuation: Self::continuation_kind(&pending.continuation),
            replay_effect_kind: REPLAY_EFFECT_NONE,
            persistence_id: pending.entry.id().get(),
            context_id: pending.entry.record().context_id(),
            context_identity: Self::context_identity_projection(
                pending.entry.record().context_id(),
            ),
            tag: Self::tag_projection(tag),
            subject: Self::subject_projection(Some(subject)),
            subject_identity: Self::subject_identity_projection(subject),
            proposal_present: proposal_round.is_some(),
            proposal_height: proposal_round.map_or(0, Round::height),
            proposal_view: proposal_round.map_or(0, Round::view),
            replay_plan: ReplayPlanProjection::empty(),
            ..BoundaryCapabilityKey::none()
        };
        if let Some(reference) = Self::wal_record_auxiliary_certificate(pending.entry.record()) {
            boundary.auxiliary_present = true;
            boundary.auxiliary_context_id = reference.context_id();
            boundary.auxiliary_height = reference.round().height();
            boundary.auxiliary_view = reference.round().view();
            boundary.auxiliary_proposal_height = reference.proposal_round().height();
            boundary.auxiliary_proposal_view = reference.proposal_round().view();
            boundary.auxiliary_phase = Self::phase_code(reference.phase());
            boundary.auxiliary_subject = reference.subject();
        }
        boundary
    }
    fn boundary_claim(
        &self,
        event: &Event,
        after: &Self,
        effects: &[Effect],
    ) -> BoundaryCapabilityKey {
        if self.pending_persistence.is_none()
            && let Some(pending) = &after.pending_persistence
        {
            return Self::boundary_for_pending(
                refinement::BOUNDARY_BEGIN_WAL,
                pending,
                after.current_tag(),
            );
        }
        if let Some(pending) = &self.pending_persistence
            && after.pending_persistence.is_none()
            && matches!(event, Event::Persisted { .. })
        {
            return Self::boundary_for_pending(
                refinement::BOUNDARY_ACKNOWLEDGE_WAL,
                pending,
                after.current_tag(),
            );
        }
        if self.applied_subject != after.applied_subject {
            return BoundaryCapabilityKey {
                kind: refinement::BOUNDARY_COMPLETE_APPLICATION,
                context_id: after.context.id(),
                context_identity: Self::context_identity_projection(after.context.id()),
                tag: Self::tag_projection(after.current_tag()),
                subject: Self::subject_projection(after.applied_subject),
                subject_identity: after.applied_subject.map_or_else(
                    CanonicalIdentityProjection::zero,
                    Self::subject_identity_projection,
                ),
                ..BoundaryCapabilityKey::none()
            };
        }
        if !self.replay_resumed
            && after.replay_resumed
            && matches!(event, Event::ResumeAfterReplay { .. })
        {
            return BoundaryCapabilityKey {
                kind: refinement::BOUNDARY_RESUME_AFTER_REPLAY,
                replay_effect_kind: Self::replay_effect_kind(after, effects),
                persistence_id: after.durable.last_id().get(),
                context_id: after.context.id(),
                context_identity: Self::context_identity_projection(after.context.id()),
                tag: Self::tag_projection(after.current_tag()),
                subject: Self::subject_projection(
                    after.durable.decision().map(QuorumCertificate::subject),
                ),
                subject_identity: after
                    .durable
                    .decision()
                    .map_or_else(CanonicalIdentityProjection::zero, |decision| {
                        Self::subject_identity_projection(decision.subject())
                    }),
                replay_plan: Self::observed_replay_plan(after, effects),
                ..BoundaryCapabilityKey::none()
            };
        }
        BoundaryCapabilityKey::none()
    }
    fn boundary_grant(
        &self,
        event: &Event,
        after: &Self,
        effects: &[Effect],
        claimed: BoundaryCapabilityKey,
    ) -> BoundaryCapabilityKey {
        match claimed.kind {
            refinement::BOUNDARY_BEGIN_WAL if self.begin_persist_is_exact(event, after) => after
                .pending_persistence
                .as_ref()
                .map_or_else(BoundaryCapabilityKey::none, |pending| {
                    Self::boundary_for_pending(
                        refinement::BOUNDARY_BEGIN_WAL,
                        pending,
                        after.current_tag(),
                    )
                }),
            refinement::BOUNDARY_ACKNOWLEDGE_WAL if self.acknowledgement_is_exact(event, after) => {
                self.pending_persistence.as_ref().map_or_else(
                    BoundaryCapabilityKey::none,
                    |pending| {
                        Self::boundary_for_pending(
                            refinement::BOUNDARY_ACKNOWLEDGE_WAL,
                            pending,
                            after.current_tag(),
                        )
                    },
                )
            }
            refinement::BOUNDARY_COMPLETE_APPLICATION
                if self.applied_subject != after.applied_subject
                    && self.application_transition_is_exact(event, after) =>
            {
                BoundaryCapabilityKey {
                    kind: refinement::BOUNDARY_COMPLETE_APPLICATION,
                    context_id: after.context.id(),
                    context_identity: Self::context_identity_projection(after.context.id()),
                    tag: Self::tag_projection(after.current_tag()),
                    subject: Self::subject_projection(after.applied_subject),
                    subject_identity: after.applied_subject.map_or_else(
                        CanonicalIdentityProjection::zero,
                        Self::subject_identity_projection,
                    ),
                    ..BoundaryCapabilityKey::none()
                }
            }
            refinement::BOUNDARY_RESUME_AFTER_REPLAY
                if self.resume_after_replay_is_exact(event, after, effects) =>
            {
                let replay_plan = self.expected_replay_plan();
                BoundaryCapabilityKey {
                    kind: refinement::BOUNDARY_RESUME_AFTER_REPLAY,
                    replay_effect_kind: Self::first_replay_plan_kind(replay_plan),
                    persistence_id: after.durable.last_id().get(),
                    context_id: after.context.id(),
                    context_identity: Self::context_identity_projection(after.context.id()),
                    tag: Self::tag_projection(after.current_tag()),
                    subject: Self::subject_projection(
                        after.durable.decision().map(QuorumCertificate::subject),
                    ),
                    subject_identity: after
                        .durable
                        .decision()
                        .map_or_else(CanonicalIdentityProjection::zero, |decision| {
                            Self::subject_identity_projection(decision.subject())
                        }),
                    replay_plan,
                    ..BoundaryCapabilityKey::none()
                }
            }
            _ => BoundaryCapabilityKey::none(),
        }
    }
    fn begin_persist_is_exact(&self, event: &Event, after: &Self) -> bool {
        let (None, Some(pending)) = (
            self.pending_persistence.as_ref(),
            after.pending_persistence.as_ref(),
        ) else {
            return false;
        };
        if self.durable != after.durable
            || self.generation != after.generation
            || self.applied_subject != after.applied_subject
            || self.durable.next_id() != Ok(pending.entry.id())
            || !Self::continuation_matches_record(pending.entry.record(), &pending.continuation)
            || !self.event_may_start_record(event, pending.entry.record())
        {
            return false;
        }
        let mut expected = self.durable.clone();
        if expected
            .apply(&self.context, self.local_validator, &pending.entry)
            .is_err()
        {
            return false;
        }
        match pending.entry.record() {
            WalRecord::ProposalIntent(proposal) => {
                after.body_state(proposal.round(), proposal.manifest().subject())
                    == BodyState::Validated
            }
            WalRecord::PrepareIntent(vote) => {
                after.body_state(vote.round(), vote.subject()) == BodyState::Validated
            }
            WalRecord::LockAndCommit { prepare, .. } => {
                after.body_state(prepare.round(), prepare.subject()) == BodyState::Validated
            }
            WalRecord::ObservePrepare(_)
            | WalRecord::TimeoutIntent(_)
            | WalRecord::InstallTimeout(_)
            | WalRecord::Decision(_) => true,
        }
    }
    #[allow(clippy::too_many_lines)]
    fn event_may_start_record(&self, event: &Event, record: &WalRecord) -> bool {
        match (event, record) {
            (Event::LocalProposalReady { manifest, .. }, WalRecord::ProposalIntent(proposal)) => {
                proposal.manifest() == manifest
            }
            (
                Event::ValidationCompleted {
                    round,
                    subject,
                    valid: true,
                    ..
                },
                WalRecord::PrepareIntent(vote),
            ) => vote.round() == *round && vote.subject() == *subject,
            (
                Event::ValidationCompleted {
                    round,
                    subject,
                    valid: true,
                    ..
                },
                WalRecord::LockAndCommit { prepare, .. },
            ) => prepare.round() == *round && prepare.subject() == *subject,
            (Event::RetransmitElapsed { .. }, WalRecord::PrepareIntent(vote)) => {
                self.candidate.as_ref().is_some_and(|proposal| {
                    vote.round() == proposal.round()
                        && vote.subject() == proposal.manifest().subject()
                        && self.body_state(vote.round(), vote.subject()) == BodyState::Validated
                })
            }
            (Event::RetransmitElapsed { .. }, WalRecord::LockAndCommit { prepare, .. }) => {
                self.pending_prepare
                    .get(&prepare.reference())
                    .is_some_and(|retained| retained == prepare)
                    && self.body_state(prepare.round(), prepare.subject()) == BodyState::Validated
            }
            (Event::TimeoutElapsed { .. }, WalRecord::TimeoutIntent(vote)) => {
                vote.round() == Round::new(self.context.height(), self.durable.current_view())
            }
            (Event::VoteReceived { vote, .. }, WalRecord::ObservePrepare(certificate)) => {
                vote.vote().phase() == Phase::Prepare
                    && certificate.reference()
                        == CertificateRef::new_with_proposal_round(
                            self.context.id(),
                            vote.vote().round(),
                            vote.vote().proposal_round(),
                            Phase::Prepare,
                            vote.vote().subject(),
                        )
            }
            (Event::VoteReceived { vote, .. }, WalRecord::LockAndCommit { prepare, .. }) => {
                vote.vote().phase() == Phase::Prepare
                    && prepare.reference()
                        == CertificateRef::new_with_proposal_round(
                            self.context.id(),
                            vote.vote().round(),
                            vote.vote().proposal_round(),
                            Phase::Prepare,
                            vote.vote().subject(),
                        )
            }
            (Event::VoteReceived { vote, .. }, WalRecord::Decision(certificate)) => {
                vote.vote().phase() == Phase::Commit
                    && certificate.reference()
                        == CertificateRef::new_with_proposal_round(
                            self.context.id(),
                            vote.vote().round(),
                            vote.vote().proposal_round(),
                            Phase::Commit,
                            vote.vote().subject(),
                        )
            }
            (
                Event::QuorumCertificateReceived { certificate, .. },
                WalRecord::ObservePrepare(record),
            ) => certificate.phase() == Phase::Prepare && certificate == record,
            (
                Event::QuorumCertificateReceived { certificate, .. },
                WalRecord::LockAndCommit { prepare, .. },
            ) => certificate.phase() == Phase::Prepare && certificate == prepare,
            (Event::QuorumCertificateReceived { certificate, .. }, WalRecord::Decision(record)) => {
                certificate.phase() == Phase::Commit && certificate == record
            }
            (Event::TimeoutVoteReceived { vote, .. }, WalRecord::InstallTimeout(certificate)) => {
                vote.vote().round() == certificate.round()
            }
            (
                Event::TimeoutCertificateReceived { certificate, .. },
                WalRecord::InstallTimeout(record),
            ) => certificate == record,
            (Event::Signed { .. }, record) => {
                self.awaiting_signature
                    .as_ref()
                    .is_some_and(|message| match (message, record) {
                        (SignableMessage::Proposal(proposal), WalRecord::PrepareIntent(vote)) => {
                            vote.phase() == Phase::Prepare
                                && vote.round() == proposal.round()
                                && vote.subject() == proposal.manifest().subject()
                        }
                        (SignableMessage::Vote(vote), WalRecord::ObservePrepare(certificate)) => {
                            vote.phase() == Phase::Prepare
                                && certificate.reference()
                                    == CertificateRef::new_with_proposal_round(
                                        self.context.id(),
                                        vote.round(),
                                        vote.proposal_round(),
                                        Phase::Prepare,
                                        vote.subject(),
                                    )
                        }
                        (SignableMessage::Vote(vote), WalRecord::LockAndCommit { prepare, .. }) => {
                            vote.phase() == Phase::Prepare
                                && prepare.reference()
                                    == CertificateRef::new_with_proposal_round(
                                        self.context.id(),
                                        vote.round(),
                                        vote.proposal_round(),
                                        Phase::Prepare,
                                        vote.subject(),
                                    )
                        }
                        (SignableMessage::Vote(vote), WalRecord::Decision(certificate)) => {
                            vote.phase() == Phase::Commit
                                && certificate.reference()
                                    == CertificateRef::new_with_proposal_round(
                                        self.context.id(),
                                        vote.round(),
                                        vote.proposal_round(),
                                        Phase::Commit,
                                        vote.subject(),
                                    )
                        }
                        (
                            SignableMessage::TimeoutVote(vote),
                            WalRecord::InstallTimeout(certificate),
                        ) => vote.round() == certificate.round(),
                        _ => false,
                    })
            }
            _ => false,
        }
    }
    fn acknowledgement_is_exact(&self, event: &Event, after: &Self) -> bool {
        let (Some(pending), Event::Persisted { id, .. }, None) = (
            self.pending_persistence.as_ref(),
            event,
            after.pending_persistence.as_ref(),
        ) else {
            return false;
        };
        if *id != pending.entry.id() || self.applied_subject != after.applied_subject {
            return false;
        }
        let mut expected = self.durable.clone();
        if expected
            .apply(&self.context, self.local_validator, &pending.entry)
            .is_err()
            || expected != after.durable
        {
            return false;
        }
        match &pending.continuation {
            Continuation::None => self.generation == after.generation,
            Continuation::Sign(_) => {
                self.generation == after.generation && after.awaiting_signature.is_some()
            }
            Continuation::InstallTimeout { certificate, .. } => {
                self.generation_after_timeout_install(certificate) == Some(after.generation)
                    && after.durable.last_timeout() == Some(certificate)
                    && after.candidate.is_none()
                    && after.pending_prepare.is_empty()
                    && after.durable.locked().map_or_else(
                        || after.body_work.is_empty(),
                        |locked| {
                            after.body_work.len() == 1
                                && after.body_state(locked.round(), locked.subject())
                                    == BodyState::Missing
                        },
                    )
            }
            Continuation::Decide { certificate, .. } => {
                let key = (
                    after.decision_body_round(certificate),
                    certificate.subject(),
                );
                let expected_work = self.body_work.get(&key).cloned().unwrap_or(BodyWork {
                    manifest: None,
                    state: BodyState::Missing,
                });
                self.generation == after.generation
                    && self.replay_resumed == after.replay_resumed
                    && after.durable.decision() == Some(certificate)
                    && after.candidate.is_none()
                    && after.candidate_signed.is_none()
                    && after.body_work.len() == 1
                    && after.body_work.get(&key) == Some(&expected_work)
                    && after.pending_prepare.is_empty()
                    && after.known_prepare.is_empty()
                    && after.votes.is_empty()
                    && after.timeout_votes.is_empty()
                    && after.formed_certificates.is_empty()
                    && after.formed_timeouts.is_empty()
                    && after.awaiting_signature.is_none()
                    && after.signature_queue.is_empty()
                    && after.outbound_control.len() == 1
                    && matches!(
                        after
                            .outbound_control
                            .get(&OutboundControlClass::CommitQc),
                        Some(ConsensusMessageV2::QuorumCertificate(retained))
                            if retained == certificate
                    )
            }
        }
    }
    /// Compute the next process-local completion generation for one exact TC
    /// install.
    ///
    /// A normal install changes the view, whose `EventTag` component already
    /// fences every old callback, and therefore starts generation zero.  A
    /// strict alternate certificate for the timeout round which installed the
    /// current view leaves the view unchanged, so it must consume one checked
    /// generation.  The check is performed before WAL application to retain
    /// the atomic fail-stop overflow boundary for malformed/unreachable state.
    fn generation_after_timeout_install(
        &self,
        certificate: &TimeoutCertificate,
    ) -> Option<Generation> {
        if self
            .durable
            .is_strict_same_round_timeout_upgrade(certificate)
        {
            self.generation.next()
        } else {
            Some(Generation::INITIAL)
        }
    }
    fn application_transition_is_exact(&self, event: &Event, after: &Self) -> bool {
        if self.applied_subject == after.applied_subject {
            return true;
        }
        let (None, Some(applied), Event::ApplicationCompleted { subject, .. }) =
            (self.applied_subject, after.applied_subject, event)
        else {
            return false;
        };
        if applied != *subject {
            return false;
        }
        after.durable.decision().is_some_and(|decision| {
            decision.subject() == *subject
                && after.body_state(after.decision_body_round(decision), *subject)
                    == BodyState::Validated
        })
    }
    fn resume_after_replay_is_exact(
        &self,
        event: &Event,
        after: &Self,
        effects: &[Effect],
    ) -> bool {
        if self.replay_resumed || !matches!(event, Event::ResumeAfterReplay { .. }) {
            return false;
        }
        let Some((expected, expected_effects)) = self.independent_replay_transition() else {
            return false;
        };
        expected == *after
            && expected_effects == effects
            && Self::observed_replay_plan(after, effects) == self.expected_replay_plan()
    }
    /// Derive the complete recovery FIFO from durable sources without calling
    /// `on_resume_after_replay` or `drive_signature`.
    ///
    /// Keeping this derivation separate is intentional: the refinement gate
    /// must reject an implementation that preserves the first replay effect
    /// while omitting or reordering later durable intents.
    fn expected_replay_signatures(&self) -> Vec<SignableMessage> {
        if self.durable.decision().is_some() {
            return Vec::new();
        }
        let mut messages = Vec::with_capacity(3);
        let round = Round::new(self.context.height(), self.durable.current_view());
        if let Some(timeout) = self.durable.timeout_intent(round) {
            messages.push(SignableMessage::TimeoutVote(timeout));
        } else if let Some(proposal) = self
            .durable
            .proposal_intent(round)
            .filter(|proposal| Self::durable_proposal_is_active(&self.durable, proposal))
        {
            messages.push(SignableMessage::Proposal(proposal.clone()));
        }
        if self.durable.timeout_intent(round).is_none()
            && let Some(vote) = self.durable.prepare_intent(round)
            && vote.round() == round
            && vote.phase() == Phase::Prepare
        {
            messages.push(SignableMessage::Vote(vote));
        }
        if let Some(vote) = self
            .durable
            .locked()
            .and_then(|locked| self.durable.commit_intent_for_lock(locked))
        {
            messages.push(SignableMessage::Vote(vote));
        }
        messages
    }
    fn replay_kind_for_signable(message: &SignableMessage) -> u8 {
        match message {
            SignableMessage::Proposal(_) => REPLAY_EFFECT_PROPOSAL,
            SignableMessage::Vote(vote) => match vote.phase() {
                Phase::Prepare => REPLAY_EFFECT_PREPARE,
                Phase::Commit => REPLAY_EFFECT_COMMIT,
            },
            SignableMessage::TimeoutVote(_) => REPLAY_EFFECT_TIMEOUT,
        }
    }
    fn replay_sign_capability(&self, message: &SignableMessage) -> EffectCapabilityKey {
        let mut capability = EffectCapabilityKey {
            kind: EFFECT_SIGN,
            tag: Self::tag_projection(self.current_tag()),
            ..EffectCapabilityKey::none()
        };
        Self::apply_signable(&mut capability, message);
        capability
    }
    fn expected_decision_fetch(&self) -> Option<Effect> {
        let certificate = self.durable.decision()?.clone();
        let round = self.decision_body_round(&certificate);
        let subject = certificate.subject();
        if self.body_state(round, subject) != BodyState::Missing {
            return None;
        }
        Some(Effect::FetchBody {
            tag: self.current_tag(),
            round,
            subject,
            manifest: self
                .body_work
                .get(&(round, subject))
                .and_then(|work| work.manifest),
            certified_sources: self.frozen_archive_sources(),
            certificate: Some(certificate),
        })
    }
    fn push_replay_plan(
        plan: &mut ReplayPlanProjection,
        kind: u8,
        capability: EffectCapabilityKey,
    ) {
        if !plan.push(kind, capability) {
            // `len = 4` is the canonical fail-closed sentinel beyond the
            // protocol's three-item recovery bound.
            plan.len = 4;
        }
    }
    fn expected_replay_plan(&self) -> ReplayPlanProjection {
        let mut plan = ReplayPlanProjection::empty();
        if let Some(effect) = self.expected_decision_fetch() {
            Self::push_replay_plan(
                &mut plan,
                REPLAY_EFFECT_DECISION,
                Self::effect_capability(&effect),
            );
            return plan;
        }
        for message in self.expected_replay_signatures() {
            let kind = Self::replay_kind_for_signable(&message);
            let capability = self.replay_sign_capability(&message);
            Self::push_replay_plan(&mut plan, kind, capability);
        }
        plan
    }
    fn observed_replay_plan(after: &Self, effects: &[Effect]) -> ReplayPlanProjection {
        let mut plan = ReplayPlanProjection::empty();
        if after.durable.decision().is_some() {
            for effect in effects {
                let kind = match effect {
                    Effect::FetchBody { .. } => REPLAY_EFFECT_DECISION,
                    Effect::Sign { message, .. } => Self::replay_kind_for_signable(message),
                    _ => u8::MAX,
                };
                Self::push_replay_plan(&mut plan, kind, Self::effect_capability(effect));
            }
            return plan;
        }
        if let Some(message) = &after.awaiting_signature {
            Self::push_replay_plan(
                &mut plan,
                Self::replay_kind_for_signable(message),
                after.replay_sign_capability(message),
            );
        }
        for message in &after.signature_queue {
            Self::push_replay_plan(
                &mut plan,
                Self::replay_kind_for_signable(message),
                after.replay_sign_capability(message),
            );
        }
        plan
    }
    const fn first_replay_plan_kind(plan: ReplayPlanProjection) -> u8 {
        if plan.len == 0 {
            REPLAY_EFFECT_NONE
        } else {
            plan.slot0.kind
        }
    }
    /// Independently materialize the exact post-replay state and first effect.
    /// This must remain structurally separate from the implementation under
    /// test so a shared omission or reordering cannot grant itself authority.
    fn independent_replay_transition(&self) -> Option<(Self, Vec<Effect>)> {
        if self.replay_resumed
            || self.pending_persistence.is_some()
            || self.awaiting_signature.is_some()
            || !self.signature_queue.is_empty()
        {
            return None;
        }
        let mut expected = self.clone();
        expected.replay_resumed = true;
        if self.durable.decision().is_some() {
            let effect = self.expected_decision_fetch()?;
            let (round, subject) = match &effect {
                Effect::FetchBody { round, subject, .. } => (*round, *subject),
                _ => unreachable!("the independent Decision frontier is one FetchBody"),
            };
            expected
                .body_work
                .entry((round, subject))
                .or_insert(BodyWork {
                    manifest: None,
                    state: BodyState::Missing,
                });
            return Some((expected, vec![effect]));
        }
        expected
            .signature_queue
            .extend(self.expected_replay_signatures());
        let Some(message) = expected.signature_queue.pop_front() else {
            return Some((expected, Vec::new()));
        };
        if !expected.signable_is_durably_authorized(&message) {
            return None;
        }
        expected.awaiting_signature = Some(message.clone());
        let effect = Effect::Sign {
            tag: expected.current_tag(),
            message,
        };
        Some((expected, vec![effect]))
    }
    fn signable_message_kind(message: &SignableMessage) -> u8 {
        match message {
            SignableMessage::Proposal(_) => SIGNED_MESSAGE_PROPOSAL,
            SignableMessage::Vote(vote) => match vote.phase() {
                Phase::Prepare => SIGNED_MESSAGE_PREPARE,
                Phase::Commit => SIGNED_MESSAGE_COMMIT,
            },
            SignableMessage::TimeoutVote(_) => SIGNED_MESSAGE_TIMEOUT,
        }
    }
    fn replay_effect_kind(after: &Self, effects: &[Effect]) -> u8 {
        match effects {
            [] => REPLAY_EFFECT_NONE,
            [Effect::Sign { message, .. }] => match message {
                SignableMessage::Proposal(_) => REPLAY_EFFECT_PROPOSAL,
                SignableMessage::Vote(vote) => match vote.phase() {
                    Phase::Prepare => REPLAY_EFFECT_PREPARE,
                    Phase::Commit => REPLAY_EFFECT_COMMIT,
                },
                SignableMessage::TimeoutVote(_) => REPLAY_EFFECT_TIMEOUT,
            },
            [
                Effect::FetchBody {
                    certificate: Some(certificate),
                    ..
                },
            ] if after
                .durable
                .decision()
                .is_some_and(|decision| decision == certificate) =>
            {
                REPLAY_EFFECT_DECISION
            }
            _ => u8::MAX,
        }
    }
    const fn event_kind(event: &Event) -> u8 {
        match event {
            Event::ResumeAfterReplay { .. } => EVENT_RESUME_AFTER_REPLAY,
            Event::LocalProposalReady { .. } => 0,
            Event::ProposalReceived { .. } => 1,
            Event::VoteReceived { .. } => 2,
            Event::QuorumCertificateReceived { .. } => 3,
            Event::TimeoutVoteReceived { .. } => 4,
            Event::TimeoutCertificateReceived { .. } => 5,
            Event::TimeoutElapsed { .. } => 6,
            Event::RetransmitElapsed { .. } => 7,
            Event::BodyAvailable { .. } => EVENT_BODY_AVAILABLE,
            Event::BodyStored { .. } => EVENT_BODY_STORED,
            Event::ValidationCompleted { .. } => 10,
            Event::Persisted { .. } => EVENT_PERSISTED,
            Event::PersistenceFailed { .. } => EVENT_PERSISTENCE_FAILED,
            Event::Signed { .. } => EVENT_SIGNED,
            Event::ApplicationCompleted { .. } => 14,
        }
    }
    const fn effect_kind(effect: &Effect) -> u8 {
        match effect {
            Effect::Persist { .. } => EFFECT_PERSIST,
            Effect::FetchBody { .. } => EFFECT_FETCH,
            Effect::StoreBody { .. } => EFFECT_STORE,
            Effect::ValidateBody { .. } => EFFECT_VALIDATE,
            Effect::Sign { .. } => EFFECT_SIGN,
            Effect::Broadcast(_) => EFFECT_BROADCAST,
            Effect::Apply { .. } => EFFECT_APPLY,
            Effect::EnterView { .. } => EFFECT_ENTER_VIEW,
            Effect::ReportEquivocation { .. } | Effect::ReportInvalidCertifiedBody { .. } => {
                EFFECT_REPORT
            }
        }
    }
    const fn wal_record_kind(record: &WalRecord) -> u8 {
        match record {
            WalRecord::ProposalIntent(_) => WAL_RECORD_PROPOSAL_INTENT,
            WalRecord::PrepareIntent(_) => WAL_RECORD_PREPARE_INTENT,
            WalRecord::ObservePrepare(_) => WAL_RECORD_OBSERVE_PREPARE,
            WalRecord::LockAndCommit { .. } => WAL_RECORD_LOCK_AND_COMMIT,
            WalRecord::TimeoutIntent(_) => WAL_RECORD_TIMEOUT_INTENT,
            WalRecord::InstallTimeout(_) => WAL_RECORD_INSTALL_TIMEOUT,
            WalRecord::Decision(_) => WAL_RECORD_DECISION,
        }
    }
    fn cardinality(value: usize) -> u64 {
        u64::try_from(value).unwrap_or(u64::MAX)
    }
    fn volatile_summary(&self) -> VolatileSummary {
        let vote_entries = self
            .votes
            .values()
            .fold(0usize, |total, pool| total.saturating_add(pool.len()));
        let timeout_vote_entries = self
            .timeout_votes
            .values()
            .fold(0usize, |total, pool| total.saturating_add(pool.len()));
        let durable_signable_limit = if self.durable.decision().is_some() {
            0
        } else {
            1usize
                .saturating_add(self.durable.prepare_intents().count())
                .saturating_add(self.durable.commit_intents().count())
        };
        VolatileSummary {
            candidate_present: self.candidate.is_some(),
            fallback_active: self.fallback_active,
            body_work: Self::cardinality(self.body_work.len()),
            pending_prepare: Self::cardinality(self.pending_prepare.len()),
            known_prepare: Self::cardinality(self.known_prepare.len()),
            vote_pools: Self::cardinality(self.votes.len()),
            vote_entries: Self::cardinality(vote_entries),
            timeout_vote_pools: Self::cardinality(self.timeout_votes.len()),
            timeout_vote_entries: Self::cardinality(timeout_vote_entries),
            formed_certificates: Self::cardinality(self.formed_certificates.len()),
            formed_timeouts: Self::cardinality(self.formed_timeouts.len()),
            outbound_control: Self::cardinality(self.outbound_control.len()),
            signature_queue: Self::cardinality(self.signature_queue.len()),
            awaiting_signature: self.awaiting_signature.is_some(),
            durable_signable_limit: Self::cardinality(durable_signable_limit),
            replay_resumed: self.replay_resumed,
        }
    }
    const fn phase_code(phase: Phase) -> u8 {
        match phase {
            Phase::Prepare => 1,
            Phase::Commit => 2,
        }
    }
    fn apply_primary_certificate(key: &mut EffectCapabilityKey, reference: CertificateRef) {
        key.context_id = reference.context_id();
        key.height = reference.round().height();
        key.view = reference.round().view();
        key.proposal_height = reference.proposal_round().height();
        key.proposal_view = reference.proposal_round().view();
        key.phase = Self::phase_code(reference.phase());
        key.subject = reference.subject();
    }
    fn apply_auxiliary_certificate(key: &mut EffectCapabilityKey, reference: CertificateRef) {
        key.auxiliary_present = true;
        key.auxiliary_context_id = reference.context_id();
        key.auxiliary_height = reference.round().height();
        key.auxiliary_view = reference.round().view();
        key.auxiliary_proposal_height = reference.proposal_round().height();
        key.auxiliary_proposal_view = reference.proposal_round().view();
        key.auxiliary_phase = Self::phase_code(reference.phase());
        key.auxiliary_subject = reference.subject();
    }
    fn apply_manifest(key: &mut EffectCapabilityKey, manifest: PayloadManifest) {
        key.subject = manifest.subject();
        key.manifest_payload = manifest.payload_hash();
        key.manifest_chunks = manifest.chunk_root();
        key.manifest_len = manifest.byte_len();
        key.manifest_count = u64::from(manifest.chunk_count());
    }
    fn apply_proposal(key: &mut EffectCapabilityKey, proposal: &Proposal) {
        key.context_id = proposal.context_id();
        key.height = proposal.round().height();
        key.view = proposal.round().view();
        key.proposal_height = proposal.round().height();
        key.proposal_view = proposal.round().view();
        key.actor = proposal.proposer();
        Self::apply_manifest(key, *proposal.manifest());
        let justification = match proposal.justification() {
            ProposalJustification::ParentCommit(parent) => *parent,
            ProposalJustification::Timeout(certificate) => certificate
                .highest_prepare()
                .map(QuorumCertificate::reference),
        };
        if let Some(reference) = justification {
            Self::apply_auxiliary_certificate(key, reference);
        }
    }
    fn apply_vote(key: &mut EffectCapabilityKey, vote: Vote) {
        key.context_id = vote.context_id();
        key.height = vote.round().height();
        key.view = vote.round().view();
        key.proposal_height = vote.proposal_round().height();
        key.proposal_view = vote.proposal_round().view();
        key.phase = Self::phase_code(vote.phase());
        key.subject = vote.subject();
        key.actor = vote.signer();
    }
    fn apply_timeout_vote(key: &mut EffectCapabilityKey, vote: &TimeoutVote) {
        key.context_id = vote.context_id();
        key.height = vote.round().height();
        key.view = vote.round().view();
        key.actor = vote.signer();
        if let Some(reference) = vote.highest_prepare_ref() {
            Self::apply_auxiliary_certificate(key, reference);
        }
    }
    fn apply_timeout_certificate(key: &mut EffectCapabilityKey, certificate: &TimeoutCertificate) {
        key.context_id = certificate.context_id();
        key.height = certificate.round().height();
        key.view = certificate.round().view();
        if let Some(reference) = certificate
            .highest_prepare()
            .map(QuorumCertificate::reference)
        {
            Self::apply_auxiliary_certificate(key, reference);
        }
    }
    fn apply_wal_record(key: &mut EffectCapabilityKey, record: &WalRecord) {
        key.record_kind = Self::wal_record_kind(record);
        match record {
            WalRecord::ProposalIntent(proposal) => Self::apply_proposal(key, proposal),
            WalRecord::PrepareIntent(vote) => Self::apply_vote(key, *vote),
            WalRecord::ObservePrepare(certificate) | WalRecord::Decision(certificate) => {
                Self::apply_primary_certificate(key, certificate.reference());
            }
            WalRecord::LockAndCommit { prepare, vote } => {
                Self::apply_vote(key, *vote);
                Self::apply_auxiliary_certificate(key, prepare.reference());
            }
            WalRecord::TimeoutIntent(vote) => Self::apply_timeout_vote(key, vote),
            WalRecord::InstallTimeout(certificate) => {
                Self::apply_timeout_certificate(key, certificate);
                if let Some(highest_prepare) = certificate.highest_prepare() {
                    key.subject = highest_prepare.subject();
                }
            }
        }
    }
    fn apply_signable(key: &mut EffectCapabilityKey, message: &SignableMessage) {
        match message {
            SignableMessage::Proposal(proposal) => Self::apply_proposal(key, proposal),
            SignableMessage::Vote(vote) => Self::apply_vote(key, *vote),
            SignableMessage::TimeoutVote(vote) => Self::apply_timeout_vote(key, vote),
        }
    }
    fn apply_consensus_message(key: &mut EffectCapabilityKey, message: &ConsensusMessageV2) {
        match message {
            ConsensusMessageV2::Proposal(proposal) => {
                Self::apply_proposal(key, proposal.proposal());
            }
            ConsensusMessageV2::Vote(vote) => Self::apply_vote(key, vote.vote()),
            ConsensusMessageV2::QuorumCertificate(certificate) => {
                Self::apply_primary_certificate(key, certificate.reference());
            }
            ConsensusMessageV2::TimeoutVote(vote) => {
                Self::apply_timeout_vote(key, &vote.vote());
            }
            ConsensusMessageV2::TimeoutCertificate(certificate) => {
                Self::apply_timeout_certificate(key, certificate);
            }
            ConsensusMessageV2::BodyRequest(subject) => key.subject = *subject,
            ConsensusMessageV2::BodyChunk(chunk) => key.subject = chunk.subject(),
        }
    }
    fn effect_capability(effect: &Effect) -> EffectCapabilityKey {
        let mut key = EffectCapabilityKey {
            kind: Self::effect_kind(effect),
            ..EffectCapabilityKey::none()
        };
        match effect {
            Effect::Persist { tag, entry } => {
                key.tag = Self::tag_projection(*tag);
                key.persistence_id = entry.id().get();
                Self::apply_wal_record(&mut key, entry.record());
            }
            Effect::FetchBody {
                tag,
                round,
                subject,
                manifest,
                certificate,
                ..
            } => {
                key.tag = Self::tag_projection(*tag);
                key.height = round.height();
                key.view = round.view();
                key.subject = *subject;
                if let Some(manifest) = manifest {
                    Self::apply_manifest(&mut key, *manifest);
                }
                if let Some(certificate) = certificate {
                    Self::apply_auxiliary_certificate(&mut key, certificate.reference());
                }
            }
            Effect::StoreBody {
                tag,
                round,
                subject,
            }
            | Effect::ValidateBody {
                tag,
                round,
                subject,
            } => {
                key.tag = Self::tag_projection(*tag);
                key.height = round.height();
                key.view = round.view();
                key.subject = *subject;
            }
            Effect::Sign { tag, message } => {
                key.tag = Self::tag_projection(*tag);
                Self::apply_signable(&mut key, message);
            }
            Effect::Broadcast(message) => Self::apply_consensus_message(&mut key, message),
            Effect::Apply {
                tag,
                subject,
                certificate,
            } => {
                key.tag = Self::tag_projection(*tag);
                Self::apply_primary_certificate(&mut key, certificate.reference());
                key.subject = *subject;
            }
            Effect::EnterView {
                tag, certificate, ..
            } => {
                key.tag = Self::tag_projection(*tag);
                Self::apply_timeout_certificate(&mut key, certificate);
            }
            Effect::ReportEquivocation { evidence } => {
                let round = evidence.round();
                key.actor = evidence.offender();
                key.height = round.height();
                key.view = round.view();
                key.phase = match evidence.kind() {
                    EquivocationKind::Vote => 1,
                    EquivocationKind::Timeout => 2,
                    EquivocationKind::Proposal => 3,
                };
            }
            Effect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => {
                Self::apply_primary_certificate(&mut key, certificate.reference());
                key.subject = *subject;
            }
        }
        key
    }
    #[allow(clippy::too_many_lines)]
    fn granted_effect_capability(
        &self,
        event: &Event,
        after: &Self,
        effect: &Effect,
    ) -> EffectCapabilityKey {
        let granted = match effect {
            Effect::Persist { entry, .. } => {
                after.pending_persistence.as_ref().and_then(|pending| {
                    (pending.entry == *entry
                        && self.event_may_start_record(event, pending.entry.record())
                        && after.record_is_role_eligible(pending.entry.record()))
                    .then(|| {
                        let mut key = EffectCapabilityKey {
                            kind: EFFECT_PERSIST,
                            tag: Self::tag_projection(after.current_tag()),
                            persistence_id: pending.entry.id().get(),
                            ..EffectCapabilityKey::none()
                        };
                        Self::apply_wal_record(&mut key, pending.entry.record());
                        key
                    })
                })
            }
            Effect::FetchBody {
                round,
                subject,
                certified_sources,
                certificate,
                ..
            } => {
                let work = after.body_work.get(&(*round, *subject));
                let certificate_valid =
                    certificate
                        .as_ref()
                        .map_or(certified_sources.is_empty(), |certificate| {
                            certificate.proposal_round() == *round
                                && certificate.subject() == *subject
                                && certificate.validate(&after.context).is_ok()
                                && certified_sources == &after.frozen_archive_sources()
                        });
                let role_eligible = certificate.as_ref().map_or_else(
                    || after.local_candidate_body_eligible(),
                    |certificate| {
                        after
                            .durable
                            .decision()
                            .is_some_and(|decision| decision == certificate)
                            || after
                                .durable
                                .locked()
                                .is_some_and(|locked| locked == certificate)
                            || after.local_certified_candidate_body_eligible()
                    },
                );
                if round.height() != after.context.height() || !certificate_valid || !role_eligible
                {
                    None
                } else {
                    work.map(|work| {
                        let mut key = EffectCapabilityKey {
                            kind: EFFECT_FETCH,
                            tag: Self::tag_projection(after.current_tag()),
                            height: round.height(),
                            view: round.view(),
                            subject: *subject,
                            ..EffectCapabilityKey::none()
                        };
                        if let Some(manifest) = work.manifest {
                            Self::apply_manifest(&mut key, manifest);
                        }
                        if let Some(certificate) = certificate {
                            Self::apply_auxiliary_certificate(&mut key, certificate.reference());
                        }
                        key
                    })
                }
            }
            Effect::StoreBody { round, subject, .. } => {
                let newly_available = matches!(
                    event,
                    Event::BodyAvailable {
                        round: event_round,
                        subject: event_subject,
                        ..
                    } if event_round == round && event_subject == subject
                );
                let retransmit_retry = matches!(event, Event::RetransmitElapsed { .. })
                    && (after.durable.decision().is_some_and(|decision| {
                        after.decision_body_round(decision) == *round
                            && decision.subject() == *subject
                    }) || (after.durable.decision().is_none()
                        && after.durable.locked().is_some_and(|locked| {
                            locked.round() == *round && locked.subject() == *subject
                        }))
                        || after.pending_prepare.values().any(|certificate| {
                            certificate.round() == *round && certificate.subject() == *subject
                        })
                        || (after.fallback_active
                            && after.candidate.as_ref().is_some_and(|proposal| {
                                proposal.round() == *round
                                    && proposal.manifest().subject() == *subject
                            })));
                (after.body_state(*round, *subject) == BodyState::Available
                    && (newly_available || retransmit_retry))
                    .then(|| EffectCapabilityKey {
                        kind: EFFECT_STORE,
                        tag: Self::tag_projection(after.current_tag()),
                        height: round.height(),
                        view: round.view(),
                        subject: *subject,
                        ..EffectCapabilityKey::none()
                    })
            }
            Effect::ValidateBody { round, subject, .. } => {
                let newly_durable = matches!(
                    event,
                    Event::BodyStored {
                        round: event_round,
                        subject: event_subject,
                        ..
                    } if event_round == round && event_subject == subject
                );
                let retransmit_retry = matches!(event, Event::RetransmitElapsed { .. })
                    && (after.durable.decision().is_some_and(|decision| {
                        after.decision_body_round(decision) == *round
                            && decision.subject() == *subject
                    }) || (after.durable.decision().is_none()
                        && after.durable.locked().is_some_and(|locked| {
                            locked.round() == *round && locked.subject() == *subject
                        }))
                        || after.pending_prepare.values().any(|certificate| {
                            certificate.round() == *round && certificate.subject() == *subject
                        })
                        || (after.fallback_active
                            && after.candidate.as_ref().is_some_and(|proposal| {
                                proposal.round() == *round
                                    && proposal.manifest().subject() == *subject
                            })));
                (after.body_state(*round, *subject) == BodyState::Durable
                    && (newly_durable || retransmit_retry))
                    .then(|| EffectCapabilityKey {
                        kind: EFFECT_VALIDATE,
                        tag: Self::tag_projection(after.current_tag()),
                        height: round.height(),
                        view: round.view(),
                        subject: *subject,
                        ..EffectCapabilityKey::none()
                    })
            }
            Effect::Sign {
                message: requested, ..
            } => after.awaiting_signature.as_ref().and_then(|message| {
                (message == requested && after.signable_is_durably_authorized(message)).then(|| {
                    let mut key = EffectCapabilityKey {
                        kind: EFFECT_SIGN,
                        tag: Self::tag_projection(after.current_tag()),
                        ..EffectCapabilityKey::none()
                    };
                    Self::apply_signable(&mut key, message);
                    key
                })
            }),
            Effect::Broadcast(message) => match message {
                ConsensusMessageV2::Proposal(signed) => after
                    .durable
                    .proposal_intent(signed.proposal().round())
                    .filter(|_| Self::durable_proposal_is_active(&after.durable, signed.proposal()))
                    .map(|proposal| {
                        let mut key = EffectCapabilityKey {
                            kind: EFFECT_BROADCAST,
                            ..EffectCapabilityKey::none()
                        };
                        Self::apply_proposal(&mut key, proposal);
                        key
                    }),
                ConsensusMessageV2::Vote(signed) => {
                    let vote = signed.vote();
                    let durable_vote =
                        Self::durable_vote_is_active(&after.durable, vote).then_some(vote);
                    durable_vote.map(|vote| {
                        let mut key = EffectCapabilityKey {
                            kind: EFFECT_BROADCAST,
                            ..EffectCapabilityKey::none()
                        };
                        Self::apply_vote(&mut key, vote);
                        key
                    })
                }
                ConsensusMessageV2::QuorumCertificate(certificate) => {
                    let source = if certificate.phase() == Phase::Prepare
                        && certificate.validate(&after.context).is_ok()
                    {
                        Some(certificate)
                    } else {
                        after.durable.decision().filter(|decision| {
                            decision.reference() == certificate.reference()
                                && certificate.validate(&after.context).is_ok()
                        })
                    };
                    source.map(|certificate| {
                        let mut key = EffectCapabilityKey {
                            kind: EFFECT_BROADCAST,
                            ..EffectCapabilityKey::none()
                        };
                        Self::apply_primary_certificate(&mut key, certificate.reference());
                        key
                    })
                }
                ConsensusMessageV2::TimeoutVote(signed) => {
                    Self::durable_timeout_vote_is_active(&after.durable, &signed.vote()).then(
                        || {
                            let vote = signed.vote();
                            let mut key = EffectCapabilityKey {
                                kind: EFFECT_BROADCAST,
                                ..EffectCapabilityKey::none()
                            };
                            Self::apply_timeout_vote(&mut key, &vote);
                            key
                        },
                    )
                }
                ConsensusMessageV2::TimeoutCertificate(requested) => after
                    .durable
                    .last_timeout()
                    .filter(|certificate| *certificate == requested)
                    .map(|certificate| {
                        let mut key = EffectCapabilityKey {
                            kind: EFFECT_BROADCAST,
                            ..EffectCapabilityKey::none()
                        };
                        Self::apply_timeout_certificate(&mut key, certificate);
                        key
                    }),
                ConsensusMessageV2::BodyRequest(_) | ConsensusMessageV2::BodyChunk(_) => None,
            },
            Effect::Apply {
                subject,
                certificate: requested,
                ..
            } => after.durable.decision().and_then(|decision| {
                let body_round = after.decision_body_round(decision);
                let exact_local_completion = match event {
                    Event::LocalProposalReady { tag, manifest } => {
                        *tag == after.current_tag()
                            && body_round == Round::new(tag.height(), tag.view())
                            && decision.subject() == manifest.subject()
                            && after
                                .body_work
                                .get(&(body_round, *subject))
                                .is_some_and(|work| {
                                    work.state == BodyState::Validated
                                        && work.manifest == Some(*manifest)
                                })
                    }
                    _ => true,
                };
                (exact_local_completion
                    && decision == requested
                    && decision.phase() == Phase::Commit
                    && decision.subject() == *subject
                    && after.applied_subject != Some(*subject)
                    && after.body_state(body_round, *subject) == BodyState::Validated)
                    .then(|| {
                        let mut key = EffectCapabilityKey {
                            kind: EFFECT_APPLY,
                            tag: Self::tag_projection(after.current_tag()),
                            ..EffectCapabilityKey::none()
                        };
                        Self::apply_primary_certificate(&mut key, decision.reference());
                        key.subject = *subject;
                        key
                    })
            }),
            Effect::EnterView {
                certificate: requested,
                protected_lock,
                ..
            } => after.durable.last_timeout().and_then(|certificate| {
                (certificate == requested
                    && protected_lock.as_ref() == after.durable.locked()
                    && certificate.round().view().checked_add(1)
                        == Some(after.durable.current_view()))
                .then(|| {
                    let mut key = EffectCapabilityKey {
                        kind: EFFECT_ENTER_VIEW,
                        tag: Self::tag_projection(after.current_tag()),
                        ..EffectCapabilityKey::none()
                    };
                    Self::apply_timeout_certificate(&mut key, certificate);
                    key
                })
            }),
            Effect::ReportEquivocation { evidence } => {
                evidence.is_conflict_in(&after.context).then(|| {
                    let round = evidence.round();
                    let kind = evidence.kind();
                    let offender = evidence.offender();
                    let mut key = EffectCapabilityKey {
                        kind: EFFECT_REPORT,
                        actor: offender,
                        height: round.height(),
                        view: round.view(),
                        ..EffectCapabilityKey::none()
                    };
                    key.phase = match kind {
                        EquivocationKind::Vote => 1,
                        EquivocationKind::Timeout => 2,
                        EquivocationKind::Proposal => 3,
                    };
                    key
                })
            }
            Effect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => (certificate.phase() == Phase::Prepare
                && certificate.subject() == *subject
                && certificate.validate(&after.context).is_ok())
            .then(|| {
                let mut key = EffectCapabilityKey {
                    kind: EFFECT_REPORT,
                    ..EffectCapabilityKey::none()
                };
                Self::apply_primary_certificate(&mut key, certificate.reference());
                key.subject = *subject;
                key
            }),
        };
        granted.unwrap_or_else(EffectCapabilityKey::none)
    }
    fn reject_tag(&self, tag: EventTag) -> Option<IgnoreReason> {
        if tag.height() != self.context.height() {
            return Some(IgnoreReason::WrongHeight);
        }
        if tag.generation() != self.generation {
            return Some(IgnoreReason::StaleGeneration);
        }
        if tag.view() != self.durable.current_view() {
            return Some(IgnoreReason::WrongView);
        }
        None
    }
    fn on_proposal(&mut self, signed: &SignedProposal) -> Result<StepOutcome, ReducerError> {
        if self.durable.decision().is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        let proposal = signed.proposal();
        if proposal.context_id() != self.context.id()
            || proposal.round().height() != self.context.height()
        {
            return Err(ReducerError::InvalidProposal);
        }
        if proposal.round().view() != self.durable.current_view() {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        match self.validate_proposal(proposal) {
            Ok(()) => {}
            Err(ReducerError::UnsafeProposal) => {
                return Ok(StepOutcome::ignored(IgnoreReason::UnsafeProposal));
            }
            Err(error) => return Err(error),
        }
        let round = proposal.round();
        if self.durable.timeout_intent(round).is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::ViewClosed));
        }
        if let Some(existing) = &self.candidate {
            if existing.manifest() == proposal.manifest() {
                return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
            }
            let Some(first) = self.candidate_signed.clone() else {
                // A candidate learned only from a QC is not an authenticated
                // first leader proposal and therefore cannot support evidence.
                return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
            };
            return Ok(StepOutcome::applied(vec![Effect::ReportEquivocation {
                evidence: EquivocationEvidence::Proposal {
                    first,
                    second: signed.clone(),
                },
            }]));
        }
        self.candidate = Some(proposal.clone());
        self.candidate_signed = Some(signed.clone());
        let subject = proposal.manifest().subject();
        self.body_work
            .entry((round, subject))
            .and_modify(|work| {
                work.manifest.get_or_insert_with(|| *proposal.manifest());
            })
            .or_insert_with(|| BodyWork {
                manifest: Some(*proposal.manifest()),
                state: BodyState::Missing,
            });
        if self.body_state(round, subject) == BodyState::Missing {
            let key = CertificateRef::new(self.context.id(), round, Phase::Prepare, subject);
            let certificate = self.pending_prepare.get(&key).cloned();
            let eligible = if certificate.is_some() {
                self.local_certified_candidate_body_eligible()
            } else {
                self.local_candidate_body_eligible()
            };
            if !eligible {
                return Ok(StepOutcome::applied(Vec::new()));
            }
            let fetch = if let Some(certificate) = certificate {
                // A PrepareQC may race ahead of its proposal. Preserve its
                // certified sources when the proposal later contributes the
                // manifest so acquisition can be monotonically upgraded.
                self.ensure_body_fetch(&certificate)
            } else {
                Effect::FetchBody {
                    tag: self.current_tag(),
                    round,
                    subject,
                    manifest: Some(*proposal.manifest()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }
            };
            Ok(StepOutcome::applied(vec![fetch]))
        } else {
            Ok(StepOutcome::applied(Vec::new()))
        }
    }
    fn on_local_proposal_ready(
        &mut self,
        manifest: PayloadManifest,
    ) -> Result<StepOutcome, ReducerError> {
        let round = Round::new(self.context.height(), self.durable.current_view());
        if let Some(decision) = self.durable.decision().cloned() {
            let body_round = self.decision_body_round(&decision);
            let exact_decided_body = body_round == round
                && decision.subject() == manifest.subject()
                && self
                    .body_work
                    .get(&(body_round, manifest.subject()))
                    .is_none_or(|work| work.manifest.is_none_or(|known| known == manifest));
            if !exact_decided_body {
                // A local proposal completion for any other round, subject, or
                // manifest remains terminal after Decision. The adapter has
                // already validated the full manifest and execution
                // commitment before reducing it to this trusted event.
                return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
            }
            if self
                .body_work
                .get(&(round, manifest.subject()))
                .is_some_and(|work| work.state == BodyState::Invalid)
            {
                // Deterministic validation cannot truthfully be both invalid
                // and valid for the same manifest. Preserve the contradictory
                // state for diagnosis and fail closed instead of laundering it
                // into application authority.
                return Err(ReducerError::ProgressWitnessViolation(
                    ProgressWitnessViolation::DecidedBodyInvalid,
                ));
            }
            if self.applied_subject == Some(decision.subject()) {
                return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
            }
            // Decision can overtake local assembly after its body has become
            // durable and valid but before ProposalIntent persistence. Bind
            // that exact trusted completion directly to the durable decision
            // and recreate the sole Apply owner atomically; proposal-only work
            // is no longer relevant at this height.
            self.body_work.insert(
                (body_round, manifest.subject()),
                BodyWork {
                    manifest: Some(manifest),
                    state: BodyState::Validated,
                },
            );
            return Ok(StepOutcome::applied(vec![Effect::Apply {
                tag: self.current_tag(),
                subject: decision.subject(),
                certificate: decision,
            }]));
        }
        let Some(proposer) = self.local_validator else {
            return Err(ReducerError::ObserverCannotPropose);
        };
        if proposer != self.context.leader(round.view()) {
            return Err(ReducerError::NotCurrentLeader);
        }
        if self.durable.timeout_intent(round).is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::ViewClosed));
        }
        let justification = if round.view() == 0 {
            ProposalJustification::ParentCommit(self.context.parent_commit())
        } else {
            let certificate = self
                .durable
                .last_timeout()
                .filter(|certificate| {
                    certificate.round().view().checked_add(1) == Some(round.view())
                })
                .ok_or(ReducerError::MissingPersistedTimeoutJustification)?;
            if !self
                .durable
                .is_exact_local_proposal_timeout_justification(round.view(), certificate)
            {
                return Err(ReducerError::InvalidProposalJustification);
            }
            ProposalJustification::Timeout(certificate.clone())
        };
        let proposal = Proposal::new(self.context.id(), round, proposer, manifest, justification);
        match self.validate_proposal(&proposal) {
            Ok(()) => {}
            Err(ReducerError::UnsafeProposal) => {
                return Ok(StepOutcome::ignored(IgnoreReason::UnsafeProposal));
            }
            Err(error) => return Err(error),
        }
        if let Some(existing) = self.durable.proposal_intent(round) {
            if existing != &proposal {
                return Err(ReducerError::ConflictingLocalProposal);
            }
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        if let Some(existing) = &self.candidate {
            if existing != &proposal {
                return Err(ReducerError::ConflictingLocalProposal);
            }
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        // The event itself is the adapter's trusted acknowledgement that the
        // exact body is already durable and deterministically valid. Recording
        // the proposal intent makes leader non-equivocation survive a crash.
        self.body_work.insert(
            (round, manifest.subject()),
            BodyWork {
                manifest: Some(manifest),
                state: BodyState::Validated,
            },
        );
        let effect = self.start_persistence(
            WalRecord::ProposalIntent(proposal.clone()),
            Continuation::Sign(SignableMessage::Proposal(proposal)),
        )?;
        Ok(StepOutcome::applied(vec![effect]))
    }
    fn process_signed_local_proposal(
        &mut self,
        proposal: &Proposal,
    ) -> Result<StepOutcome, ReducerError> {
        self.validate_proposal(proposal)?;
        let round = proposal.round();
        if self.durable.proposal_intent(round) != Some(proposal) {
            return Err(ReducerError::ProposalIntentMissing);
        }
        if self.durable.timeout_intent(round).is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::ViewClosed));
        }
        if let Some(existing) = &self.candidate {
            if existing != proposal {
                return Err(ReducerError::ConflictingLocalProposal);
            }
        } else {
            self.candidate = Some(proposal.clone());
        }
        let subject = proposal.manifest().subject();
        self.body_work.insert(
            (round, subject),
            BodyWork {
                manifest: Some(*proposal.manifest()),
                state: BodyState::Validated,
            },
        );
        self.persist_prepare_intent(round, subject)
    }
    fn validate_proposal(&self, proposal: &Proposal) -> Result<(), ReducerError> {
        if proposal.context_id() != self.context.id()
            || proposal.round().height() != self.context.height()
            || proposal.round().view() != self.durable.current_view()
            || proposal.proposer() != self.context.leader(proposal.round().view())
        {
            return Err(ReducerError::InvalidProposal);
        }
        match proposal.justification() {
            ProposalJustification::ParentCommit(parent) => {
                // The same parent subject may acquire valid CommitQCs in more
                // than one view. The successor context deliberately ignores
                // that view and the signer subset, so proposal admission uses
                // the same semantic finality key. Otherwise nodes with equal
                // context IDs could reject one another's view-zero proposal.
                let same_finalized_parent = match (*parent, self.context.parent_commit()) {
                    (None, None) => true,
                    (Some(carried), Some(frozen)) => {
                        carried.proposal_round() == carried.round()
                            && carried.round().height().checked_add(1)
                                == Some(self.context.height())
                            && carried.same_commit_decision(frozen)
                    }
                    (None, Some(_)) | (Some(_), None) => false,
                };
                if proposal.round().view() != 0 || !same_finalized_parent {
                    return Err(ReducerError::InvalidProposalJustification);
                }
            }
            ProposalJustification::Timeout(certificate) => {
                certificate.validate(&self.context)?;
                if proposal.round().view() == 0
                    || certificate.round().view().checked_add(1) != Some(proposal.round().view())
                {
                    return Err(ReducerError::InvalidProposalJustification);
                }
                if certificate
                    .highest_prepare()
                    .is_some_and(|highest| highest.subject() != proposal.manifest().subject())
                {
                    return Err(ReducerError::UnsafeProposal);
                }
            }
        }
        if !self.safe_to_prepare(proposal) {
            return Err(ReducerError::UnsafeProposal);
        }
        Ok(())
    }
    fn safe_to_prepare(&self, proposal: &Proposal) -> bool {
        Self::proposal_is_safe_for_durable_lock(&self.durable, proposal)
    }
    fn on_body_available(&mut self, round: Round, subject: Subject) -> StepOutcome {
        let Some(work) = self.body_work.get_mut(&(round, subject)) else {
            return StepOutcome::ignored(IgnoreReason::NoMatchingWork);
        };
        if work.state != BodyState::Missing {
            return StepOutcome::ignored(IgnoreReason::Duplicate);
        }
        work.state = BodyState::Available;
        StepOutcome::applied(vec![Effect::StoreBody {
            tag: self.current_tag(),
            round,
            subject,
        }])
    }
    fn on_body_stored(&mut self, round: Round, subject: Subject) -> StepOutcome {
        let Some(work) = self.body_work.get_mut(&(round, subject)) else {
            return StepOutcome::ignored(IgnoreReason::NoMatchingWork);
        };
        if work.state != BodyState::Available {
            return StepOutcome::ignored(IgnoreReason::Duplicate);
        }
        work.state = BodyState::Durable;
        StepOutcome::applied(vec![Effect::ValidateBody {
            tag: self.current_tag(),
            round,
            subject,
        }])
    }
    fn on_validation(
        &mut self,
        round: Round,
        subject: Subject,
        valid: bool,
    ) -> Result<StepOutcome, ReducerError> {
        let Some(work) = self.body_work.get_mut(&(round, subject)) else {
            return Ok(StepOutcome::ignored(IgnoreReason::NoMatchingWork));
        };
        if work.state != BodyState::Durable {
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        work.state = if valid {
            BodyState::Validated
        } else {
            BodyState::Invalid
        };
        if !valid {
            let key = CertificateRef::new(self.context.id(), round, Phase::Prepare, subject);
            let effects = self
                .pending_prepare
                .get(&key)
                .map_or_else(Vec::new, |certificate| {
                    vec![Effect::ReportInvalidCertifiedBody {
                        subject,
                        certificate: certificate.clone(),
                    }]
                });
            return Ok(StepOutcome::applied(effects));
        }
        if let Some(decision) = self.durable.decision().cloned() {
            if self.decision_body_round(&decision) == round && decision.subject() == subject {
                return Ok(StepOutcome::applied(vec![Effect::Apply {
                    tag: self.current_tag(),
                    subject,
                    certificate: decision,
                }]));
            }
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        if round.view() != self.durable.current_view() {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        if self.local_validator.is_none() {
            return Ok(StepOutcome::ignored(IgnoreReason::Observer));
        }
        if self.durable.timeout_intent(round).is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::ViewClosed));
        }
        let key = CertificateRef::new(self.context.id(), round, Phase::Prepare, subject);
        if let Some(prepare) = self.pending_prepare.get(&key).cloned() {
            return self.persist_commit_intent(prepare);
        }
        if self.candidate.as_ref().is_some_and(|proposal| {
            proposal.round() == round && proposal.manifest().subject() == subject
        }) {
            return self.persist_prepare_intent(round, subject);
        }
        Ok(StepOutcome::applied(Vec::new()))
    }
    fn persist_prepare_intent(
        &mut self,
        round: Round,
        subject: Subject,
    ) -> Result<StepOutcome, ReducerError> {
        if !self.local_candidate_body_eligible() {
            return Ok(StepOutcome::applied(Vec::new()));
        }
        if self.durable.timeout_intent(round).is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::ViewClosed));
        }
        let signer = self
            .local_validator
            .ok_or(ReducerError::ObserverCannotVote)?;
        let vote = Vote::new(self.context.id(), round, Phase::Prepare, subject, signer);
        if let Some(existing) = self.durable.prepare_intent(round) {
            if existing != vote {
                return Err(ReducerError::ConflictingLocalVote);
            }
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        let effect = self.start_persistence(
            WalRecord::PrepareIntent(vote),
            Continuation::Sign(SignableMessage::Vote(vote)),
        )?;
        Ok(StepOutcome::applied(vec![effect]))
    }
    fn persist_commit_intent(
        &mut self,
        prepare: QuorumCertificate,
    ) -> Result<StepOutcome, ReducerError> {
        if !self.local_candidate_body_eligible() {
            return Ok(StepOutcome::applied(Vec::new()));
        }
        let proposal_round = prepare.proposal_round();
        let round = Round::new(self.context.height(), self.durable.current_view());
        if proposal_round != round {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        if self.durable.timeout_intent(round).is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::ViewClosed));
        }
        let signer = self
            .local_validator
            .ok_or(ReducerError::ObserverCannotVote)?;
        let vote = Vote::new(
            self.context.id(),
            round,
            Phase::Commit,
            prepare.subject(),
            signer,
        );
        if let Some(existing) = self.durable.commit_intent(round) {
            if existing != vote {
                return Err(ReducerError::ConflictingLocalVote);
            }
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        let effect = self.start_persistence(
            WalRecord::LockAndCommit { prepare, vote },
            Continuation::Sign(SignableMessage::Vote(vote)),
        )?;
        Ok(StepOutcome::applied(vec![effect]))
    }
    fn on_vote(&mut self, signed: SignedVote) -> Result<StepOutcome, ReducerError> {
        if self.durable.decision().is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        let vote = signed.vote();
        self.validate_vote(vote)?;
        let current_view = self.durable.current_view();
        // A timeout clears volatile vote pools, but an already-durable
        // same-round Commit intent remains retransmittable. Admit Prepare
        // votes only in the current view and Commit votes only for the exact
        // proposal round of the durably installed lock. A later view must
        // first re-propose the immutable body and form a new PrepareQC; it
        // cannot relabel the old round as a current Commit round.
        let admissible = match vote.phase() {
            Phase::Prepare => vote.round().view() == current_view,
            Phase::Commit => self.durable.locked().is_some_and(|locked| {
                if locked.round() != vote.proposal_round() || locked.subject() != vote.subject() {
                    return false;
                }
                let current_round = Round::new(self.context.height(), current_view);
                vote.round() == vote.proposal_round()
                    && self
                        .durable
                        .commit_intent_for_lock(locked)
                        .map_or(vote.round() == current_round, |intent| {
                            vote.same_statement(intent)
                        })
            }),
        };
        if !admissible {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        let existing_for_signer = self
            .votes
            .iter()
            .filter(|((round, phase, _), _)| *round == vote.round() && *phase == vote.phase())
            .find_map(|(_, pool)| pool.get(&vote.signer()));
        if let Some(existing) = existing_for_signer {
            if existing.vote() == vote {
                return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
            }
            return Ok(StepOutcome::applied(vec![Effect::ReportEquivocation {
                evidence: EquivocationEvidence::Vote {
                    first: existing.clone(),
                    second: signed,
                },
            }]));
        }
        let pool = self
            .votes
            .entry((vote.round(), vote.phase(), vote.proposal_round()))
            .or_default();
        if let Some(existing) = pool.get(&vote.signer()) {
            if existing.vote() == vote {
                return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
            }
            return Ok(StepOutcome::applied(vec![Effect::ReportEquivocation {
                evidence: EquivocationEvidence::Vote {
                    first: existing.clone(),
                    second: signed,
                },
            }]));
        }
        pool.insert(vote.signer(), signed);
        let Some(certificate) = self.try_form_certificate(
            vote.round(),
            vote.proposal_round(),
            vote.phase(),
            vote.subject(),
        )?
        else {
            return Ok(StepOutcome::applied(Vec::new()));
        };
        self.on_certificate(certificate, true)
    }
    fn validate_vote(&self, vote: Vote) -> Result<(), ReducerError> {
        if vote.context_id() != self.context.id()
            || vote.round().height() != self.context.height()
            || vote.proposal_round().height() != self.context.height()
            || vote.proposal_round() != vote.round()
            || self.context.validator(&vote.signer()).is_none()
        {
            return Err(ReducerError::InvalidVote);
        }
        Ok(())
    }
    fn try_form_certificate(
        &mut self,
        round: Round,
        proposal_round: Round,
        phase: Phase,
        subject: Subject,
    ) -> Result<Option<QuorumCertificate>, ReducerError> {
        let reference = CertificateRef::new_with_proposal_round(
            self.context.id(),
            round,
            proposal_round,
            phase,
            subject,
        );
        if self.formed_certificates.contains(&reference) {
            return Ok(None);
        }
        let Some(pool) = self.votes.get(&(round, phase, proposal_round)) else {
            return Ok(None);
        };
        let matching: Vec<_> = pool
            .values()
            .filter(|signed| signed.vote().subject() == subject)
            .collect();
        let (quorum, ordered) = Quorum::from_iter(
            &self.context,
            matching.iter().map(|signed| signed.vote().signer()),
        )?;
        if !quorum.satisfies(&self.context) {
            return Ok(None);
        }
        let signatures = ordered
            .into_iter()
            .take(self.context.minimum_signer_count())
            .map(|signer| {
                let signed = pool
                    .get(&signer)
                    .expect("ordered signer originated in the vote pool");
                SignatureShare::new(signer, signed.signature().clone())
            })
            .collect();
        self.formed_certificates.insert(reference);
        Ok(Some(QuorumCertificate::new(reference, signatures)))
    }
    fn ensure_body_fetch(&mut self, certificate: &QuorumCertificate) -> Effect {
        let round = self
            .durable
            .decision()
            .filter(|decision| *decision == certificate)
            .map_or_else(
                || certificate.round(),
                |decision| self.decision_body_round(decision),
            );
        let subject = certificate.subject();
        self.body_work.entry((round, subject)).or_insert(BodyWork {
            manifest: None,
            state: BodyState::Missing,
        });
        Effect::FetchBody {
            tag: self.current_tag(),
            round,
            subject,
            manifest: self
                .body_work
                .get(&(round, subject))
                .and_then(|work| work.manifest),
            certified_sources: self.frozen_archive_sources(),
            certificate: Some(certificate.clone()),
        }
    }
    fn on_commit_certificate(
        &mut self,
        certificate: QuorumCertificate,
        formed_locally: bool,
    ) -> Result<StepOutcome, ReducerError> {
        if let Some(existing) = self.durable.decision() {
            if !existing
                .reference()
                .same_commit_decision(certificate.reference())
            {
                return Err(ReducerError::ConflictingDecision);
            }
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        // `on_certificate` has already validated the full CommitQC. A local
        // Prepare lock constrains this validator's future votes; it cannot
        // veto the first quorum-authenticated decision. In particular, a
        // higher-view quorum may legitimately decide another subject before
        // this validator learns the superseding PrepareQC. Only an already
        // durable, semantically different Decision conflicts.
        self.park_awaiting_signature_for_certified_progress();
        let effect = self.start_persistence(
            WalRecord::Decision(certificate.clone()),
            Continuation::Decide {
                certificate,
                broadcast: formed_locally,
            },
        )?;
        Ok(StepOutcome::applied(vec![effect]))
    }
    fn on_retransmit_elapsed(&mut self) -> Result<StepOutcome, ReducerError> {
        let current_view = self.durable.current_view();
        let current_round = Round::new(self.context.height(), current_view);
        let current_view_open = self.durable.timeout_intent(current_round).is_none();
        // Closed-view PrepareQCs remain retransmittable control evidence, but
        // must not reacquire body ownership ahead of the durable lock forever.
        if current_view_open
            && (self
                .candidate
                .as_ref()
                .is_some_and(|proposal| proposal.round() == current_round)
                || self
                    .pending_prepare
                    .values()
                    .any(|certificate| certificate.round() == current_round))
        {
            self.fallback_active = true;
        }
        let mut effects: Vec<_> = self
            .outbound_control
            .values()
            .cloned()
            .map(Effect::Broadcast)
            .collect();
        if let Some(decision) = self.durable.decision().cloned() {
            let body_round = self.decision_body_round(&decision);
            match self.body_state(body_round, decision.subject()) {
                BodyState::Missing => effects.push(self.ensure_body_fetch(&decision)),
                BodyState::Available => effects.push(Effect::StoreBody {
                    tag: self.current_tag(),
                    round: body_round,
                    subject: decision.subject(),
                }),
                BodyState::Durable => effects.push(Effect::ValidateBody {
                    tag: self.current_tag(),
                    round: body_round,
                    subject: decision.subject(),
                }),
                BodyState::Validated if self.applied_subject != Some(decision.subject()) => {
                    effects.push(Effect::Apply {
                        tag: self.current_tag(),
                        subject: decision.subject(),
                        certificate: decision,
                    });
                }
                BodyState::Validated | BodyState::Invalid => {}
            }
            return Ok(StepOutcome::applied(effects));
        }
        if let Some(certificate) = self
            .pending_prepare
            .values()
            .find(|certificate| current_view_open && certificate.round() == current_round)
            .cloned()
        {
            let round = certificate.round();
            let subject = certificate.subject();
            match self.body_state(round, subject) {
                BodyState::Missing => effects.push(self.ensure_body_fetch(&certificate)),
                BodyState::Available => effects.push(Effect::StoreBody {
                    tag: self.current_tag(),
                    round,
                    subject,
                }),
                BodyState::Durable => effects.push(Effect::ValidateBody {
                    tag: self.current_tag(),
                    round,
                    subject,
                }),
                BodyState::Validated
                    if self.local_validator.is_some() && self.local_candidate_body_eligible() =>
                {
                    let mut outcome = self.persist_commit_intent(certificate)?;
                    effects.append(&mut outcome.effects);
                }
                BodyState::Validated | BodyState::Invalid => {}
            }
            return Ok(StepOutcome::applied(effects));
        }
        if let Some(locked) = self.durable.locked().cloned() {
            let effect = match self.body_state(locked.round(), locked.subject()) {
                BodyState::Missing => Some(self.ensure_body_fetch(&locked)),
                BodyState::Available => Some(Effect::StoreBody {
                    tag: self.current_tag(),
                    round: locked.round(),
                    subject: locked.subject(),
                }),
                BodyState::Durable => Some(Effect::ValidateBody {
                    tag: self.current_tag(),
                    round: locked.round(),
                    subject: locked.subject(),
                }),
                BodyState::Validated | BodyState::Invalid => None,
            };
            if let Some(effect) = effect {
                effects.push(effect);
                return Ok(StepOutcome::applied(effects));
            }
        }
        if current_view_open && let Some(proposal) = self.candidate.clone() {
            let round = proposal.round();
            let subject = proposal.manifest().subject();
            match self.body_state(round, subject) {
                BodyState::Missing if self.local_candidate_body_eligible() => {
                    effects.push(Effect::FetchBody {
                        tag: self.current_tag(),
                        round,
                        subject,
                        manifest: Some(*proposal.manifest()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    });
                }
                BodyState::Available if self.local_candidate_body_eligible() => {
                    effects.push(Effect::StoreBody {
                        tag: self.current_tag(),
                        round,
                        subject,
                    });
                }
                BodyState::Durable if self.local_candidate_body_eligible() => {
                    effects.push(Effect::ValidateBody {
                        tag: self.current_tag(),
                        round,
                        subject,
                    });
                }
                BodyState::Validated if self.local_candidate_body_eligible() => {
                    let key =
                        CertificateRef::new(self.context.id(), round, Phase::Prepare, subject);
                    let mut outcome = if let Some(prepare) = self.pending_prepare.get(&key).cloned()
                    {
                        self.persist_commit_intent(prepare)?
                    } else {
                        self.persist_prepare_intent(round, subject)?
                    };
                    effects.append(&mut outcome.effects);
                }
                BodyState::Missing
                | BodyState::Available
                | BodyState::Durable
                | BodyState::Validated
                | BodyState::Invalid => {}
            }
        }
        Ok(StepOutcome::applied(effects))
    }
    fn remember_control(&mut self, message: ConsensusMessageV2) {
        let (class, round) = match &message {
            ConsensusMessageV2::Proposal(proposal) => {
                (OutboundControlClass::Proposal, proposal.proposal().round())
            }
            ConsensusMessageV2::Vote(vote) => match vote.vote().phase() {
                Phase::Prepare => (OutboundControlClass::PrepareVote, vote.vote().round()),
                Phase::Commit => (OutboundControlClass::CommitVote, vote.vote().round()),
            },
            ConsensusMessageV2::QuorumCertificate(certificate) => match certificate.phase() {
                Phase::Prepare => (OutboundControlClass::PrepareQc, certificate.round()),
                Phase::Commit => (OutboundControlClass::CommitQc, certificate.round()),
            },
            ConsensusMessageV2::TimeoutVote(vote) => {
                (OutboundControlClass::TimeoutVote, vote.vote().round())
            }
            ConsensusMessageV2::TimeoutCertificate(certificate) => (
                OutboundControlClass::TimeoutCertificate,
                certificate.round(),
            ),
            ConsensusMessageV2::BodyRequest(_) | ConsensusMessageV2::BodyChunk(_) => return,
        };
        let replace = self.outbound_control.get(&class).is_none_or(|existing| {
            let existing_round = match existing {
                ConsensusMessageV2::Proposal(proposal) => proposal.proposal().round(),
                ConsensusMessageV2::Vote(vote) => vote.vote().round(),
                ConsensusMessageV2::QuorumCertificate(certificate) => certificate.round(),
                ConsensusMessageV2::TimeoutVote(vote) => vote.vote().round(),
                ConsensusMessageV2::TimeoutCertificate(certificate) => certificate.round(),
                ConsensusMessageV2::BodyRequest(_) | ConsensusMessageV2::BodyChunk(_) => {
                    unreachable!("transport messages are never retained as control traffic")
                }
            };
            round.view() > existing_round.view()
                || &message == existing
                || (round == existing_round
                    && matches!(
                        &message,
                        ConsensusMessageV2::TimeoutCertificate(incoming)
                            if self.durable.last_timeout() == Some(incoming)
                    ))
        });
        if replace {
            self.outbound_control.insert(class, message);
        }
    }
    fn on_timeout_elapsed(&mut self) -> Result<StepOutcome, ReducerError> {
        if self.durable.decision().is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        let Some(signer) = self.local_validator else {
            return Ok(StepOutcome::ignored(IgnoreReason::Observer));
        };
        let round = Round::new(self.context.height(), self.durable.current_view());
        if self.durable.timeout_intent(round).is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        let timeout = TimeoutVote::new(
            self.context.id(),
            round,
            signer,
            self.durable.highest_prepare().cloned(),
        );
        let effect = self.start_persistence(
            WalRecord::TimeoutIntent(timeout.clone()),
            Continuation::Sign(SignableMessage::TimeoutVote(timeout)),
        )?;
        Ok(StepOutcome::applied(vec![effect]))
    }
    fn on_timeout_vote(&mut self, signed: SignedTimeoutVote) -> Result<StepOutcome, ReducerError> {
        if self.durable.decision().is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        let vote = signed.vote();
        if vote.context_id() != self.context.id()
            || vote.round().height() != self.context.height()
            || self.context.validator(&vote.signer()).is_none()
        {
            return Err(ReducerError::InvalidTimeoutVote);
        }
        if !timeout_vote_view_is_admissible(self.durable.current_view(), vote.round().view()) {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        if let Some(certificate) = vote.highest_prepare()
            && (certificate.phase() != Phase::Prepare
                || certificate.reference().context_id() != self.context.id()
                || certificate.round().height() != self.context.height()
                || certificate.round().view() > vote.round().view()
                || certificate.validate(&self.context).is_err())
        {
            return Err(ReducerError::InvalidTimeoutVote);
        }
        let pool = self.timeout_votes.entry(vote.round()).or_default();
        if let Some(existing) = pool.get(&vote.signer()) {
            if existing.vote().highest_prepare_ref() == vote.highest_prepare_ref() {
                return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
            }
            return Ok(StepOutcome::applied(vec![Effect::ReportEquivocation {
                evidence: EquivocationEvidence::Timeout {
                    first: existing.clone(),
                    second: signed,
                },
            }]));
        }
        pool.insert(vote.signer(), signed);
        let Some(certificate) = self.try_form_timeout_certificate(vote.round())? else {
            return Ok(StepOutcome::applied(Vec::new()));
        };
        self.on_timeout_certificate(certificate, true)
    }
    fn try_form_timeout_certificate(
        &mut self,
        round: Round,
    ) -> Result<Option<TimeoutCertificate>, ReducerError> {
        if self.formed_timeouts.contains(&round) {
            return Ok(None);
        }
        let Some(pool) = self.timeout_votes.get(&round) else {
            return Ok(None);
        };
        let (quorum, ordered) = Quorum::from_iter(
            &self.context,
            pool.values().map(|signed| signed.vote().signer()),
        )?;
        if !quorum.satisfies(&self.context) {
            return Ok(None);
        }
        let mut grouped: BTreeMap<
            Option<CertificateRef>,
            (Option<QuorumCertificate>, Vec<SignatureShare>),
        > = BTreeMap::new();
        for signer in ordered
            .into_iter()
            .take(self.context.minimum_signer_count())
        {
            let signed = pool
                .get(&signer)
                .expect("ordered timeout signer originated in the vote pool");
            let vote = signed.vote();
            grouped
                .entry(vote.highest_prepare_ref())
                .or_insert_with(|| (vote.highest_prepare().cloned(), Vec::new()))
                .1
                .push(SignatureShare::new(
                    vote.signer(),
                    signed.signature().clone(),
                ));
        }
        let groups = grouped
            .into_iter()
            .map(|(_, (certificate, signatures))| {
                TimeoutSignatureGroup::new(certificate, signatures)
            })
            .collect();
        let certificate = TimeoutCertificate::new(self.context.id(), round, groups);
        certificate.validate(&self.context)?;
        self.formed_timeouts.insert(round);
        Ok(Some(certificate))
    }
    fn on_timeout_certificate(
        &mut self,
        certificate: TimeoutCertificate,
        formed_locally: bool,
    ) -> Result<StepOutcome, ReducerError> {
        if self.durable.decision().is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        certificate.validate(&self.context)?;
        if certificate.round().view() < self.durable.current_view()
            && !self
                .durable
                .is_strict_same_round_timeout_upgrade(&certificate)
        {
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        self.park_awaiting_signature_for_certified_progress();
        let effect = self.start_persistence(
            WalRecord::InstallTimeout(certificate.clone()),
            Continuation::InstallTimeout {
                certificate,
                broadcast: formed_locally,
            },
        )?;
        Ok(StepOutcome::applied(vec![effect]))
    }
    fn start_persistence(
        &mut self,
        record: WalRecord,
        continuation: Continuation,
    ) -> Result<Effect, ReducerError> {
        if self.pending_persistence.is_some() {
            return Err(ReducerError::PersistenceAlreadyPending);
        }
        let id = self.durable.next_id()?;
        let entry = WalEntry::new(id, record);
        self.pending_persistence = Some(PendingPersistence {
            entry: entry.clone(),
            continuation,
        });
        Ok(Effect::Persist {
            tag: self.current_tag(),
            entry,
        })
    }
    fn on_persisted(&mut self, id: PersistenceId) -> Result<StepOutcome, ReducerError> {
        let Some(pending) = self.pending_persistence.as_ref() else {
            return Ok(StepOutcome::ignored(IgnoreReason::NoMatchingWork));
        };
        if pending.entry.id() != id {
            return Err(ReducerError::PersistenceAcknowledgementMismatch {
                expected: pending.entry.id(),
                actual: id,
            });
        }
        let pending = pending.clone();
        // Preflight the generation transition before applying the WAL entry
        // or releasing its pending owner. Normal view advances reset to zero;
        // only a same-view lock upgrade can reach the checked overflow path.
        let next_generation = match pending.entry.record() {
            WalRecord::InstallTimeout(certificate) => self
                .generation_after_timeout_install(certificate)
                .ok_or(ReducerError::GenerationOverflow)?,
            _ => self.generation,
        };
        let mut durable = self.durable.clone();
        durable.apply(&self.context, self.local_validator, &pending.entry)?;
        self.durable = durable;
        if matches!(pending.entry.record(), WalRecord::ObservePrepare(_)) {
            // Old-view observations can advance durable high QC but own no
            // current body pipeline; keep only live and durable references.
            self.prune_observed_prepare_caches();
        }
        if matches!(pending.entry.record(), WalRecord::InstallTimeout(_)) {
            // Fallback is scoped to the exact proposal generation, not merely
            // the numeric view. A same-view TC upgrade clears the candidate and
            // body work just like a normal view change, so Set B must wait for
            // the replacement proposal's own retransmission boundary.
            self.fallback_active = false;
        }
        self.pending_persistence = None;
        if matches!(pending.entry.record(), WalRecord::LockAndCommit { .. }) {
            let current_round = Round::new(self.context.height(), self.durable.current_view());
            // Once a new same-round LockAndCommit frame is durable, its round
            // supersedes every older Commit pool, including pools for an
            // earlier proposal of the same immutable body. Keeping both would
            // allow an old Commit pool, the new Commit pool, and the current
            // Prepare pool to coexist and violate the verified two-pool bound.
            // Pruning before this WAL boundary would orphan the old lock if the
            // append failed, so retire it only after acknowledgement.
            self.votes
                .retain(|(round, _, _), _| *round == current_round);
        }
        let mut effects = match pending.continuation {
            Continuation::None => Vec::new(),
            Continuation::Sign(message) => {
                self.signature_queue.push_back(message);
                self.drive_signature()
            }
            Continuation::InstallTimeout {
                certificate,
                broadcast,
            } => {
                let message = ConsensusMessageV2::TimeoutCertificate(certificate.clone());
                self.remember_control(message.clone());
                let mut install_effects = Vec::new();
                self.generation = next_generation;
                self.candidate = None;
                self.candidate_signed = None;
                self.body_work.clear();
                self.pending_prepare.clear();
                self.votes.clear();
                self.formed_certificates.clear();
                // ProposalIntent remains append-only non-equivocation
                // evidence, but an alternate same-round TC changes the exact
                // latest justification. Retire volatile signing ownership
                // whose source certificate no longer matches durable state.
                // A still-authorized task must nevertheless be reissued: the
                // new generation rejects its already-issued completion tag,
                // and the effect executor cancels that old tagged task when
                // it consumes EnterView.
                let interrupted_signature = self.awaiting_signature.take();
                self.signature_queue.retain(|message| {
                    Self::signable_is_durably_authorized_for(&self.durable, message)
                });
                if let Some(message) = interrupted_signature.filter(|message| {
                    Self::signable_is_durably_authorized_for(&self.durable, message)
                }) && !self.signature_queue.iter().any(|queued| queued == &message)
                {
                    self.signature_queue.push_front(message);
                }
                // Preserve already authenticated shares for the installed
                // view and its one-round catch-up window. Before this bound
                // existed every normal TC install cleared shares which had
                // arrived slightly early, recreating the same stagger at the
                // successor view. Stale and farther-future pools are retired
                // at this durable boundary; a same-round lock upgrade keeps
                // the identical bounded set.
                let current_view = self.durable.current_view();
                self.timeout_votes.retain(|round, _| {
                    round.height() == self.context.height()
                        && timeout_vote_view_is_admissible(current_view, round.view())
                });
                self.formed_timeouts.retain(|round| {
                    round.height() == self.context.height()
                        && timeout_vote_view_is_admissible(current_view, round.view())
                });
                self.known_prepare.clear();
                if let Some(highest) = self.durable.highest_prepare() {
                    self.known_prepare
                        .insert(highest.reference(), highest.clone());
                }
                if let Some(locked) = self.durable.locked() {
                    self.known_prepare
                        .insert(locked.reference(), locked.clone());
                }
                let context = &self.context;
                let durable = &self.durable;
                self.outbound_control.retain(|class, message| {
                    matches!(
                        class,
                        OutboundControlClass::CommitVote
                            | OutboundControlClass::PrepareQc
                            | OutboundControlClass::CommitQc
                            | OutboundControlClass::TimeoutVote
                            | OutboundControlClass::TimeoutCertificate
                    ) && Self::outbound_control_is_active(context, durable, message)
                });
                // A TC can carry a strictly newer PrepareQC than this node
                // previously retained, while a TC quorum can also omit this
                // node's exact durable high QC.  In both cases the installed
                // view must keep one immutable, exact high-QC control owner:
                // retain it when already present and reseed it from the WAL
                // projection when the TC introduced it.  Periodic
                // retransmission then disseminates the certificate
                // independently of the one-shot TC broadcast.  Matching the
                // full durable certificate above prevents an older or
                // same-projection/different-evidence QC from surviving the
                // install boundary.
                if let Some(highest) = self.durable.highest_prepare().cloned() {
                    self.remember_control(ConsensusMessageV2::QuorumCertificate(highest));
                }
                // A strict alternate TC for the preceding round changes only
                // the lock and generation. Current-round timeout receipts and
                // the local Timeout intent remain valid, so retain both the
                // partial pool above and its exact already-signed control
                // message for recipients which missed the first broadcast.
                // A normal TC advances the view and therefore clears the pool
                // and fails the active-control predicate, retiring the vote.
                // Broadcast excludes the local peer, so retransmission alone
                // cannot reconstruct this node's cleared vote pool. Queue the
                // exact still-active same-round Commit intent once for the new
                // pool generation. A TC-promoted lock without one retains its
                // exact body for unchanged re-proposal by the current leader.
                self.queue_active_locked_commit_signature();
                install_effects.push(Effect::EnterView {
                    tag: self.current_tag(),
                    certificate,
                    protected_lock: self.durable.locked().cloned(),
                });
                if let Some(locked) = self.durable.locked().cloned() {
                    install_effects.push(self.ensure_body_fetch(&locked));
                }
                // The executor installs the new reducer incarnation before
                // dispatching any other effect from this macro-step. Keep a
                // protected body's Fetch immediately behind EnterView, then
                // publish a locally formed TC as the first ordinary control
                // effect. Remote TC installation has no one-shot broadcast.
                if broadcast {
                    install_effects.push(Effect::Broadcast(message));
                }
                install_effects
            }
            Continuation::Decide {
                certificate,
                broadcast,
            } => {
                let decision_key = (
                    self.decision_body_round(&certificate),
                    certificate.subject(),
                );
                self.candidate = None;
                self.candidate_signed = None;
                self.body_work.retain(|key, _| *key == decision_key);
                self.pending_prepare.clear();
                self.known_prepare.clear();
                self.votes.clear();
                self.timeout_votes.clear();
                self.formed_certificates.clear();
                self.formed_timeouts.clear();
                self.awaiting_signature = None;
                self.signature_queue.clear();
                self.outbound_control.clear();
                let mut decision_effects = Vec::new();
                let message = ConsensusMessageV2::QuorumCertificate(certificate.clone());
                self.remember_control(message.clone());
                if broadcast {
                    decision_effects.push(Effect::Broadcast(message));
                }
                decision_effects.extend(self.decision_effects(certificate));
                decision_effects
            }
        };
        // Durable view closure or lock promotion can retire previously valid
        // Proposal, Prepare, and Commit controls without deleting their WAL
        // history. Keep retransmission authorization synchronized at every
        // acknowledgement boundary, including TimeoutIntent before a TC forms.
        self.prune_inactive_outbound_control();
        if self.pending_persistence.is_none() && self.awaiting_signature.is_none() {
            effects.extend(self.drive_signature());
        }
        Ok(StepOutcome::applied(effects))
    }
    fn on_persistence_failed(&mut self, id: PersistenceId) -> Result<StepOutcome, ReducerError> {
        let Some(pending) = &self.pending_persistence else {
            return Ok(StepOutcome::ignored(IgnoreReason::NoMatchingWork));
        };
        if pending.entry.id() != id {
            return Err(ReducerError::PersistenceAcknowledgementMismatch {
                expected: pending.entry.id(),
                actual: id,
            });
        }
        Err(ReducerError::PersistenceFailed(id))
    }
    fn drive_signature(&mut self) -> Vec<Effect> {
        if self.awaiting_signature.is_some() || self.pending_persistence.is_some() {
            return Vec::new();
        }
        while let Some(message) = self.signature_queue.pop_front() {
            // A TC may supersede queued Prepare work or promote the lock past
            // an older durable Commit intent. Such records remain immutable
            // WAL history, but no longer authorize a fresh signature.
            if self.signable_is_durably_authorized(&message) {
                self.awaiting_signature = Some(message.clone());
                return vec![Effect::Sign {
                    tag: self.current_tag(),
                    message,
                }];
            }
        }
        Vec::new()
    }
    fn on_signed(&mut self, signature: OpaqueSignature) -> Result<StepOutcome, ReducerError> {
        let Some(message) = self.awaiting_signature.take() else {
            return Ok(StepOutcome::ignored(IgnoreReason::NoMatchingWork));
        };
        let mut effects = Vec::new();
        match message {
            SignableMessage::Proposal(proposal) => {
                let signed = SignedProposal::new(proposal.clone(), signature);
                self.candidate_signed = Some(signed.clone());
                let message = ConsensusMessageV2::Proposal(signed);
                self.remember_control(message.clone());
                effects.push(Effect::Broadcast(message));
                let mut local = self
                    .process_signed_local_proposal(&proposal)?
                    .into_effects();
                effects.append(&mut local);
            }
            SignableMessage::Vote(vote) => {
                let signed = SignedVote::new(vote, signature);
                let message = ConsensusMessageV2::Vote(signed.clone());
                self.remember_control(message.clone());
                effects.push(Effect::Broadcast(message));
                let mut local = self.on_vote(signed)?.into_effects();
                effects.append(&mut local);
            }
            SignableMessage::TimeoutVote(vote) => {
                let signed = SignedTimeoutVote::new(vote, signature);
                let message = ConsensusMessageV2::TimeoutVote(signed.clone());
                self.remember_control(message.clone());
                let mut local = self.on_timeout_vote(signed)?.into_effects();
                let installs_timeout = local.iter().any(|effect| {
                    matches!(
                        effect,
                        Effect::Persist { entry, .. }
                            if matches!(entry.record(), WalRecord::InstallTimeout(_))
                    )
                });
                if installs_timeout {
                    // This signature completed the local timeout quorum. The
                    // resulting durable TC subsumes the individual old-view
                    // vote, so expose only its InstallTimeout fence. The
                    // persisted continuation can then keep EnterView first,
                    // the TC as its control broadcast, and any reconstructed
                    // signature request last in the executor macro-step.
                    effects.append(&mut local);
                } else {
                    effects.push(Effect::Broadcast(message));
                    effects.append(&mut local);
                }
            }
        }
        if self.pending_persistence.is_none() && self.awaiting_signature.is_none() {
            effects.extend(self.drive_signature());
        }
        Ok(StepOutcome::applied(effects))
    }
    fn on_application_completed(&mut self, subject: Subject) -> Result<StepOutcome, ReducerError> {
        let decision = self
            .durable
            .decision()
            .ok_or(ReducerError::InvalidApplicationCompletion)?;
        let body_round = self.decision_body_round(decision);
        if decision.subject() != subject
            || self.body_state(body_round, subject) != BodyState::Validated
        {
            return Err(ReducerError::InvalidApplicationCompletion);
        }
        if let Some(existing) = self.applied_subject {
            return if existing == subject {
                Ok(StepOutcome::ignored(IgnoreReason::Duplicate))
            } else {
                Err(ReducerError::InvalidApplicationCompletion)
            };
        }
        self.applied_subject = Some(subject);
        Ok(StepOutcome::applied(Vec::new()))
    }
    fn decision_effects(&mut self, certificate: QuorumCertificate) -> Vec<Effect> {
        let round = self.decision_body_round(&certificate);
        let subject = certificate.subject();
        match self.body_state(round, subject) {
            BodyState::Missing => vec![self.ensure_body_fetch(&certificate)],
            BodyState::Validated if self.applied_subject != Some(subject) => {
                vec![Effect::Apply {
                    tag: self.current_tag(),
                    subject,
                    certificate,
                }]
            }
            // Store and validation completions are already generation-tagged
            // continuations of the exact body pipeline. Starting a second
            // fetch here would race that retained work and can exhaust the
            // bounded executor without making PendingApply progress.
            BodyState::Available
            | BodyState::Durable
            | BodyState::Validated
            | BodyState::Invalid => Vec::new(),
        }
    }
}
include!("reducer/prepare_certificate_handling.rs");
/// Missing reducer-owned evidence that durable work can be reconstructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressWitnessViolation {
    /// The active durable Commit intent has no pending signature, local pool,
    /// decision, or recovery witness.
    LockedCommitOrphaned,
    /// A durable decision has no retained body/application pipeline state.
    DecisionApplicationOrphaned,
    /// Deterministic validation marked the body of a durable decision invalid.
    DecidedBodyInvalid,
}
/// Failure caused by malformed authenticated input or an impossible local state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReducerError {
    /// The executable transition failed the mechanically verified refinement
    /// gate and was discarded before becoming caller-visible.
    RefinementViolation,
    /// A durable progress source lost every reducer-owned reconstruction path.
    ProgressWitnessViolation(ProgressWitnessViolation),
    /// The configured local validator is absent from the frozen roster.
    LocalValidatorNotInRoster,
    /// An observer attempted to create a voting intent.
    ObserverCannotVote,
    /// An observer attempted to submit a local proposal.
    ObserverCannotPropose,
    /// The local validator is not the deterministic leader of the current view.
    NotCurrentLeader,
    /// A local proposal conflicts with the durable proposal for this round.
    ConflictingLocalProposal,
    /// A proposal signature completed without a matching durable intent.
    ProposalIntentMissing,
    /// Proposal context, height, view, or leader is invalid.
    InvalidProposal,
    /// Proposal does not carry the required parent reference or preceding TC.
    InvalidProposalJustification,
    /// A non-zero-view leader has no persisted TC for the preceding view.
    MissingPersistedTimeoutJustification,
    /// Proposal violates the lock or timeout-certificate safe-value rule.
    UnsafeProposal,
    /// Vote context, height, or signer is invalid.
    InvalidVote,
    /// Timeout vote context, height, signer, or high-QC reference is invalid.
    InvalidTimeoutVote,
    /// Local persistence was requested while another append was outstanding.
    PersistenceAlreadyPending,
    /// A WAL acknowledgement does not match the sole pending append.
    PersistenceAcknowledgementMismatch {
        /// Expected identifier.
        expected: PersistenceId,
        /// Acknowledged identifier.
        actual: PersistenceId,
    },
    /// The safety WAL failed and the reducer stopped making progress.
    PersistenceFailed(PersistenceId),
    /// The local asynchronous-completion generation is exhausted.
    GenerationOverflow,
    /// Durable state already records a different local vote for this round.
    ConflictingLocalVote,
    /// Equal-view valid `PrepareQC`s certify different subjects.
    ConflictingPrepareCertificates,
    /// Durable state already records a different decision.
    ConflictingDecision,
    /// An apply completion did not match the durable validated decision.
    InvalidApplicationCompletion,
    /// Height finalization was requested before successful local application.
    HeightNotApplied,
    /// Height finalization was requested while a safety effect was outstanding.
    HeightStillBusy,
    /// Kura's durable receipt did not match the exact reducer decision.
    DurableCommitReceiptMismatch,
    /// Certificate signer validation or dual-quorum validation failed.
    Quorum(QuorumError),
    /// WAL replay or application failed.
    Replay(ReplayError),
}
impl fmt::Display for ReducerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RefinementViolation => {
                formatter.write_str("reducer transition violated the verified refinement gate")
            }
            Self::ProgressWitnessViolation(violation) => write!(
                formatter,
                "reducer transition violated progress witness: {violation:?}"
            ),
            Self::LocalValidatorNotInRoster => {
                formatter.write_str("local validator is absent from the roster")
            }
            Self::ObserverCannotVote => formatter.write_str("observer cannot vote"),
            Self::ObserverCannotPropose => formatter.write_str("observer cannot propose"),
            Self::NotCurrentLeader => formatter.write_str("local validator is not current leader"),
            Self::ConflictingLocalProposal => formatter.write_str("conflicting local proposal"),
            Self::ProposalIntentMissing => {
                formatter.write_str("durable proposal intent is missing")
            }
            Self::InvalidProposal => formatter.write_str("invalid proposal"),
            Self::InvalidProposalJustification => {
                formatter.write_str("invalid proposal justification")
            }
            Self::MissingPersistedTimeoutJustification => {
                formatter.write_str("current view has no persisted timeout justification")
            }
            Self::UnsafeProposal => formatter.write_str("proposal violates the safe-value rule"),
            Self::InvalidVote => formatter.write_str("invalid vote"),
            Self::InvalidTimeoutVote => formatter.write_str("invalid timeout vote"),
            Self::PersistenceAlreadyPending => {
                formatter.write_str("persistence is already pending")
            }
            Self::PersistenceAcknowledgementMismatch { expected, actual } => write!(
                formatter,
                "persistence acknowledgement mismatch: expected {}, got {}",
                expected.get(),
                actual.get()
            ),
            Self::PersistenceFailed(id) => {
                write!(formatter, "persistence {} failed", id.get())
            }
            Self::GenerationOverflow => formatter.write_str("event generation overflow"),
            Self::ConflictingLocalVote => formatter.write_str("conflicting local vote"),
            Self::ConflictingPrepareCertificates => {
                formatter.write_str("conflicting PrepareQCs at one view")
            }
            Self::ConflictingDecision => formatter.write_str("conflicting decision"),
            Self::InvalidApplicationCompletion => {
                formatter.write_str("application completion does not match the decision")
            }
            Self::HeightNotApplied => formatter.write_str("decided height is not applied"),
            Self::HeightStillBusy => {
                formatter.write_str("height still has an outstanding safety effect")
            }
            Self::DurableCommitReceiptMismatch => {
                formatter.write_str("durable commit receipt does not match the decision")
            }
            Self::Quorum(error) => write!(formatter, "certificate validation failed: {error}"),
            Self::Replay(error) => write!(formatter, "durable state error: {error}"),
        }
    }
}
impl Error for ReducerError {}
impl From<QuorumError> for ReducerError {
    fn from(error: QuorumError) -> Self {
        Self::Quorum(error)
    }
}
impl From<ReplayError> for ReducerError {
    fn from(error: ReplayError) -> Self {
        Self::Replay(error)
    }
}
#[cfg(test)]
mod source_link_tests {
    use super::super::{ContextId, Digest, NetworkId, Validator, VotingMode, VotingPower};
    use super::*;
    fn reducer() -> Reducer {
        let validators = (1_u8..=4)
            .map(|byte| Validator::new(ValidatorId::repeat(byte), VotingPower::new(1)))
            .collect();
        let context = HeightContext::new(
            ContextId::repeat(0x91),
            NetworkId::repeat(0x92),
            2,
            Some(CertificateRef::new(
                ContextId::repeat(0x90),
                Round::new(1, 0),
                Phase::Commit,
                Subject::repeat(0x93),
            )),
            0,
            validators,
            VotingMode::Permissioned,
            Digest::repeat(0x94),
            Digest::repeat(0x97),
            Digest::repeat(0x95),
            Digest::repeat(0x96),
        )
        .expect("source-link fixture context");
        Reducer::new(context, Some(ValidatorId::repeat(1)), Generation::new(7))
            .expect("source-link fixture reducer")
    }
    fn decided_reducer(subject: Subject) -> (Reducer, QuorumCertificate) {
        let fresh = reducer();
        let context = fresh.context.clone();
        let decision = QuorumCertificate::new(
            CertificateRef::new(
                context.id(),
                Round::new(context.height(), 0),
                Phase::Commit,
                subject,
            ),
            (1_u8..=3)
                .map(|byte| {
                    SignatureShare::new(
                        ValidatorId::repeat(byte),
                        OpaqueSignature::new(vec![byte; 8]),
                    )
                })
                .collect(),
        );
        let mut decided = Reducer::recover(
            context,
            Some(ValidatorId::repeat(1)),
            Generation::new(8),
            [WalEntry::new(
                PersistenceId::new(1),
                WalRecord::Decision(decision.clone()),
            )],
        )
        .expect("replay exact Decision");
        decided.replay_resumed = true;
        (decided, decision)
    }
    fn certificate(
        context: &HeightContext,
        view: u64,
        phase: Phase,
        subject: Subject,
        signature_byte: u8,
    ) -> QuorumCertificate {
        QuorumCertificate::new(
            CertificateRef::new(
                context.id(),
                Round::new(context.height(), view),
                phase,
                subject,
            ),
            (1_u8..=3)
                .map(|signer| {
                    SignatureShare::new(
                        ValidatorId::repeat(signer),
                        OpaqueSignature::new(vec![signature_byte; 8]),
                    )
                })
                .collect(),
        )
    }
    fn timeout_certificate(
        context: &HeightContext,
        view: u64,
        highest_prepare: Option<QuorumCertificate>,
    ) -> TimeoutCertificate {
        TimeoutCertificate::new(
            context.id(),
            Round::new(context.height(), view),
            vec![TimeoutSignatureGroup::new(
                highest_prepare,
                (1_u8..=3)
                    .map(|signer| {
                        SignatureShare::new(
                            ValidatorId::repeat(signer),
                            OpaqueSignature::new(vec![signer; 8]),
                        )
                    })
                    .collect(),
            )],
        )
    }
    fn pending_timeout_install_at_generation(
        generation: Generation,
        highest_prepare: Option<QuorumCertificate>,
    ) -> (Reducer, Event) {
        let mut before = reducer();
        before.generation = generation;
        let certificate = timeout_certificate(&before.context, 0, highest_prepare);
        let outcome = before
            .step(Event::TimeoutCertificateReceived {
                tag: before.current_tag(),
                certificate,
            })
            .expect("start exact timeout-certificate persistence");
        let id = match outcome.effects() {
            [Effect::Persist { entry, .. }] => entry.id(),
            effects => panic!("expected one timeout persistence effect, got {effects:?}"),
        };
        let event = Event::Persisted {
            tag: before.current_tag(),
            id,
        };
        (before, event)
    }
    fn pending_timeout_install(highest_prepare: Option<QuorumCertificate>) -> (Reducer, Event) {
        pending_timeout_install_at_generation(Generation::new(7), highest_prepare)
    }
    fn pending_same_round_timeout_upgrade_at_generation(
        generation: Generation,
    ) -> (Reducer, Event) {
        let mut before = reducer();
        let first = timeout_certificate(&before.context, 0, None);
        let first_effect = before
            .step(Event::TimeoutCertificateReceived {
                tag: before.current_tag(),
                certificate: first,
            })
            .expect("stage the initial view-advancing timeout install");
        let first_id = match first_effect.effects() {
            [Effect::Persist { entry, .. }] => entry.id(),
            effects => panic!("expected one initial timeout persistence effect, got {effects:?}"),
        };
        before
            .on_persisted(first_id)
            .expect("acknowledge the initial timeout install");
        before.generation = generation;
        let selected = certificate(
            &before.context,
            0,
            Phase::Prepare,
            Subject::repeat(0xa7),
            0xa7,
        );
        let alternate = timeout_certificate(&before.context, 0, Some(selected));
        let outcome = before
            .step(Event::TimeoutCertificateReceived {
                tag: before.current_tag(),
                certificate: alternate,
            })
            .expect("stage the strict same-round timeout upgrade");
        let id = match outcome.effects() {
            [Effect::Persist { entry, .. }] => entry.id(),
            effects => panic!("expected one upgrade persistence effect, got {effects:?}"),
        };
        let event = Event::Persisted {
            tag: before.current_tag(),
            id,
        };
        (before, event)
    }
    include!("tests/reducer_timeout_and_projection.rs");
    include!("tests/v2_core_reducer_primitive_projection.rs");
    #[test]
    fn certificate_evidence_priority_and_signer_bitmap_match_the_roster_bound() {
        assert!(MAX_VOTING_ROSTER_LEN <= u128::BITS as usize);
        let fixture = reducer();
        let certificate = certificate(
            &fixture.context,
            0,
            Phase::Prepare,
            Subject::repeat(0xb9),
            0xba,
        );
        assert_eq!(
            Reducer::certificate_evidence_class(
                Some(&certificate),
                Some(&certificate),
                Some(&certificate),
            ),
            CERTIFICATE_EVIDENCE_LOCAL
        );
        assert_eq!(
            fixture.certificate_signer_projection(&certificate),
            Some((0b111, 3, 3, 3))
        );
        let (bitmap, bitmap_count, signer_count, _) = fixture
            .certificate_signer_projection(&certificate)
            .expect("valid certificate signer projection");
        assert_eq!(u64::from(bitmap.count_ones()), bitmap_count);
        assert_eq!(bitmap_count, signer_count);
    }
    #[test]
    fn decided_local_completion_cannot_overwrite_invalid_exact_body() {
        let subject = Subject::repeat(0xa7);
        let manifest =
            PayloadManifest::new(subject, Digest::repeat(0xa8), Digest::repeat(0xa9), 128, 2);
        let (mut decided, decision) = decided_reducer(subject);
        decided.body_work.insert(
            (decision.round(), subject),
            BodyWork {
                manifest: Some(manifest),
                state: BodyState::Invalid,
            },
        );
        let before = decided.clone();
        assert_eq!(
            decided.on_local_proposal_ready(manifest),
            Err(ReducerError::ProgressWitnessViolation(
                ProgressWitnessViolation::DecidedBodyInvalid
            ))
        );
        assert_eq!(decided, before);
    }
    #[test]
    fn counterfeit_effect_grant_with_a_different_primitive_key_fails_closed() {
        let before = reducer();
        let after = before.clone();
        let event = Event::RetransmitElapsed {
            tag: before.current_tag(),
        };
        let mut projection = before.transition_projection(&event, &after, &[]);
        let requested = EffectCapabilityKey {
            kind: EFFECT_BROADCAST,
            persistence_id: 41,
            ..EffectCapabilityKey::none()
        };
        let granted = EffectCapabilityKey {
            persistence_id: 42,
            ..requested
        };
        projection.effects.len = 1;
        projection.effects.slot0 = refinement::EffectSlotProjection {
            kind: EFFECT_BROADCAST,
            requested,
            granted,
        };
        assert!(!refinement::accepts(projection));
    }
    #[test]
    fn retransmit_body_stage_requires_an_exact_durable_decision_capability() {
        let before = reducer();
        let after = before.clone();
        let event = Event::RetransmitElapsed {
            tag: before.current_tag(),
        };
        let round = Round::new(before.context.height(), before.durable.current_view());
        for effect in [
            Effect::StoreBody {
                tag: before.current_tag(),
                round,
                subject: Subject::repeat(0xa1),
            },
            Effect::ValidateBody {
                tag: before.current_tag(),
                round,
                subject: Subject::repeat(0xa1),
            },
        ] {
            let projection =
                before.transition_projection(&event, &after, std::slice::from_ref(&effect));
            assert!(
                !refinement::accepts(projection),
                "a retransmit tick cannot invent {effect:?} without the exact Decision"
            );
        }
    }
    #[test]
    fn certified_fetch_capability_requires_the_exact_proposal_origin() {
        let fresh = reducer();
        let context = fresh.context.clone();
        let subject = Subject::repeat(0xb1);
        let finality_round = Round::new(context.height(), 2);
        let proposal_round = finality_round;
        let wrong_round = Round::new(context.height(), 1);
        let decision = QuorumCertificate::new(
            CertificateRef::new_with_proposal_round(
                context.id(),
                finality_round,
                proposal_round,
                Phase::Commit,
                subject,
            ),
            (1_u8..=3)
                .map(|signer| {
                    SignatureShare::new(
                        ValidatorId::repeat(signer),
                        OpaqueSignature::new(vec![signer; 8]),
                    )
                })
                .collect(),
        );
        let before = Reducer::recover(
            context,
            Some(ValidatorId::repeat(1)),
            Generation::new(10),
            [WalEntry::new(
                PersistenceId::new(1),
                WalRecord::Decision(decision.clone()),
            )],
        )
        .expect("recover a later local-view but internally same-round Decision");
        let event = Event::RetransmitElapsed {
            tag: before.current_tag(),
        };
        let certified_sources = before.frozen_archive_sources();
        assert_eq!(certified_sources.len(), before.context.roster().len());
        assert!(
            certified_sources.contains(&ValidatorId::repeat(4)),
            "a frozen-roster archive that did not sign the QC remains a deterministic source"
        );
        let mut wrong_after = before.clone();
        wrong_after.body_work.insert(
            (wrong_round, subject),
            BodyWork {
                manifest: None,
                state: BodyState::Missing,
            },
        );
        let wrong_fetch = Effect::FetchBody {
            tag: wrong_after.current_tag(),
            round: wrong_round,
            subject,
            manifest: None,
            certified_sources: certified_sources.clone(),
            certificate: Some(decision.clone()),
        };
        assert_eq!(
            before
                .granted_effect_capability(&event, &wrong_after, &wrong_fetch)
                .kind,
            refinement::EFFECT_NONE
        );
        let mut exact_after = before.clone();
        exact_after.body_work.insert(
            (proposal_round, subject),
            BodyWork {
                manifest: None,
                state: BodyState::Missing,
            },
        );
        let exact_fetch = Effect::FetchBody {
            tag: exact_after.current_tag(),
            round: proposal_round,
            subject,
            manifest: None,
            certified_sources,
            certificate: Some(decision),
        };
        assert_eq!(
            before
                .granted_effect_capability(&event, &exact_after, &exact_fetch)
                .kind,
            EFFECT_FETCH
        );
    }
    #[test]
    fn closed_proposal_round_cannot_create_a_new_commit_intent() {
        let fixture = reducer();
        let context = fixture.context.clone();
        let local = ValidatorId::repeat(1);
        let subject = Subject::repeat(0xb2);
        let proposal_round = Round::new(context.height(), 0);
        let finality_round = Round::new(context.height(), 1);
        let prepare = certificate(&context, 0, Phase::Prepare, subject, 0xb3);
        let timeout = timeout_certificate(&context, 0, Some(prepare.clone()));
        let timeout_vote =
            TimeoutVote::new(context.id(), finality_round, local, Some(prepare.clone()));
        let entries = [
            WalEntry::new(
                PersistenceId::new(1),
                WalRecord::LockAndCommit {
                    prepare: prepare.clone(),
                    vote: Vote::new_with_proposal_round(
                        context.id(),
                        proposal_round,
                        proposal_round,
                        Phase::Commit,
                        subject,
                        local,
                    ),
                },
            ),
            WalEntry::new(PersistenceId::new(2), WalRecord::InstallTimeout(timeout)),
            WalEntry::new(
                PersistenceId::new(3),
                WalRecord::TimeoutIntent(timeout_vote),
            ),
        ];
        let mut recovered = Reducer::recover(context, Some(local), Generation::new(11), entries)
            .expect("recover current-view timeout above retained same-round Commit");
        let outcome = recovered
            .persist_commit_intent(prepare)
            .expect("the closed proposal round is a non-fatal liveness outcome");
        assert_eq!(
            outcome.disposition(),
            StepDisposition::Ignored(IgnoreReason::IrrelevantView)
        );
        assert!(outcome.effects().is_empty());
        assert!(recovered.pending_persistence.is_none());
    }
    include!("reducer/counterfeit_boundary_capability_test.rs");
}
