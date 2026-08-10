//! Pure lifecycle schema and value types for the Sumeragi v2 coordinator.

use std::collections::{BTreeMap, BTreeSet};

pub(super) const MAX_PHYSICAL_SLOTS_PER_RECORD: usize = 64;
pub(super) const MAX_LIFECYCLE_RECORDS_PER_HEIGHT: usize = u16::MAX as usize + 1;

pub(super) fn has_lifecycle_record_capacity(current: usize, additional: usize) -> bool {
    current
        .checked_add(additional)
        .is_some_and(|records| records <= MAX_LIFECYCLE_RECORDS_PER_HEIGHT)
}

/// Digest of an authenticated semantic projection; never a physical work ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct LifecycleDigest(pub(super) [u8; 32]);

impl LifecycleDigest {
    /// Construct a digest from an already authenticated projection.
    pub(super) const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the canonical digest bytes.
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Context identity paired with its exact consensus height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LifecycleContext {
    pub(super) id: LifecycleDigest,
    pub(super) height: u64,
}

impl LifecycleContext {
    /// Construct a typed lifecycle context.
    pub(super) const fn new(id: LifecycleDigest, height: u64) -> Self {
        Self { id, height }
    }

    /// Return the authenticated context identity.
    pub(crate) const fn id(self) -> LifecycleDigest {
        self.id
    }

    /// Return the exact context height.
    pub(crate) const fn height(self) -> u64 {
        self.height
    }
}

/// Height/view coordinates retained in a semantic lifecycle key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LifecycleRound {
    pub(super) height: u64,
    pub(super) view: u64,
}

impl LifecycleRound {
    /// Construct deterministic height/view coordinates.
    pub(super) const fn new(height: u64, view: u64) -> Self {
        Self { height, view }
    }

    /// Return the consensus height.
    pub(crate) const fn height(self) -> u64 {
        self.height
    }

    /// Return the consensus view.
    pub(crate) const fn view(self) -> u64 {
        self.view
    }
}

/// Closed semantic phase inventory for lifecycle keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LifecyclePhase {
    Proposal,
    Prepare,
    Commit,
    Timeout,
    Fetch,
    Store,
    Validate,
    Apply,
    BroadcastProposal,
    BroadcastPrepareVote,
    BroadcastCommitVote,
    BroadcastPrepareQc,
    BroadcastCommitQc,
    BroadcastTimeoutVote,
    BroadcastTc,
    EnterView,
    DiagnosticProposalEquivocation,
    DiagnosticVoteEquivocation,
    DiagnosticTimeoutEquivocation,
    DiagnosticInvalidBody,
    Serve,
    ProducerTurn,
}

impl LifecyclePhase {
    pub(super) const ALL: [Self; 22] = [
        Self::Proposal,
        Self::Prepare,
        Self::Commit,
        Self::Timeout,
        Self::Fetch,
        Self::Store,
        Self::Validate,
        Self::Apply,
        Self::BroadcastProposal,
        Self::BroadcastPrepareVote,
        Self::BroadcastCommitVote,
        Self::BroadcastPrepareQc,
        Self::BroadcastCommitQc,
        Self::BroadcastTimeoutVote,
        Self::BroadcastTc,
        Self::EnterView,
        Self::DiagnosticProposalEquivocation,
        Self::DiagnosticVoteEquivocation,
        Self::DiagnosticTimeoutEquivocation,
        Self::DiagnosticInvalidBody,
        Self::Serve,
        Self::ProducerTurn,
    ];
}

/// Route- and carrier-independent identity of one logical lifecycle stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LifecycleKey {
    pub(super) context: LifecycleDigest,
    pub(super) round: LifecycleRound,
    pub(super) proposal_round: Option<LifecycleRound>,
    pub(super) subject: Option<LifecycleDigest>,
    pub(super) phase: LifecyclePhase,
    pub(super) execution_commitment: Option<LifecycleDigest>,
}

impl LifecycleKey {
    /// Construct a complete semantic lifecycle key.
    pub(super) const fn new(
        context: LifecycleDigest,
        round: LifecycleRound,
        proposal_round: Option<LifecycleRound>,
        subject: Option<LifecycleDigest>,
        phase: LifecyclePhase,
        execution_commitment: Option<LifecycleDigest>,
    ) -> Self {
        Self {
            context,
            round,
            proposal_round,
            subject,
            phase,
            execution_commitment,
        }
    }

    /// Return the authenticated lifecycle context.
    pub(crate) const fn context(self) -> LifecycleDigest {
        self.context
    }

    /// Return the execution round.
    pub(crate) const fn round(self) -> LifecycleRound {
        self.round
    }

    /// Return the optional proposal round.
    pub(crate) const fn proposal_round(self) -> Option<LifecycleRound> {
        self.proposal_round
    }

    /// Return the domain-separated semantic subject.
    pub(crate) const fn subject(self) -> Option<LifecycleDigest> {
        self.subject
    }

    /// Return the exact logical phase.
    pub(crate) const fn phase(self) -> LifecyclePhase {
        self.phase
    }

    /// Return the optional deterministic execution commitment.
    pub(crate) const fn execution_commitment(self) -> Option<LifecycleDigest> {
        self.execution_commitment
    }

    /// Return the canonical scheduler target derivable from durable key data.
    ///
    /// Statement-bearing work targets its authenticated subject. Work without
    /// a statement subject targets the verified height context itself, while
    /// phase and view remain separate members of the episode universe.
    pub(super) const fn scheduler_target(self) -> LifecycleDigest {
        match self.subject {
            Some(subject) => subject,
            None => self.context,
        }
    }
}

/// Derive the only valid adjacent producer key from a Certified-Serve key.
pub(super) const fn producer_turn_key_for_serve(serve: LifecycleKey) -> Option<LifecycleKey> {
    if !matches!(serve.phase, LifecyclePhase::Serve) {
        return None;
    }
    Some(LifecycleKey::new(
        serve.context,
        serve.round,
        serve.proposal_round,
        serve.subject,
        LifecyclePhase::ProducerTurn,
        serve.execution_commitment,
    ))
}

/// Check that a Serve and ProducerTurn differ only by their closed phase tag.
pub(super) fn serve_and_producer_keys_match(serve: LifecycleKey, producer: LifecycleKey) -> bool {
    producer_turn_key_for_serve(serve) == Some(producer)
}

/// Immutable causal root supplied before physical refinement evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CausalRoot(pub(super) LifecycleDigest);

impl CausalRoot {
    /// Construct a causal root from its authenticated semantic digest.
    pub(super) const fn new(digest: LifecycleDigest) -> Self {
        Self(digest)
    }

    /// Return the authenticated causal-root digest.
    pub(crate) const fn digest(self) -> LifecycleDigest {
        self.0
    }
}

/// Immutable owner shared by every successor of one causal admission root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct OwnerId {
    pub(super) causal_root: CausalRoot,
    pub(super) first_admission_ordinal: u128,
}

impl OwnerId {
    /// Construct an owner reconstructed from durable lifecycle state.
    pub(super) const fn new(causal_root: CausalRoot, first_admission_ordinal: u128) -> Self {
        Self {
            causal_root,
            first_admission_ordinal,
        }
    }

    /// Return the immutable causal root.
    pub(crate) const fn causal_root(self) -> CausalRoot {
        self.causal_root
    }

    /// Return the first ordinal allocated to this owner.
    pub(crate) const fn first_admission_ordinal(self) -> u128 {
        self.first_admission_ordinal
    }
}

/// Exhaustive adapter-effect and scheduler-only work classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LifecycleWorkClass {
    SignProposal,
    SignVote,
    SignTimeout,
    Fetch,
    Store,
    Validate,
    Apply,
    Broadcast,
    EnterView,
    EquivocationReport,
    InvalidBodyReport,
    CertifiedServe,
    ProducerTurn,
}

impl LifecycleWorkClass {
    pub(super) const ALL: [Self; 13] = [
        Self::SignProposal,
        Self::SignVote,
        Self::SignTimeout,
        Self::Fetch,
        Self::Store,
        Self::Validate,
        Self::Apply,
        Self::Broadcast,
        Self::EnterView,
        Self::EquivocationReport,
        Self::InvalidBodyReport,
        Self::CertifiedServe,
        Self::ProducerTurn,
    ];
}

/// Deterministic action taken for an exact same-owner retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetryAction {
    StutterLiveSigner,
    ReenqueueIncumbent,
    RefanoutIncumbent,
    RefanoutEnvelope,
    StutterInstalledView,
    StutterDiagnostic,
    StutterProducerTurn,
}

/// Bounded logical capacity classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CapacityClass {
    Consensus,
    Effect,
    Serve,
    Producer,
}

impl CapacityClass {
    pub(super) const ALL: [Self; 4] = [Self::Consensus, Self::Effect, Self::Serve, Self::Producer];
}

impl LifecycleWorkClass {
    pub(super) const fn retry_action(self) -> RetryAction {
        match self {
            Self::SignProposal | Self::SignVote | Self::SignTimeout => {
                RetryAction::StutterLiveSigner
            }
            Self::Fetch | Self::Store | Self::Validate | Self::CertifiedServe => {
                RetryAction::ReenqueueIncumbent
            }
            Self::Apply => RetryAction::RefanoutIncumbent,
            Self::Broadcast => RetryAction::RefanoutEnvelope,
            Self::EnterView => RetryAction::StutterInstalledView,
            Self::EquivocationReport | Self::InvalidBodyReport => RetryAction::StutterDiagnostic,
            Self::ProducerTurn => RetryAction::StutterProducerTurn,
        }
    }

    pub(super) const fn capacity_class(self) -> CapacityClass {
        match self {
            Self::SignProposal
            | Self::SignVote
            | Self::SignTimeout
            | Self::Fetch
            | Self::Store
            | Self::Validate
            | Self::Apply => CapacityClass::Effect,
            Self::Broadcast
            | Self::EnterView
            | Self::EquivocationReport
            | Self::InvalidBodyReport => CapacityClass::Consensus,
            Self::CertifiedServe => CapacityClass::Serve,
            Self::ProducerTurn => CapacityClass::Producer,
        }
    }

    /// Return whether this class, statement kind, and lifecycle stage form one
    /// of the closed production transitions.
    pub(crate) const fn accepts_phase_and_stage(
        self,
        phase: LifecyclePhase,
        stage: LifecycleStageKind,
    ) -> bool {
        match (self, phase, stage) {
            (Self::SignProposal, LifecyclePhase::Proposal, LifecycleStageKind::SignProposal)
            | (Self::SignVote, LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote)
            | (Self::SignVote, LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote)
            | (Self::SignTimeout, LifecyclePhase::Timeout, LifecycleStageKind::SignTimeoutVote)
            | (Self::Fetch, LifecyclePhase::Fetch, LifecycleStageKind::FetchBody)
            | (Self::Store, LifecyclePhase::Store, LifecycleStageKind::StoreBody)
            | (Self::Validate, LifecyclePhase::Validate, LifecycleStageKind::ValidateBody)
            | (Self::Apply, LifecyclePhase::Apply, LifecycleStageKind::ApplyDecision)
            | (
                Self::Broadcast,
                LifecyclePhase::BroadcastProposal,
                LifecycleStageKind::BroadcastProposal,
            )
            | (
                Self::Broadcast,
                LifecyclePhase::BroadcastPrepareVote,
                LifecycleStageKind::BroadcastPrepareVote,
            )
            | (
                Self::Broadcast,
                LifecyclePhase::BroadcastCommitVote,
                LifecycleStageKind::BroadcastCommitVote,
            )
            | (
                Self::Broadcast,
                LifecyclePhase::BroadcastPrepareQc,
                LifecycleStageKind::BroadcastPrepareQc,
            )
            | (
                Self::Broadcast,
                LifecyclePhase::BroadcastCommitQc,
                LifecycleStageKind::BroadcastCommitQc,
            )
            | (
                Self::Broadcast,
                LifecyclePhase::BroadcastTimeoutVote,
                LifecycleStageKind::BroadcastTimeoutVote,
            )
            | (Self::Broadcast, LifecyclePhase::BroadcastTc, LifecycleStageKind::BroadcastTc)
            | (Self::EnterView, LifecyclePhase::EnterView, LifecycleStageKind::EnterView)
            | (
                Self::EquivocationReport,
                LifecyclePhase::DiagnosticProposalEquivocation,
                LifecycleStageKind::ReportProposalEquivocation,
            )
            | (
                Self::EquivocationReport,
                LifecyclePhase::DiagnosticVoteEquivocation,
                LifecycleStageKind::ReportVoteEquivocation,
            )
            | (
                Self::EquivocationReport,
                LifecyclePhase::DiagnosticTimeoutEquivocation,
                LifecycleStageKind::ReportTimeoutEquivocation,
            )
            | (
                Self::InvalidBodyReport,
                LifecyclePhase::DiagnosticInvalidBody,
                LifecycleStageKind::ReportInvalidBody,
            )
            | (Self::CertifiedServe, LifecyclePhase::Serve, LifecycleStageKind::CertifiedServe)
            | (
                Self::ProducerTurn,
                LifecyclePhase::ProducerTurn,
                LifecycleStageKind::ProducerTurn,
            ) => true,
            _ => false,
        }
    }
}

/// Exact execution stage attached to a scheduler record.
///
/// This inventory names the operation that a lease executes and therefore
/// never conflates a source event with a future completion event. Reducer
/// provenance is not a scheduler-rank component; causal authority remains in
/// the sealed owner/effect binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LifecycleStageKind {
    SignProposal,
    SignPrepareVote,
    SignCommitVote,
    SignTimeoutVote,
    FetchBody,
    StoreBody,
    ValidateBody,
    ApplyDecision,
    BroadcastProposal,
    BroadcastPrepareVote,
    BroadcastCommitVote,
    BroadcastPrepareQc,
    BroadcastCommitQc,
    BroadcastTimeoutVote,
    BroadcastTc,
    EnterView,
    ReportProposalEquivocation,
    ReportVoteEquivocation,
    ReportTimeoutEquivocation,
    ReportInvalidBody,
    CertifiedServe,
    ProducerTurn,
}

impl LifecycleStageKind {
    pub(super) const ALL: [Self; 22] = [
        Self::SignProposal,
        Self::SignPrepareVote,
        Self::SignCommitVote,
        Self::SignTimeoutVote,
        Self::FetchBody,
        Self::StoreBody,
        Self::ValidateBody,
        Self::ApplyDecision,
        Self::BroadcastProposal,
        Self::BroadcastPrepareVote,
        Self::BroadcastCommitVote,
        Self::BroadcastPrepareQc,
        Self::BroadcastCommitQc,
        Self::BroadcastTimeoutVote,
        Self::BroadcastTc,
        Self::EnterView,
        Self::ReportProposalEquivocation,
        Self::ReportVoteEquivocation,
        Self::ReportTimeoutEquivocation,
        Self::ReportInvalidBody,
        Self::CertifiedServe,
        Self::ProducerTurn,
    ];

    /// Return the closed residual operation topology for this execution unit.
    ///
    /// Successors in the body, signing, and Serve pipelines have strictly
    /// smaller values. Terminal single-operation units have one remaining
    /// stage.
    pub(super) const fn remaining_stages(self) -> u64 {
        match self {
            Self::FetchBody => 4,
            Self::StoreBody => 3,
            Self::SignProposal
            | Self::SignPrepareVote
            | Self::SignCommitVote
            | Self::SignTimeoutVote
            | Self::ValidateBody
            | Self::CertifiedServe => 2,
            Self::ApplyDecision
            | Self::BroadcastProposal
            | Self::BroadcastPrepareVote
            | Self::BroadcastCommitVote
            | Self::BroadcastPrepareQc
            | Self::BroadcastCommitQc
            | Self::BroadcastTimeoutVote
            | Self::BroadcastTc
            | Self::EnterView
            | Self::ReportProposalEquivocation
            | Self::ReportVoteEquivocation
            | Self::ReportTimeoutEquivocation
            | Self::ReportInvalidBody
            | Self::ProducerTurn => 1,
        }
    }
}

/// The exact formal ingress rank in lexicographic field order.
///
/// Every component is a natural-number debt. In particular, `source` is the
/// source-service position from the formal runner, not reducer-event
/// provenance. Named fields prevent production adapters from silently
/// permuting the eight proof components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SchedulerRank {
    remaining_stages: u64,
    frozen_predecessors: u64,
    mode: u64,
    capacity: u64,
    selector: u64,
    lane: u64,
    source: u64,
    runner: u64,
}

impl SchedulerRank {
    /// Construct the exact eight-component scheduler rank.
    pub(super) const fn new(
        remaining_stages: u64,
        frozen_predecessors: u64,
        mode: u64,
        capacity: u64,
        selector: u64,
        lane: u64,
        source: u64,
        runner: u64,
    ) -> Self {
        Self {
            remaining_stages,
            frozen_predecessors,
            mode,
            capacity,
            selector,
            lane,
            source,
            runner,
        }
    }

    /// Return the exact eight rank components.
    pub(crate) const fn components(self) -> [u64; 8] {
        [
            self.remaining_stages,
            self.frozen_predecessors,
            self.mode,
            self.capacity,
            self.selector,
            self.lane,
            self.source,
            self.runner,
        ]
    }
}

/// Generic predecessor relation enforced before rank selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PredecessorScope {
    /// This record has no ordinal-prefix dependency.
    Independent,
    /// Exact ready ordinals frozen before this target precede the target.
    ReadyOrdinalPrefix,
    /// A ready producer handoff also prevents later ordinals from overtaking it.
    ProducerHandoffBarrier,
}

/// Exact immutable execution stage and predecessor policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LifecycleStage {
    pub(super) kind: LifecycleStageKind,
    pub(super) predecessor_scope: PredecessorScope,
}

impl LifecycleStage {
    /// Construct exact immutable stage metadata.
    pub(super) const fn new(kind: LifecycleStageKind, predecessor_scope: PredecessorScope) -> Self {
        Self {
            kind,
            predecessor_scope,
        }
    }

    /// Return the exact lifecycle stage kind.
    pub(crate) const fn kind(self) -> LifecycleStageKind {
        self.kind
    }

    /// Return the predecessor relation enforced by selection.
    pub(crate) const fn predecessor_scope(self) -> PredecessorScope {
        self.predecessor_scope
    }
}

impl LifecycleWorkClass {
    /// Return whether the statement, operation, and predecessor policy form
    /// one exact production work shape.
    pub(crate) const fn accepts_stage(self, phase: LifecyclePhase, stage: LifecycleStage) -> bool {
        self.accepts_phase_and_stage(phase, stage.kind)
            && match self {
                Self::CertifiedServe => {
                    matches!(
                        stage.predecessor_scope,
                        PredecessorScope::ReadyOrdinalPrefix
                    )
                }
                Self::ProducerTurn => matches!(
                    stage.predecessor_scope,
                    PredecessorScope::ProducerHandoffBarrier
                ),
                _ => matches!(stage.predecessor_scope, PredecessorScope::Independent),
            }
    }
}

/// Address of one finite physical replenishment slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PhysicalSlotId(pub(super) u16, pub(super) u16);

impl PhysicalSlotId {
    /// Construct a slot address within the frozen capacity geometry.
    pub(super) const fn new(class: u16, index: u16) -> Self {
        Self(class, index)
    }

    /// Construct a slot address for a typed lifecycle capacity class.
    pub(super) const fn for_capacity(class: CapacityClass, index: u16) -> Self {
        let class = match class {
            CapacityClass::Consensus => 0,
            CapacityClass::Effect => 1,
            CapacityClass::Serve => 2,
            CapacityClass::Producer => 3,
        };
        Self(class, index)
    }

    /// Return the typed capacity class encoded by this address.
    pub(crate) const fn capacity_class(self) -> Option<CapacityClass> {
        match self.0 {
            0 => Some(CapacityClass::Consensus),
            1 => Some(CapacityClass::Effect),
            2 => Some(CapacityClass::Serve),
            3 => Some(CapacityClass::Producer),
            _ => None,
        }
    }

    /// Return the finite slot index within its capacity class.
    pub(crate) const fn index(self) -> u16 {
        self.1
    }
}

/// Authenticated physical slot projection retained by the coordinator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalSlot {
    pub(super) id: PhysicalSlotId,
    pub(super) digest: LifecycleDigest,
}

impl PhysicalSlot {
    /// Construct an authenticated slot projection.
    pub(super) const fn new(id: PhysicalSlotId, digest: LifecycleDigest) -> Self {
        Self { id, digest }
    }

    /// Return the finite slot address.
    pub(crate) const fn id(self) -> PhysicalSlotId {
        self.id
    }

    /// Return the authenticated physical-work digest.
    pub(crate) const fn digest(self) -> LifecycleDigest {
        self.digest
    }
}

/// Owner universe frozen for one finite rank-preserving scheduler episode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SchedulerEpisodeUniverse {
    pub(super) target: LifecycleDigest,
    pub(super) context: LifecycleDigest,
    pub(super) leader: LifecycleDigest,
    pub(super) view: u64,
    pub(super) subject: Option<LifecycleDigest>,
    pub(super) phase: LifecyclePhase,
    pub(super) authenticated_roster_slots: BTreeSet<u16>,
    pub(super) capacity_geometry: BTreeMap<CapacityClass, usize>,
}

/// Initial and finite replenishment geometry for a lifecycle record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalGeometry {
    pub(super) initial: Vec<PhysicalSlot>,
    pub(super) replenishment_slots: BTreeSet<PhysicalSlotId>,
}

impl PhysicalGeometry {
    /// Construct geometry validated only after semantic ownership.
    pub(super) fn new(
        initial: impl IntoIterator<Item = PhysicalSlot>,
        replenishment_slots: impl IntoIterator<Item = PhysicalSlotId>,
    ) -> Self {
        Self {
            initial: initial.into_iter().collect(),
            replenishment_slots: replenishment_slots.into_iter().collect(),
        }
    }

    pub(super) fn canonicalized(&self) -> Result<Self, AdmissionRejection> {
        let mut carriers = BTreeMap::new();
        for slot in &self.initial {
            if carriers
                .insert(slot.id, slot.digest)
                .is_some_and(|existing| existing != slot.digest)
            {
                return Err(AdmissionRejection::InvalidPhysicalGeometry);
            }
        }
        Ok(Self::new(
            carriers
                .into_iter()
                .map(|(id, digest)| PhysicalSlot::new(id, digest)),
            self.replenishment_slots.iter().copied(),
        ))
    }

    pub(super) fn normalized(
        &self,
    ) -> Result<
        (
            BTreeMap<PhysicalSlotId, LifecycleDigest>,
            BTreeSet<PhysicalSlotId>,
            BTreeSet<PhysicalSlotId>,
        ),
        AdmissionRejection,
    > {
        let mut carriers = BTreeMap::new();
        for slot in &self.initial {
            if carriers
                .insert(slot.id, slot.digest)
                .is_some_and(|existing| existing != slot.digest)
            {
                return Err(AdmissionRejection::InvalidPhysicalGeometry);
            }
        }
        let mut physical = BTreeMap::new();
        let mut digests = BTreeSet::new();
        for (slot, digest) in &carriers {
            if digests.insert(*digest) {
                physical.insert(*slot, *digest);
            }
        }
        let consumed: BTreeSet<_> = carriers.keys().copied().collect();
        let mut universe = self.replenishment_slots.clone();
        universe.extend(consumed.iter().copied());
        if universe.len() > MAX_PHYSICAL_SLOTS_PER_RECORD {
            return Err(AdmissionRejection::InvalidPhysicalGeometry);
        }
        Ok((physical, universe, consumed))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SchedulerEpisode {
    pub(super) universe: SchedulerEpisodeUniverse,
    pub(super) slot_universe: BTreeSet<PhysicalSlotId>,
    pub(super) consumed_slots: BTreeSet<PhysicalSlotId>,
    pub(super) frozen_predecessors: BTreeSet<u128>,
}

/// Explicit source and generation on which a record is waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WaitSource {
    Capacity(CapacityClass),
    External(LifecycleDigest),
    Recovery(LifecycleDigest),
    ProducerTurn(u128),
}

/// Exact observed generation for a blocked lifecycle record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct WaitToken {
    pub(super) source: WaitSource,
    pub(super) observed_generation: u64,
}

impl WaitToken {
    /// Construct a generation-fenced wait token.
    pub(super) const fn new(source: WaitSource, observed_generation: u64) -> Self {
        Self {
            source,
            observed_generation,
        }
    }

    /// Return the exact wait source.
    pub(crate) const fn source(self) -> WaitSource {
        self.source
    }

    /// Return the observed generation fence.
    pub(crate) const fn observed_generation(self) -> u64 {
        self.observed_generation
    }
}

/// Stable terminal result retained as a tombstone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalOutcome {
    Advanced,
    Completed(Option<LifecycleDigest>),
    Cancelled,
    Rejected(u16),
    Failed(u16),
}

/// Immutable identifier of the coordinator's one active turn lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LeaseId(pub(super) u128);

/// The complete logical state machine for a lifecycle record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleState {
    Waiting(WaitToken),
    Ready,
    Claimed(LeaseId),
    Terminal(TerminalOutcome),
}

/// One canonical logical scheduler record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LifecycleRecord {
    pub(super) key: LifecycleKey,
    pub(super) owner: OwnerId,
    pub(super) ordinal: u128,
    pub(super) work_class: LifecycleWorkClass,
    pub(super) stage: LifecycleStage,
    pub(super) state: LifecycleState,
    pub(super) physical_slots: BTreeMap<PhysicalSlotId, LifecycleDigest>,
    pub(super) episode: SchedulerEpisode,
}

/// Restart-stable payload material owned by one durable lifecycle record.
///
/// Certified Serve uses a domain-separated six-field-key subject over the
/// block subject and exact signed-request hash. Its variants retain that exact
/// request receipt, the authorizing certificate digest, and the typed terminal
/// payload-store receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DurablePayloadReference {
    None,
    CertifiedServePending {
        request: LifecycleDigest,
        certificate: LifecycleDigest,
    },
    CertifiedServeCompleted {
        request: LifecycleDigest,
        certificate: LifecycleDigest,
        response: LifecycleDigest,
    },
    CertifiedServeNegative {
        request: LifecycleDigest,
        certificate: LifecycleDigest,
        outcome: DurableServeNegativeOutcome,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DurableServeNegativeOutcome {
    Cancelled,
    Rejected(u16),
    Failed(u16),
}

impl DurableServeNegativeOutcome {
    pub(super) const fn from_terminal(outcome: TerminalOutcome) -> Option<Self> {
        match outcome {
            TerminalOutcome::Cancelled => Some(Self::Cancelled),
            TerminalOutcome::Rejected(code) => Some(Self::Rejected(code)),
            TerminalOutcome::Failed(code) => Some(Self::Failed(code)),
            TerminalOutcome::Advanced | TerminalOutcome::Completed(_) => None,
        }
    }

    pub(super) const fn terminal(self) -> TerminalOutcome {
        match self {
            Self::Cancelled => TerminalOutcome::Cancelled,
            Self::Rejected(code) => TerminalOutcome::Rejected(code),
            Self::Failed(code) => TerminalOutcome::Failed(code),
        }
    }
}

impl DurablePayloadReference {
    pub(super) const fn certified_serve_pending(
        request: LifecycleDigest,
        certificate: LifecycleDigest,
    ) -> Self {
        Self::CertifiedServePending {
            request,
            certificate,
        }
    }

    pub(super) const fn is_certified_serve(self) -> bool {
        !matches!(self, Self::None)
    }

    pub(super) const fn certificate(self) -> Option<LifecycleDigest> {
        match self {
            Self::None => None,
            Self::CertifiedServePending { certificate, .. }
            | Self::CertifiedServeCompleted { certificate, .. }
            | Self::CertifiedServeNegative { certificate, .. } => Some(certificate),
        }
    }

    pub(super) const fn request(self) -> Option<LifecycleDigest> {
        match self {
            Self::None => None,
            Self::CertifiedServePending { request, .. }
            | Self::CertifiedServeCompleted { request, .. }
            | Self::CertifiedServeNegative { request, .. } => Some(request),
        }
    }

    pub(super) fn same_admission_material(self, other: Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (left, right) => match (
                left.request(),
                left.certificate(),
                right.request(),
                right.certificate(),
            ) {
                (
                    Some(left_request),
                    Some(left_certificate),
                    Some(right_request),
                    Some(right_certificate),
                ) => left_request == right_request && left_certificate == right_certificate,
                _ => false,
            },
        }
    }

    pub(super) const fn terminalized(self, outcome: TerminalOutcome) -> Option<Self> {
        let Self::CertifiedServePending {
            request,
            certificate,
        } = self
        else {
            return Some(self);
        };
        match outcome {
            TerminalOutcome::Completed(Some(response)) => Some(Self::CertifiedServeCompleted {
                request,
                certificate,
                response,
            }),
            TerminalOutcome::Cancelled => Some(Self::CertifiedServeNegative {
                request,
                certificate,
                outcome: DurableServeNegativeOutcome::Cancelled,
            }),
            TerminalOutcome::Rejected(code) => Some(Self::CertifiedServeNegative {
                request,
                certificate,
                outcome: DurableServeNegativeOutcome::Rejected(code),
            }),
            TerminalOutcome::Failed(code) => Some(Self::CertifiedServeNegative {
                request,
                certificate,
                outcome: DurableServeNegativeOutcome::Failed(code),
            }),
            TerminalOutcome::Advanced | TerminalOutcome::Completed(None) => None,
        }
    }

    pub(super) fn matches_terminal(
        self,
        work_class: LifecycleWorkClass,
        terminal: Option<TerminalOutcome>,
    ) -> bool {
        match (work_class, self, terminal) {
            (LifecycleWorkClass::CertifiedServe, Self::CertifiedServePending { .. }, None) => true,
            (
                LifecycleWorkClass::CertifiedServe,
                Self::CertifiedServeCompleted { response, .. },
                Some(TerminalOutcome::Completed(Some(terminal_response))),
            ) => response == terminal_response,
            (
                LifecycleWorkClass::CertifiedServe,
                Self::CertifiedServeNegative { outcome, .. },
                Some(terminal),
            ) => outcome.terminal() == terminal,
            (class, Self::None, _) => class != LifecycleWorkClass::CertifiedServe,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DurableRecordMetadata {
    pub(super) reconstruction_source: LifecycleDigest,
    pub(super) payload: DurablePayloadReference,
}

impl DurableRecordMetadata {
    pub(super) fn matches_admission(&self, candidate: &CandidateAdmission) -> bool {
        self.reconstruction_source == candidate.reconstruction_source
            && self.payload.same_admission_material(candidate.payload)
    }
}

/// Initial readiness of a newly admitted record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InitialLifecycleState {
    /// The record is immediately ready.
    Ready,
    /// The record waits on an exact generation.
    Waiting(WaitToken),
}

/// Reserved producer-turn record admitted atomically with Certified Serve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProducerTurnAdmission {
    pub(super) key: LifecycleKey,
    pub(super) stage: LifecycleStage,
    pub(super) reconstruction_source: LifecycleDigest,
    pub(super) physical_geometry: PhysicalGeometry,
}

impl ProducerTurnAdmission {
    /// Construct dormant producer-turn admission data.
    pub(super) fn new(
        key: LifecycleKey,
        stage: LifecycleStage,
        reconstruction_source: LifecycleDigest,
        physical_geometry: PhysicalGeometry,
    ) -> Self {
        Self {
            key,
            stage,
            reconstruction_source,
            physical_geometry,
        }
    }
}

/// Candidate data inspected only after semantic ownership checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateAdmission {
    pub(super) key: LifecycleKey,
    pub(super) causal_root: CausalRoot,
    pub(super) work_class: LifecycleWorkClass,
    pub(super) stage: LifecycleStage,
    pub(super) initial_state: InitialLifecycleState,
    pub(super) reconstruction_source: LifecycleDigest,
    pub(super) payload: DurablePayloadReference,
    pub(super) physical_geometry: PhysicalGeometry,
    pub(super) producer_turn: Option<ProducerTurnAdmission>,
}

impl CandidateAdmission {
    /// Construct a logical candidate admission.
    pub(super) fn new(
        key: LifecycleKey,
        causal_root: CausalRoot,
        work_class: LifecycleWorkClass,
        stage: LifecycleStage,
        initial_state: InitialLifecycleState,
        reconstruction_source: LifecycleDigest,
        payload: DurablePayloadReference,
        physical_geometry: PhysicalGeometry,
        producer_turn: Option<ProducerTurnAdmission>,
    ) -> Self {
        Self {
            key,
            causal_root,
            work_class,
            stage,
            initial_state,
            reconstruction_source,
            payload,
            physical_geometry,
            producer_turn,
        }
    }

    pub(super) fn canonicalize_geometry(&mut self) -> Result<(), AdmissionRejection> {
        self.physical_geometry = self.physical_geometry.canonicalized()?;
        if let Some(producer) = self.producer_turn.as_mut() {
            producer.physical_geometry = producer.physical_geometry.canonicalized()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CapacityAdmissionWait {
    pub(super) candidate: CandidateAdmission,
    pub(super) wait_token: WaitToken,
    pub(super) serve_payload_receipt: Option<
        crate::sumeragi::v2_certified_serve_payload_store::DurableCertifiedServeAdmissionReceipt,
    >,
}

/// Admission input distinguishing liveness candidates from ordinary effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AdmissionRequest {
    /// A lifecycle candidate subject to exact ownership and capacity checks.
    Candidate(CandidateAdmission),
    /// A sealed zero-owner effect minted only by the exhaustive classifier.
    NonCandidate(NonCandidateEffect),
}

/// Sealed proof that an input has no adapter-effect lifecycle class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NonCandidateEffect(pub(super) ());

/// Typed reason why an admission failed before exposure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdmissionRejection {
    ForeignOwner,
    SemanticDrift,
    InvalidWorkShape,
    InvalidDurableMetadata,
    MissingProducerTurn,
    UnexpectedProducerTurn,
    InvalidProducerTurn,
    InvalidInitialState,
    InvalidPhysicalGeometry,
    InvalidEpisodeUniverse,
    AdmissionQueueFull,
    EnterViewConflict,
    ForeignContext,
    OrdinalExhausted,
}

/// Complete deterministic result of one admission attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdmissionDecision {
    Admitted {
        owner: OwnerId,
        ordinal: u128,
        producer_turn_ordinal: Option<u128>,
    },
    Retry {
        owner: OwnerId,
        ordinal: u128,
        action: RetryAction,
    },
    ReplayTerminal {
        owner: OwnerId,
        outcome: TerminalOutcome,
    },
    StutterTerminal {
        owner: OwnerId,
    },
    NonCandidate,
    WaitForCapacity(WaitToken),
    Rejected(AdmissionRejection),
    FailClosed(CoordinatorFault),
}

/// Optional equal-count physical replacement accompanying readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalReplacement {
    pub(super) existing_slot: PhysicalSlotId,
    pub(super) replacement: PhysicalSlot,
}

impl PhysicalReplacement {
    /// Construct an equal-address physical replacement.
    pub(super) const fn new(existing_slot: PhysicalSlotId, replacement: PhysicalSlot) -> Self {
        Self {
            existing_slot,
            replacement,
        }
    }
}

/// Authenticated event publishing a late completion as ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReadyEvent {
    pub(super) ordinal: u128,
    pub(super) owner: OwnerId,
    pub(super) wait_token: WaitToken,
    pub(super) replacement: Option<PhysicalReplacement>,
}

impl ReadyEvent {
    /// Construct an exact readiness publication.
    pub(super) const fn new(
        ordinal: u128,
        owner: OwnerId,
        wait_token: WaitToken,
        replacement: Option<PhysicalReplacement>,
    ) -> Self {
        Self {
            ordinal,
            owner,
            wait_token,
            replacement,
        }
    }
}

/// One identity-bound row of live runtime rank debts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SchedulerReadyInputs {
    owner: OwnerId,
    key: LifecycleKey,
    mode: u64,
    capacity: u64,
    selector: u64,
    lane: u64,
    source: u64,
    runner: u64,
}

impl SchedulerReadyInputs {
    /// Construct one test row without exposing a production rank mint.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        owner: OwnerId,
        key: LifecycleKey,
        mode: u64,
        capacity: u64,
        selector: u64,
        lane: u64,
        source: u64,
        runner: u64,
    ) -> Self {
        Self {
            owner,
            key,
            mode,
            capacity,
            selector,
            lane,
            source,
            runner,
        }
    }

    /// Return whether this row names the coordinator's exact ready identity.
    pub(super) fn identity_matches(&self, owner: OwnerId, key: LifecycleKey) -> bool {
        self.owner == owner && self.key == key
    }

    /// Return the six live debts in their mandated rank order.
    pub(super) const fn live_debts(&self) -> [u64; 6] {
        [
            self.mode,
            self.capacity,
            self.selector,
            self.lane,
            self.source,
            self.runner,
        ]
    }
}

/// Authenticated generation and ready-row snapshot supplied to one planning turn.
// TODO: Add the move-only coordinator demand plus composite runtime attestation
// factory before production selection is wired; the raw mint remains test-only.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SchedulerInputs {
    generations: BTreeMap<WaitSource, u64>,
    ready: BTreeMap<u128, SchedulerReadyInputs>,
}

impl SchedulerInputs {
    /// Construct one unique test snapshot without exposing a production mint.
    #[cfg(test)]
    pub(super) fn new(
        generations: impl IntoIterator<Item = (WaitSource, u64)>,
        ready: impl IntoIterator<Item = (u128, SchedulerReadyInputs)>,
    ) -> Result<Self, SchedulerInputError> {
        let mut unique_generations = BTreeMap::new();
        for (source, generation) in generations {
            if !matches!(source, WaitSource::External(_) | WaitSource::Recovery(_)) {
                return Err(SchedulerInputError::UnsupportedGenerationSource);
            }
            if unique_generations.insert(source, generation).is_some() {
                return Err(SchedulerInputError::DuplicateGenerationSource);
            }
        }
        let mut unique_ready = BTreeMap::new();
        for (ordinal, row) in ready {
            if unique_ready.insert(ordinal, row).is_some() {
                return Err(SchedulerInputError::DuplicateReadyOrdinal);
            }
        }
        Ok(Self {
            generations: unique_generations,
            ready: unique_ready,
        })
    }

    /// Consume the move-only snapshot into its two validated maps.
    pub(super) fn into_parts(
        self,
    ) -> (
        BTreeMap<WaitSource, u64>,
        BTreeMap<u128, SchedulerReadyInputs>,
    ) {
        (self.generations, self.ready)
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Failure while minting a raw test-only scheduler snapshot.
pub(super) enum SchedulerInputError {
    /// The test input repeated one external/recovery source.
    DuplicateGenerationSource,
    /// The test input repeated one ready lifecycle ordinal.
    DuplicateReadyOrdinal,
    /// The test input tried to advance a coordinator-local generation.
    UnsupportedGenerationSource,
}

/// One claimed lifecycle turn, executed without the coordinator lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TurnLease {
    pub(super) id: LeaseId,
    pub(super) ordinal: u128,
    pub(super) owner: OwnerId,
    pub(super) key: LifecycleKey,
    pub(super) work_class: LifecycleWorkClass,
    pub(super) stage: LifecycleStage,
    pub(super) rank: SchedulerRank,
    pub(super) physical_slots: BTreeMap<PhysicalSlotId, LifecycleDigest>,
}

impl TurnLease {
    /// Return the unique lease identifier.
    pub(crate) const fn id(&self) -> LeaseId {
        self.id
    }

    /// Return the immutable lifecycle ordinal.
    pub(crate) const fn ordinal(&self) -> u128 {
        self.ordinal
    }

    /// Return the immutable lifecycle owner.
    pub(crate) const fn owner(&self) -> OwnerId {
        self.owner
    }

    /// Return the semantic lifecycle key.
    pub(crate) const fn key(&self) -> LifecycleKey {
        self.key
    }

    /// Return the exhaustive work class.
    pub(crate) const fn work_class(&self) -> LifecycleWorkClass {
        self.work_class
    }

    /// Return the exact immutable execution stage.
    pub(crate) const fn stage(&self) -> LifecycleStage {
        self.stage
    }

    /// Return the exact rank snapshot used to select this lease.
    pub(crate) const fn rank(&self) -> SchedulerRank {
        self.rank
    }

    /// Borrow the coalesced physical work projections.
    pub(crate) const fn physical_slots(&self) -> &BTreeMap<PhysicalSlotId, LifecycleDigest> {
        &self.physical_slots
    }
}

/// Exactly one result reported for an executed turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TurnOutcome {
    Advanced,
    Terminal(TerminalOutcome),
    Blocked(WaitToken),
    Replenished(PhysicalSlot),
}

/// Deterministic result of one scheduler planning call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TurnPlan {
    Execute(TurnLease),
    Waiting(BTreeSet<WaitToken>),
    Idle,
    FailClosed(CoordinatorFault),
}

/// Typed fail-closed coordinator condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CoordinatorFault {
    UnsettledLease(LeaseId),
    StaleLease,
    InvalidSchedulerInputs,
    InvalidReadyEvent,
    InvalidPhysicalTransition,
    InvalidTerminalOutcome,
    DurabilityFailure,
    CapacityAccounting,
    LeaseExhausted,
    RecoveryRejected,
    InvalidRollover,
}

/// Fixed capacity limits owned by one coordinator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CapacityGeometry {
    pub(super) limits: BTreeMap<CapacityClass, usize>,
}

impl CapacityGeometry {
    /// Construct complete capacity geometry; omitted classes have zero slots.
    pub(super) fn new(limits: impl IntoIterator<Item = (CapacityClass, usize)>) -> Self {
        let supplied: BTreeMap<_, _> = limits.into_iter().collect();
        let limits = CapacityClass::ALL
            .into_iter()
            .map(|class| (class, supplied.get(&class).copied().unwrap_or(0)))
            .collect();
        Self { limits }
    }

    /// Return the limit for one typed capacity class.
    pub(super) fn limit(&self, class: CapacityClass) -> usize {
        self.limits.get(&class).copied().unwrap_or(0)
    }
}

/// Restart-stable record material without transient scheduling state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecoveredLifecycleRecord {
    pub(super) key: LifecycleKey,
    pub(super) owner: OwnerId,
    pub(super) ordinal: u128,
    pub(super) work_class: LifecycleWorkClass,
    pub(super) stage: LifecycleStage,
    pub(super) terminal: Option<TerminalOutcome>,
    pub(super) reconstruction_source: LifecycleDigest,
    pub(super) payload: DurablePayloadReference,
    pub(super) physical_slot_universe: BTreeSet<PhysicalSlotId>,
}

impl RecoveredLifecycleRecord {
    /// Construct one authenticated record after storage reconciliation.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        key: LifecycleKey,
        owner: OwnerId,
        ordinal: u128,
        work_class: LifecycleWorkClass,
        stage: LifecycleStage,
        terminal: Option<TerminalOutcome>,
        reconstruction_source: LifecycleDigest,
        payload: DurablePayloadReference,
        physical_slot_universe: BTreeSet<PhysicalSlotId>,
    ) -> Self {
        Self {
            key,
            owner,
            ordinal,
            work_class,
            stage,
            terminal,
            reconstruction_source,
            payload,
            physical_slot_universe,
        }
    }
}

/// Internal recovery snapshot used to rebuild volatile coordinator state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecoverySnapshot {
    pub(super) context: LifecycleContext,
    pub(super) high_water: u128,
    pub(super) records: Vec<RecoveredLifecycleRecord>,
    pub(super) producer_debts: BTreeMap<u128, u128>,
}

impl RecoverySnapshot {
    /// Construct a post-storage-reconciliation restart snapshot.
    pub(super) fn new(
        context: LifecycleContext,
        high_water: u128,
        records: Vec<RecoveredLifecycleRecord>,
        producer_debts: BTreeMap<u128, u128>,
    ) -> Self {
        Self {
            context,
            high_water,
            records,
            producer_debts,
        }
    }
}

pub(super) fn first_capacity_wait(
    capacity_used: &BTreeMap<CapacityClass, usize>,
    capacity_geometry: &CapacityGeometry,
    capacity_generation: &BTreeMap<CapacityClass, u64>,
    delta: &BTreeMap<CapacityClass, usize>,
) -> Option<WaitToken> {
    delta.iter().find_map(|(class, needed)| {
        let used = capacity_used.get(class).copied().unwrap_or(0);
        let limit = capacity_geometry.limit(*class);
        (used.checked_add(*needed).is_none_or(|total| total > limit)).then(|| {
            WaitToken::new(
                WaitSource::Capacity(*class),
                capacity_generation.get(class).copied().unwrap_or(0),
            )
        })
    })
}

pub(super) fn frozen_predecessors(
    records: &BTreeMap<u128, LifecycleRecord>,
    scope: PredecessorScope,
    ordinal: u128,
) -> BTreeSet<u128> {
    if matches!(scope, PredecessorScope::Independent) {
        return BTreeSet::new();
    }
    records
        .range(..ordinal)
        .filter_map(|(predecessor, record)| {
            (!matches!(record.state, LifecycleState::Terminal(_))).then_some(*predecessor)
        })
        .collect()
}

pub(super) fn lower_enter_view_ordinals(
    records: &BTreeMap<u128, LifecycleRecord>,
    installed: LifecycleKey,
) -> Vec<u128> {
    records
        .values()
        .filter(|record| {
            record.work_class == LifecycleWorkClass::EnterView
                && record.key.context == installed.context
                && record.key.round.height == installed.round.height
                && record.key.round.view < installed.round.view
                && !matches!(record.state, LifecycleState::Terminal(_))
        })
        .map(|record| record.ordinal)
        .collect()
}
