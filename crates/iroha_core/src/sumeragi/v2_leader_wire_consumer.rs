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

pub(crate) fn vote_statement_hash(
    proposal_round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    execution_commitment: &wire::ExecutionCommitment,
) -> Hash {
    Hash::new((proposal_round, subject, execution_commitment).encode())
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
    pub(crate) const fn consumer_tag(self) -> reducer::EventTag {
        self.consumer_tag
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
    fn admits(
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
    pub(crate) fn admits_ingress_identity(
        self,
        identity: &FairV2IngressLeaderWireIdentity,
    ) -> bool {
        self.admits(
            identity.phase,
            identity.view,
            self.protects_commit_vote(identity),
            identity.timeout_prepare_view,
        )
    }
    pub(super) fn admits_payload(self, payload: &wire::ConsensusMessageV2Payload) -> bool {
        use wire::ConsensusMessageV2Payload as Payload;
        let (phase, view, exact_commit, timeout_prepare_view) = match payload {
            Payload::Proposal(proposal) => (Phase::Proposal, proposal.round.view, false, None),
            Payload::Vote(vote) => (
                match vote.phase {
                    wire::GlobalPhase::Prepare => Phase::PrepareVote,
                    wire::GlobalPhase::Commit => Phase::CommitVote,
                },
                vote.round.view,
                vote.round == vote.proposal_round
                    && self.protected_commit_statement
                        == Some(vote_statement_hash(
                            vote.proposal_round,
                            vote.subject,
                            &vote.execution_commitment,
                        )),
                None,
            ),
            Payload::QuorumCertificate(qc) => (
                match qc.phase {
                    wire::GlobalPhase::Prepare => Phase::PrepareQc,
                    wire::GlobalPhase::Commit => Phase::CommitQc,
                },
                qc.round.view,
                false,
                None,
            ),
            Payload::TimeoutVote(vote) => (Phase::TimeoutVote, vote.round.view, false, None),
            Payload::TimeoutCertificate(tc) => (
                Phase::TimeoutCertificate,
                tc.round.view,
                false,
                tc.highest_prepare_qc().map(|qc| qc.round.view),
            ),
            _ => return true,
        };
        self.admits(phase, view, exact_commit, timeout_prepare_view)
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
            && self.admits_ingress_identity(&token.identity)
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
    #[cfg(test)]
    pub(crate) fn with_protected_lock(
        self,
        protected_lock: Option<(wire::ConsensusRound, wire::BlockSubject)>,
    ) -> Result<Self, String> {
        let next = Self {
            protected_lock,
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
        .with_protected_lock(protected_lock)?;
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
