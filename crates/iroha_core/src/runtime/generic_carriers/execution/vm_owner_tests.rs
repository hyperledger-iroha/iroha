//! Actual allocator observation of the external-host VM shell used by carrier execution.

use iroha_allocation::AllocationBudget;
use ivm::{IVM, VMError, error::VmDeferral};

const LIMIT: usize = 512 * 1024 * 1024;

#[test]
fn external_host_vm_omits_exactly_the_unused_default_host_graph() {
    let budget = AllocationBudget::new(LIMIT);
    // Warm thread-local constructor controls before observing physical allocations.
    drop(IVM::try_new_with_memory_budget(100, &budget).unwrap());
    drop(IVM::try_new_for_external_host_with_memory_budget(100, &budget).unwrap());
    let mut ordinary = None;
    let ordinary_allocations = crate::test_allocations::allocations_during(|| {
        ordinary = Some(IVM::try_new_with_memory_budget(100, &budget));
    });
    let mut ordinary = ordinary.unwrap().unwrap();
    let ordinary_credit = budget.reserved_bytes();
    assert!(
        ordinary
            .host_mut_any()
            .unwrap()
            .downcast_mut::<ivm::host::DefaultHost>()
            .is_some()
    );
    drop(ordinary);
    assert_eq!(budget.reserved_bytes(), 0);

    let mut external = None;
    let external_allocations = crate::test_allocations::allocations_during(|| {
        external = Some(IVM::try_new_for_external_host_with_memory_budget(
            100, &budget,
        ));
    });
    let mut external = external.unwrap().unwrap();
    assert!(external.host_mut_any().is_none());
    assert!(external.allocation_budget().unwrap().same_pool(&budget));
    assert_eq!(budget.reserved_bytes(), ordinary_credit);
    // The ordinary constructor retains its String, Arc policy control and boxed dispatcher.
    // The new constructor omits all three allocations, without allocating a substitute owner.
    assert_eq!(ordinary_allocations, external_allocations + 3);
    drop(external);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn external_host_vm_original_credit_refusal_allocates_no_shell_or_replacement_pool() {
    let budget = AllocationBudget::new(0);
    let mut result = None;
    let allocations = crate::test_allocations::allocations_during(|| {
        result = Some(IVM::try_new_for_external_host_with_memory_budget(
            100, &budget,
        ));
    });
    assert_eq!(allocations, 0);
    assert!(matches!(
        result.unwrap(),
        Err(VMError::Deferred(VmDeferral::AllocationDeferred(_)))
    ));
    assert_eq!(budget.reserved_bytes(), 0);
}
