// Standalone protocol simulation: actual Reducer, simulated authenticated I/O.
// No real crypto, body execution, network, or production adapter is claimed.
use super::*;
use std::collections::BTreeSet;

fn context() -> HeightContext {
    HeightContext::new(
        ContextId::repeat(1),
        NetworkId::repeat(2),
        2,
        Some(CertificateRef::new(
            ContextId::repeat(3),
            Round::new(1, 0),
            Phase::Commit,
            Subject::repeat(4),
        )),
        0,
        (1..=4)
            .map(|n| Validator::new(ValidatorId::repeat(n), VotingPower::new(1)))
            .collect(),
        VotingMode::Permissioned,
        Digest::repeat(5),
        Digest::repeat(6),
        Digest::repeat(7),
        Digest::repeat(0),
    )
    .unwrap()
}

#[derive(Clone, Debug)]
enum Input {
    Local(Event),
    Wire(ConsensusMessageV2),
}
struct Network {
    reducers: Vec<Reducer>,
    wal: Vec<Vec<WalEntry>>,
    queue: Vec<(usize, Input)>,
    proposed: BTreeSet<(usize, EventTag)>,
    random: u64,
    offline: Option<usize>,
    crash: Option<(usize, usize)>,
    crashed: bool,
    tick: usize,
    drop_until: usize,
    steps: usize,
}
impl Network {
    fn new(
        seed: u64,
        offline: Option<usize>,
        crash: Option<(usize, usize)>,
        drop_until: usize,
    ) -> Self {
        Self {
            reducers: (1..=4)
                .map(|n| {
                    Reducer::new(context(), Some(ValidatorId::repeat(n)), Generation::new(0))
                        .unwrap()
                })
                .collect(),
            wal: vec![vec![]; 4],
            queue: vec![],
            proposed: BTreeSet::new(),
            random: seed + 1,
            offline,
            crash,
            crashed: false,
            tick: 0,
            drop_until,
            steps: 0,
        }
    }
    fn random(&mut self) -> u64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        self.random
    }
    fn local(&mut self, node: usize, event: Event) {
        self.queue.push((node, Input::Local(event)));
    }
    fn event(&self, node: usize, input: &Input) -> Event {
        let tag = self.reducers[node].current_tag();
        match input {
            Input::Local(event) => event.clone(),
            Input::Wire(ConsensusMessageV2::Proposal(proposal)) => Event::ProposalReceived {
                tag,
                proposal: proposal.clone(),
            },
            Input::Wire(ConsensusMessageV2::Vote(vote)) => Event::VoteReceived {
                tag,
                vote: vote.clone(),
            },
            Input::Wire(ConsensusMessageV2::QuorumCertificate(certificate)) => {
                Event::QuorumCertificateReceived {
                    tag,
                    certificate: certificate.clone(),
                }
            }
            Input::Wire(ConsensusMessageV2::TimeoutVote(vote)) => Event::TimeoutVoteReceived {
                tag,
                vote: vote.clone(),
            },
            Input::Wire(ConsensusMessageV2::TimeoutCertificate(certificate)) => {
                Event::TimeoutCertificateReceived {
                    tag,
                    certificate: certificate.clone(),
                }
            }
        }
    }
    fn effects(&mut self, node: usize, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Persist { tag, entry } => {
                    self.wal[node].push(entry.clone());
                    // Crash after the append is durable but before its acknowledgment.
                    if !self.crashed && self.crash == Some((node, self.wal[node].len())) {
                        self.crashed = true;
                        self.queue.retain(|(owner, input)| {
                            *owner != node || matches!(input, Input::Wire(_))
                        });
                        self.reducers[node] = Reducer::recover(
                            context(),
                            Some(ValidatorId::repeat((node + 1) as u8)),
                            Generation::new(100),
                            self.wal[node].clone(),
                        )
                        .unwrap();
                        let tag = self.reducers[node].current_tag();
                        self.local(node, Event::ResumeAfterReplay { tag });
                    } else {
                        self.local(
                            node,
                            Event::Persisted {
                                tag,
                                id: entry.id(),
                            },
                        );
                    }
                }
                Effect::Sign { tag, .. } => self.local(
                    node,
                    Event::Signed {
                        tag,
                        signature: OpaqueSignature::new(vec![node as u8; 8]),
                    },
                ),
                Effect::FetchBody {
                    tag,
                    round,
                    subject,
                    ..
                } => self.local(
                    node,
                    Event::BodyAvailable {
                        tag,
                        round,
                        subject,
                    },
                ),
                Effect::StoreBody {
                    tag,
                    round,
                    subject,
                } => self.local(
                    node,
                    Event::BodyStored {
                        tag,
                        round,
                        subject,
                    },
                ),
                Effect::ValidateBody {
                    tag,
                    round,
                    subject,
                } => self.local(
                    node,
                    Event::ValidationCompleted {
                        tag,
                        round,
                        subject,
                        valid: true,
                    },
                ),
                Effect::Apply { tag, subject, .. } => {
                    self.local(node, Event::ApplicationCompleted { tag, subject })
                }
                Effect::Broadcast(message) => {
                    for recipient in 0..4 {
                        if recipient == node || self.offline == Some(recipient) {
                            continue;
                        }
                        // Before healing, lose a deterministic subset. Afterward all are delivered.
                        if self.tick < self.drop_until && self.random() % 3 != 0 {
                            continue;
                        }
                        self.queue.push((recipient, Input::Wire(message.clone())));
                        if self.random() % 5 == 0 {
                            self.queue.push((recipient, Input::Wire(message.clone())));
                        }
                    }
                }
                Effect::EnterView { .. } => {}
                Effect::ReportEquivocation { .. } | Effect::ReportInvalidCertifiedBody { .. } => {
                    panic!("honest trace emitted an evidence fault")
                }
            }
        }
    }
    fn pump(&mut self, budget: usize) {
        for _ in 0..budget {
            if self.queue.is_empty() {
                break;
            }
            self.steps += 1;
            let at = self.random() as usize % self.queue.len();
            let (node, input) = self.queue.swap_remove(at);
            let event = self.event(node, &input);
            let result = self.reducers[node]
                .step(event.clone())
                .unwrap_or_else(|error| {
                    panic!("node={node} tick={} event={event:?}: {error:?}", self.tick)
                });
            if result.disposition() == StepDisposition::Ignored(IgnoreReason::Busy) {
                // This models the adapter's required exact completion/ingress retention.
                self.queue.push((node, input));
            } else {
                self.effects(node, result.into_effects());
            }
            let decisions = self
                .reducers
                .iter()
                .filter_map(|r| r.durable_state().decision().map(|q| q.subject()))
                .collect::<BTreeSet<_>>();
            assert!(
                decisions.len() <= 1,
                "honest reducers decided different subjects"
            );
        }
    }
    fn run(mut self) -> bool {
        for tick in 0..100 {
            self.tick = tick;
            for node in 0..4 {
                if self.offline == Some(node) {
                    continue;
                }
                let tag = self.reducers[node].current_tag();
                if self.reducers[node].applied_subject().is_some() {
                    self.local(node, Event::RetransmitElapsed { tag });
                    continue;
                }
                let round = Round::new(tag.height(), tag.view());
                if self.reducers[node].context().leader(tag.view())
                    == ValidatorId::repeat((node + 1) as u8)
                    && self.reducers[node]
                        .durable_state()
                        .timeout_intent(round)
                        .is_none()
                    && self.proposed.insert((node, tag))
                {
                    let subject = self.reducers[node].durable_state().locked().map_or(
                        Subject::repeat(42 + u8::try_from(tag.view()).expect("bounded trace view")),
                        |q| q.subject(),
                    );
                    self.local(
                        node,
                        Event::LocalProposalReady {
                            tag,
                            manifest: PayloadManifest::new(
                                subject,
                                Digest::repeat(43),
                                Digest::repeat(44),
                                128,
                                2,
                            ),
                        },
                    );
                }
                self.local(node, Event::RetransmitElapsed { tag });
                if tick % 8 == 7 {
                    self.local(node, Event::TimeoutElapsed { tag });
                }
            }
            self.pump(1000);
            if self
                .reducers
                .iter()
                .enumerate()
                .all(|(n, r)| self.offline == Some(n) || r.ready_to_finish())
            {
                return self.crashed;
            }
            assert!(
                self.queue.len() < 20000,
                "simulation exceeded its bounded queue"
            );
        }
        panic!(
            "no progress: steps={} tags={:?} applied={:?}",
            self.steps,
            self.reducers
                .iter()
                .map(Reducer::current_tag)
                .collect::<Vec<_>>(),
            self.reducers
                .iter()
                .map(Reducer::applied_subject)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn four_reducers_reordered_duplicates_and_loss_heal_progress() {
    for seed in 0..64 {
        Network::new(seed, None, None, 12).run();
    }
}
#[test]
fn four_reducers_progress_with_each_single_validator_offline() {
    for offline in 0..4 {
        for seed in 0..32 {
            Network::new(seed, Some(offline), None, 4).run();
        }
    }
}
#[test]
fn four_reducers_recover_after_durable_append_before_acknowledgment() {
    let mut observed_crashes = 0;
    for node in 0..4 {
        let mut node_crashes = 0;
        for boundary in 1..=4 {
            for seed in 0..8 {
                if Network::new(seed, None, Some((node, boundary)), 6).run() {
                    node_crashes += 1;
                    observed_crashes += 1;
                }
            }
        }
        assert!(
            node_crashes > 0,
            "every validator must actually reopen at least once"
        );
    }
    assert!(
        observed_crashes >= 64,
        "crash schedule must exercise real reopen cuts"
    );
    println!("actual append-before-ack restarts: {observed_crashes}");
}
