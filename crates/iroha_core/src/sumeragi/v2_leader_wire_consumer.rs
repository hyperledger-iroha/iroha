//! Admission and replay follow the actual WAL-backed reducer consumer.
use super::{AdapterError, SumeragiV2Adapter, reducer, wire};
use crate::sumeragi::{
    FairV2IngressLeaderWireIdentity, FairV2IngressLeaderWirePhase as Phase,
    FairV2IngressLeaderWireSourceClass, FairV2IngressLeaderWireToken,
};
use iroha_crypto::Hash;
use norito::codec::Encode;

/// Process-local capability minted exclusively from the open, replayed adapter.
/// No executor or service can invent a consensus frontier from a view number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LeaderWireRecoveryAuthority {
    context_id: wire::HeightContextId,
    height: wire::Height,
    owner: [u8; 32],
    consumer_tag: reducer::EventTag,
    wal_id: reducer::PersistenceId,
    decision_durable: bool,
    highest_prepare_view: Option<wire::View>,
    installed_timeout_view: Option<wire::View>,
    protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    protected_commit_statement: Option<Hash>,
}

/// Exact non-owning coordinates copied only from an authenticated envelope.
/// The runtime binds these coordinates to that envelope's immutable occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LeaderWireConsumerPosition {
    context_id: wire::HeightContextId,
    height: wire::Height,
    phase: Phase,
    view: wire::View,
    commit_statement: Option<Hash>,
    timeout_prepare_view: Option<wire::View>,
}
impl LeaderWireConsumerPosition {
    pub(crate) fn from_payload(payload: &wire::ConsensusMessageV2Payload) -> Option<Self> {
        use wire::ConsensusMessageV2Payload as Payload;
        let (round, phase, commit_statement, timeout_prepare_view) = match payload {
            Payload::Proposal(proposal) => (proposal.round, Phase::Proposal, None, None),
            Payload::Vote(vote) => (
                vote.round,
                match vote.phase {
                    wire::GlobalPhase::Prepare => Phase::PrepareVote,
                    wire::GlobalPhase::Commit => Phase::CommitVote,
                },
                (vote.round == vote.proposal_round).then(|| {
                    vote_statement_hash(
                        vote.proposal_round,
                        vote.subject,
                        &vote.execution_commitment,
                    )
                }),
                None,
            ),
            Payload::QuorumCertificate(qc) => (
                qc.round,
                match qc.phase {
                    wire::GlobalPhase::Prepare => Phase::PrepareQc,
                    wire::GlobalPhase::Commit => Phase::CommitQc,
                },
                None,
                None,
            ),
            Payload::TimeoutVote(vote) => (vote.round, Phase::TimeoutVote, None, None),
            Payload::TimeoutCertificate(tc) => (
                tc.round,
                Phase::TimeoutCertificate,
                None,
                tc.highest_prepare_qc().map(|qc| qc.round.view),
            ),
            _ => return None,
        };
        Some(Self {
            context_id: round.context_id,
            height: round.height,
            phase,
            view: round.view,
            commit_statement,
            timeout_prepare_view,
        })
    }
    pub(crate) fn projection_hash(self) -> Hash {
        Hash::new(
            (
                self.context_id,
                self.height,
                self.phase,
                self.view,
                self.commit_statement,
                self.timeout_prepare_view,
            )
                .encode(),
        )
    }
}

pub(crate) fn vote_statement_hash(
    proposal_round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    execution_commitment: &wire::ExecutionCommitment,
) -> Hash {
    Hash::new((proposal_round, subject, *execution_commitment).encode())
}

impl LeaderWireRecoveryAuthority {
    pub(super) fn from_adapter(adapter: &SumeragiV2Adapter) -> Result<Self, AdapterError> {
        adapter.ensure_ingress()?;
        let durable = adapter.reducer.durable_state();
        let tag = adapter.reducer.current_tag();
        let protected_lock = durable
            .locked()
            .map(|certificate| {
                Ok::<_, AdapterError>((
                    adapter.registry.round_to_wire(certificate.proposal_round()),
                    adapter.registry.subject(certificate.subject())?,
                ))
            })
            .transpose()?;
        // An observer may collect current-round shares. Historical shares
        // reconstruct a pool only when replay retains the exact CommitIntent.
        let protected_commit_statement = durable
            .locked()
            .filter(|locked| {
                locked.round().view() == tag.view()
                    || durable.commit_intent_for_lock(locked).is_some()
            })
            .map(|locked| {
                Ok::<_, AdapterError>(vote_statement_hash(
                    adapter.registry.round_to_wire(locked.proposal_round()),
                    adapter.registry.subject(locked.subject())?,
                    &adapter
                        .registry
                        .execution_commitment(locked.round(), locked.subject())?,
                ))
            })
            .transpose()?;
        Ok(Self {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            owner: adapter.fingerprints.node.into(),
            consumer_tag: tag,
            wal_id: durable.last_id(),
            decision_durable: durable.decision().is_some(),
            highest_prepare_view: durable.highest_prepare().map(|qc| qc.round().view()),
            installed_timeout_view: durable.last_timeout().map(|tc| tc.round().view()),
            protected_lock,
            protected_commit_statement,
        })
    }
    pub(crate) fn matches_geometry(
        self,
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
    ) -> bool {
        self.context_id == context_id && self.height == height && self.owner == owner
    }
    pub(crate) fn monotonically_extends(self, previous: Self) -> bool {
        self.matches_geometry(previous.context_id, previous.height, previous.owner)
            && self.wal_id >= previous.wal_id
            && (self.consumer_tag == previous.consumer_tag
                || self.consumer_tag.strictly_advances(previous.consumer_tag))
            && (!previous.decision_durable || self.decision_durable)
            && self.highest_prepare_view >= previous.highest_prepare_view
            && match (previous.protected_lock, self.protected_lock) {
                (None, _) => true,
                (Some(_), None) => false,
                (Some(old), Some(new)) => old == new || new.0.view > old.0.view,
            }
    }
    /// Bind every durable consumer fact used to classify retained ingress.
    pub(crate) fn projection_hash(self) -> Hash {
        let mut bytes = b"iroha:sumeragi:v2:leader-wire-consumer:v1".to_vec();
        bytes.extend(self.context_id.encode());
        bytes.extend(self.height.to_le_bytes());
        bytes.extend(self.owner);
        bytes.extend(self.consumer_tag.height().to_le_bytes());
        bytes.extend(self.consumer_tag.view().to_le_bytes());
        bytes.extend(self.consumer_tag.generation().get().to_le_bytes());
        bytes.extend(self.wal_id.get().to_le_bytes());
        bytes.push(u8::from(self.decision_durable));
        bytes.extend(self.highest_prepare_view.encode());
        bytes.extend(self.installed_timeout_view.encode());
        bytes.extend(self.protected_lock.encode());
        bytes.extend(self.protected_commit_statement.encode());
        Hash::new(bytes)
    }
    pub(crate) const fn consumer_tag(self) -> reducer::EventTag {
        self.consumer_tag
    }
    /// Verify the exact view and body owner already published from the WAL.
    pub(crate) fn matches_entered_view(
        self,
        tag: reducer::EventTag,
        protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    ) -> bool {
        self.consumer_tag == tag && self.protected_lock == protected_lock
    }
    fn protects_commit_vote(self, identity: &FairV2IngressLeaderWireIdentity) -> bool {
        identity.phase == Phase::CommitVote
            && self.protected_lock.is_some_and(|(round, subject)| {
                identity.context_id == round.context_id
                    && identity.height == round.height
                    && identity.view == round.view
                    && identity.subject_hash == Hash::new(subject.encode())
                    && identity.vote_statement_hash == self.protected_commit_statement
                    && self.protected_commit_statement.is_some()
            })
    }
    fn consumer_accepts(
        self,
        phase: Phase,
        view: wire::View,
        exact_commit: bool,
        timeout_prepare_view: Option<wire::View>,
    ) -> bool {
        if phase.source_class() != FairV2IngressLeaderWireSourceClass::Control {
            return true;
        }
        if self.decision_durable {
            return false;
        }
        let current_view = self.consumer_tag.view();
        match phase {
            Phase::Proposal | Phase::PrepareVote => view == current_view,
            Phase::CommitVote => exact_commit,
            Phase::PrepareQc => {
                view <= current_view
                    && self
                        .highest_prepare_view
                        .is_none_or(|highest| view >= highest)
            }
            Phase::CommitQc => true,
            Phase::TimeoutVote => reducer::timeout_vote_view_is_admissible(current_view, view),
            Phase::TimeoutCertificate => {
                view.checked_add(1).is_some()
                    && (view >= current_view
                        || reducer::strict_same_round_timeout_upgrade_is_allowed(
                            reducer::StrictSameRoundTimeoutUpgradeProjection {
                                current_view,
                                timeout_view: view,
                                installed_same_round: self.installed_timeout_view == Some(view),
                                selected_prepare_present: timeout_prepare_view.is_some(),
                                selected_prepare_view: timeout_prepare_view.unwrap_or(0),
                                highest_prepare_present: self.highest_prepare_view.is_some(),
                                highest_prepare_view: self.highest_prepare_view.unwrap_or(0),
                                locked_prepare_present: self.protected_lock.is_some(),
                                locked_prepare_view: self
                                    .protected_lock
                                    .map_or(0, |lock| lock.0.view),
                            },
                        ))
            }
            Phase::Chunk | Phase::CertifiedResponse => true,
        }
    }
    /// Retain bounded ownership when a later monotone WAL cut can make this
    /// wire eligible. This is intentionally broader than reducer admission:
    /// a future view is not a permanent retirement certificate.
    pub(crate) fn admits_ingress_identity(
        self,
        identity: &FairV2IngressLeaderWireIdentity,
    ) -> bool {
        self.retains(
            identity.phase,
            identity.view,
            self.protects_commit_vote(identity),
            identity.timeout_prepare_view,
        )
    }
    fn retains(
        self,
        phase: Phase,
        view: wire::View,
        exact_commit: bool,
        timeout_prepare_view: Option<wire::View>,
    ) -> bool {
        if phase.source_class() != FairV2IngressLeaderWireSourceClass::Control {
            return true;
        }
        if self.decision_durable {
            return false;
        }
        let current_view = self.consumer_tag.view();
        match phase {
            Phase::Proposal | Phase::PrepareVote | Phase::TimeoutVote => view >= current_view,
            Phase::CommitVote => view >= current_view || exact_commit,
            Phase::PrepareQc => self
                .highest_prepare_view
                .is_none_or(|highest| view >= highest),
            Phase::CommitQc => true,
            Phase::TimeoutCertificate => {
                self.consumer_accepts(phase, view, exact_commit, timeout_prepare_view)
            }
            Phase::Chunk | Phase::CertifiedResponse => true,
        }
    }
    fn consumer_accepts_identity(self, identity: &FairV2IngressLeaderWireIdentity) -> bool {
        self.consumer_accepts(
            identity.phase,
            identity.view,
            self.protects_commit_vote(identity),
            identity.timeout_prepare_view,
        )
    }
    pub(crate) fn consumer_waits_for(self, position: LeaderWireConsumerPosition) -> bool {
        let exact_commit = position.commit_statement.is_some()
            && position.commit_statement == self.protected_commit_statement;
        position.context_id == self.context_id
            && position.height == self.height
            && self.retains(
                position.phase,
                position.view,
                exact_commit,
                position.timeout_prepare_view,
            )
            && !self.consumer_accepts(
                position.phase,
                position.view,
                exact_commit,
                position.timeout_prepare_view,
            )
    }
    fn payload_coordinates(
        self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> Option<(Phase, wire::View, bool, Option<wire::View>)> {
        let position = LeaderWireConsumerPosition::from_payload(payload)?;
        Some((
            position.phase,
            position.view,
            position.commit_statement.is_some()
                && position.commit_statement == self.protected_commit_statement,
            position.timeout_prepare_view,
        ))
    }
    pub(super) fn admits_payload(self, payload: &wire::ConsensusMessageV2Payload) -> bool {
        self.payload_coordinates(payload).is_none_or(
            |(phase, view, exact_commit, timeout_prepare_view)| {
                self.consumer_accepts(phase, view, exact_commit, timeout_prepare_view)
            },
        )
    }
    pub(super) fn retains_payload(self, payload: &wire::ConsensusMessageV2Payload) -> bool {
        self.payload_coordinates(payload).is_none_or(
            |(phase, view, exact_commit, timeout_prepare_view)| {
                self.retains(phase, view, exact_commit, timeout_prepare_view)
            },
        )
    }
    pub(crate) fn retires(self, token: &FairV2IngressLeaderWireToken) -> bool {
        token.identity.phase.source_class() == FairV2IngressLeaderWireSourceClass::Control
            && !self.admits_ingress_identity(&token.identity)
    }
    /// Only carrierless deliveries can reopen; the original ordinals survive.
    pub(crate) fn rearms(
        self,
        token: &FairV2IngressLeaderWireToken,
        consumed_by: reducer::EventTag,
    ) -> bool {
        self.consumer_tag.strictly_advances(consumed_by)
            && self.consumer_accepts_identity(&token.identity)
            && matches!(
                token.identity.phase,
                Phase::Proposal | Phase::PrepareVote | Phase::CommitVote
            )
    }

    // Isolated snapshot tests may construct geometry. Production has only the
    // actual-adapter factory above; these fixtures never authorize a runtime.
    #[cfg(test)]
    pub(crate) const fn from_replayed_adapter(
        context_id: wire::HeightContextId,
        height: wire::Height,
        owner: [u8; 32],
        durable_view: wire::View,
        decision_durable: bool,
    ) -> Self {
        Self {
            context_id,
            height,
            owner,
            consumer_tag: reducer::EventTag::new(
                height,
                durable_view,
                reducer::Generation::INITIAL,
            ),
            wal_id: reducer::PersistenceId::new(0),
            decision_durable,
            highest_prepare_view: None,
            installed_timeout_view: durable_view.checked_sub(1),
            protected_lock: None,
            protected_commit_statement: None,
        }
    }
    /// Unqualified unit-fixture projection of the lock and exact Commit statement.
    /// This does not append/authenticate a CommitIntent; production authority is
    /// minted only by `from_adapter` from the actual replayed WAL and registry.
    #[cfg(test)]
    pub(crate) fn with_protected_lock(
        self,
        protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
        protected_commit_execution: Option<wire::ExecutionCommitment>,
    ) -> Result<Self, String> {
        let protected_commit_statement = match (protected_lock, protected_commit_execution) {
            (Some((round, subject)), Some(execution)) => {
                Some(vote_statement_hash(round, subject, &execution))
            }
            (None, Some(_)) => {
                return Err(
                    "a fixture Commit statement requires its exact protected lock".to_owned(),
                );
            }
            (_, None) => None,
        };
        let next = Self {
            protected_lock,
            protected_commit_statement,
            ..self
        };
        if protected_lock.is_some_and(|(round, _)| {
            round.context_id != self.context_id
                || round.height != self.height
                || round.view > self.consumer_tag.view()
        }) || !next.monotonically_extends(self)
        {
            return Err("leader-wire recovery authority regressed its protected lock".to_owned());
        }
        Ok(next)
    }
    #[cfg(test)]
    pub(crate) fn advance_view(
        self,
        durable_view: wire::View,
        protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
        protected_commit_execution: Option<wire::ExecutionCommitment>,
    ) -> Result<Self, String> {
        let next = Self {
            consumer_tag: reducer::EventTag::new(
                self.height,
                durable_view,
                reducer::Generation::INITIAL,
            ),
            installed_timeout_view: durable_view.checked_sub(1),
            ..self
        }
        .with_protected_lock(protected_lock, protected_commit_execution)?;
        if !next.monotonically_extends(self) {
            return Err("leader-wire recovery authority regressed its durable view".to_owned());
        }
        Ok(next)
    }
    #[cfg(test)]
    pub(crate) const fn with_durable_decision(self) -> Self {
        Self {
            decision_durable: true,
            ..self
        }
    }
}
