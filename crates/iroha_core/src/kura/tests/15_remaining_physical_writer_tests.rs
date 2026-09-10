// Remaining live writer boundaries: exact files, rollback images and independent recounts.

#[test]
fn physical_merge_append_and_truncate_preserve_exact_disk_bytes_and_idempotence() {
    let (_directory, kura) = metadata_physical_fixture();
    let before = metadata_physical_register(&kura);
    let entry = sample_merge_entry(1);
    kura.append_merge_entry_for_test(&entry).unwrap();
    let written = metadata_physical_assert_actual(&kura);
    let path = kura.active_merge_path.lock().clone();
    let bytes = fs::metadata(&path).unwrap().len();
    assert!(bytes > 0);
    assert_eq!(
        written[ResourceFamily::StorageBytes as usize].storage_bytes
            - before[ResourceFamily::StorageBytes as usize].storage_bytes,
        bytes
    );
    kura.append_merge_entry_for_test(&entry).unwrap();
    assert_eq!(metadata_physical_assert_actual(&kura), written);
    kura.truncate_merge_log_to_len(0).unwrap();
    assert_eq!(metadata_physical_assert_actual(&kura), before);
    assert_eq!(fs::metadata(&path).unwrap().len(), 0);
    kura.truncate_merge_log_to_len(0).unwrap();
    assert_eq!(metadata_physical_assert_actual(&kura), before);
}

#[test]
fn physical_committed_merge_repair_counts_log_and_carrier_independently() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    kura.store_block(parent).unwrap();
    metadata_physical_register(&kura);
    kura.store_block_with_merge_entry(Arc::clone(&block), &entry)
        .unwrap();
    let complete = metadata_physical_assert_actual(&kura);
    kura.truncate_merge_log_to_len(0).unwrap();
    let missing_log = metadata_physical_assert_actual(&kura);
    assert!(
        missing_log[ResourceFamily::StorageBytes as usize].storage_bytes
            < complete[ResourceFamily::StorageBytes as usize].storage_bytes
    );
    kura.store_block_with_merge_entry(Arc::clone(&block), &entry)
        .unwrap();
    assert_eq!(metadata_physical_assert_actual(&kura), complete);
    kura.store_block_with_merge_entry(block, &entry).unwrap();
    assert_eq!(metadata_physical_assert_actual(&kura), complete);
    assert_eq!(kura.merge_ledger_snapshot(), vec![entry]);
}

#[test]
fn physical_lane_artifact_checkpoint_restore_counts_rebased_rollback_and_failure() {
    for obstruct in [false, true] {
        let (_directory, kura) = metadata_physical_fixture();
        let owner = Kura::lane_artifact_dir(&kura.active_blocks_dir.lock());
        fs::create_dir_all(&owner).unwrap();
        let data = owner.join(LANE_ARTIFACTS_DATA_FILE);
        let index = owner.join(LANE_ARTIFACTS_INDEX_FILE);
        let rollback = index.with_extension("index.rollback.tmp");
        let _prune = kura.prune_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        assert!(Kura::append_indexed_sidecar(
            &data,
            &index,
            10,
            b"retained",
            "physical checkpoint fixture",
            FsyncMode::Always,
            None,
        ));
        let checkpoint = kura.capture_lane_block_artifact_checkpoint_locked(&data, &index, 10);
        let original_data = fs::read(&data).unwrap();
        let original_index = fs::read(&index).unwrap();
        let before = metadata_physical_register(&kura);
        let append = kura
            .begin_total_disk_usage_mutation()
            .with_resource_paths(Kura::sidecar_physical_resource_paths(&data, &index));
        assert!(Kura::append_indexed_sidecar(
            &data,
            &index,
            8,
            b"earlier",
            "physical checkpoint fixture",
            FsyncMode::Always,
            None,
        ));
        append.finish();
        let grown = metadata_physical_assert_actual(&kura);
        assert!(
            grown[ResourceFamily::StorageBytes as usize].storage_bytes
                > before[ResourceFamily::StorageBytes as usize].storage_bytes
        );
        if obstruct {
            fs::create_dir(&rollback).unwrap();
            metadata_physical_assert_actual(&kura);
            assert!(
                kura.restore_lane_block_artifact_checkpoint_locked(&checkpoint)
                    .is_err()
            );
            physical_guard_assert_unavailable(&kura);
            assert!(rollback.is_dir());
        } else {
            kura.restore_lane_block_artifact_checkpoint_locked(&checkpoint)
                .unwrap();
            assert_eq!(metadata_physical_assert_actual(&kura), before);
            assert_eq!(fs::read(&data).unwrap(), original_data);
            assert_eq!(fs::read(&index).unwrap(), original_index);
            assert!(!rollback.exists());
            kura.restore_lane_block_artifact_checkpoint_locked(&checkpoint)
                .unwrap();
            assert_eq!(metadata_physical_assert_actual(&kura), before);
        }
    }
}

#[test]
fn physical_debug_dump_counts_each_actual_append_including_identical_lines() {
    let directory = TempDir::new().unwrap();
    let mut config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    config.debug_output_new_blocks = true;
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let path = kura.block_plain_text_path.lock().clone().unwrap();
    let before = metadata_physical_register(&kura);
    let block = DummyBlocks::new().next();
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    kura.append_debug_block_dump(&block);
    let once = metadata_physical_assert_actual(&kura);
    let line = fs::read(&path).unwrap();
    assert!(!line.is_empty());
    assert_eq!(
        once[ResourceFamily::StorageBytes as usize].storage_bytes
            - before[ResourceFamily::StorageBytes as usize].storage_bytes,
        line.len() as u64
    );
    kura.append_debug_block_dump(&block);
    let twice = metadata_physical_assert_actual(&kura);
    assert_eq!(
        twice[ResourceFamily::StorageBytes as usize].storage_bytes
            - once[ResourceFamily::StorageBytes as usize].storage_bytes,
        line.len() as u64
    );
    assert_eq!(fs::read(&path).unwrap(), [line.clone(), line].concat());
}
