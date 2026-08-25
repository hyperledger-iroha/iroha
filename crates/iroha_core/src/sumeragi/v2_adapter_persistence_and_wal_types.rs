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
            // A quorum-forming signed TimeoutVote is subsumed by its TC and
            // emits only Persist; Persisted can emit EnterView, fetch, the TC
            // broadcast, and Sign.
            Self::InstallTimeout => PersistenceMacroStepBudget::new(1, 4),
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
/// Opaque authenticated frontier for restoring validation-marker authority.
///
/// Construction is restricted to a fully replayed adapter, so a checksummed
/// body-store marker cannot select itself for recovery. The bounded key set is
/// derived only from the durable lock/decision and the adapter's first replay
/// batch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecoveredValidationAuthority {
    context_id: wire::HeightContextId,
    height: wire::Height,
    keys: BTreeSet<(wire::ConsensusRound, wire::BlockSubject)>,
}
impl RecoveredValidationAuthority {
    /// Whether this capability belongs to the exact immutable height context.
    pub(crate) fn authorizes_context(&self, context: &wire::HeightContext) -> bool {
        self.context_id == context.id() && self.height == context.height
    }
    /// Whether one exact proposal origin belongs to the authenticated frontier.
    pub(crate) fn authorizes(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        self.keys.contains(&(round, subject))
    }
    /// Number of exact identities in the bounded replay frontier.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }
    /// Construct a bounded exact frontier for body-store seam tests.
    #[cfg(test)]
    pub(crate) fn for_test(
        context: &wire::HeightContext,
        keys: impl IntoIterator<Item = (wire::ConsensusRound, wire::BlockSubject)>,
    ) -> Self {
        let keys = keys.into_iter().collect::<BTreeSet<_>>();
        assert!(keys.len() <= MAX_RECOVERED_VALIDATION_AUTHORITIES);
        assert!(keys.iter().all(|(round, _)| {
            round.context_id == context.id() && round.height == context.height
        }));
        Self {
            context_id: context.id(),
            height: context.height,
            keys,
        }
    }
}
// RECOVERED_WAL_VOTE_SIGN_SEAL_BEGIN
/// Opaque identity of one exact verified and fsynced safety-WAL frame.
///
/// The scalar parts never leave this module as an authority-bearing tuple.
/// Sibling recovery stages may only retain the complete value and ask whether
/// it is internally exact or equal to another sealed identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RecoveredWalFrameIdentity {
    frame_sequence: u64,
    persistence_id: u64,
    frame_hash: [u8; 32],
}
/// Opaque identity minted only from one successful live WAL append.
///
/// Unlike recovered startup authority, this move-only seal requires the exact
/// fsync receipt and the retained in-memory frame produced by that append. It
/// exposes no sequence, persistence identifier, frame hash, or recovered-vote
/// conversion.
#[derive(PartialEq, Eq)]
pub(super) struct LiveWalFrameIdentity {
    frame_sequence: u64,
    persistence_id: u64,
    frame_hash: [u8; 32],
    _linearity: LiveWalFrameLinearity,
}
#[derive(PartialEq, Eq)]
struct LiveWalFrameLinearity;
impl Drop for LiveWalFrameLinearity {
    fn drop(&mut self) {}
}
impl LiveWalFrameIdentity {
    fn from_append_receipt(
        record: &RecoveredRecord,
        receipt: SafetyWalAppendReceipt,
        persistence_id: u64,
    ) -> Option<Self> {
        if !record.exactly_matches_receipt(receipt) {
            return None;
        }
        let identity = Self {
            frame_sequence: record.sequence(),
            persistence_id,
            frame_hash: record.frame_hash(),
            _linearity: LiveWalFrameLinearity,
        };
        identity.is_exact().then_some(identity)
    }
    /// Return whether the live append has the canonical reducer relation.
    pub(super) const fn is_exact(&self) -> bool {
        match self.frame_sequence.checked_add(1) {
            Some(next) => next == self.persistence_id,
            None => false,
        }
    }
    /// Project inert codec evidence without exposing locator scalar parts.
    pub(super) const fn persisted_locator(&self) -> PersistedWalFrameLocatorV1 {
        PersistedWalFrameLocatorV1 {
            frame_sequence: self.frame_sequence,
            persistence_id: self.persistence_id,
            frame_hash: self.frame_hash,
        }
    }
    /// Construct a move-only live identity for focused authority tests.
    #[cfg(test)]
    pub(super) fn for_test(frame_sequence: u64, persistence_id: u64, frame_hash: [u8; 32]) -> Self {
        Self {
            frame_sequence,
            persistence_id,
            frame_hash,
            _linearity: LiveWalFrameLinearity,
        }
    }
}
