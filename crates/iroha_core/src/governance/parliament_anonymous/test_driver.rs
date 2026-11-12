//! A deterministic driver of anonymous attempts for the reducer's tests.
//!
//! It runs blocks with strictly increasing heights and times. A block's pass draws every
//! generation whose exact-slot pulse the block carries or already carried (G1), then applies
//! every lifecycle item due at the block's time in G2a order, each at its pass position;
//! transactions run after the pass and read the block's height and time.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            AnonymousRosterBindingV1, BallotClosureRecordV1, BallotScopeV1, Digest512,
            FrozenLifecyclePolicyV1, FrozenParliamentAttemptPolicyV1, ParliamentBallotParametersV1,
            ParliamentBallotPurposeV1, ParliamentCandidateTestingPolicyV1,
            ParliamentDecisionRuleV1, ParliamentLifecycleParametersV1, ProofProfileIdV1,
        },
        types::{
            BeaconPulseId, BeaconSessionId, DeliberationPhaseV1, GovernanceExpectedHeadAbsentV1,
            GovernanceExpectedHeadV1, GovernanceStageV1, ParliamentBody, ProposalContentId,
            RiskTierV1,
        },
    },
};

use super::{
    AnonymousAttemptOriginV1, AnonymousBallotDecisionV1, AnonymousGovernanceAttemptV1,
    AnonymousLifecycleItemV1, AnonymousParliamentAttemptStateV1, AnonymousParliamentErrorV1,
    AnonymousSortitionPulseV1, BodyDecisionModeV1, stage_for_body,
};

/// The proposal every test attempt is about.
pub(super) const CONTENT: ProposalContentId = ProposalContentId::new([0x01; 32]);
/// The compare-and-set subject of every test attempt.
pub(super) const SUBJECT: [u8; 32] = [0xe0; 32];
/// Height of the creating block.
pub(super) const CREATED_HEIGHT: u64 = 100;
/// Time of the creating block.
pub(super) const CREATED_MS: u64 = 1_000_000;
/// Every lifecycle window of the test policy.
pub(super) const WINDOW_MS: u64 = 10_000;
/// The test policy's native pulse cadence.
pub(super) const CADENCE_BLOCKS: u64 = 20;
/// Seat target of every public body of the test policy.
pub(super) const PUBLIC_SEATS: u16 = 5;
/// Seat target of both juries of the test policy.
/// Twenty-five seats admit a narrow approval: 13 against 12 is a margin below 5%.
pub(super) const JURY_SEATS: u16 = 25;
/// Alternates of every body of the test policy.
pub(super) const ALTERNATES: u16 = 2;
/// The finding root every public body endorses.
pub(super) const FINDING_ROOT: [u8; 32] = [0xc0; 32];
/// The `proposal_digest` every attempt of the driver fixes at creation.
pub(super) const PROPOSAL_DIGEST: Digest512 = Digest512::new([0x02; 64]);

/// The network of every test attempt.
pub(super) fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0x5a; Hash::LENGTH]),
    ))
}

/// The candidate-testing purpose of the genesis policy.
pub(super) fn candidate() -> ParliamentBallotPurposeV1 {
    ParliamentBallotPurposeV1::CandidateTesting(ParliamentCandidateTestingPolicyV1::CANDIDATE_V1)
}

/// The contract-lifecycle pipeline with its conditional Confirmation Jury.
pub(super) fn pipeline() -> Vec<ParliamentBody> {
    vec![
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::OversightCommittee,
        ParliamentBody::PolicyJury,
        ParliamentBody::ConfirmationJury,
    ]
}

/// The public bodies of [`pipeline`].
pub(super) fn public_bodies() -> Vec<ParliamentBody> {
    pipeline()
        .into_iter()
        .filter(|body| BodyDecisionModeV1::of(*body) == BodyDecisionModeV1::PublicFinding)
        .collect()
}

/// The lifecycle parameters of the test policy: short windows, small bodies.
pub(super) fn lifecycle_parameters() -> ParliamentLifecycleParametersV1 {
    let mut parameters = ParliamentLifecycleParametersV1::default();
    parameters.lifecycle = FrozenLifecyclePolicyV1 {
        qualification_window_ms: WINDOW_MS,
        invitation_window_ms: WINDOW_MS,
        orientation_window_ms: WINDOW_MS,
        evidence_window_ms: WINDOW_MS,
        questions_window_ms: WINDOW_MS,
        responses_window_ms: WINDOW_MS,
        deliberation_window_ms: WINDOW_MS,
        public_finding_window_ms: WINDOW_MS,
        enactment_delay_ms: 20_000,
    };
    for sizing in &mut parameters.body_sizes {
        sizing.target_seats =
            if BodyDecisionModeV1::of(sizing.body) == BodyDecisionModeV1::AnonymousBallot {
                JURY_SEATS
            } else {
                PUBLIC_SEATS
            };
        sizing.alternate_cap = ALTERNATES;
    }
    parameters
}

/// The test policy frozen from [`lifecycle_parameters`].
pub(super) fn policy_with(
    parameters: &ParliamentLifecycleParametersV1,
) -> FrozenParliamentAttemptPolicyV1 {
    FrozenParliamentAttemptPolicyV1::freeze_v1(
        &pipeline(),
        parameters,
        Some(&ParliamentBallotParametersV1::default()),
        1_000,
        CADENCE_BLOCKS,
    )
    .expect("test attempt policy")
}

/// The test policy.
pub(super) fn policy() -> FrozenParliamentAttemptPolicyV1 {
    policy_with(&lifecycle_parameters())
}

/// The initial snapshot of attempt `sequence`.
pub(super) fn attempt(sequence: u32, risk_tier: RiskTierV1) -> AnonymousGovernanceAttemptV1 {
    AnonymousGovernanceAttemptV1::new_v1(CONTENT, sequence, risk_tier)
}

/// The compare-and-set head of every test attempt.
pub(super) fn head() -> GovernanceExpectedHeadV1 {
    GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
        subject_id: SUBJECT,
    })
}

/// An anchor.
pub(super) const fn anchor(height: u64, position: u32, timestamp_ms: u64) -> GovernanceAnchorV1 {
    GovernanceAnchorV1 {
        height,
        position,
        timestamp_ms,
    }
}

/// `count` distinct citizens in canonical order.
pub(super) fn citizens(count: u32) -> Vec<AccountId> {
    let mut accounts: Vec<_> = (0..count)
        .map(|index| {
            let mut seed = vec![0x77; 32];
            seed[..4].copy_from_slice(&index.to_be_bytes());
            AccountId::new(
                KeyPair::try_from_seed(seed, Algorithm::Ed25519)
                    .expect("derive citizen key")
                    .public_key()
                    .clone(),
            )
        })
        .collect();
    accounts.sort();
    accounts.dedup();
    accounts
}

/// The authenticated pulse of `slot`.
pub(super) fn pulse(slot: u64) -> AnonymousSortitionPulseV1 {
    let mut id = [0x31; 32];
    id[..8].copy_from_slice(&slot.to_be_bytes());
    let mut output = [0x32; 32];
    output[..8].copy_from_slice(&slot.to_be_bytes());
    AnonymousSortitionPulseV1 {
        beacon_session_id: BeaconSessionId::for_network_v1(&network()),
        height: slot,
        pulse_id: BeaconPulseId::new(id),
        output,
    }
}

/// The roster binding a credential adapter seals for `seats` seats of `body`.
pub(super) fn roster_binding(body: ParliamentBody, seats: usize) -> AnonymousRosterBindingV1 {
    let mut digest = [0x99; 64];
    digest[0] = body as u8;
    AnonymousRosterBindingV1 {
        list_digest: Digest512::new(digest),
        seats: u16::try_from(seats).expect("bounded seats"),
        profile_id: ProofProfileIdV1::V1,
    }
}

/// The kind and body of each item, without its occurrence.
pub(super) fn names(
    items: &[AnonymousLifecycleItemV1],
) -> Vec<(&'static str, Option<ParliamentBody>)> {
    items
        .iter()
        .map(|item| (item.name(), item.body()))
        .collect()
}

/// How invited accounts answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Answer {
    /// Every primary accepts; alternates do not answer.
    Primaries,
    /// The first `n` primaries accept; every other invitation declines.
    First(usize),
    /// Nobody answers.
    Nobody,
}

/// A deterministic run of one attempt.
pub(super) struct Driver {
    /// The attempt.
    pub(super) state: AnonymousParliamentAttemptStateV1,
    /// Height of the current block.
    pub(super) height: u64,
    /// Time of the current block.
    pub(super) now_ms: u64,
    /// The eligible citizens.
    pub(super) citizens: Vec<AccountId>,
}

impl Driver {
    /// A candidate-testing attempt of sequence zero over `citizens` eligible citizens,
    /// created in block [`CREATED_HEIGHT`] at [`CREATED_MS`].
    pub(super) fn new(risk_tier: RiskTierV1, citizens_count: u32) -> Self {
        Self::with(
            candidate(),
            attempt(0, risk_tier),
            policy(),
            0,
            citizens_count,
        )
    }

    /// An attempt of the given purpose, snapshot, policy and inherited redraw count.
    pub(super) fn with(
        purpose: ParliamentBallotPurposeV1,
        attempt: AnonymousGovernanceAttemptV1,
        policy: FrozenParliamentAttemptPolicyV1,
        randomness_redraws_before_attempt: u32,
        citizens_count: u32,
    ) -> Self {
        let state = AnonymousParliamentAttemptStateV1::try_new(
            purpose,
            attempt,
            policy,
            PROPOSAL_DIGEST,
            [0xd0; 32],
            head(),
            AnonymousAttemptOriginV1 {
                created_at_ms: CREATED_MS,
                randomness_redraws_before_attempt,
            },
        )
        .expect("new attempt");
        Self {
            state,
            height: CREATED_HEIGHT,
            now_ms: CREATED_MS,
            citizens: citizens(citizens_count),
        }
    }

    /// The attempt identifier.
    pub(super) fn id(&self) -> iroha_data_model::governance::types::GovernanceAttemptId {
        self.state.attempt().id
    }

    /// Check that the State restores: it validates, and a Norito roundtrip preserves it.
    pub(super) fn assert_restores(&self) {
        assert_restores(&self.state);
    }

    /// Start the next block `dt_ms` later and run its pass; returns the applied items.
    pub(super) fn block(&mut self, dt_ms: u64) -> Vec<AnonymousLifecycleItemV1> {
        assert!(dt_ms > 0, "block times strictly increase");
        self.height += 1;
        self.now_ms += dt_ms;
        self.pass()
    }

    /// Start the block at `height`, `dt_ms` later, and run its pass.
    pub(super) fn block_at(&mut self, height: u64, dt_ms: u64) -> Vec<AnonymousLifecycleItemV1> {
        assert!(height > self.height && dt_ms > 0);
        self.height = height;
        self.now_ms += dt_ms;
        self.pass()
    }

    /// The anchor of position `position` in the current block's pass.
    pub(super) const fn at(&self, position: u32) -> GovernanceAnchorV1 {
        anchor(self.height, position, self.now_ms)
    }

    /// Run the current block's pass: G1 draws, then the due G2a items in order.
    pub(super) fn pass(&mut self) -> Vec<AnonymousLifecycleItemV1> {
        let mut position = 0;
        for slot in self.state.awaiting_slots() {
            if slot <= self.height {
                self.state
                    .draw_generation(&network(), pulse(slot), self.at(position))
                    .expect("draw the generation of a carried pulse");
                position += 1;
            }
        }
        let due = self
            .state
            .due_lifecycle_items(self.now_ms)
            .expect("lifecycle items");
        let mut applied = Vec::new();
        for item in due {
            let result = self
                .state
                .apply_lifecycle_item(item.item, self.at(position), |seating| {
                    Ok(roster_binding(seating.body, seating.seats.len()))
                });
            // An item an earlier item of this pass ended lapses, uncharged.
            if result.is_ok() {
                applied.push(item.item);
                position += 1;
            }
        }
        applied
    }

    /// Advance to the earliest live lifecycle item's due time and run that block's pass.
    pub(super) fn step(&mut self) -> Vec<AnonymousLifecycleItemV1> {
        let due = self
            .state
            .lifecycle_items()
            .expect("lifecycle items")
            .first()
            .map(|item| item.due_ms)
            .expect("a live lifecycle item");
        let dt = due.saturating_sub(self.now_ms).max(1);
        self.block(dt)
    }

    /// Run the passes of the earliest due items until one item that `done` accepts is
    /// applied; returns every applied item.
    pub(super) fn run_until(
        &mut self,
        done: impl Fn(&AnonymousLifecycleItemV1) -> bool,
    ) -> Vec<AnonymousLifecycleItemV1> {
        let mut applied = Vec::new();
        for _ in 0..64 {
            let step = self.step();
            let finished = step.iter().any(&done);
            applied.extend(step);
            if finished {
                return applied;
            }
        }
        panic!("the awaited item was never applied: {applied:?}");
    }

    /// Start qualification and end it at its deadline.
    pub(super) fn qualify(&mut self) {
        assert_eq!(
            self.block(1_000),
            vec![AnonymousLifecycleItemV1::StartQualification]
        );
        assert_eq!(
            self.step(),
            vec![AnonymousLifecycleItemV1::EndQualification]
        );
        assert_ne!(self.state.attempt().stage, GovernanceStageV1::Qualification);
    }

    /// Register the initial generation from every citizen in the current block.
    pub(super) fn register_initial(&mut self) -> super::AnonymousRegistrationV1 {
        let id = self.id();
        self.state
            .register_initial_sortition(
                id,
                &network(),
                self.height,
                self.now_ms,
                self.citizens.clone(),
            )
            .expect("register the initial generation")
    }

    /// Register the retry generation from every citizen in the current block.
    pub(super) fn register_retry(
        &mut self,
    ) -> Result<super::AnonymousRegistrationV1, AnonymousParliamentErrorV1> {
        let id = self.id();
        self.state.register_sortition_retry(
            id,
            &network(),
            self.height,
            self.now_ms,
            self.citizens.clone(),
        )
    }

    /// Run blocks up to the earliest awaiting slot, whose pass draws it.
    pub(super) fn advance_to_slot(&mut self) {
        let slot = *self
            .state
            .awaiting_slots()
            .first()
            .expect("a generation awaits its pulse");
        let height = slot.max(self.height + 1);
        self.block_at(height, 1_000);
    }

    /// The invited accounts of `body`'s open election answer as `answer` in the current
    /// block.
    pub(super) fn answer(&mut self, body: ParliamentBody, answer: Answer) {
        let id = self.id();
        let (generation, election) = self.state.active_election(body).expect("an election");
        let election = &self.state.generations()[generation].elections()[election];
        let election_id = election.id();
        let primaries: Vec<_> = election
            .primaries()
            .iter()
            .map(|seat| seat.member.clone())
            .collect();
        let alternates: Vec<_> = election
            .alternates()
            .iter()
            .map(|seat| seat.member.clone())
            .collect();
        let accepting = match answer {
            Answer::Primaries => primaries.len(),
            Answer::First(count) => count,
            Answer::Nobody => return,
        };
        let binding = BodyDecisionModeV1::of(body) == BodyDecisionModeV1::AnonymousBallot;
        for (index, member) in primaries.iter().chain(&alternates).enumerate() {
            let accept = index < accepting && index < primaries.len();
            if answer == Answer::Primaries && index >= primaries.len() {
                continue;
            }
            let recorded = if accept && binding {
                self.state
                    .accept_binding_seat(id, election_id, body, member, self.now_ms)
            } else {
                self.state.record_invitation_response(
                    id,
                    election_id,
                    body,
                    member,
                    accept,
                    self.now_ms,
                )
            };
            recorded.expect("record the response");
        }
    }

    /// Advance the current body through its phases until it enters Reflection.
    pub(super) fn deliberate(&mut self, body: ParliamentBody) {
        assert_eq!(self.state.attempt().stage, stage_for_body(body));
        for _ in 0..5 {
            assert_eq!(names(&self.step()), vec![("AdvancePhase", Some(body))]);
        }
        assert_eq!(
            self.state
                .body(body)
                .and_then(|state| state.current_phase())
                .map(|phase| phase.phase),
            Some(DeliberationPhaseV1::Reflection)
        );
    }

    /// A quorum of the current public `body` endorses [`FINDING_ROOT`] in the current
    /// block.
    pub(super) fn endorse_quorum(&mut self, body: ParliamentBody) {
        let id = self.id();
        let state = self.state.body(body).expect("sealed");
        let instance = state.sealed_body().body_instance_id;
        let quorum = usize::try_from(
            iroha_data_model::governance::types::parliament_quorum_seats_v1(u32::from(
                state.sealed_body().seats,
            )),
        )
        .expect("small");
        let members: Vec<_> = state
            .seats()
            .iter()
            .take(quorum)
            .map(|seat| seat.member.clone())
            .collect();
        for member in members {
            self.state
                .endorse_public_finding(id, instance, FINDING_ROOT, &member, self.now_ms)
                .expect("endorse");
        }
    }

    /// Run a sealed public `body` from its Orientation to its concluded finding.
    pub(super) fn conclude_public(&mut self, body: ParliamentBody) {
        self.deliberate(body);
        self.block(1_000);
        self.endorse_quorum(body);
        assert_eq!(
            names(&self.block(1_000)),
            vec![("ConcludeFinding", Some(body))]
        );
    }

    /// Run a fresh attempt to its Policy Jury in Reflection: qualification, one initial
    /// generation where every primary accepts, and every public finding.
    pub(super) fn at_policy_reflection(risk_tier: RiskTierV1) -> Self {
        let mut driver = Self::new(risk_tier, 40);
        driver.qualify();
        driver.register_initial();
        driver.advance_to_slot();
        for body in pipeline() {
            if body != ParliamentBody::ConfirmationJury {
                driver.answer(body, Answer::Primaries);
            }
        }
        let closed = driver.step();
        assert!(
            closed
                .iter()
                .all(|item| matches!(item, AnonymousLifecycleItemV1::CloseInvitations { .. })),
            "{closed:?}"
        );
        assert_eq!(closed.len(), pipeline().len() - 1);
        for body in public_bodies() {
            driver.conclude_public(body);
        }
        driver.deliberate(ParliamentBody::PolicyJury);
        driver.assert_restores();
        driver
    }

    /// The closure record of the jury `body` with `(aye, nay, abstain)`: its ballot opens
    /// at the first pass at or after its opening time, its clock follows one block later, and
    /// it closes one casting window after the clock. The driver moves to the closing block.
    pub(super) fn close_record(
        &mut self,
        body: ParliamentBody,
        (aye, nay, abstain): (u32, u32, u32),
    ) -> BallotClosureRecordV1 {
        let sealed = *self.state.sealed_body(body).expect("sealed jury");
        let opening_at = self.state.opening_at_ms(body).expect("opening time");
        let casting_window_ms = self
            .state
            .policy()
            .binding
            .expect("binding policy")
            .casting_window_ms;
        let risk_tier = self.state.attempt().risk_tier;
        let decision = ParliamentDecisionRuleV1::V1
            .decide(body, risk_tier, sealed.seats, aye, nay, abstain)
            .expect("valid counts");
        // The opening pass.
        self.block(opening_at.saturating_sub(self.now_ms).max(1));
        let opened = self.at(0);
        // The casting clock of the next block.
        self.block(1_000);
        let clock = (self.height, self.now_ms);
        let closing_ms = clock.1 + casting_window_ms;
        // The closing pass.
        self.block(closing_ms - self.now_ms);
        BallotClosureRecordV1 {
            network_id: network(),
            proposal_content_id: CONTENT,
            proposal_digest: PROPOSAL_DIGEST,
            governance_attempt_id: self.id(),
            election_attempt_id: sealed.election_attempt_id,
            body_instance_id: sealed.body_instance_id,
            body_role: body,
            scope: BallotScopeV1::new([0x44; 64]),
            seats: sealed.seats,
            profile_id: ProofProfileIdV1::V1,
            policy_hash: Digest512::new([0xaa; 64]),
            list_digest: roster_binding(body, usize::from(sealed.seats)).list_digest,
            registration_mapping_digest: Digest512::new([0x04; 64]),
            opening_anchor: opened,
            casting_clock_height: clock.0,
            casting_clock_timestamp_ms: clock.1,
            casting_window_ms,
            closing_ms,
            closure_anchor: self.at(0),
            corpus_root: Digest512::new([0x05; 64]),
            accepted_ballots: aye + nay + abstain,
            aye,
            nay,
            abstain,
            outcome: decision.outcome,
            narrow: decision.narrow,
        }
    }

    /// Close the jury `body` with these counts, registering a Confirmation Jury from every
    /// citizen when asked.
    pub(super) fn close_ballot(
        &mut self,
        body: ParliamentBody,
        counts: (u32, u32, u32),
    ) -> Result<AnonymousBallotDecisionV1, AnonymousParliamentErrorV1> {
        let record = self.close_record(body, counts);
        let id = self.id();
        let eligible = self.citizens.clone();
        self.state.close_ballot(id, record, &network(), || eligible)
    }
}

/// Check that `state` validates and survives a Norito roundtrip unchanged.
pub(super) fn assert_restores(state: &AnonymousParliamentAttemptStateV1) {
    assert_eq!(state.validate(&network()), Ok(()), "{:?}", state.attempt());
    let bytes = norito::encode_canonical(state).expect("encode");
    let restored =
        norito::decode_canonical::<AnonymousParliamentAttemptStateV1>(&bytes).expect("decode");
    assert_eq!(&restored, state);
    assert_eq!(restored.validate(&network()), Ok(()));
}
