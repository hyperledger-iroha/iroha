//! Retained contract alias checks use original maps and the actual catalog owner.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        State,
        authority_registry::{
            complete::capture_contract_alias_bindings_once,
            leaf::{LeafError, LeafLimits},
        },
    },
    test_allocations::allocations_during,
};
use iroha_data_model::{IntoKeyValue, account::Account, prelude::Registrable};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_samples::ALICE_ID;

fn address(nonce: u64) -> ContractAddress {
    ContractAddress::derive(
        &crate::state::DEFAULT_TEST_NETWORK_ID,
        &ALICE_ID,
        nonce,
        DataSpaceId::UNIVERSAL,
    )
    .unwrap()
}

fn record(name: &str) -> ContractAliasBindingRecord {
    ContractAliasBindingRecord {
        alias: format!("{name}::universal").parse().unwrap(),
        lease_expiry_ms: None,
        grace_until_ms: None,
        bound_at_ms: 1,
    }
}

fn fixture() -> Box<World> {
    let mut world = Box::new(World::default());
    let (id, account) = Account::new(ALICE_ID.clone())
        .build(&ALICE_ID)
        .into_key_value();
    world.accounts.insert(id, account);
    world
        .contract_alias_bindings
        .insert(address(0), record("router"));
    world.rebuild_contract_alias_indexes().unwrap();
    world
}

fn check(world: &World, work: u64) -> Result<(), GroupedOwnershipError> {
    let mut result = None;
    assert_eq!(
        allocations_during(|| {
            result = Some(CheckedContractAliases::capture(world, work).map(|_| ()));
        }),
        0
    );
    result.unwrap()
}

fn image(previous: bool) -> GroupImage {
    if previous {
        GroupImage::Predecessor
    } else {
        GroupImage::Current
    }
}

#[test]
fn rename_delete_insert_and_redundant_touches_retain_exact_predecessor() {
    let mut world = fixture();
    world
        .contract_alias_bindings
        .insert(address(1), record("removed"));
    world
        .contract_alias_bindings
        .insert(address(2), record("untouched"));
    {
        let mut block = world.contract_alias_bindings.block();
        block.insert(address(0), record("renamed"));
        block.remove(address(1));
        block.insert(address(2), record("untouched"));
        block.insert(address(3), record("inserted"));
        block.remove(address(4));
        block.commit();
    }
    world.rebuild_contract_alias_indexes().unwrap();
    assert_eq!(check(&world, 1024), Ok(()));
    let checked = CheckedContractAliases::capture(&world, 1024).unwrap();
    assert_eq!(
        get_at(checked.rows(), GroupImage::Current, &address(0)),
        Some(&record("renamed"))
    );
    assert_eq!(
        get_at(checked.rows(), GroupImage::Predecessor, &address(0)),
        Some(&record("router"))
    );
    assert_eq!(
        get_at(checked.rows(), GroupImage::Predecessor, &address(1)),
        Some(&record("removed"))
    );
    assert!(get_at(checked.rows(), GroupImage::Current, &address(1)).is_none());
    assert!(get_at(checked.rows(), GroupImage::Predecessor, &address(3)).is_none());
    assert!(checked.rows().undo().contains_key(&address(4)));
}

#[test]
fn missing_wrong_and_foreign_inverse_rows_reject_in_either_image() {
    for previous in [false, true] {
        for defect in 0..3 {
            let mut world = fixture();
            let alias = record("router").alias;
            world.contract_aliases = mv::storage::Storage::new();
            if defect == 1 {
                world.contract_aliases.insert(alias.clone(), address(1));
            } else if defect == 2 {
                world.contract_aliases.insert(alias.clone(), address(0));
                world
                    .contract_aliases
                    .insert(record("foreign").alias, address(0));
            }
            if previous {
                let mut block = world.contract_aliases.block();
                block.insert(alias, address(0));
                block.remove(record("foreign").alias);
                block.commit();
            }
            assert_eq!(
                check(&world, 1024),
                Err(GroupedOwnershipError::Corrupt {
                    index: "world.contract_aliases",
                    image: image(previous),
                    mismatch: if defect == 2 {
                        GroupMismatch::ForeignMember
                    } else {
                        GroupMismatch::MissingMember
                    },
                })
            );
        }
    }
}

#[test]
fn duplicate_canonical_alias_targets_reject_in_either_image() {
    for previous in [false, true] {
        let mut world = fixture();
        world
            .contract_alias_bindings
            .insert(address(1), record("router"));
        if previous {
            let mut block = world.contract_alias_bindings.block();
            block.remove(address(1));
            block.commit();
        }
        assert_eq!(
            check(&world, 1024),
            Err(GroupedOwnershipError::Corrupt {
                index: "world.contract_aliases",
                image: image(previous),
                mismatch: GroupMismatch::MissingMember,
            })
        );
    }
}

#[test]
fn all_invalid_lease_relations_reject_without_repair_in_either_image() {
    for previous in [false, true] {
        for (expiry, grace, bound) in [
            (None, Some(2), 1),
            (Some(1), None, 1),
            (Some(2), Some(1), 1),
        ] {
            let mut world = fixture();
            let mut invalid = record("router");
            invalid.lease_expiry_ms = expiry;
            invalid.grace_until_ms = grace;
            invalid.bound_at_ms = bound;
            world
                .contract_alias_bindings
                .insert(address(0), invalid.clone());
            if previous {
                let mut block = world.contract_alias_bindings.block();
                block.insert(address(0), record("router"));
                block.commit();
            }
            assert_eq!(
                check(&world, 1024),
                Err(GroupedOwnershipError::Source {
                    table: "world.contract_alias_bindings",
                    image: image(previous),
                    reason: alias_lease::violation(expiry, grace, bound).unwrap(),
                })
            );
            let rows = world
                .contract_alias_bindings
                .try_committed_view_nonblocking()
                .unwrap();
            assert_eq!(get_at(&rows, image(previous), &address(0)), Some(&invalid));
        }
    }
}

#[test]
fn undeployed_and_expired_bindings_remain_representable_until_cleanup() {
    let mut world = fixture();
    let mut expired = record("router");
    expired.lease_expiry_ms = Some(2);
    expired.grace_until_ms = Some(3);
    world
        .contract_alias_bindings
        .insert(address(0), expired.clone());
    assert!(expired.is_grace_expired_at(u64::MAX));
    assert!(world.contract_instances.view().get(&address(0)).is_none());
    assert_eq!(check(&world, 4), Ok(()));
}

#[test]
fn exact_work_limit_charges_masked_rows_and_absent_undo_before_filtering() {
    let world = fixture();
    assert_eq!(check(&world, 3), Err(GroupedOwnershipError::WorkLimit));
    assert_eq!(check(&world, 4), Ok(()));
    {
        let mut block = world.contract_alias_bindings.block();
        block.insert(address(0), record("router"));
        block.remove(address(1));
        block.commit();
    }
    assert_eq!(check(&world, 5), Err(GroupedOwnershipError::WorkLimit));
    assert_eq!(check(&world, 6), Ok(()));
}

#[test]
fn both_original_readers_detect_publication_after_capture() {
    for source in [false, true] {
        let world = fixture();
        let checked = CheckedContractAliases::capture(&world, 4).unwrap();
        if source {
            world.contract_alias_bindings.block().commit();
        } else {
            world.contract_aliases.block().commit();
        }
        assert!(!checked.matches_current().unwrap());
    }
}

#[test]
fn checked_capture_uses_original_state_budget_and_retained_rows() {
    let state = State::new_for_testing(
        *fixture(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let limits = LeafLimits {
        max_tables: 1,
        max_rows: 16,
        max_payload_bytes: 16384,
        max_ordered_table_bytes: 32768,
        max_streamed_value_bytes: 131072,
    };
    let pool = state.ivm_execution_budget();
    pool.set_limit_bytes(0);
    assert!(matches!(
        capture_contract_alias_bindings_once(&state, limits),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    pool.set_limit_bytes(16 * 1024 * 1024);
    let snapshot = capture_contract_alias_bindings_once(&state, limits)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.table_id(), "world.contract_alias_bindings");
    assert_eq!(snapshot.row_count(), 1);
}
