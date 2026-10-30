//! Actual World transaction reads retain their native bank across refusal and rollback.
use super::*;
use crate::smartcontracts::isi::sccp::test_support::{authority, blank_state, header};
use iroha_allocation::{AllocationRefusal, release::ReleaseRegistration};
use std::task::{Context, Poll, Waker};

fn value(seed: u8) -> AssetUidBindingV1 {
    AssetUidBindingV1 {
        asset_definition_id: iroha_data_model::sccp::escrow::sccp_taira_xor_asset_definition_id(),
        account_id: authority(seed),
        uid: [seed; 32],
    }
}
fn read(
    tx: &StateTransaction<'_, '_>,
    value: &AssetUidBindingV1,
) -> Result<Option<[u8; 32]>, Error> {
    binding_uid_with_owner(
        &*tx.world,
        &value.asset_definition_id,
        &value.account_id,
        Some(&tx.world),
    )
}
fn poll(
    registration: &mut ReleaseRegistration,
    refusal: &crate::execution_attempt::ExecutionDeferred,
) -> Poll<()> {
    let Some(AllocationRefusal::Capacity { release, .. }) = refusal.allocation_refusal() else {
        panic!("original finite-pool refusal")
    };
    registration.poll_wait(release, &mut Context::from_waker(Waker::noop()))
}

#[test]
fn native_binding_transaction_retains_controls_source_and_refusal_until_external_release() {
    let _retirement = crossbeam_epoch::pin();
    let mut state = blank_state();
    let value = value(1);
    let path = binding_key(&value.asset_definition_id, &value.account_id);
    let bytes = encode(&value).unwrap();
    state
        .world
        .smart_contract_state
        .insert(path.clone(), bytes.clone());
    let mut block = state.block(header(2));
    let pool = block
        .world
        .sccp_committee_generations
        .allocation_budget()
        .clone();
    let original = BindingPreparations::try_new(&pool).unwrap();
    block.world.uid_binding_run = Some(original.run(&pool).unwrap());
    let mut tx = block.transaction();
    let watch = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut watch_credit = watch
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut watch_credit).unwrap();
    registration.bind_thread(std::thread::current());
    let controls: usize = norito::core::PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum();
    let available = path.as_ref().len() + bytes.len() + controls;
    let held = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes() - available)
        .unwrap();
    assert!(read(&tx, &value).is_err());
    let refusal = tx.take_execution_deferral().unwrap();
    let key = value
        .account_id
        .try_signatory()
        .unwrap()
        .retained_allocation_layout();
    assert!(
        matches!(refusal.allocation_refusal(), Some(AllocationRefusal::Capacity { requested_bytes, .. }) if *requested_bytes == key.size())
    );
    assert_eq!(
        poll(&mut registration, &refusal),
        Poll::Pending,
        "a native binding refusal must retain its own controls and raw source until external release"
    );
    let first = {
        let bank = original.0.lock().unwrap();
        let entry = bank.current.as_ref().unwrap();
        (
            entry.row.as_ref().unwrap().as_slice().as_ptr(),
            entry.path.as_ref().unwrap().as_slice().as_ptr(),
        )
    };
    // The actual publication guard abandons this transaction; the original bank
    // remains owned outside its World overlay and is lent to the next pass.
    tx.apply();
    assert_eq!(
        poll(&mut registration, &refusal),
        Poll::Pending,
        "the original pre-rollback refusal cannot be woken by binding-owned refunds"
    );
    assert!(matches!(
        block.execution_output_plan,
        Some(crate::state::output_capacity::ExecutionOutputPlanState::Poisoned)
    ));
    assert_eq!(block.world.smart_contract_state.get(&path), Some(&bytes));
    // Keep the actual poison guard. Whole-World refunds are measured by the
    // Worker diagnostic; this retry opens fresh State after external release.
    drop(block);
    drop(held);
    assert_eq!(poll(&mut registration, &refusal), Poll::Ready(()));
    drop(refusal);
    let mut block = state.block(header(2));
    block.world.uid_binding_run = Some(original.run(&pool).unwrap());
    let tx = block.transaction();
    let control = norito::core::PreparedDecodeWorkspace::allocation_layouts()[0];
    let (result, replaced) =
        crate::test_allocations::refuse_layout_occurrence_during(control, 0, || read(&tx, &value));
    assert!(
        !replaced,
        "retry must reuse the original physical decoder controls"
    );
    assert_eq!(result.unwrap(), Some(value.uid));
    assert!(!tx.is_execution_deferred());
    assert_eq!(tx.world.smart_contract_state.get(&path), Some(&bytes));
    let original_bytes = {
        let bank = original.0.lock().unwrap();
        let entry = bank.current.as_ref().unwrap();
        assert_eq!(
            (
                entry.row.as_ref().unwrap().as_slice().as_ptr(),
                entry.path.as_ref().unwrap().as_slice().as_ptr()
            ),
            first,
            "fresh World retry must preserve the original captured source allocations"
        );
        ChargedShared::<BankLock>::allocation_layout().size()
            + path.as_ref().len()
            + bytes.len()
            + entry.complete.as_ref().unwrap().allocation_bytes().unwrap()
    };
    drop(tx);
    block.world.uid_binding_run = None;
    let before_cancellation = pool.reserved_bytes();
    drop(original);
    assert_eq!(
        pool.reserved_bytes(),
        before_cancellation - original_bytes,
        "cancellation retires every original binding allocation"
    );
}

#[test]
fn native_binding_bank_preserves_earlier_occurrences_and_rejects_changed_rows_and_foreign_pools() {
    let _retirement = crossbeam_epoch::pin();
    let state = blank_state();
    let mut block = state.block(header(2));
    let first = value(1);
    let second = value(2);
    for value in [&first, &second] {
        block.world.smart_contract_state.insert(
            binding_key(&value.asset_definition_id, &value.account_id),
            encode(value).unwrap(),
        );
    }
    let pool = block
        .world
        .sccp_committee_generations
        .allocation_budget()
        .clone();
    let original = BindingPreparations::try_new(&pool).unwrap();
    assert!(
        original
            .run(&AllocationBudget::new(pool.limit_bytes()))
            .is_err()
    );
    block.world.uid_binding_run = Some(original.run(&pool).unwrap());
    let tx = block.transaction();
    assert_eq!(read(&tx, &first).unwrap(), Some(first.uid));
    let held = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
        .unwrap();
    assert!(read(&tx, &second).is_err());
    drop(tx);
    block.world.uid_binding_run = Some(original.run(&pool).unwrap());
    let tx = block.transaction();
    // The preceding read reuses its actual funded graph despite a completely full pool.
    assert_eq!(read(&tx, &first).unwrap(), Some(first.uid));
    assert!(read(&tx, &second).is_err());
    drop(held);
    drop(tx);
    let path = binding_key(&first.asset_definition_id, &first.account_id);
    let mut changed = first;
    changed.uid = [8; 32];
    block
        .world
        .smart_contract_state
        .insert(path, encode(&changed).unwrap());
    block.world.uid_binding_run = Some(original.run(&pool).unwrap());
    let tx = block.transaction();
    assert!(read(&tx, &changed).is_err());
    assert_eq!(
        tx.borrow_execution_deferral().unwrap().reason(),
        ivm::error::ExecutionDeferral::LocalInvariantViolation
    );
}

#[test]
fn native_binding_header_rejection_precedes_capture_admission_and_cancellation_refunds_prefix() {
    let _retirement = crossbeam_epoch::pin();
    let state = blank_state();
    let mut block = state.block(header(2));
    let value = value(1);
    let path = binding_key(&value.asset_definition_id, &value.account_id);
    block
        .world
        .smart_contract_state
        .insert(path.clone(), vec![0; 4]);
    let pool = block
        .world
        .sccp_committee_generations
        .allocation_budget()
        .clone();
    let initial_reserved = pool.reserved_bytes();
    let original = BindingPreparations::try_new(&pool).unwrap();
    block.world.uid_binding_run = Some(original.run(&pool).unwrap());
    let tx = block.transaction();
    let held = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
        .unwrap();
    assert!(read(&tx, &value).is_err());
    assert!(!tx.is_execution_deferred());
    assert!(original.0.lock().unwrap().current.is_none());
    drop(held);
    drop(tx);
    block
        .world
        .smart_contract_state
        .insert(path.clone(), encode(&value).unwrap());
    block.world.uid_binding_run = Some(original.run(&pool).unwrap());
    let tx = block.transaction();
    let baseline = pool.reserved_bytes();
    let held = pool
        .try_reserve_bytes(pool.limit_bytes() - baseline - path.as_ref().len())
        .unwrap();
    assert!(read(&tx, &value).is_err());
    assert!(tx.is_execution_deferred());
    assert!(
        original
            .0
            .lock()
            .unwrap()
            .current
            .as_ref()
            .unwrap()
            .path
            .is_some()
    );
    assert!(
        original
            .0
            .lock()
            .unwrap()
            .current
            .as_ref()
            .unwrap()
            .row
            .is_none()
    );
    drop(tx);
    block.world.uid_binding_run = None;
    drop(original);
    drop(held);
    assert_eq!(
        pool.reserved_bytes(),
        initial_reserved,
        "cancellation refunds a source-capture prefix as well as complete graphs"
    );
}

#[test]
fn native_binding_raw_capture_allocator_refusal_preserves_original_cause_and_prefix() {
    let _retirement = crossbeam_epoch::pin();
    for fail_row in [false, true] {
        let state = blank_state();
        let mut block = state.block(header(2));
        let value = value(1);
        let path = binding_key(&value.asset_definition_id, &value.account_id);
        let bytes = encode(&value).unwrap();
        block
            .world
            .smart_contract_state
            .insert(path.clone(), bytes.clone());
        let pool = block
            .world
            .sccp_committee_generations
            .allocation_budget()
            .clone();
        let original = BindingPreparations::try_new(&pool).unwrap();
        block.world.uid_binding_run = Some(original.run(&pool).unwrap());
        let tx = block.transaction();
        // Compute the key outside fault injection: this targets only the new
        // funded path/source backing, never the preexisting StatePath constructor.
        let invocation = tx.world.uid_binding_execution.as_ref().unwrap();
        let layout = std::alloc::Layout::array::<u8>(if fail_row {
            bytes.len()
        } else {
            path.as_ref().len()
        })
        .unwrap();
        let (result, refused) =
            crate::test_allocations::refuse_layout_occurrence_during(layout, 0, || {
                invocation.read(
                    &path,
                    Some(&bytes),
                    &value.asset_definition_id,
                    &value.account_id,
                    &pool,
                )
            });
        assert!(refused);
        let Err(ExecutionAttemptError::Deferred(original_refusal)) = result else {
            panic!("original physical capture refusal");
        };
        assert!(original_refusal.allocation_refusal().is_none());
        assert!(
            matches!(original_refusal.allocator_refusal().unwrap().buffer_error(), Some(ChargedBufferError::Allocator { requested_bytes }) if *requested_bytes == layout.size())
        );
        {
            let bank = original.0.lock().unwrap();
            let entry = bank.current.as_ref().unwrap();
            assert_eq!(entry.path.is_some(), fail_row);
            assert!(entry.row.is_none());
        }
        drop(tx);
        block.world.uid_binding_run = Some(original.run(&pool).unwrap());
        let tx = block.transaction();
        assert_eq!(read(&tx, &value).unwrap(), Some(value.uid));
    }
}

#[test]
fn native_binding_finish_notifies_original_pool_only_after_bank_unlock() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering::SeqCst},
    };
    use std::task::Wake;

    struct Probe {
        original: BindingPreparations,
        wakes: AtomicUsize,
        locked: AtomicUsize,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            self.wakes.fetch_add(1, SeqCst);
            if self.original.0.try_lock().is_err() {
                self.locked.fetch_add(1, SeqCst);
            }
        }
    }

    // No enclosing World refund scope can mask the new mutex's own obligation.
    let value = value(3);
    let path = binding_key(&value.asset_definition_id, &value.account_id);
    let bytes = encode(&value).unwrap();
    let pool = AllocationBudget::new(1 << 20);
    let original = BindingPreparations::try_new(&pool).unwrap();
    let controls: usize = norito::core::PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum();
    let capture_and_controls = path.as_ref().len() + bytes.len() + controls;
    let held = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes() - capture_and_controls)
        .unwrap();
    let first = original.run(&pool).unwrap().transaction();
    let refused = first.read(
        &path,
        Some(&bytes),
        &value.asset_definition_id,
        &value.account_id,
        &pool,
    );
    assert!(matches!(refused, Err(ExecutionAttemptError::Deferred(_))));
    assert!(
        original
            .0
            .lock()
            .unwrap()
            .current
            .as_ref()
            .unwrap()
            .complete
            .is_none()
    );
    drop(refused);
    drop(held);

    // Arm after the external release, on a new real refusal from the same pool.
    // The successful retained retry's finish will return its decoder controls.
    let unavailable = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes() + 1)
        .unwrap_err();
    let AllocationRefusal::Capacity { release, .. } = unavailable else {
        panic!("finite original pool capacity");
    };
    let watch_pool = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut credit = watch_pool
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut credit).unwrap();
    let probe = Arc::new(Probe {
        original: original.clone(),
        wakes: AtomicUsize::new(0),
        locked: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&probe));
    assert_eq!(
        registration.poll_wait(&release, &mut Context::from_waker(&waker)),
        Poll::Pending
    );
    let retry = original.run(&pool).unwrap().transaction();
    assert_eq!(
        retry
            .read(
                &path,
                Some(&bytes),
                &value.asset_definition_id,
                &value.account_id,
                &pool,
            )
            .unwrap(),
        Some(value.uid)
    );
    assert!(
        original
            .0
            .lock()
            .unwrap()
            .current
            .as_ref()
            .unwrap()
            .complete
            .is_some()
    );
    assert!(
        probe.wakes.load(SeqCst) > 0,
        "actual finish must refund decoder controls"
    );
    assert_eq!(
        probe.locked.load(SeqCst),
        0,
        "binding completion callbacks must run after the original bank mutex unlocks"
    );
    assert_eq!(
        registration.poll_wait(&release, &mut Context::from_waker(&waker)),
        Poll::Ready(())
    );
}

#[test]
fn captured_binding_preparation_reuses_controls_and_requires_the_actual_state_read() {
    // Initialize process-wide deadlock-detection TLS, separately from the bank's
    // physical ownership census. The original bank has never been locked here.
    drop(parking_lot::Mutex::new(()).lock());
    let value = value(5);
    let path = binding_key(&value.asset_definition_id, &value.account_id);
    let bytes = encode(&value).unwrap();
    let pool = AllocationBudget::new(1 << 20);
    let original = BindingPreparations::try_new(&pool).unwrap();
    assert_eq!(
        crate::test_allocations::allocations_during(|| original.prepare_captured(&pool).unwrap()),
        0
    );
    let controls: usize = norito::core::PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum();
    let held = pool
        .try_reserve_bytes(
            pool.limit_bytes()
                - pool.reserved_bytes()
                - path.as_ref().len()
                - bytes.len()
                - controls,
        )
        .unwrap();
    let first = original.run(&pool).unwrap().transaction();
    let Err(ExecutionAttemptError::Deferred(_)) = first.read(
        &path,
        Some(&bytes),
        &value.asset_definition_id,
        &value.account_id,
        &pool,
    ) else {
        panic!("original account backing must refuse");
    };
    drop(first);
    let pointers = {
        let bank = original.0.lock().unwrap();
        let entry = bank.current.as_ref().unwrap();
        (
            entry.path.as_ref().unwrap().as_slice().as_ptr(),
            entry.row.as_ref().unwrap().as_slice().as_ptr(),
        )
    };
    let occupied = pool.reserved_bytes();
    let refused = original.prepare_captured(&pool).unwrap_err();
    assert_eq!(
        pool.reserved_bytes(),
        occupied,
        "preparation cannot refund its own pending prefix"
    );
    let watch = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut credit = watch
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut credit).unwrap();
    registration.bind_thread(std::thread::current());
    assert_eq!(poll(&mut registration, &refused), Poll::Pending);
    drop(held);
    let control = norito::core::PreparedDecodeWorkspace::allocation_layouts()[0];
    let (prepared, replaced) =
        crate::test_allocations::refuse_layout_occurrence_during(control, 0, || {
            original.prepare_captured(&pool)
        });
    prepared.unwrap();
    assert!(
        !replaced,
        "captured preparation must reuse its physical controls"
    );
    {
        let bank = original.0.lock().unwrap();
        let entry = bank.current.as_ref().unwrap();
        assert!(entry.complete.is_some());
        assert_eq!(
            (
                entry.path.as_ref().unwrap().as_slice().as_ptr(),
                entry.row.as_ref().unwrap().as_slice().as_ptr()
            ),
            pointers
        );
    }
    assert!(
        original
            .prepare_captured(&AllocationBudget::new(1 << 20))
            .is_err()
    );
    let absent = original.run(&pool).unwrap().transaction();
    assert!(matches!(
        absent.read(
            &path,
            None,
            &value.asset_definition_id,
            &value.account_id,
            &pool
        ),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    drop(absent);
    let fresh = original.run(&pool).unwrap().transaction();
    assert_eq!(
        fresh
            .read(
                &path,
                Some(&bytes),
                &value.asset_definition_id,
                &value.account_id,
                &pool
            )
            .unwrap(),
        Some(value.uid)
    );
    drop(fresh);
    drop(original);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn captured_binding_rejection_waits_for_the_matching_authoritative_row() {
    let value = value(6);
    let path = binding_key(&value.asset_definition_id, &value.account_id);
    let mut bytes = encode(&value).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    let pool = AllocationBudget::new(1 << 20);
    let original = BindingPreparations::try_new(&pool).unwrap();
    let first = original.run(&pool).unwrap().transaction();
    assert!(matches!(
        first.read(
            &path,
            Some(&bytes),
            &value.asset_definition_id,
            &value.account_id,
            &pool
        ),
        Err(ExecutionAttemptError::Rejected(_))
    ));
    drop(first);
    original.prepare_captured(&pool).unwrap();
    assert!(
        original
            .0
            .lock()
            .unwrap()
            .current
            .as_ref()
            .unwrap()
            .rejected
            .is_some()
    );
    let absent = original.run(&pool).unwrap().transaction();
    assert!(matches!(
        absent.read(
            &path,
            None,
            &value.asset_definition_id,
            &value.account_id,
            &pool
        ),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    drop(absent);
    assert!(
        original
            .0
            .lock()
            .unwrap()
            .current
            .as_ref()
            .unwrap()
            .rejected
            .is_some()
    );
    let fresh = original.run(&pool).unwrap().transaction();
    assert!(matches!(
        fresh.read(
            &path,
            Some(&bytes),
            &value.asset_definition_id,
            &value.account_id,
            &pool
        ),
        Err(ExecutionAttemptError::Rejected(_))
    ));
    drop(fresh);
    drop(original);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn original_binding_lock_is_inline_and_remains_poisoned_after_guard_unwind() {
    // The repository enables parking_lot deadlock detection globally. Its per-thread
    // detector is shared runtime instrumentation, not a per-bank allocation.
    drop(parking_lot::Mutex::new(()).lock());
    let lock = BankLock::default();
    assert_eq!(
        crate::test_allocations::allocations_during(|| {
            let _guard = lock.lock().unwrap();
            assert!(lock.try_lock().is_err());
        }),
        0
    );
    assert!(lock.try_lock().is_ok());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = lock.lock().unwrap();
        panic!("abandon the original bank under its physical guard");
    }));
    assert!(result.is_err());
    assert!(lock.lock().is_err());
    assert!(lock.try_lock().is_err());
}

#[test]
fn original_binding_capture_and_append_admission_survive_before_world_replay() {
    for stage in 0..3 {
        let value = value(10);
        let path = binding_key(&value.asset_definition_id, &value.account_id);
        let bytes = encode(&value).unwrap();
        let pool = AllocationBudget::new(1 << 20);
        let original = BindingPreparations::try_new(&pool).unwrap();
        let mut run = original.run(&pool).unwrap();
        let invocation = run.transaction();
        if stage == 2 {
            invocation
                .read(
                    &path,
                    Some(&bytes),
                    &value.asset_definition_id,
                    &value.account_id,
                    &pool,
                )
                .unwrap();
        }
        let available = if stage == 1 { path.as_ref().len() } else { 0 };
        let held = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes() - available)
            .unwrap();
        let Err(ExecutionAttemptError::Deferred(_)) = invocation.read(
            &path,
            Some(&bytes),
            &value.asset_definition_id,
            &value.account_id,
            &pool,
        ) else {
            panic!("original capture stage must refuse");
        };
        let retained = pool.reserved_bytes();
        let refusal = original.prepare_captured(&pool).unwrap_err();
        assert_eq!(
            pool.reserved_bytes(),
            retained,
            "failed capture preparation keeps its original prefix"
        );
        let watch = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
        let mut credit = watch
            .try_reserve(ReleaseRegistration::allocation_layout())
            .unwrap();
        let mut registration = ReleaseRegistration::from_reservation(&mut credit).unwrap();
        registration.bind_thread(std::thread::current());
        assert_eq!(poll(&mut registration, &refusal), Poll::Pending);
        drop(held);
        original.prepare_captured(&pool).unwrap();
        {
            let bank = original.0.lock().unwrap();
            assert!(bank.pending_append.is_none());
            let current = bank.current.as_ref().unwrap();
            assert!(
                current.complete.is_none(),
                "empty funded capture grants no decoded authority"
            );
            assert!(current.row.as_ref().unwrap().as_slice().is_empty());
        }
        drop(invocation);
        let mut replay = original.run(&pool).unwrap();
        let invocation = replay.transaction();
        for _ in 0..=usize::from(stage == 2) {
            assert_eq!(
                invocation
                    .read(
                        &path,
                        Some(&bytes),
                        &value.asset_definition_id,
                        &value.account_id,
                        &pool
                    )
                    .unwrap(),
                Some(value.uid)
            );
        }
        drop(invocation);
        drop(replay);
        drop(run);
        drop(original);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}
