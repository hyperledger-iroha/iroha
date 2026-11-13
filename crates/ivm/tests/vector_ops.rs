//! Host-width vector helper tests: funded destinations, lane arithmetic and output geometry.
#[test]
fn test_auto_vector_helpers() {
    let lanes = ivm::simd_lanes();
    let a: Vec<u32> = (0..lanes as u32).collect();
    let b: Vec<u32> = vec![1; lanes];
    let plan = ivm::execution_memory::ExecutionMemoryPlan::array::<u32>(lanes).unwrap();
    let budget = iroha_allocation::AllocationBudget::new(plan.requested_bytes());
    let mut lease = ivm::execution_memory::ExecutionMemoryLease::reserve(&budget, plan).unwrap();
    let mut output = ivm::zero_vector(lanes, &mut lease).unwrap();
    drop(lease);
    assert_eq!(budget.reserved_bytes(), lanes * std::mem::size_of::<u32>());
    ivm::vadd32_auto_into(&a, &b, output.as_mut_slice());
    let out = output.as_slice();
    assert_eq!(out.len(), lanes);
    for i in 0..lanes {
        assert_eq!(out[i], a[i].wrapping_add(b[i]));
    }
}
#[cfg(target_os = "macos")]
#[test]
fn test_metal_bit_pipeline_loads_bundled_library() {
    if !ivm::metal_available() {
        return;
    }
    ivm::release_metal_state();
    assert!(ivm::metal_available());
    let a = [1u32, 2, 3, 4];
    let b = [5u32, 6, 7, 8];
    assert_eq!(ivm::vand(a, b), [1, 2, 3, 0]);
    assert_eq!(ivm::vxor(a, b), [4, 4, 4, 12]);
    assert_eq!(ivm::vor(a, b), [5, 6, 7, 12]);
}

#[test]
fn funded_auto_vector_destinations_preserve_all_operations_and_final_owner_charge() {
    use ivm::execution_memory::{ExecutionMemoryLease, ExecutionMemoryPlan};
    let lanes = 10;
    let plan = ExecutionMemoryPlan::array::<u32>(lanes).unwrap();
    let budget = iroha_allocation::AllocationBudget::new(plan.requested_bytes());
    let mut lease = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
    let mut output = ivm::zero_vector(lanes, &mut lease).unwrap();
    drop(lease);
    assert_eq!(output.as_slice(), vec![0; lanes]);
    let left: Vec<_> = (0..lanes)
        .map(|i| u32::MAX.wrapping_sub(i as u32))
        .collect();
    let right: Vec<_> = (0..lanes).map(|i| (i as u32).wrapping_add(2)).collect();
    ivm::vadd32_auto_into(&left, &right, output.as_mut_slice());
    assert!(output.as_slice().iter().all(|&v| v == 1));
    for (operation, scalar) in [
        (
            ivm::vand_auto_into as fn(&[u32], &[u32], &mut [u32]),
            (|a, b| a & b) as fn(u32, u32) -> u32,
        ),
        (ivm::vxor_auto_into, |a, b| a ^ b),
        (ivm::vor_auto_into, |a, b| a | b),
    ] {
        operation(&left, &right, output.as_mut_slice());
        for i in 0..lanes {
            assert_eq!(output.as_slice()[i], scalar(left[i], right[i]));
        }
    }
    ivm::vrot32_auto_into(&left, 13, output.as_mut_slice());
    for (i, value) in left.iter().enumerate() {
        assert_eq!(output.as_slice()[i], value.rotate_left(13));
    }
    ivm::vadd64_auto_into(&left, &right, output.as_mut_slice());
    for i in (0..lanes).step_by(2) {
        let a = u64::from(left[i]) | (u64::from(left[i + 1]) << 32);
        let b = u64::from(right[i]) | (u64::from(right[i + 1]) << 32);
        let expected = a.wrapping_add(b);
        assert_eq!(output.as_slice()[i], expected as u32);
        assert_eq!(output.as_slice()[i + 1], (expected >> 32) as u32);
    }
    budget.set_limit_bytes(0);
    assert_eq!(budget.reserved_bytes(), plan.requested_bytes());
    drop(output);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn zero_vector_cannot_readmit_or_grow_a_parent_with_insufficient_credit() {
    use ivm::execution_memory::{ExecutionMemoryLease, ExecutionMemoryPlan};
    let required = ExecutionMemoryPlan::array::<u32>(ivm::simd_lanes()).unwrap();
    let budget = iroha_allocation::AllocationBudget::new(required.requested_bytes());
    let mut empty = ExecutionMemoryLease::reserve(&budget, ExecutionMemoryPlan::default()).unwrap();
    assert!(ivm::zero_vector(ivm::simd_lanes(), &mut empty).is_err());
    assert_eq!(empty.remaining_bytes(), 0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn host_width_output_geometry_is_checked_before_any_vector_write() {
    let lanes = 10;
    let left = vec![1; lanes];
    let right = vec![2; lanes];
    let mut short = vec![77; lanes - 1];
    let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ivm::vadd32_auto_into(&left, &right, &mut short);
    }));
    assert!(refused.is_err());
    assert_eq!(short, vec![77; lanes - 1]);
}
