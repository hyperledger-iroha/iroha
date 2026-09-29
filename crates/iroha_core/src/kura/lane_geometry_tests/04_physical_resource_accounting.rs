// Actual geometry publication with full physical recounts.
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
    LaneGeometryBinding::from_identity(LaneStorageIdentity {
        network_id: geometry_fixture_network_id(),
        lane_id: LaneId::new(20),
        dataspace_id: DataSpaceId::UNIVERSAL,
        incarnation: Hash::new(label.as_bytes()),
        activation_height: 1,
    })
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
