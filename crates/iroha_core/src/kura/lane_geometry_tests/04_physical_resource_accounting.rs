// Real geometry publication, move, and authenticated GC owners with full physical recounts.
use crate::kura::{IndexResourceCounts, PHYSICAL_RESOURCE_FAMILIES, ResourceFamily};

fn geometry_physical_initialize(kura: &Kura) -> IndexResourceCounts {
    let actual = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .unwrap();
    kura.resource_inventory
        .initialize(
            kura.resource_inventory.reconciliation_generation().unwrap(),
            &PHYSICAL_RESOURCE_FAMILIES
                .iter()
                .map(|family| (*family, actual[*family as usize]))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    geometry_physical_assert_actual(kura)
}

fn geometry_physical_assert_actual(kura: &Kura) -> IndexResourceCounts {
    let actual = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .unwrap();
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert_eq!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .unwrap(),
            actual[family as usize],
            "{family:?}"
        );
    }
    actual
}

fn geometry_physical_assert_unavailable(kura: &Kura) {
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err(),
            "{family:?}"
        );
    }
}

fn geometry_physical_binding(label: &str) -> LaneGeometryBinding {
    LaneGeometryBinding {
        lane_id: LaneId::new(20),
        incarnation: Hash::new(label.as_bytes()),
        activation_height: 1,
        blocks_path: format!("blocks/{label}/blocks"),
        merge_path: format!("merge_ledger/{label}.log"),
    }
}

#[test]
fn geometry_physical_provision_counts_real_markers_and_is_idempotent() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let binding = geometry_physical_binding("new-physical-lane");
    let before = geometry_physical_initialize(&kura);
    kura.provision_geometry_binding(&binding).unwrap();
    let after = geometry_physical_assert_actual(&kura);
    let blocks = kura.binding_blocks_path(&binding);
    let expected = fs::metadata(blocks.join(COUNT_FILE_NAME)).unwrap().len()
        + fs::metadata(blocks.join(MARKER_FILE_NAME)).unwrap().len();
    assert!(expected > 0);
    assert_eq!(
        after[ResourceFamily::StorageBytes as usize].storage_bytes
            - before[ResourceFamily::StorageBytes as usize].storage_bytes,
        expected
    );
    assert_eq!(
        after[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries
            - before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
        2
    );
    kura.provision_geometry_binding(&binding).unwrap();
    assert_eq!(geometry_physical_assert_actual(&kura), after);
}

#[test]
fn geometry_physical_atomic_replacement_and_exact_removal_count_main_and_temp_once() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let owner = root.join("blocks/atomic-physical-owner");
    fs::create_dir(&owner).unwrap();
    let path = owner.join(MARKER_FILE_NAME);
    let temp = owner.join(MARKER_TEMP_FILE_NAME);
    fs::write(&path, [1_u8; 9]).unwrap();
    fs::write(&temp, [2_u8; 23]).unwrap();
    let before = geometry_physical_initialize(&kura);
    kura.atomic_write_geometry_file(&path, &temp, &[2_u8; 23])
        .unwrap();
    let after = geometry_physical_assert_actual(&kura);
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        9
    );
    assert_eq!(
        before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries
            - after[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
        1
    );
    assert!(!temp.exists());
    assert_eq!(fs::read(&path).unwrap(), [2_u8; 23]);
    kura.remove_accounted_geometry_file(&path).unwrap();
    let removed = geometry_physical_assert_actual(&kura);
    assert_eq!(
        after[ResourceFamily::StorageBytes as usize].storage_bytes
            - removed[ResourceFamily::StorageBytes as usize].storage_bytes,
        23
    );
    assert!(!path.exists());
}

#[test]
fn geometry_physical_rejected_atomic_temp_keeps_inventory_unavailable() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let owner = root.join("blocks/rejected-atomic-owner");
    fs::create_dir(&owner).unwrap();
    let path = owner.join(MARKER_FILE_NAME);
    let temp = owner.join(MARKER_TEMP_FILE_NAME);
    fs::write(&path, [1_u8; 9]).unwrap();
    fs::write(&temp, [3_u8; 23]).unwrap();
    geometry_physical_initialize(&kura);
    assert!(
        kura.atomic_write_geometry_file(&path, &temp, &[2_u8; 23])
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), [1_u8; 9]);
    assert_eq!(fs::read(&temp).unwrap(), [3_u8; 23]);
    geometry_physical_assert_unavailable(&kura);
}

#[test]
fn geometry_physical_file_and_directory_moves_observe_both_endpoints() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let source = root.join("blocks/move-source");
    let target = root.join("blocks/new-parent/move-target");
    fs::create_dir(&source).unwrap();
    let index = source.join(INDEX_FILE_NAME);
    fs::write(&index, [0_u8; 32]).unwrap();
    fs::write(source.join(DATA_FILE_NAME), [1_u8; 9]).unwrap();
    let before = geometry_physical_initialize(&kura);
    kura.move_geometry_path(&source, &target, true).unwrap();
    assert_eq!(geometry_physical_assert_actual(&kura), before);
    assert!(!source.exists());
    let temporary = target.join(format!("{INDEX_FILE_NAME}.tmp"));
    kura.move_geometry_path(&target.join(INDEX_FILE_NAME), &temporary, false)
        .unwrap();
    let after = geometry_physical_assert_actual(&kura);
    assert_eq!(
        after[ResourceFamily::StorageBytes as usize],
        before[ResourceFamily::StorageBytes as usize]
    );
    assert_eq!(
        before[ResourceFamily::CanonicalIndex as usize].index_bytes
            - after[ResourceFamily::CanonicalIndex as usize].index_bytes,
        32
    );
    assert_eq!(
        after[ResourceFamily::CanonicalIndex as usize].temporary_index_bytes
            - before[ResourceFamily::CanonicalIndex as usize].temporary_index_bytes,
        32
    );
}

#[test]
fn geometry_physical_stale_marker_cleanup_publishes_bytes_and_requests_old_cache_rescan() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let binding = geometry_physical_binding("stale-marker-owner");
    kura.provision_geometry_binding(&binding).unwrap();
    let blocks = kura.binding_blocks_path(&binding);
    let path = blocks.join(MARKER_FILE_NAME);
    let temp = blocks.join(MARKER_TEMP_FILE_NAME);
    let stale = kura.read_lane_marker(&path).unwrap();
    let mut intended = stale.clone();
    intended.move_target_blocks = Some("blocks/next-marker-owner/blocks".to_owned());
    intended.move_target_merge = Some("merge_ledger/next-marker-owner.log".to_owned());
    fs::write(&temp, stale.encode()).unwrap();
    let removed_bytes = fs::metadata(&temp).unwrap().len();
    let before = geometry_physical_initialize(&kura);
    kura.disk_usage_total_initialized
        .store(true, std::sync::atomic::Ordering::Relaxed);
    kura.prepare_lane_marker_temp_for_write(&temp, &binding, &intended)
        .unwrap();
    let after = geometry_physical_assert_actual(&kura);
    assert!(!temp.exists());
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        removed_bytes
    );
    assert_eq!(
        before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries
            - after[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
        1
    );
    assert!(
        !kura
            .disk_usage_total_initialized
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    assert_eq!(kura.read_lane_marker(&path).unwrap(), stale);
}

#[test]
fn geometry_physical_authenticated_archive_quarantine_and_delete_are_one_delta() {
    for interrupt in [false, true] {
        let directory = TempDir::new().unwrap();
        let root = directory.path().join("kura");
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
        assert!(checkpoint_retired_geometry(&kura, &fixture, 20).is_err());
        let journal = kura.read_lane_geometry_journal().unwrap();
        assert!(!journal.pending_archive_gc.is_empty());
        geometry_physical_initialize(&kura);
        if interrupt {
            kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_ARCHIVE_QUARANTINE);
            assert!(
                kura.remove_authenticated_geometry_archive(&journal.pending_archive_gc[0], &[])
                    .is_err()
            );
            geometry_physical_assert_unavailable(&kura);
        } else {
            let mut removed_total = 0_u64;
            let before = geometry_physical_assert_actual(&kura);
            for pending in &journal.pending_archive_gc {
                let (removed, did_remove) = kura
                    .remove_authenticated_geometry_archive(pending, &[])
                    .unwrap();
                assert!(did_remove);
                removed_total = removed_total.checked_add(removed).unwrap();
                geometry_physical_assert_actual(&kura);
                assert_eq!(
                    kura.remove_authenticated_geometry_archive(pending, &[])
                        .unwrap(),
                    (0, false)
                );
            }
            let after = geometry_physical_assert_actual(&kura);
            assert!(removed_total >= GC_PAYLOAD_LEN as u64);
            assert_eq!(
                before[ResourceFamily::StorageBytes as usize].storage_bytes
                    - after[ResourceFamily::StorageBytes as usize].storage_bytes,
                removed_total
            );
            assert!(!fixture.archive_root.exists());
        }
    }
}
