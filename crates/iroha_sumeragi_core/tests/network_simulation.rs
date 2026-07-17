//! Deterministic multi-validator simulations for the production Sumeragi v2 reducer.
//!
//! The harness deliberately keeps networking, signatures, body storage, and
//! validation outside the reducer. The network scenarios execute adapters
//! synchronously behind a deterministic lossy, duplicating, and reordering
//! scheduler. The accelerated chain-prefix gate instead queues every local
//! completion and injects deterministic WAL/replay interruptions; its QCs and
//! TCs are externally supplied fixtures, so it does not claim quorum formation.

use std::collections::{BTreeSet, VecDeque};

use iroha_sumeragi_core::{
    BodyState, CertificateRef, ChainId, ConsensusMessageV2, ContextId, Digest,
    DurableCommitReceipt, Effect, EquivocationKind, Event, EventTag, Generation, HeightContext,
    IgnoreReason, OpaqueSignature, PayloadManifest, Phase, Quorum, QuorumCertificate, QuorumError,
    Reducer, ReducerError, Round, SignableMessage, SignatureShare, SignedVote, StepDisposition,
    Subject, TimeoutCertificate, TimeoutSignatureGroup, Validator, ValidatorId, Vote, VotingMode,
    VotingPower, WalEntry, WalRecord,
};

const HEIGHT: u64 = 42;
const ACCELERATED_CHAOS_HEIGHTS: u64 = 100_000;
const ACCELERATED_CHAOS_SMOKE_HEIGHTS: u64 = 320;
const ACCELERATED_RESTART_INTERVAL: u64 = 64;
const ACCELERATED_DUPLICATE_INTERVAL: u64 = 32;
const ACCELERATED_UNDER_QUORUM_INTERVAL: u64 = 97;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AcceleratedChaosStats {
    completed_heights: u64,
    finalized_validators: u64,
    supplied_commit_qcs: u64,
    supplied_tcs: u64,
    wal_append_restarts: u64,
    fetch_restarts: u64,
    store_restarts: u64,
    validation_restarts: u64,
    application_restarts: u64,
    stale_generation_rejections: u64,
    deferred_fetch_completions: u64,
    deferred_store_completions: u64,
    deferred_validation_completions: u64,
    deferred_application_completions: u64,
    duplicate_commit_qcs: u64,
    reordered_commit_batches: u64,
    reordered_tc_batches: u64,
    insufficient_dual_qcs: u64,
    count_only_qcs: u64,
    power_only_qcs: u64,
}

impl AcceleratedChaosStats {
    fn merge(&mut self, other: Self) {
        self.completed_heights += other.completed_heights;
        self.finalized_validators += other.finalized_validators;
        self.supplied_commit_qcs += other.supplied_commit_qcs;
        self.supplied_tcs += other.supplied_tcs;
        self.wal_append_restarts += other.wal_append_restarts;
        self.fetch_restarts += other.fetch_restarts;
        self.store_restarts += other.store_restarts;
        self.validation_restarts += other.validation_restarts;
        self.application_restarts += other.application_restarts;
        self.stale_generation_rejections += other.stale_generation_rejections;
        self.deferred_fetch_completions += other.deferred_fetch_completions;
        self.deferred_store_completions += other.deferred_store_completions;
        self.deferred_validation_completions += other.deferred_validation_completions;
        self.deferred_application_completions += other.deferred_application_completions;
        self.duplicate_commit_qcs += other.duplicate_commit_qcs;
        self.reordered_commit_batches += other.reordered_commit_batches;
        self.reordered_tc_batches += other.reordered_tc_batches;
        self.insufficient_dual_qcs += other.insufficient_dual_qcs;
        self.count_only_qcs += other.count_only_qcs;
        self.power_only_qcs += other.power_only_qcs;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AcceleratedRestartPoint {
    WalAppended,
    FetchPending,
    StorePending,
    ValidationPending,
    ApplicationPending,
}

struct AcceleratedNode {
    reducer: Reducer,
    wal: Vec<WalEntry>,
    pending: VecDeque<Effect>,
}

#[derive(Clone)]
struct Envelope {
    from: usize,
    to: usize,
    message: ConsensusMessageV2,
    copy: u8,
    impair_first_copy: bool,
}

struct Node {
    reducer: Reducer,
    wal: Vec<WalEntry>,
    applied: Vec<Subject>,
    online: bool,
    corrupt_fetches_remaining: usize,
    dropped_signatures_remaining: usize,
}

#[derive(Default)]
struct NetworkStats {
    impaired_drops: usize,
    offline_drops: usize,
    reordered_deliveries: usize,
    duplicate_deliveries: usize,
    partition_drops: usize,
    ignored_inputs: usize,
    equivocations: usize,
    corrupted_chunks: usize,
    withheld_commit_messages: usize,
    crashed_signatures: usize,
}

struct Simulation {
    context: HeightContext,
    nodes: Vec<Node>,
    network: Vec<Envelope>,
    scheduler_state: u64,
    broadcasts: usize,
    signatures: u64,
    partition: Option<Vec<usize>>,
    directed_partition_drops: BTreeSet<(usize, usize)>,
    withhold_commit_traffic: bool,
    stats: NetworkStats,
}

impl Simulation {
    fn new(validator_count: usize, mode: VotingMode, offline: Option<usize>) -> Self {
        let context = context(validator_count, mode);
        let nodes = context
            .roster()
            .iter()
            .enumerate()
            .map(|(index, validator)| Node {
                reducer: Reducer::new(context.clone(), Some(validator.id()), Generation::new(1))
                    .expect("fixture validator belongs to the frozen roster"),
                wal: Vec::new(),
                applied: Vec::new(),
                online: offline != Some(index),
                corrupt_fetches_remaining: 0,
                dropped_signatures_remaining: 0,
            })
            .collect();
        Self {
            context,
            nodes,
            network: Vec::new(),
            scheduler_state: 0x5eed_5eed_cafe_babe,
            broadcasts: 0,
            signatures: 0,
            partition: None,
            directed_partition_drops: BTreeSet::new(),
            withhold_commit_traffic: false,
            stats: NetworkStats::default(),
        }
    }

    fn online_indices(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| node.online.then_some(index))
            .collect()
    }

    fn dispatch(&mut self, index: usize, event: Event) -> StepDisposition {
        let outcome = self.nodes[index]
            .reducer
            .step(event)
            .unwrap_or_else(|error| panic!("node {index} rejected a simulator event: {error}"));
        let disposition = outcome.disposition();
        if matches!(disposition, StepDisposition::Ignored(_)) {
            self.stats.ignored_inputs += 1;
        }
        self.drive_effects(index, outcome.into_effects());
        self.assert_agreement();
        disposition
    }

    #[allow(clippy::too_many_lines)]
    fn drive_effects(&mut self, index: usize, effects: Vec<Effect>) {
        let mut pending: VecDeque<_> = effects.into();
        while let Some(effect) = pending.pop_front() {
            let follow_up = match effect {
                Effect::Persist { tag, entry } => {
                    self.nodes[index].wal.push(entry.clone());
                    self.nodes[index]
                        .reducer
                        .step(Event::Persisted {
                            tag,
                            id: entry.id(),
                        })
                        .expect("the in-memory WAL acknowledges the exact requested frame")
                        .into_effects()
                }
                Effect::Sign { tag, message } => {
                    if self.nodes[index].dropped_signatures_remaining > 0 {
                        self.nodes[index].dropped_signatures_remaining -= 1;
                        self.stats.crashed_signatures += 1;
                        continue;
                    }
                    self.signatures += 1;
                    let signature = simulator_signature(
                        self.nodes[index]
                            .reducer
                            .local_validator()
                            .expect("simulated nodes are validators"),
                        self.signatures,
                        &message,
                    );
                    self.nodes[index]
                        .reducer
                        .step(Event::Signed { tag, signature })
                        .expect("signature completes the reducer's sole outstanding request")
                        .into_effects()
                }
                Effect::Broadcast(message) => {
                    self.enqueue_broadcast(index, &message);
                    Vec::new()
                }
                Effect::FetchBody {
                    tag,
                    round,
                    subject,
                    ..
                } => {
                    if self.nodes[index].corrupt_fetches_remaining > 0 {
                        self.nodes[index].corrupt_fetches_remaining -= 1;
                        self.stats.corrupted_chunks += 1;
                        Vec::new()
                    } else {
                        self.nodes[index]
                            .reducer
                            .step(Event::BodyAvailable {
                                tag,
                                round,
                                subject,
                            })
                            .expect("the deterministic body source has the requested subject")
                            .into_effects()
                    }
                }
                Effect::StoreBody {
                    tag,
                    round,
                    subject,
                } => self.nodes[index]
                    .reducer
                    .step(Event::BodyStored {
                        tag,
                        round,
                        subject,
                    })
                    .expect("the test body store durably acknowledges the body")
                    .into_effects(),
                Effect::ValidateBody {
                    tag,
                    round,
                    subject,
                } => self.nodes[index]
                    .reducer
                    .step(Event::ValidationCompleted {
                        tag,
                        round,
                        subject,
                        valid: true,
                    })
                    .expect("the deterministic validator accepts the fixture body")
                    .into_effects(),
                Effect::Apply {
                    tag,
                    subject,
                    certificate,
                } => {
                    assert_eq!(certificate.subject(), subject);
                    self.nodes[index].applied.push(subject);
                    self.nodes[index]
                        .reducer
                        .step(Event::ApplicationCompleted { tag, subject })
                        .expect("application completion matches the durable decision")
                        .into_effects()
                }
                Effect::EnterView {
                    tag, certificate, ..
                } => {
                    assert_eq!(tag, self.nodes[index].reducer.current_tag());
                    assert_eq!(tag.view(), certificate.round().view() + 1);
                    Vec::new()
                }
                Effect::ReportEquivocation { evidence } => {
                    assert_eq!(evidence.kind(), EquivocationKind::Vote);
                    self.stats.equivocations += 1;
                    Vec::new()
                }
                Effect::ReportInvalidCertifiedBody { .. } => {
                    panic!("all simulator bodies are deterministically valid")
                }
            };
            pending.extend(follow_up);
        }
    }

    fn enqueue_broadcast(&mut self, from: usize, message: &ConsensusMessageV2) {
        let broadcast = self.broadcasts;
        self.broadcasts += 1;
        for to in 0..self.nodes.len() {
            let impair_first_copy = (broadcast + from * 3 + to * 5).is_multiple_of(4);
            let copies = if (broadcast + from + to).is_multiple_of(5) {
                2
            } else {
                1
            };
            for copy in 0..copies {
                self.network.push(Envelope {
                    from,
                    to,
                    message: message.clone(),
                    copy: u8::try_from(copy).expect("at most two simulator copies"),
                    impair_first_copy,
                });
            }
        }
    }

    fn drain_network(&mut self) {
        let mut remaining_budget = 200_000_usize;
        while !self.network.is_empty() {
            assert!(
                remaining_budget > 0,
                "deterministic network failed to quiesce"
            );
            remaining_budget -= 1;
            self.scheduler_state = self
                .scheduler_state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let network_len = u64::try_from(self.network.len())
                .expect("the bounded simulator queue length fits in u64");
            let selected = usize::try_from(self.scheduler_state % network_len)
                .expect("the selected queue index fits in usize");
            if selected != 0 {
                self.stats.reordered_deliveries += 1;
            }
            let envelope = self.network.swap_remove(selected);
            let is_commit_traffic = matches!(
                &envelope.message,
                ConsensusMessageV2::Vote(vote) if vote.vote().phase() == Phase::Commit
            ) || matches!(
                &envelope.message,
                ConsensusMessageV2::QuorumCertificate(certificate)
                    if certificate.phase() == Phase::Commit
            );
            if self.withhold_commit_traffic && is_commit_traffic {
                self.stats.withheld_commit_messages += 1;
                continue;
            }
            if !self.nodes[envelope.to].online {
                self.stats.offline_drops += 1;
                continue;
            }
            if self
                .partition
                .as_ref()
                .is_some_and(|groups| groups.get(envelope.from) != groups.get(envelope.to))
                || self
                    .directed_partition_drops
                    .contains(&(envelope.from, envelope.to))
            {
                self.stats.partition_drops += 1;
                continue;
            }
            if envelope.copy == 0 && envelope.impair_first_copy {
                self.stats.impaired_drops += 1;
                continue;
            }
            if envelope.copy == 1 {
                self.stats.duplicate_deliveries += 1;
            }
            let event = network_event(
                &envelope.message,
                self.nodes[envelope.to].reducer.current_tag(),
            );
            self.dispatch(envelope.to, event);
        }
    }

    fn timeout_all_online(&mut self) {
        for index in self.online_indices() {
            let tag = self.nodes[index].reducer.current_tag();
            self.dispatch(index, Event::TimeoutElapsed { tag });
        }
        self.drain_network();
        self.retransmit_all_online(5);
    }

    fn install_partition(&mut self, groups: Vec<usize>) {
        assert_eq!(groups.len(), self.nodes.len());
        self.partition = Some(groups);
    }

    fn install_directed_partition(
        &mut self,
        dropped_links: impl IntoIterator<Item = (usize, usize)>,
    ) {
        self.directed_partition_drops = dropped_links.into_iter().collect();
    }

    fn heal_partition(&mut self) {
        self.partition = None;
        self.directed_partition_drops.clear();
    }

    fn restart(&mut self, index: usize) {
        let validator = self.nodes[index]
            .reducer
            .local_validator()
            .expect("simulated node is a validator");
        let generation = Generation::new(
            self.nodes[index]
                .reducer
                .current_tag()
                .generation()
                .get()
                .checked_add(1)
                .expect("simulation generation remains bounded"),
        );
        let mut reducer = Reducer::recover(
            self.context.clone(),
            Some(validator),
            generation,
            self.nodes[index].wal.clone(),
        )
        .expect("complete in-memory WAL replays");
        let tag = reducer.current_tag();
        let effects = reducer
            .step(Event::ResumeAfterReplay { tag })
            .expect("replay resumption passes the production refinement gate")
            .into_effects();
        self.nodes[index].reducer = reducer;
        self.nodes[index].online = true;
        self.drive_effects(index, effects);
    }

    fn retransmit_all_online(&mut self, rounds: usize) {
        for _ in 0..rounds {
            for index in self.online_indices() {
                let tag = self.nodes[index].reducer.current_tag();
                self.dispatch(index, Event::RetransmitElapsed { tag });
            }
            self.drain_network();
        }
    }

    fn inject_vote_equivocation(&mut self, signer: ValidatorId, subject: Subject) {
        let round = Round::new(self.context.height(), self.current_online_view());
        let conflicting = Subject::repeat(subject.as_bytes()[0].wrapping_add(0x40));
        assert_ne!(subject, conflicting);
        for index in self.online_indices() {
            let tag = self.nodes[index].reducer.current_tag();
            for value in [subject, conflicting] {
                let vote = Vote::new(self.context.id(), round, Phase::Prepare, value, signer);
                self.dispatch(
                    index,
                    Event::VoteReceived {
                        tag,
                        vote: SignedVote::new(
                            vote,
                            OpaqueSignature::new(vec![0xb7, value.as_bytes()[0]]),
                        ),
                    },
                );
            }
        }
    }

    fn propose(&mut self, subject: Subject) -> u64 {
        let (view, _leader_index) = self.begin_proposal(subject);
        self.drain_network();
        self.retransmit_all_online(5);
        view
    }

    fn begin_proposal(&mut self, subject: Subject) -> (u64, usize) {
        let view = self.current_online_view();
        let leader = self.context.leader(view);
        let leader_index = self
            .nodes
            .iter()
            .position(|node| node.reducer.local_validator() == Some(leader))
            .expect("leader belongs to simulated roster");
        assert!(self.nodes[leader_index].online);
        let tag = self.nodes[leader_index].reducer.current_tag();
        self.dispatch(
            leader_index,
            Event::LocalProposalReady {
                tag,
                manifest: manifest(subject),
            },
        );
        (view, leader_index)
    }

    fn install_tc(&mut self, index: usize, certificate: TimeoutCertificate) {
        let tag = self.nodes[index].reducer.current_tag();
        self.dispatch(
            index,
            Event::TimeoutCertificateReceived { tag, certificate },
        );
    }

    fn deliver_commit_qc(&mut self, index: usize, certificate: QuorumCertificate) {
        let tag = self.nodes[index].reducer.current_tag();
        self.dispatch(index, Event::QuorumCertificateReceived { tag, certificate });
    }

    fn current_online_view(&self) -> u64 {
        let mut views = self
            .nodes
            .iter()
            .filter(|node| node.online)
            .map(|node| node.reducer.current_tag().view());
        let first = views.next().expect("at least one validator remains online");
        assert!(
            views.all(|view| view == first),
            "online validators have not converged"
        );
        first
    }

    fn assert_agreement(&self) {
        let mut decided = self.nodes.iter().filter_map(|node| {
            node.reducer
                .durable_state()
                .decision()
                .map(QuorumCertificate::subject)
        });
        if let Some(first) = decided.next() {
            assert!(
                decided.all(|subject| subject == first),
                "conflicting decisions observed"
            );
        }
    }

    fn assert_online_committed(&self, expected: Subject) {
        for (index, node) in self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.online)
        {
            assert_eq!(
                node.reducer
                    .durable_state()
                    .decision()
                    .map(QuorumCertificate::subject),
                Some(expected),
                "online node {index} did not persist the common decision"
            );
            assert_eq!(node.applied.last(), Some(&expected));
        }
        self.assert_agreement();
    }
}

#[test]
fn lossy_offline_leader_simulations_commit_for_4_7_and_10_validators() {
    for validator_count in [4, 7, 10] {
        for mode in [VotingMode::Permissioned, VotingMode::Npos] {
            // A zero leader seed selects roster[0] in view zero.  Keeping that
            // validator offline forces a real dual-quorum TC and leader rotation.
            let mut simulation = Simulation::new(validator_count, mode, Some(0));
            assert_eq!(simulation.context.leader(0), validator_id(1));
            simulation.timeout_all_online();
            assert_eq!(simulation.current_online_view(), 1);
            for node in simulation.nodes.iter().filter(|node| node.online) {
                assert!(node.wal.iter().any(|entry| {
                    matches!(entry.record(), WalRecord::InstallTimeout(certificate) if certificate.round().view() == 0)
                }));
            }

            let subject = Subject::repeat(
                0x20 + u8::try_from(validator_count).expect("fixture size fits in u8")
                    + u8::from(mode == VotingMode::Npos),
            );
            // Validator 2 is below one third by both count and power in every
            // fixture. Its conflicting Prepare is reported and cannot produce
            // conflicting quorum certificates.
            simulation.inject_vote_equivocation(validator_id(2), subject);
            let successful_view = simulation.propose(subject);

            simulation.assert_online_committed(subject);
            assert!(successful_view <= validator_count as u64);
            assert!(simulation.stats.impaired_drops > 0);
            assert!(simulation.stats.offline_drops > 0);
            assert!(simulation.stats.reordered_deliveries > 0);
            assert!(simulation.stats.duplicate_deliveries > 0);
            assert!(simulation.stats.ignored_inputs > 0);
            assert!(simulation.stats.equivocations >= validator_count - 1);
        }
    }
}

#[test]
fn two_by_two_partition_cannot_advance_but_healing_retransmits_tc_and_commits() {
    let mut simulation = Simulation::new(4, VotingMode::Npos, None);
    simulation.install_partition(vec![0, 0, 1, 1]);

    simulation.timeout_all_online();
    assert_eq!(
        simulation
            .nodes
            .iter()
            .map(|node| node.reducer.current_tag().view())
            .collect::<Vec<_>>(),
        vec![0, 0, 0, 0],
        "neither half has the count quorum required to install a TC"
    );
    assert!(simulation.stats.partition_drops > 0);
    assert!(simulation.nodes.iter().all(|node| {
        node.reducer.durable_state().decision().is_none()
            && node.wal.iter().any(
                |entry| matches!(entry.record(), WalRecord::TimeoutIntent(vote) if vote.round().view() == 0),
            )
    }));

    simulation.heal_partition();
    simulation.retransmit_all_online(5);
    assert_eq!(simulation.current_online_view(), 1);
    assert!(simulation.nodes.iter().all(|node| {
        node.wal.iter().any(
            |entry| matches!(entry.record(), WalRecord::InstallTimeout(certificate) if certificate.round().view() == 0),
        )
    }));

    let subject = Subject::repeat(0x6d);
    simulation.propose(subject);
    simulation.assert_online_committed(subject);
}

#[test]
fn asymmetric_partition_stalls_without_dual_quorum_then_heals_and_applies() {
    let mut simulation = Simulation::new(4, VotingMode::Npos, None);
    simulation.install_directed_partition([(2, 0), (2, 1), (3, 0), (3, 1)]);

    let subject = Subject::repeat(0x6e);
    simulation.begin_proposal(subject);
    simulation.drain_network();
    simulation.retransmit_all_online(6);

    assert!(
        simulation
            .nodes
            .iter()
            .all(|node| node.reducer.durable_state().decision().is_none())
    );
    assert!(simulation.stats.partition_drops > 0);

    simulation.heal_partition();
    simulation.retransmit_all_online(6);
    simulation.assert_online_committed(subject);
}

#[test]
fn leader_crash_after_proposal_broadcast_does_not_block_the_remaining_quorum() {
    for mode in [VotingMode::Permissioned, VotingMode::Npos] {
        let mut simulation = Simulation::new(4, mode, None);
        let subject = Subject::repeat(0x79 + u8::from(mode == VotingMode::Npos));
        let (view, leader_index) = simulation.begin_proposal(subject);
        assert_eq!(view, 0);

        // The leader completed its durable proposal intent and broadcast, then
        // crashed before receiving its own proposal or contributing a Prepare.
        simulation.nodes[leader_index].online = false;
        simulation.drain_network();
        simulation.retransmit_all_online(5);

        simulation.assert_online_committed(subject);
        assert!(simulation.stats.offline_drops > 0);
    }
}

#[test]
fn leader_crash_with_a_locked_body_rotates_and_rebuilds_the_old_commit_quorum() {
    for (validator_count, mode) in [(4, VotingMode::Permissioned), (7, VotingMode::Npos)] {
        let mut simulation = Simulation::new(validator_count, mode, None);
        let subject = Subject::repeat(
            0x7c + u8::try_from(validator_count).expect("fixture size fits in u8")
                + u8::from(mode == VotingMode::Npos),
        );
        simulation.withhold_commit_traffic = true;

        let (locked_view, leader_index) = simulation.begin_proposal(subject);
        assert_eq!(locked_view, 0);
        simulation.drain_network();
        simulation.retransmit_all_online(5);

        let locked_round = Round::new(simulation.context.height(), locked_view);
        assert!(simulation.nodes.iter().all(|node| {
            node.reducer
                .durable_state()
                .locked()
                .map(QuorumCertificate::subject)
                == Some(subject)
                && node.reducer.body_state(locked_round, subject) == BodyState::Validated
                && node.reducer.durable_state().decision().is_none()
                && node.applied.is_empty()
        }));
        assert!(simulation.stats.withheld_commit_messages > 0);

        // The original leader crashes only after it has the same durable lock
        // and body pipeline as its peers. The responsive dual quorum must be
        // able to install a TC without it while Commit traffic is still held.
        simulation.nodes[leader_index].online = false;
        simulation.timeout_all_online();
        assert_eq!(simulation.current_online_view(), 1);
        assert!(
            simulation
                .nodes
                .iter()
                .filter(|node| node.online)
                .all(|node| node
                    .reducer
                    .durable_state()
                    .locked()
                    .map(QuorumCertificate::subject)
                    == Some(subject)
                    && node.reducer.body_state(locked_round, subject) == BodyState::Validated)
        );

        // Healing only the Commit lane models the exact reset-boundary
        // regression: retransmitted old-round votes must repopulate each new
        // volatile pool and finish the already locked body without the leader.
        simulation.withhold_commit_traffic = false;
        simulation.retransmit_all_online(6);
        simulation.assert_online_committed(subject);
    }
}

#[test]
fn corrupted_chunks_and_withheld_commit_evidence_recover_by_bounded_retransmission() {
    for validator_count in [4, 7, 10] {
        for mode in [VotingMode::Permissioned, VotingMode::Npos] {
            let mut simulation = Simulation::new(validator_count, mode, None);
            let leader = simulation.context.leader(0);
            let leader_index = simulation
                .nodes
                .iter()
                .position(|node| node.reducer.local_validator() == Some(leader))
                .expect("view-zero leader belongs to the roster");
            for (index, node) in simulation.nodes.iter_mut().enumerate() {
                if index != leader_index {
                    node.corrupt_fetches_remaining = 1;
                }
            }
            simulation.withhold_commit_traffic = true;
            let subject = Subject::repeat(
                0x90 + u8::try_from(validator_count).expect("fixture size fits u8")
                    + u8::from(mode == VotingMode::Npos),
            );

            simulation.begin_proposal(subject);
            simulation.drain_network();
            simulation.retransmit_all_online(4);
            assert!(
                simulation.nodes.iter().all(|node| node
                    .reducer
                    .durable_state()
                    .decision()
                    .is_none()),
                "withholding every Commit vote/QC must prevent a decision"
            );
            assert_eq!(simulation.stats.corrupted_chunks, validator_count - 1);
            assert!(simulation.stats.withheld_commit_messages > 0);

            simulation.withhold_commit_traffic = false;
            simulation.retransmit_all_online(6);
            simulation.assert_online_committed(subject);
        }
    }
}

#[test]
fn crash_after_proposal_wal_before_signature_replays_exact_intent() {
    for mode in [VotingMode::Permissioned, VotingMode::Npos] {
        let mut simulation = Simulation::new(4, mode, None);
        let leader = simulation.context.leader(0);
        let leader_index = simulation
            .nodes
            .iter()
            .position(|node| node.reducer.local_validator() == Some(leader))
            .expect("view-zero leader belongs to the roster");
        simulation.nodes[leader_index].dropped_signatures_remaining = 1;
        let old_tag = simulation.nodes[leader_index].reducer.current_tag();
        let subject = Subject::repeat(0xb0 + u8::from(mode == VotingMode::Npos));

        simulation.begin_proposal(subject);
        assert_eq!(simulation.stats.crashed_signatures, 1);
        assert!(simulation.nodes[leader_index].wal.iter().any(|entry| {
            matches!(entry.record(), WalRecord::ProposalIntent(proposal) if proposal.manifest().subject() == subject)
        }));
        assert!(simulation.network.is_empty());

        simulation.nodes[leader_index].online = false;
        simulation.restart(leader_index);
        assert!(matches!(
            simulation.dispatch(
                leader_index,
                Event::Signed {
                    tag: old_tag,
                    signature: OpaqueSignature::new(vec![0xff]),
                },
            ),
            StepDisposition::Ignored(IgnoreReason::StaleGeneration)
        ));
        simulation.drain_network();
        simulation.retransmit_all_online(5);
        simulation.assert_online_committed(subject);
    }
}

#[test]
fn taira_divergent_views_converge_and_commit_within_one_rotation() {
    let mut simulation = Simulation::new(4, VotingMode::Npos, None);
    let subject = Subject::repeat(0x71);
    let prepare = certificate(&simulation.context, 0, Phase::Prepare, subject);
    let tc0 = grouped_timeout(&simulation.context, 0, Some(prepare.clone()));
    let tc1 = grouped_timeout(&simulation.context, 1, Some(prepare.clone()));
    let tc2 = grouped_timeout(&simulation.context, 2, Some(prepare.clone()));

    // Recreate the captured incident shape at height H+1: one validator still
    // reports view 0 without the QC, another has installed view 1, and a third
    // is at view 2. The QC is carried by the grouped TCs rather than reacquired
    // through a separate missing-QC state machine.
    simulation.install_tc(1, tc0);
    simulation.install_tc(2, tc1);
    assert_eq!(
        simulation
            .nodes
            .iter()
            .map(|node| node.reducer.current_tag().view())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 0]
    );
    assert!(
        simulation.nodes[0]
            .reducer
            .durable_state()
            .highest_prepare()
            .is_none()
    );

    // A single verified TC for the highest observed view is sufficient to
    // converge every lower view, including validators that never saw the QC.
    for index in 0..simulation.nodes.len() {
        simulation.install_tc(index, tc2.clone());
    }
    assert_eq!(simulation.current_online_view(), 3);
    for node in &simulation.nodes {
        assert_eq!(
            node.reducer
                .durable_state()
                .highest_prepare()
                .map(QuorumCertificate::reference),
            Some(prepare.reference())
        );
    }

    // One node receives a delayed CommitQC from view zero after advancing to
    // view three. It finalizes immediately; the remaining dual quorum safely
    // commits the TC-selected subject in view three.
    let old_commit = certificate(&simulation.context, 0, Phase::Commit, subject);
    simulation.deliver_commit_qc(0, old_commit);
    assert_eq!(
        simulation.nodes[0]
            .reducer
            .durable_state()
            .decision()
            .map(QuorumCertificate::subject),
        Some(subject)
    );
    let successful_view = simulation.propose(subject);
    simulation.assert_online_committed(subject);
    assert_eq!(successful_view, 3);
    assert!(successful_view <= simulation.context.roster().len() as u64);
}

#[test]
fn accelerated_chain_chaos_smoke_preserves_prefix() {
    for mode in [VotingMode::Permissioned, VotingMode::Npos] {
        let stats = run_accelerated_chain_chaos(ACCELERATED_CHAOS_SMOKE_HEIGHTS, mode);
        assert_eq!(stats.wal_append_restarts, 1);
        assert_eq!(stats.fetch_restarts, 1);
        assert_eq!(stats.store_restarts, 1);
        assert_eq!(stats.validation_restarts, 1);
        assert_eq!(stats.application_restarts, 1);
        assert_eq!(stats.stale_generation_rejections, 5);
        assert!(stats.duplicate_commit_qcs > 0);
        assert!(stats.reordered_commit_batches > 0);
        assert!(stats.reordered_tc_batches > 0);
        assert!(stats.insufficient_dual_qcs > 0);
        if mode == VotingMode::Npos {
            assert!(stats.count_only_qcs > 0);
            assert!(stats.power_only_qcs > 0);
        }
    }
}

#[test]
#[ignore = "explicit release gate: executes 100,000 certificate-supplied reducer heights"]
fn accelerated_100_000_block_chaos_preserves_chain_prefix() {
    let per_mode = ACCELERATED_CHAOS_HEIGHTS / 2;
    let mut stats = run_accelerated_chain_chaos(per_mode, VotingMode::Permissioned);
    stats.merge(run_accelerated_chain_chaos(per_mode, VotingMode::Npos));
    println!(
        "SUMERAGI_V2_CHAOS_COMPLETED permissioned_heights={per_mode} npos_heights={per_mode} total_heights={ACCELERATED_CHAOS_HEIGHTS} supplied_commit_qcs={} supplied_tcs={} finalized_validators={} wal_append_restarts={} fetch_restarts={} store_restarts={} validation_restarts={} application_restarts={} stale_generation_rejections={} deferred_fetch_completions={} deferred_store_completions={} deferred_validation_completions={} deferred_application_completions={} duplicate_commit_qcs={} reordered_commit_batches={} reordered_tc_batches={} insufficient_dual_qcs={} count_only_qcs={} power_only_qcs={} restart_interval={} duplicate_interval={} under_quorum_interval={} certificate_source=external_fixture",
        stats.supplied_commit_qcs,
        stats.supplied_tcs,
        stats.finalized_validators,
        stats.wal_append_restarts,
        stats.fetch_restarts,
        stats.store_restarts,
        stats.validation_restarts,
        stats.application_restarts,
        stats.stale_generation_rejections,
        stats.deferred_fetch_completions,
        stats.deferred_store_completions,
        stats.deferred_validation_completions,
        stats.deferred_application_completions,
        stats.duplicate_commit_qcs,
        stats.reordered_commit_batches,
        stats.reordered_tc_batches,
        stats.insufficient_dual_qcs,
        stats.count_only_qcs,
        stats.power_only_qcs,
        ACCELERATED_RESTART_INTERVAL,
        ACCELERATED_DUPLICATE_INTERVAL,
        ACCELERATED_UNDER_QUORUM_INTERVAL,
    );
}

fn run_accelerated_chain_chaos(height_count: u64, mode: VotingMode) -> AcceleratedChaosStats {
    let mut parent = None;
    let mut stats = AcceleratedChaosStats::default();
    for height in 1..=height_count {
        let context = accelerated_context(height, parent, mode);
        let subject = accelerated_id(0xd0 + u8::from(mode == VotingMode::Npos), height);
        let certified_view = height % u64::try_from(context.roster().len()).expect("small roster");
        let commit_view = if certified_view > 0 && height.is_multiple_of(7) {
            0
        } else {
            certified_view
        };
        let decision = certificate(&context, commit_view, Phase::Commit, subject);
        run_accelerated_height(
            &context,
            subject,
            certified_view,
            &decision,
            height,
            &mut stats,
        );
        parent = Some(decision.reference());
        assert_eq!(
            parent
                .expect("decision becomes the next parent")
                .round()
                .height(),
            height
        );
        assert_eq!(
            parent.expect("decision becomes the next parent").subject(),
            subject
        );
    }
    assert_eq!(
        stats,
        expected_accelerated_chaos_stats(height_count, mode),
        "the deterministic schedule must execute every attested fault boundary"
    );
    stats
}

#[allow(clippy::too_many_lines)]
fn run_accelerated_height(
    context: &HeightContext,
    subject: Subject,
    certified_view: u64,
    decision: &QuorumCertificate,
    chaos_sequence: u64,
    stats: &mut AcceleratedChaosStats,
) {
    let mut nodes = context
        .roster()
        .iter()
        .map(|validator| AcceleratedNode {
            reducer: Reducer::new(
                context.clone(),
                Some(validator.id()),
                Generation::new(chaos_sequence),
            )
            .expect("accelerated height has a valid frozen context"),
            wal: Vec::new(),
            pending: VecDeque::new(),
        })
        .collect::<Vec<_>>();
    let mut signature_sequence = chaos_sequence;

    if certified_view > 0 {
        let timeout = grouped_timeout(context, certified_view - 1, None);
        let order = accelerated_delivery_order(nodes.len(), chaos_sequence);
        let old_tags = nodes
            .iter()
            .map(|node| node.reducer.current_tag())
            .collect::<Vec<_>>();
        if !is_canonical_delivery_order(&order) {
            stats.reordered_tc_batches += 1;
        }
        stats.supplied_tcs += 1;
        for index in order {
            let old_tag = nodes[index].reducer.current_tag();
            enqueue_accelerated_event(
                &mut nodes[index],
                Event::TimeoutCertificateReceived {
                    tag: old_tag,
                    certificate: timeout.clone(),
                },
            );
        }
        drain_accelerated_effects(
            &mut nodes,
            context,
            chaos_sequence,
            None,
            &mut signature_sequence,
            stats,
        );
        for (index, node) in nodes.iter_mut().enumerate() {
            assert_eq!(node.reducer.current_tag().view(), certified_view);
            let disposition = node
                .reducer
                .step(Event::RetransmitElapsed {
                    tag: old_tags[index],
                })
                .expect("stale retransmission is a safe reducer input")
                .disposition();
            assert!(matches!(
                disposition,
                StepDisposition::Ignored(IgnoreReason::WrongView | IgnoreReason::StaleGeneration)
            ));
        }
    }

    if chaos_sequence.is_multiple_of(ACCELERATED_UNDER_QUORUM_INTERVAL) {
        assert_under_quorum_decision_is_transactional(
            &mut nodes[0].reducer,
            context,
            subject,
            &[0, 1],
            false,
            false,
        );
        stats.insufficient_dual_qcs += 1;
        if context.mode() == VotingMode::Npos {
            assert_under_quorum_decision_is_transactional(
                &mut nodes[0].reducer,
                context,
                subject,
                &[0, 1, 2],
                true,
                false,
            );
            stats.count_only_qcs += 1;
            assert_under_quorum_decision_is_transactional(
                &mut nodes[0].reducer,
                context,
                subject,
                &[2, 3],
                false,
                true,
            );
            stats.power_only_qcs += 1;
        }
    }
    let commit_order = accelerated_delivery_order(nodes.len(), chaos_sequence.wrapping_add(1));
    if !is_canonical_delivery_order(&commit_order) {
        stats.reordered_commit_batches += 1;
    }
    stats.supplied_commit_qcs += 1;
    for index in commit_order {
        let tag = nodes[index].reducer.current_tag();
        enqueue_accelerated_event(
            &mut nodes[index],
            Event::QuorumCertificateReceived {
                tag,
                certificate: decision.clone(),
            },
        );
    }
    let restart_plan = accelerated_restart_plan(chaos_sequence, nodes.len());
    drain_accelerated_effects(
        &mut nodes,
        context,
        chaos_sequence.wrapping_add(2),
        restart_plan,
        &mut signature_sequence,
        stats,
    );

    if chaos_sequence.is_multiple_of(ACCELERATED_DUPLICATE_INTERVAL) {
        let index = usize::try_from(
            chaos_sequence % u64::try_from(nodes.len()).expect("four nodes fit in u64"),
        )
        .expect("duplicate target fits in usize");
        let tag = nodes[index].reducer.current_tag();
        let outcome = nodes[index]
            .reducer
            .step(Event::QuorumCertificateReceived {
                tag,
                certificate: decision.clone(),
            })
            .expect("an identical supplied CommitQC is a safe stutter");
        assert_eq!(
            outcome.disposition(),
            StepDisposition::Ignored(IgnoreReason::Duplicate)
        );
        assert!(outcome.effects().is_empty());
        stats.duplicate_commit_qcs += 1;
    }

    for node in nodes {
        let durable_decision = node
            .reducer
            .durable_state()
            .decision()
            .expect("every accelerated validator persists a decision");
        assert_eq!(durable_decision.reference(), decision.reference());
        assert_eq!(node.reducer.applied_subject(), Some(subject));
        assert!(node
            .wal
            .iter()
            .any(|entry| matches!(entry.record(), WalRecord::Decision(certificate) if certificate.reference() == decision.reference())));
        if certified_view > 0 {
            assert!(node
                .wal
                .iter()
                .any(|entry| matches!(entry.record(), WalRecord::InstallTimeout(certificate) if certificate.round().view() + 1 == certified_view)));
        }
        let receipt = DurableCommitReceipt::from_trusted_storage(
            context.id(),
            context.height(),
            subject,
            decision.reference(),
        );
        let finalized = node
            .reducer
            .finish_height(receipt)
            .expect("application and exact durable receipt close the height");
        assert_eq!(finalized.context(), context);
        assert_eq!(finalized.decision().reference(), decision.reference());
        stats.finalized_validators += 1;
    }
    stats.completed_heights += 1;
}

fn enqueue_accelerated_event(node: &mut AcceleratedNode, event: Event) -> StepDisposition {
    let outcome = node
        .reducer
        .step(event)
        .expect("accelerated chaos event satisfies reducer guards");
    let disposition = outcome.disposition();
    node.pending.extend(outcome.into_effects());
    disposition
}

#[allow(clippy::too_many_lines)]
fn drain_accelerated_effects(
    nodes: &mut [AcceleratedNode],
    context: &HeightContext,
    scheduler_sequence: u64,
    restart_plan: Option<(usize, AcceleratedRestartPoint)>,
    signature_sequence: &mut u64,
    stats: &mut AcceleratedChaosStats,
) {
    // Process at most one adapter effect per node and pass. Follow-up work is
    // therefore always deferred behind another deterministic scheduler rank,
    // rather than being completed recursively in the reducer call stack.
    let mut scheduler_pass = 0_u64;
    let mut restart_injected = false;
    while nodes.iter().any(|node| !node.pending.is_empty()) {
        let order = accelerated_delivery_order(
            nodes.len(),
            scheduler_sequence.wrapping_add(scheduler_pass),
        );
        let mut progressed = false;
        for index in order {
            let Some(effect) = nodes[index].pending.pop_front() else {
                continue;
            };
            progressed = true;
            if !restart_injected
                && restart_plan.is_some_and(|(target, point)| {
                    target == index && accelerated_effect_matches_restart(&effect, point)
                })
            {
                let (_, point) = restart_plan.expect("restart plan was just matched");
                restart_accelerated_node(&mut nodes[index], context, effect, point, stats);
                restart_injected = true;
                continue;
            }
            let follow_up = match effect {
                Effect::Persist { tag, entry } => {
                    nodes[index].wal.push(entry.clone());
                    nodes[index]
                        .reducer
                        .step(Event::Persisted {
                            tag,
                            id: entry.id(),
                        })
                        .expect("in-memory chaos WAL acknowledges the requested frame")
                        .into_effects()
                }
                Effect::Sign { tag, message } => {
                    *signature_sequence = signature_sequence
                        .checked_add(1)
                        .expect("accelerated signature sequence remains bounded");
                    let signature = simulator_signature(
                        nodes[index]
                            .reducer
                            .local_validator()
                            .expect("accelerated nodes are validators"),
                        *signature_sequence,
                        &message,
                    );
                    nodes[index]
                        .reducer
                        .step(Event::Signed { tag, signature })
                        .expect("signature completes the exact durable intent")
                        .into_effects()
                }
                Effect::Broadcast(_) | Effect::EnterView { .. } => Vec::new(),
                Effect::FetchBody {
                    tag,
                    round,
                    subject,
                    ..
                } => {
                    stats.deferred_fetch_completions += 1;
                    nodes[index]
                        .reducer
                        .step(Event::BodyAvailable {
                            tag,
                            round,
                            subject,
                        })
                        .expect("certified signer supplies the exact body")
                        .into_effects()
                }
                Effect::StoreBody {
                    tag,
                    round,
                    subject,
                } => {
                    stats.deferred_store_completions += 1;
                    nodes[index]
                        .reducer
                        .step(Event::BodyStored {
                            tag,
                            round,
                            subject,
                        })
                        .expect("accelerated body store acknowledges durability")
                        .into_effects()
                }
                Effect::ValidateBody {
                    tag,
                    round,
                    subject,
                } => {
                    stats.deferred_validation_completions += 1;
                    nodes[index]
                        .reducer
                        .step(Event::ValidationCompleted {
                            tag,
                            round,
                            subject,
                            valid: true,
                        })
                        .expect("deterministic validation accepts the exact body")
                        .into_effects()
                }
                Effect::Apply {
                    tag,
                    subject,
                    certificate,
                } => {
                    assert_eq!(certificate.subject(), subject);
                    stats.deferred_application_completions += 1;
                    nodes[index]
                        .reducer
                        .step(Event::ApplicationCompleted { tag, subject })
                        .expect("application matches the durable decision")
                        .into_effects()
                }
                Effect::ReportEquivocation { .. } | Effect::ReportInvalidCertifiedBody { .. } => {
                    panic!("accelerated valid corridor emitted an adversarial report")
                }
            };
            nodes[index].pending.extend(follow_up);
        }
        assert!(
            progressed,
            "deferred local-work scheduler must make progress"
        );
        scheduler_pass = scheduler_pass
            .checked_add(1)
            .expect("accelerated scheduler pass remains bounded");
    }
    assert_eq!(
        restart_injected,
        restart_plan.is_some(),
        "scheduled restart point must be reached exactly once"
    );
}

fn assert_under_quorum_decision_is_transactional(
    reducer: &mut Reducer,
    context: &HeightContext,
    subject: Subject,
    signer_indices: &[usize],
    expect_count_threshold: bool,
    expect_power_threshold: bool,
) {
    let before = reducer.clone();
    let round = Round::new(context.height(), reducer.current_tag().view());
    let signatures = signer_indices
        .iter()
        .map(|index| {
            let validator = &context.roster()[*index];
            SignatureShare::new(validator.id(), OpaqueSignature::new(vec![0xee]))
        })
        .collect::<Vec<_>>();
    let signers = signatures
        .iter()
        .map(SignatureShare::signer)
        .collect::<Vec<_>>();
    let quorum = Quorum::calculate(context, &signers).expect("fixture signers are canonical");
    assert_eq!(
        quorum.signer_count() >= context.minimum_signer_count(),
        expect_count_threshold
    );
    assert_eq!(
        u128::from(quorum.voting_power().get()) * 3
            > u128::from(context.total_voting_power().get()) * 2,
        expect_power_threshold
    );
    let under_quorum = QuorumCertificate::new(
        CertificateRef::new(context.id(), round, Phase::Commit, subject),
        signatures,
    );
    assert!(matches!(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: under_quorum,
            })
            .expect_err("either count or voting-power insufficiency must fail closed"),
        ReducerError::Quorum(QuorumError::Insufficient { .. })
    ));
    assert_eq!(reducer, &before);
}

fn accelerated_effect_matches_restart(effect: &Effect, point: AcceleratedRestartPoint) -> bool {
    match (effect, point) {
        (Effect::Persist { entry, .. }, AcceleratedRestartPoint::WalAppended) => {
            matches!(entry.record(), WalRecord::Decision(_))
        }
        (Effect::FetchBody { .. }, AcceleratedRestartPoint::FetchPending)
        | (Effect::StoreBody { .. }, AcceleratedRestartPoint::StorePending)
        | (Effect::ValidateBody { .. }, AcceleratedRestartPoint::ValidationPending)
        | (Effect::Apply { .. }, AcceleratedRestartPoint::ApplicationPending) => true,
        _ => false,
    }
}

#[allow(clippy::too_many_lines)]
fn restart_accelerated_node(
    node: &mut AcceleratedNode,
    context: &HeightContext,
    interrupted: Effect,
    point: AcceleratedRestartPoint,
    stats: &mut AcceleratedChaosStats,
) {
    let stale_event = match interrupted {
        Effect::Persist { tag, entry } => {
            assert_eq!(point, AcceleratedRestartPoint::WalAppended);
            assert!(matches!(entry.record(), WalRecord::Decision(_)));
            // The complete frame reached the durable WAL, but the process
            // dies before its Persisted acknowledgement or Decide follow-up.
            node.wal.push(entry.clone());
            stats.wal_append_restarts += 1;
            Event::Persisted {
                tag,
                id: entry.id(),
            }
        }
        Effect::FetchBody {
            tag,
            round,
            subject,
            ..
        } => {
            assert_eq!(point, AcceleratedRestartPoint::FetchPending);
            stats.fetch_restarts += 1;
            Event::BodyAvailable {
                tag,
                round,
                subject,
            }
        }
        Effect::StoreBody {
            tag,
            round,
            subject,
        } => {
            assert_eq!(point, AcceleratedRestartPoint::StorePending);
            stats.store_restarts += 1;
            Event::BodyStored {
                tag,
                round,
                subject,
            }
        }
        Effect::ValidateBody {
            tag,
            round,
            subject,
        } => {
            assert_eq!(point, AcceleratedRestartPoint::ValidationPending);
            stats.validation_restarts += 1;
            Event::ValidationCompleted {
                tag,
                round,
                subject,
                valid: true,
            }
        }
        Effect::Apply { tag, subject, .. } => {
            assert_eq!(point, AcceleratedRestartPoint::ApplicationPending);
            stats.application_restarts += 1;
            Event::ApplicationCompleted { tag, subject }
        }
        _ => panic!("restart plan matched a non-interruptible accelerated effect"),
    };

    let local_validator = node
        .reducer
        .local_validator()
        .expect("accelerated nodes are validators");
    let generation = Generation::new(
        node.reducer
            .current_tag()
            .generation()
            .get()
            .checked_add(1)
            .expect("one accelerated restart cannot exhaust generations"),
    );
    let mut recovered = Reducer::recover(
        context.clone(),
        Some(local_validator),
        generation,
        node.wal.clone(),
    )
    .expect("the complete in-memory WAL prefix recovers");
    let recovered_tag = recovered.current_tag();
    let resume = recovered
        .step(Event::ResumeAfterReplay { tag: recovered_tag })
        .expect("the recovered reducer resumes through its production gate");
    assert_eq!(resume.disposition(), StepDisposition::Applied);
    node.reducer = recovered;
    node.pending.clear();
    node.pending.extend(resume.into_effects());

    let stale = node
        .reducer
        .step(stale_event)
        .expect("a completion from the crashed generation is a safe stutter");
    assert_eq!(
        stale.disposition(),
        StepDisposition::Ignored(IgnoreReason::StaleGeneration)
    );
    assert!(stale.effects().is_empty());
    stats.stale_generation_rejections += 1;
}

fn accelerated_restart_plan(
    sequence: u64,
    node_count: usize,
) -> Option<(usize, AcceleratedRestartPoint)> {
    if !sequence.is_multiple_of(ACCELERATED_RESTART_INTERVAL) {
        return None;
    }
    // Rotate both the interrupted production boundary and the affected
    // validator without introducing a random-number dependency into the gate.
    let occurrence = sequence / ACCELERATED_RESTART_INTERVAL - 1;
    let point = match occurrence % 5 {
        0 => AcceleratedRestartPoint::WalAppended,
        1 => AcceleratedRestartPoint::FetchPending,
        2 => AcceleratedRestartPoint::StorePending,
        3 => AcceleratedRestartPoint::ValidationPending,
        4 => AcceleratedRestartPoint::ApplicationPending,
        _ => unreachable!("modulo five has exactly five residues"),
    };
    let index = usize::try_from(
        occurrence % u64::try_from(node_count).expect("accelerated node count fits in u64"),
    )
    .expect("restart target fits in usize");
    Some((index, point))
}

fn expected_accelerated_chaos_stats(height_count: u64, mode: VotingMode) -> AcceleratedChaosStats {
    let mut expected = AcceleratedChaosStats::default();
    for height in 1..=height_count {
        expected.completed_heights += 1;
        expected.finalized_validators += 4;
        expected.supplied_commit_qcs += 1;
        expected.deferred_fetch_completions += 4;
        expected.deferred_store_completions += 4;
        expected.deferred_validation_completions += 4;
        expected.deferred_application_completions += 4;

        if height % 4 != 0 {
            expected.supplied_tcs += 1;
            expected.reordered_tc_batches += 1;
        }
        if !is_canonical_delivery_order(&accelerated_delivery_order(4, height + 1)) {
            expected.reordered_commit_batches += 1;
        }
        if height.is_multiple_of(ACCELERATED_DUPLICATE_INTERVAL) {
            expected.duplicate_commit_qcs += 1;
        }
        if height.is_multiple_of(ACCELERATED_UNDER_QUORUM_INTERVAL) {
            expected.insufficient_dual_qcs += 1;
            if mode == VotingMode::Npos {
                expected.count_only_qcs += 1;
                expected.power_only_qcs += 1;
            }
        }
        if let Some((_, point)) = accelerated_restart_plan(height, 4) {
            expected.stale_generation_rejections += 1;
            match point {
                AcceleratedRestartPoint::WalAppended => expected.wal_append_restarts += 1,
                AcceleratedRestartPoint::FetchPending => expected.fetch_restarts += 1,
                AcceleratedRestartPoint::StorePending => {
                    expected.store_restarts += 1;
                    expected.deferred_fetch_completions += 1;
                }
                AcceleratedRestartPoint::ValidationPending => {
                    expected.validation_restarts += 1;
                    expected.deferred_fetch_completions += 1;
                    expected.deferred_store_completions += 1;
                }
                AcceleratedRestartPoint::ApplicationPending => {
                    expected.application_restarts += 1;
                    expected.deferred_fetch_completions += 1;
                    expected.deferred_store_completions += 1;
                    expected.deferred_validation_completions += 1;
                }
            }
        }
    }
    expected
}

fn is_canonical_delivery_order(order: &[usize]) -> bool {
    order.iter().copied().eq(0..order.len())
}

fn accelerated_delivery_order(length: usize, sequence: u64) -> Vec<usize> {
    let mut order = (0..length).collect::<Vec<_>>();
    if sequence % 2 == 1 {
        order.reverse();
    } else if length > 1 {
        order.rotate_left(
            usize::try_from(sequence % u64::try_from(length).expect("length fits u64"))
                .expect("rotation fits usize"),
        );
    }
    order
}

fn accelerated_context(
    height: u64,
    parent: Option<CertificateRef>,
    mode: VotingMode,
) -> HeightContext {
    let roster = (1_u64..=4)
        .map(|index| {
            let power = match mode {
                VotingMode::Permissioned => 1,
                VotingMode::Npos => index,
            };
            Validator::new(
                validator_id(usize::try_from(index).expect("four-validator index fits usize")),
                VotingPower::new(power),
            )
        })
        .collect();
    HeightContext::new(
        ContextId::new(*accelerated_id(0xa0, height).as_bytes()),
        ChainId::repeat(0xa1 + u8::from(mode == VotingMode::Npos)),
        height,
        parent,
        height / 1_000,
        roster,
        mode,
        Digest::new(*accelerated_id(0xa2, height).as_bytes()),
        Digest::new(*accelerated_id(0xa3, height).as_bytes()),
        Digest::new(*accelerated_id(0xa4, height).as_bytes()),
    )
    .expect("accelerated context preserves parent and frozen-roster invariants")
}

fn accelerated_id(domain: u8, sequence: u64) -> Subject {
    let mut bytes = [domain; 32];
    bytes[..8].copy_from_slice(&sequence.to_le_bytes());
    bytes[8..16].copy_from_slice(&sequence.rotate_left(17).to_le_bytes());
    Subject::new(bytes)
}

fn context(validator_count: usize, mode: VotingMode) -> HeightContext {
    let roster = (1..=validator_count)
        .map(|index| {
            let power = match mode {
                VotingMode::Permissioned => 1,
                VotingMode::Npos => u64::try_from(index).expect("fixture size fits in u64"),
            };
            Validator::new(validator_id(index), VotingPower::new(power))
        })
        .collect();
    let parent = CertificateRef::new(
        ContextId::repeat(0x10),
        Round::new(HEIGHT - 1, 0),
        Phase::Commit,
        Subject::repeat(0x11),
    );
    HeightContext::new(
        ContextId::repeat(0x42),
        ChainId::repeat(0x43),
        HEIGHT,
        Some(parent),
        7,
        roster,
        mode,
        Digest::repeat(0x44),
        Digest::repeat(0x45),
        Digest::repeat(0),
    )
    .expect("simulation context is valid")
}

fn validator_id(index: usize) -> ValidatorId {
    ValidatorId::repeat(u8::try_from(index).expect("fixture size fits in u8"))
}

fn manifest(subject: Subject) -> PayloadManifest {
    PayloadManifest::new(
        subject,
        Digest::repeat(subject.as_bytes()[0]),
        Digest::repeat(subject.as_bytes()[0].wrapping_add(1)),
        128,
        4,
    )
}

fn certificate(
    context: &HeightContext,
    view: u64,
    phase: Phase,
    subject: Subject,
) -> QuorumCertificate {
    let round = Round::new(context.height(), view);
    let signatures = context
        .roster()
        .iter()
        .map(|validator| {
            SignatureShare::new(
                validator.id(),
                OpaqueSignature::new(vec![validator.id().as_bytes()[0], view.to_le_bytes()[0]]),
            )
        })
        .collect();
    QuorumCertificate::new(
        CertificateRef::new(context.id(), round, phase, subject),
        signatures,
    )
}

fn grouped_timeout(
    context: &HeightContext,
    view: u64,
    highest_prepare: Option<QuorumCertificate>,
) -> TimeoutCertificate {
    // Use the whole roster so unequal-power fixtures satisfy the strict power
    // threshold independently of where high-power validators appear.
    let signers = context.roster();
    let split = signers.len() / 2;
    let group = |validators: &[Validator], marker: u8| {
        validators
            .iter()
            .map(|validator| {
                SignatureShare::new(
                    validator.id(),
                    OpaqueSignature::new(vec![marker, validator.id().as_bytes()[0]]),
                )
            })
            .collect()
    };
    let groups = highest_prepare.map_or_else(
        || vec![TimeoutSignatureGroup::new(None, group(signers, 0))],
        |highest| {
            vec![
                TimeoutSignatureGroup::new(None, group(&signers[..split], 0)),
                TimeoutSignatureGroup::new(Some(highest), group(&signers[split..], 1)),
            ]
        },
    );
    let certificate =
        TimeoutCertificate::new(context.id(), Round::new(context.height(), view), groups);
    certificate
        .validate(context)
        .expect("fixture TC meets count and voting-power quorums");
    certificate
}

fn simulator_signature(
    signer: ValidatorId,
    sequence: u64,
    message: &SignableMessage,
) -> OpaqueSignature {
    let kind = match message {
        SignableMessage::Proposal(_) => 0,
        SignableMessage::Vote(vote) if vote.phase() == Phase::Prepare => 1,
        SignableMessage::Vote(_) => 2,
        SignableMessage::TimeoutVote(_) => 3,
    };
    OpaqueSignature::new(vec![signer.as_bytes()[0], kind, sequence.to_le_bytes()[0]])
}

fn network_event(message: &ConsensusMessageV2, current: EventTag) -> Event {
    match message {
        ConsensusMessageV2::Proposal(proposal) => Event::ProposalReceived {
            tag: message_tag(current, proposal.proposal().round()),
            proposal: proposal.clone(),
        },
        ConsensusMessageV2::Vote(vote) if vote.vote().phase() == Phase::Commit => {
            // The production adapter admits the one exact durable locked-round
            // Commit exception and retags it to the current consumer
            // generation. Preserve that boundary here so a TC-reset pool can
            // be reconstructed by an old-round retransmission.
            Event::VoteReceived {
                tag: current,
                vote: vote.clone(),
            }
        }
        ConsensusMessageV2::Vote(vote) => Event::VoteReceived {
            tag: message_tag(current, vote.vote().round()),
            vote: vote.clone(),
        },
        ConsensusMessageV2::QuorumCertificate(certificate)
            if certificate.phase() == Phase::Commit =>
        {
            // Old-view CommitQCs remain actionable in the current view.
            Event::QuorumCertificateReceived {
                tag: current,
                certificate: certificate.clone(),
            }
        }
        ConsensusMessageV2::QuorumCertificate(certificate) => Event::QuorumCertificateReceived {
            tag: message_tag(current, certificate.round()),
            certificate: certificate.clone(),
        },
        ConsensusMessageV2::TimeoutVote(vote) => Event::TimeoutVoteReceived {
            tag: message_tag(current, vote.vote().round()),
            vote: vote.clone(),
        },
        ConsensusMessageV2::TimeoutCertificate(certificate) => {
            // A TC may legitimately move a lagging validator across views.
            Event::TimeoutCertificateReceived {
                tag: current,
                certificate: certificate.clone(),
            }
        }
        ConsensusMessageV2::BodyRequest(_) | ConsensusMessageV2::BodyChunk(_) => {
            panic!("transport payloads are handled outside the consensus reducer simulation")
        }
    }
}

fn message_tag(current: EventTag, round: Round) -> EventTag {
    EventTag::new(round.height(), round.view(), current.generation())
}
