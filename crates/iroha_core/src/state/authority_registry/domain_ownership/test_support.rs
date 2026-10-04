//! Original domain-owner fixture helpers shared by committed and frozen tests.

use super::*;
use crate::{state::snapshot_storage, test_allocations::allocations_during};
use iroha_config::parameters::actual::LaneConfig;
use iroha_data_model::{IntoKeyValue, account::Account, prelude::Registrable};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::{Storage, StorageReadOnly};
use std::collections::BTreeMap;

/// Borrowed fixture owner buckets before explicit test corruption.
pub(in crate::state) type OwnerRows = BTreeMap<AccountId, BTreeSet<DomainId>>;
/// Original owner preimages retained by the fixture.
pub(in crate::state) type OwnerUndo = BTreeMap<AccountId, Option<BTreeSet<DomainId>>>;

/// Construct a domain in the fixture dataspace.
pub(in crate::state) fn id(name: &str) -> DomainId {
    DomainId::try_new(name, "universal").unwrap()
}

/// Require the measured original-reader operation to allocate nothing.
pub(in crate::state) fn without_allocations<T>(operation: impl FnOnce() -> T) -> T {
    let mut result = None;
    let allocations = allocations_during(|| result = Some(operation()));
    assert_eq!(
        allocations, 0,
        "domain-owner inspection must allocate nothing"
    );
    result.unwrap()
}

/// Seed canonical universal accounts independently of domain ownership.
pub(in crate::state) fn empty() -> World {
    let mut world = World::default();
    for owner in [&*ALICE_ID, &*BOB_ID] {
        let (key, value) = Account::new(owner.clone()).build(owner).into_key_value();
        world.accounts.insert(key, value);
    }
    world
}

/// Build the original transfer, deletion and unchanged-owner domain cut.
pub(in crate::state) fn fixture() -> World {
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

/// Apply actual ordinary or replacement MV mutations through the World owner.
pub(in crate::state) fn change(world: &World, replacement: bool) {
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

/// Preserve exact canonical source and owner-index bytes across inspection.
pub(in crate::state) fn encoded(world: &World) -> [String; 2] {
    let mut domains = String::new();
    let mut owners = String::new();
    snapshot_storage::serialize(&world.domains, &mut domains);
    snapshot_storage::serialize(&world.domains_by_owner, &mut owners);
    [domains, owners]
}

/// Introduce an explicit fixture defect without changing canonical domains.
pub(in crate::state) fn edit_index(
    world: &mut World,
    edit: impl FnOnce(&mut OwnerRows, &mut OwnerUndo),
) {
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

/// Inspect one original committed relation without allocating.
pub(in crate::state) fn error(world: &World, limit: u64) -> DomainOwnershipError {
    without_allocations(|| CheckedDomainOwnership::capture(world, limit))
        .err()
        .expect("malformed projection or insufficient work")
}

/// Exact required work for the bounded valid original relation, without allocating.
pub(in crate::state) fn exact_work(
    domains: &impl RawStorageImages<DomainId, Domain>,
    owners: &impl RawStorageImages<AccountId, BTreeSet<DomainId>>,
) -> u64 {
    let mut low = 0;
    let mut high = 1_000_000;
    validate_original_domain_ownership(domains, owners, high).unwrap();
    while low < high {
        let allowance = low + (high - low) / 2;
        match validate_original_domain_ownership(domains, owners, allowance) {
            Ok(()) => high = allowance,
            Err(DomainOwnershipError::WorkLimit) => low = allowance + 1,
            Err(error) => panic!("valid original relation: {error}"),
        }
    }
    low
}
