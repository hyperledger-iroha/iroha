//! Actual original-pool worklist and callback scratch custody, including partial refusal.

use super::*;
use crate::execution_attempt::ExecutionAttemptError;
use crate::runtime::migration::{
    scratch::{BarrierEntry, StageEntry, barrier_census, stage_census},
    storage::{
        BarrierStorage, MigrationProgram as _, PointAndScan, SEQUENCE_TABLES, SourceChanges,
        SourceDrain, StageStorage, TableMigration,
    },
};
use iroha_allocation::{AllocationBudget, AllocationRefusal};
use std::{alloc::Layout, cell::RefCell};

fn with_fixture(test: impl FnOnce()) {
    test_catalog::with_release(&[MIGRATED, RUNTIME_LIFECYCLE], &[SEQUENCES], test);
}
fn started_world() -> World {
    let world = world();
    schedule_step(&world, MigrationStep::Start, 10);
    run_block(&world, 10, |_| {});
    world
}
fn capacity_refusal(error: MigrationBoundaryError, pool: &AllocationBudget) {
    let MigrationBoundaryError::Execution(ExecutionAttemptError::Deferred(original)) = error else {
        panic!("original capacity refusal must remain local");
    };
    let Some(AllocationRefusal::Capacity { release, .. }) = original.allocation_refusal() else {
        panic!("actual exhausted pool must refuse capacity");
    };
    assert!(pool.is_source_of(release));
}

#[test]
fn census_arrays_and_original_barrier_cursor_bytes_retain_exact_charges() {
    with_fixture(|| {
        let world = started_world();
        let mut block = world.block();
        let pool = block.sccp_committee_generations.allocation_budget().clone();
        let (id, original) = block.runtime_migrations.iter().next().unwrap();
        let (id, original) = (*id, original.clone());
        let before = pool.reserved_bytes();
        let stage = stage_census(&block, 11).unwrap();
        assert_eq!(stage.as_slice().len(), 1);
        assert_eq!(stage.as_slice()[0].id, id);
        assert!(stage.belongs_to(&pool));
        let stage_bytes = Layout::array::<StageEntry>(1).unwrap().size();
        assert_eq!(pool.reserved_bytes(), before + stage_bytes);
        let barrier = barrier_census(&block, None).unwrap();
        let barrier_bytes =
            Layout::array::<BarrierEntry>(1).unwrap().size() + original.cursor.len();
        assert_eq!(pool.reserved_bytes(), before + stage_bytes + barrier_bytes);
        let entry = &barrier.entries()[0];
        assert_eq!(entry.id, id);
        assert_eq!(entry.status, original.status);
        assert_eq!(barrier.cursor(entry), original.cursor);
        block
            .runtime_migrations
            .get_mut(&id)
            .unwrap()
            .cursor
            .clear();
        assert_eq!(barrier.cursor(entry), original.cursor);
        drop(barrier);
        assert_eq!(pool.reserved_bytes(), before + stage_bytes);
        drop(stage);
        assert_eq!(pool.reserved_bytes(), before);
    });
}

#[test]
fn full_original_pool_refuses_stage_census_before_child_or_record_changes() {
    with_fixture(|| {
        let world = started_world();
        let mut block = world.block();
        let pool = block.sccp_committee_generations.allocation_budget().clone();
        let before = block.runtime_migrations.iter().next().unwrap().1.clone();
        let observations = (
            block.tx_sequences.write_observation(),
            block.viral_bonus_paid.write_observation(),
        );
        let occupied = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
            .unwrap();
        let mut error = None;
        let allocations = crate::test_allocations::allocations_during(|| {
            error = Some(
                stage_census(&block, 11)
                    .err()
                    .expect("actual census admission"),
            );
        });
        assert_eq!(allocations, 0);
        capacity_refusal(error.unwrap().into(), &pool);
        capacity_refusal(run_stages_for_testing(&mut block, 11).unwrap_err(), &pool);
        assert_eq!(block.runtime_migrations.iter().next().unwrap().1, &before);
        assert_eq!(
            (
                block.tx_sequences.write_observation(),
                block.viral_bonus_paid.write_observation()
            ),
            observations
        );
        drop(occupied);
        run_stages_for_testing(&mut block, 11).unwrap();
        assert_eq!(
            block.runtime_migrations.iter().next().unwrap().1.rows,
            before.rows + 2
        );
    });
}

#[test]
fn arena_refusal_refunds_admitted_entries_and_same_barrier_journal_retries() {
    with_fixture(|| {
        let world = started_world();
        let mut block = world.block();
        let account = accounts()[1].clone();
        block.tx_sequences.insert(account.clone(), 9);
        let (key, expected) = successor(&account, 9).unwrap();
        assert_ne!(block.viral_bonus_paid.get(&key), Some(&expected));
        let pool = block.sccp_committee_generations.allocation_budget().clone();
        let record = block.runtime_migrations.iter().next().unwrap().1.clone();
        assert!(!record.cursor.is_empty());
        let writes = block.viral_bonus_paid.write_observation();
        let available = pool.limit_bytes() - pool.reserved_bytes();
        let entry_bytes = Layout::array::<BarrierEntry>(1).unwrap().size();
        let occupied = pool.try_reserve_bytes(available - entry_bytes).unwrap();
        let before = pool.reserved_bytes();
        let mut progress = BarrierProgress::default();
        capacity_refusal(
            resume_barrier(&mut block, 11, &mut progress).unwrap_err(),
            &pool,
        );
        assert_eq!(pool.reserved_bytes(), before);
        assert!(progress.completed_through.is_none());
        assert_eq!(block.viral_bonus_paid.write_observation(), writes);
        assert_eq!(block.runtime_migrations.iter().next().unwrap().1, &record);
        assert_eq!(block.tx_sequences.get(&account), Some(&9));
        drop(occupied);
        resume_barrier(&mut block, 11, &mut progress).unwrap();
        assert_eq!(block.viral_bonus_paid.get(&key), Some(&expected));
        let completed = progress.completed_through;
        let occupied = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
            .unwrap();
        resume_barrier(&mut block, 11, &mut progress).unwrap();
        assert_eq!(progress.completed_through, completed);
        drop(occupied);
    });
}

thread_local! {
    static ORIGINAL_POOL: RefCell<Option<AllocationBudget>> = const { RefCell::new(None) };
}
fn scratch_stage(
    storage: StageStorage<'_, '_, PointAndScan, AccountId, u64, Hash, bool>,
    _: &[u8],
    _: u32,
) -> MigrationResult<crate::runtime::catalog::migrations::StageOutcome> {
    let mut scratch = storage.scratch.buffer::<u64>(3)?;
    ORIGINAL_POOL.with(|pool| assert!(scratch.belongs_to(pool.borrow().as_ref().unwrap())));
    scratch.push_reserved(41);
    assert_eq!(scratch.as_slice(), [41]);
    Ok(crate::runtime::catalog::migrations::StageOutcome {
        cursor: Vec::new(),
        rows: 0,
        done: true,
    })
}
fn scratch_barrier(
    storage: BarrierStorage<'_, '_, '_, AccountId, u64, Hash, bool>,
    _: &[u8],
) -> MigrationResult<()> {
    let scratch = storage.scratch.buffer::<u64>(2)?;
    ORIGINAL_POOL.with(|pool| assert!(scratch.belongs_to(pool.borrow().as_ref().unwrap())));
    Ok(())
}
fn scratch_drain(
    source: SourceDrain<'_, '_, AccountId, u64>,
    _: u32,
) -> MigrationResult<crate::runtime::catalog::migrations::DrainOutcome> {
    let scratch = source.scratch().buffer::<u64>(1)?;
    ORIGINAL_POOL.with(|pool| assert!(scratch.belongs_to(pool.borrow().as_ref().unwrap())));
    Ok(crate::runtime::catalog::migrations::DrainOutcome {
        rows: 0,
        empty: source.is_empty(),
    })
}
fn stale(_: SourceChanges<'_, '_, AccountId, u64>) -> bool {
    false
}
static SCRATCH_PROGRAM: TableMigration<PointAndScan, AccountId, u64, Hash, bool> =
    SEQUENCE_TABLES.migration(scratch_stage, scratch_barrier, scratch_drain, stale);

#[test]
fn adapters_issue_original_pool_scratch_and_preserve_real_refusal() {
    with_fixture(|| {
        let world = started_world();
        let mut block = world.block();
        let pool = block.sccp_committee_generations.allocation_budget().clone();
        ORIGINAL_POOL.with(|slot| *slot.borrow_mut() = Some(pool.clone()));
        let mut child = block.transaction_without_telemetry(Default::default(), 0);
        let before = pool.reserved_bytes();
        SCRATCH_PROGRAM.stage(&mut child, &[], 1).unwrap();
        SCRATCH_PROGRAM.drain(&mut child, 1).unwrap();
        assert_eq!(pool.reserved_bytes(), before);
        let occupied = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
            .unwrap();
        capacity_refusal(
            SCRATCH_PROGRAM
                .stage(&mut child, &[], 1)
                .unwrap_err()
                .into(),
            &pool,
        );
        capacity_refusal(
            SCRATCH_PROGRAM.drain(&mut child, 1).unwrap_err().into(),
            &pool,
        );
        drop(occupied);
        drop(child);
        let before = pool.reserved_bytes();
        SCRATCH_PROGRAM.barrier(&mut block, &[]).unwrap();
        assert_eq!(pool.reserved_bytes(), before);
        let occupied = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
            .unwrap();
        capacity_refusal(SCRATCH_PROGRAM.barrier(&mut block, &[]).unwrap_err(), &pool);
        drop(occupied);
        ORIGINAL_POOL.with(|slot| drop(slot.borrow_mut().take()));
    });
}

#[test]
fn actual_worklist_allocator_refusal_retains_its_original_cause_and_refunds_credit() {
    with_fixture(|| {
        let world = started_world();
        let block = world.block();
        let pool = block.sccp_committee_generations.allocation_budget();
        let before = pool.reserved_bytes();
        let layout = Layout::array::<StageEntry>(1).unwrap();
        let (attempt, refused) =
            crate::test_allocations::refuse_one_layout_during(layout, || stage_census(&block, 11));
        assert!(refused, "the exact physical backing allocation was reached");
        let ExecutionAttemptError::Deferred(original) = attempt.err().expect("physical refusal")
        else {
            panic!("an allocator refusal must never become a migration verdict");
        };
        assert!(original.allocation_refusal().is_none());
        assert!(matches!(
            original.allocator_refusal().unwrap().buffer_error(),
            Some(iroha_allocation::ChargedBufferError::Allocator { requested_bytes })
                if *requested_bytes == layout.size()
        ));
        assert_eq!(pool.reserved_bytes(), before);
        let admitted = stage_census(&block, 11).unwrap();
        assert_eq!(admitted.as_slice().len(), 1);
        assert!(admitted.belongs_to(pool));
        drop(admitted);
        assert_eq!(pool.reserved_bytes(), before);
    });
}
