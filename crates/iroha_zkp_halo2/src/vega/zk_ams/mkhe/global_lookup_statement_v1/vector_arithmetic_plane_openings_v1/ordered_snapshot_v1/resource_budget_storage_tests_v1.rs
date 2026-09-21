//! Real detached-file reservation lifetimes; tiny geometry is not proof evidence.
use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_BUDGET_DIRECTORY_V1: AtomicU64 = AtomicU64::new(0);

struct BudgetDirectoryV1(PathBuf);
impl BudgetDirectoryV1 {
    fn new_v1() -> Self {
        let path = std::env::temp_dir().join(format!(
            "iroha-ordered-budget-{}-{}",
            std::process::id(),
            NEXT_BUDGET_DIRECTORY_V1.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for BudgetDirectoryV1 {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const RECORD_BYTES_V1: u64 = 16_400;
const PAIR_BYTES_V1: u64 = 66 * RECORD_BYTES_V1;

fn budget_plan_v1() -> OrderedPlaneSpoolPlanV1 {
    OrderedPlaneSpoolPlanV1::build_v1(GeometryV1::Tiny, [0x31; 32], [0x42; 32]).unwrap()
}

fn budget_chunk_v1(slot: u64) -> ConfidentialSpoolChunkV1 {
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
    if slot % 33 == 32 {
        chunk.as_mut_slice_v1()[31] = 1;
        chunk.as_mut_slice_v1()[32..65].copy_from_slice(
            &Point::canonical_generator()
                .unwrap()
                .to_non_identity_wire_bytes()
                .unwrap(),
        );
    } else {
        chunk.as_mut_slice_v1()[31] = slot as u8;
    }
    chunk
}

fn write_budget_pair_v1(mut writer: OrderedPlaneSpoolWriterV1) -> OrderedPlaneSpoolWriterV1 {
    for slot in 0..66 {
        writer.write_slot_v1(slot, budget_chunk_v1(slot)).unwrap();
    }
    writer
}

#[test]
#[cfg(unix)]
fn ordered_storage_budget_actual_create_write_seal_read_and_quota_refusal() {
    let directory = BudgetDirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::with_test_limits_v1(
        PAIR_BYTES_V1,
        2 * PAIR_BYTES_V1 + RECORD_BYTES_V1,
    );
    let writer =
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget)
            .unwrap();
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (PAIR_BYTES_V1, 2 * PAIR_BYTES_V1, 0)
    );
    let writer = write_budget_pair_v1(writer);
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (PAIR_BYTES_V1, PAIR_BYTES_V1, PAIR_BYTES_V1)
    );
    let mut snapshot = writer.seal_v1().unwrap();
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (PAIR_BYTES_V1, 0, 2 * PAIR_BYTES_V1)
    );
    assert_eq!(
        snapshot.read_slot_v1(33).unwrap().as_slice_v1(),
        budget_chunk_v1(33).as_slice_v1()
    );
    let identity = snapshot.snapshot_digest_v1().unwrap();
    let before = budget.usage_v1().unwrap();
    assert!(matches!(
        snapshot.read_slot_v1(0),
        Err(OrderedSnapshotErrorV1::Capacity)
    ));
    assert!(snapshot.live.is_some());
    assert_eq!(snapshot.snapshot_digest_v1().unwrap(), identity);
    assert_eq!(budget.usage_v1().unwrap(), before);
    assert!(matches!(
        snapshot.read_slot_v1(0),
        Err(OrderedSnapshotErrorV1::Capacity)
    ));
    drop(snapshot);
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (0, 0, 2 * PAIR_BYTES_V1 + RECORD_BYTES_V1)
    );
    assert!(directory.0.read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(unix)]
fn ordered_storage_budget_refuses_before_either_leaf_and_cleans_failed_second_create() {
    let directory = BudgetDirectoryV1::new_v1();
    for (spool, io) in [
        (PAIR_BYTES_V1 - 1, 2 * PAIR_BYTES_V1),
        (PAIR_BYTES_V1, 2 * PAIR_BYTES_V1 - 1),
    ] {
        let mut budget = OrderedStorageSessionBudgetV1::with_test_limits_v1(spool, io);
        let mut calls = 0;
        let result = OrderedPlaneSpoolWriterV1::create_with_plan_and_leaf_factory_v1(
            &directory.0,
            budget_plan_v1(),
            &mut budget,
            |path, layout| {
                calls += 1;
                ConfidentialSpoolWriterV1::create_in_v1(path, layout)
            },
        );
        assert!(matches!(result, Err(OrderedSnapshotErrorV1::Capacity)));
        assert_eq!(calls, 0);
        assert_eq!(budget.usage_v1().unwrap().live_spool_bytes, 0);
        assert_eq!(budget.usage_v1().unwrap().reserved_io_bytes, 0);
    }
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let mut calls = 0;
    let missing = directory.0.join("not-a-directory");
    let result = OrderedPlaneSpoolWriterV1::create_with_plan_and_leaf_factory_v1(
        &directory.0,
        budget_plan_v1(),
        &mut budget,
        |path, layout| {
            calls += 1;
            // First leaf really exists, detached/sized with its key, while the
            // second genuine crypto creation fails on the absent directory.
            ConfidentialSpoolWriterV1::create_in_v1(
                if calls == 1 { path } else { &missing },
                layout,
            )
        },
    );
    assert!(matches!(result, Err(OrderedSnapshotErrorV1::Storage)));
    assert_eq!(calls, 2);
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (0, 0, 0)
    );
    assert!(directory.0.read_dir().unwrap().next().is_none());

    calls = 0;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _writer = OrderedPlaneSpoolWriterV1::create_with_plan_and_leaf_factory_v1(
                &directory.0,
                budget_plan_v1(),
                &mut budget,
                |path, layout| {
                    calls += 1;
                    assert_eq!(calls, 1, "injected second-leaf construction unwind");
                    ConfidentialSpoolWriterV1::create_in_v1(path, layout)
                },
            );
        }))
        .is_err()
    );
    assert_eq!(calls, 2);
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (0, 0, 0)
    );
    assert!(directory.0.read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(unix)]
fn ordered_storage_budget_repeat_reads_are_charged_and_bounds_refuse_before_io() {
    let directory = BudgetDirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let writer =
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget)
            .unwrap();
    let mut snapshot = write_budget_pair_v1(writer).seal_v1().unwrap();
    for expected_reads in 1..=3 {
        assert_eq!(
            snapshot.read_slot_v1(0).unwrap().as_slice_v1(),
            budget_chunk_v1(0).as_slice_v1()
        );
        assert_eq!(
            budget.usage_v1().unwrap().attempted_io_bytes,
            2 * PAIR_BYTES_V1 + expected_reads * RECORD_BYTES_V1
        );
    }
    let before = budget.usage_v1().unwrap().attempted_io_bytes;
    assert!(matches!(
        snapshot.read_slot_v1(66),
        Err(OrderedSnapshotErrorV1::Shape)
    ));
    let usage = budget.usage_v1().unwrap();
    assert_eq!(usage.attempted_io_bytes, before);
    assert_eq!((usage.live_spool_bytes, usage.reserved_io_bytes), (0, 0));
    assert!(matches!(
        snapshot.read_slot_v1(0),
        Err(OrderedSnapshotErrorV1::Poisoned)
    ));
}

#[test]
#[cfg(unix)]
fn ordered_storage_budget_failed_second_seal_keeps_attempted_bytes_and_drops_pair() {
    let directory = BudgetDirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget)
            .unwrap();
    for slot in 0..65 {
        writer.write_slot_v1(slot, budget_chunk_v1(slot)).unwrap();
    }
    // Only outer bookkeeping is corrupted in this private test. First leaf
    // seals for real; second leaf refuses its genuinely missing final record.
    writer.next_slot = 66;
    assert!(matches!(
        writer.seal_v1(),
        Err(OrderedSnapshotErrorV1::Storage)
    ));
    let usage = budget.usage_v1().unwrap();
    assert_eq!((usage.live_spool_bytes, usage.reserved_io_bytes), (0, 0));
    assert_eq!(usage.attempted_io_bytes, (65 + 33 + 33) * RECORD_BYTES_V1);
    assert!(directory.0.read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(unix)]
fn ordered_storage_budget_two_concurrent_pairs_share_reservations_through_snapshot_drop() {
    let directory = BudgetDirectoryV1::new_v1();
    let mut budget =
        OrderedStorageSessionBudgetV1::with_test_limits_v1(2 * PAIR_BYTES_V1, 4 * PAIR_BYTES_V1);
    let first =
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget)
            .unwrap();
    let second =
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget)
            .unwrap();
    assert!(matches!(
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget),
        Err(OrderedSnapshotErrorV1::Capacity)
    ));
    let first = std::thread::spawn(move || write_budget_pair_v1(first).seal_v1().unwrap());
    let second = std::thread::spawn(move || write_budget_pair_v1(second).seal_v1().unwrap());
    let first = first.join().unwrap();
    let mut second = second.join().unwrap();
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.peak_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (2 * PAIR_BYTES_V1, 2 * PAIR_BYTES_V1, 0, 4 * PAIR_BYTES_V1)
    );
    drop(first);
    assert_eq!(budget.usage_v1().unwrap().live_spool_bytes, PAIR_BYTES_V1);
    assert!(matches!(
        second.read_slot_v1(0),
        Err(OrderedSnapshotErrorV1::Capacity)
    ));
    assert!(second.live.is_some());
    assert_eq!(budget.usage_v1().unwrap().live_spool_bytes, PAIR_BYTES_V1);
    assert_eq!(
        budget.usage_v1().unwrap().attempted_io_bytes,
        4 * PAIR_BYTES_V1
    );
    drop(second);
    assert_eq!(budget.usage_v1().unwrap().live_spool_bytes, 0);
}

#[test]
#[cfg(unix)]
fn ordered_storage_budget_temporary_capacity_preserves_pair_until_other_reservation_drops() {
    let directory = BudgetDirectoryV1::new_v1();
    let mut budget =
        OrderedStorageSessionBudgetV1::with_test_limits_v1(2 * PAIR_BYTES_V1, 4 * PAIR_BYTES_V1);
    let first =
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget)
            .unwrap();
    let mut first = write_budget_pair_v1(first).seal_v1().unwrap();
    let second =
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget)
            .unwrap();
    let identity = first.snapshot_digest_v1().unwrap();
    let leaves = first.leaf_digests;
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (2 * PAIR_BYTES_V1, 2 * PAIR_BYTES_V1, 2 * PAIR_BYTES_V1)
    );
    for _ in 0..2 {
        assert!(matches!(
            first.read_slot_v1(33),
            Err(OrderedSnapshotErrorV1::Capacity)
        ));
        assert!(first.live.is_some());
        assert_eq!(first.snapshot_digest_v1().unwrap(), identity);
        assert_eq!(first.leaf_digests, leaves);
        assert_eq!(budget.usage_v1().unwrap(), usage);
    }
    drop(second);
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (PAIR_BYTES_V1, 0, 2 * PAIR_BYTES_V1)
    );
    assert_eq!(
        first.read_slot_v1(33).unwrap().as_slice_v1(),
        budget_chunk_v1(33).as_slice_v1()
    );
    assert_eq!(first.snapshot_digest_v1().unwrap(), identity);
    assert_eq!(first.leaf_digests, leaves);
    assert_eq!(
        budget.usage_v1().unwrap().attempted_io_bytes,
        2 * PAIR_BYTES_V1 + RECORD_BYTES_V1
    );
    drop(first);
    assert_eq!(budget.usage_v1().unwrap().live_spool_bytes, 0);
    assert!(directory.0.read_dir().unwrap().next().is_none());
}

#[test]
#[cfg(unix)]
fn ordered_storage_budget_cancel_wrong_order_unwind_and_detached_issuer_lifetimes() {
    let directory = BudgetDirectoryV1::new_v1();
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    let mut writer =
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget)
            .unwrap();
    writer.write_slot_v1(0, budget_chunk_v1(0)).unwrap();
    assert!(writer.write_slot_v1(0, budget_chunk_v1(0)).is_err());
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (0, 0, RECORD_BYTES_V1)
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut writer = OrderedPlaneSpoolWriterV1::create_with_plan_v1(
                &directory.0,
                budget_plan_v1(),
                &mut budget,
            )
            .unwrap();
            writer.write_slot_v1(0, budget_chunk_v1(0)).unwrap();
            panic!("injected live storage owner unwind");
        }))
        .is_err()
    );
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (0, 0, 2 * RECORD_BYTES_V1)
    );
    let writer =
        OrderedPlaneSpoolWriterV1::create_with_plan_v1(&directory.0, budget_plan_v1(), &mut budget)
            .unwrap();
    let mut snapshot = write_budget_pair_v1(writer).seal_v1().unwrap();
    drop(budget); // The snapshot's reservation still owns the same ledger.
    assert_eq!(
        snapshot.read_slot_v1(65).unwrap().as_slice_v1(),
        budget_chunk_v1(65).as_slice_v1()
    );
    drop(snapshot);
    assert!(directory.0.read_dir().unwrap().next().is_none());
}
