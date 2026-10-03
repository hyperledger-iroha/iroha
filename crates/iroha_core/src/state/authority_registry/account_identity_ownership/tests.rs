//! Exact native identity histories, bounded traversal and consumed account capture.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        State,
        authority_registry::{
            complete::capture_accounts_table_once,
            leaf::{LeafError, LeafLimits},
        },
    },
    test_allocations::allocations_during,
};
use iroha_crypto::Hash;
use iroha_data_model::account::AccountDetails;
use iroha_model_base::metadata::Metadata;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::Storage;
use std::collections::BTreeMap;

fn uaid() -> UniversalAccountId {
    UniversalAccountId::from_hash(Hash::new(b"checked identity uaid"))
}
fn opaque(index: u8) -> OpaqueAccountId {
    OpaqueAccountId::from_hash(Hash::new([index; 32]))
}
fn details(uaid: Option<UniversalAccountId>, ids: Vec<OpaqueAccountId>) -> AccountValue {
    AccountValue::new(AccountDetails::new(Metadata::default(), None, uaid, ids))
}
fn fixture() -> World {
    let mut world = World::default();
    world
        .accounts
        .insert(ALICE_ID.clone(), details(Some(uaid()), vec![opaque(1)]));
    world.accounts.insert(BOB_ID.clone(), details(None, vec![]));
    crate::state::account_identity_restore::rebuild(&mut world).unwrap();
    world
}
fn without_allocations<T>(run: impl FnOnce() -> T) -> T {
    let mut value = None;
    assert_eq!(allocations_during(|| value = Some(run())), 0);
    value.unwrap()
}
fn check_error(world: &World) -> IdentityOwnershipError {
    without_allocations(|| CheckedAccountIdentities::capture(world, 1024))
        .err()
        .unwrap()
}

#[test]
fn reassignment_retains_original_current_predecessor_and_implicit_accounts() {
    let world = fixture();
    let mut accounts = world.accounts.block();
    let mut uaids = world.uaid_accounts.block();
    let mut opaques = world.opaque_uaids.block();
    accounts.insert(ALICE_ID.clone(), details(None, vec![]));
    accounts.insert(BOB_ID.clone(), details(Some(uaid()), vec![opaque(1)]));
    uaids.insert(uaid(), BOB_ID.clone());
    opaques.insert(opaque(1), uaid());
    accounts.commit();
    uaids.commit();
    opaques.commit();
    let checked = without_allocations(|| CheckedAccountIdentities::capture(&world, 128)).unwrap();
    assert_eq!(
        checked.accounts().get(&*ALICE_ID).unwrap().as_ref().uaid(),
        None
    );
    assert_eq!(
        get_at(checked.accounts(), IdentityImage::Predecessor, &*ALICE_ID)
            .unwrap()
            .as_ref()
            .uaid(),
        Some(&uaid())
    );
    assert_eq!(
        get_at(&checked.uaids, IdentityImage::Predecessor, &uaid()),
        Some(&*ALICE_ID)
    );
    assert_eq!(checked.opaques.undo().get(&opaque(1)), Some(&Some(uaid())));
    assert!(without_allocations(|| checked.matches_current()).unwrap());
    drop(checked);
    // Replacement restores the exact prior account, inverse and redundant touch.
    world.block_and_revert().commit();
    let checked = without_allocations(|| CheckedAccountIdentities::capture(&world, 128)).unwrap();
    assert_eq!(checked.uaids.get(&uaid()), Some(&*ALICE_ID));
}

#[test]
fn current_indexes_require_exact_bidirectional_membership() {
    for (mutation, expected) in [
        (0, IdentityMismatch::UaidBinding),
        (1, IdentityMismatch::UaidBinding),
        (2, IdentityMismatch::OpaqueBinding),
        (3, IdentityMismatch::OpaqueBinding),
        (4, IdentityMismatch::ForeignUaid),
        (5, IdentityMismatch::ForeignOpaque),
    ] {
        let mut world = fixture();
        let other = UniversalAccountId::from_hash(Hash::new(b"other identity uaid"));
        match mutation {
            0 => world.uaid_accounts = Storage::default(),
            1 => {
                world.uaid_accounts.insert(uaid(), BOB_ID.clone());
            }
            2 => world.opaque_uaids = Storage::default(),
            3 => {
                world.opaque_uaids.insert(opaque(1), other);
            }
            4 => {
                world.uaid_accounts.insert(other, BOB_ID.clone());
            }
            5 => {
                world.opaque_uaids.insert(opaque(2), uaid());
            }
            _ => unreachable!(),
        }
        assert_eq!(
            check_error(&world),
            corrupt(IdentityImage::Current, expected)
        );
    }
}

#[test]
fn duplicate_and_unbound_sources_cannot_hide_behind_valid_index_rows() {
    for (mutation, expected) in [
        (0, IdentityMismatch::DuplicateOpaque),
        (1, IdentityMismatch::UaidBinding),
        (2, IdentityMismatch::OpaqueBinding),
        (3, IdentityMismatch::OpaqueWithoutUaid),
    ] {
        let mut world = fixture();
        match mutation {
            0 => {
                world.accounts.insert(
                    ALICE_ID.clone(),
                    details(Some(uaid()), vec![opaque(1), opaque(1)]),
                );
            }
            1 => {
                world
                    .accounts
                    .insert(BOB_ID.clone(), details(Some(uaid()), vec![]));
            }
            2 => {
                let other = UniversalAccountId::from_hash(Hash::new(b"duplicate opaque uaid"));
                world
                    .accounts
                    .insert(BOB_ID.clone(), details(Some(other), vec![opaque(1)]));
                world.uaid_accounts.insert(other, BOB_ID.clone());
            }
            3 => {
                world
                    .accounts
                    .insert(BOB_ID.clone(), details(None, vec![opaque(2)]));
            }
            _ => unreachable!(),
        }
        assert_eq!(
            check_error(&world),
            corrupt(IdentityImage::Current, expected)
        );
    }
    // An extra reverse row cannot compensate for duplicate source cardinality:
    // reverse membership is checked before comparing counts.
    let mut world = fixture();
    world.accounts.insert(
        ALICE_ID.clone(),
        details(Some(uaid()), vec![opaque(1), opaque(1)]),
    );
    world.opaque_uaids.insert(opaque(2), uaid());
    assert_eq!(
        check_error(&world),
        corrupt(IdentityImage::Current, IdentityMismatch::ForeignOpaque)
    );
}

#[test]
fn correct_current_indexes_do_not_hide_missing_or_foreign_predecessors() {
    for (mutation, expected) in [
        (0, IdentityMismatch::UaidBinding),
        (1, IdentityMismatch::OpaqueBinding),
        (2, IdentityMismatch::ForeignUaid),
        (3, IdentityMismatch::ForeignOpaque),
        (4, IdentityMismatch::DuplicateOpaque),
    ] {
        let mut world = fixture();
        match mutation {
            0 => {
                world.uaid_accounts = Storage::from_snapshot_parts(
                    BTreeMap::from([(uaid(), ALICE_ID.clone())]),
                    BTreeMap::from([(uaid(), None)]),
                )
            }
            1 => {
                world.opaque_uaids = Storage::from_snapshot_parts(
                    BTreeMap::from([(opaque(1), uaid())]),
                    BTreeMap::from([(opaque(1), None)]),
                )
            }
            2 => {
                let rows = world
                    .accounts
                    .view()
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();
                world.accounts =
                    Storage::from_snapshot_parts(rows, BTreeMap::from([(ALICE_ID.clone(), None)]));
            }
            3 => {
                world.opaque_uaids = Storage::from_snapshot_parts(
                    BTreeMap::from([(opaque(1), uaid())]),
                    BTreeMap::from([(opaque(2), Some(uaid()))]),
                )
            }
            4 => {
                let rows = world
                    .accounts
                    .view()
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();
                world.accounts = Storage::from_snapshot_parts(
                    rows,
                    BTreeMap::from([(
                        ALICE_ID.clone(),
                        Some(details(Some(uaid()), vec![opaque(1), opaque(1)])),
                    )]),
                );
            }
            _ => unreachable!(),
        }
        assert_eq!(
            check_error(&world),
            corrupt(IdentityImage::Predecessor, expected)
        );
    }
}

#[test]
fn work_charges_physical_tombstones_and_each_source_and_reverse_member() {
    let world = fixture();
    // Two account rows, one row per index and two member inspections per image.
    for bound in 0..12 {
        assert_eq!(
            without_allocations(|| CheckedAccountIdentities::capture(&world, bound))
                .err()
                .unwrap(),
            IdentityOwnershipError::WorkLimit
        );
    }
    drop(without_allocations(|| CheckedAccountIdentities::capture(&world, 12)).unwrap());
    let mut world = World::default();
    world.accounts =
        Storage::from_snapshot_parts(BTreeMap::new(), BTreeMap::from([(ALICE_ID.clone(), None)]));
    world.uaid_accounts =
        Storage::from_snapshot_parts(BTreeMap::new(), BTreeMap::from([(uaid(), None)]));
    world.opaque_uaids =
        Storage::from_snapshot_parts(BTreeMap::new(), BTreeMap::from([(opaque(1), None)]));
    for bound in 0..3 {
        assert_eq!(
            without_allocations(|| CheckedAccountIdentities::capture(&world, bound))
                .err()
                .unwrap(),
            IdentityOwnershipError::WorkLimit
        );
    }
    drop(without_allocations(|| CheckedAccountIdentities::capture(&world, 3)).unwrap());
    let mut world = fixture();
    world.accounts.insert(
        ALICE_ID.clone(),
        details(Some(uaid()), (0..64).map(opaque).collect()),
    );
    crate::state::account_identity_restore::rebuild(&mut world).unwrap();
    assert_eq!(
        without_allocations(|| CheckedAccountIdentities::capture(&world, 64))
            .err()
            .unwrap(),
        IdentityOwnershipError::WorkLimit
    );
    drop(without_allocations(|| CheckedAccountIdentities::capture(&world, 8192)).unwrap());
}

#[test]
fn equal_value_publication_of_every_original_dependency_invalidates_capture() {
    for source in 0..3 {
        let world = fixture();
        let checked =
            without_allocations(|| CheckedAccountIdentities::capture(&world, 128)).unwrap();
        match source {
            0 => {
                let mut block = world.accounts.block();
                block.insert(ALICE_ID.clone(), block.get(&*ALICE_ID).unwrap().clone());
                block.commit();
            }
            1 => {
                let mut block = world.uaid_accounts.block();
                block.insert(uaid(), ALICE_ID.clone());
                block.commit();
            }
            2 => {
                let mut block = world.opaque_uaids.block();
                block.insert(opaque(1), uaid());
                block.commit();
            }
            _ => unreachable!(),
        }
        assert!(!without_allocations(|| checked.matches_current()).unwrap());
    }
}

#[test]
fn consumed_accounts_capture_checks_sources_before_allocating_leaves() {
    let mut world = fixture();
    world.opaque_uaids.insert(opaque(2), uaid());
    let mut state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let limits = LeafLimits {
        max_tables: 1,
        max_rows: 16,
        max_payload_bytes: 16_384,
        max_ordered_table_bytes: 32_768,
        max_streamed_value_bytes: 131_072,
    };
    let pool = state.ivm_execution_budget();
    pool.set_limit_bytes(0);
    assert!(matches!(
        capture_accounts_table_once(&state, limits),
        Err(LeafError::IdentityOwnership(
            IdentityOwnershipError::Corrupt { .. }
        ))
    ));
    crate::state::account_identity_restore::rebuild(&mut state.world).unwrap();
    assert!(matches!(
        capture_accounts_table_once(
            &state,
            LeafLimits {
                max_rows: 0,
                ..limits
            }
        ),
        Err(LeafError::IdentityOwnership(
            IdentityOwnershipError::WorkLimit
        ))
    ));
    assert!(matches!(
        capture_accounts_table_once(&state, limits),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    pool.set_limit_bytes(16 * 1024 * 1024);
    let snapshot = capture_accounts_table_once(&state, limits)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.table_id(), "world.accounts");
    assert_eq!(snapshot.row_count(), 2);
}
