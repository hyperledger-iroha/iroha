fn canonical_storage_budget_base_for_test(kura: &Kura) -> u64 {
    let used = kura
        .refresh_disk_usage_bytes()
        .expect("measure canonical storage-budget physical bytes");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure canonical storage-budget durable frontier");
    let pending = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure canonical storage-budget pending bytes");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure canonical storage-budget terminal reservations");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure canonical storage-budget post-WSV reservations");
    let certified = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure canonical storage-budget certified-bundle reservations");
    used.checked_add(pending)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .expect("canonical storage-budget base fits u64")
}
#[test]
fn store_block_with_merge_entry_appends_log() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    let expected = entry.clone();
    kura.store_block(parent).expect("store carrier parent");
    kura.store_block_with_merge_entry(block, &entry)
        .expect("store block with merge entry");
    assert_eq!(kura.blocks_count(), 2);
    kura.block_data.lock()[1].1 = None;
    assert_eq!(
        kura.get_merge_entry_by_carrier_height(nonzero!(2_usize))
            .expect("resolve exact canonical carrier after a block-cache miss"),
        Some(expected.clone())
    );
    assert_eq!(kura.merge_ledger_snapshot(), vec![expected]);
}
#[test]
fn retained_merge_reference_survives_remote_only_body_eviction() {
    let kura = Kura::blank_kura_for_testing_with_blocks_in_memory(nonzero!(2_usize));
    let mut generator = DummyBlocks::new();
    let genesis = generator.next();
    let mut entry = sample_merge_entry(1);
    let carrier = next_merge_carrier(&mut generator, &mut entry);
    let expected_reference = CertifiedMergeLedgerReference::new(&entry);
    let third = generator.next();
    let fourth = generator.next();
    kura.store_block(Arc::clone(&genesis))
        .expect("store retained-witness genesis");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("store retained-witness merge carrier");
    kura.store_block(Arc::clone(&third))
        .expect("store retained-witness third block");
    kura.store_block(Arc::clone(&fourth))
        .expect("store retained-witness tail");
    let blocks = vec![genesis, carrier.clone(), third, fourth];
    let finality = v2_finality_artifacts_for_chain(&blocks)[1].clone();
    let _commit_receipt = kura
        .store_v2_finality_artifact(&finality)
        .expect("persist exact carrier finality and retained witness");
    let height = nonzero!(2_usize);
    let (_, payload_len) = advertise_required_replicas(&kura, height);
    assert!(
        kura.evict_block_bodies(payload_len)
            .expect("evict finalized merge carrier")
            > 0
    );
    kura.remove_evicted_block_sidecar_for_testing(height)
        .expect("make the retained merge carrier remote-only");
    assert!(
        kura.get_block_without_merge_sidecar(height).is_none(),
        "the test must not recover serving authority from a local body"
    );
    let (header, recovered_finality, recovered_reference) = kura
        .v2_finality_artifact_with_merge_reference(2)
        .expect("validate body-independent retained merge witness")
        .expect("retained merge witness exists");
    assert_eq!(header, carrier.header());
    assert_eq!(recovered_finality, finality);
    assert_eq!(recovered_reference, Some(expected_reference));
}
#[test]
fn merge_pending_cleanup_releases_block_data_before_waiting_for_sidecar_lock() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let carrier = next_merge_carrier(&mut blocks, &mut entry);
    let entry_hash = entry.canonical_hash();
    kura.store_block(parent).expect("store carrier parent");
    // Pause after the pending recovery source is durable, then acquire the
    // sidecar lock before allowing canonical publication to continue. This
    // isolates the post-commit cleanup wait from the earlier staging lock.
    kura.pause_next_store_after_pending_merge_stage_for_tests();
    let worker_kura = Arc::clone(&kura);
    let worker = thread::spawn(move || worker_kura.store_block_with_merge_entry(carrier, &entry));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.store_paused_after_pending_merge_stage_for_tests() {
        assert!(
            Instant::now() < deadline,
            "worker did not pause after staging the pending recovery sidecar"
        );
        thread::yield_now();
    }
    assert!(kura.pending_merge_entry_path(entry_hash).is_file());
    let sidecar_guard = kura.sidecar_lock.lock();
    kura.resume_store_after_pending_merge_stage_for_tests();
    let block_data_guard = loop {
        if let Some(guard) = kura.block_data.try_lock()
            && guard.len() == 2
        {
            break guard;
        }
        assert!(
            Instant::now() < deadline,
            "canonical in-memory state was not published and released while pending-sidecar cleanup waited for sidecar_lock"
        );
        thread::yield_now();
    };
    assert_eq!(
        block_data_guard.len(),
        2,
        "canonical in-memory state must be published before pending-sidecar cleanup"
    );
    drop(block_data_guard);
    drop(sidecar_guard);
    worker
        .join()
        .expect("join carrier store worker")
        .expect("store carrier after releasing sidecar lock");
    assert_eq!(
        kura.exact_durable_blocks_count()
            .expect("read stable durable count after carrier store"),
        2
    );
    assert!(
        !kura.pending_merge_entry_path(entry_hash).exists(),
        "committed pending sidecar should be removed after lock contention clears"
    );
}
#[test]
fn committed_block_succeeds_when_redundant_pending_cleanup_fails() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let carrier = next_merge_carrier(&mut blocks, &mut entry);
    let carrier_hash = carrier.hash();
    let entry_hash = entry.canonical_hash();
    kura.store_block(parent).expect("store carrier parent");
    kura.persist_pending_certified_merge_entry(&entry)
        .expect("persist pending merge entry");
    kura.fail_next_pending_merge_cleanup
        .store(true, Ordering::Relaxed);
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("redundant cleanup failure must not report canonical commit failure");
    assert_eq!(kura.blocks_count(), 2);
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(2_usize)),
        Some(carrier_hash)
    );
    assert_eq!(kura.merge_ledger_snapshot(), vec![entry.clone()]);
    assert!(
        kura.pending_merge_entry_path(entry_hash).is_file(),
        "injected cleanup failure must leave the redundant pending sidecar"
    );
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("idempotent retry cleans the redundant pending sidecar");
    assert!(!kura.pending_merge_entry_path(entry_hash).exists());
}
#[test]
fn corrupted_merge_carrier_fails_closed_after_block_cache_miss() {
    #[derive(Clone, Copy)]
    enum Corruption {
        BlockHash,
        EntryHash,
        Epoch,
    }
    for (corruption, label) in [
        (Corruption::BlockHash, "block hash"),
        (Corruption::EntryHash, "entry hash"),
        (Corruption::Epoch, "epoch"),
    ] {
        let (kura, mut blocks) = blank_kura_with_blocks();
        let parent = blocks.next();
        let mut entry = sample_merge_entry(1);
        let block = next_merge_carrier(&mut blocks, &mut entry);
        let height = NonZeroUsize::new(
            usize::try_from(block.header().height().get()).expect("carrier height fits usize"),
        )
        .expect("carrier height is non-zero");
        kura.store_block(parent).expect("store carrier parent");
        kura.store_block_with_merge_entry(block, &entry)
            .expect("store block with merge entry");
        let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
        let mut record = kura
            .merge_carrier_for_entry(entry.canonical_hash())
            .expect("load canonical carrier index")
            .expect("canonical carrier record");
        match corruption {
            Corruption::BlockHash => {
                record.block_hash = HashOf::from_untyped_unchecked(Hash::new(
                    b"adversarial merge carrier block hash",
                ));
            }
            Corruption::EntryHash => {
                record.entry_hash = HashOf::from_untyped_unchecked(Hash::new(
                    b"adversarial merge carrier entry hash",
                ));
            }
            Corruption::Epoch => record.epoch_id = record.epoch_id.saturating_add(1),
        }
        let path = kura.merge_carrier_path(record.block_height);
        let corrupted_bytes = norito::to_bytes(&record).expect("encode corrupt carrier record");
        std::fs::write(&path, &corrupted_bytes).expect("persist corrupt carrier fixture");
        *kura.merge_carrier_index.lock() = MergeCarrierIndex::default();
        kura.block_data.lock()[height.get() - 1].1 = None;
        let error = kura
            .get_merge_entry_by_carrier_height(height)
            .expect_err("corrupt carrier must fail closed");
        assert!(
            matches!(error, Error::MergeCarrierConflict(_)),
            "{label} corruption returned the wrong error: {error}"
        );
        assert_eq!(
            std::fs::read(&path).expect("reread corrupt carrier fixture"),
            corrupted_bytes,
            "{label} corruption must not be repaired or rewritten during lookup"
        );
        assert_eq!(
            kura.merge_ledger_snapshot(),
            vec![entry],
            "{label} corruption must not mutate the committed merge log"
        );
    }
}
#[test]
fn store_block_with_merge_entry_backfills_missing_log_for_existing_block() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    let expected = entry.clone();
    kura.store_block(parent).expect("store carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&block), &entry)
        .expect("store block and original merge entry");
    kura.truncate_merge_log_to_len(0)
        .expect("simulate missing merge log after durable carrier publication");
    kura.block_data.lock()[1].1 = None;
    let error = kura
        .get_merge_entry_by_carrier_height(nonzero!(2_usize))
        .expect_err("a carrier with no merge-log frame must fail closed");
    assert!(
        matches!(error, Error::MergeCarrierConflict(_)),
        "missing merge-log frame returned the wrong error: {error}"
    );
    kura.store_block_with_merge_entry(Arc::clone(&block), &entry)
        .expect("backfill merge entry");
    kura.store_block_with_merge_entry(block, &entry)
        .expect("idempotent retry");
    assert_eq!(kura.blocks_count(), 2);
    assert_eq!(kura.merge_ledger_snapshot(), vec![expected]);
}
#[test]
fn complete_merge_retry_ignores_unrelated_pending_sidecar_capacity() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    let block_hash = block.hash();
    kura.store_block(parent).expect("store carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&block), &entry)
        .expect("complete the original merge association");
    let directory = kura.pending_merge_entry_dir();
    std::fs::create_dir_all(&directory).expect("create pending sidecar directory");
    #[cfg(any(unix, windows))]
    let mut crash_published_path = None;
    for index in 0..kura.pending_control_sidecar_limits.certified_merge_entries {
        let mut pending = sample_merge_entry(200);
        pending.global_state_root = Hash::new(index.to_le_bytes());
        pending.merge_qc.message_digest = Hash::new(index.to_be_bytes());
        let path = kura.pending_merge_entry_path(pending.canonical_hash());
        std::fs::write(&path, pending.canonical_bytes())
            .expect("seed a valid unrelated pending sidecar");
        #[cfg(any(unix, windows))]
        if index == 0 {
            crash_published_path = Some(path);
        }
    }
    #[cfg(any(unix, windows))]
    {
        let target_path = crash_published_path.expect("capture a pending sidecar path");
        let temp_path = target_path.with_extension("norito.tmp");
        std::fs::hard_link(&target_path, &temp_path)
            .expect("stage a crash-published pending merge hard link at capacity");
        sync_dir(&directory).expect("sync crash-published pending merge fixture");
        kura.validate_pending_merge_entries_on_startup()
            .expect("recover the one extra hard-link path at logical capacity");
        assert!(
            !temp_path.exists(),
            "recovery must remove only the crash temporary"
        );
    }
    let mut overflow = sample_merge_entry(201);
    overflow.global_state_root = Hash::new(b"pending-capacity-overflow");
    assert!(
        kura.persist_pending_certified_merge_entry(&overflow)
            .is_err(),
        "fixture must saturate the pending sidecar count"
    );
    kura.store_block_with_merge_entry(block, &entry)
        .expect("a complete idempotent replay must not recreate its pending sidecar");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    assert_eq!(
        kura.merge_entry_for_carrier(2, block_hash)
            .expect("read complete merge association"),
        Some(entry)
    );
}
#[test]
fn store_block_with_merge_entry_rejects_out_of_order_existing_backfill() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let parent = blocks.next();
    let mut entry1 = sample_merge_entry(1);
    let block1 = next_merge_carrier(&mut blocks, &mut entry1);
    let mut entry2 = sample_merge_entry(2);
    let block2 = next_merge_carrier(&mut blocks, &mut entry2);
    kura.store_block(parent).expect("store carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&block1), &entry1)
        .expect("store first carrier and merge entry");
    kura.store_block_with_merge_entry(Arc::clone(&block2), &entry2)
        .expect("store second carrier and merge entry");
    kura.truncate_merge_log_to_len(0)
        .expect("simulate missing merge log before out-of-order backfill");
    let err = kura
        .store_block_with_merge_entry(block2.clone(), &entry2)
        .expect_err("merge log backfill must be sequential");
    assert!(matches!(
        err,
        Error::NoritoFrame(norito::core::Error::Message(message))
            if message.contains("expected 1, got 2")
    ));
    assert!(
        kura.merge_ledger_snapshot().is_empty(),
        "rejected epoch must not append a merge entry"
    );
    kura.store_block_with_merge_entry(block1, &entry1)
        .expect("backfill first merge entry");
    kura.store_block_with_merge_entry(block2, &entry2)
        .expect("backfill second merge entry after first");
    assert_eq!(kura.blocks_count(), 3);
    assert_eq!(kura.merge_ledger_snapshot(), vec![entry1, entry2]);
}
#[test]
fn store_block_with_merge_entry_conflict_does_not_append_log() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    let stored_hash = block.hash();
    let expected = entry.clone();
    kura.store_block(parent).expect("store carrier parent");
    kura.store_block_with_merge_entry(block, &entry)
        .expect("store block with merge entry");
    let genesis_hash = kura
        .get_block_hash(nonzero!(1_usize))
        .expect("stored genesis hash");
    let conflicting_raw: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(2_u64));
            header.set_prev_block_hash(Some(genesis_hash));
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    let mut conflicting_entry = entry.clone();
    let conflicting =
        bind_merge_entry_to_carrier(Arc::new(conflicting_raw), &mut conflicting_entry);
    let conflicting_hash = conflicting.hash();
    assert_ne!(stored_hash, conflicting_hash);
    let err = kura
        .store_block_with_merge_entry(conflicting, &conflicting_entry)
        .expect_err("same-height different hash must fail");
    assert!(matches!(
        err,
        Error::BlockHeightConflict {
            height: 2,
            expected,
            actual,
        } if expected == stored_hash && actual == conflicting_hash
    ));
    assert_eq!(kura.blocks_count(), 2);
    assert_eq!(kura.merge_ledger_snapshot(), vec![expected]);
}
#[test]
fn durable_block_payload_len_requires_committed_marker() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = FsyncMode::Batched;
    config.fsync_interval = Duration::from_secs(3600);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura init");
    let block = DummyBlocks::new().next();
    let block_hash = block.hash();
    {
        let mut store = kura.block_store.lock();
        store
            .append_block_to_chain(block.as_ref())
            .expect("append block without forced fsync");
        assert_eq!(store.read_index_count().expect("index count"), 1);
        assert_eq!(
            store
                .read_durable_index_count()
                .expect("durable index count"),
            0,
            "batched append should leave the commit marker behind"
        );
    }
    kura.block_data
        .lock()
        .push((block_hash, Some(Arc::clone(&block))));
    kura.set_block_height_index_entry(1, block_hash);
    assert_eq!(
        kura.durable_block_payload_len_by_hash(block_hash),
        None,
        "replica metadata must not be advertised before the block is durable"
    );
    {
        let mut store = kura.block_store.lock();
        store
            .flush_pending_fsync(true)
            .expect("force pending fsync");
    }
    let (height, payload_len) = kura
        .durable_block_payload_len_by_hash(block_hash)
        .expect("durable payload metadata after marker advances");
    let index_len = {
        let mut store = kura.block_store.lock();
        store.read_block_index(0).expect("block index").length
    };
    assert_eq!(height, 1);
    assert_eq!(payload_len, index_len);
}
#[test]
fn replace_top_block_same_hash_requires_durable_marker() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = FsyncMode::Batched;
    config.fsync_interval = Duration::from_secs(3600);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("kura init");
    let block = DummyBlocks::new().next();
    let block_hash = block.hash();
    {
        let mut store = kura.block_store.lock();
        store
            .append_block_to_chain(block.as_ref())
            .expect("append block without forced fsync");
    }
    kura.block_data
        .lock()
        .push((block_hash, Some(Arc::clone(&block))));
    kura.set_block_height_index_entry(1, block_hash);
    let err = kura
        .replace_top_block(Arc::clone(&block))
        .expect_err("idempotent replace still needs durable Kura marker");
    assert!(matches!(
        err,
        Error::BlockHeightGap {
            expected_next_height: 1,
            actual_height: 1,
        }
    ));
    {
        let mut store = kura.block_store.lock();
        store
            .flush_pending_fsync(true)
            .expect("force pending fsync");
    }
    kura.replace_top_block(block)
        .expect("idempotent replace succeeds after durable marker");
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(block_hash)
    );
}
#[test]
fn store_block_is_durable_before_return() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_interval = Duration::from_secs(3600);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("initialize kura");
    let block = DummyBlocks::new().next();
    let block_hash = block.hash();
    kura.store_block(block).expect("store block");
    let mut store = kura.block_store.lock();
    assert_eq!(store.read_index_count().expect("index count"), 1);
    assert_eq!(
        store
            .read_durable_index_count()
            .expect("durable index count"),
        1,
        "commit marker must advance before store_block returns"
    );
    assert_eq!(
        store.read_block_hashes(0, 1).expect("stored hash"),
        vec![block_hash]
    );
}
#[test]
fn store_block_is_idempotent_for_same_height_and_hash() {
    let (kura, block) = blank_kura_with_next_block();
    let block_hash = block.hash();
    kura.store_block(Arc::clone(&block)).expect("store block");
    let (index_len, data_len, hashes_len) = {
        let mut store = kura.block_store.lock();
        (
            store.index_file_len().expect("index len"),
            store.data_file_len().expect("data len"),
            store.hashes_file_len().expect("hashes len"),
        )
    };
    kura.store_block(block).expect("idempotent store");
    assert_eq!(kura.blocks_count(), 1);
    let mut store = kura.block_store.lock();
    assert_eq!(store.index_file_len().expect("index len"), index_len);
    assert_eq!(store.data_file_len().expect("data len"), data_len);
    assert_eq!(store.hashes_file_len().expect("hashes len"), hashes_len);
    assert_eq!(
        store.read_block_hashes(0, 1).expect("stored hash"),
        vec![block_hash]
    );
}
#[test]
fn store_block_rejects_height_gap() {
    let kura = Kura::blank_kura_for_testing();
    let block: SignedBlock = ValidBlock::new_dummy(checked_keypair().private_key()).into();
    let err = kura.store_block(block).expect_err("height gap");
    assert!(matches!(
        err,
        Error::BlockHeightGap {
            expected_next_height: 1,
            actual_height: 2,
        }
    ));
    assert_eq!(kura.blocks_count(), 0);
}
#[test]
fn store_block_rejects_same_height_different_hash() {
    let (kura, block) = blank_kura_with_next_block();
    let stored_hash = block.hash();
    kura.store_block(block).expect("store first block");
    let conflicting: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    let conflicting_hash = conflicting.hash();
    assert_ne!(stored_hash, conflicting_hash);
    let err = kura
        .store_block(conflicting)
        .expect_err("same-height different hash must fail");
    assert!(matches!(
        err,
        Error::BlockHeightConflict {
            height: 1,
            expected,
            actual,
        } if expected == stored_hash && actual == conflicting_hash
    ));
    assert_eq!(kura.blocks_count(), 1);
}
#[test]
fn store_block_with_merge_entry_stages_retry_without_publishing_on_block_write_failure() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    let entry_hash = entry.canonical_hash();
    kura.store_block(parent).expect("store carrier parent");
    kura.fail_next_block_write_for_tests();
    let err = kura
        .store_block_with_merge_entry(block, &entry)
        .expect_err("injected block write failure");
    assert!(matches!(err, Error::IO(_, _)));
    assert_eq!(kura.blocks_count(), 1);
    assert!(
        kura.merge_ledger_snapshot().is_empty(),
        "a pre-commit block failure must not publish the merge log"
    );
    assert_eq!(
        kura.merge_carrier_for_entry(entry_hash)
            .expect("carrier index remains readable"),
        None,
        "a pre-commit block failure must not publish a carrier association"
    );
    assert_eq!(
        kura.pending_certified_merge_entries()
            .expect("read staged retry sidecars"),
        vec![(entry_hash, entry)],
        "the exact pre-staged entry must remain available for a later retry"
    );
    assert!(
        kura.merge_carrier_records()
            .expect("carrier rollback snapshot")
            .is_empty(),
        "merge carrier must be rolled back when block write fails"
    );
}
#[test]
fn store_block_injected_failure_aborts_sync_append() {
    let (kura, block) = blank_kura_with_next_block();
    kura.fail_next_store_for_tests();
    let result = kura.store_block(block.clone());
    assert!(result.is_err());
    assert_eq!(
        kura.blocks_count(),
        0,
        "failing append should not expose the block in memory"
    );
    assert!(kura.merge_ledger_snapshot().is_empty());
}
#[test]
fn store_block_rejects_when_budget_exceeded() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let baseline = canonical_storage_budget_base_for_test(&kura);
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = baseline.saturating_add(1);
    let block = DummyBlocks::new().next();
    let metrics = Arc::new(Metrics::default());
    let telemetry = StateTelemetry::new(metrics.clone(), true);
    kura.attach_telemetry(telemetry);
    let err = kura
        .store_block(block)
        .expect_err("budgeted kura should reject new blocks");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    assert_eq!(kura.blocks_count(), 0);
    let expected_used = canonical_storage_budget_base_for_test(&kura);
    assert_eq!(
        metrics
            .storage_budget_bytes_used
            .with_label_values(&["kura"])
            .get(),
        expected_used
    );
    assert_eq!(
        metrics
            .storage_budget_bytes_limit
            .with_label_values(&["kura"])
            .get(),
        kura.max_disk_usage_bytes
    );
    assert_eq!(
        metrics
            .storage_budget_exceeded_total
            .with_label_values(&["kura"])
            .get(),
        1
    );
}
#[test]
fn durable_budget_snapshot_avoids_repeated_metadata_reads() {
    let kura = Kura::blank_kura_for_testing();
    kura.invalidate_durable_budget_snapshot();
    assert_eq!(
        kura.durable_budget_metadata_reads.load(Ordering::Relaxed),
        0
    );
    assert_eq!(
        kura.persisted_count_and_unindexed_bytes()
            .expect("cold durable budget snapshot"),
        (0, 0)
    );
    assert_eq!(
        kura.durable_budget_metadata_reads.load(Ordering::Relaxed),
        1,
        "cold snapshot should use one raw metadata read"
    );
    assert_eq!(
        kura.persisted_count_and_unindexed_bytes()
            .expect("cached durable budget snapshot"),
        (0, 0)
    );
    assert_eq!(
        kura.durable_budget_metadata_reads.load(Ordering::Relaxed),
        1,
        "cached snapshot should avoid repeated raw metadata reads"
    );
    let block = DummyBlocks::new().next();
    kura.persist_block_immediate_for_tests(&block);
    assert_eq!(
        kura.persisted_count_and_unindexed_bytes()
            .expect("published durable budget snapshot"),
        (1, 0)
    );
    assert_eq!(
        kura.durable_budget_metadata_reads.load(Ordering::Relaxed),
        1,
        "successful append should publish durable budget metadata directly"
    );
}
#[test]
fn kura_budget_check_scales_with_pending_depth() {
    const PENDING_DEPTH: usize = 128;
    let mut kura = Kura::blank_kura_for_testing();
    Arc::get_mut(&mut kura)
        .expect("exclusive test Kura")
        .max_disk_usage_bytes = u64::MAX / 4;
    let mut blocks = DummyBlocks::new();
    for _ in 0..PENDING_DEPTH {
        kura.append_pending_block_for_bench(blocks.next());
    }
    assert_eq!(kura.pending_budget_raw_scans.load(Ordering::Relaxed), 0);
    let candidate = blocks.next();
    for _ in 0..16 {
        kura.check_storage_budget_for_bench(candidate.as_ref())
            .expect("budget check should fit within the large test limit");
    }
    assert_eq!(
        kura.pending_budget_raw_scans.load(Ordering::Relaxed),
        1,
        "cached pending bytes should avoid repeated raw pending-queue scans"
    );
    let cached_pending_bytes = kura.pending_budget_bytes.load(Ordering::Relaxed);
    assert!(
        cached_pending_bytes > 0,
        "pending budget cache should include queued blocks"
    );
    let extra_pending = blocks.next();
    kura.append_pending_block_for_bench(extra_pending);
    let replacement_candidate = blocks.next();
    kura.check_storage_budget_for_bench(replacement_candidate.as_ref())
        .expect("budget check should still fit after adding one pending block");
    assert_eq!(
        kura.pending_budget_raw_scans.load(Ordering::Relaxed),
        2,
        "cache invalidation should force exactly one fresh raw pending scan"
    );
    assert!(
        kura.pending_budget_bytes.load(Ordering::Relaxed) > cached_pending_bytes,
        "fresh pending cache should include the additional pending block"
    );
}
#[test]
fn store_block_with_merge_entry_counts_budget() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let block = DummyBlocks::new().next();
    let block_required = Kura::block_required_bytes(&block).expect("block bytes");
    let kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let baseline = canonical_storage_budget_base_for_test(&kura);
    let association_stage_required = kura
        .canonical_association_stage_additional_bytes(block.as_ref(), None)
        .expect("account canonical association stage");
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = baseline
        .saturating_add(block_required)
        .saturating_add(association_stage_required);
    kura.store_block(block).expect("budgeted store block");
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    let parent_required = Kura::block_required_bytes(&parent).expect("parent block bytes");
    let block_required = Kura::block_required_bytes(&block).expect("carrier block bytes");
    let merge_log_required = Kura::merge_entry_bytes(&entry).expect("merge log frame bytes");
    let carrier_required = u64::try_from(
        norito::to_bytes(&MergeLedgerCarrierRecord::new(&entry, &block))
            .expect("encode carrier record")
            .len(),
    )
    .expect("carrier record bytes fit u64");
    let mut kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    kura_cfg.max_disk_usage_bytes = iroha_config::base::util::Bytes(u64::MAX);
    let (mut kura, _) =
        Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let baseline = canonical_storage_budget_base_for_test(&kura);
    let parent_association_stage_required = kura
        .canonical_association_stage_additional_bytes(parent.as_ref(), None)
        .expect("account parent association stage");
    let carrier_association_stage_required = kura
        .canonical_association_stage_additional_bytes(block.as_ref(), Some(&entry))
        .expect("account carrier association stage");
    let parent_peak = baseline
        .saturating_add(parent_required)
        .saturating_add(parent_association_stage_required);
    let carrier_peak_without_pending_duplicate = baseline
        .saturating_add(parent_required)
        .saturating_add(block_required)
        .saturating_add(merge_log_required)
        .saturating_add(carrier_required)
        .saturating_add(carrier_association_stage_required);
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = parent_peak.max(carrier_peak_without_pending_duplicate);
    kura.store_block(parent).expect("store carrier parent");
    let err = kura
        .store_block_with_merge_entry(block, &entry)
        .expect_err("transient pending/log duplication should exceed budget");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
}
#[test]
fn store_block_rejects_when_storage_exceeds_budget() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut blocks = DummyBlocks::new();
    let block1 = blocks.next();
    let block2 = blocks.next();
    let block1_required = Kura::block_required_bytes(&block1).expect("block1 required bytes");
    let block2_required = Kura::block_required_bytes(&block2).expect("block2 required bytes");
    let kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let baseline = canonical_storage_budget_base_for_test(&kura);
    let block1_peak = block1_required.saturating_add(
        kura.canonical_association_stage_additional_bytes(block1.as_ref(), None)
            .expect("account block1 association stage"),
    );
    let block2_peak = block2_required.saturating_add(
        kura.canonical_association_stage_additional_bytes(block2.as_ref(), None)
            .expect("account block2 association stage"),
    );
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = baseline.saturating_add(block1_peak.max(block2_peak));
    kura.store_block(block1).expect("store first block");
    let err = kura
        .store_block(block2)
        .expect_err("stored bytes should exceed budget");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
}
#[test]
fn store_block_rejects_when_single_block_exceeds_budget() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let kura_cfg = kura_config_for_dir(
        &temp_dir,
        NonZeroUsize::new(1).expect("non-zero"),
    );
    let (mut kura, _) =
        Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let used = kura.kura_disk_usage_bytes().expect("baseline usage");
    let overhead = BlockIndex::SIZE.saturating_add(SIZE_OF_BLOCK_HASH);
    let budget_limit = used
        .saturating_add(overhead.saturating_mul(2))
        .saturating_add(1);
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = budget_limit;
    let make_block = |message: &str, prev: Option<&SignedBlock>| -> Arc<SignedBlock> {
        let tx = TransactionBuilder::new(
            test_network_id(b"test"),
            SAMPLE_GENESIS_ACCOUNT_ID.to_owned(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, message.to_owned())])
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
        let acc = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        Arc::new(
            BlockBuilder::new(vec![acc])
                .chain(0, prev)
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
                .unpack(|_| {})
                .into(),
        )
    };
    let payload = "x".repeat(4096);
    let block1 = make_block(&payload, None);
    let block1_required = Kura::block_required_bytes(&block1).expect("block1 bytes");
    assert!(
        block1_required > budget_limit,
        "expected block to exceed budget"
    );
    let err = kura
        .store_block(Arc::clone(&block1))
        .expect_err("single block larger than the budget should be rejected");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    assert_eq!(kura.blocks_count(), 0);
}
#[test]
fn store_block_schedules_background_eviction_when_budget_exceeded() {
    let case = background_budget_eviction_case();
    let kura = case.kura;
    let block4 = case.retry_block;
    let block2_len = case.evictable_body_len;
    let err = kura
        .store_block(Arc::clone(&block4))
        .expect_err("over-budget store should schedule eviction and fail fast");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    assert!(
        kura.pending_budget_eviction_bytes.load(Ordering::Acquire) > 0,
        "budget check should request background eviction"
    );
    {
        let rx_guard = kura.block_notify_rx.lock();
        let rx = rx_guard
            .as_ref()
            .expect("writer receiver should be available before Kura::start");
        assert_eq!(
            rx.try_recv(),
            Ok(BlockNotify::StorageBudgetEviction),
            "budget eviction should wake the Kura writer"
        );
    }
    let (index_before, da_path_before) = {
        let mut store = kura.block_store.lock();
        (
            store.read_block_index(1).expect("block2 index"),
            store.da_block_path(2),
        )
    };
    assert!(
        !index_before.is_evicted(),
        "budget checks must not compact block storage inline"
    );
    assert!(
        !da_path_before.exists(),
        "foreground budget check should not create DA sidecars inline"
    );
    let freed = kura.flush_pending_budget_eviction();
    assert!(
        freed >= block2_len,
        "background maintenance should reclaim the replicated block body"
    );
    kura.store_block(Arc::clone(&block4))
        .expect("retry should store block4 after background eviction");
    wait_for_block_hash(&kura, 4, block4.hash());
    let (index, da_path) = {
        let mut store = kura.block_store.lock();
        (
            store.read_block_index(1).expect("block2 index"),
            store.da_block_path(2),
        )
    };
    assert!(index.is_evicted());
    assert!(
        da_path.exists(),
        "budget eviction should create a DA sidecar"
    );
    assert!(
        kura.get_block(nonzero!(2_usize)).is_some(),
        "DA-sidecar-backed body should remain locally readable"
    );
}
#[test]
fn kura_start_rejects_unbound_local_peer_identity() {
    let kura = Kura::blank_kura_for_testing();
    assert!(matches!(
        Kura::start(kura, ShutdownSignal::new()),
        Err(Error::KuraReplicaLocalPeerUnbound)
    ));
}
#[test]
fn kura_background_eviction_retry_latency_threshold() {
    const BACKGROUND_EVICTION_RETRY_THRESHOLD: Duration = Duration::from_secs(2);
    let case = background_budget_eviction_case();
    let kura = case.kura;
    let block4 = case.retry_block;
    kura.bind_local_peer_id(checked_peer_id())
        .expect("bind local peer before Kura start");
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let shutdown_signal = ShutdownSignal::new();
    let _handle = {
        let _rt_guard = rt.enter();
        Kura::start(Arc::clone(&kura), shutdown_signal.clone())
    };
    let started_at = Instant::now();
    let err = kura
        .store_block(Arc::clone(&block4))
        .expect_err("over-budget store should fail fast before maintenance");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    let deadline = started_at + BACKGROUND_EVICTION_RETRY_THRESHOLD;
    loop {
        let pending = kura.pending_budget_eviction_bytes.load(Ordering::Acquire);
        let (index, da_path) = {
            let mut store = kura.block_store.lock();
            (
                store.read_block_index(1).expect("block2 index"),
                store.da_block_path(2),
            )
        };
        if pending == 0 && index.is_evicted() && da_path.exists() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background Kura budget eviction did not complete within {:?}",
            BACKGROUND_EVICTION_RETRY_THRESHOLD
        );
        thread::sleep(Duration::from_millis(10));
    }
    kura.store_block(Arc::clone(&block4))
        .expect("retry should store block4 after writer maintenance");
    wait_for_block_hash(&kura, 4, block4.hash());
    let elapsed = started_at.elapsed();
    assert!(
        elapsed <= BACKGROUND_EVICTION_RETRY_THRESHOLD,
        "background Kura budget eviction plus retry took {elapsed:?}, threshold {:?}",
        BACKGROUND_EVICTION_RETRY_THRESHOLD
    );
    assert!(
        kura.get_block(nonzero!(2_usize)).is_some(),
        "evicted body should remain readable through the DA sidecar"
    );
    shutdown_signal.send();
}
#[test]
fn replace_top_block_rejects_when_replacement_exceeds_budget() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (mut kura, _) = Kura::new(&kura_cfg, &lane_config).expect("initialize kura");
    let make_block = |message: &str| -> SignedBlock {
        let tx = TransactionBuilder::new(
            test_network_id(b"test"),
            SAMPLE_GENESIS_ACCOUNT_ID.to_owned(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, message.to_owned())])
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
        let acc = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        BlockBuilder::new(vec![acc])
            .chain(0, None)
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into()
    };
    let small_block = make_block("short");
    let large_block = make_block(&"x".repeat(4096));
    let ownership = small_block
        .execution_context()
        .and_then(|context| context.lane_payload_ownerships.first())
        .expect("test block carries default lane ownership");
    assert_eq!(
        large_block
            .execution_context()
            .and_then(|context| context.lane_payload_ownerships.first())
            .map(|candidate| candidate.lane_incarnation),
        Some(ownership.lane_incarnation),
        "replacement uses the same default lane incarnation"
    );
    let lane_entry = lane_config
        .entry(ownership.lane_id)
        .expect("default lane is configured");
    kura.install_lane_incarnation_marker_for_test(lane_entry, ownership.lane_incarnation, 0)
        .expect("install default lane marker");
    let small_bytes = kura
        .block_required_bytes_for_budget(&small_block, None, u64::MAX)
        .expect("small bytes");
    let large_bytes = kura
        .block_required_bytes_for_budget(&large_block, None, u64::MAX)
        .expect("large bytes");
    assert!(
        large_bytes > small_bytes,
        "expected large block to be larger"
    );
    let baseline = canonical_storage_budget_base_for_test(&kura);
    let small_association_stage_bytes = kura
        .canonical_association_stage_additional_bytes(&small_block, None)
        .expect("account small-block association stage");
    let large_association_stage_bytes = kura
        .canonical_association_stage_additional_bytes(&large_block, None)
        .expect("account large-block association stage");
    let limit = baseline
        .saturating_add(small_bytes)
        .saturating_add(small_association_stage_bytes);
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = limit;
    kura.bind_local_peer_id(checked_peer_id())
        .expect("bind local peer before Kura start");
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let _handle = {
        let _rt_guard = rt.enter();
        Kura::start(kura.clone(), ShutdownSignal::new())
    };
    let small_hash = small_block.hash();
    kura.store_block(small_block).expect("store small block");
    assert!(
        baseline
            .saturating_add(large_bytes)
            .saturating_add(large_association_stage_bytes)
            > limit,
        "expected replacement block to exceed budget"
    );
    let err = kura
        .replace_top_block(large_block.clone())
        .expect_err("replacement larger than the budget should be rejected");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    let (index, da_path) = {
        let mut store = kura.block_store.lock();
        (
            store.read_block_index(0).expect("block index"),
            store.da_block_path(1),
        )
    };
    assert!(!index.is_evicted());
    assert!(
        !da_path.exists(),
        "rejected replacement must not create a sidecar payload"
    );
    assert_eq!(
        kura.get_block_hash(nonzero!(1_usize)),
        Some(small_hash),
        "rejected replacement must leave the original top block visible"
    );
}
#[test]
fn store_block_rejects_when_sidecar_bytes_exceed_budget() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let block = DummyBlocks::new().next();
    let budget_limit = Kura::block_required_bytes(&block).expect("block bytes");
    let kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let association_stage_required = kura
        .canonical_association_stage_additional_bytes(block.as_ref(), None)
        .expect("account canonical association stage");
    let exact_limit = canonical_storage_budget_base_for_test(&kura)
        .saturating_add(budget_limit)
        .saturating_add(association_stage_required);
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = exact_limit;
    let blocks_dir = RuntimeLaneConfig::default()
        .primary()
        .blocks_dir(temp_dir.path());
    let pipeline_dir = blocks_dir.join(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    std::fs::write(pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE), [0u8; 1])
        .expect("write sidecar data");
    kura.refresh_disk_usage_bytes()
        .expect("refresh disk usage after sidecar write");
    let err = kura
        .store_block(block)
        .expect_err("sidecar bytes should exceed budget");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    let native_temp_dir = TempDir::new().expect("create Native AMX budget temp dir");
    let mut native_cfg = kura_config_for_dir(&native_temp_dir, BLOCKS_IN_MEMORY);
    native_cfg.lane_history_retention = nonzero!(2_usize);
    let (mut native_kura, _) =
        Kura::new(&native_cfg, &RuntimeLaneConfig::default()).expect("initialize Native Kura");
    let configured_prune_bound = Kura::native_amx_evidence_prune_intent_max_bytes_for_retention(
        native_cfg.lane_history_retention,
        V2_PENDING_CONTROL_SIDECAR_BYTES.get(),
    )
    .expect("configured Native AMX prune bound");
    assert_eq!(
        native_kura.native_amx_evidence_prune_intent_max_bytes(),
        configured_prune_bound
    );
    assert!(
        configured_prune_bound
            < Kura::native_amx_evidence_prune_intent_max_bytes_for_retention(
                LANE_HISTORY_RETENTION,
                V2_PENDING_CONTROL_SIDECAR_BYTES.get(),
            )
            .expect("default Native AMX prune bound"),
        "the prune-journal hard limit must derive from configured retention"
    );
    let native_block = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    assert!(
        native_block
            .execution_context()
            .is_none_or(|context| context.lane_payload_ownerships.is_empty()),
        "Native accounting fixture must isolate standalone evidence bytes"
    );
    let native_manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
            &native_block,
        )
        .expect("construct Native AMX accounting manifest");
    let placeholder_finality_hash = HashOf::from_untyped_unchecked(Hash::new(
        b"Native AMX application disk-accounting finality placeholder",
    ));
    let mut exact_evidence_bytes = 0_u64;
    let mut unique_routes = BTreeSet::new();
    for (index, entry) in native_manifest.entries().iter().enumerate() {
        let leaf_index = u32::try_from(index).expect("fixture leaf index fits u32");
        let manifest_artifact = NativeAmxParticipantApplicationManifestArtifactV1 {
            version: NativeAmxParticipantApplicationManifestArtifactV1::VERSION,
            leaf: entry.leaf.clone(),
            leaf_index,
            proof: native_manifest
                .proof(leaf_index)
                .expect("fixture manifest proof"),
            manifest_root: native_manifest.root(),
            manifest_leaf_count: native_manifest.count(),
            finality_artifact_hash: placeholder_finality_hash,
        };
        let receipt = NativeAmxParticipantApplicationReceiptArtifact::new(
            entry,
            HashOf::new(&manifest_artifact),
            placeholder_finality_hash,
        );
        let latest = NativeAmxParticipantReceiptLatestIndexV2::from_receipt(&receipt);
        exact_evidence_bytes = exact_evidence_bytes
            .checked_add(
                u64::try_from(
                    manifest_artifact
                        .encode_framed()
                        .expect("encode fixture manifest")
                        .len(),
                )
                .expect("manifest length fits u64"),
            )
            .and_then(|bytes| {
                bytes.checked_add(
                    u64::try_from(
                        receipt
                            .encode_framed()
                            .expect("encode fixture receipt")
                            .len(),
                    )
                    .expect("receipt length fits u64"),
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    u64::try_from(
                        norito::to_bytes(&latest)
                            .expect("encode fixture latest index")
                            .len(),
                    )
                    .expect("latest length fits u64"),
                )
            })
            .expect("fixture evidence accounting does not overflow");
        unique_routes.insert((
            entry.leaf.lane_id,
            entry.leaf.dataspace_id,
            entry.leaf.lane_incarnation,
        ));
    }
    assert!(
        unique_routes.len() > 1,
        "fixture must cover multiple routes"
    );
    let expected_native_artifacts = exact_evidence_bytes
        .checked_add(
            u64::try_from(configured_prune_bound)
                .expect("configured prune bound fits u64")
                .checked_mul(u64::try_from(unique_routes.len()).expect("route count fits u64"))
                .expect("prune reservation does not overflow"),
        )
        .expect("Native artifact accounting does not overflow");
    assert_eq!(
        native_kura
            .lane_artifact_required_bytes_for_block(&native_block, None)
            .expect("account Native artifacts"),
        expected_native_artifacts,
        "each unique Native route must reserve exactly one configured prune journal"
    );
    let native_required = native_kura
        .block_required_bytes_for_budget(&native_block, None, u64::MAX)
        .expect("account complete Native block");
    let native_association_stage_required = native_kura
        .canonical_association_stage_additional_bytes(&native_block, None)
        .expect("account Native canonical association stage");
    let exact_limit = canonical_storage_budget_base_for_test(&native_kura)
        .checked_add(native_required)
        .and_then(|bytes| bytes.checked_add(native_association_stage_required))
        .expect("exact Native storage budget fits u64");
    Arc::get_mut(&mut native_kura)
        .expect("exclusive Native Kura before budget check")
        .max_disk_usage_bytes = exact_limit;
    native_kura
        .check_storage_budget(&native_block, None)
        .expect("exact Native evidence budget must admit the block");
    Arc::get_mut(&mut native_kura)
        .expect("exclusive Native Kura before negative budget check")
        .max_disk_usage_bytes = exact_limit - 1;
    let err = native_kura
        .check_storage_budget(&native_block, None)
        .expect_err("one byte below exact Native evidence budget must reject");
    assert!(matches!(
        err,
        Error::StorageBudgetExceeded {
            limit,
            required,
            ..
        } if limit == exact_limit - 1 && required == exact_limit
    ));
}
#[test]
fn kura_disk_usage_includes_temp_and_debug_files() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    kura_cfg.debug_output_new_blocks = true;
    let (kura, _) = Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let base = kura.disk_usage_bytes().expect("base usage");
    let blocks_dir = RuntimeLaneConfig::default()
        .primary()
        .blocks_dir(temp_dir.path());
    let debug_path = kura
        .block_plain_text_path
        .lock()
        .clone()
        .expect("debug path");
    std::fs::write(&debug_path, [0u8; 7]).expect("write debug blocks");
    let temp_marker = blocks_dir
        .join(COUNT_FILE_NAME)
        .with_extension("norito.tmp");
    std::fs::write(&temp_marker, [0u8; 5]).expect("write temp marker");
    let pipeline_dir = blocks_dir.join(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    let temp_sidecar = pipeline_dir
        .join(PIPELINE_SIDECARS_DATA_FILE)
        .with_extension("norito.tmp");
    std::fs::write(&temp_sidecar, [0u8; 3]).expect("write temp sidecar");
    let updated = kura.refresh_disk_usage_bytes().expect("usage with extras");
    let extra = 7u64 + 5 + 3;
    assert_eq!(updated, base.saturating_add(extra));
}
#[test]
fn purge_retired_segments_removes_retired_dir() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let active_merge_dir = temp_dir.path().join("merge_ledger");
    std::fs::create_dir_all(&active_merge_dir).expect("create active merge directory");
    std::fs::write(active_merge_dir.join("accounting-baseline.log"), [0u8; 64])
        .expect("write surviving accounting baseline");
    let enforced_before = kura
        .refresh_disk_usage_bytes()
        .expect("measure enforced baseline");
    let total_before = kura
        .refresh_total_disk_usage_bytes()
        .expect("measure total baseline");
    let retired_root = temp_dir.path().join("retired");
    let retired_dir = RuntimeLaneConfig::default()
        .primary()
        .blocks_dir(&retired_root);
    std::fs::create_dir_all(&retired_dir).expect("create retired block store");
    std::fs::write(retired_dir.join(DATA_FILE_NAME), [0u8; 11])
        .expect("write budgeted retired bytes");
    let retained_dir = retired_dir.join(RETAINED_BLOCKS_DIR_NAME);
    std::fs::create_dir_all(&retained_dir).expect("create retired retained-record directory");
    std::fs::write(retained_dir.join("accounting.norito"), [0u8; 7])
        .expect("write total-only retired bytes");
    let retired_merge_dir = retired_root.join("merge_ledger");
    std::fs::create_dir_all(&retired_merge_dir).expect("create retired merge directory");
    std::fs::write(retired_merge_dir.join("accounting.log"), [0u8; 5])
        .expect("write retired merge bytes");
    assert_eq!(
        kura.refresh_disk_usage_bytes()
            .expect("measure seeded retired budget usage"),
        enforced_before.saturating_add(16)
    );
    assert_eq!(
        kura.refresh_total_disk_usage_bytes()
            .expect("measure seeded retired total usage"),
        total_before.saturating_add(23)
    );
    assert!(
        kura.purge_retired_segments().unwrap(),
        "purge should remove retired data"
    );
    assert!(
        !temp_dir.path().join("retired").exists(),
        "retired dir should be removed"
    );
    assert_eq!(
        kura.disk_usage.load(Ordering::Relaxed),
        enforced_before,
        "retired purge must subtract the enforced bytes exactly once"
    );
    assert_eq!(
        kura.disk_usage_total.load(Ordering::Relaxed),
        total_before,
        "retired purge must not subtract enforced bytes twice from total usage"
    );
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("rescan enforced usage after retired purge"),
        enforced_before
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("rescan total usage after retired purge"),
        total_before
    );
}
#[test]
fn total_disk_usage_scan_retries_across_replacement_and_unfinished_mutation() {
    let (temp_dir, config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
    let merge_dir = temp_dir.path().join("merge_ledger");
    std::fs::create_dir_all(&merge_dir).expect("create accounted merge directory");
    let path = merge_dir.join("accounting-race.log");
    std::fs::write(&path, [0_u8; 4]).expect("seed existing accounted file");
    let enforced_baseline = kura
        .refresh_disk_usage_bytes()
        .expect("establish enforced-usage baseline");
    let baseline = kura
        .refresh_total_disk_usage_bytes()
        .expect("establish total-usage baseline");
    kura.pause_next_total_disk_usage_scan_after_scan_for_tests();
    let scan_kura = Arc::clone(&kura);
    let (scan_tx, scan_rx) = mpsc::channel();
    let scan = thread::spawn(move || {
        scan_tx
            .send(scan_kura.refresh_total_disk_usage_bytes())
            .expect("report total-usage scan result");
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.total_disk_usage_scan_paused_for_tests() {
        assert!(
            Instant::now() < deadline,
            "total-usage scan did not reach its publication barrier"
        );
        thread::yield_now();
    }
    let accounting_mutation = kura.begin_total_disk_usage_mutation();
    std::fs::write(&path, [0_u8; 9]).expect("replace existing accounted file");
    kura.update_disk_usage_delta(4, 9);
    accounting_mutation.finish();
    assert!(
        matches!(
            scan_rx.recv_timeout(Duration::from_millis(50)),
            Err(RecvTimeoutError::Timeout)
        ),
        "a stale scan must not publish before its deterministic barrier is released"
    );
    kura.resume_total_disk_usage_scan_for_tests();
    let refreshed = scan_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("total-usage scan must retry")
        .expect("retried total-usage scan succeeds");
    scan.join().expect("join total-usage scan");
    let exact_after_replacement = kura
        .kura_total_disk_usage_bytes()
        .expect("rescan after replacement");
    assert_eq!(exact_after_replacement, baseline.saturating_add(5));
    assert_eq!(refreshed, exact_after_replacement);
    assert_eq!(
        kura.disk_usage_total.load(Ordering::Relaxed),
        exact_after_replacement,
        "generation change must force the stale scan to retry before publication"
    );
    assert_eq!(
        kura.disk_usage.load(Ordering::Relaxed),
        enforced_baseline.saturating_add(5)
    );
    kura.pause_next_total_disk_usage_scan_after_scan_for_tests();
    let budget_scan_kura = Arc::clone(&kura);
    let (budget_tx, budget_rx) = mpsc::channel();
    let budget_scan = thread::spawn(move || {
        budget_tx
            .send(budget_scan_kura.refresh_disk_usage_bytes())
            .expect("report enforced-usage scan result");
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.total_disk_usage_scan_paused_for_tests() {
        assert!(
            Instant::now() < deadline,
            "enforced-usage scan did not reach its publication barrier"
        );
        thread::yield_now();
    }
    let budget_mutation = kura.begin_total_disk_usage_mutation();
    std::fs::write(&path, [0_u8; 11]).expect("replace existing budgeted file");
    kura.update_disk_usage_delta(9, 11);
    budget_mutation.finish();
    assert!(matches!(
        budget_rx.recv_timeout(Duration::from_millis(50)),
        Err(RecvTimeoutError::Timeout)
    ));
    kura.resume_total_disk_usage_scan_for_tests();
    let refreshed_enforced = budget_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("enforced-usage scan must retry")
        .expect("retried enforced-usage scan succeeds");
    budget_scan.join().expect("join enforced-usage scan");
    let exact_enforced = kura
        .kura_disk_usage_bytes()
        .expect("rescan enforced usage after replacement");
    let exact_total = kura
        .kura_total_disk_usage_bytes()
        .expect("rescan total usage after budget replacement");
    assert_eq!(refreshed_enforced, exact_enforced);
    assert_eq!(
        kura.disk_usage.load(Ordering::Relaxed),
        exact_enforced,
        "stale enforced scan must retry before publication"
    );
    assert_eq!(
        kura.disk_usage_total.load(Ordering::Relaxed),
        exact_total,
        "combined refresh must publish total usage from the same stable generation"
    );
    let unfinished = kura.begin_total_disk_usage_mutation();
    std::fs::write(&path, [0_u8; 13]).expect("simulate a partially accounted replacement");
    drop(unfinished);
    assert!(
        !kura.disk_usage_total_initialized.load(Ordering::Relaxed),
        "an unfinished mutation must invalidate the total cache"
    );
    assert!(
        !kura.disk_usage_initialized.load(Ordering::Relaxed),
        "an unfinished mutation must invalidate the enforced cache"
    );
    let exact_after_unfinished = kura
        .kura_total_disk_usage_bytes()
        .expect("rescan unfinished replacement");
    let exact_enforced_after_unfinished = kura
        .kura_disk_usage_bytes()
        .expect("rescan enforced usage after unfinished replacement");
    assert_eq!(exact_after_unfinished, exact_total.saturating_add(2));
    assert_eq!(
        kura.refresh_disk_usage_bytes()
            .expect("invalidated enforced cache must refresh on demand"),
        exact_enforced_after_unfinished
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("combined refresh must restore the total cache"),
        exact_after_unfinished
    );
    assert!(kura.disk_usage_initialized.load(Ordering::Relaxed));
    assert!(kura.disk_usage_total_initialized.load(Ordering::Relaxed));
}
#[test]
fn cached_total_usage_read_waits_for_in_flight_mutation_publication() {
    let (temp_dir, config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
    kura.refresh_disk_usage_bytes()
        .expect("establish exact disk-usage baseline");
    let baseline_total = kura
        .disk_usage_bytes()
        .expect("read exact total-usage baseline");
    let journal_path = kura
        .store_root()
        .join(crate::query::index_status::QueryIndexJournal::JOURNAL_FILE);
    assert!(!journal_path.exists());
    let accounting_mutation = kura.begin_total_disk_usage_mutation();
    std::fs::write(&journal_path, [0xA5_u8; 17]).expect("write an in-flight counted journal");
    let reader_kura = Arc::clone(&kura);
    let (reader_tx, reader_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        reader_tx
            .send(reader_kura.disk_usage_bytes())
            .expect("report cached total-usage read");
    });
    assert!(
        matches!(
            reader_rx.recv_timeout(Duration::from_millis(50)),
            Err(RecvTimeoutError::Timeout)
        ),
        "a cached total read must not observe an in-flight filesystem mutation"
    );
    kura.update_disk_usage_delta(0, 17);
    accounting_mutation.finish();
    let observed = reader_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("cached reader must resume after publication")
        .expect("cached total-usage read succeeds");
    reader.join().expect("join cached total-usage reader");
    assert_eq!(observed, baseline_total.saturating_add(17));
    assert_eq!(
        observed,
        kura.kura_total_disk_usage_bytes()
            .expect("scan exact usage after publication")
    );
}
#[test]
fn total_only_refresh_invalidates_cached_total_on_scan_error() {
    let (temp_dir, config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
    kura.refresh_disk_usage_bytes()
        .expect("establish exact disk-usage baseline");
    let cached_total = kura.disk_usage_total.load(Ordering::Relaxed);
    let cached_enforced = kura.disk_usage.load(Ordering::Relaxed);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let invalid_total_only_directory = Kura::retained_block_rewrite_staging_dir_for(&blocks_dir);
    std::fs::write(&invalid_total_only_directory, b"not a directory")
        .expect("plant invalid total-only directory path");
    assert!(kura.refresh_total_disk_usage_bytes().is_err());
    assert!(
        !kura.disk_usage_total_initialized.load(Ordering::Relaxed),
        "a failed total-only scan must invalidate the old total cache"
    );
    assert!(
        kura.disk_usage_initialized.load(Ordering::Relaxed),
        "a total-only scan failure must not discard an unchanged enforced cache"
    );
    assert_eq!(kura.disk_usage_total.load(Ordering::Relaxed), cached_total);
    assert_eq!(kura.disk_usage.load(Ordering::Relaxed), cached_enforced);
    std::fs::remove_file(&invalid_total_only_directory).expect("remove invalid total-only path");
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("invalidated total cache refreshes on demand"),
        kura.kura_total_disk_usage_bytes()
            .expect("scan exact total usage after recovery")
    );
    assert!(kura.disk_usage_total_initialized.load(Ordering::Relaxed));
}
#[test]
fn partial_retired_tree_deletion_invalidates_caches_before_returning() {
    let (temp_dir, config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
    let retired_merge = temp_dir.path().join("retired/merge_ledger");
    std::fs::create_dir_all(&retired_merge).expect("create retired merge tree");
    std::fs::write(retired_merge.join("a.merge"), [0x11_u8; 5])
        .expect("write first retired segment");
    std::fs::write(retired_merge.join("b.merge"), [0x22_u8; 7])
        .expect("write second retired segment");
    let enforced_before = kura
        .refresh_disk_usage_bytes()
        .expect("establish exact pre-purge usage");
    let total_before = kura
        .disk_usage_bytes()
        .expect("read exact pre-purge total usage");
    kura.fail_next_retired_tree_purge_after_one_removal_for_tests();
    assert!(
        !kura.purge_retired_segments().unwrap(),
        "a partially failed purge must not claim a completed retired-tree removal"
    );
    assert_eq!(
        std::fs::read_dir(&retired_merge)
            .expect("read partially purged tree")
            .count(),
        1,
        "the failure injection must remove exactly one file"
    );
    let usage = kura
        .disk_usage_accounting_snapshot_for_tests()
        .expect("inspect raw caches after partial deletion");
    assert!(!usage.enforced_initialized);
    assert!(!usage.total_initialized);
    assert!(usage.exact_enforced_bytes < enforced_before);
    assert!(usage.exact_total_bytes < total_before);
    assert_eq!(
        kura.refresh_disk_usage_bytes()
            .expect("invalidated caches must rescan the partial deletion"),
        usage.exact_enforced_bytes
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("total cache must match the partial-deletion rescan"),
        usage.exact_total_bytes
    );
}
#[test]
fn combined_disk_usage_refresh_invalidates_both_caches_on_total_scan_error() {
    let (temp_dir, config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
    let enforced_before = kura
        .refresh_disk_usage_bytes()
        .expect("establish enforced baseline");
    assert!(kura.durable_budget_snapshot().is_some());
    let total_before = kura.disk_usage_total.load(Ordering::Relaxed);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let invalid_total_only_directory = Kura::retained_block_rewrite_staging_dir_for(&blocks_dir);
    std::fs::write(&invalid_total_only_directory, b"not a directory")
        .expect("plant invalid total-only directory path");
    assert!(kura.refresh_disk_usage_bytes().is_err());
    assert!(!kura.disk_usage_initialized.load(Ordering::Relaxed));
    assert!(!kura.disk_usage_total_initialized.load(Ordering::Relaxed));
    assert!(
        kura.durable_budget_snapshot().is_none(),
        "a failed combined scan must invalidate the durable-budget snapshot"
    );
    assert_eq!(kura.disk_usage.load(Ordering::Relaxed), enforced_before);
    assert_eq!(
        kura.disk_usage_total.load(Ordering::Relaxed),
        total_before,
        "a failed combined scan must not partially publish either counter"
    );
    std::fs::remove_file(&invalid_total_only_directory).expect("remove invalid total-only path");
    kura.refresh_disk_usage_bytes()
        .expect("combined refresh recovers after invalid path removal");
    assert!(kura.disk_usage_initialized.load(Ordering::Relaxed));
    assert!(kura.disk_usage_total_initialized.load(Ordering::Relaxed));
}
#[test]
fn retired_geometry_evidence_is_accounted_and_never_legacy_purged() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let baseline = kura.refresh_disk_usage_bytes().expect("baseline usage");
    let geometry_file = temp_dir
        .path()
        .join("retired/lane_geometry/transition/lane_0000000001/blocks/data.norito");
    std::fs::create_dir_all(geometry_file.parent().expect("geometry parent"))
        .expect("create geometry archive");
    std::fs::write(&geometry_file, [0u8; 7]).expect("write geometry evidence");
    let geometry_journal = kura.lane_geometry_journal_path();
    assert!(
        !geometry_journal.exists(),
        "fresh Kura must not already have a geometry journal"
    );
    std::fs::write(&geometry_journal, [0u8; 5]).expect("write geometry journal");
    let with_evidence = kura
        .refresh_disk_usage_bytes()
        .expect("usage with geometry evidence");
    assert_eq!(with_evidence, baseline.saturating_add(12));
    assert!(
        !kura.purge_retired_segments().unwrap(),
        "geometry evidence alone is not disposable retired storage"
    );
    assert_eq!(
        std::fs::read(&geometry_file).expect("geometry evidence retained"),
        [0u8; 7]
    );
    assert!(geometry_journal.exists(), "geometry journal retained");
    let retired_root = temp_dir.path().join("retired");
    let retired_blocks = RuntimeLaneConfig::default()
        .primary()
        .blocks_dir(&retired_root);
    std::fs::create_dir_all(&retired_blocks).expect("create legacy retired blocks");
    std::fs::write(retired_blocks.join(DATA_FILE_NAME), [0u8; 3])
        .expect("write legacy retired block bytes");
    assert_eq!(
        kura.refresh_disk_usage_bytes()
            .expect("usage with legacy retired bytes"),
        baseline.saturating_add(15)
    );
    assert!(
        kura.purge_retired_segments().unwrap(),
        "legacy retired blocks should remain purgeable"
    );
    assert!(!retired_root.join("blocks").exists());
    assert!(
        geometry_file.exists(),
        "geometry archive must survive purge"
    );
    assert!(
        geometry_journal.exists(),
        "geometry journal must survive purge"
    );
    assert_eq!(
        kura.refresh_disk_usage_bytes()
            .expect("usage after legacy purge"),
        baseline.saturating_add(12)
    );
}
#[test]
fn store_block_rejects_when_other_lane_storage_exceeds_budget() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().to_path_buf();
    let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::from(1),
        alias: "beta".to_string(),
        ..ModelLaneConfig::default()
    };
    let catalog = LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("lane catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let block = DummyBlocks::new().next();
    let budget_limit = Kura::block_required_bytes(&block).expect("block bytes");
    let kura_cfg = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (mut kura, _) = Kura::new(&kura_cfg, &lane_config).expect("initialize kura");
    let association_stage_required = kura
        .canonical_association_stage_additional_bytes(block.as_ref(), None)
        .expect("account canonical association stage");
    let exact_limit = canonical_storage_budget_base_for_test(&kura)
        .saturating_add(budget_limit)
        .saturating_add(association_stage_required);
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = exact_limit;
    let lane1_entry = lane_config.entry(LaneId::from(1)).expect("lane 1 entry");
    let lane1_blocks = lane1_entry.blocks_dir(&store_root);
    std::fs::write(lane1_blocks.join(DATA_FILE_NAME), [0u8; 1]).expect("seed lane1 data");
    kura.refresh_disk_usage_bytes()
        .expect("refresh disk usage after lane1 seed");
    let err = kura
        .store_block(block)
        .expect_err("lane 1 bytes should exceed budget");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
}
#[test]
fn store_block_reclaims_retired_storage_when_budget_exceeded() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let block = DummyBlocks::new().next();
    let budget_limit = Kura::block_required_bytes(&block).expect("block bytes");
    let kura_cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("initialize kura");
    let baseline = canonical_storage_budget_base_for_test(&kura);
    let association_stage_required = kura
        .canonical_association_stage_additional_bytes(block.as_ref(), None)
        .expect("account canonical association stage");
    Arc::get_mut(&mut kura)
        .expect("exclusive kura handle")
        .max_disk_usage_bytes = baseline
        .saturating_add(budget_limit)
        .saturating_add(association_stage_required);
    let retired_root = temp_dir.path().join("retired");
    let lane_cfg = RuntimeLaneConfig::default();
    let retired_dir = lane_cfg.primary().blocks_dir(&retired_root);
    std::fs::create_dir_all(&retired_dir).expect("create retired dir");
    std::fs::write(retired_dir.join(DATA_FILE_NAME), [0u8; 1]).expect("seed retired file");
    kura.refresh_disk_usage_bytes()
        .expect("refresh disk usage after retired seed");
    kura.store_block(block)
        .expect("store block after retired purge");
    assert!(
        !temp_dir.path().join("retired").exists(),
        "retired storage should be purged"
    );
}
#[test]
fn store_block_with_merge_entry_repairs_post_commit_append_failure_on_exact_retry() {
    let (temp_dir, config) = kura_storage_fixture("create Kura root", BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("open persistent Kura");
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    let block_hash = block.hash();
    let entry_hash = entry.canonical_hash();
    kura.store_block(parent).expect("store carrier parent");
    kura.fail_next_merge_append_for_test();
    let err = kura
        .store_block_with_merge_entry(Arc::clone(&block), &entry)
        .expect_err("merge log append should fail");
    assert!(matches!(
        err,
        Error::CanonicalBlockCommittedRecoveryRequired { .. }
    ));
    assert!(err.requires_restart_recovery());
    assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert_eq!(
        Kura::read_durable_hash_at_height(&mut kura.block_store.lock(), 2)
            .expect("read committed carrier hash while poisoned"),
        Some(block_hash),
        "the block fsync is the irreversible Kura commit point"
    );
    assert!(kura.merge_ledger_snapshot().is_empty());
    assert_eq!(
        kura.merge_carrier_for_entry(entry_hash)
            .expect("carrier index remains readable"),
        None
    );
    assert_eq!(
        kura.pending_certified_merge_entries()
            .expect("read retained exact sidecar"),
        vec![(entry_hash, entry.clone())]
    );
    assert!(matches!(
        kura.store_block_with_merge_entry(Arc::clone(&block), &entry),
        Err(Error::CanonicalStoragePoisoned)
    ));
    drop(kura);
    let (kura, BlockCount(count)) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("restart repairs the committed association");
    assert_eq!(count, 2);
    assert_eq!(kura.merge_ledger_snapshot(), vec![entry.clone()]);
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    assert_eq!(
        kura.merge_carrier_for_entry(entry_hash)
            .expect("read repaired carrier")
            .map(|record| record.block_hash),
        Some(block_hash)
    );
    assert!(
        kura.pending_certified_merge_entries()
            .expect("read pending store after retry")
            .is_empty()
    );
    kura.store_block_with_merge_entry(block, &entry)
        .expect("exact post-recovery retry is idempotent");
    {
        let index = kura.transaction_entrypoint_index.lock();
        assert!(
            index.complete,
            "the combined ordinary/merge index becomes complete only after repair"
        );
        assert!(index.incomplete_merge_heights.is_empty());
    }
}
#[test]
fn merge_append_boundary_failures_recover_for_retry_and_reopen() {
    let failure_points = [
        MergeLedgerAppendFailurePoint::AfterLength,
        MergeLedgerAppendFailurePoint::AfterPayload,
        MergeLedgerAppendFailurePoint::AfterSync,
    ];
    for retry_before_reopen in [false, true] {
        for failure_point in failure_points {
            let (temp_dir, config) = kura_storage_fixture("temporary Kura root", BLOCKS_IN_MEMORY);
            let (kura, _) =
                Kura::new(&config, &RuntimeLaneConfig::default()).expect("open persistent Kura");
            let mut blocks = DummyBlocks::new();
            let parent = blocks.next();
            let mut entry = sample_merge_entry(1);
            let block = next_merge_carrier(&mut blocks, &mut entry);
            let block_hash = block.hash();
            let entry_hash = entry.canonical_hash();
            kura.store_block(parent).expect("store carrier parent");
            kura.fail_next_merge_append_after_for_test(failure_point);
            let error = kura
                .store_block_with_merge_entry(Arc::clone(&block), &entry)
                .expect_err("in-place merge append boundary must report failure");
            assert!(
                matches!(error, Error::CanonicalBlockCommittedRecoveryRequired { .. }),
                "{failure_point:?} must fail-stop after the canonical marker commits: {error}"
            );
            assert!(error.requires_restart_recovery());
            assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
            assert_eq!(
                Kura::read_durable_hash_at_height(&mut kura.block_store.lock(), 2)
                    .expect("read committed carrier hash while poisoned"),
                Some(block_hash)
            );
            assert_eq!(
                kura.merge_log_tracked_bytes()
                    .expect("measure recovered merge log tail"),
                0,
                "{failure_point:?} must roll the reported append back to its exact frame offset"
            );
            assert!(
                kura.pending_merge_entry_path(entry_hash).is_file(),
                "the exact recovery sidecar must remain durable"
            );
            if retry_before_reopen {
                assert!(matches!(
                    kura.store_block_with_merge_entry(Arc::clone(&block), &entry),
                    Err(Error::CanonicalStoragePoisoned)
                ));
            }
            drop(kura);
            let (reopened, BlockCount(block_count)) =
                Kura::new(&config, &RuntimeLaneConfig::default())
                    .expect("reopen and repair the exact append boundary");
            assert_eq!(block_count, 2);
            assert_eq!(
                reopened
                    .merge_ledger_all_entries()
                    .expect("read recovered merge history"),
                vec![entry.clone()],
                "{failure_point:?} retry_before_reopen={retry_before_reopen} must leave exactly one frame"
            );
            let _ = persist_v2_finality_chain_through(&reopened, nonzero!(2_usize));
            assert_eq!(
                reopened
                    .merge_carrier_for_entry(entry_hash)
                    .expect("read recovered sparse carrier"),
                Some(MergeLedgerCarrierRecord::new(&entry, &block))
            );
            assert!(
                reopened
                    .pending_certified_merge_entries()
                    .expect("read recovered pending store")
                    .is_empty()
            );
        }
    }
}
#[test]
fn startup_repairs_each_block_first_merge_publication_crash_window() {
    for published_merge_parts in 0_u8..=2 {
        let (temp_dir, config) = kura_storage_fixture("temporary Kura root", BLOCKS_IN_MEMORY);
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let mut blocks = DummyBlocks::new();
        let parent = blocks.next();
        let mut entry = sample_merge_entry(1);
        let block = next_merge_carrier(&mut blocks, &mut entry);
        let block_hash = block.hash();
        kura.store_block(parent).expect("store carrier parent");
        kura.persist_pending_certified_merge_entry(&entry)
            .expect("stage exact recovery sidecar");
        let write_guard = kura.lock_block_store_for_write();
        kura.persist_block_at_height_while_locked(&block, 2, &write_guard)
            .expect("simulate canonical block commit before in-memory publication");
        drop(write_guard);
        if published_merge_parts >= 1 {
            kura.append_merge_entry(&entry)
                .expect("simulate merge-log publication");
        }
        if published_merge_parts >= 2 {
            let record = MergeLedgerCarrierRecord::new(&entry, &block);
            let _guard = kura.merge_carrier_lock.lock();
            kura.write_merge_carrier_record_unlocked(record)
                .expect("simulate sparse-carrier publication");
        }
        drop(kura);
        let (reopened, BlockCount(block_count)) =
            Kura::new(&config, &RuntimeLaneConfig::default()).expect("repair crash image");
        assert_eq!(block_count, 2);
        assert_eq!(
            reopened.get_durable_block_hash(nonzero!(2_usize)),
            Some(block_hash)
        );
        assert_eq!(
            reopened
                .merge_ledger_all_entries()
                .expect("read repaired merge log"),
            vec![entry.clone()],
            "restart must leave exactly one merge frame at crash window {published_merge_parts}"
        );
        let _ = persist_v2_finality_chain_through(&reopened, nonzero!(2_usize));
        assert_eq!(
            reopened
                .merge_carrier_records()
                .expect("read repaired carrier index"),
            vec![MergeLedgerCarrierRecord::new(&entry, &block)],
            "restart must leave exactly one carrier at crash window {published_merge_parts}"
        );
        assert!(
            reopened
                .pending_certified_merge_entries()
                .expect("read repaired pending store")
                .is_empty(),
            "restart must retire the exact pending sidecar at crash window {published_merge_parts}"
        );
        let transaction_index = reopened.transaction_entrypoint_index.lock();
        assert!(transaction_index.complete);
        assert!(transaction_index.incomplete_merge_heights.is_empty());
        assert_eq!(transaction_index.indexed_heights.len(), 2);
        assert!(
            transaction_index
                .indexed_heights
                .contains(&nonzero!(1_usize))
        );
        assert!(
            transaction_index
                .indexed_heights
                .contains(&nonzero!(2_usize))
        );
    }
}
#[test]
fn merge_log_truncated_when_block_store_pruned() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = kura_config_for_dir(&dir, BLOCKS_IN_MEMORY);
    let lane_cfg = RuntimeLaneConfig::default();
    let merge_path = lane_cfg.primary().merge_log_path(dir.path());
    {
        let mut merge_log = MergeLedgerLog::open_at(&merge_path, MERGE_LEDGER_CACHE_CAPACITY)
            .expect("prepare merge log");
        merge_log
            .append(&sample_merge_entry(1))
            .expect("append first entry");
        merge_log
            .append(&sample_merge_entry(2))
            .expect("append second entry");
    }
    let (kura, block_count) = Kura::new(&config, &lane_cfg).expect("init kura");
    assert_eq!(block_count.0, 0);
    assert_eq!(kura.merge_ledger_snapshot().len(), 0);
    assert_eq!(
        fs::metadata(&merge_path).expect("merge log metadata").len(),
        0,
        "merge log should be truncated alongside empty block store"
    );
}
#[test]
fn merge_log_truncates_partial_tail_on_load() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_path = dir.path().join("merge.log");
    let entry1 = sample_merge_entry(1);
    {
        let mut merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
            .expect("open merge log");
        merge_log.append(&entry1).expect("append entry1");
    }
    let entry2 = sample_merge_entry(2);
    let encoded2 = Encode::encode(&entry2);
    let len2 = u32::try_from(encoded2.len()).expect("entry length fits in u32");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&log_path)
        .expect("open merge log for truncation");
    file.write_all(&len2.to_le_bytes())
        .expect("write partial length");
    let partial_len = encoded2.len() / 2;
    file.write_all(&encoded2[..partial_len])
        .expect("write partial payload");
    file.flush().expect("flush partial payload");
    let expected_len = 4 + Encode::encode(&entry1).len();
    let mut merge_log =
        MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY).expect("reopen merge log");
    let file_len = fs::metadata(&log_path).expect("merge log metadata").len();
    assert_eq!(file_len, expected_len as u64);
    let snapshot = merge_log.snapshot();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].epoch_id, entry1.epoch_id);
    let replacement_entry2 = sample_merge_entry(2);
    merge_log
        .append(&replacement_entry2)
        .expect("append replacement entry2");
    drop(merge_log);
    let merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
        .expect("reopen after append");
    let snapshot = merge_log.snapshot();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].epoch_id, entry1.epoch_id);
    assert_eq!(snapshot[1].epoch_id, replacement_entry2.epoch_id);
}
#[test]
fn merge_log_rejects_exact_decode_failure_on_load_without_mutation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_path = dir.path().join("merge.log");
    let entry1 = sample_merge_entry(1);
    let encoded1 = Encode::encode(&entry1);
    {
        let mut merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
            .expect("open merge log");
        merge_log.append(&entry1).expect("append entry1");
    }
    let corrupt = [0xFF_u8; 16];
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&log_path)
        .expect("open merge log for corruption");
    file.write_all(
        &u32::try_from(corrupt.len())
            .expect("corrupt fixture length fits u32")
            .to_le_bytes(),
    )
    .expect("write corrupt frame length");
    file.write_all(&corrupt).expect("write corrupt frame");
    file.sync_data().expect("sync corrupt frame");
    drop(file);
    let corrupt_len = 4 + corrupt.len();
    let exact_bytes = fs::read(&log_path).expect("read exact corrupt log image");
    let error = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
        .expect_err("a complete invalid frame must fail closed");
    assert!(matches!(error, Error::MergeCarrierConflict(_)));
    assert_eq!(
        fs::metadata(&log_path).expect("merge log metadata").len(),
        u64::try_from(4 + encoded1.len() + corrupt_len).expect("merge log length fits u64"),
        "a complete corrupt frame must not be mistaken for a torn tail"
    );
    assert!(matches!(
        error,
        Error::MergeCarrierConflict(ref message)
            if message.contains("failed exact Norito decode")
    ));
    assert_eq!(
        fs::read(&log_path).expect("read rejected corrupt log image"),
        exact_bytes,
        "startup rejection must retain every byte for operator recovery"
    );
}
#[test]
fn merge_log_rejects_unsupported_entry_version_without_mutation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_path = dir.path().join("merge.log");
    let entry1 = sample_merge_entry(1);
    {
        let mut merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
            .expect("open merge log");
        merge_log.append(&entry1).expect("append entry1");
    }
    let mut unsupported = sample_merge_entry(2);
    unsupported.version = MergeLedgerEntry::VERSION + 1;
    let unsupported_bytes = unsupported.encode();
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&log_path)
        .expect("open merge log for unsupported frame");
    file.write_all(
        &u32::try_from(unsupported_bytes.len())
            .expect("unsupported fixture length fits u32")
            .to_le_bytes(),
    )
    .expect("write unsupported frame length");
    file.write_all(&unsupported_bytes)
        .expect("write unsupported frame");
    file.sync_data().expect("sync unsupported frame");
    drop(file);
    let exact_bytes = fs::read(&log_path).expect("read exact unsupported log image");
    let error = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
        .expect_err("a complete unsupported-version frame must fail closed");
    assert!(matches!(
        error,
        Error::MergeCarrierConflict(ref message)
            if message.contains("unsupported merge ledger entry version")
    ));
    assert_eq!(
        fs::read(&log_path).expect("read rejected unsupported log image"),
        exact_bytes,
        "startup rejection must retain every byte for operator recovery"
    );
}
#[test]
fn merge_log_rejects_oversized_frame_on_load_without_mutation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_path = dir.path().join("merge.log");
    let entry1 = sample_merge_entry(1);
    {
        let mut merge_log = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
            .expect("open merge log");
        merge_log.append(&entry1).expect("append entry1");
    }
    let oversize_len =
        u32::try_from(MAX_MERGE_LEDGER_ENTRY_BYTES + 1).expect("max entry size fits in u32");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&log_path)
        .expect("open merge log for oversize");
    file.write_all(&oversize_len.to_le_bytes())
        .expect("write oversize length");
    file.write_all(&[0u8; 8]).expect("write stub payload");
    file.sync_data().expect("sync oversize frame");
    drop(file);
    let exact_bytes = fs::read(&log_path).expect("read exact oversized log image");
    let error = MergeLedgerLog::open_at(&log_path, MERGE_LEDGER_CACHE_CAPACITY)
        .expect_err("an oversized frame declaration must fail closed");
    assert!(matches!(
        error,
        Error::MergeCarrierConflict(ref message)
            if message.contains("frame length") && message.contains("exceeds")
    ));
    assert_eq!(
        fs::read(&log_path).expect("read rejected oversized log image"),
        exact_bytes,
        "startup rejection must not truncate an adversarial frame declaration"
    );
}
#[test]
fn merge_log_rejects_oversized_entry() {
    let mut merge_log = MergeLedgerLog::in_memory(MERGE_LEDGER_CACHE_CAPACITY);
    let mut entry = sample_merge_entry(1);
    entry.merge_qc.aggregate_signature = vec![0u8; MAX_MERGE_LEDGER_ENTRY_BYTES];
    let err = merge_log
        .append(&entry)
        .expect_err("oversized merge entry should error");
    assert!(matches!(err, Error::NoritoFrame(_)));
}
#[test]
fn merge_log_respects_cache_capacity() {
    let kura = Kura::blank_kura_for_testing();
    *kura.merge_log.lock() = MergeLedgerLog::in_memory(2);
    kura.append_merge_entry(&sample_merge_entry(1))
        .expect("append entry1");
    kura.append_merge_entry(&sample_merge_entry(2))
        .expect("append entry2");
    kura.append_merge_entry(&sample_merge_entry(3))
        .expect("append entry3");
    let snapshot = kura.merge_ledger_snapshot();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].epoch_id, 2);
    assert_eq!(snapshot[1].epoch_id, 3);
    assert_eq!(
        kura.merge_ledger_latest_snapshot(1)
            .iter()
            .map(|entry| entry.epoch_id)
            .collect::<Vec<_>>(),
        vec![3],
        "bounded diagnostics must clone only the newest requested suffix"
    );
    assert_eq!(
        kura.merge_ledger_latest_snapshot(2)
            .iter()
            .map(|entry| entry.epoch_id)
            .collect::<Vec<_>>(),
        vec![3, 2],
        "bounded diagnostics suffixes are newest-first and deterministic"
    );
    assert!(kura.merge_ledger_latest_snapshot(0).is_empty());
}
#[test]
fn merge_log_append_rejects_canonical_storage_poison_without_effects() {
    let kura = Kura::blank_kura_for_testing();
    kura.canonical_storage_poisoned
        .store(true, Ordering::Release);
    assert!(matches!(
        kura.append_merge_entry(&sample_merge_entry(1)),
        Err(Error::CanonicalStoragePoisoned)
    ));
    assert!(
        kura.merge_ledger_snapshot().is_empty(),
        "fail-stop rejection must not append an in-memory merge entry"
    );
}
#[test]
fn store_block_does_not_depend_on_writer_channel() {
    let kura = Kura::blank_kura_for_testing();
    kura.block_notify_rx.lock().take();
    let block = DummyBlocks::new().next();
    kura.store_block(block).expect("store block");
    assert_eq!(kura.blocks_count(), 1);
}
#[test]
fn store_block_does_not_depend_on_writer_fault() {
    let kura = Kura::blank_kura_for_testing();
    kura.record_writer_fault("test", &Error::BlockWriterUnavailable);
    let block = DummyBlocks::new().next();
    kura.store_block(block).expect("store block");
    assert_eq!(kura.blocks_count(), 1);
}
#[test]
fn store_block_treats_readable_new_marker_after_ack_failure_as_committed() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = FsyncMode::Batched;
    config.fsync_interval = Duration::from_secs(60);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
    kura.block_store
        .lock()
        .fail_next_commit_marker_ack_after_persist
        .store(true, Ordering::Release);
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block))
        .expect("readable new marker is committed success");
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(block.hash())
    );
    assert_eq!(
        kura.get_block(nonzero!(1_usize)).as_deref(),
        Some(block.as_ref())
    );
    assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
}
#[test]
fn pre_marker_rewrite_failure_preserves_exact_retry_without_poison() {
    let (temp_dir, config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
    let original = DummyBlocks::new().next();
    let original_hash = original.hash();
    kura.store_block(Arc::clone(&original))
        .expect("store original block");
    let replacement: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into(),
    );
    let replacement_hash = replacement.hash();
    assert_ne!(replacement_hash, original_hash);
    kura.block_store
        .lock()
        .fail_next_da_rewrite_before_marker
        .store(true, Ordering::Release);
    let error = kura
        .replace_top_block(Arc::clone(&replacement))
        .expect_err("pre-marker rewrite fault must reject the replacement");
    assert!(matches!(error, Error::IO(_, _)));
    assert!(!error.requires_restart_recovery());
    assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert_eq!(kura.get_block_hash(nonzero!(1_usize)), Some(original_hash));
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(original_hash)
    );
    kura.replace_top_block(Arc::clone(&replacement))
        .expect("the exact replacement must remain retryable");
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(replacement_hash)
    );
    assert_eq!(
        kura.get_block(nonzero!(1_usize)).as_deref(),
        Some(replacement.as_ref())
    );
}
#[test]
fn pre_marker_association_recovery_failure_remains_exactly_retryable() {
    let (temp_dir, config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
    let stage_path = kura.canonical_association_stage_path();
    fs::create_dir(&stage_path).expect("plant invalid pre-marker association stage");
    let block = DummyBlocks::new().next();
    let error = kura
        .store_block(Arc::clone(&block))
        .expect_err("invalid pre-marker stage must reject mutation");
    assert!(matches!(error, Error::IO(_, _)));
    assert!(!error.requires_restart_recovery());
    assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert_eq!(kura.blocks_count(), 0);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 0);
    fs::remove_dir(&stage_path).expect("remove invalid pre-marker stage");
    kura.store_block(Arc::clone(&block))
        .expect("same block must remain exactly retryable");
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(block.hash())
    );
}
#[test]
fn post_marker_rewrite_recovery_failure_poison_gates_until_restart() {
    let (temp_dir, config) = kura_storage_fixture("create Kura root", BLOCKS_IN_MEMORY);
    let replacement = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let original = DummyBlocks::new().next();
        let original_hash = original.hash();
        kura.store_block(Arc::clone(&original))
            .expect("store original block");
        let replacement: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(1_u64));
                header.set_prev_block_hash(None);
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into(),
        );
        let replacement_hash = replacement.hash();
        {
            let store = kura.block_store.lock();
            store
                .fail_next_da_rewrite_after_marker
                .store(true, Ordering::Release);
            store
                .fail_next_da_rewrite_recovery
                .store(true, Ordering::Release);
        }
        let error = kura
            .replace_top_block(Arc::clone(&replacement))
            .expect_err("committed rewrite with failed promotion must require restart");
        assert!(matches!(
            error,
            Error::CanonicalBlockCommittedRecoveryRequired { .. }
        ));
        assert!(error.requires_restart_recovery());
        assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
        assert_eq!(
            kura.block_data.lock().first().map(|(hash, _)| *hash),
            Some(original_hash),
            "the live in-memory image must not pretend the replacement was published"
        );
        assert_eq!(
            Kura::read_durable_hash_at_height(&mut kura.block_store.lock(), 1)
                .expect("read committed replacement hash while poisoned"),
            Some(replacement_hash),
            "the new marker must remain the durable recovery authority"
        );
        assert!(matches!(
            kura.replace_top_block(Arc::clone(&replacement)),
            Err(Error::CanonicalStoragePoisoned)
        ));
        replacement
    };
    let (reopened, count) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("restart must complete the marker-selected replacement");
    assert_eq!(count.0, 1);
    assert_eq!(
        reopened.get_durable_block_hash(nonzero!(1_usize)),
        Some(replacement.hash())
    );
    assert_eq!(
        reopened.get_block(nonzero!(1_usize)).as_deref(),
        Some(replacement.as_ref())
    );
    assert!(!reopened.canonical_storage_poisoned.load(Ordering::Acquire));
}
#[test]
fn unreadable_append_marker_state_poison_gates_live_kura_and_restart_rolls_back() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = FsyncMode::Batched;
    config.fsync_interval = Duration::from_secs(60);
    {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        kura.block_store
            .lock()
            .fail_next_commit_marker_write_and_readback
            .store(true, Ordering::Release);
        let block = DummyBlocks::new().next();
        assert!(matches!(
            kura.store_block(block),
            Err(Error::DaBlockRewriteCommitStateUnknown { .. })
        ));
        assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
        assert!(kura.get_block(nonzero!(1_usize)).is_none());
        assert!(matches!(
            kura.store_block(DummyBlocks::new().next()),
            Err(Error::CanonicalStoragePoisoned)
        ));
    }
    let (reopened, count) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("old marker prunes the ambiguous unpublished append on restart");
    assert_eq!(count.0, 0);
    assert_eq!(reopened.blocks_count(), 0);
}
#[test]
fn unknown_marker_resolution_applies_or_discards_lane_association_stage() {
    let lane_id = LaneId::SINGLE;
    let lane_block_height = 1;
    for new_marker_won in [false, true] {
        let temp_dir = TempDir::new().expect("create Kura root");
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.fsync_mode = FsyncMode::Batched;
        config.fsync_interval = Duration::from_secs(60);
        {
            let (kura, _) =
                test_kura_with_default_lane_markers(&config, &RuntimeLaneConfig::default());
            let block = dummy_block_with_lane_payload_ownership(
                lane_id,
                DataSpaceId::UNIVERSAL,
                lane_block_height,
            );
            let store = kura.block_store.lock();
            if new_marker_won {
                store
                    .fail_next_commit_marker_ack_and_readback
                    .store(true, Ordering::Release);
            } else {
                store
                    .fail_next_commit_marker_write_and_readback
                    .store(true, Ordering::Release);
            }
            drop(store);
            assert!(matches!(
                kura.store_block(block),
                Err(Error::DaBlockRewriteCommitStateUnknown { .. })
            ));
            assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
            assert!(kura.block_data.lock().is_empty());
            assert!(kura.block_height_index.lock().is_empty());
            let transaction_index = kura.transaction_entrypoint_index.lock();
            assert!(transaction_index.indexed_heights.is_empty());
            assert!(transaction_index.incomplete_merge_heights.is_empty());
            assert!(transaction_index.heights_by_entrypoint.is_empty());
            drop(transaction_index);
            assert_eq!(
                kura.durable_budget_persisted_count.load(Ordering::Acquire),
                0,
                "fatal publication must not advance process-local budget metadata"
            );
            assert_eq!(
                kura.block_store
                    .lock()
                    .read_commit_marker()
                    .expect("read selected marker")
                    .expect("marker exists")
                    .count,
                u64::from(new_marker_won),
                "the durable marker, not process-local metadata, selects restart recovery"
            );
            assert!(kura.canonical_association_stage_path().is_file());
            assert!(
                kura.read_lane_block_artifact(lane_id, lane_block_height)
                    .is_none(),
                "lane association must wait for marker resolution"
            );
        }
        let (reopened, count) = Kura::new(&config, &RuntimeLaneConfig::default())
            .expect("startup resolves lane association stage by marker");
        assert_eq!(count.0, usize::from(new_marker_won));
        assert_eq!(
            reopened
                .read_lane_block_artifact(lane_id, lane_block_height)
                .is_some(),
            new_marker_won
        );
        assert!(!reopened.canonical_association_stage_path().exists());
    }
}
