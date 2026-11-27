//! Original codec refusals abort the disposable migration operation, including successful prefixes.

use super::*;
use crate::execution_attempt::{
    ExecutionAttemptError, ExecutionDeferred, canonical_decode_attempt_error,
};
use crate::runtime::{
    catalog::migrations::{DrainOutcome, MigrationAddition, StageOutcome, test_migrations},
    migration::storage::{
        BarrierStorage, PointAndScan, SEQUENCE_TABLES, SourceChanges, SourceDrain, StageStorage,
        TableMigration,
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::BinaryHeap,
};

thread_local! {
    static PROBE_LIMIT: Cell<Option<usize>> = const { Cell::new(None) };
    static COMPLETED_CALLBACKS: Cell<u32> = const { Cell::new(0) };
    static ORIGINAL_REFUSAL: RefCell<Option<norito::core::DecodeLocalRefusal>> = const { RefCell::new(None) };
}

/// Constrain only this fixture's real serializer scratch request. No reason is manufactured.
fn with_probe_refusal<T>(action: impl FnOnce() -> T) -> T {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PROBE_LIMIT.with(|limit| limit.set(self.0));
        }
    }
    let _restore = Restore(PROBE_LIMIT.with(|limit| limit.replace(Some(0))));
    ORIGINAL_REFUSAL.with(|owner| *owner.borrow_mut() = None);
    action()
}

fn probe() -> MigrationResult<()> {
    // A heap's canonical encoding is deterministic but sorts through serializer scratch.
    let values = BinaryHeap::from([(3_u64, 7_u64), (1, 9)]);
    let result = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            PROBE_LIMIT.with(|limit| limit.get().unwrap_or(usize::MAX)),
            usize::MAX,
        ),
        || norito::core::classify_decode_attempt(|| norito::encode_canonical(&values)),
    );
    result.map(|_| ()).map_err(|error| {
        ORIGINAL_REFUSAL.with(|owner| *owner.borrow_mut() = error.local_refusal());
        canonical_decode_attempt_error(error, |error| error.to_string())
    })
}

fn assert_original(error: MigrationBoundaryError) -> ExecutionDeferred {
    let MigrationBoundaryError::Execution(ExecutionAttemptError::Deferred(original)) = error else {
        panic!("unfinished serializer was converted into a deterministic migration result");
    };
    let expected = ORIGINAL_REFUSAL
        .with(|owner| owner.borrow_mut().take())
        .unwrap();
    assert_eq!(original.decode_refusal(), Some(&expected));
    original
}

fn stage(
    storage: StageStorage<'_, '_, PointAndScan, AccountId, u64, Hash, bool>,
    cursor: &[u8],
    limit: u32,
) -> MigrationResult<StageOutcome> {
    let result = test_migrations::stage(storage, cursor, limit)?;
    COMPLETED_CALLBACKS.with(|count| count.set(count.get() + 1));
    probe()?;
    Ok(result)
}
fn barrier_after_writes(
    storage: BarrierStorage<'_, '_, '_, AccountId, u64, Hash, bool>,
    cursor: &[u8],
) -> MigrationResult<()> {
    test_migrations::barrier(storage, cursor)?;
    COMPLETED_CALLBACKS.with(|count| count.set(count.get() + 1));
    probe()
}
fn drain(
    storage: SourceDrain<'_, '_, AccountId, u64>,
    limit: u32,
) -> MigrationResult<DrainOutcome> {
    let result = test_migrations::drain(storage, limit)?;
    COMPLETED_CALLBACKS.with(|count| count.set(count.get() + 1));
    probe()?;
    Ok(result)
}
fn stale(source: SourceChanges<'_, '_, AccountId, u64>) -> bool {
    source.touched_entries().any(|entry| entry.after.is_some())
}
static PROGRAM: TableMigration<PointAndScan, AccountId, u64, Hash, bool> =
    SEQUENCE_TABLES.migration(stage, barrier_after_writes, drain, stale);

fn with_program<T>(action: impl FnOnce() -> T) -> T {
    test_catalog::with_release(
        &[MIGRATED, RUNTIME_LIFECYCLE],
        &[MigrationAddition {
            code: &PROGRAM,
            ..SEQUENCES
        }],
        action,
    )
}

#[derive(Debug, PartialEq, Eq)]
struct Observed {
    source: Vec<(AccountId, u64)>,
    successor: Vec<(Hash, bool)>,
    selection: RootSelectionV1,
    records: Vec<(DefinitionId, MigrationRecordV1)>,
}
fn observe(world: &World) -> Observed {
    let (source, successor) = rows(world);
    let view = world.view();
    Observed {
        source,
        successor,
        selection: view.runtime_selection().clone(),
        records: view
            .runtime_migrations()
            .iter()
            .map(|(id, record)| (*id, record.clone()))
            .collect(),
    }
}

#[test]
fn stage_refusal_after_successful_copy_restores_its_original_child_and_cursor() {
    with_program(|| {
        let world = world();
        schedule_step(&world, MigrationStep::Start, 10);
        let original = observe(&world);
        let mut block = world.block();
        let mut activation = block.transaction_without_telemetry(Default::default(), 0);
        activate_due(&mut activation, 10, 1_000, &committee()).unwrap();
        activation.apply();
        let completed = COMPLETED_CALLBACKS.with(Cell::get);
        let successor_observation = block.viral_bonus_paid.write_observation();
        let refusal = assert_original(
            with_probe_refusal(|| run_stages_for_testing(&mut block, 10)).unwrap_err(),
        );
        assert_eq!(COMPLETED_CALLBACKS.with(Cell::get), completed + 1);
        assert_eq!(block.viral_bonus_paid.iter().count(), 0);
        assert_eq!(
            block.viral_bonus_paid.write_observation(),
            successor_observation
        );
        assert_eq!(block.viral_bonus_paid.touched_entries().count(), 0);
        assert!(
            block
                .runtime_migrations
                .iter()
                .all(|(_, record)| record.status == MigrationStatusV1::Running
                    && record.cursor.is_empty()
                    && record.rows == 0
                    && record.stages == 0)
        );
        drop(block);
        assert_eq!(observe(&world), original);
        assert!(
            refusal.decode_refusal().is_some(),
            "the owner survives rollback"
        );

        run_block(&world, 10, |_| {});
        assert_eq!(record(&world).rows, 2);
        assert_eq!(record(&world).status, MigrationStatusV1::Running);
        let (old, new) = rows(&world);
        assert_eq!(new, copied(&old[..2]));
    });
}

#[test]
fn barrier_refusal_rolls_back_its_successor_and_retries_the_same_original_journal() {
    with_program(|| {
        let world = world();
        let accounts = accounts();
        schedule_step(&world, MigrationStep::Start, 10);
        run_block(&world, 10, |_| {});
        let original = observe(&world);
        let mut block = world.block();
        run_stages_for_testing(&mut block, 11).unwrap();
        block.tx_sequences.insert(accounts[1].clone(), 43);
        let key = successor(&accounts[1], 0).unwrap().0;
        assert_eq!(block.viral_bonus_paid.get(&key), Some(&false));
        let completed = COMPLETED_CALLBACKS.with(Cell::get);
        let successor_observation = block.viral_bonus_paid.write_observation();
        let successor_journal = block
            .viral_bonus_paid
            .touched_entries()
            .map(|entry| (*entry.key, entry.before.copied(), entry.after.copied()))
            .collect::<Vec<_>>();
        let refusal = assert_original(with_probe_refusal(|| barrier(&mut block, 11)).unwrap_err());
        assert_eq!(COMPLETED_CALLBACKS.with(Cell::get), completed + 1);
        assert_eq!(block.viral_bonus_paid.get(&key), Some(&false));
        assert_eq!(block.tx_sequences.get(&accounts[1]), Some(&43));
        assert_eq!(
            block.viral_bonus_paid.write_observation(),
            successor_observation
        );
        assert_eq!(
            block
                .viral_bonus_paid
                .touched_entries()
                .map(|entry| (*entry.key, entry.before.copied(), entry.after.copied()))
                .collect::<Vec<_>>(),
            successor_journal
        );
        // No new source transaction, clone or replay: retry the exact retained source journal.
        barrier(&mut block, 11).unwrap();
        assert_eq!(block.viral_bonus_paid.get(&key), Some(&true));
        drop(block);
        assert_eq!(observe(&world), original);
        assert!(refusal.decode_refusal().is_some());

        run_block(&world, 11, |block| {
            block.tx_sequences.insert(accounts[1].clone(), 43);
        });
        assert_eq!(record(&world).rows, 4);
        let (old, new) = rows(&world);
        assert_eq!(new, copied(&old[..4]));
    });
}

#[test]
fn drain_refusal_after_removal_restores_old_rows_and_cutover_status() {
    with_program(|| {
        let world = world();
        schedule_step(&world, MigrationStep::Start, 10);
        for height in 10..=12 {
            run_block(&world, height, |_| {});
        }
        schedule_step(&world, MigrationStep::CutOver, 13);
        run_block(&world, 13, |_| {});
        let original = observe(&world);
        let mut block = world.block();
        let completed = COMPLETED_CALLBACKS.with(Cell::get);
        let source_observation = block.tx_sequences.write_observation();
        let refusal = assert_original(
            with_probe_refusal(|| run_stages_for_testing(&mut block, 14)).unwrap_err(),
        );
        assert_eq!(COMPLETED_CALLBACKS.with(Cell::get), completed + 1);
        assert_eq!(block.tx_sequences.iter().count(), 5);
        assert_eq!(block.tx_sequences.write_observation(), source_observation);
        assert_eq!(block.tx_sequences.touched_entries().count(), 0);
        assert_eq!(
            block.runtime_migrations.iter().next().unwrap().1,
            &original.records[0].1
        );
        drop(block);
        assert_eq!(observe(&world), original);
        assert!(refusal.decode_refusal().is_some());

        run_block(&world, 14, |_| {});
        assert_eq!(rows(&world).0.len(), 3);
        assert_eq!(record(&world).status, MigrationStatusV1::CutOver(13));
        assert_eq!(record(&world).stages, original.records[0].1.stages + 1);
    });
}

fn rejected_stage(
    storage: StageStorage<'_, '_, PointAndScan, AccountId, u64, Hash, bool>,
    cursor: &[u8],
    limit: u32,
) -> MigrationResult<StageOutcome> {
    let outcome = test_migrations::stage(storage, cursor, limit)?;
    assert_eq!(
        outcome.rows, 2,
        "the rejection follows a successful copying prefix"
    );
    Err(ExecutionAttemptError::Rejected(
        "fixture rejection after writes".to_owned(),
    ))
}
static REJECTED_PROGRAM: TableMigration<PointAndScan, AccountId, u64, Hash, bool> = SEQUENCE_TABLES
    .migration(
        rejected_stage,
        test_migrations::barrier,
        test_migrations::drain,
        stale,
    );

#[test]
fn deterministic_stage_rejection_discards_successful_prefix_before_recording_faulted() {
    test_catalog::with_release(
        &[MIGRATED, RUNTIME_LIFECYCLE],
        &[MigrationAddition {
            code: &REJECTED_PROGRAM,
            ..SEQUENCES
        }],
        || {
            let world = world();
            let original_source = rows(&world).0;
            schedule_step(&world, MigrationStep::Start, 10);
            run_block(&world, 10, |_| {});
            let record = record(&world);
            assert_eq!(record.status, MigrationStatusV1::Faulted(10));
            assert!(record.cursor.is_empty());
            assert_eq!((record.rows, record.stages), (0, 0));
            assert_eq!(rows(&world), (original_source, Vec::new()));
        },
    );
}

fn cursor_refused_stage(
    storage: StageStorage<'_, '_, PointAndScan, AccountId, u64, Hash, bool>,
    cursor: &[u8],
    limit: u32,
) -> MigrationResult<StageOutcome> {
    if cursor.is_empty() {
        return test_migrations::stage(storage, cursor, limit);
    }
    // Limit only the actual cursor decoder, after original World-child acquisition.
    norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || test_migrations::stage(storage, cursor, limit),
    )
}
static CURSOR_PROGRAM: TableMigration<PointAndScan, AccountId, u64, Hash, bool> = SEQUENCE_TABLES
    .migration(
        cursor_refused_stage,
        test_migrations::barrier,
        test_migrations::drain,
        stale,
    );

#[test]
fn actual_cursor_decode_refusal_is_not_a_faulted_migration() {
    test_catalog::with_release(
        &[MIGRATED, RUNTIME_LIFECYCLE],
        &[MigrationAddition {
            code: &CURSOR_PROGRAM,
            ..SEQUENCES
        }],
        || {
            let world = world();
            schedule_step(&world, MigrationStep::Start, 10);
            run_block(&world, 10, |_| {});
            let original = observe(&world);
            let mut block = world.block();
            let result = run_stages_for_testing(&mut block, 11);
            let Err(MigrationBoundaryError::Execution(ExecutionAttemptError::Deferred(refusal))) =
                result
            else {
                panic!("expected original cursor decoder refusal");
            };
            assert!(refusal.decode_refusal().is_some());
            assert_eq!(
                block.runtime_migrations.iter().next().unwrap().1,
                &original.records[0].1
            );
            drop(block);
            assert_eq!(observe(&world), original);
        },
    );
}
