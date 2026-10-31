//! Authored controls only: this detached candidate has not been compiled or run.
use super::*;
use crate::{
    account::AccountId,
    events::time::{ExecutionTime, Schedule, TimeEventFilter},
    transaction::{Executable, IvmBytecode},
    trigger::action::Repeats,
};
use iroha_crypto::{Algorithm, KeyPair};

fn source_frame() -> Vec<u8> {
    // An allocating codec oracle is confined to these authored fixtures. The
    // production capture path receives complete bytes and never builds Action.
    let key = KeyPair::try_from_seed(vec![17; 32], Algorithm::Ed25519).unwrap();
    let action = Action::new(
        Executable::Ivm(IvmBytecode::from_compiled(vec![0, 1, 255, 9])),
        Repeats::Exactly(2),
        AccountId::new(key.public_key().clone()),
        TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
            std::time::Duration::from_millis(7),
        ))),
    )
    .unwrap();
    norito::encode_canonical(&action).unwrap()
}

fn limits() -> ActionScannerLimitsV1 {
    ActionScannerLimitsV1 {
        max_frame_bytes: 65_536,
        max_total_frame_bytes: 131_072,
        max_frames: 4,
        max_work: 2_000_000,
        max_fields: 1000,
        max_sequence_elements: 65_536,
        max_depth: 32,
    }
}

fn scanner(
    pool: &AllocationBudget,
    limits: ActionScannerLimitsV1,
) -> BoundedActionFrameScannerV1<'_> {
    BoundedActionFrameScannerV1::try_new(
        pool,
        limits,
        DecodeLimits::new(1000, 65_536, 1000, 65_536, 32),
    )
    .unwrap()
}

#[test]
fn prepared_custody_keeps_all_original_bytes_and_the_actual_pool() {
    let mut source = source_frame();
    let expected = source.clone();
    let pool = AllocationBudget::new(1_048_576);
    let foreign = AllocationBudget::new(1_048_576);
    let mut scanner = scanner(&pool, limits());
    let (owned, semantics) = scanner.capture_and_scan(&source).unwrap();
    assert!(matches!(
        semantics.filter_kind,
        TriggerLifecycleStateKindV1::Time
    ));
    assert!(!semantics.has_retry_policy);
    assert_eq!(owned.as_bytes(), expected);
    assert_ne!(owned.as_bytes().as_ptr(), source.as_ptr());
    assert_eq!(owned.backing_bytes(), expected.len());
    assert!(owned.belongs_to(&pool));
    assert!(!owned.belongs_to(&foreign));
    source.fill(0);
    source.clear();
    assert_eq!(
        owned.as_bytes(),
        expected,
        "custody owns the original complete copy"
    );
    drop(scanner);
    assert_eq!(pool.reserved_bytes(), expected.len());
    drop(owned);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn exact_backing_admission_refuses_before_copy_and_growth_does_not_retry() {
    let source = source_frame();
    let pool = AllocationBudget::new(1_048_576);
    let mut scanner = scanner(&pool, limits());
    let controls = pool.reserved_bytes();
    pool.set_limit_bytes(controls);
    let prior_peak = pool.peak_reserved_bytes();
    let first = scanner.capture_and_scan(&source).unwrap_err();
    assert!(first.frame().is_none());
    assert!(matches!(
        first.failure(),
        ActionFrameScanFailureV1::Capture(iroha_allocation::ChargedBufferError::Admission(_))
    ));
    let first_address = std::ptr::from_ref(first.failure());
    drop(first);
    assert_eq!(pool.reserved_bytes(), controls);
    assert_eq!(pool.peak_reserved_bytes(), prior_peak);
    pool.set_limit_bytes(1_048_576);
    let later = scanner.capture_and_scan(&source).unwrap_err();
    assert!(later.frame().is_none());
    assert_eq!(std::ptr::from_ref(later.failure()), first_address);
    drop(later);
    assert_eq!(pool.reserved_bytes(), controls);
    assert_eq!(pool.peak_reserved_bytes(), prior_peak);
    drop(scanner);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn malformed_frame_returns_the_complete_original_owner_and_first_error() {
    let mut source = source_frame();
    // Keep the original header but change payload bytes: checksum validation
    // must refuse, and custody must preserve the rejected bytes exactly.
    *source.last_mut().unwrap() ^= 1;
    let pool = AllocationBudget::new(1_048_576);
    let mut scanner = scanner(&pool, limits());
    let controls = pool.reserved_bytes();
    let first = scanner.capture_and_scan(&source).unwrap_err();
    assert!(matches!(
        first.failure(),
        ActionFrameScanFailureV1::Codec(_)
    ));
    let first_address = std::ptr::from_ref(first.failure());
    let captured_address = first.frame().unwrap().as_bytes().as_ptr();
    assert_eq!(first.frame().unwrap().as_bytes(), source);
    assert_eq!(pool.reserved_bytes(), controls + source.len());
    let owned = first.into_frame().unwrap();
    assert_eq!(owned.as_bytes().as_ptr(), captured_address);
    let valid = source_frame();
    let later = scanner.capture_and_scan(&valid).unwrap_err();
    assert!(later.frame().is_none());
    assert_eq!(std::ptr::from_ref(later.failure()), first_address);
    drop(later);
    assert_eq!(pool.reserved_bytes(), controls + source.len());
    drop(scanner);
    assert_eq!(pool.reserved_bytes(), source.len());
    assert_eq!(owned.as_bytes(), source);
    drop(owned);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn dropping_a_refused_preparation_disposes_its_byte_owner() {
    let source = [0xff; 64];
    let pool = AllocationBudget::new(1_048_576);
    let mut scanner = scanner(&pool, limits());
    let controls = pool.reserved_bytes();
    let refused = scanner.capture_and_scan(&source).unwrap_err();
    assert_eq!(refused.frame().unwrap().as_bytes(), source);
    assert_eq!(pool.reserved_bytes(), controls + source.len());
    drop(refused);
    assert_eq!(pool.reserved_bytes(), controls);
    assert!(
        scanner.failure().is_some(),
        "dropping byte custody cannot reset the range"
    );
    drop(scanner);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn copy_work_limit_refuses_before_backing_admission() {
    let source = source_frame();
    let pool = AllocationBudget::new(1_048_576);
    let mut limit = limits();
    // begin_frame admits the complete envelope work; the separate complete copy
    // cannot fit, so even the frame backing must never be admitted.
    limit.max_work = source.len() as u64;
    let mut scanner = scanner(&pool, limit);
    let controls = pool.reserved_bytes();
    let peak = pool.peak_reserved_bytes();
    let failure = scanner.capture_and_scan(&source).unwrap_err();
    assert!(failure.frame().is_none());
    assert!(matches!(
        failure.failure(),
        ActionFrameScanFailureV1::Structure(_)
    ));
    drop(failure);
    assert_eq!(pool.reserved_bytes(), controls);
    assert_eq!(pool.peak_reserved_bytes(), peak);
}

#[test]
fn frame_count_remains_cumulative_while_original_custody_is_retained() {
    let source = source_frame();
    let pool = AllocationBudget::new(1_048_576);
    let mut limit = limits();
    limit.max_frames = 1;
    let mut scanner = scanner(&pool, limit);
    let (owned, _) = scanner.capture_and_scan(&source).unwrap();
    let original_address = owned.as_bytes().as_ptr();
    let retained = pool.reserved_bytes();
    let peak = pool.peak_reserved_bytes();
    let failure = scanner.capture_and_scan(&source).unwrap_err();
    assert!(failure.frame().is_none());
    drop(failure);
    assert_eq!(pool.reserved_bytes(), retained);
    assert_eq!(pool.peak_reserved_bytes(), peak);
    assert_eq!(owned.as_bytes().as_ptr(), original_address);
    assert_eq!(owned.as_bytes(), source);
    drop(scanner);
    assert_eq!(pool.reserved_bytes(), source.len());
    drop(owned);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn complete_frame_byte_sum_is_checked_before_a_second_copy() {
    let source = source_frame();
    let pool = AllocationBudget::new(1_048_576);
    let mut limit = limits();
    limit.max_total_frame_bytes = source.len() as u64;
    let mut scanner = scanner(&pool, limit);
    let (owned, _) = scanner.capture_and_scan(&source).unwrap();
    let retained = pool.reserved_bytes();
    let peak = pool.peak_reserved_bytes();
    let failure = scanner.capture_and_scan(&source).unwrap_err();
    assert!(failure.frame().is_none());
    drop(failure);
    assert_eq!(pool.reserved_bytes(), retained);
    assert_eq!(pool.peak_reserved_bytes(), peak);
    drop(scanner);
    assert_eq!(pool.reserved_bytes(), source.len());
    drop(owned);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn abandoning_unpublished_success_on_unwind_releases_only_its_backing() {
    let source = source_frame();
    let pool = AllocationBudget::new(1_048_576);
    let mut scanner = scanner(&pool, limits());
    let controls = pool.reserved_bytes();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (owned, _) = scanner.capture_and_scan(&source).unwrap();
        assert_eq!(owned.as_bytes(), source);
        assert_eq!(pool.reserved_bytes(), controls + source.len());
        panic!("abandon unpublished frame custody");
    }));
    assert!(result.is_err());
    assert_eq!(pool.reserved_bytes(), controls);
    drop(scanner);
    assert_eq!(pool.reserved_bytes(), 0);
}
