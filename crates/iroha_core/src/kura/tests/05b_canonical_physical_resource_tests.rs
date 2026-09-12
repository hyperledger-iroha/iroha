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
        let before = initialize_retained_physical_fixture(&kura);
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
        assert_retained_physical_unavailable(&kura);
        store
            .append_block_batch_at(1, std::slice::from_ref(&next), u64::from(evicted))
            .unwrap();
        store.flush_pending_fsync(true).unwrap();
        assert_eq!(store.read_block_index(1).unwrap().is_evicted(), evicted);
        resources.finish_resources_before_disk_rescan();
        drop(store);
        let after = assert_retained_physical_fixture(&kura);
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
    initialize_retained_physical_fixture(&kura);
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
    assert_retained_physical_unavailable(&kura);
    resources.finish_resources_before_disk_rescan();
    drop(store);
    assert_retained_physical_fixture(&kura);
}

#[test]
fn canonical_physical_recovery_preserves_both_exact_marker_and_carrier_pin_choices() {
    for new_marker in [false, true] {
        for correct_pin in [false, true] {
            let (_directory, kura, blocks) = canonical_physical_fixture(2);
            canonical_physical_seed_da_suffix(&kura, &blocks);
            let replacement = canonical_physical_block_at(&blocks, 2);
            canonical_physical_leave_rewrite(&kura, &replacement, new_marker);
            initialize_retained_physical_fixture(&kura);
            let _write = kura.block_store_write_lock.lock();
            let mut store = kura.block_store.lock();
            let selected = if new_marker {
                replacement.hash()
            } else {
                blocks[1].hash()
            };
            let opposite = if new_marker {
                blocks[1].hash()
            } else {
                replacement.hash()
            };
            let pins = BTreeMap::from([(2, if correct_pin { selected } else { opposite })]);
            let marker_before = store.read_commit_marker().unwrap();
            let stage_before = fs::read(store.da_block_rewrite_stage_path()).unwrap();
            let resources = kura.begin_canonical_physical_mutation(
                &mut store,
                CanonicalPhysicalOperation::Recovery,
            );
            let result = store.recover_canonical_storage_stages_with_carrier_pins(&pins);
            if correct_pin {
                result.unwrap();
                resources.finish_resources_before_disk_rescan();
                assert!(!store.da_block_rewrite_stage_path().exists());
                assert_eq!(store.read_block_hashes(1, 1).unwrap(), vec![selected]);
                drop(store);
                assert_retained_physical_fixture(&kura);
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("pinned canonical replica terminal carrier")
                );
                drop(resources);
                assert_eq!(store.read_commit_marker().unwrap(), marker_before);
                assert_eq!(
                    fs::read(store.da_block_rewrite_stage_path()).unwrap(),
                    stage_before
                );
                assert_retained_physical_unavailable(&kura);
            }
        }
    }
}

#[test]
fn canonical_physical_invalid_plan_changed_stage_and_unfinished_leaf_stay_unavailable() {
    for case in 0..4 {
        let (_directory, kura, blocks) = canonical_physical_fixture(2);
        let replacement = canonical_physical_block_at(&blocks, 2);
        canonical_physical_leave_rewrite(&kura, &replacement, false);
        initialize_retained_physical_fixture(&kura);
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
        assert_retained_physical_unavailable(&kura);
    }
}

#[test]
fn canonical_physical_committed_recovery_fault_never_publishes_complete_resources() {
    let (_directory, kura, blocks) = canonical_physical_fixture(1);
    let replacement = canonical_physical_block_at(&blocks, 1);
    initialize_retained_physical_fixture(&kura);
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
    assert_retained_physical_unavailable(&kura);
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
        initialize_retained_physical_fixture(&kura);
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
            assert_retained_physical_unavailable(&kura);
            store.prune(2).unwrap();
            drop(store);
            // Reconciliation is explicit and complete, after the real idempotent
            // recovery finished; a later incremental delta cannot clear a fault.
            initialize_retained_physical_fixture(&kura);
            store = kura.block_store.lock();
        }
        assert_eq!(store.read_durable_index_count().unwrap(), 2);
        assert!(store.da_block_path(2).is_file());
        assert!(!store.da_block_path(3).exists());
        assert!(!store.da_block_path(4).exists());
        drop(store);
        assert_retained_physical_fixture(&kura);
    }
}

#[test]
fn canonical_physical_cache_authenticates_signed_wire_and_counts_exact_retry() {
    let (_directory, kura, blocks) = canonical_physical_fixture(4);
    let (_, length) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.evict_block_bodies(length).unwrap();
    kura.block_store.lock().remove_da_block_file(2).unwrap();
    let before = initialize_retained_physical_fixture(&kura);
    let wrong = canonical_physical_block_at(&blocks, 2);
    assert!(kura.cache_block_body(&wrong).is_err());
    assert_eq!(assert_retained_physical_fixture(&kura), before);
    kura.cache_block_body(&blocks[1]).unwrap();
    let after = assert_retained_physical_fixture(&kura);
    assert_eq!(
        after[ResourceFamily::StorageBytes as usize].storage_bytes
            - before[ResourceFamily::StorageBytes as usize].storage_bytes,
        length
    );
    kura.cache_block_body(&blocks[1]).unwrap();
    assert_eq!(assert_retained_physical_fixture(&kura), after);
    assert_eq!(
        kura.get_block(nonzero!(2_usize)).unwrap().hash(),
        blocks[1].hash()
    );
}

#[test]
fn canonical_physical_eviction_zero_after_authority_expiry_keeps_published_copy_bytes() {
    let (_directory, kura, _blocks) = canonical_physical_fixture(4);
    let (_, length) = advertise_required_replicas(&kura, nonzero!(2_usize));
    let before = initialize_retained_physical_fixture(&kura);
    kura.pause_next_eviction_before_stage_publication_for_tests();
    let worker_kura = Arc::clone(&kura);
    let worker = thread::spawn(move || worker_kura.evict_block_bodies(length));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.eviction_paused_before_stage_publication_for_tests() {
        assert!(!worker.is_finished());
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert_retained_physical_unavailable(&kura);
    let expired = Instant::now()
        .checked_sub(kura.replica_advert_ttl() + Duration::from_secs(1))
        .unwrap();
    kura.replica_registry.lock().retain(|_, adverts| {
        for advert in adverts.values_mut() {
            advert.observed_at = expired;
        }
        true
    });
    kura.resume_eviction_before_stage_publication_for_tests();
    assert_eq!(worker.join().unwrap().unwrap(), 0);
    let store = kura.block_store.lock();
    assert!(store.da_block_path(2).is_file());
    assert!(!store.eviction_compaction_stage_path().exists());
    drop(store);
    assert!(kura.retained_block_record_path(2).is_file());
    let after = assert_retained_physical_fixture(&kura);
    assert!(
        after[ResourceFamily::StorageBytes as usize].storage_bytes
            > before[ResourceFamily::StorageBytes as usize].storage_bytes
    );
    assert!(!kura.disk_usage_total_initialized.load(Ordering::Acquire));
}

#[test]
fn canonical_physical_periodic_forced_and_empty_eviction_flush_publish_pending_marker() {
    for mode in 0..3 {
        let (_directory, kura, blocks) = canonical_physical_fixture(1);
        let next = canonical_physical_block_at(&blocks, 2);
        {
            let mut store = kura.block_store.lock();
            store.fsync.mode = FsyncMode::Batched;
            store.fsync.interval = Duration::from_secs(3600);
            store.append_block_to_chain(&next).unwrap();
            assert_eq!(store.read_durable_index_count().unwrap(), 1);
            if mode == 0 {
                store.fsync.pending_since = Some(
                    Instant::now()
                        .checked_sub(Duration::from_secs(3601))
                        .unwrap(),
                );
            }
        }
        // The direct append retains the real pending-marker boundary. Mirror
        // that exact signed block into the accepted fixture image, as the existing
        // batched-fsync eviction fixture does; the normal Kura call forces fsync.
        kura.block_data
            .lock()
            .push((next.hash(), Some(Arc::clone(&next))));
        assert_eq!(kura.block_data.lock().len(), 2);
        initialize_retained_physical_fixture(&kura);
        if mode == 2 {
            // Genesis and the retained newest body leave no eviction candidates;
            // the initial flush must still publish the real pending marker.
            assert_eq!(kura.evict_block_bodies(1).unwrap(), 0);
        } else {
            let _write = kura.block_store_write_lock.lock();
            kura.flush_pending_fsync_with_resources(&mut kura.block_store.lock(), mode == 1)
                .unwrap();
        }
        assert_eq!(
            kura.block_store.lock().read_durable_index_count().unwrap(),
            2
        );
        assert_retained_physical_fixture(&kura);
        assert!(!kura.disk_usage_total_initialized.load(Ordering::Acquire));
    }
}
