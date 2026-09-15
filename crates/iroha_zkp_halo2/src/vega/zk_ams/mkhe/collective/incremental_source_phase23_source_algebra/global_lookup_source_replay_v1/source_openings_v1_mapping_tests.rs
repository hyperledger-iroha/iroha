//! Canonical source-opening coordinate, mapping, and shared-cache controls.

use super::*;

#[test]
fn source_and_inverse_packing_coordinates_are_exact_bijections() {
    let mut seen = [false; SOURCE_OPENING_SCALARS_PER_GROUP_V1];
    for block in 0..SOURCE_OPENING_BLOCKS_PER_GROUP_V1 {
        for coefficient in 0..SOURCE_OPENING_SCALARS_PER_BLOCK_V1 {
            let source_j = 256 * block + coefficient;
            let packing_k = source_to_packing_coordinate_v1(source_j).unwrap();
            assert_eq!(packing_k, 64 * coefficient + block);
            assert!(!seen[packing_k]);
            seen[packing_k] = true;
        }
    }
    assert!(!seen.contains(&false));
    assert!(source_to_packing_coordinate_v1(16_384).is_err());
    for ordinal in 0..SOURCE_OPENING_GROUP_COUNT_V1 {
        let coordinate = source_opening_group_coordinate_v1(ordinal).unwrap();
        assert_eq!(usize::from(coordinate.record), ordinal / 8);
        assert_eq!(usize::from(coordinate.group), ordinal % 8);
    }
    assert!(source_opening_group_coordinate_v1(344).is_err());
}
#[test]
fn mapping_and_context_kats_reject_order_duplicates_and_wrong_axes() {
    let groups: [u16; SOURCE_OPENING_GROUP_COUNT_V1] = core::array::from_fn(|index| index as u16);
    let source: [u16; SOURCE_OPENING_SCALARS_PER_GROUP_V1] =
        core::array::from_fn(|index| index as u16);
    let mapping = source_opening_mapping_digest_for_orders_v1(&groups, &source).unwrap();
    assert_eq!(mapping, exact_source_opening_mapping_digest_v1().unwrap());
    assert_eq!(
        hex::encode(mapping),
        "fcb9825186f9e7e51269d8df04cc22956143ba26485c194592458587ff51632d"
    );
    let mut reordered_groups = groups;
    reordered_groups.swap(0, 1);
    assert_ne!(
        source_opening_mapping_digest_for_orders_v1(&reordered_groups, &source).unwrap(),
        mapping
    );
    let mut duplicate_groups = groups;
    duplicate_groups[1] = 0;
    assert!(source_opening_mapping_digest_for_orders_v1(&duplicate_groups, &source).is_err());
    let mut reordered_source = source;
    reordered_source.swap(0, 1);
    assert_ne!(
        source_opening_mapping_digest_for_orders_v1(&groups, &reordered_source).unwrap(),
        mapping
    );
    let mut duplicate_source = source;
    duplicate_source[1] = 0;
    assert!(source_opening_mapping_digest_for_orders_v1(&groups, &duplicate_source).is_err());
    assert!(source_opening_mapping_digest_for_orders_v1(&groups[..343], &source).is_err());
    assert!(source_opening_mapping_digest_for_orders_v1(&groups, &source[..16_383]).is_err());
    let context = source_opening_context_digest_v1(
        &context_axes_v1(),
        GLOBAL_LOOKUP_TOPOLOGY_KAT_V1,
        mapping,
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
    )
    .unwrap();
    assert_eq!(
        hex::encode(context),
        "4e39e5279a38372551166def33c85008c7946128b9b95473d2315381c2712d81"
    );
    let mut changed = context_axes_v1();
    changed.source_receipt_digest[0] ^= 1;
    assert_ne!(
        source_opening_context_digest_v1(
            &changed,
            GLOBAL_LOOKUP_TOPOLOGY_KAT_V1,
            mapping,
            ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
        )
        .unwrap(),
        context
    );
    let mut wrong_basis = ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1;
    wrong_basis[0] ^= 1;
    assert!(
        source_opening_context_digest_v1(
            &context_axes_v1(),
            GLOBAL_LOOKUP_TOPOLOGY_KAT_V1,
            mapping,
            wrong_basis,
        )
        .is_err()
    );
}
#[test]
fn cached_canonical_mapping_is_shared_without_caching_mutable_order_validation() {
    let groups: [u16; SOURCE_OPENING_GROUP_COUNT_V1] = core::array::from_fn(|index| index as u16);
    let source: [u16; SOURCE_OPENING_SCALARS_PER_GROUP_V1] =
        core::array::from_fn(|index| index as u16);
    let expected = source_opening_mapping_digest_for_orders_v1(&groups, &source).unwrap();
    assert_eq!(exact_source_opening_mapping_digest_v1().unwrap(), expected);
    std::thread::scope(|scope| {
        let readers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    for _ in 0..16 {
                        assert_eq!(exact_source_opening_mapping_digest_v1().unwrap(), expected);
                    }
                })
            })
            .collect();
        for reader in readers {
            reader.join().unwrap();
        }
    });
    let mut changed = source;
    changed.swap(0, 1);
    assert_ne!(
        source_opening_mapping_digest_for_orders_v1(&groups, &changed).unwrap(),
        expected
    );
    changed[1] = changed[0];
    assert!(source_opening_mapping_digest_for_orders_v1(&groups, &changed).is_err());
    assert_eq!(exact_source_opening_mapping_digest_v1().unwrap(), expected);
}
