//! The board against the crate's genuine fixtures: real registration proofs, the pinned
//! fixture ballots with their real vote proofs and stamps, and the fixture scope.

use std::sync::OnceLock;

use iroha_data_model::governance::anonymous_ballot::{
    AnonymousBallotRejectionV1, BallotOutcomeV1, CredentialRegistrationV1,
    ParliamentCandidateTestingPolicyV1,
};

use super::*;
use crate::{
    ballot::BlockInvalidReasonV1,
    inputs::registration_mapping_v1,
    test_support::{
        ABSTAIN_3, AYE_0, AYE_2, NAY_0, NAY_1, SEATS, account, body_context, body_instance_id,
        canonical_bytes, fixture_bytes, forged_envelope, network_id, policy, proposal_content_id,
        proposal_digest, scoped_context, sealed_list, secret_bytes,
    },
    wallet::{CredentialHandleV1, prepare_registration_v1},
};

/// One fixture member beyond the seated roster: an accepted alternate.
const ALTERNATE: u8 = SEATS;

fn assignment(member: u8) -> AssignmentId {
    AssignmentId::new([0x70 + member; 32])
}

fn seated() -> Vec<AssignmentId> {
    (0..SEATS).map(assignment).collect()
}

/// The genuine registration of fixture `member` for its own seat, proved once.
fn registration(member: u8) -> CredentialRegistrationV1 {
    static REGISTRATIONS: OnceLock<Vec<CredentialRegistrationV1>> = OnceLock::new();
    REGISTRATIONS.get_or_init(|| {
        (0..=ALTERNATE)
            .map(|member| {
                let handle =
                    CredentialHandleV1::import_v1(body_context(), &secret_bytes(member), None)
                        .expect("fixture credential");
                prepare_registration_v1(&handle, &account(), &assignment(member))
                    .expect("real registration relation")
            })
            .collect()
    })[usize::from(member)]
    .clone()
}

/// The ledger with every seated member and the alternate registered.
fn ledger() -> RegistrationLedgerV1 {
    static LEDGER: OnceLock<RegistrationLedgerV1> = OnceLock::new();
    LEDGER
        .get_or_init(|| {
            let mut ledger = RegistrationLedgerV1::new(body_context()).expect("binding body");
            for member in 0..=ALTERNATE {
                let record = ledger
                    .register(&account(), assignment(member), &registration(member))
                    .expect("genuine registration");
                assert_eq!(record.commitment, registration(member).commitment);
            }
            ledger
        })
        .clone()
}

fn candidate() -> ParliamentBallotPurposeV1 {
    ParliamentBallotPurposeV1::CandidateTesting(ParliamentCandidateTestingPolicyV1::CANDIDATE_V1)
}

/// The facts a node derives for the fixture body's seal.
fn seal_facts(seal: &SealedRosterV1) -> BoardSealFactsV1 {
    let assignments = seated()
        .into_iter()
        .map(|assignment_id| ParliamentSeatAssignmentV1 {
            assignment_id,
            member: account(),
        })
        .collect::<Vec<_>>();
    BoardSealFactsV1 {
        purpose: candidate(),
        proposal_digest: proposal_digest(),
        proposal_content_id: proposal_content_id(),
        body_instance_id: body_instance_id(),
        roster_root: seal.roster_root(&assignments).expect("roster root"),
        sealed_timestamp_ms: SEALED_MS,
        policy: policy(),
    }
}

/// The fixture board: sealed from the ledger, so its scope is the fixture scope.
fn board() -> BallotBoardV1 {
    let seal = ledger().seal(&seated()).expect("seal the seated roster");
    BallotBoardV1::sealed(&seal, seal_facts(&seal)).expect("fixture board")
}

fn boards(board: BallotBoardV1) -> BTreeMap<BallotScopeV1, BallotBoardV1> {
    BTreeMap::from([(*board.scope(), board)])
}

const SEALED_MS: u64 = 400_000;
const REFLECTION_MS: u64 = 500_000;
const OPENING_HEIGHT: u64 = 100;
const OPENING: GovernanceAnchorV1 = GovernanceAnchorV1 {
    height: OPENING_HEIGHT,
    position: 2,
    timestamp_ms: 87_000_000,
};
const CLOCK_MS: u64 = 87_001_000;

fn height(height: u64) -> NonZeroU64 {
    NonZeroU64::new(height).expect("nonzero height")
}

fn execute(
    boards: &mut BTreeMap<BallotScopeV1, BallotBoardV1>,
    fixture: usize,
    block: u64,
    timestamp_ms: u64,
    input_index: u32,
) -> BlockBallotDecisionV1 {
    execute_block_ballot_v1(
        network_id(),
        boards,
        &fixture_bytes(fixture),
        height(block),
        timestamp_ms,
        input_index,
    )
    .expect("the node can decide a fixture ballot")
}

fn rejected(decision: &BlockBallotDecisionV1) -> Option<AnonymousBallotRejectionV1> {
    match decision {
        BlockBallotDecisionV1::Rejected(reason) => Some(*reason),
        _ => None,
    }
}

/// An opened board with its casting clock established.
fn open_board() -> BallotBoardV1 {
    let mut board = board();
    board
        .schedule_opening(REFLECTION_MS)
        .expect("fix the opening time");
    board.open(OPENING).expect("open casting");
    board
        .establish_casting_clock(OPENING_HEIGHT + 1, CLOCK_MS)
        .expect("establish the casting clock");
    board
}

#[test]
fn the_ledger_verifies_registrations_and_keeps_seats_and_commitments_unique() {
    let mut ledger = RegistrationLedgerV1::new(body_context()).expect("binding body");
    assert!(ledger.is_empty());
    let member_0 = registration(0);
    ledger
        .register(&account(), assignment(0), &member_0)
        .expect("genuine registration");
    assert_eq!(ledger.len(), 1);
    assert_eq!(
        ledger
            .registered(&assignment(0))
            .map(|record| record.commitment),
        Some(member_0.commitment)
    );

    // A seat registers once, and a copied commitment never registers for another seat:
    // both are refused before any proof is verified.
    assert_eq!(
        ledger.register(&account(), assignment(0), &registration(1)),
        Err(RegistrationLedgerErrorV1::SeatRegistered)
    );
    assert_eq!(
        ledger.register(&account(), assignment(1), &member_0),
        Err(RegistrationLedgerErrorV1::DuplicateCommitment)
    );

    // A genuine registration replayed for another seat is another statement.
    assert_eq!(
        ledger.register(&account(), assignment(2), &registration(1)),
        Err(RegistrationLedgerErrorV1::Registration(
            RegistrationErrorV1::Rejected
        ))
    );
    // An authorization value that the proof does not prove is rejected.
    let mut wrong_authorization = registration(1);
    wrong_authorization.authorization = member_0.authorization;
    let refused = ledger
        .register(&account(), assignment(1), &wrong_authorization)
        .expect_err("auth is bound into the proof");
    assert_eq!(
        refused,
        RegistrationLedgerErrorV1::Registration(RegistrationErrorV1::Rejected)
    );
    assert!(!refused.is_local());

    // Every refusal left the ledger as it was.
    assert_eq!(ledger.len(), 1);
    assert!(ledger.registered(&assignment(1)).is_none());
    assert!(ledger.registered(&assignment(2)).is_none());
}

#[test]
fn a_restored_ledger_rechecks_uniqueness() {
    let ledger = ledger();
    let records = ledger
        .records()
        .map(|(assignment_id, record)| (*assignment_id, *record))
        .collect::<Vec<_>>();
    assert_eq!(
        RegistrationLedgerV1::restore(body_context(), records.clone()),
        Ok(ledger)
    );
    let mut duplicated = records.clone();
    duplicated[1].1 = duplicated[0].1;
    assert_eq!(
        RegistrationLedgerV1::restore(body_context(), duplicated),
        Err(RegistrationLedgerErrorV1::DuplicateCommitment)
    );
    let mut repeated = records;
    repeated.push(repeated[0]);
    assert_eq!(
        RegistrationLedgerV1::restore(body_context(), repeated),
        Err(RegistrationLedgerErrorV1::SeatRegistered)
    );
}

#[test]
fn the_seal_lists_exactly_the_seated_credentials() {
    let ledger = ledger();
    assert_eq!(ledger.len(), usize::from(SEATS) + 1);
    let seal = ledger.seal(&seated()).expect("seal the seated roster");

    // The accepted alternate is registered and absent from `L` and `R`.
    assert_eq!(seal.list(), &sealed_list());
    assert!(
        !seal
            .list()
            .commitments
            .contains(&registration(ALTERNATE).commitment)
    );
    assert_eq!(seal.list_digest(), scoped_context().list_digest());
    assert_eq!(
        seal.binding(),
        AnonymousRosterBindingV1 {
            list_digest: *scoped_context().list_digest(),
            seats: u16::from(SEATS),
            profile_id: ProofProfileIdV1::V1,
        }
    );

    // The mapping authenticates which seat registered which commitment.
    assert_eq!(seal.pairs().len(), usize::from(SEATS));
    assert!(
        seal.pairs()
            .windows(2)
            .all(|pair| pair[0].assignment_id.as_bytes() < pair[1].assignment_id.as_bytes())
    );
    assert_eq!(
        seal.registration_mapping_digest(),
        &registration_mapping_v1(seal.list(), seal.pairs()).expect("mapping digest")
    );

    // The seat order offered does not change the seal.
    let mut reversed = seated();
    reversed.reverse();
    assert_eq!(ledger.seal(&reversed), Ok(seal.clone()));

    // The roster root binds `(R, S, profile_id)` and the published output carries the
    // exact list bytes.
    let assignments = seated()
        .into_iter()
        .map(|assignment_id| ParliamentSeatAssignmentV1 {
            assignment_id,
            member: account(),
        })
        .collect::<Vec<_>>();
    let roster_root = seal.roster_root(&assignments).expect("roster root");
    let with_alternate = ledger
        .seal(&[
            assignment(0),
            assignment(1),
            assignment(2),
            assignment(ALTERNATE),
        ])
        .expect("another seated roster");
    assert_ne!(seal.list_digest(), with_alternate.list_digest());
    assert_eq!(
        seal.roster_root(&assignments[..3]),
        Err(SealErrorV1::MissingCredential)
    );
    let published = seal
        .published(body_instance_id(), roster_root)
        .expect("seal output");
    assert_eq!(published.roster_root, roster_root);
    assert_eq!(published.list_digest, *seal.list_digest());
    assert_eq!(
        published.sealed_list_bytes.decode_list().as_ref(),
        Ok(seal.list())
    );
}

#[test]
fn a_roster_below_three_seats_is_no_roster_and_never_a_seal() {
    let ledger = ledger();
    for seats in 0..3 {
        assert_eq!(ledger.seal(&seated()[..seats]), Err(SealErrorV1::NoRoster));
    }
    assert_eq!(
        ledger.seal(&[assignment(0), assignment(1), assignment(0)]),
        Err(SealErrorV1::DuplicateAssignment)
    );
    assert_eq!(
        ledger.seal(&[assignment(0), assignment(1), assignment(0x20)]),
        Err(SealErrorV1::MissingCredential)
    );
}

#[test]
fn the_sealed_board_has_the_fixture_scope_and_an_empty_corpus() {
    let board = board();
    assert_eq!(board.scope(), scoped_context().scope());
    assert_eq!(**board.context(), *scoped_context());
    assert_eq!(board.corpus().accepted(), 0);
    assert!(board.window().is_none());
    assert_eq!(board.status_at(u64::MAX), CastingWindowStatusV1::NotYetDue);
}

#[test]
fn a_board_records_its_purpose_and_refuses_an_unqualified_one() {
    let seal = ledger().seal(&seated()).expect("seal the seated roster");
    let board = board();
    assert_eq!(board.purpose(), &candidate());
    assert_eq!(board.proposal_content_id(), proposal_content_id());
    assert_eq!(
        BallotBoardV1::restore(board.record())
            .expect("the original board restores")
            .proposal_content_id(),
        proposal_content_id()
    );
    assert!(!board.purpose().is_production());
    assert_eq!(board.sealed_timestamp_ms(), SEALED_MS);
    assert_eq!(board.roster_root(), &seal_facts(&seal).roster_root);
    assert_eq!(board.record().purpose, candidate());

    // This release names no qualification record, so no production board can be sealed
    // or restored.
    let qualified = ParliamentBallotPurposeV1::Qualified(Digest512::new([0x71; 64]));
    assert_eq!(
        BallotBoardV1::sealed(
            &seal,
            BoardSealFactsV1 {
                purpose: qualified,
                ..seal_facts(&seal)
            }
        ),
        Err(BoardErrorV1::Purpose(
            ParliamentBallotPurposeErrorV1::Unqualified
        ))
    );
    let mut record = board.record();
    record.purpose = qualified;
    assert!(matches!(
        BallotBoardV1::restore(record),
        Err(BoardErrorV1::Record(_))
    ));
}

#[test]
fn transitions_out_of_order_are_refused_and_change_nothing() {
    let mut board = board();
    let sealed = board.clone();
    assert_eq!(board.open(OPENING), Err(BoardErrorV1::OpeningNotScheduled));
    assert_eq!(
        board.establish_casting_clock(OPENING_HEIGHT + 1, CLOCK_MS),
        Err(BoardErrorV1::NotOpen)
    );
    assert_eq!(board.close(OPENING).map(|_| ()), Err(BoardErrorV1::NotOpen));
    assert_eq!(board, sealed);

    let notice = board.context().policy().binding.vote_notice_ms;
    let opening_at_ms = board
        .schedule_opening(REFLECTION_MS)
        .expect("fix the opening time");
    assert_eq!(opening_at_ms, REFLECTION_MS.max(SEALED_MS + notice));
    assert_eq!(
        board.schedule_opening(REFLECTION_MS + 1),
        Err(BoardErrorV1::OpeningScheduled)
    );
    assert!(!board.opening_due(opening_at_ms - 1));
    assert!(board.opening_due(opening_at_ms));
    assert_eq!(
        board.open(GovernanceAnchorV1 {
            timestamp_ms: opening_at_ms - 1,
            ..OPENING
        }),
        Err(BoardErrorV1::OpeningNotDue { opening_at_ms })
    );
    assert_eq!(
        board.status_at(opening_at_ms - 1),
        CastingWindowStatusV1::NotYetDue
    );
    assert_eq!(
        board.status_at(opening_at_ms),
        CastingWindowStatusV1::LocallyDue
    );

    let opened = board.open(OPENING).expect("open casting");
    assert_eq!(opened.scope, *board.scope());
    assert_eq!(opened.opening_at_ms, opening_at_ms);
    assert_eq!(board.open(OPENING), Err(BoardErrorV1::AlreadyOpen));
    assert_eq!(board.status_at(0), CastingWindowStatusV1::Open);
    assert!(!board.opening_due(u64::MAX));

    // The clock belongs to exactly the first block after the opening block.
    assert!(!board.casting_clock_due(OPENING_HEIGHT));
    assert!(board.casting_clock_due(OPENING_HEIGHT + 1));
    assert_eq!(
        board.establish_casting_clock(OPENING_HEIGHT + 2, CLOCK_MS),
        Err(BoardErrorV1::ClockHeight {
            expected: OPENING_HEIGHT + 1,
            actual: OPENING_HEIGHT + 2,
        })
    );
    assert_eq!(
        board.establish_casting_clock(OPENING_HEIGHT + 1, OPENING.timestamp_ms - 1),
        Err(BoardErrorV1::Anchor {
            transition: "casting clock"
        })
    );
    assert_eq!(
        board
            .close(GovernanceAnchorV1 {
                height: OPENING_HEIGHT + 1,
                position: 0,
                timestamp_ms: u64::MAX,
            })
            .map(|_| ()),
        Err(BoardErrorV1::ClockNotEstablished)
    );
    assert_eq!(board.closing_ms(), None);
}

#[test]
fn a_stale_opening_never_shortens_voting() {
    let mut board = board();
    board
        .schedule_opening(REFLECTION_MS)
        .expect("fix the opening time");
    board.open(OPENING).expect("open casting");
    // The opening block committed long after its own timestamp: the successor block's
    // time, not `s0`, starts the casting window.
    let late_clock_ms = OPENING.timestamp_ms + 3_600_000;
    let window = board.context().policy().binding.casting_window_ms;
    let established = board
        .establish_casting_clock(OPENING_HEIGHT + 1, late_clock_ms)
        .expect("establish the casting clock");
    assert_eq!(established.casting_window_ms, window);
    assert_eq!(established.closing_ms, late_clock_ms + window);
    assert_eq!(board.closing_ms(), Some(late_clock_ms + window));
    assert_eq!(board.opening(), Some(&OPENING));
    assert_eq!(
        board.establish_casting_clock(OPENING_HEIGHT + 1, late_clock_ms),
        Err(BoardErrorV1::ClockEstablished)
    );
    assert!(!board.closure_due(late_clock_ms + window - 1));
    assert!(board.closure_due(late_clock_ms + window));
    assert_eq!(
        board
            .close(GovernanceAnchorV1 {
                height: OPENING_HEIGHT + 2,
                position: 0,
                timestamp_ms: late_clock_ms + window - 1,
            })
            .map(|_| ()),
        Err(BoardErrorV1::ClosureNotDue {
            closing_ms: late_clock_ms + window
        })
    );
}

#[test]
fn ballots_execute_against_the_board_from_opening_to_closure() {
    let mut sealed = board();
    sealed
        .schedule_opening(REFLECTION_MS)
        .expect("fix the opening time");
    let mut boards = boards(sealed);
    let scope = *scoped_context().scope();
    let window = scoped_context().policy().binding.casting_window_ms;
    let closing_ms = CLOCK_MS + window;

    // A genuine ballot before the opening pass is valid and outside the window.
    let early = execute(
        &mut boards,
        AYE_0,
        OPENING_HEIGHT - 1,
        OPENING.timestamp_ms - 1_000,
        0,
    );
    assert_eq!(
        rejected(&early),
        Some(AnonymousBallotRejectionV1::OutsideCastingWindow)
    );

    // Casting opens in the opening block's start pass: ballots of that block count.
    boards
        .get_mut(&scope)
        .expect("fixture board")
        .open(OPENING)
        .expect("open casting");
    let BlockBallotDecisionV1::Accepted(first) =
        execute(&mut boards, AYE_0, OPENING_HEIGHT, OPENING.timestamp_ms, 3)
    else {
        panic!("a ballot in the opening block is inside the window");
    };
    assert_eq!((first.height, first.input_index), (OPENING_HEIGHT, 3));

    // The first otherwise-valid ballot wins its nullifier, in the same block too.
    let conflicting = execute(&mut boards, NAY_0, OPENING_HEIGHT, OPENING.timestamp_ms, 4);
    assert_eq!(
        rejected(&conflicting),
        Some(AnonymousBallotRejectionV1::DuplicateNullifier)
    );

    // After the opening block the casting clock is mandatory.
    let unclocked = execute(&mut boards, NAY_1, OPENING_HEIGHT + 1, CLOCK_MS, 0);
    assert_eq!(
        rejected(&unclocked),
        Some(AnonymousBallotRejectionV1::OutsideCastingWindow)
    );
    boards
        .get_mut(&scope)
        .expect("fixture board")
        .establish_casting_clock(OPENING_HEIGHT + 1, CLOCK_MS)
        .expect("establish the casting clock");
    assert!(matches!(
        execute(&mut boards, NAY_1, OPENING_HEIGHT + 1, CLOCK_MS, 0),
        BlockBallotDecisionV1::Accepted(_)
    ));
    assert!(matches!(
        execute(&mut boards, AYE_2, OPENING_HEIGHT + 2, closing_ms - 1, 7),
        BlockBallotDecisionV1::Accepted(_)
    ));

    // An invalid proof makes its block invalid and reserves nothing.
    let before = boards.clone();
    let forged = execute_block_ballot_v1(
        network_id(),
        &mut boards,
        &canonical_bytes(&forged_envelope()),
        height(OPENING_HEIGHT + 2),
        closing_ms - 1,
        8,
    )
    .expect("the node can decide a forged ballot");
    assert_eq!(
        forged,
        BlockBallotDecisionV1::BlockInvalid(BlockInvalidReasonV1::ProofRejected)
    );
    assert_eq!(boards, before);

    // At or beyond `closing_ms` a ballot is late even while the closure is deferred.
    let late = execute(&mut boards, ABSTAIN_3, OPENING_HEIGHT + 3, closing_ms, 0);
    assert_eq!(
        rejected(&late),
        Some(AnonymousBallotRejectionV1::OutsideCastingWindow)
    );

    let board = boards.get_mut(&scope).expect("fixture board");
    assert_eq!(board.status_at(closing_ms), CastingWindowStatusV1::Closed);
    let closure_anchor = GovernanceAnchorV1 {
        height: OPENING_HEIGHT + 4,
        position: 0,
        timestamp_ms: closing_ms + 5_000,
    };
    let record = board
        .close(closure_anchor)
        .expect("close the ballot")
        .clone();
    assert_eq!(record.scope, scope);
    assert_eq!(record.seats, u16::from(SEATS));
    assert_eq!(record.opening_anchor, OPENING);
    assert_eq!(record.casting_clock_height, OPENING_HEIGHT + 1);
    assert_eq!(record.closing_ms, closing_ms);
    assert_eq!(record.closure_anchor, closure_anchor);
    assert_eq!(
        (
            record.accepted_ballots,
            record.aye,
            record.nay,
            record.abstain
        ),
        (3, 2, 1, 0)
    );
    assert_eq!(record.outcome, BallotOutcomeV1::Approved);
    assert!(!record.narrow);
    assert_eq!(record.corpus_root, *board.corpus().root());
    assert_eq!(
        record.registration_mapping_digest,
        *ledger()
            .seal(&seated())
            .expect("seal")
            .registration_mapping_digest()
    );

    // There is no reopening, no second closure and no ballot after the closing pass.
    assert_eq!(
        board.close(closure_anchor).map(|_| ()),
        Err(BoardErrorV1::Closed)
    );
    assert_eq!(board.status_at(0), CastingWindowStatusV1::Closed);
    let after = execute(
        &mut boards,
        ABSTAIN_3,
        OPENING_HEIGHT + 4,
        closing_ms + 5_000,
        0,
    );
    assert_eq!(
        rejected(&after),
        Some(AnonymousBallotRejectionV1::OutsideCastingWindow)
    );
    assert_eq!(boards[&scope].entries().len(), 3);
}

#[test]
fn queue_admission_reads_the_board_at_the_local_clock() {
    use crate::ballot::{AdmissionErrorV1, AdmissionRejectionV1, precheck_admission_v1};

    let nobody_pending = |_: &BallotScopeV1, _: &NullifierV1| false;
    let admit = |boards: &BTreeMap<BallotScopeV1, BallotBoardV1>, now_ms: u64, fixture: usize| {
        precheck_admission_v1(
            &fixture_bytes(fixture),
            &BoardsAtAdmissionV1::new(network_id(), now_ms, boards, nobody_pending),
        )
        .map(|admitted| admitted.identity())
    };
    let rejection = |error: AdmissionErrorV1| match error {
        AdmissionErrorV1::Rejected(rejection) => rejection,
        AdmissionErrorV1::Unavailable(error) => panic!("the node can check a fixture: {error}"),
    };

    // No sealed body has the scope.
    assert_eq!(
        admit(&BTreeMap::new(), 0, AYE_0).map_err(rejection),
        Err(AdmissionRejectionV1::UnknownScope)
    );

    // Sealed and scheduled: not due, then due by the local clock before any opening.
    let mut sealed = board();
    let opening_at_ms = sealed
        .schedule_opening(REFLECTION_MS)
        .expect("fix the opening time");
    let mut boards = boards(sealed);
    let scope = *scoped_context().scope();
    assert_eq!(
        admit(&boards, opening_at_ms - 1, AYE_0).map_err(rejection),
        Err(AdmissionRejectionV1::CastingNotOpen)
    );
    let identity = admit(&boards, opening_at_ms, AYE_0).expect("locally due window admits");
    assert_eq!(identity.0, scope);

    // Open in committed state; a committed nullifier is refused, another seat admitted.
    let board = boards.get_mut(&scope).expect("fixture board");
    board.open(OPENING).expect("open casting");
    board
        .establish_casting_clock(OPENING_HEIGHT + 1, CLOCK_MS)
        .expect("establish the casting clock");
    assert!(matches!(
        execute(&mut boards, AYE_0, OPENING_HEIGHT + 1, CLOCK_MS, 0),
        BlockBallotDecisionV1::Accepted(_)
    ));
    assert_eq!(
        admit(&boards, CLOCK_MS, NAY_0).map_err(rejection),
        Err(AdmissionRejectionV1::NullifierCommitted)
    );
    assert!(admit(&boards, CLOCK_MS, NAY_1).is_ok());

    // The queue's verified pending holder is the queue's fact, not the board's.
    let held = fixture_bytes(NAY_1);
    let holder = precheck_admission_v1(
        &held,
        &BoardsAtAdmissionV1::new(network_id(), CLOCK_MS, &boards, |_, _| true),
    );
    assert_eq!(
        holder
            .map(|admitted| admitted.identity())
            .map_err(rejection),
        Err(AdmissionRejectionV1::NullifierPending)
    );

    // At the closing time the window is closed for admission before the closure runs.
    let closing_ms = boards[&scope].closing_ms().expect("clocked board");
    assert_eq!(
        admit(&boards, closing_ms, NAY_1).map_err(rejection),
        Err(AdmissionRejectionV1::CastingClosed)
    );
}

#[test]
fn a_ballot_of_an_unknown_scope_makes_its_block_invalid() {
    let mut empty = BTreeMap::new();
    let decision = execute_block_ballot_v1(
        network_id(),
        &mut empty,
        &fixture_bytes(AYE_0),
        height(OPENING_HEIGHT),
        OPENING.timestamp_ms,
        0,
    )
    .expect("the node can decide");
    assert_eq!(
        decision,
        BlockBallotDecisionV1::BlockInvalid(BlockInvalidReasonV1::UnknownScope)
    );
}

#[test]
fn a_ballot_below_turnout_closes_no_quorum() {
    let mut boards = boards(open_board());
    let scope = *scoped_context().scope();
    assert!(matches!(
        execute(&mut boards, AYE_0, OPENING_HEIGHT + 1, CLOCK_MS, 0),
        BlockBallotDecisionV1::Accepted(_)
    ));
    assert!(matches!(
        execute(&mut boards, AYE_2, OPENING_HEIGHT + 1, CLOCK_MS, 1),
        BlockBallotDecisionV1::Accepted(_)
    ));
    let board = boards.get_mut(&scope).expect("fixture board");
    let closing_ms = board.closing_ms().expect("clocked board");
    let record = board
        .close(GovernanceAnchorV1 {
            height: OPENING_HEIGHT + 2,
            position: 0,
            timestamp_ms: closing_ms,
        })
        .expect("close the ballot");
    // Two unanimous ballots of four seats: below `max(3, ceil(2*4/3))`.
    assert_eq!((record.accepted_ballots, record.aye), (2, 2));
    assert_eq!(record.outcome, BallotOutcomeV1::NoQuorum);
    assert!(!record.narrow);
}

#[test]
fn a_restored_board_is_recomputed_at_every_phase() {
    let mut sealed = board();
    assert_eq!(BallotBoardV1::restore(sealed.record()), Ok(sealed.clone()));
    sealed
        .schedule_opening(REFLECTION_MS)
        .expect("fix the opening time");
    assert_eq!(BallotBoardV1::restore(sealed.record()), Ok(sealed));

    let mut boards = boards(open_board());
    let scope = *scoped_context().scope();
    for (input_index, fixture) in [AYE_0, NAY_1, AYE_2].into_iter().enumerate() {
        assert!(matches!(
            execute(
                &mut boards,
                fixture,
                OPENING_HEIGHT + 1,
                CLOCK_MS,
                u32::try_from(input_index).expect("small index")
            ),
            BlockBallotDecisionV1::Accepted(_)
        ));
    }
    let mut board = boards.remove(&scope).expect("fixture board");
    assert_eq!(BallotBoardV1::restore(board.record()), Ok(board.clone()));

    let closing_ms = board.closing_ms().expect("clocked board");
    board
        .close(GovernanceAnchorV1 {
            height: OPENING_HEIGHT + 2,
            position: 1,
            timestamp_ms: closing_ms,
        })
        .expect("close the ballot");
    let parts = board.record();
    assert_eq!(BallotBoardV1::restore(parts.clone()), Ok(board));

    // Stored values that no sequence of transitions produces are refused.
    let mut altered_counts = parts.clone();
    altered_counts.closure.as_mut().expect("closed").aye += 1;
    assert!(BallotBoardV1::restore(altered_counts).is_err());

    let mut reordered = parts.clone();
    reordered.entries.swap(0, 1);
    assert_eq!(
        BallotBoardV1::restore(reordered),
        Err(BoardErrorV1::Corpus(CorpusErrorV1::Order))
    );

    let mut dropped_entry = parts.clone();
    dropped_entry.entries.pop();
    assert_eq!(
        BallotBoardV1::restore(dropped_entry),
        Err(BoardErrorV1::Restore { field: "closure" })
    );

    let mut unscheduled = parts.clone();
    unscheduled.opening_at_ms = None;
    assert_eq!(
        BallotBoardV1::restore(unscheduled),
        Err(BoardErrorV1::Restore {
            field: "opening_at_ms"
        })
    );

    // An opening time inside the vote notice was never fixed by Reflection.
    let mut inside_notice = parts.clone();
    inside_notice.sealed_timestamp_ms += 1_000_000;
    assert_eq!(
        BallotBoardV1::restore(inside_notice),
        Err(BoardErrorV1::Restore {
            field: "opening_at_ms"
        })
    );

    let mut misplaced_clock = parts.clone();
    misplaced_clock
        .casting_clock
        .as_mut()
        .expect("clocked")
        .height += 1;
    assert_eq!(
        BallotBoardV1::restore(misplaced_clock),
        Err(BoardErrorV1::Restore {
            field: "casting_clock"
        })
    );

    let mut unopened_with_entries = parts.clone();
    unopened_with_entries.closure = None;
    unopened_with_entries.casting_clock = None;
    unopened_with_entries.opening = None;
    assert_eq!(
        BallotBoardV1::restore(unopened_with_entries),
        Err(BoardErrorV1::Restore { field: "entries" })
    );

    let mut foreign_mapping = parts;
    foreign_mapping.registration_mapping_digest = Digest512::new([0x99; 64]);
    assert_eq!(
        BallotBoardV1::restore(foreign_mapping),
        Err(BoardErrorV1::Restore { field: "closure" })
    );
}

#[test]
fn only_a_node_local_proof_input_failure_is_local() {
    use crate::inputs::ProofInputErrorV1;

    let local = ContextErrorV1::Inputs(ProofInputErrorV1::ProfileIdentity);
    let invalid = ContextErrorV1::Mismatch { field: "list" };
    assert!(local.is_local());
    assert!(!invalid.is_local());
    assert!(SealErrorV1::Context(local.clone()).is_local());
    assert!(!SealErrorV1::Context(invalid.clone()).is_local());
    assert!(!SealErrorV1::MissingCredential.is_local());
    assert!(BoardErrorV1::Context(local.clone()).is_local());
    assert!(!BoardErrorV1::Context(invalid).is_local());
    assert!(!BoardErrorV1::Closed.is_local());
    assert!(RegistrationLedgerErrorV1::Context(local).is_local());
    assert!(!RegistrationLedgerErrorV1::DuplicateCommitment.is_local());
}
