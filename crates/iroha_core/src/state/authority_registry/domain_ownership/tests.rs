//! Real MV history, malformed projections and exact local work controls.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        State,
        authority_registry::{
            complete::capture_domains_table_once,
            leaf::{LeafError, LeafLimits},
        },
        snapshot_storage,
    },
    test_allocations::allocations_during,
};
use iroha_config::parameters::actual::LaneConfig;
use iroha_data_model::{IntoKeyValue, account::Account, prelude::Registrable};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::Storage;
use std::collections::BTreeMap;

type OwnerRows = BTreeMap<AccountId, BTreeSet<DomainId>>;
type OwnerUndo = BTreeMap<AccountId, Option<BTreeSet<DomainId>>>;

fn id(name: &str) -> DomainId {
    DomainId::try_new(name, "universal").unwrap()
}

fn without_allocations<T>(operation: impl FnOnce() -> T) -> T {
    let mut result = None;
    let allocations = allocations_during(|| result = Some(operation()));
    assert_eq!(
        allocations, 0,
        "domain-owner inspection must allocate nothing"
    );
    result.unwrap()
}

fn empty() -> World {
    let mut world = World::default();
    for owner in [&*ALICE_ID, &*BOB_ID] {
        let (key, value) = Account::new(owner.clone()).build(owner).into_key_value();
        world.accounts.insert(key, value);
    }
    world
}

fn fixture() -> World {
    let mut world = empty();
    for (name, owner) in [
        ("moving", &*ALICE_ID),
        ("removed", &*ALICE_ID),
        ("shared", &*ALICE_ID),
        ("untouched", &*BOB_ID),
    ] {
        world
            .domains
            .insert(id(name), Domain::new(id(name)).build(owner));
    }
    world.rebuild_domain_owner_index();
    world
}

fn change(world: &World, replacement: bool) {
    let mut block = if replacement {
        world.block_and_revert()
    } else {
        world.block()
    };
    let mut tx = block.transaction_without_telemetry(LaneConfig::default(), 0);
    let owner = if replacement { &*ALICE_ID } else { &*BOB_ID };
    tx.insert_domain_entry(id("moving"), Domain::new(id("moving")).build(owner));
    tx.insert_domain_entry(id("added"), Domain::new(id("added")).build(&ALICE_ID));
    tx.remove_domain_entry(&id("removed")).unwrap();
    // No-op remove/reinsert and unchanged-owner writes must retain both images.
    let shared = tx.remove_domain_entry(&id("shared")).unwrap();
    tx.insert_domain_entry(id("shared"), shared);
    tx.insert_domain_entry(id("untouched"), Domain::new(id("untouched")).build(&BOB_ID));
    tx.domains.remove(id("absent"));
    tx.apply();
    block.commit();
}

fn encoded(world: &World) -> [String; 2] {
    let mut domains = String::new();
    let mut owners = String::new();
    snapshot_storage::serialize(&world.domains, &mut domains);
    snapshot_storage::serialize(&world.domains_by_owner, &mut owners);
    [domains, owners]
}

fn edit_index(world: &mut World, edit: impl FnOnce(&mut OwnerRows, &mut OwnerUndo)) {
    let snapshot = world.domains_by_owner.snapshot();
    let mut current = snapshot
        .current()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let mut undo = snapshot
        .revert_map()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    drop(snapshot);
    edit(&mut current, &mut undo);
    world.domains_by_owner = Storage::from_snapshot_parts(current, undo);
}

fn error(world: &World, limit: u64) -> DomainOwnershipError {
    without_allocations(|| CheckedDomainOwnership::capture(world, limit))
        .err()
        .expect("malformed projection or insufficient work")
}

#[test]
fn native_transfers_deletions_reinsertions_and_replacement_preserve_both_cuts() {
    let world = fixture();
    for replacement in [false, true] {
        change(&world, replacement);
        let original = encoded(&world);
        let checked = without_allocations(|| CheckedDomainOwnership::capture(&world, 128)).unwrap();
        assert_eq!(
            checked.domains().get(&id("moving")).unwrap().owned_by(),
            if replacement { &*ALICE_ID } else { &*BOB_ID }
        );
        assert_eq!(
            before(checked.domains(), &id("moving")).unwrap().owned_by(),
            &*ALICE_ID
        );
        assert!(checked.domains().get(&id("removed")).is_none());
        assert!(before(checked.domains(), &id("removed")).is_some());
        assert!(before(checked.domains(), &id("added")).is_none());
        assert!(checked.domains().undo().contains_key(&id("absent")));
        assert!(checked.domains().undo().contains_key(&id("untouched")));
        assert!(without_allocations(|| checked.matches_current()).unwrap());
        drop(checked);
        assert_eq!(
            encoded(&world),
            original,
            "validation may not rewrite current or undo maps"
        );
    }
}

#[test]
fn rejects_missing_extra_duplicate_wrong_owner_and_empty_current_buckets() {
    for mutation in 0..5 {
        let mut world = fixture();
        edit_index(&mut world, |current, _| match mutation {
            0 => {
                current.get_mut(&*ALICE_ID).unwrap().remove(&id("moving"));
            }
            1 => {
                current.get_mut(&*ALICE_ID).unwrap().insert(id("ghost"));
            }
            2 => {
                current.get_mut(&*BOB_ID).unwrap().insert(id("moving"));
            }
            3 => {
                current.get_mut(&*ALICE_ID).unwrap().remove(&id("moving"));
                current.get_mut(&*BOB_ID).unwrap().insert(id("moving"));
            }
            4 => {
                current.insert(BOB_ID.clone(), BTreeSet::new());
            }
            _ => unreachable!(),
        });
        let original = encoded(&world);
        let mismatch = if matches!(mutation, 0 | 3 | 4) {
            OwnershipMismatch::MissingDomain
        } else {
            OwnershipMismatch::ForeignDomain
        };
        assert_eq!(
            error(&world, 128),
            DomainOwnershipError::Corrupt {
                image: OwnershipImage::Current,
                mismatch
            }
        );
        assert_eq!(encoded(&world), original);
    }
    // An otherwise unused owner cannot have an empty bucket either.
    let mut world = empty();
    world
        .domains_by_owner
        .insert(ALICE_ID.clone(), BTreeSet::new());
    assert_eq!(
        error(&world, 1),
        DomainOwnershipError::Corrupt {
            image: OwnershipImage::Current,
            mismatch: OwnershipMismatch::EmptyBucket
        }
    );
}

#[test]
fn correct_current_rows_do_not_hide_corrupt_predecessor_membership() {
    for mutation in 0..4 {
        let mut world = fixture();
        change(&world, false);
        edit_index(&mut world, |_, undo| match mutation {
            0 => {
                undo.insert(ALICE_ID.clone(), None);
            }
            1 => {
                undo.get_mut(&*ALICE_ID)
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .insert(id("ghost"));
            }
            2 => {
                undo.get_mut(&*BOB_ID)
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .insert(id("moving"));
            }
            3 => {
                undo.get_mut(&*ALICE_ID)
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .remove(&id("removed"));
            }
            _ => unreachable!(),
        });
        let original = encoded(&world);
        let mismatch = if matches!(mutation, 0 | 3) {
            OwnershipMismatch::MissingDomain
        } else {
            OwnershipMismatch::ForeignDomain
        };
        assert_eq!(
            error(&world, 128),
            DomainOwnershipError::Corrupt {
                image: OwnershipImage::Predecessor,
                mismatch
            }
        );
        assert_eq!(encoded(&world), original);
    }
}

#[test]
fn exact_work_bound_charges_absent_preimages_and_never_reports_them_as_corruption() {
    let mut world = empty();
    world.domains = Storage::from_snapshot_parts(
        BTreeMap::from([(id("live"), Domain::new(id("live")).build(&ALICE_ID))]),
        BTreeMap::from([(id("absent"), None)]),
    );
    world.domains_by_owner = Storage::from_snapshot_parts(
        BTreeMap::from([(ALICE_ID.clone(), BTreeSet::from([id("live")]))]),
        // A transient owner during the block may leave a redundant absent
        // preimage. Only current/predecessor contents are derived, not the
        // intermediate sequence of touched buckets.
        BTreeMap::from([(BOB_ID.clone(), None)]),
    );
    let original = encoded(&world);
    for limit in 0..8 {
        assert_eq!(error(&world, limit), DomainOwnershipError::WorkLimit);
    }
    drop(without_allocations(|| CheckedDomainOwnership::capture(&world, 8)).unwrap());
    assert_eq!(encoded(&world), original);
    let empty = World::default();
    drop(without_allocations(|| CheckedDomainOwnership::capture(&empty, 0)).unwrap());
}

#[test]
fn checked_owner_retains_original_rows_and_detects_equal_value_publication() {
    for canonical in [false, true] {
        let world = fixture();
        let checked = without_allocations(|| CheckedDomainOwnership::capture(&world, 128)).unwrap();
        if canonical {
            let mut block = world.domains.block();
            let value = block.get(&id("moving")).unwrap().clone();
            block.insert(id("moving"), value);
            block.commit();
        } else {
            let mut block = world.domains_by_owner.block();
            let value = block.get(&*ALICE_ID).unwrap().clone();
            block.insert(ALICE_ID.clone(), value);
            block.commit();
        }
        assert!(!without_allocations(|| checked.matches_current()).unwrap());
        assert_eq!(
            checked.domains().get(&id("moving")).unwrap().owned_by(),
            &*ALICE_ID
        );
        assert!(
            checked.domains().undo().is_empty(),
            "the checked reader must not silently refresh its undo"
        );
    }
}

fn limits() -> LeafLimits {
    LeafLimits {
        max_tables: 1,
        max_rows: 16,
        max_payload_bytes: 16_384,
        max_ordered_table_bytes: 32_768,
        max_streamed_value_bytes: 131_072,
    }
}

#[test]
fn scoped_native_capture_consumes_checked_source_and_preserves_operational_refusal() {
    let mut world = fixture();
    edit_index(&mut world, |current, _| {
        current.get_mut(&*ALICE_ID).unwrap().insert(id("ghost"));
    });
    let mut state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let pool = state
        .pipeline_ivm_prepared_cache
        .read()
        .execution_budget()
        .clone();
    // A corrupt index is rejected before allocating any leaf. Rebuilding is an
    // explicit fixture operation; capture must not repair or mutate live World.
    pool.set_limit_bytes(0);
    assert!(matches!(
        capture_domains_table_once(&state, limits()),
        Err(LeafError::DomainOwnership(DomainOwnershipError::Corrupt {
            image: OwnershipImage::Current,
            mismatch: OwnershipMismatch::ForeignDomain
        }))
    ));
    state.world.rebuild_domain_owner_index();
    assert!(matches!(
        capture_domains_table_once(
            &state,
            LeafLimits {
                max_rows: 0,
                ..limits()
            }
        ),
        Err(LeafError::DomainOwnership(DomainOwnershipError::WorkLimit))
    ));
    assert!(matches!(
        capture_domains_table_once(&state, limits()),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    pool.set_limit_bytes(16 * 1024 * 1024);
    let snapshot = capture_domains_table_once(&state, limits())
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.table_id(), "world.domains");
    assert_eq!(snapshot.row_count(), 4);
}
