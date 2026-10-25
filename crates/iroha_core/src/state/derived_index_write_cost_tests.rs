//! Per-transaction write cost of the row-oriented derived membership indexes.
//!
//! The vendored concread B+tree copies a leaf, and each branch on its path, on
//! a block's first write to it, and a transaction clones a value before it
//! mutates it. A grouped `K -> BTreeSet<M>` index therefore cloned every member
//! of the touched group whenever one member changed. These tests count member
//! clones through the production row helpers and pin that a membership change
//! stays bounded by tree geometry, independent of the group size.

use std::{cell::Cell, collections::BTreeSet};

use mv::storage::Storage;

use super::{
    insert_index_member,
    range_bounds::{index_contains, index_members},
    remove_index_member,
};

thread_local! {
    static MEMBER_CLONES: Cell<usize> = const { Cell::new(0) };
}

/// Index member that counts its clones on the current thread.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CountedMember(u64);

impl Clone for CountedMember {
    fn clone(&self) -> Self {
        MEMBER_CLONES.with(|clones| clones.set(clones.get() + 1));
        Self(self.0)
    }
}

/// Count the member clones `operation` performs on this thread.
fn member_clones(operation: impl FnOnce()) -> usize {
    MEMBER_CLONES.with(|clones| clones.set(0));
    operation();
    MEMBER_CLONES.with(Cell::get)
}

const GROUP: u64 = 1;
const GROUP_SIZES: [u64; 3] = [16, 1_024, 16_384];
/// Clones allowed for one membership change: the transaction's own key, the
/// undo record, and copies of one leaf and the branches on its path. A tree
/// holding 16,384 rows is at most six levels deep with seven keys per node.
const ROW_WRITE_CLONE_BOUND: usize = 64;

/// Even members, so an odd member is absent and sorts inside the group.
fn seeded_members(group_size: u64) -> impl Iterator<Item = CountedMember> {
    (0..group_size).map(|member| CountedMember(member * 2))
}

/// One large group between two small neighbouring groups.
fn row_index(group_size: u64) -> Storage<(u64, CountedMember), ()> {
    let neighbours = [GROUP - 1, GROUP + 1]
        .into_iter()
        .flat_map(|key| seeded_members(8).map(move |member| (key, member)));
    seeded_members(group_size)
        .map(|member| (GROUP, member))
        .chain(neighbours)
        .map(|row| (row, ()))
        .collect()
}

/// Clones of one row insertion and its removal, each in its own block.
fn row_round_trip_clones(
    index: &Storage<(u64, CountedMember), ()>,
    member: &CountedMember,
) -> (usize, usize) {
    let inserted = member_clones(|| {
        let mut block = index.block();
        let mut transaction = block.transaction();
        assert!(insert_index_member(&mut transaction, &GROUP, member));
        assert!(!insert_index_member(&mut transaction, &GROUP, member));
        transaction.apply();
        block.commit();
    });
    let removed = member_clones(|| {
        let mut block = index.block();
        let mut transaction = block.transaction();
        assert!(remove_index_member(&mut transaction, &GROUP, member));
        assert!(!remove_index_member(&mut transaction, &GROUP, member));
        transaction.apply();
        block.commit();
    });
    (inserted, removed)
}

#[test]
fn row_membership_write_clones_do_not_grow_with_group_size() {
    for group_size in GROUP_SIZES {
        let index = row_index(group_size);
        let member = CountedMember(group_size | 1);
        let (inserted, removed) = row_round_trip_clones(&index, &member);
        assert!(
            inserted <= ROW_WRITE_CLONE_BOUND && removed <= ROW_WRITE_CLONE_BOUND,
            "group of {group_size}: insert cloned {inserted} members, remove cloned {removed}"
        );
        let view = index.view();
        assert!(!index_contains(&view, &GROUP, &member));
        assert!(
            index_members(&view, &GROUP)
                .map(|member| member.0)
                .eq((0..group_size).map(|member| member * 2))
        );
        assert_eq!(index_members(&view, &(GROUP + 1)).count(), 8);
    }
}

#[test]
fn grouped_reference_layout_clones_every_group_member() {
    for group_size in GROUP_SIZES {
        let index: Storage<u64, BTreeSet<CountedMember>> =
            [(GROUP, seeded_members(group_size).collect())]
                .into_iter()
                .collect();
        let clones = member_clones(|| {
            let mut block = index.block();
            let mut transaction = block.transaction();
            transaction
                .get_mut(&GROUP)
                .expect("seeded group")
                .insert(CountedMember(group_size | 1));
            transaction.apply();
            block.commit();
        });
        let group_size = usize::try_from(group_size).expect("test group size fits usize");
        assert!(
            clones >= group_size,
            "the counter must observe the grouped copy: {clones} clones for {group_size} members"
        );
    }
}
