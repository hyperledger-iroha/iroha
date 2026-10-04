//! Original-pool custody, concrete demand and flat ancestor boundary controls.

use super::super::checked_tests::{Table, two_pairs};
use super::*;

fn limits(updates: usize) -> TransferSmtBuildLimits {
    TransferSmtBuildLimits::for_update_limit(updates).unwrap()
}

fn output_bytes(updates: usize) -> usize {
    Layout::array::<[TransferSmtWitness; 2]>(updates / 2)
        .unwrap()
        .size()
        + Layout::array::<AllocationCharge>(updates * 2)
            .unwrap()
            .size()
        + updates
            * (Layout::array::<u8>(4).unwrap().size()
                + Layout::array::<[u8; 32]>(HEIGHT).unwrap().size())
}

#[test]
fn checked_layout_demand_covers_exact_backings_and_rejects_overflow() {
    for (updates, keys) in [(0, 0), (2, 1), (2, 2), (4, 2), (64, 32)] {
        let limits = limits(updates);
        let actual = limits.allocation_bytes(updates, keys).unwrap();
        let expected = Layout::array::<u32>(keys).unwrap().size()
            + Layout::array::<Node>(keys * (HEIGHT + 1)).unwrap().size()
            + Layout::array::<bool>(keys).unwrap().size()
            + Layout::array::<bool>(updates).unwrap().size()
            + output_bytes(updates);
        assert_eq!(actual, expected);
    }
    let huge = TransferSmtBuildLimits {
        max_updates: usize::MAX,
        max_unique_keys: usize::MAX,
        max_retained_nodes: usize::MAX,
        max_sibling_hashes: usize::MAX,
        max_node_hashes: usize::MAX,
    };
    assert!(huge.allocation_bytes(2, usize::MAX).is_err());
    assert!(huge.allocation_bytes(usize::MAX - 1, 1).is_err());
    assert!(limits(4).allocation_bytes(3, 1).is_err());
    assert!(limits(4).allocation_bytes(0, 1).is_err());
    assert!(limits(4).allocation_bytes(2, 0).is_err());
    assert_eq!(limits(0).allocation_bytes(0, 0).unwrap(), 0);
}

#[test]
fn private_witnesses_hold_exact_original_credit_through_moves_and_borrows() {
    let table = two_pairs();
    let demand = limits(4).allocation_bytes(4, table.keys.len()).unwrap();
    let untouched = 37;
    let budget = AllocationBudget::new(demand + untouched);
    let mut reservation = budget.try_reserve_bytes(demand + untouched).unwrap();
    let built = derive(&table, limits(4), &budget, &mut reservation).unwrap();
    assert_eq!(reservation.remaining_bytes(), untouched);
    assert_eq!(budget.reserved_bytes(), untouched + output_bytes(4));
    assert!(built.pairs.pairs.belongs_to(&budget));
    assert!(built.pairs.nested.belongs_to(&budget));
    assert!(
        built
            .pairs
            .nested
            .as_slice()
            .iter()
            .all(|charge| charge.belongs_to(&budget))
    );
    let pairs_ptr = built.pairs().as_ptr();
    let path_ptr = built.pairs()[0][0].path_bits.as_ptr();
    let siblings_ptr = built.pairs()[0][0].siblings.as_ptr();
    let moved = built;
    assert_eq!(moved.pairs().as_ptr(), pairs_ptr);
    assert_eq!(moved.pairs()[0][0].path_bits.as_ptr(), path_ptr);
    assert_eq!(moved.pairs()[0][0].siblings.as_ptr(), siblings_ptr);
    assert_eq!(moved.intermediate_roots().len(), 1);
    for pair in moved.pairs() {
        for witness in pair {
            assert_eq!(
                (witness.path_bits.len(), witness.path_bits.capacity()),
                (4, 4)
            );
            assert_eq!(
                (witness.siblings.len(), witness.siblings.capacity()),
                (HEIGHT, HEIGHT)
            );
        }
    }
    drop(reservation);
    assert_eq!(budget.reserved_bytes(), output_bytes(4));
    drop(moved);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn short_reservation_and_foreign_pool_refuse_before_consuming_credit() {
    let table = two_pairs();
    let demand = limits(4).allocation_bytes(4, table.keys.len()).unwrap();
    let budget = AllocationBudget::new(demand);
    let foreign = AllocationBudget::new(demand);
    let mut short = budget.try_reserve_bytes(demand - 1).unwrap();
    assert!(matches!(derive(&table, limits(4), &budget, &mut short),
        Err(crate::Error::AllocationReservation(refusal))
        if refusal.requested_bytes == demand && refusal.remaining_bytes == demand - 1));
    assert_eq!(short.remaining_bytes(), demand - 1);
    assert_eq!(budget.reserved_bytes(), demand - 1);
    assert!(matches!(
        derive(&table, limits(4), &foreign, &mut short),
        Err(crate::Error::AllocationForeignPool)
    ));
    assert_eq!(short.remaining_bytes(), demand - 1);
    assert_eq!(foreign.reserved_bytes(), 0);
    drop(short);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn late_chronology_failure_refunds_prior_pairs_and_in_flight_witness() {
    // Last leg fails after a complete returned-to-collector pair and one live
    // first-leg witness. Both canonical nested owners must deallocate on error.
    let mut table = two_pairs();
    let last = table.pairs[1].row_indices[1];
    let wrong = super::super::super::digest_limbs(Hash::new(b"late funded mismatch").into());
    table.rows[last].update.old_leaf = wrong;
    table.pairs[1].updates[1].old_leaf = wrong;
    let demand = limits(4).allocation_bytes(4, table.keys.len()).unwrap();
    let budget = AllocationBudget::new(demand);
    let mut reservation = budget.try_reserve_bytes(demand).unwrap();
    assert!(
        matches!(derive(&table, limits(4), &budget, &mut reservation),
        Err(crate::Error::TransferInvariant { details })
        if details == "SMT chronological pre-leaf does not match current state")
    );
    assert_eq!(reservation.remaining_bytes(), 0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn empty_sequence_needs_zero_bytes_and_preserves_root_check() {
    let mut table = Table {
        inputs: PublicInputs::default(),
        keys: Vec::new(),
        rows: Vec::new(),
        pairs: Vec::new(),
    };
    let budget = AllocationBudget::new(0);
    let mut reservation = budget.try_reserve_bytes(0).unwrap();
    let built = derive(&table, limits(0), &budget, &mut reservation).unwrap();
    assert!(built.pairs().is_empty());
    assert_eq!(built.work(), TransferSmtBuildWork::default());
    assert_eq!(
        built.roots(),
        (table.inputs.old_root, table.inputs.new_root)
    );
    assert_eq!(budget.reserved_bytes(), 0);
    table.inputs.new_root = Hash::new(b"changed empty funded root").into();
    assert!(derive(&table, limits(0), &budget, &mut reservation).is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

fn recursive_root(paths: &[u32], level: usize, before: Hash, changed: Option<(u32, Hash)>) -> Hash {
    if paths.is_empty() {
        return padding(level);
    }
    if level == 0 {
        assert_eq!(paths.len(), 1);
        return changed
            .filter(|(path, _)| *path == paths[0])
            .map_or(before, |(_, hash)| hash);
    }
    let split = paths.partition_point(|path| path >> (level - 1) & 1 == 0);
    let left = recursive_root(&paths[..split], level - 1, before, changed);
    let right = recursive_root(&paths[split..], level - 1, before, changed);
    Hash::new_from_chunks(&[NODE_DOMAIN, left.as_ref(), right.as_ref()])
}

#[test]
fn flat_levels_cover_extreme_paths_and_match_independent_root_folds() {
    for paths in [vec![0], vec![u32::MAX], vec![0, 1, 1 << 31, u32::MAX]] {
        let count: usize = (0..=HEIGHT)
            .map(|level| {
                let mut prefixes: Vec<_> =
                    paths.iter().map(|path| u64::from(*path) >> level).collect();
                prefixes.dedup();
                prefixes.len()
            })
            .sum();
        let bytes = Layout::array::<Node>(count).unwrap().size();
        let budget = AllocationBudget::new(bytes + 4 + HEIGHT * 32);
        let mut reservation = budget.try_reserve_bytes(bytes + 4 + HEIGHT * 32).unwrap();
        let mut tree = Tree::new(&paths, count, &mut reservation).unwrap();
        let before = Hash::new(b"flat extreme old leaf");
        let after = Hash::new(b"flat extreme new leaf");
        for path in &paths {
            tree.set(0, *path, before);
        }
        tree.seed();
        assert_eq!(tree.root(), recursive_root(&paths, HEIGHT, before, None));
        assert_eq!(tree.nodes.as_slice().len(), count);
        assert_eq!(tree.hashes, count - paths.len());
        assert_eq!(tree.start[HEIGHT + 1] - tree.start[HEIGHT], 1);
        assert_eq!(tree.nodes.as_slice()[tree.start[HEIGHT]].path, 0);
        let path = *paths.last().unwrap();
        let update = PublicUpdate {
            path,
            old_leaf: super::super::super::digest_limbs(before.into()),
            new_leaf: super::super::super::digest_limbs(after.into()),
        };
        let funded = tree.update(update, &mut reservation).unwrap();
        assert_eq!(
            tree.root(),
            recursive_root(&paths, HEIGHT, before, Some((path, after)))
        );
        assert_eq!(funded.witness.path_bits, path.to_le_bytes());
        for (leaf, expected) in [
            (before, funded.witness.root_before),
            (after, funded.witness.root_after),
        ] {
            let mut hash = leaf;
            for (level, sibling) in funded.witness.siblings.iter().enumerate() {
                let sibling = Hash::prehashed(*sibling);
                let (left, right) = if path >> level & 1 == 0 {
                    (hash, sibling)
                } else {
                    (sibling, hash)
                };
                hash = Hash::new_from_chunks(&[NODE_DOMAIN, left.as_ref(), right.as_ref()]);
            }
            assert_eq!(<[u8; 32]>::from(hash), expected);
        }
        assert_eq!(tree.hashes, count - paths.len() + HEIGHT);
        drop(reservation);
        drop(tree);
        assert_eq!(budget.reserved_bytes(), 4 + HEIGHT * 32);
        drop(funded);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn sibling_reservation_failure_refunds_path_before_any_tree_mutation() {
    let nodes = HEIGHT + 1;
    let tree_bytes = Layout::array::<Node>(nodes).unwrap().size();
    let budget = AllocationBudget::new(tree_bytes + 4);
    let mut reservation = budget.try_reserve_bytes(tree_bytes + 4).unwrap();
    let mut tree = Tree::new(&[u32::MAX], nodes, &mut reservation).unwrap();
    let before = Hash::new(b"funded partial old leaf");
    let after = Hash::new(b"funded partial new leaf");
    tree.set(0, u32::MAX, before);
    tree.seed();
    let root = tree.root();
    let hashes = tree.hashes;
    let update = PublicUpdate {
        path: u32::MAX,
        old_leaf: super::super::super::digest_limbs(before.into()),
        new_leaf: super::super::super::digest_limbs(after.into()),
    };
    assert!(matches!(tree.update(update, &mut reservation),
        Err(crate::Error::AllocationBacking(iroha_allocation::PrepaidBufferError::Reservation(refusal)))
        if refusal.requested_bytes == HEIGHT * 32 && refusal.remaining_bytes == 0));
    assert_eq!(tree.root(), root);
    assert_eq!(tree.hashes, hashes);
    assert_eq!(tree.get(0, u32::MAX), Some(before));
    assert_eq!(reservation.remaining_bytes(), 0);
    assert_eq!(budget.reserved_bytes(), tree_bytes);
    drop(tree);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn rooted_builder_mismatch_releases_its_complete_private_output() {
    use super::super::super::quantity_tests::{delta, fixture};
    use iroha_primitives::numeric::Quantity;

    let (claims, rows, inputs) = fixture(vec![delta(
        Quantity::from(20_u32),
        Quantity::from(5_u32),
        Quantity::from(3_u32),
    )]);
    let prepared = prepare_quantity_public_transfers(
        &rows,
        &claims,
        inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let demand = limits(2)
        .allocation_bytes(2, prepared.keys().len())
        .unwrap();
    let budget = AllocationBudget::new(demand);
    let mut reservation = budget.try_reserve_bytes(demand).unwrap();
    assert!(
        matches!(prepared.build_smt_witnesses(limits(2), &budget, &mut reservation),
        Err(crate::Error::TransferInvariant { details }) if details.contains("roots differ"))
    );
    assert_eq!(reservation.remaining_bytes(), 0);
    assert_eq!(budget.reserved_bytes(), 0);
}
