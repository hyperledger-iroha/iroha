//! Complete resource snapshots across nonblocking recovery and actual poison boundaries.

use super::*;
use resource_inventory::Unavailable;

fn snapshot_owner_fixture() -> Arc<Kura> {
    let kura = Kura::blank_kura_for_testing();
    {
        let _carrier = kura.merge_carrier_lock.lock();
        // Blank Kura deliberately leaves this lazy owner uninitialized. Use
        // its actual bounded disk reader before asking for all 23 families.
        kura.ensure_merge_carrier_index_initialized_unlocked()
            .unwrap();
    }
    kura.reconcile_physical_resource_inventory().unwrap();
    kura.reconcile_resident_resource_inventory().unwrap();
    assert!(kura.resource_inventory_snapshot().is_ok());
    kura
}

#[test]
fn resource_snapshot_observation_does_not_wait_for_storage_or_bootstrap_owners() {
    let kura = snapshot_owner_fixture();
    let expected = kura.resource_inventory_snapshot().unwrap().components;
    // These exact owner locks may be busy for unrelated read/preparation work.
    // A scrape never acquires any of them or walks their filesystem namespace.
    let prune = kura.prune_lock.lock();
    let canonical = kura.canonical_chain_lock.lock();
    let writer = kura.block_store_write_lock.lock();
    let store = kura.block_store.lock();
    let sidecar = kura.sidecar_lock.lock();
    assert_eq!(
        kura.resource_inventory_snapshot().unwrap().components,
        expected
    );
    let bootstrap = kura.provisional_snapshot_bootstrap.lock();
    assert!(matches!(
        kura.resource_inventory_snapshot(),
        Err(Unavailable::Busy)
    ));
    drop(bootstrap);
    assert_eq!(
        kura.resource_inventory_snapshot().unwrap().components,
        expected
    );
    drop(sidecar);
    drop(store);
    drop(writer);
    drop(canonical);
    drop(prune);
    let mutation = kura
        .resource_inventory
        .begin(ResourceFamily::ResidentQueue.mask())
        .unwrap();
    assert!(matches!(
        kura.resource_inventory_snapshot(),
        Err(Unavailable::Busy)
    ));
    mutation
        .publish(&[(
            ResourceFamily::ResidentQueue,
            ResourceUsage::default(),
            ResourceUsage::default(),
        )])
        .unwrap();
    assert_eq!(
        kura.resource_inventory_snapshot().unwrap().components,
        expected
    );
}

#[test]
fn resource_snapshot_rejects_every_deferred_or_unresolved_owner_state_after_initialization() {
    let mut kura = snapshot_owner_fixture();
    let expected = kura.resource_inventory_snapshot().unwrap().components;
    Arc::get_mut(&mut kura).unwrap().auxiliary_history_deferred = true;
    assert!(matches!(
        kura.resource_inventory_snapshot(),
        Err(Unavailable::Unregistered)
    ));
    Arc::get_mut(&mut kura).unwrap().auxiliary_history_deferred = false;
    *kura.provisional_snapshot_bootstrap.lock() = SnapshotBootstrapRuntimeState::Finalizing;
    assert!(matches!(
        kura.resource_inventory_snapshot(),
        Err(Unavailable::Unregistered)
    ));
    *kura.provisional_snapshot_bootstrap.lock() = SnapshotBootstrapRuntimeState::Authenticated;
    // These are availability-state controls only: they neither publish durable
    // application evidence nor grant a mutation/recovery authority.
    for (flag, invalid_value, expected_error) in [
        (
            &kura.canonical_storage_poisoned,
            true,
            Unavailable::InvalidInventory,
        ),
        (
            &kura.latest_certified_frontier_storage_unknown,
            true,
            Unavailable::InvalidInventory,
        ),
        (
            &kura.prune_recovery_required,
            true,
            Unavailable::InvalidInventory,
        ),
        (&kura.prune_in_progress, true, Unavailable::Busy),
        (
            &kura.post_wsv_resident_recovery_complete,
            false,
            Unavailable::Unregistered,
        ),
        (
            &kura.certified_resident_recovery_complete,
            false,
            Unavailable::Unregistered,
        ),
    ] {
        flag.store(invalid_value, Ordering::Release);
        assert_eq!(
            kura.resource_inventory_snapshot().unwrap_err(),
            expected_error
        );
        flag.store(!invalid_value, Ordering::Release);
        assert_eq!(
            kura.resource_inventory_snapshot().unwrap().components,
            expected
        );
    }
}

#[test]
fn canonical_read_poison_invalidates_physical_generation_after_closing_consensus() {
    let kura = snapshot_owner_fixture();
    let guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    kura.bind_consensus_output_guard(Arc::clone(&guard))
        .unwrap();
    assert!(guard.acquire().is_some());
    let generation = kura.resource_inventory.reconciliation_generation().unwrap();
    let scope = kura.physical_resource_scope().unwrap();
    let counts = scope.observe(kura.evidence_resource_limits()).unwrap();
    let old_values = PHYSICAL_RESOURCE_FAMILIES
        .iter()
        .map(|family| (*family, counts[*family as usize]))
        .collect::<Vec<_>>();
    let resident_before = kura
        .resource_inventory
        .component_usage_for_tests(ResourceFamily::ResidentCanonical)
        .unwrap();
    let result: Result<()> = kura.consensus_storage_read(Err(Error::CanonicalStoragePoisoned));
    assert!(matches!(result, Err(Error::CanonicalStoragePoisoned)));
    assert!(guard.restart_required());
    assert!(guard.acquire().is_none());
    assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert!(matches!(
        kura.resource_inventory_snapshot(),
        Err(Unavailable::InvalidInventory)
    ));
    assert!(kura.resource_inventory.reconciliation_generation().unwrap() > generation);
    assert_eq!(
        kura.resource_inventory.initialize(generation, &old_values),
        Err(Unavailable::GenerationChanged)
    );
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert_eq!(
            kura.resource_inventory.component_usage_for_tests(family),
            Err(Unavailable::InvalidInventory)
        );
    }
    assert_eq!(
        kura.resource_inventory
            .component_usage_for_tests(ResourceFamily::ResidentCanonical)
            .unwrap(),
        resident_before
    );
    assert!(kura.reconcile_physical_resource_inventory().is_err());
    assert!(kura.resource_inventory_snapshot().is_err());
}

#[test]
fn resource_snapshot_rechecks_poison_after_capturing_a_complete_vector() {
    let kura = snapshot_owner_fixture();
    let observed = std::cell::Cell::new(false);
    let result = kura.resource_inventory_snapshot_after_observation(|| {
        observed.set(true);
        kura.poison_canonical_storage(
            "resource snapshot observation race",
            &Error::CanonicalStoragePoisoned,
        );
    });
    assert!(
        observed.get(),
        "the fixture must cross an actually complete registry observation"
    );
    assert!(matches!(result, Err(Unavailable::InvalidInventory)));
    assert!(kura.resource_inventory.try_snapshot().is_err());
    assert!(kura.resource_inventory_snapshot().is_err());
}

#[test]
fn resource_snapshot_rejects_unavailable_owner_before_observation_hook() {
    let kura = snapshot_owner_fixture();
    let expected = kura.resource_inventory_snapshot().unwrap().components;
    // The registry remains complete: only the owning Kura availability changes.
    // This independently protects the first gate, before any observation hook.
    kura.prune_in_progress.store(true, Ordering::Release);
    assert_eq!(
        kura.resource_inventory.try_snapshot().unwrap().components,
        expected
    );
    let observed = std::cell::Cell::new(false);
    let result = kura.resource_inventory_snapshot_after_observation(|| observed.set(true));
    assert!(matches!(result, Err(Unavailable::Busy)));
    assert!(
        !observed.get(),
        "an unavailable owner must reject before the observation hook"
    );
    kura.prune_in_progress.store(false, Ordering::Release);
    let restored = kura
        .resource_inventory_snapshot_after_observation(|| observed.set(true))
        .unwrap();
    assert!(
        observed.get(),
        "the restored complete owner must reach the observation hook"
    );
    assert_eq!(restored.components, expected);
    assert_eq!(
        kura.resource_inventory_snapshot().unwrap().components,
        expected
    );
}
