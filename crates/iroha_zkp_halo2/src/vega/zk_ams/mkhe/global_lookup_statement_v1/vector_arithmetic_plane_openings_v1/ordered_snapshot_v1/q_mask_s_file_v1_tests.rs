//! Actual sibling-file custody; tiny original pair supplies no source authority.
use super::*;
use crate::testing::TestDirectory;
// Leaf tests fund real named buffers but mint no original-source authority.
fn create(
    original: &OrderedPlaneSpoolSnapshotV1,
    directory: &Path,
    plan: QMaskSFilePlanV1,
) -> Result<QMaskSFileV1, OrderedSnapshotErrorV1> {
    let mut memory = RnsNativeProofResourceBudgetV1::default();
    let owned = plan.reserve_memory_v1(&mut memory)?;
    original.create_q_mask_s_file_v1(directory, plan, owned)
}
fn create_with(
    original: &OrderedPlaneSpoolSnapshotV1,
    directory: &Path,
    plan: QMaskSFilePlanV1,
    factory: impl FnOnce(
        &Path,
        ConfidentialSpoolLayoutV1,
    ) -> Result<ConfidentialSpoolWriterV1, ConfidentialSpoolErrorV1>,
) -> Result<QMaskSFileV1, OrderedSnapshotErrorV1> {
    let mut memory = RnsNativeProofResourceBudgetV1::default();
    let owned = plan.reserve_memory_v1(&mut memory)?;
    original.create_q_mask_s_file_with_v1(directory, plan, owned, factory)
}
const PAIR_BYTES: u64 = 66 * 16_400;
fn snapshot(
    directory: &Path,
    budget: &mut OrderedStorageSessionBudgetV1,
    context: [u8; 32],
) -> OrderedPlaneSpoolSnapshotV1 {
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_tiny_for_test_v1(directory, context, budget).unwrap();
    for slot in 0..66 {
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        if slot % 33 == 32 {
            chunk.as_mut_slice_v1()[31] = 1;
            chunk.as_mut_slice_v1()[32..65].copy_from_slice(
                &Point::canonical_generator()
                    .unwrap()
                    .to_non_identity_wire_bytes()
                    .unwrap(),
            );
        }
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    writer.seal_v1().unwrap()
}
#[test]
#[cfg(unix)]
fn qmask_sibling_capacity_refuses_before_factory_and_retries_original_pair() {
    let directory = TestDirectory::new("qmask-sibling-capacity");
    let mut budget = OrderedStorageSessionBudgetV1::with_test_limits_v1(
        PAIR_BYTES + S_FILE_BYTES_V1,
        2 * PAIR_BYTES + 2 * S_FILE_BYTES_V1,
    );
    let original = snapshot(directory.path(), &mut budget, [41; 32]);
    let identity = original.snapshot_digest_v1().unwrap();
    let competitor = create(
        &original,
        directory.path(),
        original.q_mask_s_file_plan_v1().unwrap(),
    )
    .unwrap();
    let before = budget.test_usage_words_v1();
    assert!(matches!(
        create_with(
            &original,
            directory.path(),
            original.q_mask_s_file_plan_v1().unwrap(),
            |_, _| panic!("capacity must precede any handle/key allocation")
        ),
        Err(OrderedSnapshotErrorV1::Capacity)
    ));
    assert_eq!(budget.test_usage_words_v1(), before);
    assert_eq!(original.snapshot_digest_v1().unwrap(), identity);
    drop(competitor);
    assert_eq!(budget.test_usage_words_v1()[0], PAIR_BYTES);
    let retry = create(
        &original,
        directory.path(),
        original.q_mask_s_file_plan_v1().unwrap(),
    )
    .unwrap();
    assert_eq!(
        budget.test_usage_words_v1()[0],
        PAIR_BYTES + S_FILE_BYTES_V1
    );
    drop(original);
    assert_eq!(budget.test_usage_words_v1()[0], S_FILE_BYTES_V1);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    drop(retry);
    assert_eq!(budget.test_usage_words_v1()[0], 0);
    assert_eq!(budget.test_usage_words_v1()[2], 0);
    assert_eq!(budget.test_usage_words_v1()[3], 2 * PAIR_BYTES);
}
#[test]
#[cfg(unix)]
fn qmask_sibling_exact_first_block_keeps_full_file_and_unspent_seal_reservation() {
    let directory = TestDirectory::new("qmask-sibling-first");
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let original = snapshot(directory.path(), &mut budget, [42; 32]);
    let mut file = create(
        &original,
        directory.path(),
        original.q_mask_s_file_plan_v1().unwrap(),
    )
    .unwrap();
    for slot in 0_u64..8 {
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
        chunk.as_mut_slice_v1()[..8].copy_from_slice(&slot.to_le_bytes());
        file.write_slot_v1(slot, chunk).unwrap();
    }
    let closed = file.finish_block_v1().unwrap();
    assert_eq!(closed.file.next_slot, 8);
    let usage = budget.test_usage_words_v1();
    assert_eq!(usage[0], PAIR_BYTES + S_FILE_BYTES_V1);
    assert_eq!(usage[2], 2 * S_FILE_BYTES_V1 - 8 * 16_400);
    assert_eq!(usage[3], 2 * PAIR_BYTES + 8 * 16_400);
    // A partial first block cannot be promoted to a completed spool. Exercise
    // the real leaf's rejection only inside this private control.
    let QMaskSFileLiveV1 {
        writer,
        reservation,
        memory,
    } = closed.file.live.unwrap();
    assert!(writer.seal_v1().is_err());
    drop(reservation);
    drop(memory);
    drop(original);
    assert_eq!(budget.test_usage_words_v1()[0], 0);
    assert_eq!(budget.test_usage_words_v1()[2], 0);
    assert_eq!(budget.test_usage_words_v1()[3], 2 * PAIR_BYTES + 8 * 16_400);
}
#[test]
#[cfg(unix)]
fn qmask_sibling_rejects_substitution_reordering_missing_and_repeated_slots() {
    let directory = TestDirectory::new("qmask-sibling-order");
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let first = snapshot(directory.path(), &mut budget, [43; 32]);
    let other = snapshot(directory.path(), &mut budget, [44; 32]);
    let before = budget.test_usage_words_v1();
    assert!(matches!(
        create(
            &first,
            directory.path(),
            other.q_mask_s_file_plan_v1().unwrap()
        ),
        Err(OrderedSnapshotErrorV1::Context)
    ));
    assert_eq!(budget.test_usage_words_v1(), before);
    for slot in [1, 8, u64::MAX] {
        let mut file = create(
            &first,
            directory.path(),
            first.q_mask_s_file_plan_v1().unwrap(),
        )
        .unwrap();
        assert_eq!(
            file.write_slot_v1(
                slot,
                ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap()
            ),
            Err(OrderedSnapshotErrorV1::Order)
        );
        assert!(file.live.is_none());
        assert_eq!(budget.test_usage_words_v1()[0], 2 * PAIR_BYTES);
    }
    assert!(matches!(
        ConfidentialSpoolChunkV1::new_zeroed_v1(16_385),
        Err(ConfidentialSpoolErrorV1::LimitExceeded(
            "plaintext chunk length"
        ))
    ));
    {
        let length = 16_383;
        let mut file = create(
            &first,
            directory.path(),
            first.q_mask_s_file_plan_v1().unwrap(),
        )
        .unwrap();
        let before = budget.test_usage_words_v1()[3];
        assert_eq!(
            file.write_slot_v1(0, ConfidentialSpoolChunkV1::new_zeroed_v1(length).unwrap()),
            Err(OrderedSnapshotErrorV1::Order)
        );
        assert!(file.live.is_none());
        assert_eq!(budget.test_usage_words_v1()[0], 2 * PAIR_BYTES);
        assert_eq!(budget.test_usage_words_v1()[3], before);
    }
    let file = create(
        &first,
        directory.path(),
        first.q_mask_s_file_plan_v1().unwrap(),
    )
    .unwrap();
    assert!(matches!(
        file.finish_block_v1(),
        Err(OrderedSnapshotErrorV1::Order)
    ));
    let mut file = create(
        &first,
        directory.path(),
        first.q_mask_s_file_plan_v1().unwrap(),
    )
    .unwrap();
    file.write_slot_v1(0, ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap())
        .unwrap();
    assert_eq!(
        file.write_slot_v1(0, ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap()),
        Err(OrderedSnapshotErrorV1::Order)
    );
    assert!(file.live.is_none());
    assert_eq!(budget.test_usage_words_v1()[3], 4 * PAIR_BYTES + 16_400);
}
#[test]
#[cfg(unix)]
fn qmask_sibling_creation_failure_unwind_and_real_leaf_failure_refund_only_unspent() {
    let directory = TestDirectory::new("qmask-sibling-failure");
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let original = snapshot(directory.path(), &mut budget, [45; 32]);
    assert!(matches!(
        create(
            &original,
            &directory.path().join("missing"),
            original.q_mask_s_file_plan_v1().unwrap()
        ),
        Err(OrderedSnapshotErrorV1::Storage)
    ));
    assert_eq!(budget.test_usage_words_v1()[0], PAIR_BYTES);
    assert_eq!(budget.test_usage_words_v1()[2], 0);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = create_with(
            &original,
            directory.path(),
            original.q_mask_s_file_plan_v1().unwrap(),
            |path, layout| {
                let _actual = ConfidentialSpoolWriterV1::create_in_v1(path, layout).unwrap();
                panic!("actual created detached leaf unwinds before installation");
            },
        );
    }));
    assert!(panic.is_err());
    assert_eq!(budget.test_usage_words_v1()[0], PAIR_BYTES);
    assert_eq!(budget.test_usage_words_v1()[2], 0);
    let mut file = create(
        &original,
        directory.path(),
        original.q_mask_s_file_plan_v1().unwrap(),
    )
    .unwrap();
    // Test-only perturbation of the actual leaf cursor; no production bypass or
    // new fault API. The wrapper still charges the failed real leaf call.
    file.live
        .as_mut()
        .unwrap()
        .writer
        .write_slot_v1(0, ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap())
        .unwrap();
    assert_eq!(
        file.write_slot_v1(0, ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap()),
        Err(OrderedSnapshotErrorV1::Storage)
    );
    assert!(file.live.is_none());
    assert_eq!(budget.test_usage_words_v1()[0], PAIR_BYTES);
    assert_eq!(budget.test_usage_words_v1()[2], 0);
    assert_eq!(budget.test_usage_words_v1()[3], 2 * PAIR_BYTES + 16_400);
}

#[test]
#[cfg(unix)]
fn qmask_sibling_memory_stays_charged_through_file_lifetime_after_issuer_scope() {
    let directory = TestDirectory::new("qmask-sibling-memory");
    let mut storage = OrderedStorageSessionBudgetV1::new_v1();
    let original = snapshot(directory.path(), &mut storage, [46; 32]);
    let plan = original.q_mask_s_file_plan_v1().unwrap();
    let mut proof = RnsNativeProofResourceBudgetV1::default();
    let memory = plan.reserve_memory_v1(&mut proof).unwrap();
    let charged = proof.live_bytes().unwrap();
    assert!(charged > 16_384);
    let file = original
        .create_q_mask_s_file_v1(directory.path(), plan, memory)
        .unwrap();
    assert_eq!(proof.live_bytes().unwrap(), charged);
    drop(original);
    assert_eq!(proof.live_bytes().unwrap(), charged);
    assert_eq!(storage.test_usage_words_v1()[0], S_FILE_BYTES_V1);
    drop(file);
    assert_eq!(proof.live_bytes().unwrap(), 0);
    assert_eq!(storage.test_usage_words_v1()[0], 0);
}

#[test]
#[cfg(unix)]
fn qmask_first_openings_binding_requires_actual_closed_eight_slot_owner() {
    let directory = TestDirectory::new("qmask-first-openings-file");
    let mut storage = OrderedStorageSessionBudgetV1::new_v1();
    let original = snapshot(directory.path(), &mut storage, [92; 32]);
    let plan = original.q_mask_s_file_plan_v1().unwrap();
    let mut proof = RnsNativeProofResourceBudgetV1::default();
    let memory = plan.reserve_memory_v1(&mut proof).unwrap();
    let mut file = original
        .create_q_mask_s_file_v1(directory.path(), plan, memory)
        .unwrap();
    let expected = file.binding_v1();
    for slot in 0..8 {
        file.write_slot_v1(
            slot,
            ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap(),
        )
        .unwrap();
    }
    let mut closed = file.finish_block_v1().unwrap();
    assert_eq!(closed.require_block_binding_v1(), Ok(expected));
    closed.require_original_budget_v1(&proof).unwrap();
    let other = RnsNativeProofResourceBudgetV1::default();
    assert_eq!(
        closed.require_original_budget_v1(&other),
        Err(OrderedSnapshotErrorV1::Context)
    );
    closed.file.next_slot = 7;
    assert_eq!(
        closed.require_block_binding_v1(),
        Err(OrderedSnapshotErrorV1::Order)
    );
    closed.file.next_slot = 8;
    closed.file.live = None;
    assert_eq!(
        closed.require_block_binding_v1(),
        Err(OrderedSnapshotErrorV1::Order)
    );
    assert_eq!(storage.test_usage_words_v1()[0], PAIR_BYTES);
}
