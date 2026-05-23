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
            || !self.proposal_satisfies_lock(&proposal)
        {
            return Vec::new();
        }
        self.state.phase = EnginePhase::Prepare;
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
        if let Some(committed) = self.committed.get(&certificate.round.height)
            && committed != &certificate.subject.block_hash
        {
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
        let qc = qc_ref_from_certificate(&certificate);
        self.state.locked_qc = Some(qc);
        self.record_highest_qc(qc);
        self.state.phase = EnginePhase::Commit;
        vec![ConsensusOutput::SignVote {
            phase: CertPhase::Commit,
            round: certificate.round,
            subject: certificate.subject,
            highest_qc: None,
        }]
    }

    fn on_commit_qc(&mut self, certificate: Certificate) -> Vec<ConsensusOutput> {
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
        if let Some(highest) = certificate.highest_qc {
            self.record_highest_qc(highest);
        }
        if certificate.round.view <= self.state.round.view {
            return Vec::new();
        }
        self.state.round = certificate.round;
        self.state.phase = EnginePhase::Proposal;
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
        if valid || round != self.state.round {
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
        if let Some(committed) = self.committed.get(&round.height)
            && committed != &block_hash
        {
            return Vec::new();
        }
        self.committed.insert(round.height, block_hash);
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

        let prepare = certificate(CertPhase::Prepare, round(2), base);
        let commit = certificate(CertPhase::Commit, round(2), base);
        assert_eq!(select_highest_qc([prepare, commit]), None);
    }

    #[test]
    fn commit_qc_waits_for_payload_before_finality() {
        let mut engine = ConsensusEngine::new(round(0), QuorumPolicy::PermissionedCount(4));
        let subject = subject(b"delayed");
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

        assert_eq!(
            engine.handle(ConsensusInput::PayloadAvailable(subject)),
            vec![ConsensusOutput::CommitBlock { subject }]
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
}
