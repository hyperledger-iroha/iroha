//! Real retained snapshots bound graph work while preserving original custody and decode refusal.

use super::*;
use crate::managed::stream_token_custody::{
    bootstrap_test_support::NativeEnrollmentReads,
    renewal_tests::{Fixture, wait_until},
};
use std::{cell::Cell, time::Duration};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    records: usize,
    snapshots: usize,
}
std::thread_local! {
    static COUNTS: Cell<Option<Counts>> = const { Cell::new(None) };
}
pub(super) fn record_read() {
    COUNTS.with(|state| {
        if let Some(mut counts) = state.get() {
            counts.records += 1;
            state.set(Some(counts));
        }
    });
}
pub(super) fn snapshot_visit() {
    COUNTS.with(|state| {
        if let Some(mut counts) = state.get() {
            counts.snapshots += 1;
            state.set(Some(counts));
        }
    });
}
fn counted<T>(action: impl FnOnce() -> T) -> (T, Counts) {
    struct Restore(Option<Counts>);
    impl Drop for Restore {
        fn drop(&mut self) {
            COUNTS.with(|state| state.set(self.0));
        }
    }
    let _restore = Restore(COUNTS.with(|state| state.replace(Some(Counts::default()))));
    let result = action();
    let counts = COUNTS.with(|state| state.get().unwrap());
    (result, counts)
}

// Two genuine signed bodies, one naturally expired and canonically retired request, and
// unchanged native custody. No forged History, proof, signature or synthetic expiry clock.
fn history() -> (Fixture, BodyHistory) {
    let fixture = Fixture::enrolled_with_renewal_validity(4_000, 8_000);
    wait_until(
        fixture.initial.issued_at_unix_ms + 2_000,
        Duration::from_secs(4),
    );
    let mut retained: Option<BodyHistory> = None;
    for ordinal in 1..=2 {
        if let Some(history) = retained.as_ref() {
            wait_until(
                history
                    .bodies
                    .last()
                    .unwrap()
                    .reservation
                    .unsigned
                    .statement
                    .expires_at_unix_ms,
                Duration::from_secs(10),
            );
        }
        drop(retained.take());
        let mut turn = fixture.renewal_turn();
        let (checkpoint, current) = fixture.current();
        let (finished, authorization) = if ordinal > 1 {
            let previous = BodyHistory::open(&fixture.owner, CustodyPurpose::Renewal(2))
                .unwrap()
                .unwrap();
            let authorization = turn
                .authorize_retained(&fixture.owner, &previous, fixture.options.deadline)
                .unwrap();
            let terms = Terms::new(now_ms().unwrap() + 8_000, &fixture.options).unwrap();
            let unsigned = fixture
                .owner
                .select_renewal_unsigned(
                    2,
                    &fixture.policy,
                    &current,
                    &checkpoint,
                    &terms,
                    fixture.options.deadline,
                )
                .unwrap();
            let selected = previous
                .reserve_successor(
                    &fixture.owner,
                    unsigned,
                    &current,
                    authorization,
                    fixture.options.deadline,
                )
                .unwrap();
            let finished = selected
                .finish_pending_with_reads(
                    &fixture.owner,
                    &current,
                    &SigningTurn::Generated(authorization),
                    fixture.options.deadline,
                    &NativeEnrollmentReads(&fixture.native),
                )
                .unwrap();
            (finished, authorization)
        } else {
            let terms = Terms::new(now_ms().unwrap() + 8_000, &fixture.options).unwrap();
            let unsigned = fixture
                .owner
                .select_renewal_unsigned(
                    2,
                    &fixture.policy,
                    &current,
                    &checkpoint,
                    &terms,
                    fixture.options.deadline,
                )
                .unwrap();
            let selected = BodyHistory::initialize(
                &fixture.owner,
                CustodyPurpose::Renewal(2),
                unsigned,
                &Fees::from_options(&fixture.options).unwrap(),
                &SigningTurn::RenewalSelection(&turn),
                fixture.options.deadline,
            )
            .unwrap();
            let authorization = turn
                .authorize_retained(&fixture.owner, &selected, fixture.options.deadline)
                .unwrap();
            let finished = selected
                .finish_pending_with_reads(
                    &fixture.owner,
                    &current,
                    &SigningTurn::Generated(authorization),
                    fixture.options.deadline,
                    &NativeEnrollmentReads(&fixture.native),
                )
                .unwrap();
            (finished, authorization)
        };
        assert!(finished.original().unwrap().is_some());
        if ordinal == 1 {
            let selected = fixture
                .retain_generated_attempt(authorization, &finished, &current)
                .unwrap();
            assert!(
                !selected
                    .directory()
                    .path()
                    .join("transaction/payload.json")
                    .exists()
            );
        }
        drop(turn);
        retained = Some(finished.reopen(&fixture.owner).unwrap());
    }
    (fixture, retained.unwrap())
}

#[test]
fn genuine_graph_snapshot_pass_bounds_reads_and_preserves_mutation_and_decode_refusals() {
    let _resources = crate::managed::native_test_guard();
    let (fixture, history) = history();
    assert_eq!(history.bodies.len(), 2);
    assert_eq!(fixture.native.chain.height(), 4);
    let native = history.current_history().unwrap();
    let ((result, census), work) = counted(|| native.test_require_current(4));
    result.unwrap();
    assert_eq!(census.visits, 4);
    assert_eq!(census.distinct_histories, 2);
    // Four whole-head reads: entry/exit for each original forward/reverse graph pass.
    // Each head contains three root records plus four records for each genuine body.
    assert_eq!(work.records, 4 * (3 + 4 * history.bodies.len()));
    assert_eq!(work.snapshots, 4 * (2 + history.bodies.len()));

    let evidence = native.test_enrollment_evidence().unwrap();
    let reopened = BodyHistory::open(&fixture.owner, CustodyPurpose::Renewal(2))
        .unwrap()
        .unwrap();
    let foreign = reopened
        .current_history()
        .unwrap()
        .test_enrollment_evidence()
        .unwrap();
    evidence
        .with_snapshot_read_pass(&mut |pass| {
            let pass = pass.unwrap();
            assert!(pass.covers(&history.bodies[0].snapshots));
            assert!(pass.covers(&history.bodies[1].snapshots));
            assert!(!pass.covers(&reopened.bodies[1].snapshots));
            let (result, work) = counted(|| foreign.revalidate_with_snapshot_read_pass(Some(pass)));
            result?;
            assert_eq!(work.records, 3 + 4 * reopened.bodies.len());
            // A nested decode owner cannot inherit a token created outside its budget.
            let limits = norito::DecodeLimits::new(
                MAX_BODY_BYTES,
                MAX_ALL_BODY_BYTES,
                MAX_ALL_BODY_BYTES,
                512 * 1024 * 1024,
                64,
            );
            let (result, nested) = counted(|| {
                norito::core::with_decode_limits_scope(limits, || {
                    assert!(!pass.covers(&history.bodies[1].snapshots));
                    evidence.revalidate_with_snapshot_read_pass(Some(pass))
                })
            });
            result?;
            assert_eq!(nested.records, 3 + 4 * history.bodies.len());
            Ok(())
        })
        .unwrap();
    drop(reopened);

    let oldest = &history.bodies[0].directory;
    let original = oldest
        .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
        .unwrap();
    let mut changed = original.to_vec();
    changed[0] ^= 1;
    let error = evidence.with_snapshot_read_pass(&mut |pass| {
        assert!(pass.is_some());
        oldest.write_atomic("original.nrt", &changed, PublishMode::Replace)?;
        Err(invalid("inner observation failed"))
    });
    assert!(matches!(error, Err(crate::managed::Error::Invalid(message))
        if message == "retained enrollment body material changed"));
    assert!(evidence.revalidate().is_err()); // No verdict survives the closed lexical pass.
    oldest
        .write_atomic("original.nrt", &original, PublishMode::Replace)
        .unwrap();
    evidence.revalidate().unwrap();
    native.test_require_current(4).0.unwrap();

    // Closure records are not snapshot memoization candidates: original native metadata
    // and receipt verification must still refuse a changed oldest closed transition.
    let closed = oldest
        .read("closed.nrt", attempts::MAX_RECORD_BYTES)
        .unwrap();
    let mut changed = closed.to_vec();
    changed[0] ^= 1;
    oldest
        .write_atomic("closed.nrt", &changed, PublishMode::Replace)
        .unwrap();
    assert!(native.test_require_current(4).0.is_err());
    oldest
        .write_atomic("closed.nrt", &closed, PublishMode::Replace)
        .unwrap();
    native.test_require_current(4).0.unwrap();

    let wide = norito::DecodeLimits::new(
        MAX_BODY_BYTES,
        MAX_ALL_BODY_BYTES,
        MAX_ALL_BODY_BYTES,
        512 * 1024 * 1024,
        64,
    );
    let ((result, census), active) =
        counted(|| norito::core::with_decode_limits_scope(wide, || native.test_require_current(4)));
    result.unwrap();
    assert_eq!(census.visits, 4);
    // The active owner retains the complete old recipe: each History's local census
    // checks its whole snapshot prefix before and after, in both graph traversals.
    assert_eq!(active.records, 4 * ((3 + 4) + (3 + 8)));
    assert!(active.records > work.records);
    for allocation in [0, 1] {
        let limits = norito::DecodeLimits::new(
            MAX_BODY_BYTES,
            MAX_ALL_BODY_BYTES,
            MAX_ALL_BODY_BYTES,
            allocation,
            64,
        );
        assert!(
            norito::core::with_decode_limits_scope(limits, || native.test_require_current(4).0)
                .is_err()
        );
    }
    native.test_require_current(4).0.unwrap();
    assert_eq!(
        oldest
            .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
            .unwrap(),
        original
    );
    assert_eq!(
        oldest
            .read("closed.nrt", attempts::MAX_RECORD_BYTES)
            .unwrap(),
        closed
    );

    #[cfg(unix)]
    {
        let path = oldest.path().to_owned();
        let held = fixture._temporary.path().join("held-oldest-snapshot");
        let result = evidence.with_snapshot_read_pass(&mut |_| {
            std::fs::rename(&path, &held)?;
            let replacement = PrivateDirectory::open_or_create(&path)?;
            replacement.write_atomic("original.nrt", &original, PublishMode::CreateNew)?;
            Ok(())
        });
        assert!(result.is_err());
        std::fs::remove_file(path.join("original.nrt")).unwrap();
        std::fs::remove_dir(&path).unwrap();
        std::fs::rename(&held, &path).unwrap();
        native.test_require_current(4).0.unwrap();
    }
}
