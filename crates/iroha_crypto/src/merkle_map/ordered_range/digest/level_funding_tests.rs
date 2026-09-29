//! Exact prepaid geometry, local refusal and unwind controls for Merkle levels.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn entries(count: usize, budget: &AllocationBudget) -> Vec<DigestEntry> {
    (0..count)
        .map(|index| DigestEntry {
            key: ChargedBuffer::new(0, budget).unwrap(),
            value_digest: Hash::new(index.to_le_bytes()),
        })
        .collect()
}

#[test]
fn checked_geometry_matches_every_concrete_level_and_outer_layout() {
    for count in [0, 1, 2, 3, 7, MAX_NORITO_TREE_ENTRIES] {
        let geometry = Geometry::for_entries(count).unwrap();
        let mut length = geometry.leaves;
        let mut bytes = Layout::array::<ChargedBuffer<Hash>>(geometry.levels)
            .unwrap()
            .size();
        let mut levels = 0;
        while length > 0 {
            bytes += Layout::array::<Hash>(length).unwrap().size();
            levels += 1;
            length /= 2;
        }
        assert_eq!(geometry.levels, levels);
        assert_eq!(geometry.bytes, bytes);
    }
    assert!(matches!(
        Geometry::for_entries(MAX_NORITO_TREE_ENTRIES + 1),
        Err(NoritoKeyRangeError::Capacity)
    ));
    assert!(matches!(
        Geometry::for_entries(usize::MAX),
        Err(NoritoKeyRangeError::Capacity)
    ));
}

#[test]
fn prepaid_levels_survive_budget_shrink_without_reacquisition_or_growth() {
    for count in [0, 1, 3, 5, 8] {
        let geometry = Geometry::for_entries(count).unwrap();
        let budget = AllocationBudget::new(geometry.bytes);
        let rows = entries(count, &budget);
        let mut prepaid = budget.try_reserve_bytes(geometry.bytes).unwrap();
        budget.set_limit_bytes(0);
        let levels = build_prepaid(&rows, geometry, &mut prepaid).unwrap();
        assert_eq!(prepaid.remaining_bytes(), 0);
        assert_eq!(levels.capacity(), geometry.levels);
        assert_eq!(levels.as_slice().len(), geometry.levels);
        let mut length = geometry.leaves;
        for level in levels.as_slice() {
            assert_eq!(level.capacity(), length);
            assert_eq!(level.as_slice().len(), length);
            length /= 2;
        }
        if count > 0 {
            let leaves = levels.as_slice()[0].as_slice();
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(
                    leaves[index],
                    leaf_hash(
                        u32::try_from(index).expect("bounded fixture leaf index"),
                        digest_frame(KEY_DOMAIN, row.key.as_slice()),
                        row.value_digest
                    )
                );
            }
            for (index, leaf) in leaves.iter().enumerate().skip(count) {
                assert_eq!(
                    *leaf,
                    pad_hash(u32::try_from(index).expect("bounded fixture leaf index"))
                );
            }
            for (level, pair) in levels.as_slice().windows(2).enumerate() {
                for (children, parent) in pair[0].as_slice().chunks_exact(2).zip(pair[1].as_slice())
                {
                    assert_eq!(
                        *parent,
                        branch_hash(
                            u8::try_from(level).expect("bounded fixture tree height"),
                            children[0],
                            children[1]
                        )
                    );
                }
            }
        }
        drop(prepaid);
        assert_eq!(budget.reserved_bytes(), geometry.bytes);
        drop(levels);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn short_prepaid_parent_is_unchanged_and_pool_refusal_retains_no_levels() {
    let geometry = Geometry::for_entries(3).unwrap();
    let budget = AllocationBudget::new(geometry.bytes);
    let rows = entries(3, &budget);
    let mut short = budget.try_reserve_bytes(geometry.bytes - 1).unwrap();
    assert!(matches!(
        build_prepaid(&rows, geometry, &mut short),
        Err(NoritoKeyRangeError::PrepaidCapacity(InsufficientReservation {
            requested_bytes, remaining_bytes
        })) if requested_bytes == geometry.bytes && remaining_bytes == geometry.bytes - 1
    ));
    assert_eq!(short.remaining_bytes(), geometry.bytes - 1);
    assert_eq!(budget.reserved_bytes(), geometry.bytes - 1);
    assert!(matches!(
        build(&rows, &budget),
        Err(NoritoKeyRangeError::Admission(
            AllocationRefusal::Capacity { .. }
        ))
    ));
    assert_eq!(budget.reserved_bytes(), geometry.bytes - 1);
    drop(short);
    let levels = build(&rows, &budget).unwrap();
    assert_eq!(budget.reserved_bytes(), geometry.bytes);
    drop(levels);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn completed_level_owners_release_all_original_credit_during_unwind() {
    let geometry = Geometry::for_entries(3).unwrap();
    let budget = AllocationBudget::new(geometry.bytes);
    let rows = entries(3, &budget);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _levels = build(&rows, &budget).unwrap();
        assert_eq!(budget.reserved_bytes(), geometry.bytes);
        panic!("exercise complete level owner cleanup");
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn underestimated_level_geometry_refuses_extra_rows_without_growing_or_leaking() {
    let geometry = Geometry::for_entries(1).unwrap();
    let budget = AllocationBudget::new(geometry.bytes);
    let rows = entries(2, &budget);
    let mut prepaid = budget.try_reserve_bytes(geometry.bytes).unwrap();
    assert!(matches!(
        build_prepaid(&rows, geometry, &mut prepaid),
        Err(NoritoKeyRangeError::Capacity)
    ));
    assert_eq!(prepaid.remaining_bytes(), 0);
    assert_eq!(
        budget.reserved_bytes(),
        0,
        "both allocated buffers were reclaimed"
    );
}
