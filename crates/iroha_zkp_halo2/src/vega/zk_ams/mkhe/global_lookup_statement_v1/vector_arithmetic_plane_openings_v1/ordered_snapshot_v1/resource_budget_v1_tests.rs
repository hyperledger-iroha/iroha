//! Atomic admission, conservative I/O debits and cancellation under shared limits.
use super::*;

#[test]
fn ordered_storage_budget_exact_native_limits_and_atomic_overflow_refusal() {
    let mut budget = OrderedStorageSessionBudgetV1::new_v1();
    assert_eq!(budget.ledger.spool_limit, 16 * 1024 * 1024 * 1024);
    assert_eq!(budget.ledger.io_limit, 64 * 1024 * 1024 * 1024);
    let spool = ZK_AMS_MKHE_RNS_NATIVE_SPOOL_MAX_BYTES_V1;
    let reservation = budget.reserve_files_v1(spool).unwrap();
    let before = budget.usage_v1().unwrap();
    assert_eq!(before.live_spool_bytes, spool);
    assert_eq!(before.reserved_io_bytes, 2 * spool);
    assert!(matches!(
        budget.reserve_files_v1(1),
        Err(StorageBudgetErrorV1::SpoolLimit)
    ));
    assert!(matches!(
        budget.reserve_files_v1(u64::MAX),
        Err(StorageBudgetErrorV1::Overflow)
    ));
    assert!(matches!(
        budget.reserve_files_v1(0),
        Err(StorageBudgetErrorV1::Order)
    ));
    assert_eq!(budget.usage_v1().unwrap(), before);
    drop(reservation);
    assert_eq!(budget.usage_v1().unwrap().live_spool_bytes, 0);
    assert_eq!(budget.usage_v1().unwrap().reserved_io_bytes, 0);
    assert_eq!(budget.usage_v1().unwrap().peak_spool_bytes, spool);
}

#[test]
fn ordered_storage_budget_cancellation_refunds_only_unattempted_io() {
    let mut budget = OrderedStorageSessionBudgetV1::with_test_limits_v1(200, 450);
    let mut first = budget.reserve_files_v1(100).unwrap();
    let mut second = budget.reserve_files_v1(100).unwrap();
    first.charge_reserved_io_v1(40).unwrap();
    second.charge_reserved_io_v1(100).unwrap();
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (200, 260, 140)
    );
    assert_eq!(first.charge_read_io_v1(1), Err(StorageBudgetErrorV1::Order));
    assert_eq!(first.require_sealed_v1(), Err(StorageBudgetErrorV1::Order));
    assert_eq!(
        first.charge_reserved_io_v1(161),
        Err(StorageBudgetErrorV1::Order)
    );
    assert_eq!(
        first.charge_reserved_io_v1(0),
        Err(StorageBudgetErrorV1::Order)
    );
    assert_eq!(budget.usage_v1().unwrap(), usage);
    drop(first);
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (100, 100, 140)
    );
    second.charge_reserved_io_v1(100).unwrap();
    second.require_sealed_v1().unwrap();
    second.charge_read_io_v1(210).unwrap();
    assert_eq!(
        second.charge_read_io_v1(1),
        Err(StorageBudgetErrorV1::IoLimit)
    );
    assert_eq!(
        second.charge_read_io_v1(0),
        Err(StorageBudgetErrorV1::Order)
    );
    drop(second);
    let usage = budget.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (0, 0, 450)
    );
    assert!(matches!(
        budget.reserve_files_v1(1),
        Err(StorageBudgetErrorV1::IoLimit)
    ));
}

#[test]
fn ordered_storage_budget_new_pair_cannot_spend_io_reserved_for_an_existing_pair() {
    let mut budget = OrderedStorageSessionBudgetV1::with_test_limits_v1(200, 400);
    let mut first = budget.reserve_files_v1(100).unwrap();
    let second = budget.reserve_files_v1(100).unwrap();
    first.charge_reserved_io_v1(200).unwrap();
    first.require_sealed_v1().unwrap();
    assert_eq!(
        first.charge_read_io_v1(1),
        Err(StorageBudgetErrorV1::IoLimit)
    );
    drop(second);
    first.charge_read_io_v1(200).unwrap();
    assert_eq!(budget.usage_v1().unwrap().attempted_io_bytes, 400);
}

#[test]
fn ordered_storage_budget_unwind_and_poison_cleanup_never_restore_admission() {
    let mut budget = OrderedStorageSessionBudgetV1::with_test_limits_v1(100, 200);
    let reservation = budget.reserve_files_v1(100).unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = budget.ledger.usage.lock().unwrap();
            panic!("injected storage-ledger poison");
        }))
        .is_err()
    );
    assert_eq!(budget.usage_v1(), Err(StorageBudgetErrorV1::Poisoned));
    assert!(matches!(
        budget.reserve_files_v1(1),
        Err(StorageBudgetErrorV1::Poisoned)
    ));
    drop(reservation);
    let usage = budget.ledger.usage.lock().unwrap_err().into_inner();
    assert_eq!((usage.live_spool_bytes, usage.reserved_io_bytes), (0, 0));
    drop(usage);
    assert!(matches!(
        budget.reserve_files_v1(1),
        Err(StorageBudgetErrorV1::Poisoned)
    ));

    let mut healthy = OrderedStorageSessionBudgetV1::with_test_limits_v1(100, 200);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut reservation = healthy.reserve_files_v1(100).unwrap();
            reservation.charge_reserved_io_v1(7).unwrap();
            panic!("injected storage-owner unwind");
        }))
        .is_err()
    );
    let usage = healthy.usage_v1().unwrap();
    assert_eq!(
        (
            usage.live_spool_bytes,
            usage.reserved_io_bytes,
            usage.attempted_io_bytes
        ),
        (0, 0, 7)
    );
}

#[test]
fn qmask_sibling_reservation_requires_original_seal_and_retains_exact_arc() {
    let mut budget = OrderedStorageSessionBudgetV1::with_test_limits_v1(300, 600);
    let mut original = budget.reserve_files_v1(100).unwrap();
    let before = budget.test_usage_words_v1();
    assert!(matches!(
        original.reserve_sibling_file_v1(100),
        Err(StorageBudgetErrorV1::Order)
    ));
    assert_eq!(budget.test_usage_words_v1(), before);
    original.charge_reserved_io_v1(200).unwrap();
    let child = original.reserve_sibling_file_v1(200).unwrap();
    assert!(Arc::ptr_eq(&original.ledger, &child.ledger));
    assert!(Arc::ptr_eq(&budget.ledger, &child.ledger));
    assert_eq!(budget.test_usage_words_v1(), [300, 300, 400, 200]);
    drop(original);
    assert_eq!(budget.test_usage_words_v1(), [200, 300, 400, 200]);
    let before = budget.test_usage_words_v1();
    assert!(matches!(
        budget.reserve_files_v1(1),
        Err(StorageBudgetErrorV1::IoLimit)
    ));
    assert_eq!(budget.test_usage_words_v1(), before);
    drop(child);
    assert_eq!(budget.test_usage_words_v1(), [0, 300, 0, 200]);
}
