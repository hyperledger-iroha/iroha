//! Deterministic multi-validator simulations for the production Sumeragi v2 reducer.
//!
//! The harness deliberately keeps networking, signatures, body storage, and
//! validation outside the reducer.  It acknowledges those adapter effects
//! synchronously while delivering network messages through a deterministic
//! lossy, duplicating, and reordering scheduler.

use std::collections::{BTreeSet, VecDeque};

use super::{
    BodyState, CertificateRef, ChainId, ConsensusMessageV2, ContextId, Digest, Effect,
    EquivocationKind, Event, EventTag, Generation, HeightContext, IgnoreReason, OpaqueSignature,
    PayloadManifest, Phase, QuorumCertificate, Reducer, Round, SignableMessage, SignatureShare,
    SignedVote, StepDisposition, Subject, TimeoutCertificate, TimeoutSignatureGroup, Validator,
    ValidatorId, Vote, VotingMode, VotingPower, WalEntry, WalRecord,
};

const HEIGHT: u64 = 42;

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
fn historical_prepare_qc_uses_current_consumer_tag_after_timeout_install() {
    let mut simulation = Simulation::new(4, VotingMode::Permissioned, None);
    let timeout = grouped_timeout(&simulation.context, 0, None);
    let initial_tag = simulation.nodes[0].reducer.current_tag();
    assert_eq!(
        simulation.dispatch(
            0,
            Event::TimeoutCertificateReceived {
                tag: initial_tag,
                certificate: timeout,
            },
        ),
        StepDisposition::Applied
    );
    assert_eq!(simulation.nodes[0].reducer.current_tag().view(), 1);

    let subject = Subject::repeat(0x6f);
    let historical = certificate(&simulation.context, 0, Phase::Prepare, subject);
    let event = network_event(
        &ConsensusMessageV2::QuorumCertificate(historical.clone()),
        simulation.nodes[0].reducer.current_tag(),
    );
    assert!(matches!(
        &event,
        Event::QuorumCertificateReceived { tag, certificate }
            if *tag == simulation.nodes[0].reducer.current_tag()
                && certificate == &historical
    ));
    assert_eq!(
        simulation.dispatch(0, event),
        StepDisposition::Applied,
        "an old certified round is payload evidence, not a stale consumer tag"
    );
    assert_eq!(
        simulation.nodes[0]
            .reducer
            .durable_state()
            .highest_prepare(),
        Some(&historical)
    );

    let retransmit_tag = simulation.nodes[0].reducer.current_tag();
    simulation.dispatch(
        0,
        Event::RetransmitElapsed {
            tag: retransmit_tag,
        },
    );
    assert!(simulation.network.iter().any(|envelope| {
        envelope.from == 0
            && matches!(
                &envelope.message,
                ConsensusMessageV2::QuorumCertificate(certificate)
                    if certificate == &historical
            )
    }));
}

#[test]
fn responsive_source_redelivers_exact_prepare_qc_after_lagger_installs_tc() {
    for mode in [VotingMode::Permissioned, VotingMode::Npos] {
        let mut simulation = Simulation::new(4, mode, None);
        let source = 0;
        let lagger = 1;
        let timeout = grouped_timeout(&simulation.context, 0, None);

        simulation.install_tc(source, timeout.clone());
        assert_eq!(simulation.nodes[source].reducer.current_tag().view(), 1);
        assert_eq!(simulation.nodes[lagger].reducer.current_tag().view(), 0);

        let subject = Subject::repeat(0x70);
        let prepare = certificate(&simulation.context, 1, Phase::Prepare, subject);
        let source_event = network_event(
            &ConsensusMessageV2::QuorumCertificate(prepare.clone()),
            simulation.nodes[source].reducer.current_tag(),
        );
        assert_eq!(
            simulation.dispatch(source, source_event),
            StepDisposition::Applied
        );
        assert_eq!(
            simulation.nodes[source]
                .reducer
                .durable_state()
                .highest_prepare(),
            Some(&prepare)
        );
        assert!(simulation.nodes[source].wal.iter().any(|entry| matches!(
            entry.record(),
            WalRecord::ObservePrepare(certificate) if certificate == &prepare
        )));
        assert!(
            simulation.nodes[source]
                .reducer
                .outbound_messages()
                .any(|message| matches!(
                    message,
                    ConsensusMessageV2::QuorumCertificate(certificate)
                        if certificate == &prepare
                ))
        );

        let lagger_before = simulation.nodes[lagger].reducer.clone();
        let lagger_wal_len = simulation.nodes[lagger].wal.len();
        let future_event = network_event(
            &ConsensusMessageV2::QuorumCertificate(prepare.clone()),
            simulation.nodes[lagger].reducer.current_tag(),
        );
        assert_eq!(
            simulation.dispatch(lagger, future_event),
            StepDisposition::Ignored(IgnoreReason::IrrelevantView)
        );
        assert_eq!(&simulation.nodes[lagger].reducer, &lagger_before);
        assert_eq!(simulation.nodes[lagger].wal.len(), lagger_wal_len);

        let queued_before = simulation.network.len();
        let lagger_tag = simulation.nodes[lagger].reducer.current_tag();
        assert_eq!(
            simulation.dispatch(lagger, Event::RetransmitElapsed { tag: lagger_tag }),
            StepDisposition::Applied
        );
        assert_eq!(
            simulation.network.len(),
            queued_before,
            "the ignored future QC must not acquire retransmission ownership"
        );

        simulation.install_tc(lagger, timeout);
        assert_eq!(simulation.nodes[lagger].reducer.current_tag().view(), 1);
        assert!(
            simulation.nodes[lagger]
                .reducer
                .durable_state()
                .highest_prepare()
                .is_none()
        );

        let source_tag = simulation.nodes[source].reducer.current_tag();
        assert_eq!(
            simulation.dispatch(source, Event::RetransmitElapsed { tag: source_tag }),
            StepDisposition::Applied
        );
        assert!(simulation.network.iter().any(|envelope| {
            envelope.from == source
                && envelope.to == lagger
                && matches!(
                    &envelope.message,
                    ConsensusMessageV2::QuorumCertificate(certificate)
                        if certificate == &prepare
                )
        }));
        simulation.drain_network();

        assert_eq!(
            simulation.nodes[lagger]
                .reducer
                .durable_state()
                .highest_prepare(),
            Some(&prepare)
        );
        assert!(simulation.nodes[lagger].wal.iter().any(|entry| matches!(
            entry.record(),
            WalRecord::ObservePrepare(certificate) if certificate == &prepare
        )));
        assert!(
            simulation.nodes[lagger]
                .reducer
                .outbound_messages()
                .any(|message| matches!(
                    message,
                    ConsensusMessageV2::QuorumCertificate(certificate)
                        if certificate == &prepare
                ))
        );

        let lagger_tag = simulation.nodes[lagger].reducer.current_tag();
        assert_eq!(
            simulation.dispatch(lagger, Event::RetransmitElapsed { tag: lagger_tag }),
            StepDisposition::Applied
        );
        assert_eq!(
            simulation.nodes[lagger]
                .reducer
                .body_state(prepare.round(), subject),
            BodyState::Validated
        );
        assert!(simulation.nodes[lagger].wal.iter().any(|entry| matches!(
            entry.record(),
            WalRecord::LockAndCommit {
                prepare: locked,
                ..
            } if locked == &prepare
        )));
    }
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
    // Validators that installed the lock in an earlier view retain and
    // retransmit that exact durable Commit intent across later TCs. Validators
    // learning the lock from `tc2` create their intent in view three.
    for node in &simulation.nodes {
        assert_eq!(
            node.reducer
                .durable_state()
                .highest_prepare()
                .map(QuorumCertificate::reference),
            Some(prepare.reference())
        );
        // Learning an old PrepareQC through a TC installs the lock and its
        // exact body owner, but it cannot manufacture a post-timeout Commit
        // intent for that closed round. These validators did not persist such
        // an intent before timing out, so the old vote pool must remain empty;
        // progress below comes from unchanged reproposal in view three.
        assert!(node.reducer.vote_pool_snapshots().is_empty());
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
    // A retained Commit intent can complete the decision before a node exposes
    // an intermediate current-view lock. If a lock is installed, it must still
    // retire every inert historical Commit pool.
    for node in &simulation.nodes {
        if node
            .reducer
            .durable_state()
            .locked()
            .is_none_or(|locked| locked.round().view() != successful_view)
        {
            continue;
        }
        assert!(
            node.reducer
                .vote_pool_snapshots()
                .iter()
                .all(|pool| pool.round.view() == successful_view),
            "a newly durable lock must retire the inert historical Commit pool"
        );
    }
    assert_eq!(successful_view, 3);
    assert!(successful_view <= simulation.context.roster().len() as u64);
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
    // Production authenticates and semantically filters the wire payload,
    // then dispatches every admitted message with the reducer's current
    // consumer tag.  The certified evidence retains its own historical round;
    // putting that round in the event tag would incorrectly reject an old
    // PrepareQC after a TC advanced the receiver, exactly when epidemic
    // high-QC recovery needs it.
    match message {
        ConsensusMessageV2::Proposal(proposal) => Event::ProposalReceived {
            tag: current,
            proposal: proposal.clone(),
        },
        ConsensusMessageV2::Vote(vote) => Event::VoteReceived {
            tag: current,
            vote: vote.clone(),
        },
        ConsensusMessageV2::QuorumCertificate(certificate) => Event::QuorumCertificateReceived {
            tag: current,
            certificate: certificate.clone(),
        },
        ConsensusMessageV2::TimeoutVote(vote) => Event::TimeoutVoteReceived {
            tag: current,
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
