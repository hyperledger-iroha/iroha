// Real retained-record and indexed-rewrite owners; resource observations grant no authority.

fn retained_physical_fixture(count: usize) -> (TempDir, Arc<Kura>, Vec<Arc<SignedBlock>>) {
    let directory = TempDir::new().expect("create retained resource fixture");
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("open authenticated retained resource fixture");
    let blocks = store_dummy_block_arcs(&kura, count);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    for block in &blocks {
        kura.persist_retained_block_record(&blocks_dir, block.hash(), block.as_ref())
            .expect("persist exact canonical retained record");
    }
    (directory, kura, blocks)
}

fn initialize_retained_physical_fixture(kura: &Kura) -> IndexResourceCounts {
    let generation = kura.resource_inventory.reconciliation_generation().unwrap();
    let counts = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .expect("independently observe every physical fixture owner");
    kura.resource_inventory
        .initialize(
            generation,
            &PHYSICAL_RESOURCE_FAMILIES
                .iter()
                .map(|family| (*family, counts[*family as usize]))
                .collect::<Vec<_>>(),
        )
        .expect("register only observed physical families after fixture preparation");
    counts
}

fn assert_retained_physical_fixture(kura: &Kura) -> IndexResourceCounts {
    let observed = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .unwrap();
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert_eq!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .unwrap(),
            observed[family as usize],
            "independent real-file recount for {family:?}",
        );
    }
    observed
}

fn assert_retained_physical_unavailable(kura: &Kura) {
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err()
        );
    }
}

fn retained_physical_replacement(blocks: &[Arc<SignedBlock>]) -> Arc<SignedBlock> {
    let height = blocks.len();
    Arc::new(
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(NonZeroU64::new(height as u64).unwrap());
            header.set_prev_block_hash(height.checked_sub(2).map(|index| blocks[index].hash()));
            header.set_view_change_index(
                blocks[height - 1]
                    .header()
                    .view_change_index()
                    .saturating_add(1),
            );
        })
        .into(),
    )
}

#[test]
fn retained_physical_staging_and_error_restore_preserve_exact_resources() {
    let (_directory, kura, _blocks) = retained_physical_fixture(3);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let unchanged = fs::read(kura.retained_block_record_path(1)).unwrap();
    let before = initialize_retained_physical_fixture(&kura);
    let _canonical_guard = kura.canonical_chain_lock.lock();
    let stage = kura
        .stage_retained_block_records_for_rewrite(&blocks_dir, 2)
        .unwrap()
        .unwrap();
    assert_eq!(stage.entries.len(), 2);
    assert!(!kura.retained_block_record_path(2).exists());
    assert_eq!(assert_retained_physical_fixture(&kura), before);
    kura.reconcile_staged_retained_block_rewrite_after_error(&stage)
        .unwrap();
    assert_eq!(assert_retained_physical_fixture(&kura), before);
    assert_eq!(
        fs::read(kura.retained_block_record_path(1)).unwrap(),
        unchanged
    );
    assert!(kura.retained_block_record_path(2).is_file());
    assert!(!Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).exists());
}

#[test]
fn retained_physical_suffix_prune_is_exact_and_retry_is_idempotent() {
    let (_directory, kura, _blocks) = retained_physical_fixture(3);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let removed = [2, 3]
        .iter()
        .map(|height| {
            fs::metadata(kura.retained_block_record_path(*height))
                .unwrap()
                .len()
        })
        .sum::<u64>();
    let before = initialize_retained_physical_fixture(&kura);
    let _canonical_guard = kura.canonical_chain_lock.lock();
    kura.prune_retained_block_records_from(&blocks_dir, 2)
        .unwrap();
    let after = assert_retained_physical_fixture(&kura);
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        removed,
    );
    assert_eq!(
        before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries
            - after[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
        2,
    );
    assert!(kura.retained_block_record_path(1).is_file());
    assert!(!kura.retained_block_record_path(2).exists());
    kura.prune_retained_block_records_from(&blocks_dir, 2)
        .unwrap();
    assert_eq!(assert_retained_physical_fixture(&kura), after);
}

#[test]
fn retained_physical_discard_subtracts_only_after_complete_tree_removal() {
    for interrupt in [false, true] {
        let (_directory, kura, blocks) = retained_physical_fixture(1);
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        let _canonical_guard = kura.canonical_chain_lock.lock();
        let stage = kura
            .stage_retained_block_records_for_rewrite(&blocks_dir, 1)
            .unwrap()
            .unwrap();
        let replacement = retained_physical_replacement(&blocks);
        kura.persist_block_at_height(&replacement, 1).unwrap();
        assert_eq!(
            kura.get_durable_block_hash(nonzero!(1_usize)),
            Some(replacement.hash())
        );
        let before = initialize_retained_physical_fixture(&kura);
        if interrupt {
            kura.fail_retained_rewrite_discard_after_for_tests(0);
            assert!(kura.discard_staged_retained_block_rewrite(&stage).is_err());
            assert_retained_physical_unavailable(&kura);
            assert!(Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).is_dir());
        } else {
            kura.discard_staged_retained_block_rewrite(&stage).unwrap();
            let after = assert_retained_physical_fixture(&kura);
            assert_eq!(
                before[ResourceFamily::StorageBytes as usize].storage_bytes
                    - after[ResourceFamily::StorageBytes as usize].storage_bytes,
                stage.removed_total_bytes,
            );
            assert_eq!(
                before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries
                    - after[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
                1,
            );
            assert!(!Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).exists());
        }
    }
}

#[test]
fn retained_physical_startup_recovery_counts_restore_duplicate_and_obsolete_records() {
    let (_directory, kura, blocks) = retained_physical_fixture(3);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let original_two = fs::read(kura.retained_block_record_path(2)).unwrap();
    let _canonical_guard = kura.canonical_chain_lock.lock();
    let stage = kura
        .stage_retained_block_records_for_rewrite(&blocks_dir, 1)
        .unwrap()
        .unwrap();
    // Publish only the real last-block replacement. Earlier exact retained records
    // still match their canonical headers; the old last record is now obsolete.
    let replacement = retained_physical_replacement(&blocks);
    kura.persist_block_at_height(&replacement, 3).unwrap();
    kura.persist_retained_block_record(&blocks_dir, blocks[0].hash(), blocks[0].as_ref())
        .unwrap();
    let removed = stage.entries[0].bytes_len + stage.entries[2].bytes_len;
    let before = initialize_retained_physical_fixture(&kura);
    kura.recover_retained_block_rewrite_stage_on_startup(&blocks_dir)
        .unwrap();
    let after = assert_retained_physical_fixture(&kura);
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        removed,
    );
    assert_eq!(
        before[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries
            - after[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
        2,
    );
    assert_eq!(
        fs::read(kura.retained_block_record_path(2)).unwrap(),
        original_two
    );
    assert!(kura.retained_block_record_path(1).is_file());
    assert!(!kura.retained_block_record_path(3).exists());
    assert!(!Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).exists());
    kura.recover_retained_block_rewrite_stage_on_startup(&blocks_dir)
        .unwrap();
    assert_eq!(assert_retained_physical_fixture(&kura), after);
}

#[test]
fn retained_physical_empty_stage_keeps_bytes_exact_and_requests_disk_rescan() {
    let (_directory, kura, _blocks) = retained_physical_fixture(1);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let stage = Kura::retained_block_rewrite_staging_dir_for(&blocks_dir);
    fs::create_dir(&stage).unwrap();
    let before = initialize_retained_physical_fixture(&kura);
    kura.refresh_total_disk_usage_bytes().unwrap();
    assert!(kura.disk_usage_total_initialized.load(Ordering::Acquire));
    let _canonical_guard = kura.canonical_chain_lock.lock();
    kura.recover_retained_block_rewrite_stage_on_startup(&blocks_dir)
        .unwrap();
    assert_eq!(assert_retained_physical_fixture(&kura), before);
    assert!(!stage.exists());
    assert!(!kura.disk_usage_total_initialized.load(Ordering::Acquire));
}

#[test]
fn retained_physical_partial_recovery_failure_never_publishes_a_complete_inventory() {
    let (_directory, kura, _blocks) = retained_physical_fixture(2);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let _canonical_guard = kura.canonical_chain_lock.lock();
    let _stage = kura
        .stage_retained_block_records_for_rewrite(&blocks_dir, 1)
        .unwrap()
        .unwrap();
    let corrupt = Kura::retained_block_rewrite_staging_path_for(&blocks_dir, 2);
    fs::write(&corrupt, [1_u8; 8]).unwrap();
    initialize_retained_physical_fixture(&kura);
    assert!(
        kura.recover_retained_block_rewrite_stage_on_startup(&blocks_dir)
            .is_err()
    );
    assert!(kura.retained_block_record_path(1).is_file());
    assert_eq!(fs::read(&corrupt).unwrap(), [1_u8; 8]);
    assert_retained_physical_unavailable(&kura);
    kura.begin_total_disk_usage_mutation()
        .with_resource_children(0)
        .finish();
    assert_retained_physical_unavailable(&kura);
}

#[test]
fn lane_compaction_physical_recovery_counts_both_pair_files_and_temporary_bytes() {
    let fixture = merge_receipt_compaction_fixture();
    let (data, index) = ensure_merge_receipt_lane_artifact_pair(&fixture);
    let data_tmp = data.with_extension("norito.tmp");
    let index_tmp = index.with_extension("index.tmp");
    fs::copy(&data, &data_tmp).unwrap();
    fs::copy(&index, &index_tmp).unwrap();
    let removed = fs::metadata(&data_tmp).unwrap().len() + fs::metadata(&index_tmp).unwrap().len();
    let index_bytes = fs::metadata(&index_tmp).unwrap().len();
    let before = initialize_retained_physical_fixture(&fixture.kura);
    assert_eq!(
        before[ResourceFamily::OwnershipIndex as usize].temporary_index_bytes,
        index_bytes,
    );
    assert_eq!(
        compact_fixture_lane_histories(&fixture.kura, &fixture.lane_entry, &fixture.frontier)
            .unwrap(),
        LaneHistoryCompactionOutcome::Complete,
    );
    let after = assert_retained_physical_fixture(&fixture.kura);
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        removed,
    );
    assert_eq!(
        after[ResourceFamily::OwnershipIndex as usize].temporary_index_bytes,
        0
    );
    assert!(!data_tmp.exists());
    assert!(!index_tmp.exists());
}

#[test]
fn lane_compaction_physical_invalid_recovery_cannot_publish_partial_resources() {
    let fixture = merge_receipt_compaction_fixture();
    let (data, index) = ensure_merge_receipt_lane_artifact_pair(&fixture);
    let data_tmp = data.with_extension("norito.tmp");
    fs::copy(&data, &data_tmp).unwrap();
    // An orphan data temp is a rejected recovery shape, even though its length
    // is a perfectly valid physical byte observation.
    initialize_retained_physical_fixture(&fixture.kura);
    let before = fs::read(&data).unwrap();
    assert!(
        compact_fixture_lane_histories(&fixture.kura, &fixture.lane_entry, &fixture.frontier)
            .is_err()
    );
    assert_eq!(fs::read(&data).unwrap(), before);
    assert!(data_tmp.is_file());
    assert!(!index.with_extension("index.tmp").exists());
    assert_retained_physical_unavailable(&fixture.kura);
}
