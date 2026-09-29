// Remaining live writer boundaries: exact files, rollback images and independent recounts.

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
    let block = NativeBlocks::new().next();
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
