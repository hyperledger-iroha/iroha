// Included in Kura's existing test module: real evidence writers and crash protocols.

fn with_native_resource_batch_for_test<T>(
    kura: &Kura,
    operation: impl FnOnce(&mut TotalDiskUsageMutation<'_>) -> Result<T>,
) -> Result<T> {
    let mut resources = kura
        .begin_total_disk_usage_mutation()
        .with_resource_children(1);
    let result = operation(&mut resources)?;
    resources.finish_resources_before_disk_rescan();
    Ok(result)
}

// Only isolated fixture setup may scan. Each family is initialized from actual
// regular files; this neither initializes residents nor qualifies a full snapshot.
fn initialize_actual_physical_evidence_fixture(kura: &Kura) -> IndexResourceCounts {
    let counts = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .expect("observe every actual fixture file before testing production deltas");
    kura.resource_inventory
        .initialize(
            kura.resource_inventory.reconciliation_generation().unwrap(),
            &PHYSICAL_RESOURCE_FAMILIES
                .iter()
                .map(|family| (*family, counts[*family as usize]))
                .collect::<Vec<_>>(),
        )
        .expect("publish actual physical fixture inventory");
    counts
}

fn physical_evidence_component(kura: &Kura, family: ResourceFamily) -> ResourceUsage {
    kura.resource_inventory
        .component_usage_for_tests(family)
        .expect("the exact physical owner completed without fault or an outstanding child")
}

#[test]
fn evidence_resources_follow_real_checkpoint_replacement_and_manifest_join() {
    let directory = TempDir::new().unwrap();
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let block = store_dummy_block_arcs(&kura, 1).remove(0);
    let before = initialize_actual_physical_evidence_fixture(&kura);
    let state_hash = Hash::new(b"actual resource checkpoint");
    kura.store_wsv_checkpoint(1, block.hash(), state_hash)
        .unwrap();
    let path = kura.wsv_checkpoint_path(1);
    let checkpoint_len = fs::metadata(&path).unwrap().len();
    let evidence = physical_evidence_component(&kura, ResourceFamily::EvidenceKeyRecords);
    assert_eq!(
        evidence.persisted_entries,
        before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries + 1
    );
    assert_eq!(
        evidence.index_bytes,
        before[ResourceFamily::EvidenceKeyRecords as usize].index_bytes + checkpoint_len
    );
    assert_eq!(
        evidence.temporary_index_bytes,
        before[ResourceFamily::EvidenceKeyRecords as usize].temporary_index_bytes
    );
    assert!(!path.with_extension("norito.tmp").exists());
    assert_eq!(
        physical_evidence_component(&kura, ResourceFamily::StorageBytes).storage_bytes,
        before[ResourceFamily::StorageBytes as usize].storage_bytes + checkpoint_len
    );
    kura.store_wsv_checkpoint(1, block.hash(), state_hash)
        .unwrap();
    assert_eq!(
        physical_evidence_component(&kura, ResourceFamily::EvidenceKeyRecords),
        evidence
    );
    let manifest = CommitManifest::new(1, block.hash(), None, None, state_hash, None);
    kura.store_commit_manifest(manifest.clone()).unwrap();
    let manifest_len = fs::metadata(kura.commit_manifest_path(1)).unwrap().len();
    let bound_checkpoint_len = fs::metadata(&path).unwrap().len();
    assert!(
        bound_checkpoint_len > checkpoint_len,
        "the exact manifest digest grows the real checkpoint record"
    );
    let joined = physical_evidence_component(&kura, ResourceFamily::EvidenceKeyRecords);
    assert_eq!(
        joined.persisted_entries,
        before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries + 2
    );
    assert_eq!(
        joined.index_bytes,
        before[ResourceFamily::EvidenceKeyRecords as usize].index_bytes
            + manifest_len
            + bound_checkpoint_len
    );
    assert_eq!(
        joined.temporary_index_bytes,
        before[ResourceFamily::EvidenceKeyRecords as usize].temporary_index_bytes
    );
    assert_eq!(
        physical_evidence_component(&kura, ResourceFamily::StorageBytes).storage_bytes,
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            + manifest_len
            + bound_checkpoint_len
    );
    kura.store_commit_manifest(manifest).unwrap();
    assert_eq!(
        physical_evidence_component(&kura, ResourceFamily::EvidenceKeyRecords),
        joined
    );
}

#[test]
fn evidence_resources_keep_failed_checkpoint_io_unavailable_until_actual_reaudit() {
    let directory = TempDir::new().unwrap();
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let block = store_dummy_block_arcs(&kura, 1).remove(0);
    let state_hash = Hash::new(b"stable resource checkpoint");
    kura.store_wsv_checkpoint(1, block.hash(), state_hash)
        .unwrap();
    initialize_actual_physical_evidence_fixture(&kura);
    let path = kura.wsv_checkpoint_path(1);
    let original = fs::read(&path).unwrap();
    // A different state fails before I/O because checkpoints are immutable.
    // Prove that rejection does not invalidate an unchanged physical owner.
    let before = physical_evidence_component(&kura, ResourceFamily::EvidenceKeyRecords);
    assert!(
        kura.store_wsv_checkpoint(1, block.hash(), Hash::new(b"immutable drift"))
            .is_err()
    );
    assert_eq!(
        physical_evidence_component(&kura, ResourceFamily::EvidenceKeyRecords),
        before
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    let temporary = path.with_extension("norito.tmp");
    fs::create_dir(&temporary).unwrap();
    assert!(
        kura.store_wsv_checkpoint(1, block.hash(), state_hash)
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    for family in [
        ResourceFamily::EvidenceKeyRecords,
        ResourceFamily::StorageBytes,
    ] {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err()
        );
    }
    fs::remove_dir(&temporary).unwrap();
    assert!(
        kura.resource_inventory
            .component_usage_for_tests(ResourceFamily::EvidenceKeyRecords)
            .is_err(),
        "removing a bad path does not repair the recorded fault"
    );
    let observed = initialize_actual_physical_evidence_fixture(&kura);
    assert_eq!(
        physical_evidence_component(&kura, ResourceFamily::EvidenceKeyRecords),
        observed[ResourceFamily::EvidenceKeyRecords as usize]
    );
}

#[test]
fn native_resources_account_recovery_latest_publication_and_bounded_pair_pruning() {
    for scenario in ["receipt-temp", "manifest-and-receipt-temp", "prune-pair"] {
        let directory = TempDir::new().unwrap();
        let mut config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
        config.lane_history_retention = NonZeroUsize::new(1).unwrap();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .unwrap();
        let entry = kura.lane_storage_entry(LaneId::SINGLE).unwrap();
        let heights: &[u64] = if scenario == "prune-pair" {
            &[1, 2]
        } else {
            &[1]
        };
        let receipts = install_native_amx_evidence_fixture_heights(&kura, &entry, heights);
        let newest = receipts.last().unwrap();
        let manifest =
            Kura::native_amx_application_manifest_path_for_entry(&entry, &kura.store_root, 1);
        let receipt =
            Kura::native_amx_participant_receipt_path_for_entry(&entry, &kura.store_root, 1);
        let manifest_len = fs::metadata(&manifest).unwrap().len();
        let receipt_len = fs::metadata(&receipt).unwrap().len();
        let mut promoted_bytes = 0;
        if scenario != "prune-pair" {
            fs::rename(&receipt, receipt.with_extension("norito.tmp")).unwrap();
            promoted_bytes += receipt_len;
            if scenario == "manifest-and-receipt-temp" {
                fs::rename(&manifest, manifest.with_extension("norito.tmp")).unwrap();
                promoted_bytes += manifest_len;
            }
        }
        let before = initialize_actual_physical_evidence_fixture(&kura);
        let expected_latest = norito::encode_canonical(
            &NativeAmxParticipantReceiptLatestIndexV2::from_receipt(newest),
        )
        .unwrap();
        assert_eq!(
            kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
                .unwrap(),
            1
        );
        let (latest, latest_temp) = native_amx_latest_index_test_paths(&kura, &entry);
        assert_eq!(fs::read(&latest).unwrap(), expected_latest);
        assert!(!latest_temp.exists());
        let removed_bytes = if scenario == "prune-pair" {
            manifest_len + receipt_len
        } else {
            0
        };
        let removed_records = if scenario == "prune-pair" { 2 } else { 0 };
        let evidence = physical_evidence_component(&kura, ResourceFamily::EvidenceKeyRecords);
        assert_eq!(
            evidence.persisted_entries,
            before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries - removed_records,
            "{scenario}"
        );
        assert_eq!(
            evidence.index_bytes,
            before[ResourceFamily::EvidenceKeyRecords as usize].index_bytes + promoted_bytes
                - removed_bytes,
            "{scenario}"
        );
        assert_eq!(
            evidence.temporary_index_bytes,
            before[ResourceFamily::EvidenceKeyRecords as usize].temporary_index_bytes
                - promoted_bytes,
            "{scenario}"
        );
        let index = physical_evidence_component(&kura, ResourceFamily::NativeLatestRecord);
        assert_eq!(index.persisted_entries, 1);
        assert_eq!(index.index_bytes, expected_latest.len() as u64);
        assert_eq!(index.temporary_index_bytes, 0);
        let storage = physical_evidence_component(&kura, ResourceFamily::StorageBytes);
        assert_eq!(
            storage.storage_bytes,
            before[ResourceFamily::StorageBytes as usize].storage_bytes
                + expected_latest.len() as u64
                - removed_bytes,
            "{scenario}"
        );
        assert_eq!(
            kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
                .unwrap(),
            0
        );
        assert_eq!(
            physical_evidence_component(&kura, ResourceFamily::EvidenceKeyRecords),
            evidence
        );
        assert_eq!(
            physical_evidence_component(&kura, ResourceFamily::StorageBytes),
            storage
        );
        assert_eq!(manifest.exists(), scenario != "prune-pair");
        assert_eq!(receipt.exists(), scenario != "prune-pair");
    }
}

#[test]
fn native_resources_fail_closed_on_authenticated_protocol_failure_without_losing_bytes() {
    let directory = TempDir::new().unwrap();
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let entry = kura.lane_storage_entry(LaneId::SINGLE).unwrap();
    install_native_amx_evidence_fixture_heights(&kura, &entry, &[1]);
    let (latest, temporary) = native_amx_latest_index_test_paths(&kura, &entry);
    let malformed = [0xA5];
    fs::write(&temporary, malformed).unwrap();
    initialize_actual_physical_evidence_fixture(&kura);
    assert!(
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .is_err()
    );
    assert_eq!(fs::read(&temporary).unwrap(), malformed);
    assert!(!latest.exists());
    for family in [
        ResourceFamily::NativeLatestRecord,
        ResourceFamily::EvidenceKeyRecords,
        ResourceFamily::StorageBytes,
    ] {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err(),
            "{family:?}"
        );
    }
}
