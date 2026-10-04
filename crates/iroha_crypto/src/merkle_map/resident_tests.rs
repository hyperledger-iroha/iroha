//! Exact mutation demand, error priority and final snapshot funding controls.

use super::*;
use iroha_allocation::release::ReleaseRegistration;

fn key(byte: u8) -> Hash {
    let mut bytes = [0; Hash::LENGTH];
    bytes[0] = byte;
    Hash::prehashed(bytes)
}

#[test]
fn exact_new_path_peak_preserves_old_snapshots_and_zero_allocation_operations() {
    let layout = ChargedShared::<Node>::allocation_layout();
    let unit = layout.size();
    let budget = AllocationBudget::new(unit * 8);
    let mut map = MerkleMap::new(&budget);
    let value = Hash::new(b"value");
    let changed = Hash::new(b"changed");
    map.replace(key(0), None, Some(value)).unwrap();
    assert_eq!(budget.reserved_bytes(), unit);
    map.replace(key(128), None, Some(value)).unwrap();
    assert_eq!(budget.reserved_bytes(), unit * 3);
    let before = map.clone();
    let root = map.root();
    budget.set_limit_bytes(unit * 4);
    assert!(matches!(map.replace(key(0), Some(value), Some(changed)),
        Err(MerkleMapError::Admission(AllocationRefusal::Capacity { requested_bytes, reserved_bytes, .. }))
        if requested_bytes == unit * 2 && reserved_bytes == unit * 3));
    assert_eq!(map.root(), root);
    assert_eq!(map.len(), 2);
    assert_eq!(budget.reserved_bytes(), unit * 3);
    budget.set_limit_bytes(0);
    assert!(matches!(
        map.replace(key(0), None, Some(changed)),
        Err(MerkleMapError::PreimageMismatch { .. })
    ));
    assert!(!map.replace(key(0), Some(value), Some(value)).unwrap());
    // Removing either leaf collapses its parent and needs no new allocation.
    assert!(map.replace(key(128), Some(value), None).unwrap());
    assert_eq!(
        budget.reserved_bytes(),
        unit * 3,
        "snapshot still owns all three nodes"
    );
    drop(map);
    assert_eq!(before.root(), root);
    assert_eq!(budget.reserved_bytes(), unit * 3);
    drop(before);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn public_path_geometry_counts_insert_replace_and_nonroot_delete_exactly() {
    let unit = ChargedShared::<Node>::allocation_layout().size();
    let budget = AllocationBudget::new(unit * 16);
    let mut map = MerkleMap::new(&budget);
    let value = Hash::new(b"original");
    for byte in [0, 128, 64] {
        map.replace(key(byte), None, Some(value)).unwrap();
    }
    assert_eq!(budget.reserved_bytes(), unit * 5);
    assert_eq!(
        resident::replacement_nodes(map.node.as_ref(), &key(0), Some(value)),
        3
    );
    assert_eq!(
        resident::replacement_nodes(map.node.as_ref(), &key(0), None),
        1
    );
    assert_eq!(
        resident::replacement_nodes(map.node.as_ref(), &key(32), Some(value)),
        4
    );
    let snapshot = map.clone();
    let root = snapshot.root();
    budget.set_limit_bytes(unit * 6);
    map.replace(key(0), Some(value), None).unwrap();
    assert_eq!(budget.reserved_bytes(), unit * 6);
    assert_eq!(budget.peak_reserved_bytes(), unit * 6);
    assert_eq!(snapshot.root(), root);
    assert_eq!(map.len(), 2);
    drop(snapshot);
    assert_eq!(budget.reserved_bytes(), unit * 3);
    map.replace(key(64), Some(value), None).unwrap();
    assert_eq!(budget.reserved_bytes(), unit);
    map.replace(key(128), Some(value), None).unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(map.root(), MerkleMap::new(&budget).root());
}

#[test]
fn original_capacity_observation_survives_unrelated_release_and_final_snapshot() {
    use std::task::{Context, Poll, Waker};
    let unit = ChargedShared::<Node>::allocation_layout().size();
    let observer_bytes = ReleaseRegistration::allocation_layout().size();
    let budget = AllocationBudget::new(unit * 3 + observer_bytes);
    let mut registration = ReleaseRegistration::from_reservation(
        &mut budget
            .try_reserve(ReleaseRegistration::allocation_layout())
            .unwrap(),
    )
    .unwrap();
    assert!(registration.belongs_to(&budget));
    let value = Hash::new(b"value");
    let mut blocker = MerkleMap::new(&budget);
    blocker.replace(key(0), None, Some(value)).unwrap();
    blocker.replace(key(128), None, Some(value)).unwrap();
    let borrowed = blocker.clone();
    let mut map = MerkleMap::new(&budget);
    let Err(MerkleMapError::Admission(AllocationRefusal::Capacity { release, .. })) =
        map.replace(key(1), None, Some(value))
    else {
        panic!("original pool must refuse");
    };
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(
        registration.poll_wait(&release, &mut context),
        Poll::Pending
    );
    let other = AllocationBudget::new(unit);
    drop(other.try_reserve_bytes(unit).unwrap());
    drop(blocker);
    assert_eq!(
        registration.poll_wait(&release, &mut context),
        Poll::Pending
    );
    drop(borrowed);
    assert_eq!(
        registration.poll_wait(&release, &mut context),
        Poll::Ready(())
    );
    map.replace(key(1), None, Some(value)).unwrap();
    drop(map);
    assert_eq!(budget.reserved_bytes(), observer_bytes);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}
