// Exact physical accounting at metadata suffix deletion and recovery boundaries.

#[derive(Clone, Copy, Debug)]
enum MetadataPhysicalKind {
    Wsv,
    Manifest,
    Finality,
}

fn metadata_physical_directory(kura: &Kura, kind: MetadataPhysicalKind) -> PathBuf {
    match kind {
        MetadataPhysicalKind::Wsv => kura.wsv_checkpoint_dir(),
        MetadataPhysicalKind::Manifest => kura.commit_manifest_dir(),
        MetadataPhysicalKind::Finality => kura.v2_finality_artifact_dir(),
    }
}

fn metadata_physical_fixture() -> (TempDir, Arc<Kura>) {
    let directory = TempDir::new().unwrap();
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    (directory, kura)
}

fn metadata_physical_register(kura: &Kura) -> IndexResourceCounts {
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
    metadata_physical_assert_actual(kura)
}

fn metadata_physical_assert_actual(kura: &Kura) -> IndexResourceCounts {
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

fn metadata_physical_prune(kura: &Kura, kind: MetadataPhysicalKind, height: u64) -> Result<()> {
    let _prune_guard = kura.prune_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    let directory = metadata_physical_directory(kura, kind);
    match kind {
        MetadataPhysicalKind::Wsv => kura
            .prune_wsv_checkpoints_above_in_dir(&directory, height)
            .map(|_| ()),
        MetadataPhysicalKind::Manifest => {
            kura.prune_commit_manifests_above_in_dir(&directory, height)
        }
        MetadataPhysicalKind::Finality => {
            kura.prune_v2_finality_artifacts_above_in_dir(&directory, height)
        }
    }
}

fn exercise_metadata_physical_suffix(kind: MetadataPhysicalKind) {
    let (_directory, kura) = metadata_physical_fixture();
    let owner = metadata_physical_directory(&kura, kind);
    fs::create_dir_all(&owner).unwrap();
    let retained = owner.join(format!("{:020}.norito", 1));
    let removed = owner.join(format!("{:020}.norito", 2));
    // Prune owners select the same numbered filenames as before; these byte
    // fixtures exercise physical representation and do not grant evidence authority.
    fs::write(&retained, [1_u8; 9]).unwrap();
    fs::write(&removed, [2_u8; 23]).unwrap();
    let before = metadata_physical_register(&kura);
    metadata_physical_prune(&kura, kind, 1).unwrap();
    let after = metadata_physical_assert_actual(&kura);
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        23
    );
    assert_eq!(
        before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries
            - after[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
        1
    );
    assert_eq!(fs::read(&retained).unwrap(), [1_u8; 9]);
    assert!(!removed.exists());
    assert!(!kura.disk_usage_total_initialized.load(Ordering::Relaxed));
    metadata_physical_prune(&kura, kind, 1).unwrap();
    assert_eq!(metadata_physical_assert_actual(&kura), after);
}

#[test]
fn metadata_physical_wsv_prune_counts_only_selected_actual_files() {
    exercise_metadata_physical_suffix(MetadataPhysicalKind::Wsv);
}

#[test]
fn metadata_physical_manifest_prune_counts_only_selected_actual_files() {
    exercise_metadata_physical_suffix(MetadataPhysicalKind::Manifest);
}

#[test]
fn metadata_physical_finality_prune_counts_only_selected_actual_files() {
    exercise_metadata_physical_suffix(MetadataPhysicalKind::Finality);
}

#[test]
fn metadata_physical_failed_suffix_owner_cannot_publish_partial_success() {
    for kind in [
        MetadataPhysicalKind::Wsv,
        MetadataPhysicalKind::Manifest,
        MetadataPhysicalKind::Finality,
    ] {
        let (_directory, kura) = metadata_physical_fixture();
        let owner = metadata_physical_directory(&kura, kind);
        fs::create_dir_all(&owner).unwrap();
        let retained = owner.join(format!("{:020}.norito", 1));
        fs::write(&retained, [1_u8; 9]).unwrap();
        fs::write(owner.join(format!("{:020}.norito", 2)), [2_u8; 23]).unwrap();
        let blocked = owner.join(format!("{:020}.norito", 3));
        fs::create_dir(&blocked).unwrap();
        metadata_physical_register(&kura);
        assert!(metadata_physical_prune(&kura, kind, 1).is_err());
        assert_eq!(fs::read(retained).unwrap(), [1_u8; 9]);
        assert!(blocked.is_dir());
        for family in PHYSICAL_RESOURCE_FAMILIES {
            assert!(
                kura.resource_inventory
                    .component_usage_for_tests(family)
                    .is_err(),
                "{kind:?}: {family:?}"
            );
        }
    }
}

#[test]
fn metadata_physical_absent_and_empty_suffix_owners_preserve_measured_baselines() {
    for kind in [
        MetadataPhysicalKind::Wsv,
        MetadataPhysicalKind::Manifest,
        MetadataPhysicalKind::Finality,
    ] {
        let (_directory, kura) = metadata_physical_fixture();
        let owner = metadata_physical_directory(&kura, kind);
        let before = metadata_physical_register(&kura);
        metadata_physical_prune(&kura, kind, 1).unwrap();
        assert_eq!(metadata_physical_assert_actual(&kura), before);
        fs::create_dir_all(owner).unwrap();
        let before = metadata_physical_register(&kura);
        metadata_physical_prune(&kura, kind, 1).unwrap();
        assert_eq!(metadata_physical_assert_actual(&kura), before);
    }
}

#[test]
fn metadata_physical_finality_wrapper_publishes_its_leaf_owner() {
    let (_directory, kura) = metadata_physical_fixture();
    let owner = kura.v2_finality_artifact_dir();
    fs::create_dir_all(&owner).unwrap();
    let removed = owner.join(format!("{:020}.norito", 2));
    fs::write(&removed, [2_u8; 23]).unwrap();
    let before = metadata_physical_register(&kura);
    let _prune_guard = kura.prune_lock.lock();
    kura.prune_v2_finality_artifacts_above(1).unwrap();
    let after = metadata_physical_assert_actual(&kura);
    assert!(!removed.exists());
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        23
    );
}

#[test]
fn metadata_physical_retired_tree_purge_subtracts_both_exact_roots() {
    let (_directory, kura) = metadata_physical_fixture();
    let retired = kura.store_root.join("retired");
    let blocks = retired.join("blocks/fixture");
    let merge = retired.join("merge_ledger");
    fs::create_dir_all(&blocks).unwrap();
    fs::create_dir_all(&merge).unwrap();
    fs::write(blocks.join(DATA_FILE_NAME), [1_u8; 9]).unwrap();
    fs::write(blocks.join(INDEX_FILE_NAME), [0_u8; 16]).unwrap();
    fs::write(merge.join("fixture.log"), [2_u8; 23]).unwrap();
    let before = metadata_physical_register(&kura);
    assert!(kura.purge_retired_segments().unwrap());
    let after = metadata_physical_assert_actual(&kura);
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        48
    );
    assert!(!retired.join("blocks").exists());
    assert!(!retired.join("merge_ledger").exists());
    assert!(!kura.purge_retired_segments().unwrap());
    assert_eq!(metadata_physical_assert_actual(&kura), after);
}

#[test]
fn metadata_physical_failed_retired_tree_purge_stays_unavailable() {
    let (_directory, kura) = metadata_physical_fixture();
    let blocks = kura.store_root.join("retired/blocks/fixture");
    fs::create_dir_all(&blocks).unwrap();
    fs::write(blocks.join(DATA_FILE_NAME), [1_u8; 9]).unwrap();
    fs::write(blocks.join(INDEX_FILE_NAME), [0_u8; 16]).unwrap();
    metadata_physical_register(&kura);
    kura.fail_next_retired_tree_purge_after_one_removal_for_tests();
    assert!(!kura.purge_retired_segments().unwrap());
    assert_eq!(fs::read_dir(&blocks).unwrap().count(), 1);
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err(),
            "{family:?}"
        );
    }
    assert!(blocks.exists());
}

#[test]
fn metadata_physical_top_replacement_prunes_exact_wsv_evidence() {
    let (_directory, kura) = metadata_physical_fixture();
    let blocks = store_dummy_block_arcs(&kura, 3);
    // Only the uncommitted top may be replaced. Committed prefix evidence must
    // survive; actual evidence deletion is exercised by canonical prune below.
    for (index, block) in blocks.iter().take(2).enumerate() {
        kura.store_wsv_checkpoint((index + 1) as u64, block.hash(), Hash::new([index as u8]))
            .unwrap();
    }
    metadata_physical_register(&kura);
    let replacement: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(3_u64));
            header.set_prev_block_hash(Some(blocks[1].hash()));
            header.set_view_change_index(blocks[2].header().view_change_index().saturating_add(1));
        })
        .into(),
    );
    kura.replace_top_block(Arc::clone(&replacement)).unwrap();
    metadata_physical_assert_actual(&kura);
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(3_usize)),
        Some(replacement.hash())
    );
    assert!(kura.wsv_checkpoint_path(1).is_file());
    assert!(kura.wsv_checkpoint_path(2).is_file());
    assert!(!kura.wsv_checkpoint_path(3).exists());
    kura.replace_top_block(replacement).unwrap();
    metadata_physical_assert_actual(&kura);

    // Publishing the top checkpoint changes its authority, not its hash. A
    // subsequent distinct replacement must reject before any physical mutation.
    let committed = kura.get_block(nonzero!(3_usize)).unwrap();
    assert_ne!(committed.hash(), blocks[2].hash());
    kura.store_wsv_checkpoint(3, committed.hash(), Hash::new(b"committed replacement"))
        .unwrap();
    let checkpoint_bytes = (1..=3)
        .map(|height| fs::read(kura.wsv_checkpoint_path(height)).unwrap())
        .collect::<Vec<_>>();
    let before_rejection = metadata_physical_assert_actual(&kura);
    assert!(matches!(
        kura.replace_top_block(Arc::clone(&blocks[2])),
        Err(Error::CommittedBlockReplacementForbidden { height: 3 })
    ));
    assert_eq!(metadata_physical_assert_actual(&kura), before_rejection);
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(3_usize)),
        Some(committed.hash())
    );
    for (index, expected) in checkpoint_bytes.iter().enumerate() {
        assert_eq!(
            &fs::read(kura.wsv_checkpoint_path((index + 1) as u64)).unwrap(),
            expected
        );
    }
    assert!(!kura.canonical_association_stage_path().exists());
    kura.replace_top_block(committed).unwrap();
    assert_eq!(metadata_physical_assert_actual(&kura), before_rejection);
}

#[test]
fn metadata_physical_canonical_prune_keeps_leaf_deltas_disjoint() {
    let (_directory, kura) = metadata_physical_fixture();
    let blocks = store_dummy_block_arcs(&kura, 3);
    for (index, block) in blocks.iter().enumerate() {
        kura.store_wsv_checkpoint((index + 1) as u64, block.hash(), Hash::new([index as u8]))
            .unwrap();
    }
    metadata_physical_register(&kura);
    kura.prune_to_height(1).unwrap();
    let after = metadata_physical_assert_actual(&kura);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(blocks[0].hash())
    );
    assert!(kura.wsv_checkpoint_path(1).is_file());
    assert!(!kura.wsv_checkpoint_path(2).exists());
    assert!(!kura.wsv_checkpoint_path(3).exists());
    assert!(!kura.prune_recovery_required.load(Ordering::Acquire));
    kura.prune_to_height(1).unwrap();
    assert_eq!(metadata_physical_assert_actual(&kura), after);
}
