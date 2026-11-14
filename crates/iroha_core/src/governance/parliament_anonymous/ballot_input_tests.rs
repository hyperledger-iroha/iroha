//! Signerless ballot inputs against World rows, with genuine fixed-tape ballots and pinned
//! stamps (design §3.6, §3.7 and §13 "Ballot validity and admission").
//!
//! The jury is the pass tests' small one: three seated credentials whose secrets are the
//! credential tests' fixed ones, so its sealed list, scope and policy hash are reproducible.
//! Every ballot is the candidate row's real proof of its real statement, made with a fixed
//! public tape (`iroha_parliament_ballot::fixtures`), and carries a stamp nonce that was
//! mined once for exactly those bytes and is verified here at the frozen 28-bit difficulty
//! on every run. When an input of the sealed context changes, the ignored
//! `print_fixture_stamp_targets` prints the new targets to mine.

use std::sync::{Arc, OnceLock};

use iroha_data_model::governance::{
    anonymous_ballot::{
        AdmissionNonceV1, AnonymousBallotAuthorizationV1, BallotChoiceV1, BallotOutcomeV1,
        BallotProofBytesV1, SealedCredentialListBytesV1, SealedCredentialListPublishedV1,
    },
    types::GovernanceAttemptStatusV1,
};
use iroha_parliament_ballot::{
    context::ScopedBallotContextV1,
    corpus::ClosureAnchorsV1,
    fixtures::{fixture_authorization_v1, fixture_envelope_v1},
    replay::{
        FullVerificationRequestV1, FullVerificationV1, RecordedBallotResultV1, ReplayBallotV1,
        ReplayBlockV1, ReplaySourceErrorV1, verify_full_v1,
    },
    stamp::StampTargetV1,
};

use super::*;
use crate::{
    governance::parliament_anonymous::{
        credentials_tests::JURY,
        world_pass::tests::{Rows, header, jury_in_deliberation, no_pulse},
    },
    state::{StateBlock, WorldReadOnly},
};

/// One fixture ballot: the invited seat whose credential casts it, its choice, the tag of
/// its fixed tape and the pinned offset of its stamp nonce.
#[derive(Debug, Clone, Copy)]
struct Fixture {
    name: &'static str,
    seat: u8,
    vote: BallotChoiceV1,
    tape: u8,
    /// Flip one proof byte after proving: a well-stamped invalid proof.
    forged: bool,
    /// The stamp nonce: this offset, little-endian, in the first eight nonce bytes.
    nonce: u64,
}

/// Seat 0 votes Aye.
pub(crate) const AYE_0: usize = 0;
/// Seat 1 votes Aye.
pub(crate) const AYE_1: usize = 1;
/// Seat 2 votes Aye.
pub(crate) const AYE_2: usize = 2;
/// Seat 0 again with the conflicting choice Nay: the nullifier of [`AYE_0`].
pub(crate) const NAY_0: usize = 3;
/// A ballot of seat 1 whose proof has one byte flipped, with a valid stamp of its own.
pub(crate) const FORGED_1: usize = 4;

/// The fixture ballots and their pinned stamp nonces.
const FIXTURES: [Fixture; 5] = [
    Fixture {
        name: "AYE_0",
        seat: 0,
        vote: BallotChoiceV1::Aye,
        tape: 0x20,
        forged: false,
        nonce: STAMPS[AYE_0],
    },
    Fixture {
        name: "AYE_1",
        seat: 1,
        vote: BallotChoiceV1::Aye,
        tape: 0x21,
        forged: false,
        nonce: STAMPS[AYE_1],
    },
    Fixture {
        name: "AYE_2",
        seat: 2,
        vote: BallotChoiceV1::Aye,
        tape: 0x22,
        forged: false,
        nonce: STAMPS[AYE_2],
    },
    Fixture {
        name: "NAY_0",
        seat: 0,
        vote: BallotChoiceV1::Nay,
        tape: 0x23,
        forged: false,
        nonce: STAMPS[NAY_0],
    },
    Fixture {
        name: "FORGED_1",
        seat: 1,
        vote: BallotChoiceV1::Nay,
        tape: 0x24,
        forged: true,
        nonce: STAMPS[FORGED_1],
    },
];

/// The pinned nonce offsets, in fixture order. Each reaches 28 bits for the payload digest
/// `print_fixture_stamp_targets` prints; the `mine_fixture_stamp` example of
/// `iroha_parliament_ballot` finds one.
const STAMPS: [u64; 5] = [
    150_271_695,
    21_001_429,
    153_397_210,
    905_274_527,
    85_105_371,
];

/// Byte of the proof flipped in a forged fixture.
const FORGED_PROOF_BYTE: usize = 1_000;

/// The nonce `offset`: little-endian in the first eight bytes.
fn nonce(offset: u64) -> [u8; AdmissionNonceV1::LENGTH] {
    let mut out = [0_u8; AdmissionNonceV1::LENGTH];
    out[..8].copy_from_slice(&offset.to_le_bytes());
    out
}

/// The pass tests' rows with the jury in Deliberation and its board sealed, and the height
/// of their last block. Built once: its three registration proofs are genuine.
fn deliberating() -> (Rows, u64) {
    static ROWS: OnceLock<(Rows, u64)> = OnceLock::new();
    ROWS.get_or_init(jury_in_deliberation).clone()
}

/// The checked context of the sealed jury's scope.
fn context() -> Arc<ScopedBallotContextV1> {
    let (rows, _) = deliberating();
    let board = BallotBoardV1::restore(rows.boards[0].1.clone()).expect("the sealed board");
    Arc::clone(board.context())
}

/// The authorization of fixture `index`, proved once per test binary.
pub(crate) fn authorization(index: usize) -> AnonymousBallotAuthorizationV1 {
    static PROOFS: [OnceLock<AnonymousBallotAuthorizationV1>; 5] = [const { OnceLock::new() }; 5];
    PROOFS[index]
        .get_or_init(|| {
            let fixture = FIXTURES[index];
            let mut authorization = fixture_authorization_v1(
                &context(),
                &[0x40 + fixture.seat; 48],
                fixture.vote,
                &[fixture.tape; 112],
            )
            .expect("a seated credential proves its ballot");
            if fixture.forged {
                let mut proof = authorization.proof.into_bytes();
                proof[FORGED_PROOF_BYTE] ^= 0x01;
                authorization.proof = BallotProofBytesV1::new(proof).expect("exact length");
            }
            authorization
        })
        .clone()
}

/// The canonical envelope of `authorization` stamped with the nonce `offset`.
pub(crate) fn stamped(
    authorization: &AnonymousBallotAuthorizationV1,
    offset: u64,
) -> CanonicalAnonymousBallotEnvelopeV1 {
    fixture_envelope_v1(authorization, nonce(offset)).expect("a canonical envelope")
}

/// Fixture ballot `index` with its pinned stamp.
pub(crate) fn envelope(index: usize) -> CanonicalAnonymousBallotEnvelopeV1 {
    stamped(&authorization(index), FIXTURES[index].nonce)
}

/// The pinned nonce offset of fixture `index`.
pub(crate) fn pinned_offset(index: usize) -> u64 {
    FIXTURES[index].nonce
}

/// The pass tests' rows with the jury's casting window open and its clock established, the
/// height of their last block and the window's closing time. No ballot has been cast.
fn casting() -> (Rows, u64, u64) {
    let (mut rows, mut height) = deliberating();
    height += 1;
    rows.block(height, rows.next_due());
    if rows.boards[0].1.opening.is_none() {
        let opening_at = rows.attempt().opening_at_ms(JURY).expect("scheduled");
        height += 1;
        rows.block(height, opening_at);
    }
    let s0 = rows.boards[0].1.opening.expect("casting opened");
    height = s0.height + 1;
    rows.block(height, s0.timestamp_ms + 1);
    let closing_ms = BallotBoardV1::restore(rows.boards[0].1.clone())
        .expect("the board")
        .closing_ms()
        .expect("the clock fixes the closing time");
    (rows, height, closing_ms)
}

/// The open casting window of [`casting`] for the tests of other modules (the queue's ballot
/// partition, block validation): they see committed State and never the pass tests' rows.
pub(crate) struct CastingFixtureV1 {
    rows: Rows,
    height: u64,
    closing_ms: u64,
}

impl CastingFixtureV1 {
    /// The jury's casting window, open with its clock established; no ballot has been cast.
    pub(crate) fn open() -> Self {
        let (rows, height, closing_ms) = casting();
        Self {
            rows,
            height,
            closing_ms,
        }
    }

    /// The scope of the jury's ballot.
    pub(crate) fn scope(&self) -> BallotScopeV1 {
        self.rows.boards[0].0
    }

    /// The time at which the window closes.
    pub(crate) fn closing_ms(&self) -> u64 {
        self.closing_ms
    }

    /// The height of the block after the fixture's last one.
    pub(crate) fn next_height(&self) -> u64 {
        self.height + 1
    }

    /// The header of the block after the fixture's last one, created at `timestamp_ms`.
    pub(crate) fn next_header(&self, timestamp_ms: u64) -> iroha_data_model::block::BlockHeader {
        header(self.height + 1, timestamp_ms)
    }

    /// A State that commits the fixture's rows on a global root whose selection installs the
    /// anonymous family.
    pub(crate) fn state(&self) -> crate::state::State {
        self.rows.state_on_global_root()
    }

    /// Commit the next block, created at `timestamp_ms`, whose only input `envelope` is
    /// accepted into the corpus.
    pub(crate) fn accept(
        &mut self,
        timestamp_ms: u64,
        envelope: &CanonicalAnonymousBallotEnvelopeV1,
    ) {
        self.height += 1;
        self.rows
            .run_with(self.height, timestamp_ms, no_pulse, true, |block| {
                assert_eq!(
                    execute(block, envelope, 0),
                    Ok(BallotInputOutcomeV1::Accepted)
                );
            })
            .expect("no item is due");
    }
}

/// Execute `envelope` at input `index` of `block` in its own transaction, applied only with
/// an accepted row, as the Network output owner does.
pub(crate) fn execute(
    block: &mut StateBlock<'_>,
    envelope: &CanonicalAnonymousBallotEnvelopeV1,
    index: u32,
) -> Result<BallotInputOutcomeV1, BallotInputErrorV1> {
    let mut tx = block.try_transaction().expect("a World transaction");
    let outcome = execute_ballot_input(&mut tx, envelope, index);
    if outcome == Ok(BallotInputOutcomeV1::Accepted) {
        tx.apply();
    }
    outcome
}

/// [`execute`] for an input a valid block carries: the input and the result its row records
/// join `carried`, the evidence of the block's independent replay.
fn carry(
    block: &mut StateBlock<'_>,
    carried: &mut Vec<ReplayBallotV1>,
    envelope: &CanonicalAnonymousBallotEnvelopeV1,
) -> BallotInputOutcomeV1 {
    let input_index = u32::try_from(carried.len()).expect("few inputs");
    let outcome = execute(block, envelope, input_index).expect("an input a valid block carries");
    carried.push(ReplayBallotV1 {
        input_index,
        envelope: envelope.as_bytes().to_vec(),
        recorded: match outcome {
            BallotInputOutcomeV1::Accepted => RecordedBallotResultV1::Accepted,
            BallotInputOutcomeV1::Rejected(reason) => RecordedBallotResultV1::Rejected(reason),
        },
    });
    outcome
}

/// The board `block` keeps for `scope`.
fn board(block: &StateBlock<'_>, scope: &BallotScopeV1) -> ParliamentBallotBoardRecordV1 {
    block
        .world
        .parliament_anonymous_boards
        .get(scope)
        .expect("the jury's board")
        .clone()
}

#[test]
#[ignore = "prints the stamp targets to mine after a fixture input changed"]
fn print_fixture_stamp_targets() {
    let context = context();
    for (index, fixture) in FIXTURES.iter().enumerate() {
        let target = StampTargetV1::for_ballot(&authorization(index), context.context())
            .expect("a stamp target");
        println!(
            "{} payload_digest={} difficulty_bits={} pinned_offset={} pinned_ok={}",
            fixture.name,
            hex::encode(target.payload_digest().as_bytes()),
            target.difficulty_bits(),
            fixture.nonce,
            target.accepts(&AdmissionNonceV1::new(nonce(fixture.nonce))),
        );
    }
}

#[test]
fn the_pinned_stamps_reach_the_frozen_difficulty() {
    let context = context();
    for (index, fixture) in FIXTURES.iter().enumerate() {
        let target = StampTargetV1::for_ballot(&authorization(index), context.context())
            .expect("a stamp target");
        assert!(target.difficulty_bits() >= 28, "the frozen difficulty");
        assert!(
            target.accepts(&AdmissionNonceV1::new(nonce(fixture.nonce))),
            "re-mine {} with `print_fixture_stamp_targets`",
            fixture.name
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one sealed jury carries its ballots from before the opening to the closure"
)]
fn ballots_are_decided_by_their_board_from_before_the_opening_to_the_closure() {
    let (mut rows, mut height) = deliberating();
    let scope = rows.boards[0].0;
    let unchanged = |block: &StateBlock<'_>, before: &ParliamentBallotBoardRecordV1| {
        assert_eq!(
            &board(block, &scope),
            before,
            "a refused input changes nothing"
        );
    };

    // Before Reflection the sealed board has no window: a valid ballot is recorded as
    // rejected and changes nothing. Its bytes stay good for a later block.
    let reflection_ms = rows.next_due();
    height += 1;
    let idle = rows
        .run_with(height, reflection_ms - 1, no_pulse, true, |block| {
            let before = board(block, &scope);
            assert_eq!(
                execute(block, &envelope(AYE_0), 0),
                Ok(BallotInputOutcomeV1::Rejected(
                    AnonymousBallotRejectionV1::OutsideCastingWindow
                ))
            );
            unchanged(block, &before);
        })
        .expect("the pass");
    assert!(!idle.governance_due_applied());

    // The opening block: its pass opens the window, so its own ballots are inside. The
    // same bytes that were early are accepted now, and the first ballot in canonical order
    // wins its nullifier against a conflicting choice of the same credential.
    // Every input a block from the opening through the closure carries, with its recorded
    // result: the evidence of the independent replay below.
    let mut replay: Vec<ReplayBlockV1> = Vec::new();
    let opening_inputs = |block: &mut StateBlock<'_>| {
        let opening = board(block, &scope)
            .opening
            .expect("the pass opened the window");
        let mut carried = Vec::new();
        assert_eq!(
            carry(block, &mut carried, &envelope(AYE_0)),
            BallotInputOutcomeV1::Accepted
        );
        let accepted = board(block, &scope);
        let [entry] = accepted.entries.as_slice() else {
            panic!("one accepted ballot: {:?}", accepted.entries)
        };
        assert_eq!(entry.position(), (opening.height, 0));
        assert_eq!(entry.vote, BallotChoiceV1::Aye);
        assert_eq!(
            carry(block, &mut carried, &envelope(NAY_0)),
            BallotInputOutcomeV1::Rejected(AnonymousBallotRejectionV1::DuplicateNullifier)
        );
        unchanged(block, &accepted);
        carried
    };
    let mut opening_block = None;
    height += 1;
    rows.run_with(height, reflection_ms, no_pulse, true, |block| {
        if board(block, &scope).opening.is_some() {
            opening_block = Some(opening_inputs(block));
        }
    })
    .expect("the pass enters Reflection");
    let opening_at = rows.attempt().opening_at_ms(JURY).expect("scheduled");
    if opening_block.is_none() {
        // The notice had not elapsed at Reflection: the window opens at its own pass.
        height += 1;
        rows.run_with(height, opening_at, no_pulse, true, |block| {
            opening_block = Some(opening_inputs(block));
        })
        .expect("the pass opens the window");
    }
    let s0 = rows.boards[0].1.opening.expect("casting opened");
    assert_eq!(rows.boards[0].1.entries.len(), 1);
    replay.push(ReplayBlockV1 {
        height: s0.height,
        timestamp_ms: s0.timestamp_ms,
        ballots: opening_block.expect("the opening block ran its inputs"),
    });

    // The successor block establishes the clock in its prelude; its ballots are inside.
    height = s0.height + 1;
    let clock_ms = s0.timestamp_ms + 1;
    let mut carried = Vec::new();
    rows.run_with(height, clock_ms, no_pulse, true, |block| {
        assert_eq!(
            carry(block, &mut carried, &envelope(AYE_1)),
            BallotInputOutcomeV1::Accepted
        );
        let accepted = board(block, &scope);
        assert_eq!(accepted.entries.len(), 2);
        assert_eq!(accepted.entries[1].position(), (height, 0));

        // A byte-identical ballot of an earlier block is decided again by its nullifier.
        assert_eq!(
            carry(block, &mut carried, &envelope(AYE_0)),
            BallotInputOutcomeV1::Rejected(AnonymousBallotRejectionV1::DuplicateNullifier)
        );
        unchanged(block, &accepted);

        // Inputs no valid block carries. None of them is recorded, and each is a verdict.
        let mut invalid = |envelope: &CanonicalAnonymousBallotEnvelopeV1| {
            let mut tx = block.try_transaction().expect("a World transaction");
            let error = execute_ballot_input(&mut tx, envelope, 2).expect_err("an invalid input");
            assert!(!error.is_local(), "a verdict on the block: {error}");
            let BallotInputErrorV1::BlockInvalid(reason) = error else {
                unreachable!("not local")
            };
            reason
        };
        // A stamp that does not reach the frozen difficulty.
        assert_eq!(
            invalid(&stamped(&authorization(AYE_2), FIXTURES[AYE_2].nonce + 1)),
            BlockInvalidReasonV1::InsufficientStamp
        );
        // A well-stamped proof that does not verify.
        assert_eq!(
            invalid(&envelope(FORGED_1)),
            BlockInvalidReasonV1::ProofRejected
        );
        // A scope no sealed body has, and another network.
        let mut foreign = authorization(AYE_2);
        foreign.scope = BallotScopeV1::new([0xee; 64]);
        assert_eq!(
            invalid(&stamped(&foreign, FIXTURES[AYE_2].nonce)),
            BlockInvalidReasonV1::UnknownScope
        );
        let mut foreign = authorization(AYE_2);
        foreign.network_id = crate::unit_test_support::synthetic_network_id("another network");
        assert_eq!(
            invalid(&stamped(&foreign, FIXTURES[AYE_2].nonce)),
            BlockInvalidReasonV1::WrongNetwork
        );
        unchanged(block, &accepted);
    })
    .expect("the prelude establishes the clock");
    replay.push(ReplayBlockV1 {
        height,
        timestamp_ms: clock_ms,
        ballots: carried,
    });
    let closing_ms = BallotBoardV1::restore(rows.boards[0].1.clone())
        .expect("the board")
        .closing_ms()
        .expect("the clock fixes the closing time");

    // One millisecond before the closing time the window is still open.
    height += 1;
    let mut carried = Vec::new();
    rows.run_with(height, closing_ms - 1, no_pulse, true, |block| {
        assert_eq!(
            carry(block, &mut carried, &envelope(AYE_2)),
            BallotInputOutcomeV1::Accepted
        );
    })
    .expect("no item is due");
    assert_eq!(rows.boards[0].1.entries.len(), 3);
    replay.push(ReplayBlockV1 {
        height,
        timestamp_ms: closing_ms - 1,
        ballots: carried,
    });

    // The closing block: its pass freezes the corpus first, so a ballot of that block is
    // late, and the window is decided before the nullifier.
    height += 1;
    let mut carried = Vec::new();
    let report = rows
        .run_with(height, closing_ms, no_pulse, true, |block| {
            let closed = board(block, &scope);
            assert!(closed.closure.is_some(), "the pass closed the ballot");
            assert_eq!(
                carry(block, &mut carried, &envelope(AYE_0)),
                BallotInputOutcomeV1::Rejected(AnonymousBallotRejectionV1::OutsideCastingWindow)
            );
            unchanged(block, &closed);
        })
        .expect("the pass closes the ballot");
    assert_eq!(report.processed, 1);
    replay.push(ReplayBlockV1 {
        height,
        timestamp_ms: closing_ms,
        ballots: carried,
    });
    let closure = rows.boards[0].1.closure.expect("the closure record");
    assert_eq!(
        (
            closure.accepted_ballots,
            closure.aye,
            closure.nay,
            closure.abstain
        ),
        (3, 3, 0, 0),
        "the closure counts exactly the accepted corpus"
    );
    assert_eq!(closure.outcome, BallotOutcomeV1::Approved);
    assert_eq!(closure.opening_anchor, s0);
    assert_ne!(
        rows.attempt().attempt().status,
        GovernanceAttemptStatusV1::Rejected
    );
    assert!(rows.attempt().rejection().is_none());

    // Independent full verification (design §3.9): from the seal, the anchors, the recorded
    // closure and exactly the inputs the blocks carried, every stamp and proof is verified
    // again, rejected ballots included, and the rebuilt corpus, counts and closure record
    // are the ones World recorded.
    let record = &rows.boards[0].1;
    let board = BallotBoardV1::restore(record.clone()).expect("the closed board");
    let clock = record.casting_clock.expect("the casting clock");
    let request = FullVerificationRequestV1 {
        context: Arc::clone(board.context()),
        seal: SealedCredentialListPublishedV1 {
            body_context: record.context.body_context,
            body_instance_id: record.body_instance_id,
            roster_root: record.roster_root,
            list_digest: *board.context().list_digest(),
            registration_mapping_digest: record.registration_mapping_digest,
            sealed_list_bytes: SealedCredentialListBytesV1::from_list(
                &record.context.sealed_list().expect("the sealed list"),
            )
            .expect("canonical list bytes"),
        },
        other_scopes: Vec::new(),
        proposal_content_id: record.proposal_content_id,
        anchors: ClosureAnchorsV1 {
            opening: s0,
            casting_clock_height: clock.height,
            casting_clock_timestamp_ms: clock.timestamp_ms,
            closure: closure.closure_anchor,
        },
        closure_record: closure,
        certified_result_root: None,
    };
    let verdict = verify_full_v1(
        &request,
        replay.clone().into_iter().map(Ok::<_, ReplaySourceErrorV1>),
    );
    let FullVerificationV1::IndependentFullVerification(verified) = verdict else {
        panic!("the committed ballot verifies independently: {verdict:?}")
    };
    assert_eq!(verified.closure_record, closure);
    assert_eq!(
        (
            verified.blocks,
            verified.ballots_verified,
            verified.accepted,
            verified.rejected_duplicate,
            verified.rejected_outside_window
        ),
        (4, 6, 3, 2, 1)
    );
    // An omitted accepted input is incomplete evidence, never another outcome.
    let mut omitted = replay;
    omitted[2].ballots.clear();
    assert!(matches!(
        verify_full_v1(
            &request,
            omitted.into_iter().map(Ok::<_, ReplaySourceErrorV1>)
        ),
        FullVerificationV1::IncompleteEvidence(_) | FullVerificationV1::InvalidEvidence(_)
    ));
}

#[test]
fn a_stored_board_that_does_not_recompute_defers_the_input() {
    let (mut rows, height) = deliberating();
    // An opening time before the end of the notice: no transition produces it.
    rows.boards[0].1.opening_at_ms = Some(0);
    let state = rows.state();
    let mut block = state.block(header(height + 1, rows.next_due() - 1));
    let mut tx = block.try_transaction().expect("a World transaction");
    // The stamp is irrelevant: the board is recomputed when its scope is looked up.
    let error = execute_ballot_input(&mut tx, &stamped(&authorization(AYE_0), 0), 0)
        .expect_err("an unusable board");
    assert!(matches!(error, BallotInputErrorV1::Board(_)), "{error}");
    assert!(error.is_local(), "World's corruption is never a verdict");
}

#[test]
fn the_block_budget_charges_one_reservation_and_the_copied_bytes_per_ballot() {
    let (mut rows, _) = deliberating();
    let context = context();
    // Stamps do not enter the budget: any nonce sizes the envelope.
    let ballots: Vec<_> = [AYE_0, AYE_1, AYE_2]
        .into_iter()
        .map(|index| stamped(&authorization(index), 0))
        .collect();
    let check = |rows: &Rows, ballots: &[&CanonicalAnonymousBallotEnvelopeV1]| {
        let state = rows.state();
        let view = state.view();
        let world = view.world();
        check_block_ballot_budget(
            world.parameters(),
            |scope| world.parliament_anonymous_boards().get(scope),
            ballots.iter().copied(),
        )
    };

    // One verification copies the canonical envelope and the scope's canonical context.
    let one = {
        let state = rows.state();
        let view = state.view();
        verifier_input_bytes(
            |scope| view.world().parliament_anonymous_boards().get(scope),
            &ballots[0],
        )
        .expect("a sealed scope")
    };
    assert_eq!(
        usize::try_from(one).expect("bytes"),
        ballots[0].as_bytes().len() + context.encoded_context().as_bytes().len()
    );

    // The default budget is two reservations.
    assert_eq!(check(&rows, &[]), Ok(()));
    assert_eq!(check(&rows, &[&ballots[0], &ballots[1]]), Ok(()));
    assert_eq!(
        check(&rows, &[&ballots[0], &ballots[1], &ballots[2]]),
        Err(BallotBudgetErrorV1::Reservations { limit: 2 })
    );
    // One carrier is carried once, and every carrier names a sealed scope.
    assert_eq!(
        check(&rows, &[&ballots[0], &ballots[0]]),
        Err(BallotBudgetErrorV1::DuplicateCarrier)
    );
    let mut foreign = authorization(AYE_0);
    foreign.scope = BallotScopeV1::new([0xee; 64]);
    let error = check(&rows, &[&stamped(&foreign, 0)]).expect_err("an unknown scope");
    assert_eq!(error, BallotBudgetErrorV1::UnknownScope);
    assert!(!error.is_local());

    // The byte budget of one maximal verification call admits one of these ballots, not two.
    let limit = iroha_data_model::parameter::ParliamentBallotParametersV1::MIN_VERIFIER_INPUT_BYTES;
    rows.ballot_budget = Some((8, limit));
    assert!(one <= limit && 2 * one > limit);
    assert_eq!(check(&rows, &[&ballots[0]]), Ok(()));
    assert_eq!(
        check(&rows, &[&ballots[0], &ballots[1]]),
        Err(BallotBudgetErrorV1::Bytes { limit })
    );
}

#[test]
fn a_charge_that_does_not_fit_takes_nothing() {
    let parameters = iroha_data_model::parameter::Parameters::default();
    let mut budget = BallotBlockBudgetV1::of(&parameters).expect("the default policy");
    let fresh = budget;
    let default = iroha_data_model::parameter::ParliamentBallotParametersV1::default();
    let limit = default.max_verifier_input_bytes_per_block;
    assert_eq!(default.max_verification_reservations_per_block, 2);

    // More bytes than the whole budget: nothing is charged, the reservation included.
    assert_eq!(
        budget.charge(limit + 1),
        Err(BallotBudgetErrorV1::Bytes { limit })
    );
    assert_eq!(budget, fresh);
    // Exactly the budget in two charges, then no reservation is left even for zero bytes.
    assert_eq!(budget.charge(limit - 1), Ok(()));
    assert_eq!(budget.charge(1), Ok(()));
    assert_eq!(
        budget.charge(0),
        Err(BallotBudgetErrorV1::Reservations { limit: 2 }),
        "a block carries at most B_work ballots, whatever their bytes"
    );
}
