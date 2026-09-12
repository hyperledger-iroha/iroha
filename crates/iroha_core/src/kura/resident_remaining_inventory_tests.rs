//! Independent collection, publication and last-owner lifetime regressions.

use super::{
    resident_inventory::{ResidentMutex, ResidentOwner, lengths},
    resident_inventory_lifetimes::{VerificationAllocations, VerificationLease},
    resident_nested_map::{AssociationValue, NestedMap},
    resource_inventory::{Family, Inventory, Unavailable, Usage},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn registry() -> Arc<Inventory> {
    let registry = Arc::new(Inventory::default());
    registry
        .initialize(
            registry.reconciliation_generation().unwrap(),
            &super::resource_inventory::ALL_FAMILIES.map(|family| (family, Usage::default())),
        )
        .unwrap();
    registry
}
fn count(registry: &Inventory, family: Family) -> u64 {
    registry
        .component_usage_for_tests(family)
        .unwrap()
        .resident_associations
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Peers(BTreeSet<u64>);
impl AssociationValue for Peers {
    const FAMILY: Family = Family::ResidentReplica;
    fn association_weight(&self) -> Result<u64, Unavailable> {
        lengths([1, self.0.len()])
    }
}
#[test]
fn nested_resident_map_tracks_duplicate_replacement_partial_removal_and_whole_reload() {
    let inventory = registry();
    let owner = ResidentMutex::new(NestedMap::<u64, Peers>::default(), &inventory);
    owner.lock().insert(1, Peers(BTreeSet::from([1, 2, 3])));
    assert_eq!(count(&inventory, Family::ResidentReplica), 4);
    owner.lock().insert(1, Peers(BTreeSet::from([1, 2, 3])));
    assert_eq!(count(&inventory, Family::ResidentReplica), 4);
    owner.lock().entry(1).or_default().0.insert(4);
    assert_eq!(count(&inventory, Family::ResidentReplica), 5);
    owner.lock().entry(2).or_default().0.insert(9);
    assert_eq!(count(&inventory, Family::ResidentReplica), 7);
    owner.lock().get_mut(&1).unwrap().0.remove(&2);
    assert_eq!(count(&inventory, Family::ResidentReplica), 6);
    owner.lock().retain(|key, value| {
        value.0.retain(|peer| *peer != 3);
        *key == 1
    });
    assert_eq!(count(&inventory, Family::ResidentReplica), 3);
    owner.lock().insert(1, Peers(BTreeSet::from([8])));
    assert_eq!(count(&inventory, Family::ResidentReplica), 2);
    assert!(owner.lock().remove(&99).is_none());
    assert_eq!(count(&inventory, Family::ResidentReplica), 2);
    *owner.lock() = NestedMap::from(BTreeMap::from([
        (5, Peers(BTreeSet::from([1, 2]))),
        (6, Peers::default()),
    ]));
    assert_eq!(count(&inventory, Family::ResidentReplica), 4);
    owner.lock().clear();
    assert_eq!(count(&inventory, Family::ResidentReplica), 0);
}
#[test]
fn nested_resident_maps_publish_independent_objects_and_reject_unwind() {
    let inventory = registry();
    let first = ResidentMutex::new(NestedMap::<u64, Peers>::default(), &inventory);
    let second = ResidentMutex::new(NestedMap::<u64, Peers>::default(), &inventory);
    first.lock().insert(1, Peers(BTreeSet::from([1, 2])));
    second.lock().insert(1, Peers(BTreeSet::from([1, 2])));
    assert_eq!(count(&inventory, Family::ResidentReplica), 6);
    first.lock().remove(&1);
    assert_eq!(count(&inventory, Family::ResidentReplica), 3);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut locked = second.lock();
        let mut value = locked.get_mut(&1).unwrap();
        value.0.insert(3);
        assert!(matches!(inventory.try_snapshot(), Err(Unavailable::Busy)));
        panic!("after a real nested mutation");
    }));
    assert!(result.is_err());
    assert!(inventory.try_snapshot().is_err());
    assert!(second.lock().resident_associations().is_err());
    assert!(!second.lock().resident_complete());
    assert_eq!(second.lock().get(&1).unwrap().0.len(), 3);
    // Reinitializing the registry cannot make the actual interrupted owner complete.
    let generation = inventory.reconciliation_generation().unwrap();
    inventory
        .initialize(
            generation,
            &[(
                Family::ResidentReplica,
                Usage {
                    resident_associations: 4,
                    ..Usage::default()
                },
            )],
        )
        .unwrap();
    assert!(!second.lock().resident_complete());
}

#[derive(Debug)]
struct VerificationData {
    entries: BTreeSet<u64>,
    inventory: Arc<Inventory>,
    destroyed: Arc<AtomicBool>,
}
impl ResidentOwner for VerificationData {
    const FAMILY: Family = Family::ResidentVerification;
    fn resident_associations(&self) -> Result<u64, Unavailable> {
        lengths([self.entries.len()])
    }
    fn resident_complete(&self) -> bool {
        true
    }
}
impl Drop for VerificationData {
    fn drop(&mut self) {
        assert!(matches!(
            self.inventory.try_snapshot(),
            Err(Unavailable::Busy)
        ));
        self.entries.clear();
        self.destroyed.store(true, Ordering::Release);
    }
}
fn allocation(
    inventory: &Arc<Inventory>,
    ledger: &Arc<ResidentMutex<VerificationAllocations>>,
    values: &[u64],
    destroyed: &Arc<AtomicBool>,
) -> VerificationLease<VerificationData> {
    VerificationLease::new(
        VerificationData {
            entries: values.iter().copied().collect(),
            inventory: Arc::clone(inventory),
            destroyed: Arc::clone(destroyed),
        },
        ledger,
    )
}
#[test]
fn startup_inventory_lease_counts_last_arc_after_installation_is_cleared() {
    let inventory = registry();
    let ledger = Arc::new(ResidentMutex::new(
        VerificationAllocations::default(),
        &inventory,
    ));
    let destroyed = Arc::new(AtomicBool::new(false));
    let mut installed = Some(Arc::new(allocation(
        &inventory,
        &ledger,
        &[1, 2, 3],
        &destroyed,
    )));
    let reader = Arc::clone(installed.as_ref().unwrap());
    assert_eq!(count(&inventory, Family::ResidentVerification), 3);
    installed = None;
    assert!(installed.is_none());
    assert!(!destroyed.load(Ordering::Acquire));
    assert_eq!(count(&inventory, Family::ResidentVerification), 3);
    drop(reader);
    assert!(destroyed.load(Ordering::Acquire));
    assert_eq!(count(&inventory, Family::ResidentVerification), 0);
}
#[test]
fn startup_inventory_lease_counts_replacement_and_unique_mutation_without_cloned_ownership() {
    let inventory = registry();
    let ledger = Arc::new(ResidentMutex::new(
        VerificationAllocations::default(),
        &inventory,
    ));
    let destroyed1 = Arc::new(AtomicBool::new(false));
    let destroyed2 = Arc::new(AtomicBool::new(false));
    let old = Arc::new(allocation(&inventory, &ledger, &[1, 2], &destroyed1));
    let mut current = Arc::new(allocation(&inventory, &ledger, &[1, 2, 3], &destroyed2));
    assert_eq!(count(&inventory, Family::ResidentVerification), 5);
    let reader = Arc::clone(&current);
    assert!(Arc::get_mut(&mut current).is_none());
    drop(reader);
    Arc::get_mut(&mut current).unwrap().with_mut(|data| {
        assert!(matches!(inventory.try_snapshot(), Err(Unavailable::Busy)));
        data.entries.remove(&1);
        data.entries.insert(8);
    });
    assert_eq!(count(&inventory, Family::ResidentVerification), 5);
    drop(old);
    assert!(destroyed1.load(Ordering::Acquire));
    assert_eq!(count(&inventory, Family::ResidentVerification), 3);
    drop(current);
    assert!(destroyed2.load(Ordering::Acquire));
    assert_eq!(count(&inventory, Family::ResidentVerification), 0);
}
#[test]
fn startup_inventory_lease_reconciliation_rejects_creation_crossing_the_read_generation() {
    let inventory = registry();
    let ledger = Arc::new(ResidentMutex::new(
        VerificationAllocations::default(),
        &inventory,
    ));
    let generation = inventory.reconciliation_generation().unwrap();
    let captured = ledger.lock().resident_associations().unwrap();
    let destroyed = Arc::new(AtomicBool::new(false));
    let held = allocation(&inventory, &ledger, &[4, 5, 6, 7], &destroyed);
    assert!(matches!(
        inventory.initialize(
            generation,
            &[(
                Family::ResidentVerification,
                Usage {
                    resident_associations: captured,
                    ..Usage::default()
                }
            )]
        ),
        Err(Unavailable::GenerationChanged)
    ));
    assert_eq!(count(&inventory, Family::ResidentVerification), 4);
    drop(held);
    assert_eq!(count(&inventory, Family::ResidentVerification), 0);
}

#[test]
fn nested_resident_retained_guard_caught_callback_unwind_stays_unavailable() {
    let inventory = registry();
    let owner = ResidentMutex::new(NestedMap::<u64, Peers>::default(), &inventory);
    owner.lock().insert(1, Peers(BTreeSet::from([1, 2])));
    let mut retained_guard = owner.lock();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        retained_guard.retain(|_, value| {
            value.0.insert(3);
            panic!("callback unwind caught while outer owner remains locked");
        });
    }));
    assert!(result.is_err());
    assert_eq!(retained_guard.get(&1).unwrap().0.len(), 3);
    assert!(!retained_guard.resident_complete());
    drop(retained_guard);
    assert!(inventory.try_snapshot().is_err());
    assert!(!owner.lock().resident_complete());
}
#[test]
fn nested_resident_forgotten_value_and_entry_guards_cannot_publish_stale_counts() {
    for entry in [false, true] {
        let inventory = registry();
        let owner = ResidentMutex::new(NestedMap::<u64, Peers>::default(), &inventory);
        owner.lock().insert(1, Peers(BTreeSet::from([1, 2])));
        let mut retained_guard = owner.lock();
        let mut value = if entry {
            retained_guard.entry(2).or_default()
        } else {
            retained_guard.get_mut(&1).unwrap()
        };
        value.0.insert(3);
        std::mem::forget(value);
        assert!(!retained_guard.resident_complete());
        drop(retained_guard);
        assert!(inventory.try_snapshot().is_err());
        assert!(!owner.lock().resident_complete());
    }
}
