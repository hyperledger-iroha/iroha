//! Scoped physical startup/re-audit observations on real Kura storage fixtures.

use super::*;
use resource_inventory::Unavailable;

fn physical_component(kura: &Kura, family: ResourceFamily) -> ResourceUsage {
    kura.resource_inventory
        .component_usage_for_tests(family)
        .unwrap()
}

#[test]
fn physical_initialization_observes_owned_trees_markers_and_all_fifteen_families() {
    let kura = Kura::blank_kura_for_testing();
    kura.reconcile_physical_resource_inventory().unwrap();
    let before = physical_component(&kura, ResourceFamily::StorageBytes).storage_bytes;
    let blocks = kura.store_root.join("retired/blocks/resource-fixture");
    let mut added = 0_u64;
    let mut write = |path: PathBuf, bytes: Vec<u8>| {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        assert!(!path.exists(), "fixture must add distinct retained storage");
        added += bytes.len() as u64;
        std::fs::write(path, bytes).unwrap();
    };
    for (name, family) in [
        (PIPELINE_SIDECARS_INDEX_FILE, ResourceFamily::PipelineIndex),
        (LANE_ARTIFACTS_INDEX_FILE, ResourceFamily::OwnershipIndex),
        (
            CERTIFIED_LANE_BLOCKS_INDEX_FILE,
            ResourceFamily::CertifiedIndex,
        ),
        (
            LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE,
            ResourceFamily::ExecutionInputIndex,
        ),
        (
            LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE,
            ResourceFamily::ExecutionPreflightIndex,
        ),
        (
            LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE,
            ResourceFamily::ApplicationReceiptIndex,
        ),
        (
            AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE,
            ResourceFamily::MergeBundleIndex,
        ),
        (
            CANONICAL_AUTONOMOUS_LANE_REPLICAS_INDEX_FILE,
            ResourceFamily::CanonicalReplicaIndex,
        ),
    ] {
        let mut bytes = SidecarIndexLayout::base_header(1).to_vec();
        bytes.extend_from_slice(&[0_u8; PIPELINE_INDEX_ENTRY_SIZE]);
        write(blocks.join(name), bytes);
        assert_eq!(physical_component(&kura, family).persisted_entries, 0);
    }
    write(
        kura.store_root.join(MERGE_CARRIERS_DIR).join("1.norito"),
        vec![7; 9],
    );
    write(
        blocks.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE),
        vec![8; 11],
    );
    write(
        kura.store_root
            .join(crate::query::index_status::QueryIndexJournal::JOURNAL_FILE),
        vec![9; 13],
    );
    write(
        kura.store_root.join(
            "retired/lane_geometry/a/blocks/lane/retained_blocks/00000000000000000001.norito",
        ),
        vec![10; 15],
    );
    write(
        kura.store_root.join("merge_ledger/retained.norito"),
        vec![11; 17],
    );
    write(
        kura.lane_geometry_journal_path()
            .with_extension("norito.restore.tmp"),
        vec![12; 19],
    );
    drop(write);
    // Consensus stores own these files independently. They cannot inflate Kura's
    // physical family or be mistaken for an uninstrumented Kura writer.
    let delegated = kura
        .sumeragi_v2_storage_root()
        .join("wal/transactions.index");
    std::fs::create_dir_all(delegated.parent().unwrap()).unwrap();
    std::fs::write(&delegated, [12_u8; 101]).unwrap();
    kura.reconcile_physical_resource_inventory().unwrap();
    assert_eq!(
        physical_component(&kura, ResourceFamily::StorageBytes).storage_bytes,
        before + added
    );
    for family in PHYSICAL_RESOURCE_FAMILIES {
        let usage = physical_component(&kura, family);
        assert_eq!(usage.resident_associations, 0);
        if !matches!(
            family,
            ResourceFamily::CanonicalIndex
                | ResourceFamily::CanonicalHashes
                | ResourceFamily::StorageBytes
        ) {
            assert!(usage.persisted_entries > 0, "{family:?}");
        }
    }
    assert!(
        kura.resource_inventory_snapshot().is_err(),
        "physical families cannot qualify uninitialized resident owners"
    );
}

#[test]
fn physical_scope_rejects_duplicate_overlap_and_whole_store_roots() {
    let kura = Kura::blank_kura_for_testing();
    let mut scope = kura.physical_resource_scope().unwrap();
    scope.trees[1] = scope.trees[0].clone();
    assert!(scope.validate_disjoint(&kura.store_root).is_err());
    scope.trees[1] = scope.trees[0].join("nested");
    assert!(scope.validate_disjoint(&kura.store_root).is_err());
    scope.trees[1] = kura.store_root.clone();
    assert!(scope.validate_disjoint(&kura.store_root).is_err());
    let mut scope = kura.physical_resource_scope().unwrap();
    scope.files[1] = scope.files[0].clone();
    assert!(scope.validate_disjoint(&kura.store_root).is_err());
}

#[test]
fn physical_scope_retains_existing_and_absent_tree_parent_identity() {
    for occupied in [false, true] {
        let kura = Kura::blank_kura_for_testing();
        let retired = kura.store_root.join("retired");
        std::fs::create_dir_all(&retired).unwrap();
        if occupied {
            std::fs::create_dir(retired.join("blocks")).unwrap();
        }
        let scope = kura.physical_resource_scope().unwrap();
        scope.observe(kura.evidence_resource_limits()).unwrap();
        std::fs::rename(&retired, kura.store_root.join("moved-retired-fixture")).unwrap();
        std::fs::create_dir_all(&retired).unwrap();
        assert!(scope.validate_bindings().is_err(), "occupied={occupied}");
        assert!(scope.observe(kura.evidence_resource_limits()).is_err());
    }
}

#[test]
fn physical_reaudit_failure_invalidates_every_family_and_requires_complete_new_audit() {
    let kura = Kura::blank_kura_for_testing();
    kura.reconcile_physical_resource_inventory().unwrap();
    let expected = physical_component(&kura, ResourceFamily::StorageBytes);
    let foreign = kura.active_blocks_dir.lock().join("unregistered.index");
    std::fs::write(&foreign, [0_u8; 3]).unwrap();
    assert!(kura.reconcile_physical_resource_inventory().is_err());
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err(),
            "{family:?}"
        );
    }
    std::fs::remove_file(foreign).unwrap();
    assert!(
        kura.resource_inventory
            .component_usage_for_tests(ResourceFamily::StorageBytes)
            .is_err()
    );
    kura.reconcile_physical_resource_inventory().unwrap();
    assert_eq!(
        physical_component(&kura, ResourceFamily::StorageBytes),
        expected
    );
}

#[test]
#[cfg(all(unix, not(target_os = "espidf")))]
fn physical_reaudit_rejects_busy_resident_owner_and_real_writer_generation_crossing() {
    let kura = Kura::blank_kura_for_testing();
    kura.reconcile_physical_resource_inventory().unwrap();
    let resident = kura
        .resource_inventory
        .begin(ResourceFamily::ResidentQueue.mask())
        .unwrap();
    assert_eq!(
        kura.reconcile_physical_resource_inventory(),
        Err(Unavailable::Busy)
    );
    drop(resident);
    kura.reconcile_physical_resource_inventory().unwrap();
    let generation = kura.resource_inventory.reconciliation_generation().unwrap();
    let scope = kura.physical_resource_scope().unwrap();
    let counts = scope.observe(kura.evidence_resource_limits()).unwrap();
    let receipt = kura
        .persist_fastpq_artifact(b"real intervening writer")
        .unwrap();
    assert_eq!(
        kura.read_fastpq_artifact(receipt.reference()).unwrap(),
        b"real intervening writer"
    );
    let values = PHYSICAL_RESOURCE_FAMILIES
        .iter()
        .map(|family| (*family, counts[*family as usize]))
        .collect::<Vec<_>>();
    assert_eq!(
        kura.resource_inventory.initialize(generation, &values),
        Err(Unavailable::GenerationChanged)
    );
    kura.reconcile_physical_resource_inventory().unwrap();
    assert!(
        physical_component(&kura, ResourceFamily::StorageBytes).storage_bytes
            > counts[ResourceFamily::StorageBytes as usize].storage_bytes
    );
}

#[test]
fn physical_initialization_rejects_deferred_finalizing_poisoned_and_recovery_pending_states() {
    let mut kura = Kura::blank_kura_for_testing();
    std::sync::Arc::get_mut(&mut kura)
        .unwrap()
        .auxiliary_history_deferred = true;
    assert!(kura.reconcile_physical_resource_inventory().is_err());
    std::sync::Arc::get_mut(&mut kura)
        .unwrap()
        .auxiliary_history_deferred = false;
    *kura.provisional_snapshot_bootstrap.lock() = SnapshotBootstrapRuntimeState::Finalizing;
    assert!(kura.reconcile_physical_resource_inventory().is_err());
    *kura.provisional_snapshot_bootstrap.lock() = SnapshotBootstrapRuntimeState::Authenticated;
    kura.canonical_storage_poisoned
        .store(true, Ordering::Release);
    assert!(kura.reconcile_physical_resource_inventory().is_err());
    kura.canonical_storage_poisoned
        .store(false, Ordering::Release);
    kura.prune_recovery_required.store(true, Ordering::Release);
    assert!(kura.reconcile_physical_resource_inventory().is_err());
    kura.prune_recovery_required.store(false, Ordering::Release);
    kura.reconcile_physical_resource_inventory().unwrap();
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_ok()
        );
    }
}

#[test]
#[cfg(all(unix, not(target_os = "espidf")))]
fn physical_initialization_checks_full_fastpq_policy_before_registering_any_family() {
    let mut kura = Kura::blank_kura_for_testing();
    std::sync::Arc::get_mut(&mut kura)
        .unwrap()
        .fastpq_artifact_policy = iroha_config::parameters::actual::KuraFastpqArtifactPolicy {
        max_artifact_bytes: std::num::NonZeroUsize::new(8).unwrap(),
        max_artifacts: std::num::NonZeroUsize::new(1).unwrap(),
        max_total_bytes: std::num::NonZeroU64::new(8).unwrap(),
    };
    kura.persist_fastpq_artifact(b"12345678").unwrap();
    let pending = kura
        .store_root
        .join(fastpq_artifact_store::DIRECTORY)
        .join(fastpq_artifact_store::TEMPORARY);
    std::fs::write(&pending, b"x").unwrap();
    assert!(kura.reconcile_physical_resource_inventory().is_err());
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err()
        );
    }
    assert_eq!(std::fs::read(&pending).unwrap(), b"x");
    std::fs::write(&pending, []).unwrap();
    kura.reconcile_physical_resource_inventory().unwrap();
    assert!(physical_component(&kura, ResourceFamily::EvidenceKeyRecords).persisted_entries >= 2);
}

#[test]
fn physical_root_discovery_is_bounded_and_rejects_unresolved_reserved_residue() {
    let kura = Kura::blank_kura_for_testing();
    let malformed = kura.store_root.join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}unresolved"
    ));
    std::fs::write(&malformed, b"x").unwrap();
    assert!(kura.physical_resource_scope().is_err());
    std::fs::remove_file(malformed).unwrap();
    for index in 0..=AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ROOT_ENTRY_LIMIT {
        std::fs::write(kura.store_root.join(format!("foreign-{index}")), []).unwrap();
    }
    assert!(kura.physical_resource_scope().is_err());
}

#[test]
fn physical_writer_scope_uses_same_owned_roots_and_rejects_delegated_or_new_roots() {
    let kura = Kura::blank_kura_for_testing();
    for tree in kura.physical_resource_owned_trees() {
        assert!(kura.physical_resource_path_is_owned(&tree));
        assert!(kura.physical_resource_path_is_owned(&tree.join("owned.data")));
    }
    for file in kura.physical_resource_fixed_root_files() {
        assert!(kura.physical_resource_path_is_owned(&file));
    }
    for path in [
        kura.store_root.clone(),
        kura.store_root.join(STORE_ROOT_LOCK_FILE_NAME),
        kura.sumeragi_v2_storage_root()
            .join("wal/transactions.index"),
        kura.store_root.join("future-owner/blocks.data"),
        kura.store_root.join("new-marker.norito"),
        kura.store_root.join("blocks/../sumeragi_v2/foreign.data"),
        kura.store_root.join(format!(
            "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-invalid"
        )),
    ] {
        assert!(
            !kura.physical_resource_path_is_owned(&path),
            "{}",
            path.display()
        );
    }
    assert!(
        kura.physical_resource_path_is_owned(&kura.store_root.join(format!(
            "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}pending"
        )))
    );
}

#[test]
#[cfg(all(unix, not(target_os = "espidf")))]
fn physical_reaudit_rejects_replaced_live_kura_root_identity() {
    let kura = Kura::blank_kura_for_testing();
    let root = kura.store_root.clone();
    let displaced = root.with_extension("physical-root-identity-fixture");
    std::fs::rename(&root, &displaced).unwrap();
    std::fs::create_dir(&root).unwrap();
    assert!(kura.physical_resource_scope().is_err());
    assert!(kura.reconcile_physical_resource_inventory().is_err());
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err()
        );
    }
    std::fs::remove_dir(&root).unwrap();
    std::fs::rename(&displaced, &root).unwrap();
    kura.reconcile_physical_resource_inventory().unwrap();
}
