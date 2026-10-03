//! Histogram parity, bounded staging, refusal and final-borrower regressions.

use super::*;
use iroha_allocation::AllocationRefusal;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn histogram(numbers: &[u32], budget: Option<&AllocationBudget>) -> Result<SyscallUsages, VMError> {
    let mut scratch = UsageScratch::new(numbers.len(), budget)?;
    for number in numbers {
        scratch.push(*number)?;
    }
    scratch.finish()
}

#[test]
fn complete_unsigned_ids_and_repeated_counts_match_diagnostic_output() {
    let numbers = [0x00ff_ffff, 0, 255, 256, 0x00ff_ffff, 0, 256];
    let budget = AllocationBudget::new(4096);
    let funded = histogram(&numbers, Some(&budget)).unwrap();
    let local = histogram(&numbers, None).unwrap();
    assert_eq!(funded, local);
    assert_eq!(
        &*funded,
        &[
            SyscallUsage {
                number: 0,
                count: 2
            },
            SyscallUsage {
                number: 255,
                count: 1
            },
            SyscallUsage {
                number: 256,
                count: 2
            },
            SyscallUsage {
                number: 0x00ff_ffff,
                count: 2
            },
        ]
    );
    let output = funded.0.as_ref().unwrap();
    assert!(output.belongs_to(&budget));
    assert_eq!(budget.reserved_bytes(), output.allocation_bytes());
    assert_eq!(
        budget.peak_reserved_bytes(),
        output.allocation_bytes() + std::mem::size_of_val(&numbers)
    );
    drop(funded);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn exact_scratch_and_output_refusals_preserve_original_credit_and_retry() {
    let numbers = [7, 8, 7];
    let large = AllocationBudget::new(4096);
    let measure = histogram(&numbers, Some(&large)).unwrap();
    let output_bytes = measure.0.as_ref().unwrap().allocation_bytes();
    drop(measure);
    let scratch_bytes = std::mem::size_of_val(&numbers);
    let budget = AllocationBudget::new(0);
    assert!(
        matches!(histogram(&numbers, Some(&budget)), Err(VMError::AllocationDeferred(AllocationRefusal::ExceedsLimit { requested_bytes, .. })) if requested_bytes == scratch_bytes)
    );
    assert_eq!(budget.peak_reserved_bytes(), 0);
    budget.set_limit_bytes(scratch_bytes + output_bytes - 1);
    assert!(
        matches!(histogram(&numbers, Some(&budget)), Err(VMError::AllocationDeferred(AllocationRefusal::Capacity { requested_bytes, .. })) if requested_bytes == output_bytes)
    );
    assert_eq!(budget.peak_reserved_bytes(), scratch_bytes);
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(scratch_bytes + output_bytes);
    let occupied = budget.try_reserve_bytes(budget.limit_bytes()).unwrap();
    assert!(matches!(
        histogram(&numbers, Some(&budget)),
        Err(VMError::AllocationDeferred(
            AllocationRefusal::Capacity { .. }
        ))
    ));
    drop(occupied);
    let result = histogram(&numbers, Some(&budget)).unwrap();
    assert_eq!(budget.reserved_bytes(), output_bytes);
    drop(result);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn zero_retention_eviction_shrink_and_unwind_keep_final_histogram_owner() {
    let _limits = crate::ivm_cache::CacheLimitsGuard::new(crate::ivm_cache::CacheLimits {
        capacity: 0,
        max_bytes: 0,
        max_decoded_ops: 0,
    });
    let budget = AllocationBudget::new(4096);
    let source = histogram(&[7, 7], Some(&budget)).unwrap();
    assert!(!source.try_retain());
    let bytes = budget.reserved_bytes();
    let cached = source.clone().into_cache_owner();
    let borrowed = cached.clone();
    assert_eq!(budget.reserved_bytes(), bytes);
    drop((cached, source));
    budget.set_limit_bytes(0);
    assert_eq!(budget.reserved_bytes(), bytes);
    assert_eq!(
        borrowed[0],
        SyscallUsage {
            number: 7,
            count: 2
        }
    );
    assert!(
        catch_unwind(AssertUnwindSafe(move || {
            let _last = borrowed;
            panic!("analysis borrower unwinds");
        }))
        .is_err()
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn empty_histogram_has_no_shell_and_inexact_accumulation_reclaims_scratch() {
    let zero = AllocationBudget::new(0);
    let empty = histogram(&[], Some(&zero)).unwrap();
    assert!(empty.is_empty());
    assert!(empty.0.is_none());
    assert!(empty.try_retain());
    assert_eq!(zero.peak_reserved_bytes(), 0);
    let budget = AllocationBudget::new(4096);
    let mut short = UsageScratch::new(2, Some(&budget)).unwrap();
    short.push(7).unwrap();
    assert!(matches!(short.finish(), Err(VMError::DecodeError)));
    assert_eq!(budget.reserved_bytes(), 0);
    let mut full = UsageScratch::new(1, Some(&budget)).unwrap();
    full.push(7).unwrap();
    assert!(matches!(full.push(8), Err(VMError::DecodeError)));
    assert_eq!(
        &*full.finish().unwrap(),
        &[SyscallUsage {
            number: 7,
            count: 1
        }]
    );
    assert_eq!(budget.reserved_bytes(), 0);
}
