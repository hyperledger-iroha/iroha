//! Actual State publication retains a completed tail across original barrier refusals.
//! Test-only catalog composition is an engine fixture, never governance qualification.

use super::*;
use crate::{
    execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error},
    runtime::{
        catalog::{
            MigrationStep,
            additions::{RUNTIME_LIFECYCLE, test_additions::MIGRATED},
            migrations::{MigrationAddition, test_migrations},
            test_catalog,
        },
        change_set::PublishedDefinitions,
        migration::{
            MigrationResult,
            storage::{BarrierStorage, PointAndScan, SEQUENCE_TABLES, TableMigration},
        },
    },
    state::acquisition_fixture_tests::{direct_commit_musubi_scratch_tests::staged_block, fixture},
};
use iroha_data_model::runtime::obligation::{MigrationRecordV1, MigrationStatusV1};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    time::Duration,
};

#[derive(Clone, Copy)]
enum Refusal {
    None,
    Codec,
    Pool,
}
thread_local! {
    static REFUSAL: Cell<Refusal> = const { Cell::new(Refusal::None) };
    static CALLBACKS: Cell<usize> = const { Cell::new(0) };
    static TAILS: Cell<usize> = const { Cell::new(0) };
    static POOL: RefCell<Option<AllocationBudget>> = const { RefCell::new(None) };
    static BLOCKER: RefCell<Option<iroha_allocation::AllocationReservation>> = const { RefCell::new(None) };
    static ORIGINAL_CODEC: RefCell<Option<norito::core::DecodeLocalRefusal>> = const { RefCell::new(None) };
    static ORIGINAL_POOL: RefCell<Option<iroha_allocation::AllocationRefusal>> = const { RefCell::new(None) };
}
pub(super) fn observe_tail() {
    TAILS.with(|count| count.set(count.get() + 1));
}

fn barrier(
    storage: BarrierStorage<'_, '_, '_, AccountId, u64, Hash, bool>,
    cursor: &[u8],
) -> MigrationResult<()> {
    test_migrations::barrier(storage, cursor)?;
    CALLBACKS.with(|count| count.set(count.get() + 1));
    match REFUSAL.with(Cell::get) {
        Refusal::None => Ok(()),
        Refusal::Codec => {
            let source = HashMap::from([(3_u64, 7_u64), (1, 9)]);
            norito::with_decode_limits_scope(
                norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
                || norito::core::classify_decode_attempt(|| norito::encode_canonical(&source)),
            )
            .map(|_| ())
            .map_err(|original| {
                ORIGINAL_CODEC.with(|slot| *slot.borrow_mut() = original.local_refusal());
                canonical_decode_attempt_error(original, |error| error.to_string())
            })
        }
        Refusal::Pool => POOL.with(|slot| {
            let pool = slot.borrow();
            let pool = pool.as_ref().expect("actual State execution pool");
            let remaining = pool.limit_bytes() - pool.reserved_bytes();
            let blocker = pool.try_reserve_bytes(remaining).unwrap();
            BLOCKER.with(|slot| *slot.borrow_mut() = Some(blocker));
            let original = pool
                .try_reserve(std::alloc::Layout::new::<u64>())
                .unwrap_err();
            ORIGINAL_POOL.with(|slot| *slot.borrow_mut() = Some(original.clone()));
            Err(ExecutionAttemptError::Deferred(original.into()))
        }),
    }
}
fn stale(
    source: crate::runtime::migration::storage::SourceChanges<'_, '_, AccountId, u64>,
) -> bool {
    source.touched_entries().any(|entry| entry.after.is_some())
}
static PROGRAM: TableMigration<PointAndScan, AccountId, u64, Hash, bool> = SEQUENCE_TABLES
    .migration(
        test_migrations::stage,
        barrier,
        test_migrations::drain,
        stale,
    );

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            test_catalog::with_release(
                &[MIGRATED, RUNTIME_LIFECYCLE],
                &[MigrationAddition {
                    code: &PROGRAM,
                    ..test_migrations::SEQUENCES
                }],
                test,
            )
        })
        .unwrap()
        .join()
        .unwrap_or_else(|error| std::panic::resume_unwind(error));
}

fn prepared_fixture() -> (Box<State>, SignedBlock, AccountId) {
    let (state, proposal) = fixture();
    crate::runtime::activate_additions_for_testing(&state.world, RUNTIME_LIFECYCLE.module).unwrap();
    let catalog = crate::runtime::release_catalog().unwrap();
    let account = iroha_test_samples::ALICE_ID.clone();
    let mut world = state.world.block();
    let from = catalog.world_storage[test_migrations::FROM];
    world
        .runtime_definitions
        .insert(from, catalog.definitions[&from].clone());
    if !world
        .runtime_selection
        .get()
        .installed_storage
        .contains(&from)
    {
        world
            .runtime_selection
            .get_mut()
            .installed_storage
            .push(from);
        world
            .runtime_selection
            .get_mut()
            .installed_storage
            .sort_unstable();
    }
    let change = catalog
        .migration_step(
            test_migrations::MODULE,
            MigrationStep::Start,
            world.runtime_selection.get(),
            &PublishedDefinitions(&world.runtime_definitions),
            *state.network_id_ref(),
            1,
            [0x71; 32],
        )
        .unwrap();
    let applied = change
        .apply(
            world.runtime_selection.get(),
            &PublishedDefinitions(&world.runtime_definitions),
        )
        .unwrap();
    for (id, descriptor) in applied.published {
        world.runtime_definitions.insert(id, descriptor);
    }
    *world.runtime_selection.get_mut() = applied.selection;
    let id = *world
        .runtime_selection
        .get()
        .active
        .iter()
        .find(|id| catalog.migration(id).is_some())
        .unwrap();
    let mut record = MigrationRecordV1::started(1);
    record.cursor = norito::encode_canonical(&account).unwrap();
    record.rows = 1;
    record.stages = 1;
    record.status = MigrationStatusV1::Complete(1);
    world.runtime_migrations.insert(id, record);
    world.tx_sequences.insert(account.clone(), 2);
    let (key, value) = test_migrations::successor(&account, 2).unwrap();
    world.viral_bonus_paid.insert(key, value);
    world.commit();
    (state, proposal, account)
}

fn visible(state: &State) -> (String, String, String, u64, usize) {
    (
        norito::json::to_json(&state.world.tx_sequences).unwrap(),
        norito::json::to_json(&state.world.viral_bonus_paid).unwrap(),
        norito::json::to_json(&state.world.runtime_migrations).unwrap(),
        state.state_view_generation(),
        state.transactions.latest_height(),
    )
}
fn writers_free(state: &State) {
    assert!(state.state_write_lock.try_lock().is_some());
    assert!(state.state_commit_lock.try_lock().is_some());
    assert!(state.lane_lifecycle_lock.try_lock().is_some());
    // Actual complete census acquisition catches a retained writer anywhere in World.
    drop(
        state
            .world
            .try_block(&state.ivm_execution_budget())
            .unwrap(),
    );
    for probe in state
        .canonical_runtime
        .probe_original_writers_for_testing()
        .into_iter()
        .chain(
            state
                .native_execution_tip
                .probe_original_writers_for_testing(),
        )
        .chain(state.commit_topology.probe_original_writers_for_testing())
        .chain(
            state
                .prev_commit_topology
                .probe_original_writers_for_testing(),
        )
    {
        assert!(probe.acquired && !probe.poisoned);
    }
}
fn defer(original: &mut StateBlock<'_>, kind: Refusal) {
    REFUSAL.with(|slot| slot.set(kind));
    let outcome = original.try_publish();
    REFUSAL.with(|slot| slot.set(Refusal::None));
    let StatePublicationOutcome::Deferred(TransactionsBlockError::ExecutionDeferred(reason)) =
        outcome
    else {
        panic!("original barrier must defer without publication: {outcome:?}");
    };
    match kind {
        Refusal::Codec => assert_eq!(
            reason.decode_refusal(),
            ORIGINAL_CODEC
                .with(|slot| slot.borrow_mut().take())
                .as_ref()
        ),
        Refusal::Pool => assert_eq!(
            reason.allocation_refusal(),
            ORIGINAL_POOL.with(|slot| slot.borrow_mut().take()).as_ref()
        ),
        Refusal::None => unreachable!(),
    }
    BLOCKER.with(|slot| drop(slot.borrow_mut().take()));
}

#[test]
fn original_barrier_codec_and_pool_refusals_release_writers_then_publish_once() {
    run(|| {
        for kind in [Refusal::Codec, Refusal::Pool] {
            let (state, proposal, account) = prepared_fixture();
            let before = visible(&state);
            let mut original = Box::new(staged_block(&state, proposal.header(), false, false));
            original.world.tx_sequences.insert(account.clone(), 9);
            let identity = std::ptr::from_ref(&*original);
            POOL.with(|slot| *slot.borrow_mut() = Some(state.ivm_execution_budget()));
            let tails = TAILS.with(Cell::get);
            let calls = CALLBACKS.with(Cell::get);
            defer(&mut original, kind);
            assert_eq!(visible(&state), before);
            writers_free(&state);
            assert_eq!(TAILS.with(Cell::get), tails + 1);
            assert_eq!(CALLBACKS.with(Cell::get), calls + 1);
            assert!(original.publication.as_ref().unwrap().world_suspended);
            assert!(
                original
                    .publication
                    .as_ref()
                    .unwrap()
                    .world_effects
                    .is_some()
            );
            let rejected_mutation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                original.world.tx_sequences.insert(account.clone(), 77);
            }));
            assert!(
                rejected_mutation.is_err(),
                "pending State never exposes mutation authority"
            );
            assert!(matches!(
                original.try_publish(),
                StatePublicationOutcome::Published
            ));
            assert_eq!(std::ptr::from_ref(&*original), identity);
            assert_eq!(TAILS.with(Cell::get), tails + 1);
            assert_eq!(CALLBACKS.with(Cell::get), calls + 2);
            assert_eq!(state.world.tx_sequences.view().get(&account), Some(&9));
            let (key, value) = test_migrations::successor(&account, 9).unwrap();
            assert_eq!(state.world.viral_bonus_paid.view().get(&key), Some(&value));
            assert!(matches!(
                original.try_publish(),
                StatePublicationOutcome::Published
            ));
            assert_eq!(TAILS.with(Cell::get), tails + 1);
            assert_eq!(CALLBACKS.with(Cell::get), calls + 2);
            POOL.with(|slot| drop(slot.borrow_mut().take()));
            let (clean_state, clean_proposal, clean_account) = prepared_fixture();
            let mut clean = staged_block(&clean_state, clean_proposal.header(), false, false);
            clean.world.tx_sequences.insert(clean_account, 9);
            assert!(matches!(
                clean.try_publish(),
                StatePublicationOutcome::Published
            ));
            assert_eq!(
                visible(&state),
                visible(&clean_state),
                "retained retry equals the clean publication"
            );
        }
    });
}

#[test]
fn suspended_original_reacquisition_contention_rolls_back_partial_writers_before_retry() {
    run(|| {
        let (state, proposal, account) = prepared_fixture();
        let before = visible(&state);
        let mut original = staged_block(&state, proposal.header(), false, false);
        original.world.tx_sequences.insert(account, 9);
        defer(&mut original, Refusal::Codec);
        let calls = CALLBACKS.with(Cell::get);
        let tails = TAILS.with(Cell::get);
        std::thread::scope(|scope| {
            let (ready, rx) = std::sync::mpsc::channel();
            let (release, wait) = std::sync::mpsc::channel();
            let state = &*state;
            let worker = scope.spawn(move || {
                // An actual late World cell, held after the original suspended.
                let held = state.world.soradns_last_publish_ms.block();
                ready.send(()).unwrap();
                let released = wait.recv_timeout(Duration::from_secs(5)).is_ok();
                drop(held);
                released
            });
            rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(matches!(
                original.try_publish(),
                StatePublicationOutcome::Deferred(TransactionsBlockError::PublicationBusy(_))
            ));
            assert_eq!(CALLBACKS.with(Cell::get), calls);
            assert_eq!(TAILS.with(Cell::get), tails);
            assert_eq!(visible(state), before);
            release.send(()).unwrap();
            assert!(
                worker.join().unwrap(),
                "retry must return while the original contender remains held"
            );
        });
        writers_free(&state);
        assert!(matches!(
            original.try_publish(),
            StatePublicationOutcome::Published
        ));
    });
}

#[test]
fn dropped_barrier_owner_discards_original_private_effects_and_all_writers() {
    run(|| {
        let (state, proposal, account) = prepared_fixture();
        let before = visible(&state);
        let mut original = staged_block(&state, proposal.header(), false, false);
        original.world.tx_sequences.insert(account, 9);
        defer(&mut original, Refusal::Codec);
        drop(original);
        assert_eq!(visible(&state), before);
        writers_free(&state);
    });
}

#[test]
fn changed_original_predecessor_requires_recovery_without_repeating_barrier() {
    run(|| {
        let (state, proposal, account) = prepared_fixture();
        let mut original = staged_block(&state, proposal.header(), false, false);
        original.world.tx_sequences.insert(account, 9);
        defer(&mut original, Refusal::Codec);
        let calls = CALLBACKS.with(Cell::get);
        // The values are unchanged, but this ordinary exact field owner really publishes.
        state.world.soradns_last_publish_ms.block().commit();
        let after_competitor = visible(&state);
        assert!(matches!(
            original.try_publish(),
            StatePublicationOutcome::RecoveryRequired(
                TransactionsBlockError::SnapshotObservationChanged
            )
        ));
        assert_eq!(visible(&state), after_competitor);
        assert_eq!(CALLBACKS.with(Cell::get), calls);
        writers_free(&state);
    });
}

#[test]
fn completed_barrier_prefix_never_repeats_a_successful_callback() {
    run(|| {
        let (state, proposal, account) = prepared_fixture();
        let mut original = staged_block(&state, proposal.header(), false, false);
        original.world.tx_sequences.insert(account, 9);
        let mut progress = crate::runtime::migration::BarrierProgress::default();
        let before = CALLBACKS.with(Cell::get);
        crate::runtime::migration::resume_barrier(&mut original.world, 1, &mut progress).unwrap();
        assert_eq!(CALLBACKS.with(Cell::get), before + 1);
        REFUSAL.with(|slot| slot.set(Refusal::Codec));
        let result =
            crate::runtime::migration::resume_barrier(&mut original.world, 1, &mut progress);
        REFUSAL.with(|slot| slot.set(Refusal::None));
        result.unwrap();
        assert_eq!(CALLBACKS.with(Cell::get), before + 1);
    });
}
