//! Cross-owner ballot transitions and restore controls with a genuinely registered seal.

use super::*;
use crate::governance::parliament_anonymous::{
    AnonymousElectionCredentialsV1,
    credentials_tests::{
        JURY, answer_public_bodies, close_invitations, drawn_small_jury, jury_effect, register,
    },
    test_driver::{Driver, anchor, network, public_bodies},
};
use iroha_data_model::governance::{anonymous_ballot::BallotOutcomeV1, types::GovernanceStageV1};

fn sealed() -> (Driver, AnonymousElectionCredentialsV1, BallotBoardV1) {
    let mut driver = drawn_small_jury();
    let mut records = Vec::new();
    register(&mut driver, &mut records, 0..3);
    answer_public_bodies(&mut driver);
    let outcomes = close_invitations(&mut driver, &records);
    let record = jury_effect(&outcomes)
        .board
        .as_ref()
        .expect("sealed board")
        .board
        .clone();
    (
        driver,
        records,
        BallotBoardV1::restore(record).expect("original board"),
    )
}

fn reflect(driver: &mut Driver) {
    for body in public_bodies() {
        driver.conclude_public(body);
    }
    driver.deliberate(JURY);
}

fn check(
    driver: &Driver,
    records: &AnonymousElectionCredentialsV1,
    board: &BallotBoardV1,
) -> Result<(), Error> {
    driver.state.validate_credentials(
        &network(),
        driver.height,
        |_, accepted| {
            records
                .iter()
                .filter(|(id, _)| accepted.contains(id))
                .copied()
                .collect()
        },
        [(board.scope(), &board.record())],
    )
}

#[test]
fn an_opening_requires_the_reducers_reflection_and_exact_schedule() {
    let (mut driver, records, mut board) = sealed();
    let original = board.record();
    assert!(
        driver
            .state
            .schedule_ballot_board(&mut board, &network())
            .is_err()
    );
    assert_eq!(board.record(), original);
    // A board-valid schedule before the reducer entered Reflection cannot restore.
    board
        .schedule_opening(driver.now_ms)
        .expect("board alone accepts a time");
    assert!(matches!(
        check(&driver, &records, &board),
        Err(Error::Restore {
            field: "board opening schedule"
        })
    ));
    board = BallotBoardV1::restore(original).expect("original");
    reflect(&mut driver);
    let mut other_proposal = board.record();
    other_proposal.proposal_content_id =
        iroha_data_model::governance::types::ProposalContentId::new([0xf1; 32]);
    let mut other_proposal = BallotBoardV1::restore(other_proposal)
        .expect("board alone does not authenticate its proposal");
    let before = other_proposal.record();
    assert!(matches!(
        driver
            .state
            .schedule_ballot_board(&mut other_proposal, &network()),
        Err(Error::Restore {
            field: "board identity"
        })
    ));
    assert_eq!(other_proposal.record(), before);
    let due = driver
        .state
        .schedule_ballot_board(&mut board, &network())
        .expect("Reflection schedules opening");
    assert_eq!(Some(due), driver.state.opening_at_ms(JURY));
    check(&driver, &records, &board).expect("matching scheduled board");

    let reflection = driver
        .state
        .body(JURY)
        .unwrap()
        .current_phase()
        .unwrap()
        .entered_at;
    let before = board.record();
    assert!(
        driver
            .state
            .open_ballot_board(&mut board, &network(), reflection)
            .is_err()
    );
    assert_eq!(board.record(), before);
    driver.height += 1;
    driver.now_ms = due;
    let at = driver.at(0);
    driver
        .state
        .open_ballot_board(&mut board, &network(), at)
        .expect("later positioned opening");
    check(&driver, &records, &board).expect("matching opening");
    assert!(
        driver
            .state
            .open_ballot_board(&mut board, &network(), at)
            .is_err()
    );
}

#[test]
fn the_clock_and_actual_corpus_closure_keep_the_board_and_reducer_together() {
    let (mut driver, records, mut board) = sealed();
    reflect(&mut driver);
    let due = driver
        .state
        .schedule_ballot_board(&mut board, &network())
        .unwrap();
    driver.height += 1;
    driver.now_ms = due;
    driver
        .state
        .open_ballot_board(&mut board, &network(), driver.at(0))
        .unwrap();
    let original = board.record();
    assert!(
        driver
            .state
            .establish_ballot_board_clock(&mut board, &network(), driver.height + 2, due + 2)
            .is_err()
    );
    assert_eq!(board.record(), original);
    driver.height += 1;
    driver.now_ms += 1;
    driver
        .state
        .establish_ballot_board_clock(&mut board, &network(), driver.height, driver.now_ms)
        .unwrap();
    let before_attempt = driver.state.clone();
    let before_board = board.record();
    let premature = driver.at(0);
    assert!(
        driver
            .state
            .close_ballot_board(&mut board, &network(), premature, || panic!(
                "no confirmation on a refusal"
            ))
            .is_err()
    );
    assert_eq!(driver.state, before_attempt);
    assert_eq!(board.record(), before_board);

    driver.height += 1;
    driver.now_ms = board.closing_ms().unwrap();
    let at = driver.at(0);
    // The board itself can close, but a reducer-stage refusal must roll it back too.
    driver.state.attempt.stage = GovernanceStageV1::Qualification;
    let wrong_stage = driver.state.clone();
    assert!(
        driver
            .state
            .close_ballot_board(&mut board, &network(), at, || panic!("not requested"))
            .is_err()
    );
    assert_eq!(driver.state, wrong_stage);
    assert_eq!(board.record(), before_board);
    driver.state = before_attempt;
    let (decision, closure) = driver
        .state
        .close_ballot_board(&mut board, &network(), at, || {
            panic!("no confirmation for no quorum")
        })
        .unwrap();
    assert_eq!(
        decision,
        AnonymousBallotDecisionV1::Rejected(BallotOutcomeV1::NoQuorum)
    );
    assert_eq!(board.closure(), Some(&closure));
    check(&driver, &records, &board).expect("the closed board and rejection agree");
    driver.assert_restores();
    assert!(driver.state.enactment_due_ms().is_none());

    let mut missing = board.record();
    missing.closure = None;
    assert!(matches!(
        driver.state.validate_board_lifecycle(&missing),
        Err(Error::Restore {
            field: "board closure outcome"
        })
    ));
}

#[test]
fn restoring_a_board_cannot_invent_a_closure_or_move_its_reflection_schedule() {
    let (mut driver, records, mut board) = sealed();
    reflect(&mut driver);
    let due = driver
        .state
        .schedule_ballot_board(&mut board, &network())
        .unwrap();
    let mut moved = board.record();
    moved.opening_at_ms = Some(due + 1);
    let moved = BallotBoardV1::restore(moved).expect("the board alone cannot know Reflection");
    assert!(check(&driver, &records, &moved).is_err());
    let mut overflow = board.record();
    overflow.opening = Some(anchor(driver.height + 1, 0, u64::MAX));
    let overflow = BallotBoardV1::restore(overflow).expect("board has no reducer deadlines");
    assert!(matches!(
        check(&driver, &records, &overflow),
        Err(Error::Schedule)
    ));
    let height = driver.height + 1;
    driver
        .state
        .open_ballot_board(&mut board, &network(), anchor(height, 0, due))
        .unwrap();
    driver
        .state
        .establish_ballot_board_clock(&mut board, &network(), height + 1, due + 1)
        .unwrap();
    board
        .close(anchor(height + 2, 0, board.closing_ms().unwrap()))
        .unwrap();
    driver.height = height + 2;
    assert!(matches!(
        check(&driver, &records, &board),
        Err(Error::Restore {
            field: "board closure outcome"
        })
    ));
}
