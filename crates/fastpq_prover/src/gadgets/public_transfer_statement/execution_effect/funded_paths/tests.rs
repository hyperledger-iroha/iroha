//! Exact legacy interval-map paths/work/refusals and fixed AVL custody controls.

use std::collections::{BTreeMap, BTreeSet};

use super::super::super::allocate_path;
use super::*;

fn arena(keys: usize) -> (AllocationBudget, FundedPaths) {
    let bytes = FundedPaths::allocation_bytes(keys).unwrap();
    let budget = AllocationBudget::new(bytes);
    let mut reservation = budget.try_reserve_bytes(bytes).unwrap();
    let paths = FundedPaths::from_reservation(keys, &budget, &mut reservation).unwrap();
    assert_eq!(reservation.remaining_bytes(), 0);
    (budget, paths)
}

fn audit(paths: &FundedPaths) -> Vec<(u32, u32)> {
    fn walk(
        paths: &FundedPaths,
        index: usize,
        seen: &mut BTreeSet<usize>,
        rows: &mut Vec<(u32, u32)>,
    ) -> u8 {
        if index == NONE {
            return 0;
        }
        assert!(seen.insert(index), "live node reused/cycle");
        let node = paths.nodes.as_slice()[index];
        assert!(node.start <= node.end);
        let left = walk(paths, node.left, seen, rows);
        rows.push((node.start, node.end));
        let right = walk(paths, node.right, seen, rows);
        assert_eq!(node.height, 1 + left.max(right));
        assert!((i16::from(left) - i16::from(right)).abs() <= 1);
        node.height
    }
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    let height = walk(paths, paths.root, &mut seen, &mut rows);
    assert_eq!(seen.len(), paths.live);
    assert!(paths.live <= paths.key_count);
    for pair in rows.windows(2) {
        assert!(
            pair[0].1.checked_add(1).unwrap() < pair[1].0,
            "sorted maximal disjoint intervals"
        );
    }
    let mut free = paths.free;
    while free != NONE {
        assert!(seen.insert(free), "free node reused/cycle/live alias");
        let node = paths.nodes.as_slice()[free];
        assert_eq!(
            (node.start, node.end, node.right, node.height),
            (0, 0, NONE, 0)
        );
        free = node.left;
    }
    assert_eq!(seen.len(), paths.nodes.as_slice().len());
    assert!(paths.nodes.as_slice().len() <= paths.nodes.capacity());
    if paths.live == 0 {
        assert_eq!(height, 0);
    } else {
        assert!(u32::from(height) <= 2 * (usize::BITS - paths.live.leading_zeros()));
    }
    rows
}

fn compare(sequence: &[u32], maximum: usize, initial_steps: usize) {
    let (budget, mut paths) = arena(sequence.len());
    let pointer = paths.nodes.as_slice().as_ptr();
    let mut old = BTreeMap::new();
    let (mut before_steps, mut after_steps) = (initial_steps, initial_steps);
    for &base in sequence {
        let state_before = audit(&paths);
        let expected = allocate_path(&mut old, base, sequence.len(), &mut before_steps, maximum)
            .map_err(|error| error.to_string());
        let actual = paths
            .allocate(base, &mut after_steps, maximum)
            .map_err(|error| error.to_string());
        assert_eq!(actual, expected, "base {base}, cap {maximum}");
        assert_eq!(after_steps, before_steps);
        assert_eq!(
            audit(&paths),
            old.iter()
                .map(|(&start, &end)| (start, end))
                .collect::<Vec<_>>()
        );
        assert_eq!(paths.nodes.as_slice().as_ptr(), pointer);
        assert_eq!(
            budget.reserved_bytes(),
            FundedPaths::allocation_bytes(sequence.len()).unwrap()
        );
        if actual.is_err() {
            assert_eq!(
                audit(&paths),
                state_before,
                "failed lookup must not mutate intervals"
            );
            break;
        }
    }
    drop(paths);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn exhaustive_small_sequences_preserve_paths_steps_and_each_budget_refusal() {
    const BASES: [u32; 5] = [0, 1, 2, u32::MAX - 1, u32::MAX];
    for count in 0..=5_u32 {
        for mut code in 0..5_usize.pow(count) {
            let mut sequence = Vec::new();
            for _ in 0..count {
                sequence.push(BASES[code % BASES.len()]);
                code /= BASES.len();
            }
            for limit in 0..=sequence.len() * 4 {
                compare(&sequence, limit, 0);
            }
            compare(&sequence, usize::MAX, 7);
        }
    }
}

#[test]
fn sorted_reverse_random_and_bridge_sequences_keep_logarithmic_balance() {
    let ascending: Vec<_> = (0..512_u32).map(|value| value * 2).collect();
    let descending: Vec<_> = ascending.iter().rev().copied().collect();
    let mut bridges = ascending.clone();
    bridges.extend((0..511_u32).map(|value| value * 2 + 1));
    let mut state = 0x91a2_3b4c_u32;
    let random: Vec<_> = (0..1024)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        })
        .collect();
    for sequence in [ascending, descending, bridges, random] {
        compare(&sequence, usize::MAX, 0);
    }
}

#[test]
fn coalescing_two_neighbours_reclaims_both_slots_before_single_reuse() {
    let (budget, mut paths) = arena(9);
    let mut steps = 0;
    for base in [10, 30, 20, 40, 0, 2] {
        paths.allocate(base, &mut steps, usize::MAX).unwrap();
    }
    let allocated = paths.nodes.as_slice().len();
    let live = paths.live;
    paths.allocate(1, &mut steps, usize::MAX).unwrap();
    assert_eq!(paths.nodes.as_slice().len(), allocated);
    assert_eq!(paths.live, live - 1);
    assert_ne!(paths.free, NONE);
    assert!(audit(&paths).contains(&(0, 2)));
    paths.allocate(50, &mut steps, usize::MAX).unwrap();
    assert_eq!(paths.nodes.as_slice().len(), allocated);
    assert_eq!(paths.free, NONE);
    audit(&paths);
    drop(paths);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn deletion_with_two_children_preserves_successor_and_free_chain() {
    let (_, mut paths) = arena(9);
    let mut steps = 0;
    for base in [40, 20, 60, 10, 30, 50, 70] {
        paths.allocate(base, &mut steps, usize::MAX).unwrap();
    }
    let root = paths.nodes.as_slice()[paths.root];
    assert_ne!(root.left, NONE);
    assert_ne!(root.right, NONE);
    let mut expected = audit(&paths);
    expected.retain(|(start, _)| *start != root.start);
    paths.root = paths.remove(paths.root, root.start);
    assert_eq!(audit(&paths), expected);
    let allocated = paths.nodes.as_slice().len();
    paths.allocate(80, &mut steps, usize::MAX).unwrap();
    assert_eq!(paths.nodes.as_slice().len(), allocated);
    audit(&paths);
}

#[test]
fn empty_probe_full_space_and_step_overflow_match_original_refusals() {
    compare(&[], 0, 0);
    compare(&[0], usize::MAX, usize::MAX);
    let (_, mut empty) = arena(0);
    let (mut old_steps, mut new_steps) = (0, 0);
    let old = allocate_path(&mut BTreeMap::new(), 0, 0, &mut old_steps, usize::MAX).unwrap_err();
    let new = empty.allocate(0, &mut new_steps, usize::MAX).unwrap_err();
    assert_eq!(old.to_string(), new.to_string());
    assert_eq!((old_steps, new_steps), (1, 1));
    let (_, mut full) = arena(1);
    // Synthetic full-space state isolates the unreachable-in-small-fixtures
    // checked wrap endpoint; it is not an authenticated source construction.
    full.root = full.acquire(0, u32::MAX);
    for cap in [0, 1, 2, usize::MAX] {
        let mut old = BTreeMap::from([(0, u32::MAX)]);
        let (mut old_steps, mut new_steps) = (0, 0);
        let expected = allocate_path(&mut old, u32::MAX, 1, &mut old_steps, cap).unwrap_err();
        let actual = full.allocate(u32::MAX, &mut new_steps, cap).unwrap_err();
        assert_eq!(actual.to_string(), expected.to_string());
        assert_eq!(old_steps, new_steps);
        assert_eq!(audit(&full), vec![(0, u32::MAX)]);
    }
}

#[test]
fn arena_layout_refusal_pool_identity_and_drop_preserve_original_credit() {
    assert!(FundedPaths::allocation_bytes(usize::MAX).is_err());
    assert_eq!(FundedPaths::allocation_bytes(0).unwrap(), 0);
    let bytes = FundedPaths::allocation_bytes(8).unwrap();
    assert_eq!(bytes, Layout::array::<Interval>(8).unwrap().size());
    let budget = AllocationBudget::new(bytes + 17);
    let foreign = AllocationBudget::new(bytes + 17);
    let mut reservation = budget.try_reserve_bytes(bytes - 1).unwrap();
    assert!(matches!(
        FundedPaths::from_reservation(8, &budget, &mut reservation),
        Err(Error::AllocationBacking(
            iroha_allocation::PrepaidBufferError::Reservation(_)
        ))
    ));
    assert_eq!(reservation.remaining_bytes(), bytes - 1);
    assert!(matches!(
        FundedPaths::from_reservation(8, &foreign, &mut reservation),
        Err(Error::AllocationForeignPool)
    ));
    assert_eq!(reservation.remaining_bytes(), bytes - 1);
    drop(reservation);
    assert_eq!(budget.reserved_bytes(), 0);
    let mut reservation = budget.try_reserve_bytes(bytes + 17).unwrap();
    let paths = FundedPaths::from_reservation(8, &budget, &mut reservation).unwrap();
    assert!(paths.nodes.belongs_to(&budget));
    assert_eq!(reservation.remaining_bytes(), 17);
    drop(reservation);
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(paths);
    assert_eq!(budget.reserved_bytes(), 0);
}
