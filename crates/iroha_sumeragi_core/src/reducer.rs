use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    fmt,
};

use crate::{
    CertificateRef, ConsensusMessageV2, DurableState, EventTag, Generation, HeightContext,
    OpaqueSignature, PayloadManifest, PersistenceId, Phase, Proposal, ProposalJustification,
    Quorum, QuorumCertificate, QuorumError, ReplayError, Round, SignatureShare, SignedProposal,
    SignedTimeoutVote, SignedVote, Subject, TimeoutCertificate, TimeoutSignatureGroup, TimeoutVote,
    ValidatorId, Vote, WalEntry, WalRecord,
    refinement::{
        self, ACTION_ACKNOWLEDGE_WAL, ACTION_BEGIN_WAL, ACTION_BODY_PROGRESS,
        ACTION_COMPLETE_APPLICATION, ACTION_STUTTER, ACTION_VOLATILE_PROTOCOL, CONTINUATION_DECIDE,
        CONTINUATION_INSTALL_TIMEOUT, CONTINUATION_NONE, CONTINUATION_SIGN, EFFECT_APPLY,
        EFFECT_BROADCAST, EFFECT_ENTER_VIEW, EFFECT_FETCH, EFFECT_PERSIST, EFFECT_REPORT,
        EFFECT_SIGN, EFFECT_STORE, EFFECT_VALIDATE, EVENT_BODY_AVAILABLE, EVENT_BODY_STORED,
        EVENT_PERSISTED, EVENT_SIGNED, EffectTrace, SIGNED_MESSAGE_COMMIT, SIGNED_MESSAGE_NONE,
        SIGNED_MESSAGE_PREPARE, SIGNED_MESSAGE_PROPOSAL, SIGNED_MESSAGE_TIMEOUT, TransitionFacts,
        VolatileSummary, WAL_RECORD_DECISION, WAL_RECORD_INSTALL_TIMEOUT,
        WAL_RECORD_LOCK_AND_COMMIT, WAL_RECORD_NONE, WAL_RECORD_OBSERVE_PREPARE,
        WAL_RECORD_PREPARE_INTENT, WAL_RECORD_PROPOSAL_INTENT, WAL_RECORD_TIMEOUT_INTENT,
    },
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

/// Side effect requested from an asynchronous production adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Append one complete frame to the safety WAL and fsync it.
    Persist {
        /// Tag that must accompany the acknowledgement.
        tag: EventTag,
        /// Complete frame to append.
        entry: WalEntry,
    },
    /// Fetch an exact body, optionally using QC signers as certified sources.
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
        /// Certificate authorizing a certified fetch, absent for an
        /// uncertified leader proposal.
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
    },
    /// Report authenticated equivocation without changing safety state.
    ReportEquivocation {
        /// Validator that produced conflicting messages.
        offender: ValidatorId,
        /// Round in which the conflict occurred.
        round: Round,
        /// Conflicting message class.
        kind: EquivocationKind,
    },
    /// Report a deterministic validation failure for a certified subject.
    ReportInvalidCertifiedBody {
        /// Subject rejected by the deterministic validator.
        subject: Subject,
        /// `PrepareQC` whose signers certified the subject as valid.
        certificate: QuorumCertificate,
    },
}

/// Storage-issued evidence that the decided block and its exact `CommitQC` are
/// durably visible as one finalized Kura height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DurableCommitReceipt {
    context_id: crate::ContextId,
    height: u64,
    subject: Subject,
    certificate: CertificateRef,
}

impl DurableCommitReceipt {
    /// Construct a receipt at the trusted durable-storage boundary.
    #[must_use]
    pub const fn from_trusted_storage(
        context_id: crate::ContextId,
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
    /// Ask the reducer to repeat liveness-critical body acquisition after the
    /// derived retransmission interval elapses.
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
            Self::LocalProposalReady { tag, .. }
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

    /// Replace the local delivery tag on an already authenticated network
    /// input before retrying it after reducer backpressure.
    ///
    /// Async adapter completions must retain their original tag. This method
    /// exists only so a serialized adapter can re-deliver authenticated
    /// Proposal/Vote/QC/TC messages after a queued TC changes the local view;
    /// in particular, an old-view `CommitQC` remains valid for the height.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    /// The certificate or vote is from a view that cannot affect local state.
    IrrelevantView,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingPersistence {
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

/// The sole executable Sumeragi v2 consensus state machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reducer {
    context: HeightContext,
    local_validator: Option<ValidatorId>,
    generation: Generation,
    durable: DurableState,
    candidate: Option<Proposal>,
    body_work: BTreeMap<(Round, Subject), BodyWork>,
    pending_prepare: BTreeMap<CertificateRef, QuorumCertificate>,
    known_prepare: BTreeMap<CertificateRef, QuorumCertificate>,
    votes: BTreeMap<(Round, Phase), BTreeMap<ValidatorId, SignedVote>>,
    timeout_votes: BTreeMap<Round, BTreeMap<ValidatorId, SignedTimeoutVote>>,
    formed_certificates: BTreeSet<CertificateRef>,
    formed_timeouts: BTreeSet<Round>,
    outbound_control: BTreeMap<OutboundControlClass, ConsensusMessageV2>,
    pending_persistence: Option<PendingPersistence>,
    awaiting_signature: Option<SignableMessage>,
    signature_queue: VecDeque<SignableMessage>,
    replay_resumed: bool,
    applied_subject: Option<Subject>,
}

impl Reducer {
    /// Constructs a fresh reducer at view zero.
    ///
    /// # Errors
    ///
    /// Returns an error if the configured local validator is absent from the
    /// frozen voting roster.
    pub fn new(
        context: HeightContext,
        local_validator: Option<ValidatorId>,
        generation: Generation,
    ) -> Result<Self, ReducerError> {
        let durable = DurableState::new(&context);
        Self::from_durable(context, local_validator, generation, durable)
    }

    /// Reconstructs a reducer from complete WAL frames.
    ///
    /// # Errors
    ///
    /// Returns an error if WAL replay fails or the configured local validator
    /// is absent from the frozen roster.
    pub fn recover(
        context: HeightContext,
        local_validator: Option<ValidatorId>,
        generation: Generation,
        entries: impl IntoIterator<Item = WalEntry>,
    ) -> Result<Self, ReducerError> {
        let durable = DurableState::replay(&context, local_validator, entries)?;
        Self::from_durable(context, local_validator, generation, durable)
    }

    fn from_durable(
        context: HeightContext,
        local_validator: Option<ValidatorId>,
        generation: Generation,
        durable: DurableState,
    ) -> Result<Self, ReducerError> {
        if local_validator.is_some_and(|id| context.validator(&id).is_none()) {
            return Err(ReducerError::LocalValidatorNotInRoster);
        }
        let mut known_prepare = BTreeMap::new();
        if let Some(certificate) = durable.highest_prepare() {
            known_prepare.insert(certificate.reference(), certificate.clone());
        }
        if let Some(certificate) = durable.locked() {
            known_prepare.insert(certificate.reference(), certificate.clone());
        }
        let mut outbound_control = BTreeMap::new();
        if let Some(certificate) = durable.highest_prepare() {
            outbound_control.insert(
                OutboundControlClass::PrepareQc,
                ConsensusMessageV2::QuorumCertificate(certificate.clone()),
            );
        }
        if let Some(certificate) = durable.decision() {
            outbound_control.insert(
                OutboundControlClass::CommitQc,
                ConsensusMessageV2::QuorumCertificate(certificate.clone()),
            );
        }
        if let Some(certificate) = durable.last_timeout() {
            outbound_control.insert(
                OutboundControlClass::TimeoutCertificate,
                ConsensusMessageV2::TimeoutCertificate(certificate.clone()),
            );
        }
        Ok(Self {
            context,
            local_validator,
            generation,
            durable,
            candidate: None,
            body_work: BTreeMap::new(),
            pending_prepare: BTreeMap::new(),
            known_prepare,
            votes: BTreeMap::new(),
            timeout_votes: BTreeMap::new(),
            formed_certificates: BTreeSet::new(),
            formed_timeouts: BTreeSet::new(),
            outbound_control,
            pending_persistence: None,
            awaiting_signature: None,
            signature_queue: VecDeque::new(),
            replay_resumed: false,
            applied_subject: None,
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

    /// Returns the state reconstructed from acknowledged WAL frames.
    #[must_use]
    pub const fn durable_state(&self) -> &DurableState {
        &self.durable
    }

    /// Return the decision subject successfully applied in this reducer
    /// incarnation, if any.
    #[must_use]
    pub const fn applied_subject(&self) -> Option<Subject> {
        self.applied_subject
    }

    /// Return whether the height can be consumed with a matching durable Kura
    /// receipt without dropping unfinished safety work.
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

    /// Returns the body state for a round and subject.
    #[must_use]
    pub fn body_state(&self, round: Round, subject: Subject) -> BodyState {
        self.body_work
            .get(&(round, subject))
            .map_or(BodyState::Missing, |work| work.state)
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

    /// Resumes safe effects after WAL replay.
    ///
    /// The WAL intent is the authorization boundary. Replay may reconstruct
    /// and retransmit an already-authorized Prepare or Commit signature even
    /// after a later timeout intent, but it never creates a new vote intent.
    /// A durable decision is fetched or applied instead.
    #[must_use]
    pub fn resume_after_replay(&mut self) -> Vec<Effect> {
        if self.replay_resumed {
            return Vec::new();
        }
        self.replay_resumed = true;
        if let Some(decision) = self.durable.decision().cloned() {
            return self.decision_effects(decision);
        }
        let round = Round::new(self.context.height(), self.durable.current_view());
        if let Some(timeout) = self.durable.timeout_intent(round) {
            self.signature_queue
                .push_back(SignableMessage::TimeoutVote(timeout));
        } else if let Some(proposal) = self.durable.proposal_intent(round) {
            self.signature_queue
                .push_back(SignableMessage::Proposal(proposal.clone()));
        }
        for vote in self.durable.prepare_intents() {
            self.signature_queue.push_back(SignableMessage::Vote(vote));
        }
        for vote in self.durable.commit_intents() {
            self.signature_queue.push_back(SignableMessage::Vote(vote));
        }
        self.drive_signature()
    }

    /// Applies one input to the serialized state machine.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed authenticated input, an invalid
    /// certificate, failed persistence, or an impossible durable-state change.
    pub fn step(&mut self, event: Event) -> Result<StepOutcome, ReducerError> {
        // The candidate transition is always private until the executable
        // refinement gate accepts its exact state/effect projection.  Even an
        // error path passes through the same gate as an empty stutter before
        // returning, so no `Reducer::step` exit bypasses the production kernel
        // verified in `verus_proofs.rs`.
        let audit_event = event.clone();
        let mut next = self.clone();
        match next.step_in_place(event) {
            Ok(outcome) => {
                if !self.transition_refines(&audit_event, &next, outcome.effects()) {
                    return Err(ReducerError::RefinementViolation);
                }
                *self = next;
                Ok(outcome)
            }
            Err(error) => {
                if !self.transition_refines(&audit_event, self, &[]) {
                    return Err(ReducerError::RefinementViolation);
                }
                Err(error)
            }
        }
    }

    fn step_in_place(&mut self, event: Event) -> Result<StepOutcome, ReducerError> {
        if let Some(reason) = self.reject_tag(event.tag()) {
            return Ok(StepOutcome::ignored(reason));
        }
        if self.pending_persistence.is_some()
            && !matches!(
                event,
                Event::Persisted { .. } | Event::PersistenceFailed { .. }
            )
        {
            return Ok(StepOutcome::ignored(IgnoreReason::Busy));
        }
        if self.awaiting_signature.is_some() && !matches!(event, Event::Signed { .. }) {
            return Ok(StepOutcome::ignored(IgnoreReason::Busy));
        }
        match event {
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
            Event::RetransmitElapsed { .. } => Ok(self.on_retransmit_elapsed()),
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
        let tag_matches = self.reject_tag(event.tag()).is_none();
        let persistence_completion = matches!(
            event,
            Event::Persisted { .. } | Event::PersistenceFailed { .. }
        );
        let signing_completion = matches!(event, Event::Signed { .. });
        let busy_fence_open = (self.pending_persistence.is_none() || persistence_completion)
            && (self.awaiting_signature.is_none() || signing_completion);
        let trace = self.effect_trace(event, after, effects);

        let begin_persist_exact = self.begin_persist_is_exact(event, after);
        let acknowledge_persist_exact = self.acknowledgement_is_exact(event, after);
        let application_unchanged = self.applied_subject == after.applied_subject;
        let action_kind = if begin_persist_exact {
            ACTION_BEGIN_WAL
        } else if acknowledge_persist_exact {
            ACTION_ACKNOWLEDGE_WAL
        } else if self == after && effects.is_empty() {
            ACTION_STUTTER
        } else if !application_unchanged {
            ACTION_COMPLETE_APPLICATION
        } else if matches!(
            event,
            Event::BodyAvailable { .. }
                | Event::BodyStored { .. }
                | Event::ValidationCompleted { .. }
        ) {
            ACTION_BODY_PROGRESS
        } else {
            ACTION_VOLATILE_PROTOCOL
        };
        let wal_record_kind = if begin_persist_exact {
            after
                .pending_persistence
                .as_ref()
                .map_or(WAL_RECORD_NONE, |pending| {
                    Self::wal_record_kind(pending.entry.record())
                })
        } else if acknowledge_persist_exact {
            self.pending_persistence
                .as_ref()
                .map_or(WAL_RECORD_NONE, |pending| {
                    Self::wal_record_kind(pending.entry.record())
                })
        } else {
            WAL_RECORD_NONE
        };
        let signed_message_kind = if action_kind == ACTION_STUTTER
            || !matches!(event, Event::Signed { .. })
        {
            SIGNED_MESSAGE_NONE
        } else {
            self.awaiting_signature
                .as_ref()
                .map_or(SIGNED_MESSAGE_NONE, |message| match message {
                    SignableMessage::Proposal(_) => SIGNED_MESSAGE_PROPOSAL,
                    SignableMessage::Vote(vote) => match vote.phase() {
                        Phase::Prepare => SIGNED_MESSAGE_PREPARE,
                        Phase::Commit => SIGNED_MESSAGE_COMMIT,
                    },
                    SignableMessage::TimeoutVote(_) => SIGNED_MESSAGE_TIMEOUT,
                })
        };
        let acknowledgement_continuation = if acknowledge_persist_exact {
            self.pending_persistence
                .as_ref()
                .map_or(CONTINUATION_NONE, |pending| match pending.continuation {
                    Continuation::None => CONTINUATION_NONE,
                    Continuation::Sign(_) => CONTINUATION_SIGN,
                    Continuation::InstallTimeout { .. } => CONTINUATION_INSTALL_TIMEOUT,
                    Continuation::Decide { .. } => CONTINUATION_DECIDE,
                })
        } else {
            CONTINUATION_NONE
        };

        refinement::accepts(TransitionFacts {
            before_invariant: self.production_safety_invariant(),
            after_invariant: after.production_safety_invariant(),
            context_unchanged: self.context == after.context
                && self.local_validator == after.local_validator,
            tag_matches,
            busy_fence_open,
            event_kind: Self::event_kind(event),
            action_kind,
            wal_record_kind,
            signed_message_kind,
            validator_count: Self::cardinality(self.context.roster().len()),
            volatile_before: self.volatile_summary(),
            volatile_after: after.volatile_summary(),
            durable_unchanged: self.durable == after.durable,
            pending_unchanged: self.pending_persistence == after.pending_persistence,
            generation_unchanged: self.generation == after.generation,
            application_unchanged,
            begin_persist_exact,
            acknowledge_persist_exact,
            application_transition_exact: self.application_transition_is_exact(event, after),
            acknowledgement_continuation,
            effects: trace,
        })
    }

    fn effect_trace(&self, event: &Event, after: &Self, effects: &[Effect]) -> EffectTrace {
        let mut trace = EffectTrace::empty();
        for effect in effects {
            if !trace.push(
                Self::effect_kind(effect),
                self.effect_is_authorized(event, after, effect),
            ) {
                // A non-canonical length is rejected by the verified kernel.
                trace.len = u8::try_from(refinement::MAX_EFFECTS_PER_STEP + 1)
                    .expect("the fixed effect bound fits in u8");
                break;
            }
        }
        trace
    }

    fn production_safety_invariant(&self) -> bool {
        let durable_context_id = self.durable.context_id();
        let expected_context_id = self.context.id();
        let durable_height = self.durable.height();
        let expected_height = self.context.height();
        let durable_identity_matches =
            durable_context_id == expected_context_id && durable_height == expected_height;
        let async_fences_are_exclusive =
            self.pending_persistence.is_none() || self.awaiting_signature.is_none();
        if !durable_identity_matches || !async_fences_are_exclusive {
            return false;
        }
        if self.durable.highest_prepare().is_some_and(|certificate| {
            certificate.phase() != Phase::Prepare
                || certificate.round().view() > self.durable.current_view()
                || certificate.validate(&self.context).is_err()
        }) {
            return false;
        }
        if self.durable.locked().is_some_and(|locked| {
            locked.phase() != Phase::Prepare
                || locked.round().view() > self.durable.current_view()
                || locked.validate(&self.context).is_err()
                || self.durable.highest_prepare().is_none_or(|highest| {
                    highest.round().view() < locked.round().view()
                        || (highest.round().view() == locked.round().view()
                            && highest.subject() != locked.subject())
                })
        }) {
            return false;
        }
        if self.durable.last_timeout().is_some_and(|certificate| {
            certificate.validate(&self.context).is_err()
                || certificate.round().view().checked_add(1) != Some(self.durable.current_view())
        }) {
            return false;
        }
        if self.durable.decision().is_some_and(|certificate| {
            certificate.phase() != Phase::Commit || certificate.validate(&self.context).is_err()
        }) {
            return false;
        }
        if let Some(pending) = &self.pending_persistence {
            if self.durable.next_id() != Ok(pending.entry.id())
                || !Self::continuation_matches_record(pending.entry.record(), &pending.continuation)
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
        }
        if self
            .awaiting_signature
            .as_ref()
            .is_some_and(|message| !self.signable_is_durably_authorized(message))
            || self
                .signature_queue
                .iter()
                .any(|message| !self.signable_is_durably_authorized(message))
        {
            return false;
        }
        if let Some(subject) = self.applied_subject {
            let Some(decision) = self.durable.decision() else {
                return false;
            };
            if decision.subject() != subject
                || self.body_state(decision.round(), subject) != BodyState::Validated
            {
                return false;
            }
        }
        true
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
        match message {
            SignableMessage::Proposal(proposal) => {
                self.durable.proposal_intent(proposal.round()) == Some(proposal)
            }
            SignableMessage::Vote(vote) => match vote.phase() {
                Phase::Prepare => self.durable.prepare_intent(vote.round()) == Some(*vote),
                Phase::Commit => {
                    self.durable.commit_intent(vote.round()) == Some(*vote)
                        && self.durable.locked().is_some_and(|locked| {
                            locked.round() == vote.round() && locked.subject() == vote.subject()
                        })
                }
            },
            SignableMessage::TimeoutVote(vote) => {
                self.durable.timeout_intent(vote.round()) == Some(*vote)
            }
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
            (Event::TimeoutElapsed { .. }, WalRecord::TimeoutIntent(vote)) => {
                vote.round() == Round::new(self.context.height(), self.durable.current_view())
            }
            (Event::VoteReceived { vote, .. }, WalRecord::ObservePrepare(certificate)) => {
                vote.vote().phase() == Phase::Prepare
                    && certificate.reference()
                        == CertificateRef::new(
                            self.context.id(),
                            vote.vote().round(),
                            Phase::Prepare,
                            vote.vote().subject(),
                        )
            }
            (Event::VoteReceived { vote, .. }, WalRecord::LockAndCommit { prepare, .. }) => {
                vote.vote().phase() == Phase::Prepare
                    && prepare.reference()
                        == CertificateRef::new(
                            self.context.id(),
                            vote.vote().round(),
                            Phase::Prepare,
                            vote.vote().subject(),
                        )
            }
            (Event::VoteReceived { vote, .. }, WalRecord::Decision(certificate)) => {
                vote.vote().phase() == Phase::Commit
                    && certificate.reference()
                        == CertificateRef::new(
                            self.context.id(),
                            vote.vote().round(),
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
                                    == CertificateRef::new(
                                        self.context.id(),
                                        vote.round(),
                                        Phase::Prepare,
                                        vote.subject(),
                                    )
                        }
                        (SignableMessage::Vote(vote), WalRecord::LockAndCommit { prepare, .. }) => {
                            vote.phase() == Phase::Prepare
                                && prepare.reference()
                                    == CertificateRef::new(
                                        self.context.id(),
                                        vote.round(),
                                        Phase::Prepare,
                                        vote.subject(),
                                    )
                        }
                        (SignableMessage::Vote(vote), WalRecord::Decision(certificate)) => {
                            vote.phase() == Phase::Commit
                                && certificate.reference()
                                    == CertificateRef::new(
                                        self.context.id(),
                                        vote.round(),
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
                self.generation.next() == Some(after.generation)
                    && after.durable.last_timeout() == Some(certificate)
                    && after.candidate.is_none()
                    && after.body_work.is_empty()
                    && after.pending_prepare.is_empty()
            }
            Continuation::Decide { certificate, .. } => {
                self.generation == after.generation && after.durable.decision() == Some(certificate)
            }
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
                && after.body_state(decision.round(), *subject) == BodyState::Validated
        })
    }

    const fn event_kind(event: &Event) -> u8 {
        match event {
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
            Event::PersistenceFailed { .. } => 12,
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
        let durable_signable_limit = 1usize
            .saturating_add(self.durable.prepare_intents().count())
            .saturating_add(self.durable.commit_intents().count());
        VolatileSummary {
            candidate_present: self.candidate.is_some(),
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

    #[allow(clippy::too_many_lines)]
    fn effect_is_authorized(&self, event: &Event, after: &Self, effect: &Effect) -> bool {
        match effect {
            Effect::Persist { tag, entry } => {
                *tag == after.current_tag()
                    && after
                        .pending_persistence
                        .as_ref()
                        .is_some_and(|pending| pending.entry == *entry)
                    && self.event_may_start_record(event, entry.record())
            }
            Effect::FetchBody {
                tag,
                round,
                subject,
                manifest,
                certified_sources,
                certificate,
            } => {
                if *tag != after.current_tag()
                    || round.height() != after.context.height()
                    || after.body_work.get(&(*round, *subject)).is_none_or(|work| {
                        manifest.is_some_and(|manifest| work.manifest != Some(manifest))
                    })
                {
                    return false;
                }
                certificate
                    .as_ref()
                    .map_or(certified_sources.is_empty(), |certificate| {
                        certificate.round() == *round
                            && certificate.subject() == *subject
                            && certificate.validate(&after.context).is_ok()
                            && certified_sources
                                == &certificate
                                    .signatures()
                                    .iter()
                                    .map(SignatureShare::signer)
                                    .collect::<Vec<_>>()
                    })
            }
            Effect::StoreBody {
                tag,
                round,
                subject,
            } => {
                *tag == after.current_tag()
                    && matches!(
                        event,
                        Event::BodyAvailable {
                            round: event_round,
                            subject: event_subject,
                            ..
                        } if event_round == round && event_subject == subject
                    )
                    && after.body_state(*round, *subject) == BodyState::Available
            }
            Effect::ValidateBody {
                tag,
                round,
                subject,
            } => {
                *tag == after.current_tag()
                    && matches!(
                        event,
                        Event::BodyStored {
                            round: event_round,
                            subject: event_subject,
                            ..
                        } if event_round == round && event_subject == subject
                    )
                    && after.body_state(*round, *subject) == BodyState::Durable
            }
            Effect::Sign { tag, message } => {
                *tag == after.current_tag()
                    && after.awaiting_signature.as_ref() == Some(message)
                    && after.signable_is_durably_authorized(message)
            }
            Effect::Broadcast(message) => match message {
                ConsensusMessageV2::Proposal(signed) => {
                    after.durable.proposal_intent(signed.proposal().round())
                        == Some(signed.proposal())
                }
                ConsensusMessageV2::Vote(signed) => {
                    let vote = signed.vote();
                    match vote.phase() {
                        Phase::Prepare => after.durable.prepare_intent(vote.round()) == Some(vote),
                        Phase::Commit => {
                            after.durable.commit_intent(vote.round()) == Some(vote)
                                && after.durable.locked().is_some_and(|locked| {
                                    locked.round() == vote.round()
                                        && locked.subject() == vote.subject()
                                })
                        }
                    }
                }
                ConsensusMessageV2::QuorumCertificate(certificate) => {
                    certificate.validate(&after.context).is_ok()
                        && (certificate.phase() == Phase::Prepare
                            || after.durable.decision().is_some_and(|decision| {
                                decision.reference() == certificate.reference()
                            }))
                }
                ConsensusMessageV2::TimeoutVote(signed) => {
                    after.durable.timeout_intent(signed.vote().round()) == Some(signed.vote())
                }
                ConsensusMessageV2::TimeoutCertificate(certificate) => {
                    after.durable.last_timeout() == Some(certificate)
                }
                ConsensusMessageV2::BodyRequest(_) | ConsensusMessageV2::BodyChunk(_) => false,
            },
            Effect::Apply {
                tag,
                subject,
                certificate,
            } => {
                *tag == after.current_tag()
                    && certificate.phase() == Phase::Commit
                    && after.durable.decision() == Some(certificate)
                    && certificate.subject() == *subject
                    && after.body_state(certificate.round(), *subject) == BodyState::Validated
            }
            Effect::EnterView { tag, certificate } => {
                *tag == after.current_tag()
                    && after.durable.last_timeout() == Some(certificate)
                    && certificate.round().view().checked_add(1)
                        == Some(after.durable.current_view())
            }
            Effect::ReportEquivocation {
                offender, round, ..
            } => {
                round.height() == after.context.height()
                    && after.context.validator(offender).is_some()
            }
            Effect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => {
                certificate.phase() == Phase::Prepare
                    && certificate.subject() == *subject
                    && certificate.validate(&after.context).is_ok()
            }
        }
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
        self.validate_proposal(proposal)?;
        let round = proposal.round();
        if self.durable.timeout_intent(round).is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::ViewClosed));
        }
        if let Some(existing) = &self.candidate {
            if existing.manifest() == proposal.manifest() {
                return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
            }
            return Ok(StepOutcome::applied(vec![Effect::ReportEquivocation {
                offender: proposal.proposer(),
                round,
                kind: EquivocationKind::Proposal,
            }]));
        }
        self.candidate = Some(proposal.clone());
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
            Ok(StepOutcome::applied(vec![Effect::FetchBody {
                tag: self.current_tag(),
                round,
                subject,
                manifest: Some(*proposal.manifest()),
                certified_sources: Vec::new(),
                certificate: None,
            }]))
        } else {
            Ok(StepOutcome::applied(Vec::new()))
        }
    }

    fn on_local_proposal_ready(
        &mut self,
        manifest: PayloadManifest,
    ) -> Result<StepOutcome, ReducerError> {
        if self.durable.decision().is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        let Some(proposer) = self.local_validator else {
            return Err(ReducerError::ObserverCannotPropose);
        };
        let round = Round::new(self.context.height(), self.durable.current_view());
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
                .cloned()
                .ok_or(ReducerError::MissingPersistedTimeoutJustification)?;
            ProposalJustification::Timeout(certificate)
        };
        let proposal = Proposal::new(self.context.id(), round, proposer, manifest, justification);
        self.validate_proposal(&proposal)?;
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
        let high = match proposal.justification() {
            ProposalJustification::ParentCommit(parent) => {
                // The same parent subject may acquire valid CommitQCs in more
                // than one view. The successor context deliberately ignores
                // that view and the signer subset, so proposal admission uses
                // the same semantic finality key. Otherwise nodes with equal
                // context IDs could reject one another's view-zero proposal.
                let same_finalized_parent = match (*parent, self.context.parent_commit()) {
                    (None, None) => true,
                    (Some(carried), Some(frozen)) => carried.same_commit_decision(frozen),
                    (None, Some(_)) | (Some(_), None) => false,
                };
                if proposal.round().view() != 0 || !same_finalized_parent {
                    return Err(ReducerError::InvalidProposalJustification);
                }
                None
            }
            ProposalJustification::Timeout(certificate) => {
                certificate.validate(&self.context)?;
                if proposal.round().view() == 0
                    || certificate.round().view().checked_add(1) != Some(proposal.round().view())
                {
                    return Err(ReducerError::InvalidProposalJustification);
                }
                let high = certificate.highest_prepare();
                if high.is_some_and(|certificate| {
                    certificate.subject() != proposal.manifest().subject()
                }) {
                    return Err(ReducerError::UnsafeProposal);
                }
                high
            }
        };
        if !self.safe_to_prepare(proposal.manifest().subject(), high) {
            return Err(ReducerError::UnsafeProposal);
        }
        Ok(())
    }

    fn safe_to_prepare(&self, subject: Subject, proposal_high: Option<&QuorumCertificate>) -> bool {
        let Some(locked) = self.durable.locked() else {
            return true;
        };
        if locked.subject() == subject {
            return true;
        }
        proposal_high.is_some_and(|high| {
            high.phase() == Phase::Prepare
                && high.subject() == subject
                && high.round().view() > locked.round().view()
        })
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
        if let Some(decision) = self.durable.decision().cloned()
            && decision.subject() == subject
        {
            return Ok(StepOutcome::applied(vec![Effect::Apply {
                tag: self.current_tag(),
                subject,
                certificate: decision,
            }]));
        }
        if self.durable.decision().is_some() {
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
        let round = prepare.round();
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
        if vote.round().view() != self.durable.current_view() {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        let pool = self.votes.entry((vote.round(), vote.phase())).or_default();
        if let Some(existing) = pool.get(&vote.signer()) {
            if existing.vote().subject() == vote.subject() {
                return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
            }
            return Ok(StepOutcome::applied(vec![Effect::ReportEquivocation {
                offender: vote.signer(),
                round: vote.round(),
                kind: EquivocationKind::Vote,
            }]));
        }
        pool.insert(vote.signer(), signed);
        let Some(certificate) =
            self.try_form_certificate(vote.round(), vote.phase(), vote.subject())?
        else {
            return Ok(StepOutcome::applied(Vec::new()));
        };
        self.on_certificate(certificate, true)
    }

    fn validate_vote(&self, vote: Vote) -> Result<(), ReducerError> {
        if vote.context_id() != self.context.id()
            || vote.round().height() != self.context.height()
            || self.context.validator(&vote.signer()).is_none()
        {
            return Err(ReducerError::InvalidVote);
        }
        Ok(())
    }

    fn try_form_certificate(
        &mut self,
        round: Round,
        phase: Phase,
        subject: Subject,
    ) -> Result<Option<QuorumCertificate>, ReducerError> {
        let reference = CertificateRef::new(self.context.id(), round, phase, subject);
        if self.formed_certificates.contains(&reference) {
            return Ok(None);
        }
        let Some(pool) = self.votes.get(&(round, phase)) else {
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

    fn on_certificate(
        &mut self,
        certificate: QuorumCertificate,
        formed_locally: bool,
    ) -> Result<StepOutcome, ReducerError> {
        certificate.validate(&self.context)?;
        match certificate.phase() {
            Phase::Prepare => self.on_prepare_certificate(certificate, formed_locally),
            Phase::Commit => self.on_commit_certificate(certificate, formed_locally),
        }
    }

    fn on_prepare_certificate(
        &mut self,
        certificate: QuorumCertificate,
        formed_locally: bool,
    ) -> Result<StepOutcome, ReducerError> {
        if self.durable.decision().is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        if certificate.round().view() > self.durable.current_view() {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        let reference = certificate.reference();
        self.known_prepare
            .entry(reference)
            .or_insert_with(|| certificate.clone());
        self.pending_prepare
            .entry(reference)
            .or_insert_with(|| certificate.clone());
        self.remember_control(ConsensusMessageV2::QuorumCertificate(certificate.clone()));
        let mut effects = Vec::new();
        if formed_locally {
            let message = ConsensusMessageV2::QuorumCertificate(certificate.clone());
            self.remember_control(message.clone());
            effects.push(Effect::Broadcast(message));
        }

        let current = certificate.round().view() == self.durable.current_view();
        let validated =
            self.body_state(certificate.round(), certificate.subject()) == BodyState::Validated;
        let view_closed = self.durable.timeout_intent(certificate.round()).is_some();
        if current && validated && !view_closed && self.local_validator.is_some() {
            let mut outcome = self.persist_commit_intent(certificate)?;
            effects.append(&mut outcome.effects);
            return Ok(StepOutcome::applied(effects));
        }

        if current
            && self.body_state(certificate.round(), certificate.subject()) == BodyState::Missing
        {
            effects.push(self.ensure_body_fetch(&certificate));
        }

        let should_persist_high = match self.durable.highest_prepare() {
            None => true,
            Some(existing) => {
                if existing.round().view() == certificate.round().view()
                    && existing.subject() != certificate.subject()
                {
                    return Err(ReducerError::ConflictingPrepareCertificates);
                }
                certificate.round().view() > existing.round().view()
            }
        };
        if should_persist_high {
            let persist =
                self.start_persistence(WalRecord::ObservePrepare(certificate), Continuation::None)?;
            effects.push(persist);
        }
        Ok(StepOutcome::applied(effects))
    }

    fn ensure_body_fetch(&mut self, certificate: &QuorumCertificate) -> Effect {
        let round = certificate.round();
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
            certified_sources: certificate
                .signatures()
                .iter()
                .map(SignatureShare::signer)
                .collect(),
            certificate: Some(certificate.clone()),
        }
    }

    fn on_commit_certificate(
        &mut self,
        certificate: QuorumCertificate,
        formed_locally: bool,
    ) -> Result<StepOutcome, ReducerError> {
        if let Some(existing) = self.durable.decision() {
            if existing.subject() != certificate.subject() {
                return Err(ReducerError::ConflictingDecision);
            }
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        let effect = self.start_persistence(
            WalRecord::Decision(certificate.clone()),
            Continuation::Decide {
                certificate,
                broadcast: formed_locally,
            },
        )?;
        Ok(StepOutcome::applied(vec![effect]))
    }

    fn on_retransmit_elapsed(&mut self) -> StepOutcome {
        let mut effects: Vec<_> = self
            .outbound_control
            .values()
            .cloned()
            .map(Effect::Broadcast)
            .collect();
        if let Some(decision) = self.durable.decision().cloned() {
            match self.body_state(decision.round(), decision.subject()) {
                BodyState::Missing => effects.push(self.ensure_body_fetch(&decision)),
                BodyState::Validated if self.applied_subject != Some(decision.subject()) => {
                    effects.push(Effect::Apply {
                        tag: self.current_tag(),
                        subject: decision.subject(),
                        certificate: decision,
                    });
                }
                BodyState::Available
                | BodyState::Durable
                | BodyState::Validated
                | BodyState::Invalid => {}
            }
            return StepOutcome::applied(effects);
        }

        if let Some(certificate) = self
            .pending_prepare
            .values()
            .find(|certificate| {
                certificate.round().view() == self.durable.current_view()
                    && self.body_state(certificate.round(), certificate.subject())
                        == BodyState::Missing
            })
            .cloned()
        {
            effects.push(self.ensure_body_fetch(&certificate));
            return StepOutcome::applied(effects);
        }

        if let Some(proposal) = self.candidate.clone()
            && self.body_state(proposal.round(), proposal.manifest().subject())
                == BodyState::Missing
        {
            effects.push(Effect::FetchBody {
                tag: self.current_tag(),
                round: proposal.round(),
                subject: proposal.manifest().subject(),
                manifest: Some(*proposal.manifest()),
                certified_sources: Vec::new(),
                certificate: None,
            });
            return StepOutcome::applied(effects);
        }

        StepOutcome::applied(effects)
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
            round.view() > existing_round.view() || &message == existing
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
            self.durable
                .highest_prepare()
                .map(QuorumCertificate::reference),
        );
        let effect = self.start_persistence(
            WalRecord::TimeoutIntent(timeout),
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
        if vote.round().view() != self.durable.current_view() {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        if let Some(reference) = vote.highest_prepare()
            && (reference.phase() != Phase::Prepare
                || reference.context_id() != self.context.id()
                || reference.round().height() != self.context.height()
                || reference.round().view() > vote.round().view())
        {
            return Err(ReducerError::InvalidTimeoutVote);
        }
        let pool = self.timeout_votes.entry(vote.round()).or_default();
        if let Some(existing) = pool.get(&vote.signer()) {
            if existing.vote().highest_prepare() == vote.highest_prepare() {
                return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
            }
            return Ok(StepOutcome::applied(vec![Effect::ReportEquivocation {
                offender: vote.signer(),
                round: vote.round(),
                kind: EquivocationKind::Timeout,
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
        let (quorum, _) = Quorum::from_iter(
            &self.context,
            pool.values().map(|signed| signed.vote().signer()),
        )?;
        if !quorum.satisfies(&self.context) {
            return Ok(None);
        }
        let mut grouped: BTreeMap<Option<CertificateRef>, Vec<SignatureShare>> = BTreeMap::new();
        for signed in pool.values() {
            let vote = signed.vote();
            if vote
                .highest_prepare()
                .is_some_and(|reference| !self.known_prepare.contains_key(&reference))
            {
                return Ok(None);
            }
            grouped
                .entry(vote.highest_prepare())
                .or_default()
                .push(SignatureShare::new(
                    vote.signer(),
                    signed.signature().clone(),
                ));
        }
        let groups = grouped
            .into_iter()
            .map(|(reference, signatures)| {
                TimeoutSignatureGroup::new(
                    reference.and_then(|key| self.known_prepare.get(&key).cloned()),
                    signatures,
                )
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
        if certificate.round().view() < self.durable.current_view() {
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
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
        let mut durable = self.durable.clone();
        durable.apply(&self.context, self.local_validator, &pending.entry)?;
        self.durable = durable;
        self.pending_persistence = None;

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
                if broadcast {
                    vec![Effect::Broadcast(message)]
                } else {
                    Vec::new()
                }
            }
            .into_iter()
            .chain({
                self.generation = self
                    .generation
                    .next()
                    .ok_or(ReducerError::GenerationOverflow)?;
                self.candidate = None;
                self.body_work.clear();
                self.pending_prepare.clear();
                self.votes.clear();
                self.timeout_votes.clear();
                self.formed_certificates.clear();
                self.formed_timeouts.clear();
                self.known_prepare.clear();
                if let Some(highest) = self.durable.highest_prepare() {
                    self.known_prepare
                        .insert(highest.reference(), highest.clone());
                }
                if let Some(locked) = self.durable.locked() {
                    self.known_prepare
                        .insert(locked.reference(), locked.clone());
                }
                self.outbound_control.retain(|class, _| {
                    matches!(
                        class,
                        OutboundControlClass::CommitVote
                            | OutboundControlClass::CommitQc
                            | OutboundControlClass::TimeoutCertificate
                    )
                });
                [Effect::EnterView {
                    tag: self.current_tag(),
                    certificate,
                }]
            })
            .collect(),
            Continuation::Decide {
                certificate,
                broadcast,
            } => {
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
        let Some(message) = self.signature_queue.pop_front() else {
            return Vec::new();
        };
        self.awaiting_signature = Some(message.clone());
        vec![Effect::Sign {
            tag: self.current_tag(),
            message,
        }]
    }

    fn on_signed(&mut self, signature: OpaqueSignature) -> Result<StepOutcome, ReducerError> {
        let Some(message) = self.awaiting_signature.take() else {
            return Ok(StepOutcome::ignored(IgnoreReason::NoMatchingWork));
        };
        let mut effects = Vec::new();
        match message {
            SignableMessage::Proposal(proposal) => {
                let signed = SignedProposal::new(proposal.clone(), signature);
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
                effects.push(Effect::Broadcast(message));
                let mut local = self.on_timeout_vote(signed)?.into_effects();
                effects.append(&mut local);
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
        if decision.subject() != subject
            || self.body_state(decision.round(), subject) != BodyState::Validated
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
        let round = certificate.round();
        let subject = certificate.subject();
        if self.body_state(round, subject) == BodyState::Validated
            && self.applied_subject != Some(subject)
        {
            return vec![Effect::Apply {
                tag: self.current_tag(),
                subject,
                certificate,
            }];
        }
        vec![self.ensure_body_fetch(&certificate)]
    }
}

/// Failure caused by malformed authenticated input or an impossible local state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReducerError {
    /// The executable transition failed the mechanically verified refinement
    /// gate and was discarded before becoming caller-visible.
    RefinementViolation,
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
