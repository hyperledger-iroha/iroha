//! Parliament's items of the positioned pass over World: every block is run on a World
//! rebuilt from the rows the previous block left, and what a block leaves is a cut restore
//! accepts. Registration proofs are the credential tests' genuine cached ones.

use std::num::NonZeroU64;

use iroha_data_model::{
    block::BlockHeader,
    governance::{
        anonymous_ballot::{
            BallotOutcomeV1, ParliamentCandidateTestingPolicyV1, ParliamentCredentialRecordV1,
        },
        types::AssignmentId,
    },
    parameter::Parameter,
};
use iroha_model_base::chain::ChainId;

use super::*;
use crate::{
    governance::parliament_anonymous::{
        AnonymousAttemptRejectionV1, AnonymousElectionCredentialsV1, AnonymousItemOutcomeV1,
        AnonymousParliamentAttemptStateV1,
        credentials_tests::{
            JURY, answer_public_bodies, close_invitations, drawn_small_jury, jury_effect, register,
        },
        test_driver::{CREATED_HEIGHT, CREATED_MS, Driver, network, public_bodies, pulse},
    },
    runtime::catalog::additions::PARLIAMENT_ANONYMOUS,
    state::{State, World, parliament_anonymous_tables::validate_cut},
};
use iroha_data_model::block::execution_output::validate_governance_start_rows_v1;
use iroha_data_model::governance::types::RiskTierV1;

/// The rows of the three tables between two blocks, and the committed `K` when one is set.
#[derive(Clone, Default)]
struct Rows {
    attempts: Vec<(GovernanceAttemptId, AnonymousParliamentAttemptStateV1)>,
    registrations: Vec<(AssignmentId, ParliamentCredentialRecordV1)>,
    boards: Vec<(BallotScopeV1, ParliamentBallotBoardRecordV1)>,
    limit: Option<u16>,
    /// The time of the parent block, when the next block has one: the height-progress
    /// ticket is due from it.
    parent_time_ms: Option<u64>,
}

fn header(height: u64, timestamp_ms: u64) -> BlockHeader {
    BlockHeader::new(
        NonZeroU64::new(height).expect("nonzero height"),
        None,
        None,
        timestamp_ms,
        0,
    )
}

fn no_pulse(_: u64) -> Option<AnonymousSortitionPulseV1> {
    None
}

impl Rows {
    /// World as `driver` and its `registrations` are committed.
    fn of(driver: &Driver, registrations: &AnonymousElectionCredentialsV1) -> Self {
        Self {
            attempts: vec![(driver.id(), driver.state.clone())],
            registrations: registrations.clone(),
            ..Self::default()
        }
    }

    /// A State on the driver's network whose World activates the family and keeps the rows.
    fn state(&self) -> State {
        self.state_with(true)
    }

    /// A State on the driver's network whose World keeps the rows. Without `installed` its
    /// selection does not install the family: rows no restore would accept, kept to observe
    /// that nothing reads them.
    fn state_with(&self, installed: bool) -> State {
        let world = World::new();
        if installed {
            crate::runtime::activate_additions_for_testing(&world, PARLIAMENT_ANONYMOUS.module)
                .expect("activate the anonymous family");
        }
        let mut block = world.block();
        block
            .set_parameter_for_testing(Parameter::Custom(
                ParliamentCandidateTestingPolicyV1::CANDIDATE_V1
                    .to_custom_parameter()
                    .expect("candidate policy"),
            ))
            .expect("select candidate testing");
        if let Some(limit) = self.limit {
            let ballot = ParliamentBallotParametersV1 {
                max_nonterminal_attempts: limit,
                ..ParliamentBallotParametersV1::default()
            };
            block
                .set_parameter_for_testing(Parameter::Custom(
                    ballot.to_custom_parameter().expect("ballot parameters"),
                ))
                .expect("commit the ballot parameters");
        }
        for (id, attempt) in &self.attempts {
            block
                .parliament_anonymous_attempts
                .insert(*id, attempt.clone());
        }
        for (assignment, record) in &self.registrations {
            block
                .parliament_anonymous_registrations
                .insert(*assignment, *record);
        }
        for (scope, board) in &self.boards {
            block
                .parliament_anonymous_boards
                .insert(*scope, board.clone());
        }
        block.commit();
        State::new_with_chain_and_network_id_for_testing(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
            ChainId::from("parliament-anonymous-pass-tests"),
            network(),
        )
    }

    /// Run the pass of the block `(height, timestamp_ms)` and keep the rows it leaves.
    /// `restorable` checks them as a restored cut.
    fn run(
        &mut self,
        height: u64,
        timestamp_ms: u64,
        pulse_at: impl Fn(u64) -> Option<AnonymousSortitionPulseV1>,
        restorable: bool,
    ) -> Result<AnonymousPassReportV1, GovernancePassErrorV1<AnonymousPassWorldErrorV1>> {
        let state = self.state();
        let mut block = state.block(header(height, timestamp_ms));
        // Tables that disagree have no due answer; the pass reports them.
        let due = parliament_due(
            &block.world,
            height,
            timestamp_ms,
            self.parent_time_ms,
            |slot| pulse_at(slot).is_some(),
        )
        .ok();
        let report = run_parliament_pass(&mut block, pulse_at, self.parent_time_ms)?;
        if let Some(due) = due {
            assert_eq!(
                due,
                report.governance_due_applied(),
                "the due predicate holds exactly when the pass applies work"
            );
        }
        if restorable {
            validate_cut(&block.world, &network(), height)
                .expect("the pass leaves a cut restore accepts");
        }
        self.attempts = block
            .world
            .parliament_anonymous_attempts
            .iter()
            .map(|(id, attempt)| (*id, attempt.clone()))
            .collect();
        self.registrations = block
            .world
            .parliament_anonymous_registrations
            .iter()
            .map(|(assignment, record)| (*assignment, *record))
            .collect();
        self.boards = block
            .world
            .parliament_anonymous_boards
            .iter()
            .map(|(scope, board)| (*scope, board.clone()))
            .collect();
        Ok(report)
    }

    /// A block without a pulse whose rows restore.
    fn block(&mut self, height: u64, timestamp_ms: u64) -> AnonymousPassReportV1 {
        self.run(height, timestamp_ms, no_pulse, true)
            .expect("the pass")
    }

    fn attempt(&self) -> &AnonymousParliamentAttemptStateV1 {
        &self.attempts[0].1
    }

    /// Casting windows the committed rows count as open.
    fn open_windows(&self) -> usize {
        let state = self.state();
        let block = state.block(header(1, 1));
        open_casting_windows(&*block.world.parliament_anonymous_boards)
    }

    /// The earliest due time of the attempt's live lifecycle items.
    fn next_due(&self) -> u64 {
        self.attempt().lifecycle_items().expect("items")[0].due_ms
    }
}

/// Rows whose small jury sealed in World, with the block of the seal, its outputs, and the
/// reducer's own outcomes of the same block.
fn sealed_in_world() -> (
    Rows,
    Driver,
    AnonymousPassReportV1,
    Vec<AnonymousItemOutcomeV1>,
) {
    let mut driver = drawn_small_jury();
    let mut records = Vec::new();
    register(&mut driver, &mut records, 0..3);
    answer_public_bodies(&mut driver);
    let mut rows = Rows::of(&driver, &records);
    let outcomes = close_invitations(&mut driver, &records);
    let report = rows.block(driver.height, driver.now_ms);
    (rows, driver, report, outcomes)
}

/// Rows whose jury is in Deliberation with its board sealed and unscheduled, and the height
/// of the last block. The public bodies conclude through recorded endorsements, which are
/// transactions: the reducer records them here.
fn jury_in_deliberation() -> (Rows, u64) {
    let (mut rows, mut driver, _, _) = sealed_in_world();
    assert_eq!(rows.attempt(), &driver.state);
    for body in public_bodies() {
        driver.conclude_public(body);
    }
    rows.attempts = vec![(driver.id(), driver.state.clone())];
    let mut height = driver.height;
    for _ in 0..4 {
        height += 1;
        let report = rows.block(height, rows.next_due());
        assert_eq!(report.processed, 1);
        assert!(report.outputs().is_empty());
    }
    assert_eq!(rows.attempt().opening_at_ms(JURY), None);
    assert_eq!(rows.boards[0].1.opening_at_ms, None);
    (rows, height)
}

fn pass_anchor(output: &GovernanceBlockStartOutputV1) -> GovernanceAnchorV1 {
    assert_eq!(output.validate(), Ok(()));
    *output.anchor.pass_anchor().expect("a positioned output")
}

#[test]
fn the_world_pass_seals_and_publishes_what_the_reducer_plans() {
    let (mut rows, driver, report, outcomes) = sealed_in_world();
    assert_eq!(rows.attempts, vec![(driver.id(), driver.state.clone())]);
    let sealed = jury_effect(&outcomes)
        .board
        .as_ref()
        .expect("the jury sealed");
    assert_eq!(
        rows.boards
            .iter()
            .map(|(_, board)| board)
            .collect::<Vec<_>>(),
        [&sealed.board]
    );
    let scope = *typed_board(sealed.board.clone()).expect("board").scope();
    assert_eq!(rows.boards[0].0, scope);
    let applied = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, AnonymousItemOutcomeV1::Applied(_)))
        .count();
    assert_eq!(usize::try_from(report.processed), Ok(applied));
    assert_eq!(report.clocks, 0);
    assert!(report.governance_due_applied());
    let outputs = report.outputs();
    let [output] = outputs.as_slice() else {
        panic!("only the jury's seal publishes: {outputs:?}")
    };
    assert_eq!(
        output.event,
        GovernanceBlockStartEventV1::SealedCredentialListPublished(sealed.published.clone())
    );
    let anchor = pass_anchor(output);
    assert_eq!(
        (anchor.height, anchor.timestamp_ms),
        (driver.height, driver.now_ms)
    );
    assert_eq!(
        rows.attempt().body(JURY).expect("sealed jury").sealed_at(),
        anchor
    );

    // Every positioned item emitted its receipt first, and the rows are a valid leading
    // sequence of their block. The seal's receipt names the board World stored.
    let receipts = report.receipts();
    assert_eq!(receipts.len(), applied);
    assert_eq!(
        validate_governance_start_rows_v1(report.rows.rows(), driver.height),
        Ok(applied + 1)
    );
    let applied_receipts: Vec<_> = receipts
        .iter()
        .map(|receipt| match receipt.event {
            GovernanceBlockStartEventV1::LifecycleApplied(applied) => (receipt.anchor, applied),
            ref other => panic!("a lifecycle receipt: {other:?}"),
        })
        .collect();
    let seals: Vec<_> = applied_receipts
        .iter()
        .filter(|(_, applied)| applied.board.is_some())
        .collect();
    let [(sealed_at, seal)] = seals.as_slice() else {
        panic!("one item stored a board: {seals:?}")
    };
    assert_eq!(*sealed_at, output.anchor);
    assert!(matches!(
        seal.item,
        GovernanceLifecycleItemV1::CloseInvitations { body: JURY, .. }
    ));
    assert_eq!(
        seal.board,
        Some(LifecycleBoardDigestV1 {
            scope,
            record_digest: board_record_digest_v1(&sealed.board).expect("digest"),
        })
    );
    // The last receipt carries the State the block left.
    let (_, last) = applied_receipts.last().expect("a receipt");
    let left = rows.attempt();
    assert_eq!(
        (
            last.governance_attempt_id,
            last.status,
            last.stage,
            last.attempt_state_digest
        ),
        (
            driver.id(),
            left.attempt().status,
            left.attempt().stage,
            attempt_state_digest_v1(left).expect("digest")
        )
    );

    // Nothing is due one millisecond later, and an idle block changes nothing.
    let before = rows.clone();
    let idle = rows.block(driver.height + 1, driver.now_ms + 1);
    assert!(!idle.governance_due_applied());
    assert!(idle.outputs().is_empty());
    assert_eq!(rows.attempts, before.attempts);
    assert_eq!(rows.boards, before.boards);
    assert_eq!(rows.registrations, before.registrations);
}

#[test]
fn a_carried_pulse_draws_its_generation_once_and_a_missing_pulse_is_not_due() {
    let mut driver = Driver::new(RiskTierV1::Standard, 40);
    driver.qualify();
    driver.register_initial();
    let slot = driver.state.awaiting_slots()[0];
    let height = slot.max(driver.height + 1);
    let now_ms = driver.now_ms + 1_000;
    let mut rows = Rows::of(&driver, &Vec::new());

    // Admission counts the candidate height's own slot as potential due work.
    {
        let state = rows.state();
        let block = state.block(header(height, now_ms));
        let due = |available: bool| {
            parliament_due(&block.world, height, now_ms, None, |wanted| {
                available && wanted == height
            })
            .expect("a due answer")
        };
        assert_eq!((due(true), due(false)), (slot == height, false));
    }

    // The slot's block without its pulse draws nothing.
    let before = rows.clone();
    let missing = rows.block(height, now_ms);
    assert!(!missing.governance_due_applied());
    assert_eq!(rows.attempts, before.attempts);

    let carried = |wanted: u64| (wanted == slot).then(|| pulse(wanted));
    let drawn = rows
        .run(height, now_ms, carried, true)
        .expect("the pass draws");
    assert_eq!((drawn.processed, drawn.clocks), (1, 0));
    assert!(drawn.outputs().is_empty());
    driver.advance_to_slot();
    assert_eq!((driver.height, driver.now_ms), (height, now_ms));
    assert_eq!(rows.attempt(), &driver.state);

    // The generation consumed its pulse: a later block carrying it draws nothing.
    let again = rows
        .run(height + 1, now_ms + 1, carried, true)
        .expect("the pass");
    assert_eq!(again.processed, 0);
    assert_eq!(rows.attempt(), &driver.state);
}

#[test]
fn a_jury_ballot_runs_from_reflection_to_its_closure_in_world() {
    let (mut rows, mut height) = jury_in_deliberation();
    let id = rows.attempts[0].0;

    // Entering Reflection fixes the opening time on the board before G2c is listed.
    height += 1;
    let reflection_ms = rows.next_due();
    let mut outputs = rows.block(height, reflection_ms).outputs();
    let opening_at = rows
        .attempt()
        .opening_at_ms(JURY)
        .expect("Reflection schedules");
    assert_eq!(rows.boards[0].1.opening_at_ms, Some(opening_at));
    if rows.boards[0].1.opening.is_none() {
        // The notice had not elapsed: the ballot opens at the first pass at its time.
        assert!(outputs.is_empty() && opening_at > reflection_ms);
        height += 1;
        let report = rows.block(height, opening_at);
        assert_eq!(report.processed, 1);
        outputs = report.outputs();
    }
    let s0 = rows.boards[0].1.opening.expect("casting opened");
    let [opened] = outputs.as_slice() else {
        panic!("one opening: {outputs:?}")
    };
    assert_eq!(pass_anchor(opened), s0);
    let GovernanceBlockStartEventV1::CastingOpened(event) = &opened.event else {
        panic!("an opening: {opened:?}")
    };
    assert_eq!(
        (event.scope, event.opening_at_ms),
        (rows.boards[0].0, opening_at)
    );
    assert_eq!(rows.open_windows(), 1);

    // The successor block establishes the clock: unpositioned and outside the budget.
    height = s0.height + 1;
    let clock_ms = s0.timestamp_ms + 1;
    let report = rows.block(height, clock_ms);
    assert_eq!((report.clocks, report.processed), (1, 0));
    assert!(report.governance_due_applied());
    let outputs = report.outputs();
    let [clock] = outputs.as_slice() else {
        panic!("one clock: {outputs:?}")
    };
    assert_eq!(clock.validate(), Ok(()));
    assert_eq!(
        clock.anchor,
        GovernanceOutputAnchorV1::CastingClockPrelude {
            height,
            timestamp_ms: clock_ms
        }
    );
    let GovernanceBlockStartEventV1::CastingClockEstablished(established) = &clock.event else {
        panic!("a clock: {clock:?}")
    };
    assert_eq!(established.scope, rows.boards[0].0);
    let closing_ms = established.closing_ms;
    assert_eq!(closing_ms, clock_ms + established.casting_window_ms);
    assert_eq!(rows.boards[0].1.opening, Some(s0), "s0 is preserved");

    // One millisecond before the closing time nothing closes.
    height += 1;
    let early = rows.block(height, closing_ms - 1);
    assert!(!early.governance_due_applied());

    // The closure decides the attempt in the same item: no ballot was cast.
    height += 1;
    let report = rows.block(height, closing_ms);
    assert_eq!((report.clocks, report.processed), (0, 1));
    let outputs = report.outputs();
    let [closed] = outputs.as_slice() else {
        panic!("one closure: {outputs:?}")
    };
    let GovernanceBlockStartEventV1::BallotClosed(closure) = &closed.event else {
        panic!("a closure: {closed:?}")
    };
    assert_eq!(pass_anchor(closed), closure.closure_anchor);
    assert_eq!((closure.opening_anchor, closure.body_role), (s0, JURY));
    assert_ne!(closure.outcome, BallotOutcomeV1::Approved);
    assert_eq!(rows.boards[0].1.closure.as_ref(), Some(closure));
    assert_eq!(
        rows.attempt().attempt().status,
        GovernanceAttemptStatusV1::Rejected
    );
    assert!(matches!(
        rows.attempt().rejection(),
        Some(AnonymousAttemptRejectionV1::Ballot(recorded)) if **recorded == *closure
    ));
    assert_eq!(rows.attempts[0].0, id);
    assert_eq!(rows.open_windows(), 0);

    // A terminal attempt has no due item.
    let after = rows.block(height + 1, closing_ms + 1);
    assert!(!after.governance_due_applied());
}

#[test]
fn a_full_casting_set_defers_the_opening_without_a_position() {
    let (mut rows, mut height) = jury_in_deliberation();
    let reflection_ms = rows.next_due();
    // One window is already open, and the chain admits one: a row that occupies the slot
    // and never closes here.
    let occupied = BallotScopeV1::new([0xee; 64]);
    let mut occupant = rows.boards[0].1.clone();
    occupant.body_instance_id = BodyInstanceId::new([0xee; 32]);
    occupant.opening_at_ms = Some(reflection_ms - 1);
    occupant.opening = Some(GovernanceAnchorV1 {
        height,
        position: 0,
        timestamp_ms: reflection_ms - 1,
    });
    occupant.casting_clock = Some(
        iroha_data_model::governance::anonymous_ballot::BallotCastingClockV1 {
            height: height + 1,
            timestamp_ms: u64::MAX / 2,
        },
    );
    rows.boards.push((occupied, occupant));
    rows.limit = Some(1);
    let real = |rows: &Rows| {
        rows.boards
            .iter()
            .find(|(scope, _)| *scope != occupied)
            .expect("the jury's board")
            .1
            .clone()
    };

    // Reflection takes position 0 and schedules the board.
    height += 1;
    let reflected = rows
        .run(height, reflection_ms, no_pulse, false)
        .expect("the pass");
    assert_eq!(reflected.processed, 1);
    assert!(reflected.outputs().is_empty());
    let opening_at = real(&rows).opening_at_ms.expect("scheduled at Reflection");
    assert_eq!(real(&rows).opening, None);

    // At its opening time the ballot finds no slot: no position, no charge, no output.
    height += 1;
    let due_ms = opening_at.max(reflection_ms + 1);
    let deferred = rows.run(height, due_ms, no_pulse, false).expect("the pass");
    assert!(!deferred.governance_due_applied());
    assert_eq!(real(&rows).opening, None);

    // A second slot opens the deferred opening at its own pass: `s0` is that anchor.
    rows.limit = Some(2);
    height += 1;
    let opened = rows
        .run(height, due_ms + 1, no_pulse, false)
        .expect("the pass");
    assert_eq!(opened.processed, 1);
    assert_eq!(
        real(&rows).opening,
        Some(GovernanceAnchorV1 {
            height,
            position: 0,
            timestamp_ms: due_ms + 1,
        })
    );
    assert_eq!(real(&rows).opening_at_ms, Some(opening_at));
}

#[test]
fn the_casting_limit_is_the_committed_parameter_or_its_default() {
    let default = usize::from(ParliamentBallotParametersV1::default().max_nonterminal_attempts);
    let limit = |rows: Rows| {
        let state = rows.state();
        let block = state.block(header(1, 1));
        casting_window_limit(&block.world).expect("a valid committed limit")
    };
    assert_eq!(limit(Rows::default()), default);
    assert_eq!(
        limit(Rows {
            limit: Some(1),
            ..Rows::default()
        }),
        1
    );
}

#[test]
fn tables_that_disagree_abandon_the_block_without_a_verdict_of_their_own() {
    let (mut rows, height) = jury_in_deliberation();
    // A scheduled board whose attempt World does not keep.
    let now_ms = rows.next_due();
    rows.boards[0].1.opening_at_ms = Some(now_ms);
    rows.attempts.clear();
    let error = rows
        .run(height + 1, now_ms, no_pulse, false)
        .expect_err("a board without its attempt");
    let GovernancePassErrorV1::Owner(error) = error else {
        panic!("the owner's error is preserved: {error:?}")
    };
    assert!(matches!(error, AnonymousPassWorldErrorV1::World(_)));
    assert!(!error.is_local());
    assert!(!AnonymousPassWorldErrorV1::from(Error::StaleItem).is_local());
}

#[test]
fn eligible_citizens_meet_the_frozen_bond_in_account_order() {
    use iroha_primitives::numeric::Quantity;

    use crate::{
        governance::parliament_anonymous::test_driver::citizens, state::CitizenshipRecord,
    };

    let accounts = citizens(4);
    let world = World::new();
    let mut block = world.block();
    for (bond, account) in [7_u64, 5, 4, 5].into_iter().zip(&accounts) {
        block.citizens.insert(
            account.clone(),
            CitizenshipRecord::new(account.clone(), Quantity::from(bond), 1),
        );
    }
    block.commit();
    let state = State::new_with_chain_and_network_id_for_testing(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        ChainId::from("parliament-anonymous-pass-tests"),
        network(),
    );
    let mut block = state.block(header(1, 1));
    let tx = block.transaction();
    let mut expected = vec![
        accounts[0].clone(),
        accounts[1].clone(),
        accounts[3].clone(),
    ];
    expected.sort_unstable();
    assert_eq!(
        eligible_citizens(&tx.world.citizens, &Quantity::from(5_u64)),
        expected
    );
    assert!(eligible_citizens(&tx.world.citizens, &Quantity::from(8_u64)).is_empty());
}

/// The committed block cadence of the World `rows` are committed in.
fn block_cadence_ms(rows: &Rows) -> u64 {
    let state = rows.state();
    let block = state.block(header(1, 1));
    block.world.parameters().sumeragi().block_cadence_ms.get()
}

#[test]
fn the_height_progress_ticket_is_due_from_the_parent_cadence_and_ends_at_its_slot() {
    let mut driver = Driver::new(RiskTierV1::Standard, 40);
    driver.qualify();
    driver.register_initial();
    let slot = driver.state.awaiting_slots()[0];
    let height = driver.height + 1;
    assert!(slot > height + 1, "the generation awaits a slot ahead");
    let now_ms = driver.now_ms + 1_000;
    let mut rows = Rows::of(&driver, &Vec::new());
    let cadence = block_cadence_ms(&rows);
    let before = rows.clone();

    // One millisecond before the parent's cadence deadline the ticket is not due, and a
    // block without a parent time has none.
    rows.parent_time_ms = Some(now_ms - cadence + 1);
    let early = rows.block(height, now_ms);
    assert!(
        !early.governance_due_applied(),
        "a height-progress ticket is not due before the parent's cadence deadline"
    );
    rows.parent_time_ms = None;
    assert!(!rows.block(height, now_ms).governance_due_applied());

    // At the deadline the pass consumes it as G1's first item. It changes no row.
    rows.parent_time_ms = Some(now_ms - cadence);
    let advanced = rows.block(height, now_ms);
    assert_eq!((advanced.processed, advanced.clocks), (1, 0));
    assert!(advanced.outputs().is_empty());
    assert_eq!(
        advanced.receipts(),
        [GovernanceBlockStartOutputV1 {
            anchor: GovernanceOutputAnchorV1::Pass(GovernanceAnchorV1 {
                height,
                position: 0,
                timestamp_ms: now_ms,
            }),
            event: GovernanceBlockStartEventV1::NativeSlotAdvanced(NativeSlotAdvancedV1 {
                target_height: slot,
                cursor_height: height - 1,
                ticket_height: height,
            }),
        }]
    );
    assert_eq!(rows.attempts, before.attempts);

    // The block of the slot consumes the last ticket, then draws from its carried pulse.
    let carried = |wanted: u64| (wanted == slot).then(|| pulse(wanted));
    let at_slot = rows.run(slot, now_ms, carried, true).expect("the pass");
    assert_eq!(at_slot.processed, 2);
    let receipts = at_slot.receipts();
    let positions: Vec<_> = receipts
        .iter()
        .map(|receipt| pass_anchor(receipt).position)
        .collect();
    assert_eq!(positions, [0, 1]);
    assert_eq!(
        receipts[0].event,
        GovernanceBlockStartEventV1::NativeSlotAdvanced(NativeSlotAdvancedV1 {
            target_height: slot,
            cursor_height: slot - 1,
            ticket_height: slot,
        })
    );
    assert!(matches!(
        receipts[1].event,
        GovernanceBlockStartEventV1::LifecycleApplied(applied)
            if applied.item == GovernanceLifecycleItemV1::DrawGeneration { slot }
    ));

    // A slot that passed without its pulse leaves no ticket: no keepalive loop follows it.
    let mut missed = before;
    missed.parent_time_ms = Some(now_ms - cadence);
    assert!(
        !missed.block(slot + 1, now_ms).governance_due_applied(),
        "a slot that passed without its pulse leaves no height-progress ticket"
    );
}

#[test]
fn an_item_whose_rows_do_not_fit_the_partition_stays_due() {
    let mut driver = Driver::new(RiskTierV1::Standard, 40);
    driver.qualify();
    driver.register_initial();
    let slot = driver.state.awaiting_slots()[0];
    let rows = Rows::of(&driver, &Vec::new());
    let state = rows.state();
    let block = state.block(header(slot, driver.now_ms + 1_000));
    let draw = GovernancePassItemV1::Draw {
        slot,
        attempt: driver.id(),
    };
    let lapsed = GovernancePassItemV1::Draw {
        slot: slot + 1,
        attempt: driver.id(),
    };
    let limit = casting_window_limit(&block.world).expect("limit");

    // A partition that holds every row it may.
    let mut full = GovernanceStartOutputBudgetV1::new();
    full.close_prelude();
    let mut position = 0;
    while full.fits(GovernanceStartItemCeilingV1::Receipt) {
        full.admit_item(
            GovernanceStartItemCeilingV1::Receipt,
            vec![GovernanceBlockStartOutputV1 {
                anchor: GovernanceOutputAnchorV1::Pass(GovernanceAnchorV1 {
                    height: slot,
                    position,
                    timestamp_ms: driver.now_ms + 1_000,
                }),
                event: GovernanceBlockStartEventV1::NativeSlotAdvanced(NativeSlotAdvancedV1 {
                    target_height: slot,
                    cursor_height: slot - 1,
                    ticket_height: slot,
                }),
            }],
        )
        .expect("a row that fits");
        position += 1;
    }

    let empty = GovernanceStartOutputBudgetV1::new();
    assert_eq!(
        inspect(&block, draw, limit, &empty).expect("occurrence"),
        GovernanceOccurrenceV1::Live
    );
    assert_eq!(
        inspect(&block, draw, limit, &full).expect("occurrence"),
        GovernanceOccurrenceV1::NoOutputCapacity,
        "an item whose rows do not fit the governance-start partition stays due"
    );
    // An occurrence that passed lapses whatever the partition holds: it emits nothing.
    for rows in [&empty, &full] {
        assert_eq!(
            inspect(&block, lapsed, limit, rows).expect("occurrence"),
            GovernanceOccurrenceV1::Lapsed
        );
    }
    // The ceilings: a binding seal, a closure and an opening emit one ballot output more.
    let body = JURY;
    let id = driver.id();
    for (item, ceiling) in [
        (draw, GovernanceStartItemCeilingV1::Receipt),
        (
            GovernancePassItemV1::HeightProgress,
            GovernanceStartItemCeilingV1::Receipt,
        ),
        (
            GovernancePassItemV1::Close {
                due_ms: 0,
                attempt: id,
                body,
            },
            GovernanceStartItemCeilingV1::ReceiptAndOutput,
        ),
        (
            GovernancePassItemV1::Open {
                due_ms: 0,
                attempt: id,
                body,
            },
            GovernanceStartItemCeilingV1::ReceiptAndOutput,
        ),
    ] {
        assert_eq!(output_ceiling(item), ceiling);
    }
}

#[test]
fn the_dispatch_runs_parliament_only_on_a_root_that_installs_the_family() {
    use crate::{
        smartcontracts::isi::sccp::parliament_iface::{
            governance_due, governance_due_on_global_root,
        },
        state::governance_start::run_on_global_chain,
    };

    // A new attempt: `StartQualification` is due from the block after its creation.
    let driver = Driver::new(RiskTierV1::Standard, 3);
    let rows = Rows::of(&driver, &Vec::new());
    let (height, now_ms) = (CREATED_HEIGHT + 1, CREATED_MS + 1);
    for installed in [false, true] {
        let state = rows.state_with(installed);
        let mut block = state.block(header(height, now_ms));
        assert_eq!(
            governance_due_on_global_root(&block.world, None, height, now_ms),
            Ok(installed),
            "a root that does not install the anonymous family has no Parliament due work"
        );
        // This World names no committed root scope: it is no global chain, whose pass alone
        // runs Parliament's items, so it has no Parliament due work either way.
        assert_eq!(
            governance_due(&block.world, None, height, now_ms),
            Ok(false)
        );
        let mut calls = 0;
        run_on_global_chain::<()>(&mut block, |_| {
            calls += 1;
            Ok(())
        })
        .expect("the pass");
        assert_eq!(calls, 1, "the step before the enactments runs exactly once");
        let stored = block
            .world
            .parliament_anonymous_attempts
            .get(&driver.id())
            .expect("the seeded attempt");
        if installed {
            assert!(block.governance_due_applied());
            let committed = block.governance_start_rows().expect("a reserved partition");
            assert_eq!(
                validate_governance_start_rows_v1(committed, height),
                Ok(1),
                "the qualification start emits its receipt"
            );
            assert_eq!(
                stored.qualification_due_ms(),
                Some(now_ms + crate::governance::parliament_anonymous::test_driver::WINDOW_MS)
            );
        } else {
            assert!(
                !block.governance_due_applied(),
                "a root whose selection does not install the anonymous family runs no Parliament item"
            );
            assert!(block.governance_start_rows().is_none());
            assert_eq!(stored, &driver.state);
        }
    }
}
