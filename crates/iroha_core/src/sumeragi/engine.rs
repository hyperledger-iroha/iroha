//! Pure Sumeragi V1 consensus state machine.
//!
//! Network ingress, signature workers, block validation, RBC, telemetry, and
//! storage are adapters around this deterministic engine.
//!
//! TODO: route the remaining network, worker, and storage adapters through this
//! engine so consensus decisions are owned by one state machine.

use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::{
    BlockHeader,
    consensus::{
        BlockSubject, CertPhase, Certificate, PayloadRequest, QcRef, QuorumPolicy, RoundId,
        ValidatorSetId,
    },
};

/// Proposal input accepted by the pure engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineProposal {
    /// Proposed round.
    pub round: RoundId,
    /// Proposed block subject.
    pub subject: BlockSubject,
    /// Highest QC carried by the proposer.
    pub highest_qc: Option<QcRef>,
}

/// Validator-set change scheduled by a finalized reconfiguration block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatorSetChange {
    /// First height using the new validator set.
    pub activation_height: u64,
    /// New validator-set id.
    pub validator_set_id: ValidatorSetId,
    /// New quorum policy.
    pub quorum_policy: QuorumPolicy,
}

/// Inputs consumed by [`ConsensusEngine`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusInput {
    /// Logical pacemaker tick.
    Tick {
        /// Monotonic adapter timestamp in milliseconds.
        now_ms: u64,
    },
    /// Candidate proposal for the current height/view.
    Proposal(EngineProposal),
    /// Verified aggregate certificate.
    Certificate(Certificate),
    /// Local payload availability notification.
    PayloadAvailable(BlockSubject),
    /// Asynchronous block validation result.
    ValidationResult {
        /// Validated round.
        round: RoundId,
        /// Validated block hash.
        block_hash: HashOf<BlockHeader>,
        /// Whether validation accepted the block.
        valid: bool,
    },
    /// Committed block notification from storage/application.
    CommittedBlock {
        /// Committed round.
        round: RoundId,
        /// Committed block hash.
        block_hash: HashOf<BlockHeader>,
        /// Optional epoch-boundary validator-set change finalized by this block.
        reconfiguration: Option<ValidatorSetChange>,
    },
}

/// Outputs emitted by [`ConsensusEngine`] for adapters to execute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusOutput {
    /// Ask the signing adapter to sign a vote.
    SignVote {
        /// Vote phase to sign.
        phase: CertPhase,
        /// Vote round.
        round: RoundId,
        /// Vote subject.
        subject: BlockSubject,
        /// Highest QC to bind into a `NewView` vote.
        highest_qc: Option<QcRef>,
    },
    /// Ask the validation adapter to validate a block.
    ValidateBlock {
        /// Block subject to validate.
        subject: BlockSubject,
    },
    /// Ask the payload/RBC adapter to fetch missing payload bytes.
    FetchPayload(PayloadRequest),
    /// Apply finality for a locally available block.
    CommitBlock {
        /// Finalized subject.
        subject: BlockSubject,
    },
    /// Advance local view.
    AdvanceView {
        /// New round.
        round: RoundId,
    },
    /// Activate a validator-set change at an epoch boundary.
    ActivateValidatorSet(ValidatorSetChange),
}

/// Observable phase of the pure consensus engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnginePhase {
    /// Waiting for a proposal.
    Proposal,
    /// Waiting for prepare quorum.
    Prepare,
    /// Waiting for commit quorum.
    Commit,
    /// Commit QC exists but local payload is missing.
    PendingFinality,
}

/// Snapshot of the pure engine state for status adapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineState {
    /// Current round.
    pub round: RoundId,
    /// Current phase.
    pub phase: EnginePhase,
    /// Locked QC, if any.
    pub locked_qc: Option<QcRef>,
    /// Highest QC, if any.
    pub highest_qc: Option<QcRef>,
    /// Pending finality subject, if a commit QC arrived before payload.
    pub pending_finality: Option<BlockSubject>,
    /// Current quorum policy.
    pub quorum_policy: QuorumPolicy,
}

/// Deterministic Sumeragi V1 state machine.
#[derive(Clone, Debug)]
pub struct ConsensusEngine {
    state: EngineState,
    available_payloads: BTreeSet<(HashOf<BlockHeader>, Hash)>,
    pending_finality: BTreeMap<HashOf<BlockHeader>, Certificate>,
    committed: BTreeMap<u64, HashOf<BlockHeader>>,
    pending_reconfiguration: Option<ValidatorSetChange>,
    validating: Option<BlockSubject>,
    commit_votes: BTreeMap<RoundId, BlockSubject>,
}

impl ConsensusEngine {
    /// Create a pure engine for `round`.
    #[must_use]
    pub fn new(round: RoundId, quorum_policy: QuorumPolicy) -> Self {
        Self {
            state: EngineState {
                round,
                phase: EnginePhase::Proposal,
                locked_qc: None,
                highest_qc: None,
                pending_finality: None,
                quorum_policy,
            },
            available_payloads: BTreeSet::new(),
            pending_finality: BTreeMap::new(),
            committed: BTreeMap::new(),
            pending_reconfiguration: None,
            validating: None,
            commit_votes: BTreeMap::new(),
        }
    }

    /// Return the current state snapshot.
    #[must_use]
    pub fn state(&self) -> &EngineState {
        &self.state
    }

    /// Return the block hash committed at `height`, if known.
    #[must_use]
    pub fn committed_at(&self, height: u64) -> Option<HashOf<BlockHeader>> {
        self.committed.get(&height).copied()
    }

    /// Apply one input and return adapter commands.
    pub fn handle(&mut self, input: ConsensusInput) -> Vec<ConsensusOutput> {
        match input {
            ConsensusInput::Tick { .. } => self.on_tick(),
            ConsensusInput::Proposal(proposal) => self.on_proposal(proposal),
            ConsensusInput::Certificate(certificate) => self.on_certificate(certificate),
            ConsensusInput::PayloadAvailable(subject) => self.on_payload_available(subject),
            ConsensusInput::ValidationResult {
                round,
                block_hash,
                valid,
            } => self.on_validation_result(round, block_hash, valid),
            ConsensusInput::CommittedBlock {
                round,
                block_hash,
                reconfiguration,
            } => self.on_committed_block(round, block_hash, reconfiguration),
        }
    }

    fn on_tick(&mut self) -> Vec<ConsensusOutput> {
        let next = RoundId {
            view: self.state.round.view.saturating_add(1),
            ..self.state.round
        };
        self.state.round = next;
        self.state.phase = EnginePhase::Proposal;
        self.validating = None;
        vec![
            ConsensusOutput::SignVote {
                phase: CertPhase::NewView,
                round: next,
                subject: self
                    .state
                    .highest_qc
                    .map(qc_subject)
                    .unwrap_or_else(zero_subject),
                highest_qc: self.state.highest_qc,
            },
            ConsensusOutput::AdvanceView { round: next },
        ]
    }

    fn on_proposal(&mut self, proposal: EngineProposal) -> Vec<ConsensusOutput> {
        if self.state.phase != EnginePhase::Proposal
            || proposal.round != self.state.round
            || !proposal
                .highest_qc
                .is_none_or(|highest| qc_ref_is_compatible_with_round(&highest, &proposal.round))
            || !self.proposal_satisfies_lock(&proposal)
        {
            return Vec::new();
        }
        self.state.phase = EnginePhase::Prepare;
        self.validating = Some(proposal.subject);
        vec![
            ConsensusOutput::ValidateBlock {
                subject: proposal.subject,
            },
            ConsensusOutput::SignVote {
                phase: CertPhase::Prepare,
                round: proposal.round,
                subject: proposal.subject,
                highest_qc: None,
            },
        ]
    }

    fn proposal_satisfies_lock(&self, proposal: &EngineProposal) -> bool {
        let Some(locked_qc) = self.state.locked_qc else {
            return true;
        };
        if proposal.subject.block_hash == locked_qc.subject_block_hash {
            return true;
        }
        proposal
            .highest_qc
            .is_some_and(|highest| qc_ref_cmp(&highest, &locked_qc).is_gt())
    }

    fn on_certificate(&mut self, certificate: Certificate) -> Vec<ConsensusOutput> {
        if self.committed.contains_key(&certificate.round.height) {
            return Vec::new();
        }
        if certificate.round.height != self.state.round.height
            || certificate.round.epoch != self.state.round.epoch
            || certificate.round.validator_set_id != self.state.round.validator_set_id
        {
            return Vec::new();
        }
        if certificate.quorum_policy != self.state.quorum_policy {
            return Vec::new();
        }
        if matches!(certificate.phase, CertPhase::Prepare | CertPhase::Commit)
            && certificate.round.view != self.state.round.view
        {
            return Vec::new();
        }
        match certificate.phase {
            CertPhase::Prepare => self.on_prepare_qc(certificate),
            CertPhase::Commit => self.on_commit_qc(certificate),
            CertPhase::NewView => self.on_new_view_qc(certificate),
        }
    }

    fn on_prepare_qc(&mut self, certificate: Certificate) -> Vec<ConsensusOutput> {
        if let Some(existing) = self.commit_votes.get(&certificate.round) {
            if existing != &certificate.subject {
                return Vec::new();
            }
            return Vec::new();
        }
        if self.state.phase == EnginePhase::PendingFinality {
            return Vec::new();
        }
        let qc = qc_ref_from_certificate(&certificate);
        self.state.locked_qc = Some(qc);
        self.record_highest_qc(qc);
        self.state.phase = EnginePhase::Commit;
        self.commit_votes
            .insert(certificate.round, certificate.subject);
        vec![ConsensusOutput::SignVote {
            phase: CertPhase::Commit,
            round: certificate.round,
            subject: certificate.subject,
            highest_qc: None,
        }]
    }

    fn on_commit_qc(&mut self, certificate: Certificate) -> Vec<ConsensusOutput> {
        self.validating = None;
        if let Some(pending) = self.state.pending_finality {
            if pending != certificate.subject {
                return Vec::new();
            }
            return Vec::new();
        }
        let qc = qc_ref_from_certificate(&certificate);
        self.record_highest_qc(qc);
        if self.has_payload(certificate.subject) {
            self.commit_subject(certificate.subject)
        } else {
            self.state.phase = EnginePhase::PendingFinality;
            self.state.pending_finality = Some(certificate.subject);
            self.pending_finality
                .insert(certificate.subject.block_hash, certificate.clone());
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: certificate.round,
                block_hash: certificate.subject.block_hash,
                payload_hash: certificate.subject.payload_hash,
            })]
        }
    }

    fn on_new_view_qc(&mut self, certificate: Certificate) -> Vec<ConsensusOutput> {
        if certificate.round.view <= self.state.round.view {
            return Vec::new();
        }
        if let Some(highest) = certificate.highest_qc
            && !qc_ref_is_compatible_with_round(&highest, &certificate.round)
        {
            return Vec::new();
        }
        if let Some(highest) = certificate.highest_qc {
            self.record_highest_qc(highest);
        }
        self.state.round = certificate.round;
        self.state.phase = EnginePhase::Proposal;
        self.validating = None;
        vec![ConsensusOutput::AdvanceView {
            round: certificate.round,
        }]
    }

    fn on_payload_available(&mut self, subject: BlockSubject) -> Vec<ConsensusOutput> {
        self.available_payloads
            .insert((subject.block_hash, subject.payload_hash));
        let Some(certificate) = self.pending_finality.get(&subject.block_hash).cloned() else {
            return Vec::new();
        };
        if certificate.subject != subject {
            return Vec::new();
        }
        self.pending_finality.remove(&subject.block_hash);
        self.commit_subject(subject)
    }

    fn on_validation_result(
        &mut self,
        round: RoundId,
        block_hash: HashOf<BlockHeader>,
        valid: bool,
    ) -> Vec<ConsensusOutput> {
        if round != self.state.round {
            return Vec::new();
        }
        let Some(validating) = self.validating else {
            return Vec::new();
        };
        if validating.block_hash != block_hash {
            return Vec::new();
        }
        self.validating = None;
        if valid {
            return Vec::new();
        }
        let next = RoundId {
            view: round.view.saturating_add(1),
            ..round
        };
        self.state.round = next;
        self.state.phase = EnginePhase::Proposal;
        let subject = self
            .state
            .highest_qc
            .map(qc_subject)
            .unwrap_or(BlockSubject {
                parent_block: block_hash,
                block_hash,
                payload_hash: Hash::prehashed([0; Hash::LENGTH]),
            });
        vec![
            ConsensusOutput::SignVote {
                phase: CertPhase::NewView,
                round: next,
                subject,
                highest_qc: self.state.highest_qc,
            },
            ConsensusOutput::AdvanceView { round: next },
        ]
    }

    fn on_committed_block(
        &mut self,
        round: RoundId,
        block_hash: HashOf<BlockHeader>,
        reconfiguration: Option<ValidatorSetChange>,
    ) -> Vec<ConsensusOutput> {
        if self.committed.contains_key(&round.height) {
            return Vec::new();
        }
        self.committed.insert(round.height, block_hash);
        if round.height == self.state.round.height {
            self.validating = None;
            self.state.phase = EnginePhase::Proposal;
            if let Some(pending) = self.state.pending_finality.take() {
                self.pending_finality.remove(&pending.block_hash);
            }
        }
        let mut outputs = Vec::new();
        if let Some(change) = reconfiguration {
            if change.activation_height == round.height.saturating_add(1) {
                self.pending_reconfiguration = Some(change.clone());
                outputs.push(ConsensusOutput::ActivateValidatorSet(change));
            }
        }
        outputs
    }

    fn has_payload(&self, subject: BlockSubject) -> bool {
        self.available_payloads
            .contains(&(subject.block_hash, subject.payload_hash))
    }

    fn commit_subject(&mut self, subject: BlockSubject) -> Vec<ConsensusOutput> {
        if let Some(committed) = self.committed.get(&self.state.round.height)
            && committed != &subject.block_hash
        {
            return Vec::new();
        }
        self.committed
            .insert(self.state.round.height, subject.block_hash);
        self.state.phase = EnginePhase::Proposal;
        self.state.pending_finality = None;
        self.validating = None;
        vec![ConsensusOutput::CommitBlock { subject }]
    }

    fn record_highest_qc(&mut self, candidate: QcRef) {
        let update = self
            .state
            .highest_qc
            .is_none_or(|current| qc_ref_cmp(&candidate, &current).is_gt());
        if update {
            self.state.highest_qc = Some(candidate);
        }
    }
}

/// Select the deterministic highest QC from a new-view quorum.
#[must_use]
pub fn select_highest_qc<I>(certificates: I) -> Option<QcRef>
where
    I: IntoIterator,
    I::Item: Borrow<Certificate>,
{
    certificates
        .into_iter()
        .filter_map(|certificate| {
            let certificate = certificate.borrow();
            (certificate.phase == CertPhase::NewView)
                .then_some(certificate.highest_qc)
                .flatten()
        })
        .max_by(qc_ref_cmp)
}

fn qc_ref_from_certificate(certificate: &Certificate) -> QcRef {
    QcRef {
        height: certificate.round.height,
        view: certificate.round.view,
        epoch: certificate.round.epoch,
        subject_block_hash: certificate.subject.block_hash,
        phase: certificate.phase,
    }
}

fn qc_ref_cmp(left: &QcRef, right: &QcRef) -> std::cmp::Ordering {
    left.height
        .cmp(&right.height)
        .then_with(|| left.view.cmp(&right.view))
        .then_with(|| phase_rank(left.phase).cmp(&phase_rank(right.phase)))
        .then_with(|| {
            left.subject_block_hash
                .as_ref()
                .as_ref()
                .cmp(right.subject_block_hash.as_ref().as_ref())
        })
}

fn qc_ref_is_compatible_with_round(qc: &QcRef, round: &RoundId) -> bool {
    qc.epoch == round.epoch
        && (qc.height < round.height || (qc.height == round.height && qc.view <= round.view))
}

const fn phase_rank(phase: CertPhase) -> u8 {
    match phase {
        CertPhase::Prepare => 0,
        CertPhase::NewView => 1,
        CertPhase::Commit => 2,
    }
}

fn qc_subject(qc: QcRef) -> BlockSubject {
    BlockSubject {
        parent_block: qc.subject_block_hash,
        block_hash: qc.subject_block_hash,
        payload_hash: Hash::prehashed([0; Hash::LENGTH]),
    }
}

fn zero_subject() -> BlockSubject {
    let zero = HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]));
    BlockSubject {
        parent_block: zero,
        block_hash: zero,
        payload_hash: Hash::prehashed([0; Hash::LENGTH]),
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;

    use super::*;

    fn block_hash(label: &[u8]) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::new(label))
    }

    fn prehashed_block_hash(byte: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([byte; Hash::LENGTH]))
    }

    fn validator_set(label: &[u8]) -> ValidatorSetId {
        ValidatorSetId {
            hash: HashOf::from_untyped_unchecked(Hash::new(label)),
        }
    }

    fn round(view: u64) -> RoundId {
        RoundId {
            height: 1,
            view,
            epoch: 0,
            validator_set_id: validator_set(b"validators"),
        }
    }

    fn subject(label: &[u8]) -> BlockSubject {
        BlockSubject {
            parent_block: block_hash(b"parent"),
            block_hash: block_hash(label),
            payload_hash: Hash::new([b"payload".as_slice(), label].concat()),
        }
    }

    fn certificate(phase: CertPhase, round: RoundId, subject: BlockSubject) -> Certificate {
        Certificate {
            phase,
            round,
            subject,
            quorum_policy: QuorumPolicy::PermissionedCount(4),
            highest_qc: None,
            signers_bitmap: vec![0b0000_0111],
            bls_aggregate_signature: vec![1, 2, 3],
        }
    }

    #[test]
    fn conflicting_blocks_cannot_both_commit_at_same_height() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let first = subject(b"first");
        let second = subject(b"second");

        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                first,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: first.block_hash,
                payload_hash: first.payload_hash,
            })]
        );
        assert_eq!(
            engine.handle(ConsensusInput::PayloadAvailable(first)),
            vec![ConsensusOutput::CommitBlock { subject: first }]
        );
        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Commit,
                    round(0),
                    second,
                )))
                .is_empty()
        );
        assert_eq!(engine.committed_at(1), Some(first.block_hash));
    }

    #[test]
    fn locked_qc_blocks_unsafe_prepare_votes() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let locked = subject(b"locked");
        let unsafe_subject = subject(b"unsafe");
        let prepare = certificate(CertPhase::Prepare, round(0), locked);
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(prepare)),
            vec![ConsensusOutput::SignVote {
                phase: CertPhase::Commit,
                round: round(0),
                subject: locked,
                highest_qc: None,
            }]
        );

        assert!(
            engine
                .handle(ConsensusInput::Proposal(EngineProposal {
                    round: round(0),
                    subject: unsafe_subject,
                    highest_qc: None,
                }))
                .is_empty()
        );
    }

    #[test]
    fn prepare_qc_replays_and_conflicts_do_not_emit_extra_commit_votes() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let first = subject(b"first-prepare-qc");
        let conflicting = subject(b"conflicting-prepare-qc");
        let prepare = certificate(CertPhase::Prepare, round(0), first);

        assert_eq!(
            engine.handle(ConsensusInput::Certificate(prepare.clone())),
            vec![ConsensusOutput::SignVote {
                phase: CertPhase::Commit,
                round: round(0),
                subject: first,
                highest_qc: None,
            }]
        );
        assert!(
            engine
                .handle(ConsensusInput::Certificate(prepare))
                .is_empty()
        );
        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Prepare,
                    round(0),
                    conflicting,
                )))
                .is_empty()
        );
        assert_eq!(
            engine.state().locked_qc,
            Some(QcRef {
                height: 1,
                view: 0,
                epoch: 0,
                subject_block_hash: first.block_hash,
                phase: CertPhase::Prepare,
            })
        );
    }

    #[test]
    fn prepare_qcs_with_wrong_round_context_are_ignored() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let base = round(0);
        let mut wrong_height = base.clone();
        wrong_height.height = wrong_height.height.saturating_add(1);
        let mut wrong_epoch = base.clone();
        wrong_epoch.epoch = wrong_epoch.epoch.saturating_add(1);
        let mut wrong_validator_set = base;
        wrong_validator_set.validator_set_id = validator_set(b"other-validators");

        for (round, label) in [
            (wrong_height, b"wrong-height".as_slice()),
            (wrong_epoch, b"wrong-epoch".as_slice()),
            (wrong_validator_set, b"wrong-validator-set".as_slice()),
        ] {
            assert!(
                engine
                    .handle(ConsensusInput::Certificate(certificate(
                        CertPhase::Prepare,
                        round,
                        subject(label),
                    )))
                    .is_empty()
            );
        }
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.state().locked_qc, None);
        assert_eq!(engine.state().highest_qc, None);
    }

    #[test]
    fn prepare_qc_for_committed_height_is_ignored() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let committed = block_hash(b"committed-before-prepare");
        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash: committed,
                    reconfiguration: None,
                })
                .is_empty()
        );
        assert_eq!(engine.committed_at(1), Some(committed));

        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Prepare,
                    round(0),
                    subject(b"prepare-after-commit"),
                )))
                .is_empty()
        );
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.state().locked_qc, None);
        assert_eq!(engine.state().highest_qc, None);
    }

    #[test]
    fn prepare_qc_during_pending_finality_does_not_emit_commit_vote() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let pending = subject(b"pending-finality-block");
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                pending,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: pending.block_hash,
                payload_hash: pending.payload_hash,
            })]
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(pending));
        let highest_before = engine.state().highest_qc;

        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Prepare,
                    round(0),
                    subject(b"prepare-while-pending-finality"),
                )))
                .is_empty()
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(pending));
        assert_eq!(engine.state().locked_qc, None);
        assert_eq!(engine.state().highest_qc, highest_before);
    }

    #[test]
    fn conflicting_proposal_requires_strictly_higher_qc_to_unlock() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let locked = subject(b"locked-for-unlock-rule");
        let conflicting = subject(b"conflicting-unlock-attempt");
        assert!(
            !engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Prepare,
                    round(0),
                    locked,
                )))
                .is_empty()
        );

        let mut new_view = certificate(CertPhase::NewView, round(1), locked);
        new_view.highest_qc = engine.state().highest_qc;
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(new_view)),
            vec![ConsensusOutput::AdvanceView { round: round(1) }]
        );

        let locked_qc = engine.state().locked_qc.expect("lock recorded");
        assert!(
            engine
                .handle(ConsensusInput::Proposal(EngineProposal {
                    round: round(1),
                    subject: conflicting,
                    highest_qc: Some(locked_qc),
                }))
                .is_empty(),
            "equal QC must not unlock a conflicting proposal"
        );

        let lower_qc = QcRef {
            height: locked_qc.height.saturating_sub(1),
            view: locked_qc.view,
            epoch: locked_qc.epoch,
            subject_block_hash: block_hash(b"lower-unlock-qc"),
            phase: CertPhase::Prepare,
        };
        assert!(
            engine
                .handle(ConsensusInput::Proposal(EngineProposal {
                    round: round(1),
                    subject: conflicting,
                    highest_qc: Some(lower_qc),
                }))
                .is_empty(),
            "non-greater QC must not unlock a conflicting proposal"
        );

        let higher_qc = QcRef {
            view: locked_qc.view.saturating_add(1),
            ..locked_qc
        };
        assert_eq!(
            engine.handle(ConsensusInput::Proposal(EngineProposal {
                round: round(1),
                subject: conflicting,
                highest_qc: Some(higher_qc),
            })),
            vec![
                ConsensusOutput::ValidateBlock {
                    subject: conflicting,
                },
                ConsensusOutput::SignVote {
                    phase: CertPhase::Prepare,
                    round: round(1),
                    subject: conflicting,
                    highest_qc: None,
                },
            ],
            "strictly higher QC may unlock a conflicting proposal"
        );
    }

    #[test]
    fn proposal_with_incompatible_highest_qc_cannot_unlock_conflicting_lock() {
        for highest_qc in [
            QcRef {
                height: 2,
                view: 0,
                epoch: 0,
                subject_block_hash: block_hash(b"proposal-future-height"),
                phase: CertPhase::Commit,
            },
            QcRef {
                height: 1,
                view: 2,
                epoch: 0,
                subject_block_hash: block_hash(b"proposal-future-view"),
                phase: CertPhase::Commit,
            },
            QcRef {
                height: 1,
                view: 1,
                epoch: 1,
                subject_block_hash: block_hash(b"proposal-wrong-epoch"),
                phase: CertPhase::Commit,
            },
        ] {
            let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
            let locked = subject(b"proposal-incompatible-lock");
            let conflicting = subject(b"proposal-incompatible-conflict");
            assert!(
                !engine
                    .handle(ConsensusInput::Certificate(certificate(
                        CertPhase::Prepare,
                        round(0),
                        locked,
                    )))
                    .is_empty()
            );
            let locked_qc = engine.state().locked_qc.expect("lock recorded");

            let mut new_view = certificate(CertPhase::NewView, round(1), locked);
            new_view.highest_qc = Some(locked_qc);
            assert_eq!(
                engine.handle(ConsensusInput::Certificate(new_view)),
                vec![ConsensusOutput::AdvanceView { round: round(1) }]
            );

            assert!(
                engine
                    .handle(ConsensusInput::Proposal(EngineProposal {
                        round: round(1),
                        subject: conflicting,
                        highest_qc: Some(highest_qc),
                    }))
                    .is_empty()
            );
            assert_eq!(engine.state().phase, EnginePhase::Proposal);
            assert_eq!(engine.state().locked_qc, Some(locked_qc));
            assert_eq!(engine.state().highest_qc, Some(locked_qc));
        }
    }

    #[test]
    fn proposal_with_incompatible_highest_qc_is_rejected_without_lock() {
        for highest_qc in [
            QcRef {
                height: 2,
                view: 0,
                epoch: 0,
                subject_block_hash: block_hash(b"unlocked-proposal-future-height"),
                phase: CertPhase::Commit,
            },
            QcRef {
                height: 1,
                view: 1,
                epoch: 0,
                subject_block_hash: block_hash(b"unlocked-proposal-future-view"),
                phase: CertPhase::Commit,
            },
            QcRef {
                height: 1,
                view: 0,
                epoch: 1,
                subject_block_hash: block_hash(b"unlocked-proposal-wrong-epoch"),
                phase: CertPhase::Commit,
            },
        ] {
            let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));

            assert!(
                engine
                    .handle(ConsensusInput::Proposal(EngineProposal {
                        round: round(0),
                        subject: subject(b"proposal-with-incompatible-carried-qc"),
                        highest_qc: Some(highest_qc),
                    }))
                    .is_empty()
            );
            assert_eq!(engine.state().round, round(0));
            assert_eq!(engine.state().phase, EnginePhase::Proposal);
            assert_eq!(engine.state().locked_qc, None);
            assert_eq!(engine.state().highest_qc, None);
        }
    }

    #[test]
    fn proposals_are_ignored_outside_proposal_phase() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let first = subject(b"first-proposal");
        let replay = subject(b"proposal-replay");

        assert_eq!(
            engine.handle(ConsensusInput::Proposal(EngineProposal {
                round: round(0),
                subject: first,
                highest_qc: None,
            })),
            vec![
                ConsensusOutput::ValidateBlock { subject: first },
                ConsensusOutput::SignVote {
                    phase: CertPhase::Prepare,
                    round: round(0),
                    subject: first,
                    highest_qc: None,
                },
            ]
        );
        assert_eq!(engine.state().phase, EnginePhase::Prepare);
        assert!(
            engine
                .handle(ConsensusInput::Proposal(EngineProposal {
                    round: round(0),
                    subject: replay,
                    highest_qc: None,
                }))
                .is_empty()
        );
    }

    #[test]
    fn proposals_with_wrong_round_context_are_ignored() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let base = round(0);
        let mut wrong_height = base.clone();
        wrong_height.height = wrong_height.height.saturating_add(1);
        let mut wrong_epoch = base.clone();
        wrong_epoch.epoch = wrong_epoch.epoch.saturating_add(1);
        let mut wrong_validator_set = base.clone();
        wrong_validator_set.validator_set_id = validator_set(b"other-proposal-validators");
        let mut wrong_view = base;
        wrong_view.view = wrong_view.view.saturating_add(1);

        for (round, label) in [
            (wrong_height, b"wrong-proposal-height".as_slice()),
            (wrong_epoch, b"wrong-proposal-epoch".as_slice()),
            (
                wrong_validator_set,
                b"wrong-proposal-validator-set".as_slice(),
            ),
            (wrong_view, b"wrong-proposal-view".as_slice()),
        ] {
            assert!(
                engine
                    .handle(ConsensusInput::Proposal(EngineProposal {
                        round,
                        subject: subject(label),
                        highest_qc: None,
                    }))
                    .is_empty()
            );
        }
        assert_eq!(engine.state().round, round(0));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.state().locked_qc, None);
        assert_eq!(engine.state().highest_qc, None);
    }

    #[test]
    fn certificates_with_wrong_view_or_quorum_policy_are_ignored() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let mut wrong_view_round = round(0);
        wrong_view_round.view += 1;

        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Prepare,
                    wrong_view_round,
                    subject(b"wrong-view-prepare"),
                )))
                .is_empty()
        );
        assert_eq!(engine.state().phase, EnginePhase::Proposal);

        let mut wrong_quorum = certificate(CertPhase::Prepare, round(0), subject(b"wrong-quorum"));
        wrong_quorum.quorum_policy = QuorumPolicy::PermissionedCount(5);
        assert!(
            engine
                .handle(ConsensusInput::Certificate(wrong_quorum))
                .is_empty()
        );
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.state().locked_qc, None);
    }

    #[test]
    fn prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        assert_eq!(
            engine.handle(ConsensusInput::Tick { now_ms: 10 }),
            vec![
                ConsensusOutput::SignVote {
                    phase: CertPhase::NewView,
                    round: round(1),
                    subject: zero_subject(),
                    highest_qc: None,
                },
                ConsensusOutput::AdvanceView { round: round(1) },
            ]
        );

        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Prepare,
                    round(0),
                    subject(b"stale-prepare-after-timeout"),
                )))
                .is_empty()
        );
        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Commit,
                    round(0),
                    subject(b"stale-commit-after-timeout"),
                )))
                .is_empty()
        );
        assert_eq!(engine.state().round, round(1));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.committed_at(1), None);
    }

    #[test]
    fn new_view_certificates_with_wrong_epoch_or_validator_set_are_ignored() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));

        let mut wrong_epoch = round(1);
        wrong_epoch.epoch = wrong_epoch.epoch.saturating_add(1);
        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::NewView,
                    wrong_epoch,
                    subject(b"wrong-epoch"),
                )))
                .is_empty()
        );

        let mut wrong_validator_set = round(1);
        wrong_validator_set.validator_set_id = validator_set(b"other-validator-set");
        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::NewView,
                    wrong_validator_set,
                    subject(b"wrong-validator-set"),
                )))
                .is_empty()
        );

        assert_eq!(engine.state().round, round(0));
        assert_eq!(engine.state().highest_qc, None);
    }

    #[test]
    fn new_view_certificate_rejects_incompatible_highest_qc() {
        for highest_qc in [
            QcRef {
                height: 2,
                view: 0,
                epoch: 0,
                subject_block_hash: block_hash(b"future-height-highest"),
                phase: CertPhase::Prepare,
            },
            QcRef {
                height: 1,
                view: 2,
                epoch: 0,
                subject_block_hash: block_hash(b"future-view-highest"),
                phase: CertPhase::Prepare,
            },
            QcRef {
                height: 1,
                view: 0,
                epoch: 1,
                subject_block_hash: block_hash(b"wrong-epoch-highest"),
                phase: CertPhase::Commit,
            },
        ] {
            let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
            let mut new_view = certificate(CertPhase::NewView, round(1), subject(b"new-view"));
            new_view.highest_qc = Some(highest_qc);

            assert!(
                engine
                    .handle(ConsensusInput::Certificate(new_view))
                    .is_empty()
            );
            assert_eq!(engine.state().round, round(0));
            assert_eq!(engine.state().highest_qc, None);
        }

        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let compatible_previous_height = QcRef {
            height: 0,
            view: 99,
            epoch: 0,
            subject_block_hash: block_hash(b"previous-height-high-view"),
            phase: CertPhase::Commit,
        };
        let mut new_view = certificate(CertPhase::NewView, round(1), subject(b"compatible"));
        new_view.highest_qc = Some(compatible_previous_height);
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(new_view)),
            vec![ConsensusOutput::AdvanceView { round: round(1) }]
        );
        assert_eq!(engine.state().highest_qc, Some(compatible_previous_height));
    }

    #[test]
    fn stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let accepted_highest = QcRef {
            height: 1,
            view: 0,
            epoch: 0,
            subject_block_hash: block_hash(b"accepted-highest"),
            phase: CertPhase::Prepare,
        };
        let mut current_new_view = certificate(CertPhase::NewView, round(3), subject(b"advance"));
        current_new_view.highest_qc = Some(accepted_highest);
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(current_new_view)),
            vec![ConsensusOutput::AdvanceView { round: round(3) }]
        );
        assert_eq!(engine.state().highest_qc, Some(accepted_highest));

        let adversarial_highest = QcRef {
            height: 1,
            view: 99,
            epoch: 0,
            subject_block_hash: block_hash(b"adversarial-highest"),
            phase: CertPhase::Commit,
        };
        let mut stale_new_view = certificate(CertPhase::NewView, round(2), subject(b"stale"));
        stale_new_view.highest_qc = Some(adversarial_highest);
        assert!(
            engine
                .handle(ConsensusInput::Certificate(stale_new_view))
                .is_empty()
        );

        assert_eq!(engine.state().round, round(3));
        assert_eq!(engine.state().highest_qc, Some(accepted_highest));
    }

    #[test]
    fn accepted_new_view_certificate_cannot_downgrade_highest_qc() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let high = QcRef {
            height: 1,
            view: 2,
            epoch: 0,
            subject_block_hash: block_hash(b"high-carried-qc"),
            phase: CertPhase::Commit,
        };
        let mut advance_with_high = certificate(CertPhase::NewView, round(3), subject(b"high"));
        advance_with_high.highest_qc = Some(high);
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(advance_with_high)),
            vec![ConsensusOutput::AdvanceView { round: round(3) }]
        );
        assert_eq!(engine.state().highest_qc, Some(high));

        let lower = QcRef {
            height: 1,
            view: 1,
            epoch: 0,
            subject_block_hash: block_hash(b"lower-carried-qc"),
            phase: CertPhase::Prepare,
        };
        let mut advance_with_lower = certificate(CertPhase::NewView, round(4), subject(b"lower"));
        advance_with_lower.highest_qc = Some(lower);
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(advance_with_lower)),
            vec![ConsensusOutput::AdvanceView { round: round(4) }]
        );
        assert_eq!(engine.state().round, round(4));
        assert_eq!(
            engine.state().highest_qc,
            Some(high),
            "accepted new-view evidence must never regress local highest QC"
        );
    }

    #[test]
    fn new_view_certificate_selects_highest_qc_deterministically() {
        let base = subject(b"base");
        let high = QcRef {
            height: 2,
            view: 0,
            epoch: 0,
            subject_block_hash: block_hash(b"high"),
            phase: CertPhase::Prepare,
        };
        let low = QcRef {
            height: 1,
            view: 9,
            epoch: 0,
            subject_block_hash: block_hash(b"low"),
            phase: CertPhase::Commit,
        };
        let mut a = certificate(CertPhase::NewView, round(2), base);
        a.highest_qc = Some(low);
        let mut b = certificate(CertPhase::NewView, round(2), base);
        b.highest_qc = Some(high);

        assert_eq!(select_highest_qc([&a, &b]), Some(high));
        assert_eq!(select_highest_qc([&b, &a]), Some(high));

        let phase_commit_wins = QcRef {
            height: 2,
            view: 1,
            epoch: 0,
            subject_block_hash: prehashed_block_hash(1),
            phase: CertPhase::Commit,
        };
        let phase_prepare_loses = QcRef {
            height: 2,
            view: 1,
            epoch: 0,
            subject_block_hash: prehashed_block_hash(255),
            phase: CertPhase::Prepare,
        };
        let mut phase_a = certificate(CertPhase::NewView, round(2), base);
        phase_a.highest_qc = Some(phase_prepare_loses);
        let mut phase_b = certificate(CertPhase::NewView, round(2), base);
        phase_b.highest_qc = Some(phase_commit_wins);

        assert_eq!(
            select_highest_qc([&phase_a, &phase_b]),
            Some(phase_commit_wins)
        );
        assert_eq!(
            select_highest_qc([&phase_b, &phase_a]),
            Some(phase_commit_wins)
        );

        let subject_low = QcRef {
            height: 2,
            view: 1,
            epoch: 0,
            subject_block_hash: prehashed_block_hash(1),
            phase: CertPhase::NewView,
        };
        let subject_high = QcRef {
            subject_block_hash: prehashed_block_hash(2),
            ..subject_low
        };
        let mut subject_a = certificate(CertPhase::NewView, round(2), base);
        subject_a.highest_qc = Some(subject_low);
        let mut subject_b = certificate(CertPhase::NewView, round(2), base);
        subject_b.highest_qc = Some(subject_high);

        assert_eq!(
            select_highest_qc([&subject_a, &subject_b]),
            Some(subject_high)
        );
        assert_eq!(
            select_highest_qc([&subject_b, &subject_a]),
            Some(subject_high)
        );

        let prepare = certificate(CertPhase::Prepare, round(2), base);
        let commit = certificate(CertPhase::Commit, round(2), base);
        assert_eq!(select_highest_qc([prepare, commit]), None);
    }

    #[test]
    fn commit_qc_waits_for_payload_before_finality() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let subject = subject(b"delayed");
        let expected_qc = QcRef {
            height: 1,
            view: 0,
            epoch: 0,
            subject_block_hash: subject.block_hash,
            phase: CertPhase::Commit,
        };
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                subject,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: subject.block_hash,
                payload_hash: subject.payload_hash,
            })]
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(subject));
        assert_eq!(engine.state().highest_qc, Some(expected_qc));

        assert_eq!(
            engine.handle(ConsensusInput::PayloadAvailable(subject)),
            vec![ConsensusOutput::CommitBlock { subject }]
        );
        assert_eq!(engine.state().pending_finality, None);
        assert_eq!(engine.state().highest_qc, Some(expected_qc));
    }

    #[test]
    fn commit_qcs_with_wrong_round_context_are_ignored() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let base = round(0);
        let mut wrong_height = base.clone();
        wrong_height.height = wrong_height.height.saturating_add(1);
        let mut wrong_epoch = base.clone();
        wrong_epoch.epoch = wrong_epoch.epoch.saturating_add(1);
        let mut wrong_validator_set = base;
        wrong_validator_set.validator_set_id = validator_set(b"other-commit-validators");

        for (round, label) in [
            (wrong_height, b"wrong-commit-height".as_slice()),
            (wrong_epoch, b"wrong-commit-epoch".as_slice()),
            (
                wrong_validator_set,
                b"wrong-commit-validator-set".as_slice(),
            ),
        ] {
            assert!(
                engine
                    .handle(ConsensusInput::Certificate(certificate(
                        CertPhase::Commit,
                        round,
                        subject(label),
                    )))
                    .is_empty()
            );
        }
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.state().pending_finality, None);
        assert_eq!(engine.state().highest_qc, None);
        assert_eq!(engine.committed_at(1), None);
    }

    #[test]
    fn pending_finality_ignores_payload_hash_mismatch_until_exact_payload_arrives() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let subject = subject(b"delayed-with-wrong-payload-first");
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                subject,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: subject.block_hash,
                payload_hash: subject.payload_hash,
            })]
        );

        let mut wrong_payload = subject;
        wrong_payload.payload_hash = Hash::new(b"wrong-payload-hash");
        assert!(
            engine
                .handle(ConsensusInput::PayloadAvailable(wrong_payload))
                .is_empty()
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(subject));
        assert_eq!(engine.committed_at(1), None);

        assert_eq!(
            engine.handle(ConsensusInput::PayloadAvailable(subject)),
            vec![ConsensusOutput::CommitBlock { subject }]
        );
        assert_eq!(engine.committed_at(1), Some(subject.block_hash));
        assert_eq!(engine.state().pending_finality, None);
    }

    #[test]
    fn payload_availability_without_commit_qc_never_finalizes() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let subject = subject(b"payload-only");
        let expected_qc = QcRef {
            height: 1,
            view: 0,
            epoch: 0,
            subject_block_hash: subject.block_hash,
            phase: CertPhase::Commit,
        };

        assert!(
            engine
                .handle(ConsensusInput::PayloadAvailable(subject))
                .is_empty()
        );
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.committed_at(1), None);

        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                subject,
            ))),
            vec![ConsensusOutput::CommitBlock { subject }]
        );
        assert_eq!(engine.state().highest_qc, Some(expected_qc));
        assert_eq!(engine.state().pending_finality, None);
        assert_eq!(engine.committed_at(1), Some(subject.block_hash));
    }

    #[test]
    fn pending_commit_qc_replays_and_conflicts_do_not_refetch_payload() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let first = subject(b"first-pending-commit-qc");
        let conflicting = subject(b"conflicting-pending-commit-qc");
        let commit = certificate(CertPhase::Commit, round(0), first);

        assert_eq!(
            engine.handle(ConsensusInput::Certificate(commit.clone())),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: first.block_hash,
                payload_hash: first.payload_hash,
            })]
        );
        assert!(
            engine
                .handle(ConsensusInput::Certificate(commit))
                .is_empty()
        );
        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Commit,
                    round(0),
                    conflicting,
                )))
                .is_empty()
        );
        assert_eq!(engine.state().pending_finality, Some(first));
        assert!(
            engine
                .handle(ConsensusInput::PayloadAvailable(conflicting))
                .is_empty()
        );
        assert_eq!(
            engine.handle(ConsensusInput::PayloadAvailable(first)),
            vec![ConsensusOutput::CommitBlock { subject: first }]
        );
    }

    #[test]
    fn committed_commit_qc_replay_does_not_emit_duplicate_finality() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let subject = subject(b"replayed-after-commit");
        assert!(
            engine
                .handle(ConsensusInput::PayloadAvailable(subject))
                .is_empty()
        );
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                subject,
            ))),
            vec![ConsensusOutput::CommitBlock { subject }]
        );
        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Commit,
                    round(0),
                    subject,
                )))
                .is_empty()
        );
    }

    #[test]
    fn pending_finality_rejects_payload_hash_and_subject_replays_without_dropping_qc() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let subject = subject(b"pending");
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                subject,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: subject.block_hash,
                payload_hash: subject.payload_hash,
            })]
        );

        let mut wrong_payload = subject;
        wrong_payload.payload_hash = Hash::new(b"wrong-payload");
        assert!(
            engine
                .handle(ConsensusInput::PayloadAvailable(wrong_payload))
                .is_empty()
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);

        let mut replayed_parent = subject;
        replayed_parent.parent_block = block_hash(b"replayed-parent");
        assert!(
            engine
                .handle(ConsensusInput::PayloadAvailable(replayed_parent))
                .is_empty()
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);

        assert_eq!(
            engine.handle(ConsensusInput::PayloadAvailable(subject)),
            vec![ConsensusOutput::CommitBlock { subject }]
        );
    }

    #[test]
    fn pending_finality_survives_timeout_and_view_change_noise() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let pending = subject(b"pending-across-timeout");
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                pending,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: pending.block_hash,
                payload_hash: pending.payload_hash,
            })]
        );
        let commit_qc = engine.state().highest_qc.expect("commit QC recorded");
        assert_eq!(engine.state().pending_finality, Some(pending));

        assert_eq!(
            engine.handle(ConsensusInput::Tick { now_ms: 10 }),
            vec![
                ConsensusOutput::SignVote {
                    phase: CertPhase::NewView,
                    round: round(1),
                    subject: qc_subject(commit_qc),
                    highest_qc: Some(commit_qc),
                },
                ConsensusOutput::AdvanceView { round: round(1) },
            ]
        );
        assert_eq!(engine.state().pending_finality, Some(pending));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);

        let mut new_view = certificate(CertPhase::NewView, round(2), pending);
        new_view.highest_qc = Some(commit_qc);
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(new_view)),
            vec![ConsensusOutput::AdvanceView { round: round(2) }]
        );
        assert_eq!(engine.state().pending_finality, Some(pending));

        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Commit,
                    round(2),
                    subject(b"competing-after-pending-timeout"),
                )))
                .is_empty()
        );
        assert_eq!(engine.state().pending_finality, Some(pending));
        assert_eq!(engine.committed_at(1), None);

        assert_eq!(
            engine.handle(ConsensusInput::PayloadAvailable(pending)),
            vec![ConsensusOutput::CommitBlock { subject: pending }]
        );
        assert_eq!(engine.committed_at(1), Some(pending.block_hash));
        assert_eq!(engine.state().pending_finality, None);
    }

    #[test]
    fn future_round_certificates_do_not_move_local_phase() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let mut future_round = round(0);
        future_round.height += 1;

        assert!(
            engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Prepare,
                    future_round,
                    subject(b"future"),
                )))
                .is_empty()
        );
        assert_eq!(engine.state().round, round(0));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.state().locked_qc, None);
    }

    #[test]
    fn validation_results_for_unknown_or_completed_proposals_do_not_force_view_change() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let proposal = subject(b"validated");
        assert_eq!(
            engine.handle(ConsensusInput::Proposal(EngineProposal {
                round: round(0),
                subject: proposal,
                highest_qc: None,
            })),
            vec![
                ConsensusOutput::ValidateBlock { subject: proposal },
                ConsensusOutput::SignVote {
                    phase: CertPhase::Prepare,
                    round: round(0),
                    subject: proposal,
                    highest_qc: None,
                },
            ]
        );

        assert!(
            engine
                .handle(ConsensusInput::ValidationResult {
                    round: round(0),
                    block_hash: block_hash(b"not-the-proposal"),
                    valid: false,
                })
                .is_empty()
        );
        assert_eq!(engine.state().round, round(0));
        assert_eq!(engine.state().phase, EnginePhase::Prepare);

        assert!(
            engine
                .handle(ConsensusInput::ValidationResult {
                    round: round(0),
                    block_hash: proposal.block_hash,
                    valid: true,
                })
                .is_empty()
        );
        assert!(
            engine
                .handle(ConsensusInput::ValidationResult {
                    round: round(0),
                    block_hash: proposal.block_hash,
                    valid: false,
                })
                .is_empty()
        );
        assert_eq!(engine.state().round, round(0));
        assert_eq!(engine.state().phase, EnginePhase::Prepare);
    }

    #[test]
    fn timeout_clears_inflight_validation_before_late_failure_arrives() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let proposal = subject(b"timeout-clears-validation");
        assert!(
            !engine
                .handle(ConsensusInput::Proposal(EngineProposal {
                    round: round(0),
                    subject: proposal,
                    highest_qc: None,
                }))
                .is_empty()
        );

        assert_eq!(
            engine.handle(ConsensusInput::Tick { now_ms: 1 }),
            vec![
                ConsensusOutput::SignVote {
                    phase: CertPhase::NewView,
                    round: round(1),
                    subject: zero_subject(),
                    highest_qc: None,
                },
                ConsensusOutput::AdvanceView { round: round(1) },
            ]
        );
        assert!(
            engine
                .handle(ConsensusInput::ValidationResult {
                    round: round(0),
                    block_hash: proposal.block_hash,
                    valid: false,
                })
                .is_empty()
        );
        assert_eq!(engine.state().round, round(1));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
    }

    #[test]
    fn tick_binds_highest_qc_and_clears_inflight_validation() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let locked = subject(b"tick-highest-qc");
        assert!(
            !engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Prepare,
                    round(0),
                    locked,
                )))
                .is_empty()
        );
        let locked_qc = engine.state().highest_qc.expect("highest QC recorded");

        let mut new_view = certificate(CertPhase::NewView, round(1), locked);
        new_view.highest_qc = Some(locked_qc);
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(new_view)),
            vec![ConsensusOutput::AdvanceView { round: round(1) }]
        );

        let proposal = subject(b"tick-clears-validation");
        let unlock_qc = QcRef {
            view: locked_qc.view.saturating_add(1),
            ..locked_qc
        };
        assert_eq!(
            engine.handle(ConsensusInput::Proposal(EngineProposal {
                round: round(1),
                subject: proposal,
                highest_qc: Some(unlock_qc),
            })),
            vec![
                ConsensusOutput::ValidateBlock { subject: proposal },
                ConsensusOutput::SignVote {
                    phase: CertPhase::Prepare,
                    round: round(1),
                    subject: proposal,
                    highest_qc: None,
                },
            ]
        );

        assert_eq!(
            engine.handle(ConsensusInput::Tick { now_ms: 2 }),
            vec![
                ConsensusOutput::SignVote {
                    phase: CertPhase::NewView,
                    round: round(2),
                    subject: qc_subject(locked_qc),
                    highest_qc: Some(locked_qc),
                },
                ConsensusOutput::AdvanceView { round: round(2) },
            ]
        );
        assert_eq!(engine.state().round, round(2));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.state().highest_qc, Some(locked_qc));

        assert!(
            engine
                .handle(ConsensusInput::ValidationResult {
                    round: round(1),
                    block_hash: proposal.block_hash,
                    valid: false,
                })
                .is_empty()
        );
        assert_eq!(engine.state().round, round(2));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
    }

    #[test]
    fn invalid_validation_new_view_vote_uses_highest_qc_subject() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let locked = subject(b"highest-qc-subject");
        assert!(
            !engine
                .handle(ConsensusInput::Certificate(certificate(
                    CertPhase::Prepare,
                    round(0),
                    locked,
                )))
                .is_empty()
        );
        let mut new_view = certificate(CertPhase::NewView, round(1), locked);
        new_view.highest_qc = engine.state().highest_qc;
        assert!(
            !engine
                .handle(ConsensusInput::Certificate(new_view))
                .is_empty()
        );

        let invalid = subject(b"invalid-proposal-with-highest-qc");
        let unlock_qc = QcRef {
            view: engine
                .state()
                .locked_qc
                .expect("lock exists")
                .view
                .saturating_add(1),
            ..engine.state().locked_qc.expect("lock exists")
        };
        assert!(
            !engine
                .handle(ConsensusInput::Proposal(EngineProposal {
                    round: round(1),
                    subject: invalid,
                    highest_qc: Some(unlock_qc),
                }))
                .is_empty()
        );

        assert_eq!(
            engine.handle(ConsensusInput::ValidationResult {
                round: round(1),
                block_hash: invalid.block_hash,
                valid: false,
            }),
            vec![
                ConsensusOutput::SignVote {
                    phase: CertPhase::NewView,
                    round: round(2),
                    subject: BlockSubject {
                        parent_block: locked.block_hash,
                        block_hash: locked.block_hash,
                        payload_hash: Hash::prehashed([0; Hash::LENGTH]),
                    },
                    highest_qc: engine.state().highest_qc,
                },
                ConsensusOutput::AdvanceView { round: round(2) },
            ]
        );
    }

    #[test]
    fn invalid_validation_result_for_current_proposal_advances_view_once() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let proposal = subject(b"invalid-proposal");
        assert!(
            !engine
                .handle(ConsensusInput::Proposal(EngineProposal {
                    round: round(0),
                    subject: proposal,
                    highest_qc: None,
                }))
                .is_empty()
        );

        assert_eq!(
            engine.handle(ConsensusInput::ValidationResult {
                round: round(0),
                block_hash: proposal.block_hash,
                valid: false,
            }),
            vec![
                ConsensusOutput::SignVote {
                    phase: CertPhase::NewView,
                    round: round(1),
                    subject: BlockSubject {
                        parent_block: proposal.block_hash,
                        block_hash: proposal.block_hash,
                        payload_hash: Hash::prehashed([0; Hash::LENGTH]),
                    },
                    highest_qc: None,
                },
                ConsensusOutput::AdvanceView { round: round(1) },
            ]
        );
        assert!(
            engine
                .handle(ConsensusInput::ValidationResult {
                    round: round(0),
                    block_hash: proposal.block_hash,
                    valid: false,
                })
                .is_empty()
        );
        assert_eq!(engine.state().round, round(1));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
    }

    #[test]
    fn commit_qc_supersedes_late_invalid_validation_result() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let proposal = subject(b"commit-qc-before-validation");
        assert_eq!(
            engine.handle(ConsensusInput::Proposal(EngineProposal {
                round: round(0),
                subject: proposal,
                highest_qc: None,
            })),
            vec![
                ConsensusOutput::ValidateBlock { subject: proposal },
                ConsensusOutput::SignVote {
                    phase: CertPhase::Prepare,
                    round: round(0),
                    subject: proposal,
                    highest_qc: None,
                },
            ]
        );
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                proposal,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: proposal.block_hash,
                payload_hash: proposal.payload_hash,
            })]
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(proposal));

        assert!(
            engine
                .handle(ConsensusInput::ValidationResult {
                    round: round(0),
                    block_hash: proposal.block_hash,
                    valid: false,
                })
                .is_empty()
        );
        assert_eq!(engine.state().round, round(0));
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(proposal));
        assert_eq!(engine.committed_at(1), None);
    }

    #[test]
    fn conflicting_commit_qc_supersedes_late_invalid_validation_result() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let proposal = subject(b"conflicting-commit-qc-proposal");
        let certified = subject(b"conflicting-commit-qc-certified");
        assert_eq!(
            engine.handle(ConsensusInput::Proposal(EngineProposal {
                round: round(0),
                subject: proposal,
                highest_qc: None,
            })),
            vec![
                ConsensusOutput::ValidateBlock { subject: proposal },
                ConsensusOutput::SignVote {
                    phase: CertPhase::Prepare,
                    round: round(0),
                    subject: proposal,
                    highest_qc: None,
                },
            ]
        );
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                certified,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: certified.block_hash,
                payload_hash: certified.payload_hash,
            })]
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(certified));
        assert_eq!(
            engine.state().highest_qc.map(|qc| qc.subject_block_hash),
            Some(certified.block_hash)
        );

        assert!(
            engine
                .handle(ConsensusInput::ValidationResult {
                    round: round(0),
                    block_hash: proposal.block_hash,
                    valid: false,
                })
                .is_empty()
        );
        assert_eq!(engine.state().round, round(0));
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(certified));
        assert_eq!(engine.committed_at(1), None);
    }

    #[test]
    fn committed_block_notification_supersedes_late_invalid_validation_result() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let proposal = subject(b"committed-before-validation");
        assert!(
            !engine
                .handle(ConsensusInput::Proposal(EngineProposal {
                    round: round(0),
                    subject: proposal,
                    highest_qc: None,
                }))
                .is_empty()
        );

        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash: proposal.block_hash,
                    reconfiguration: None,
                })
                .is_empty()
        );
        assert_eq!(engine.committed_at(1), Some(proposal.block_hash));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);

        assert!(
            engine
                .handle(ConsensusInput::ValidationResult {
                    round: round(0),
                    block_hash: proposal.block_hash,
                    valid: false,
                })
                .is_empty()
        );
        assert_eq!(engine.state().round, round(0));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.committed_at(1), Some(proposal.block_hash));
    }

    #[test]
    fn committed_block_notification_clears_matching_pending_finality() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let pending = subject(b"committed-notification-clears-pending");
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                pending,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: pending.block_hash,
                payload_hash: pending.payload_hash,
            })]
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(pending));

        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash: pending.block_hash,
                    reconfiguration: None,
                })
                .is_empty()
        );
        assert_eq!(engine.committed_at(1), Some(pending.block_hash));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.state().pending_finality, None);
        assert!(
            engine
                .handle(ConsensusInput::PayloadAvailable(pending))
                .is_empty(),
            "storage finality already superseded the pending fetch"
        );
    }

    #[test]
    fn conflicting_committed_block_notification_clears_pending_finality() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let pending = subject(b"pending-before-conflicting-storage-commit");
        let committed = block_hash(b"conflicting-storage-commit");
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                pending,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: pending.block_hash,
                payload_hash: pending.payload_hash,
            })]
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(pending));

        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash: committed,
                    reconfiguration: None,
                })
                .is_empty()
        );
        assert_eq!(engine.committed_at(1), Some(committed));
        assert_eq!(engine.state().phase, EnginePhase::Proposal);
        assert_eq!(engine.state().pending_finality, None);
        assert!(
            engine
                .handle(ConsensusInput::PayloadAvailable(pending))
                .is_empty(),
            "late payload for the superseded pending finality must not commit"
        );
        assert_eq!(engine.committed_at(1), Some(committed));
    }

    #[test]
    fn committed_block_notification_for_other_height_does_not_clear_pending_finality() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let pending = subject(b"pending-survives-other-height-commit");
        let mut other_round = round(0);
        other_round.height = other_round.height.saturating_add(1);
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                pending,
            ))),
            vec![ConsensusOutput::FetchPayload(PayloadRequest {
                round: round(0),
                block_hash: pending.block_hash,
                payload_hash: pending.payload_hash,
            })]
        );

        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: other_round,
                    block_hash: block_hash(b"other-height-commit"),
                    reconfiguration: None,
                })
                .is_empty()
        );
        assert_eq!(engine.state().phase, EnginePhase::PendingFinality);
        assert_eq!(engine.state().pending_finality, Some(pending));

        assert_eq!(
            engine.handle(ConsensusInput::PayloadAvailable(pending)),
            vec![ConsensusOutput::CommitBlock { subject: pending }]
        );
        assert_eq!(engine.committed_at(1), Some(pending.block_hash));
    }

    #[test]
    fn committed_block_notifications_do_not_overwrite_conflicting_height() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let first = block_hash(b"first-commit");
        let second = block_hash(b"second-commit");

        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash: first,
                    reconfiguration: None,
                })
                .is_empty()
        );
        assert_eq!(engine.committed_at(1), Some(first));

        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash: second,
                    reconfiguration: None,
                })
                .is_empty()
        );
        assert_eq!(engine.committed_at(1), Some(first));
    }

    #[test]
    fn conflicting_committed_block_notification_cannot_activate_reconfiguration() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let first = block_hash(b"first-reconfig-commit");
        let conflicting = block_hash(b"conflicting-reconfig-commit");
        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash: first,
                    reconfiguration: None,
                })
                .is_empty()
        );

        let change = ValidatorSetChange {
            activation_height: 2,
            validator_set_id: validator_set(b"conflicting-next-set"),
            quorum_policy: QuorumPolicy::PermissionedCount(5),
        };
        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash: conflicting,
                    reconfiguration: Some(change),
                })
                .is_empty()
        );
        assert_eq!(engine.committed_at(1), Some(first));
    }

    #[test]
    fn reconfiguration_activates_only_after_old_set_finality() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let change = ValidatorSetChange {
            activation_height: 2,
            validator_set_id: validator_set(b"next"),
            quorum_policy: QuorumPolicy::PermissionedCount(5),
        };
        assert_eq!(
            engine.handle(ConsensusInput::CommittedBlock {
                round: round(0),
                block_hash: block_hash(b"reconfig-block"),
                reconfiguration: Some(change.clone()),
            }),
            vec![ConsensusOutput::ActivateValidatorSet(change)]
        );
    }

    #[test]
    fn duplicate_committed_block_notification_does_not_reactivate_reconfiguration() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let block_hash = block_hash(b"reconfig-idempotent");
        let change = ValidatorSetChange {
            activation_height: 2,
            validator_set_id: validator_set(b"idempotent-next"),
            quorum_policy: QuorumPolicy::PermissionedCount(5),
        };

        assert_eq!(
            engine.handle(ConsensusInput::CommittedBlock {
                round: round(0),
                block_hash,
                reconfiguration: Some(change.clone()),
            }),
            vec![ConsensusOutput::ActivateValidatorSet(change.clone())]
        );
        assert_eq!(engine.committed_at(1), Some(block_hash));

        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash,
                    reconfiguration: Some(change),
                })
                .is_empty()
        );
        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash,
                    reconfiguration: None,
                })
                .is_empty()
        );
        assert_eq!(engine.committed_at(1), Some(block_hash));
    }

    #[test]
    fn reconfiguration_with_non_boundary_activation_is_not_activated() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let future_change = ValidatorSetChange {
            activation_height: 3,
            validator_set_id: validator_set(b"too-far"),
            quorum_policy: QuorumPolicy::PermissionedCount(5),
        };
        assert!(
            engine
                .handle(ConsensusInput::CommittedBlock {
                    round: round(0),
                    block_hash: block_hash(b"non-boundary-reconfig"),
                    reconfiguration: Some(future_change),
                })
                .is_empty()
        );
        assert_eq!(
            engine.handle(ConsensusInput::Certificate(certificate(
                CertPhase::Commit,
                round(0),
                subject(b"already-committed-height"),
            ))),
            Vec::new(),
        );
    }
}
