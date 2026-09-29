// Actual canonical journals, signed body custody and complete physical recounts.

fn canonical_physical_fixture(count: usize) -> (TempDir, Arc<Kura>, Vec<Arc<SignedBlock>>) {
    let directory = TempDir::new().unwrap();
    let config = kura_config_for_dir(&directory, nonzero!(1_usize));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let blocks = store_dummy_block_arcs(&kura, count);
    (directory, kura, blocks)
}

fn canonical_physical_block_at(blocks: &[Arc<SignedBlock>], height: usize) -> Arc<SignedBlock> {
    Arc::new(
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(NonZeroU64::new(height as u64).unwrap());
            header.set_prev_block_hash(height.checked_sub(2).map(|index| blocks[index].hash()));
            header.set_view_change_index(7);
        })
        .into(),
    )
}

fn canonical_physical_seed_da_suffix(kura: &Kura, blocks: &[Arc<SignedBlock>]) {
    let _write = kura.block_store_write_lock.lock();
    let mut store = kura.block_store.lock();
    // First publish the real durable prefix. Rewriting an already-published
    // count/hash into different inline/evicted bytes has identical recovery
    // markers and is correctly forbidden. Both real writer operations finish
    // before the audit baseline, restoring the original logical chain.
    assert!(blocks.len() > 1);
    store.prune(1).unwrap();
    assert_eq!(store.read_durable_index_count().unwrap(), 1);
    store.append_block_batch_at(1, &blocks[1..], 1).unwrap();
    assert!(store.read_block_index(1).unwrap().is_evicted());
    assert_eq!(
        store.read_durable_index_count().unwrap(),
        blocks.len() as u64
    );
    for (index, block) in blocks.iter().enumerate().skip(1) {
        let entry = store.read_block_index(index as u64).unwrap();
        assert!(entry.is_evicted());
        let bytes = store
            .read_da_block_bytes((index + 1) as u64, entry.length)
            .unwrap();
        assert_eq!(
            decode_versioned_signed_block(&bytes).unwrap().hash(),
            block.hash()
        );
    }
    assert!(store.read_da_block_rewrite_stage().unwrap().is_none());
}

fn canonical_physical_leave_rewrite(kura: &Kura, replacement: &Arc<SignedBlock>, new_marker: bool) {
    let _write = kura.block_store_write_lock.lock();
    let mut store = kura.block_store.lock();
    if new_marker {
        store
            .crash_next_da_rewrite_after_marker
            .store(true, Ordering::Release);
    } else {
        store
            .crash_next_da_rewrite_before_marker
            .store(true, Ordering::Release);
    }
    assert!(
        store
            .append_block_batch_at(1, std::slice::from_ref(replacement), 0)
            .is_err()
    );
    assert!(store.read_da_block_rewrite_stage().unwrap().is_some());
}

#[test]
fn canonical_physical_append_uses_only_fixed_and_actual_written_height() {
    for evicted in [false, true] {
        let (_directory, kura, blocks) = canonical_physical_fixture(1);
        let next = canonical_physical_block_at(&blocks, 2);
        let before = initialize_physical_fixture(&kura);
        let cached_total = kura.kura_total_disk_usage_bytes().unwrap();
        kura.resolve_canonical_storage_before_mutation().unwrap();
        assert!(kura.disk_usage_total_initialized.load(Ordering::Acquire));
        assert_eq!(kura.disk_usage_total.load(Ordering::Acquire), cached_total);
        let _write = kura.block_store_write_lock.lock();
        let mut store = kura.block_store.lock();
        let operation = CanonicalPhysicalOperation::Append {
            start_height: 1,
            block_count: 1,
        };
        let (paths, _) = Kura::canonical_physical_paths(&mut store, operation).unwrap();
        assert_eq!(paths.len(), 12);
        assert_eq!(
            paths
                .iter()
                .filter(|path| path.starts_with(&store.da_blocks_dir))
                .count(),
            1
        );
        assert!(paths.contains(&store.da_block_path(2)));
        let resources = kura.begin_canonical_physical_mutation(&mut store, operation);
        assert!(resources.complete);
        assert_eq!(resources.leaves.len(), 1);
        assert_physical_unavailable(&kura);
        store
            .append_block_batch_at(1, std::slice::from_ref(&next), u64::from(evicted))
            .unwrap();
        store.flush_pending_fsync(true).unwrap();
        assert_eq!(store.read_block_index(1).unwrap().is_evicted(), evicted);
        resources.finish_resources_before_disk_rescan();
        drop(store);
        let after = assert_physical_fixture(&kura);
        assert!(
            after[ResourceFamily::StorageBytes as usize].storage_bytes
                > before[ResourceFamily::StorageBytes as usize].storage_bytes
        );
        assert!(!kura.disk_usage_total_initialized.load(Ordering::Acquire));
    }
}

#[test]
fn canonical_physical_rewrite_preobserves_multiple_chunks_and_keeps_fence_busy() {
    let (_directory, kura, blocks) = canonical_physical_fixture(50);
    canonical_physical_seed_da_suffix(&kura, &blocks);
    let replacement = canonical_physical_block_at(&blocks, 2);
    canonical_physical_leave_rewrite(&kura, &replacement, false);
    initialize_physical_fixture(&kura);
    let _write = kura.block_store_write_lock.lock();
    let mut store = kura.block_store.lock();
    let operation = CanonicalPhysicalOperation::Append {
        start_height: 1,
        block_count: 1,
    };
    let (paths, _) = Kura::canonical_physical_paths(&mut store, operation).unwrap();
    assert_eq!(
        paths.len(),
        60,
        "fixed11 plus the49 actual image heights, globally deduplicated"
    );
    let mut resources = kura.begin_canonical_physical_mutation(&mut store, operation);
    assert_eq!(resources.leaves.len(), 2);
    assert!(resources.complete);
    store
        .append_block_batch_at(1, std::slice::from_ref(&replacement), 0)
        .unwrap();
    store.flush_pending_fsync(true).unwrap();
    assert_eq!(store.read_durable_index_count().unwrap(), 2);
    assert!(
        !store.da_block_path(2).exists(),
        "new inline image consumes the old evicted body"
    );
    resources.leaves.pop().unwrap().finish().unwrap();
    assert_physical_unavailable(&kura);
    resources.finish_resources_before_disk_rescan();
    drop(store);
    assert_physical_fixture(&kura);
}

#[test]
fn canonical_physical_recovery_preserves_both_exact_marker_choices() {
    for new_marker in [false, true] {
        let (_directory, kura, blocks) = canonical_physical_fixture(2);
        canonical_physical_seed_da_suffix(&kura, &blocks);
        let replacement = canonical_physical_block_at(&blocks, 2);
        canonical_physical_leave_rewrite(&kura, &replacement, new_marker);
        initialize_physical_fixture(&kura);
        let _write = kura.block_store_write_lock.lock();
        let mut store = kura.block_store.lock();
        let selected = if new_marker { replacement.hash() } else { blocks[1].hash() };
        let resources = kura.begin_canonical_physical_mutation(
            &mut store, CanonicalPhysicalOperation::Recovery,
        );
        store.recover_canonical_storage_stages().unwrap();
        resources.finish_resources_before_disk_rescan();
        assert!(!store.da_block_rewrite_stage_path().exists());
        assert_eq!(store.read_block_hashes(1, 1).unwrap(), vec![selected]);
        drop(store);
        assert_physical_fixture(&kura);
    }
}

#[test]
fn canonical_physical_invalid_plan_changed_stage_and_unfinished_leaf_stay_unavailable() {
    for case in 0..4 {
        let (_directory, kura, blocks) = canonical_physical_fixture(2);
        let replacement = canonical_physical_block_at(&blocks, 2);
        canonical_physical_leave_rewrite(&kura, &replacement, false);
        initialize_physical_fixture(&kura);
        let _write = kura.block_store_write_lock.lock();
        let mut store = kura.block_store.lock();
        match case {
            0 => {
                let (identity, _) = CanonicalPhysicalStageIdentity::capture(&store).unwrap();
                let path = store.da_block_rewrite_stage_path();
                let old = fs::read(&path).unwrap();
                fs::remove_file(&path).unwrap();
                fs::write(&path, &old).unwrap();
                assert!(
                    !identity.unchanged(),
                    "equal stage bytes in a different inode cannot discharge the retained identity"
                );
                let mut bytes = old;
                bytes[0] ^= 1;
                fs::write(&path, bytes).unwrap();
                let resources = kura.begin_canonical_physical_mutation(
                    &mut store,
                    CanonicalPhysicalOperation::Recovery,
                );
                assert!(!resources.complete);
                resources.finish();
            }
            1 | 2 => {
                let operation = CanonicalPhysicalOperation::Append {
                    start_height: if case == 1 { u64::MAX } else { 1 },
                    block_count: if case == 1 {
                        1
                    } else {
                        MAX_DA_BLOCK_REWRITE_STAGE_ENTRIES + 1
                    },
                };
                assert!(Kura::canonical_physical_paths(&mut store, operation).is_err());
                let resources = kura.begin_canonical_physical_mutation(&mut store, operation);
                assert!(!resources.complete);
                resources.finish();
            }
            _ => {
                let resources = kura.begin_canonical_physical_mutation(
                    &mut store,
                    CanonicalPhysicalOperation::Recovery,
                );
                store.recover_canonical_storage_stages().unwrap();
                drop(resources);
            }
        }
        assert_physical_unavailable(&kura);
    }
}

#[test]
fn canonical_physical_committed_recovery_fault_never_publishes_complete_resources() {
    let (_directory, kura, blocks) = canonical_physical_fixture(1);
    let replacement = canonical_physical_block_at(&blocks, 1);
    initialize_physical_fixture(&kura);
    {
        let store = kura.block_store.lock();
        store
            .fail_next_da_rewrite_after_marker
            .store(true, Ordering::Release);
        store
            .fail_next_da_rewrite_recovery
            .store(true, Ordering::Release);
    }
    let error = kura.persist_block_at_height(&replacement, 1).unwrap_err();
    assert!(matches!(
        error,
        Error::CanonicalBlockCommittedRecoveryRequired { .. }
    ));
    assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert_eq!(
        Kura::read_durable_hash_at_height(&mut kura.block_store.lock(), 1).unwrap(),
        Some(replacement.hash())
    );
    assert_eq!(
        kura.block_data.lock().first().map(|(hash, _)| *hash),
        Some(blocks[0].hash())
    );
    assert_physical_unavailable(&kura);
}

#[test]
fn canonical_physical_prune_counts_actual_da_suffix_and_failed_prefix_recovers() {
    for fail_stage in [
        0,
        PRUNE_STAGE_BLOCK_MARKER,
        PRUNE_STAGE_BLOCK_INDEX,
        PRUNE_STAGE_BLOCK_HASHES,
        PRUNE_STAGE_BLOCK_DATA,
        PRUNE_STAGE_DA_SIDECARS,
    ] {
        let (_directory, kura, blocks) = canonical_physical_fixture(4);
        canonical_physical_seed_da_suffix(&kura, &blocks);
        initialize_physical_fixture(&kura);
        let _write = kura.block_store_write_lock.lock();
        let mut store = kura.block_store.lock();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let resources = kura
                .begin_canonical_physical_mutation(&mut store, CanonicalPhysicalOperation::Prune);
            store.prune_with_failpoint(2, fail_stage).unwrap();
            resources.finish_resources_before_disk_rescan();
        }));
        assert_eq!(result.is_ok(), fail_stage == 0);
        if fail_stage != 0 {
            assert_physical_unavailable(&kura);
            store.prune(2).unwrap();
            drop(store);
            // Reconciliation is explicit and complete, after the real idempotent
            // recovery finished; a later incremental delta cannot clear a fault.
            initialize_physical_fixture(&kura);
            store = kura.block_store.lock();
        }
        assert_eq!(store.read_durable_index_count().unwrap(), 2);
        assert!(store.da_block_path(2).is_file());
        assert!(!store.da_block_path(3).exists());
        assert!(!store.da_block_path(4).exists());
        drop(store);
        assert_physical_fixture(&kura);
    }
}

#[test]
fn immutable_instance_disk_scan_counts_current_and_retained_files_once() {
    let directory = TempDir::new().unwrap();
    let root = directory.path();
    let blocks = root.join("blocks");
    let first = LaneStorageIdentity {
        network_id: test_network_id(b"immutable-accounting"),
        lane_id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(2),
        incarnation: Hash::new(b"first-accounting-incarnation"),
        activation_height: 3,
    };
    let second = LaneStorageIdentity {
        incarnation: Hash::new(b"second-accounting-incarnation"),
        activation_height: 4,
        ..first
    };
    let canonical = Kura::canonical_storage_path(root);
    let paths = [
        (canonical.join(DATA_FILE_NAME), 11),
        (first.blocks_dir(root).join(".lane-incarnation.norito"), 17),
        (
            first
                .blocks_dir(root)
                .join(PIPELINE_DIR_NAME)
                .join("accounting.norito.tmp"),
            19,
        ),
        (
            first
                .blocks_dir(root)
                .join(DA_BLOCKS_DIR_NAME)
                .join("accounting.norito"),
            23,
        ),
        (second.blocks_dir(root).join(".lane-incarnation.norito"), 29),
    ];
    for (path, length) in &paths {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0_u8; *length]).unwrap();
    }
    // Accounting must include both identities without an active LaneId map;
    // bytes stay charged until their actual collection, including temporaries.
    assert_eq!(Kura::blocks_root_usage_bytes(&blocks).unwrap(), (76, 99));
    assert_eq!(Kura::blocks_root_bytes(&blocks).unwrap(), 76);
    fs::remove_file(&paths[2].0).unwrap();
    assert_eq!(Kura::blocks_root_usage_bytes(&blocks).unwrap(), (57, 80));
}

#[test]
fn immutable_instance_disk_scan_rejects_unknown_nested_entries() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("blocks");
    let unknown = root.join("instances").join("unowned");
    fs::create_dir_all(&unknown).unwrap();
    assert!(Kura::blocks_root_usage_bytes(&root).is_err());
}

#[cfg(unix)]
#[test]
fn immutable_instance_disk_scan_rejects_symlinks_and_hardlinks() {
    use std::os::unix::fs::symlink;
    for hardlink in [false, true] {
        let directory = TempDir::new().unwrap();
        let root = directory.path().join("blocks");
        let instance = root.join("instances").join("a".repeat(64));
        fs::create_dir_all(instance.parent().unwrap()).unwrap();
        let outside = directory.path().join("outside");
        if hardlink {
            fs::write(&outside, [0_u8; 7]).unwrap();
            fs::create_dir(&instance).unwrap();
            fs::hard_link(&outside, instance.join(".lane-incarnation.norito")).unwrap();
        } else {
            fs::create_dir(&outside).unwrap();
            symlink(&outside, instance).unwrap();
        }
        assert!(Kura::blocks_root_usage_bytes(&root).is_err());
        assert_eq!(fs::symlink_metadata(outside).unwrap().is_dir(), !hardlink);
    }
}
