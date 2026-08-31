#[test]
fn sidecar_index_rejects_overflowing_based_header() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().join(LANE_ARTIFACTS_DATA_FILE);
    let index_path = temp_dir.path().join(LANE_ARTIFACTS_INDEX_FILE);
    fs::write(&data_path, []).expect("create sidecar data");
    let mut index_bytes = SidecarIndexLayout::base_header(u64::MAX - 1).to_vec();
    let empty_entry = SidecarIndexEntry { offset: 0, len: 0 }.to_bytes();
    index_bytes.extend_from_slice(&empty_entry);
    index_bytes.extend_from_slice(&empty_entry);
    fs::write(&index_path, &index_bytes).expect("write overflowing based index");
    let payload = norito::to_bytes(&DummySidecar {
        height: u64::MAX - 1,
    })
    .expect("encode sidecar");
    assert!(
        Kura::indexed_sidecar_height_range(&index_path, "dummy lane sidecar").is_none(),
        "overflowing based indexes must fail closed during scans"
    );
    assert!(!Kura::append_indexed_sidecar(
        &data_path,
        &index_path,
        u64::MAX - 1,
        &payload,
        "dummy lane sidecar",
        FsyncMode::Batched,
        None,
    ));
    assert_eq!(
        fs::read(&index_path).expect("read rejected index"),
        index_bytes,
        "rejecting an overflowing header must not rewrite it"
    );
}
#[test]
fn sidecar_index_rejects_corrupt_base_header_checksum() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().join(LANE_ARTIFACTS_DATA_FILE);
    let index_path = temp_dir.path().join(LANE_ARTIFACTS_INDEX_FILE);
    fs::write(&data_path, []).expect("create sidecar data");
    let mut index_bytes = SidecarIndexLayout::base_header(10_000).to_vec();
    index_bytes[INDEXED_SIDECAR_BASE_HEADER_SIZE - 1] ^= 0x01;
    index_bytes.extend_from_slice(&SidecarIndexEntry { offset: 0, len: 0 }.to_bytes());
    fs::write(&index_path, &index_bytes).expect("write corrupt based index");
    let payload = norito::to_bytes(&DummySidecar { height: 10_000 }).expect("encode sidecar");
    assert!(
        Kura::read_indexed_sidecar_from_paths(
            10_000,
            &data_path,
            &index_path,
            norito::decode_from_bytes::<DummySidecar>,
            "dummy lane sidecar",
        )
        .is_none()
    );
    assert!(!Kura::append_indexed_sidecar(
        &data_path,
        &index_path,
        10_000,
        &payload,
        "dummy lane sidecar",
        FsyncMode::Batched,
        None,
    ));
    assert_eq!(fs::read(&index_path).expect("read index"), index_bytes);
}
#[test]
fn indexed_sidecars_prune_to_retention() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
    let retention = NonZeroUsize::new(2).expect("non-zero retention");
    for height in 1..=4 {
        let payload = norito::to_bytes(&DummySidecar { height }).expect("encode dummy sidecar");
        assert!(
            Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                height,
                &payload,
                "dummy sidecar",
                FsyncMode::Batched,
                Some(retention),
            ),
            "append at height {height} must succeed"
        );
    }
    let mut index = std::fs::File::open(&index_path).expect("index exists");
    let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
    index
        .seek(SeekFrom::Start(INDEXED_SIDECAR_BASE_HEADER_SIZE_U64))
        .expect("seek past V1 header");
    let mut entries = Vec::new();
    for _ in 0..4 {
        index.read_exact(&mut buf).expect("read index entry");
        entries.push(SidecarIndexEntry::from_bytes(buf));
    }
    assert_eq!(entries[0].len, 0);
    assert_eq!(entries[1].len, 0);
    assert!(entries[2].len > 0);
    assert!(entries[3].len > 0);
    assert_eq!(entries[2].offset, 0);
    assert_eq!(entries[3].offset, entries[2].len);
    let mut data = std::fs::File::open(&data_path).expect("data exists");
    for (idx, expected_height) in [3_u64, 4_u64].into_iter().enumerate() {
        let entry = &entries[idx + 2];
        let len = usize::try_from(entry.len).expect("len fits in usize");
        let mut payload = vec![0u8; len];
        data.seek(SeekFrom::Start(entry.offset))
            .expect("seek to payload");
        data.read_exact(&mut payload).expect("read payload");
        let decoded: DummySidecar =
            norito::decode_from_bytes(&payload).expect("decode dummy sidecar");
        assert_eq!(decoded.height, expected_height);
    }
}
#[test]
fn hashes_count_math() {
    let dir = TempDir::new().unwrap();
    let mut store = BlockStore::new(dir.path());
    store.create_files_if_they_do_not_exist().unwrap();
    // Fresh store: no hashes
    assert_eq!(store.read_hashes_count().unwrap(), 0);
    // Manually extend the hashes file to 3 full entries
    let path = dir.path().join(HASHES_FILE_NAME);
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(3 * SIZE_OF_BLOCK_HASH).unwrap();
    assert_eq!(store.read_hashes_count().unwrap(), 3);
    // Non-multiple of 32 is truncated by integer division
    file.set_len(2 * SIZE_OF_BLOCK_HASH + 16).unwrap();
    assert_eq!(store.read_hashes_count().unwrap(), 2);
}
#[test]
fn read_block_hashes_out_of_bounds() {
    let dir = TempDir::new().unwrap();
    let mut store = BlockStore::new(dir.path());
    store.create_files_if_they_do_not_exist().unwrap();
    // Prepare exactly 2 hash slots worth of data
    let path = dir.path().join(HASHES_FILE_NAME);
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(2 * SIZE_OF_BLOCK_HASH).unwrap();
    // Attempt to read 3 hashes from the start should be out of bounds
    let err = store.read_block_hashes(0, 3).unwrap_err();
    match err {
        Error::OutOfBoundsBlockRead {
            start_block_height,
            block_count,
        } => {
            assert_eq!(start_block_height, 0);
            assert_eq!(block_count, 3);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn read_block_indices_out_of_bounds() {
    let dir = TempDir::new().unwrap();
    let mut store = BlockStore::new(dir.path());
    store.create_files_if_they_do_not_exist().unwrap();
    // Prepare exactly 2 index entries worth of data
    let path = dir.path().join(INDEX_FILE_NAME);
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(2 * BlockIndex::SIZE).unwrap();
    let mut buf = vec![BlockIndex::default(); 3];
    let err = store.read_block_indices(0, &mut buf).unwrap_err();
    match err {
        Error::OutOfBoundsBlockRead {
            start_block_height,
            block_count,
        } => {
            assert_eq!(start_block_height, 0);
            assert_eq!(block_count, 3);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn strict_init_prunes_oversized_block_length() {
    let dir = TempDir::new().unwrap();
    let mut store = BlockStore::new(dir.path());
    store.create_files_if_they_do_not_exist().unwrap();
    let mut blocks = DummyBlocks::new();
    let block = blocks.next();
    store.append_block_to_chain(&block).unwrap();
    let BlockIndex { start, .. } = store.read_block_index(0).unwrap();
    let huge_len = STRICT_INIT_MAX_BLOCK_BYTES + 1;
    store.write_block_index(0, start, huge_len).unwrap();
    let validation = Kura::init_canonical_chain(&mut store, 1, None).unwrap();
    assert!(validation.truncated);
    assert!(validation.hashes.is_empty());
    assert_eq!(store.read_index_count().unwrap(), 0);
}
#[test]
fn strict_init_repairs_only_the_hash_suffix_above_v2_finality() {
    let dir = TempDir::new().unwrap();
    let mut store = new_block_store(&dir);
    store.create_files_if_they_do_not_exist().unwrap();
    let mut generator = DummyBlocks::new();
    let blocks = vec![generator.next(), generator.next(), generator.next()];
    for block in &blocks {
        store.append_block_to_chain(block).unwrap();
    }
    let hashes_path = primary_blocks_dir(&dir).join(HASHES_FILE_NAME);
    let finalized_prefix_len = usize::try_from(2 * SIZE_OF_BLOCK_HASH).unwrap();
    let pristine_bytes = std::fs::read(&hashes_path).expect("read pristine hash journal");
    let finalized_prefix = pristine_bytes[..finalized_prefix_len].to_vec();
    let forged = HashOf::from_untyped_unchecked(Hash::prehashed([0xD7; Hash::LENGTH]));
    assert_ne!(forged, blocks[2].hash());
    store
        .write_block_hash(2, forged)
        .expect("corrupt only the unfinalized hash suffix");
    let validation = Kura::init_canonical_chain(&mut store, 3, Some(2))
        .expect("repair above finality must succeed");
    assert!(validation.hash_mismatch);
    assert!(!validation.truncated);
    assert_eq!(
        validation.hashes,
        blocks.iter().map(|block| block.hash()).collect::<Vec<_>>()
    );
    let repaired_bytes = std::fs::read(&hashes_path).expect("read repaired hash journal");
    assert_eq!(
        &repaired_bytes[..finalized_prefix_len],
        finalized_prefix.as_slice(),
        "repairing an unfinalized suffix must leave finalized-prefix bytes untouched"
    );
    assert_eq!(repaired_bytes, pristine_bytes);
}
#[test]
fn strict_init_reconstructs_a_missing_hash_suffix_above_v2_finality() {
    let dir = TempDir::new().unwrap();
    let mut store = new_block_store(&dir);
    store.create_files_if_they_do_not_exist().unwrap();
    let mut generator = DummyBlocks::new();
    let blocks = vec![generator.next(), generator.next(), generator.next()];
    for block in &blocks {
        store.append_block_to_chain(block).unwrap();
    }
    let hashes_path = primary_blocks_dir(&dir).join(HASHES_FILE_NAME);
    let pristine_bytes = std::fs::read(&hashes_path).expect("read pristine hash journal");
    let finalized_prefix_len = usize::try_from(2 * SIZE_OF_BLOCK_HASH).unwrap();
    store
        .truncate_hashes_to_count(2)
        .expect("remove only the unfinalized hash suffix");
    let validation = Kura::init_canonical_chain(&mut store, 3, Some(2))
        .expect("reconstruct a missing suffix without rewriting finality");
    assert!(!validation.hash_mismatch);
    assert!(!validation.truncated);
    assert_eq!(
        validation.hashes,
        blocks.iter().map(|block| block.hash()).collect::<Vec<_>>()
    );
    let repaired_bytes = std::fs::read(&hashes_path).expect("read reconstructed journal");
    assert_eq!(
        &repaired_bytes[..finalized_prefix_len],
        &pristine_bytes[..finalized_prefix_len]
    );
    assert_eq!(repaired_bytes, pristine_bytes);
}
#[test]
fn hard_fork_data_backed_count_preserves_hash_only_tail() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = new_block_store(&temp_dir);
    store.create_files_if_they_do_not_exist().unwrap();
    let mut blocks = DummyBlocks::new();
    store.append_block_to_chain(&blocks.next()).unwrap();
    let tail = blocks.next();
    store.write_block_index(1, EVICTED_BLOCK_START, 0).unwrap();
    store.write_block_hash(1, tail.as_ref().hash()).unwrap();
    assert_eq!(
        store.data_backed_count(2, 2, None).unwrap(),
        1,
        "ordinary init must still treat a zero-length evicted tail as not data-backed"
    );
    assert_eq!(
        store.data_backed_count(2, 2, Some((0, 2))).unwrap(),
        2,
        "hard-fork snapshot bootstrap keeps hash-only tail metadata durable"
    );
}
#[test]
fn zero_length_hash_metadata_is_classified_as_hash_only_body_unavailable() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let first = blocks.next();
    let hash_only =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x41; Hash::LENGTH]));
    {
        let mut store = kura.block_store.lock();
        store.create_files_if_they_do_not_exist().unwrap();
        store.append_block_to_chain(&first).unwrap();
        store.write_block_index(1, EVICTED_BLOCK_START, 0).unwrap();
        store.write_block_hash(1, hash_only).unwrap();
    }
    {
        let mut data = kura.block_data.lock();
        data.clear();
        data.push((first.hash(), Some(first)));
        data.push((hash_only, None));
    }
    kura.hard_fork_hash_only_block_count
        .store(1, Ordering::Relaxed);
    assert!(kura.is_hash_only_block_height(nonzero!(2_usize)));
    assert!(kura.get_block(nonzero!(2_usize)).is_none());
}
#[test]
fn hash_only_snapshot_rejects_divergent_existing_prefix() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 3);
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, BlockCount(block_count)) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    assert_eq!(block_count, 3);
    let first_hash = kura
        .block_hash_at_height(nonzero!(1_usize))
        .expect("first hash exists");
    let snapshot_hash_2 =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x42; Hash::LENGTH]));
    let snapshot_hash_3 =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x43; Hash::LENGTH]));
    let snapshot_hash_4 =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x44; Hash::LENGTH]));
    let snapshot_hashes = vec![
        first_hash,
        snapshot_hash_2,
        snapshot_hash_3,
        snapshot_hash_4,
    ];
    assert!(matches!(
        kura.extend_hash_only_prefix_from_snapshot(&snapshot_hashes),
        Err(Error::BlockHeightConflict { height: 2, .. })
    ));
    assert_eq!(kura.blocks_count(), 3);
    assert_eq!(kura.get_durable_block_hash(nonzero!(4_usize)), None);
}
#[test]
fn hard_fork_extend_hash_only_marks_matching_snapshot_suffix_without_rewrite() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, BlockCount(block_count)) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    assert_eq!(block_count, 2);
    let first_hash = kura
        .block_hash_at_height(nonzero!(1_usize))
        .expect("first hash exists");
    let _first_block = kura
        .get_block(nonzero!(1_usize))
        .expect("first block body should be cacheable before hash-only activation");
    let second_hash = kura
        .block_hash_at_height(nonzero!(2_usize))
        .expect("second hash exists");
    {
        let mut store = kura.block_store.lock();
        store
            .write_block_index(1, EVICTED_BLOCK_START, 0)
            .expect("make matching tail hash-only");
    }
    {
        let mut block_data = kura.block_data.lock();
        block_data[1].1 = None;
    }
    kura.extend_hash_only_prefix_from_snapshot(&[first_hash, second_hash])
        .expect("matching snapshot hash journal should activate hash-only window");
    assert_eq!(
        kura.hard_fork_hash_only_block_count.load(Ordering::Relaxed),
        2,
        "matching snapshot suffix must still be classified as hash-only"
    );
    assert!(
        kura.get_block(nonzero!(2_usize)).is_none(),
        "matching hash-only suffix has no trusted block body"
    );
    assert!(
        kura.is_hash_only_block_height(nonzero!(1_usize)),
        "audited snapshot bodies remain logically unavailable even when cached"
    );
    assert!(
        kura.is_hash_only_block_height(nonzero!(2_usize)),
        "uncached matching suffix must be reported as hash-only unavailable"
    );
    assert_eq!(
        kura.hash_only_unavailable_prefix_len(2),
        2,
        "the authenticated prefix is unavailable independently of stale body caches"
    );
    {
        let mut block_data = kura.block_data.lock();
        block_data[0].1 = None;
    }
    assert!(
        kura.is_hash_only_block_height(nonzero!(1_usize)),
        "uncached hard-fork snapshot prefix must be reported as hash-only unavailable"
    );
    assert_eq!(
        kura.hash_only_unavailable_prefix_len(2),
        2,
        "fully uncached snapshot prefix should be skippable without per-height body probes"
    );
    drop(kura);
    for init_mode in [InitMode::Fast, InitMode::Strict] {
        let mut reopen_config = config.clone();
        reopen_config.init_mode = init_mode;
        assert!(matches!(
            Kura::open_test_kura_with_configured_lane_config(
                &reopen_config,
                &RuntimeLaneConfig::default()
            ),
            Err(Error::InvalidSnapshotBootstrapMarker { .. })
        ));
        if init_mode == InitMode::Fast {
            assert!(matches!(
                Kura::new_inner(
                    &reopen_config,
                    &RuntimeLaneConfig::default(),
                    None,
                    Some(2),
                    false,
                    PendingControlSidecarLimits::default(),
                ),
                Err(Error::InvalidSnapshotBootstrapMarker { .. })
            ));
            continue;
        }
        let (reopened, BlockCount(reopened_count)) = Kura::new_inner(
            &reopen_config,
            &RuntimeLaneConfig::default(),
            None,
            Some(2),
            false,
            PendingControlSidecarLimits::default(),
        )
        .expect("Strict mode opens the durable prefix for signed-lineage reauthentication");
        assert_eq!(reopened_count, 2);
        assert!(reopened.provisional_snapshot_bootstrap_pending());
        assert_eq!(reopened.hash_only_unavailable_prefix_len(2), 2);
        assert!(reopened.get_block(nonzero!(1_usize)).is_none());
        assert!(reopened.get_block(nonzero!(2_usize)).is_none());
    }
}
#[test]
fn hash_only_snapshot_rejects_shorter_existing_prefix() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 3);
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, BlockCount(block_count)) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    assert_eq!(block_count, 3);
    let first_hash = kura
        .block_hash_at_height(nonzero!(1_usize))
        .expect("first hash exists");
    let second_hash = kura
        .block_hash_at_height(nonzero!(2_usize))
        .expect("second hash exists");
    {
        let mut block_data = kura.block_data.lock();
        for index in 0..block_data.len() {
            block_data[index].1 = None;
        }
    }
    kura.hard_fork_hash_only_block_count
        .store(3, Ordering::Relaxed);
    assert!(
        kura.get_block(nonzero!(3_usize)).is_none(),
        "pre-fix hard-fork window hides the post-snapshot body"
    );
    assert!(matches!(
        kura.extend_hash_only_prefix_from_snapshot(&[first_hash, second_hash]),
        Err(Error::HashesFileHeightMismatch)
    ));
    assert_eq!(
        kura.hard_fork_hash_only_block_count.load(Ordering::Relaxed),
        3,
        "a rejected shorter snapshot must not change the existing marker"
    );
    assert!(
        kura.is_hash_only_block_height(nonzero!(2_usize)),
        "snapshot prefix remains hash-only"
    );
    assert!(kura.get_block(nonzero!(3_usize)).is_none());
}
#[test]
fn data_backed_count_preserves_hash_only_tail_for_hard_fork_bootstrap() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let mut store = new_block_store(&temp_dir);
    let snapshot_tail_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x8a; 32]));
    store.write_block_hash(2, snapshot_tail_hash).unwrap();
    store.write_block_index(2, EVICTED_BLOCK_START, 0).unwrap();
    assert_eq!(store.read_index_count().unwrap(), 3);
    assert_eq!(store.read_hashes_count().unwrap(), 3);
    assert_eq!(
        store.data_backed_count(3, 3, None).unwrap(),
        2,
        "normal recovery should prune hash-only placeholder tails"
    );
    assert_eq!(
        store.data_backed_count(3, 3, Some((0, 3))).unwrap(),
        3,
        "hard-fork bootstrap should preserve audited hash-only placeholder tails"
    );
}
#[test]
fn extend_hash_only_prefix_publishes_marker_with_batched_fsync() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = FsyncMode::Batched;
    let (kura, BlockCount(count)) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    assert_eq!(count, 2);
    let mut snapshot_hashes = vec![
        kura.get_block_hash(nonzero!(1_usize)).unwrap(),
        kura.get_block_hash(nonzero!(2_usize)).unwrap(),
    ];
    let snapshot_tail_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x9b; 32]));
    snapshot_hashes.push(snapshot_tail_hash);
    assert_eq!(
        kura.extend_hash_only_prefix_from_snapshot(&snapshot_hashes)
            .unwrap(),
        1
    );
    assert_eq!(kura.blocks_count(), 3);
    assert!(kura.get_block(nonzero!(3_usize)).is_none());
    let blocks_dir = primary_blocks_dir(&temp_dir);
    let mut reopened = BlockStore::with_fsync(&blocks_dir, FsyncMode::Batched, FSYNC_INTERVAL);
    reopened.create_files_if_they_do_not_exist().unwrap();
    assert_eq!(reopened.read_index_count().unwrap(), 3);
    assert_eq!(reopened.read_hashes_count().unwrap(), 3);
    assert_eq!(reopened.read_durable_index_count().unwrap(), 3);
    let marker = reopened
        .read_commit_marker()
        .unwrap()
        .expect("commit marker");
    assert_eq!(marker.count, 3);
    assert_eq!(
        reopened.read_block_index(2).unwrap(),
        (EVICTED_BLOCK_START, 0)
    );
}
#[test]
fn durable_count_fallback_releases_store_before_snapshot_extension_resumes() {
    let (_temp_dir, _config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
    let canonical = store_dummy_block_arcs(&kura, 1)
        .pop()
        .expect("store canonical prefix block");
    let snapshot_tail_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x7d; 32]));
    let snapshot_hashes = vec![canonical.hash(), snapshot_tail_hash];
    kura.pause_next_hash_only_extension_before_store_for_tests();
    let (extension_tx, extension_rx) = std::sync::mpsc::sync_channel(1);
    let extension_kura = Arc::clone(&kura);
    let extension = thread::spawn(move || {
        let result =
            extension_kura.extend_hash_only_suffix_from_verified_snapshot(&snapshot_hashes);
        extension_tx
            .send(result)
            .expect("report snapshot extension result");
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.hash_only_extension_paused_before_store_for_tests() {
        if Instant::now() >= deadline {
            kura.resume_hash_only_extension_before_store_for_tests();
            panic!("snapshot extension did not pause while owning block_data");
        }
        thread::yield_now();
    }
    kura.force_next_durable_blocks_count_fallback_for_tests();
    let (count_tx, count_rx) = std::sync::mpsc::sync_channel(1);
    let count_kura = Arc::clone(&kura);
    let counter = thread::spawn(move || {
        count_tx
            .send(count_kura.durable_blocks_count_lossy())
            .expect("report durable count fallback");
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.durable_blocks_count_fallback_reached_for_tests() {
        if Instant::now() >= deadline {
            kura.resume_hash_only_extension_before_store_for_tests();
            panic!("durable count did not reach its in-memory fallback");
        }
        thread::yield_now();
    }
    assert!(matches!(
        count_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    // The count thread is now waiting for `block_data`. Snapshot extension must be able to
    // acquire `block_store`, publish the new durable tail, and release `block_data` first.
    kura.resume_hash_only_extension_before_store_for_tests();
    extension_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("snapshot extension must not deadlock behind durable count")
        .expect("extend verified hash-only snapshot tail");
    assert_eq!(
        count_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("durable count fallback must finish after snapshot extension"),
        2
    );
    extension.join().expect("join snapshot extension thread");
    counter.join().expect("join durable count thread");
}
#[test]
fn exact_durable_count_rejects_corrupt_or_non_file_marker_without_logical_fallback() {
    for mutation in ["corrupt", "non-file"] {
        let (temp_dir, _config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
        store_dummy_block_arcs(&kura, 1);
        assert_eq!(kura.blocks_count(), 1);
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
        let marker_path = primary_blocks_dir(&temp_dir).join(COUNT_FILE_NAME);
        match mutation {
            "corrupt" => {
                std::fs::write(&marker_path, b"not a canonical marker").expect("corrupt marker")
            }
            "non-file" => {
                std::fs::remove_file(&marker_path).expect("remove marker");
                std::fs::create_dir(&marker_path).expect("replace marker with directory");
            }
            _ => unreachable!("covered mutations"),
        }
        assert!(
            kura.exact_durable_blocks_count().is_err(),
            "{mutation} marker must fail closed"
        );
        assert_eq!(
            kura.blocks_count(),
            1,
            "fixture proves the exact accessor did not return the logical height"
        );
    }
}
#[test]
fn exact_durable_count_rejects_partial_index_and_hash_entries() {
    for journal in [INDEX_FILE_NAME, HASHES_FILE_NAME] {
        let (temp_dir, _config, kura) = kura_root_fixture(BLOCKS_IN_MEMORY);
        store_dummy_block_arcs(&kura, 1);
        let path = primary_blocks_dir(&temp_dir).join(journal);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(&[0xA5]))
            .expect("append partial journal entry");
        assert!(
            kura.exact_durable_blocks_count().is_err(),
            "partial {journal} entry must fail closed"
        );
        assert_eq!(kura.blocks_count(), 1);
    }
}
#[test]
fn ambiguous_hash_only_marker_poison_gates_live_mutation_until_restart() {
    for new_marker_won in [false, true] {
        let temp_dir = TempDir::new().expect("create Kura root");
        populate_store(&temp_dir, 2);
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.fsync_mode = FsyncMode::Batched;
        config.fsync_interval = Duration::from_secs(60);
        let lane_config = RuntimeLaneConfig::default();
        let snapshot_tail_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x9c; 32]));
        {
            let (kura, BlockCount(count)) =
                Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
                    .expect("open Kura");
            assert_eq!(count, 2);
            let snapshot_hashes = vec![
                kura.get_block_hash(nonzero!(1_usize)).expect("height one"),
                kura.get_block_hash(nonzero!(2_usize)).expect("height two"),
                snapshot_tail_hash,
            ];
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
            let error = kura
                .extend_hash_only_suffix_from_verified_snapshot(&snapshot_hashes)
                .expect_err("unreadable marker state must fail stop");
            assert!(matches!(
                error,
                Error::DaBlockRewriteCommitStateUnknown { .. }
            ));
            assert!(error.requires_restart_recovery());
            assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
            assert_eq!(
                kura.block_data.lock().len(),
                2,
                "the live canonical image must not speculate about marker outcome"
            );
            assert!(matches!(
                kura.extend_hash_only_suffix_from_verified_snapshot(&snapshot_hashes),
                Err(Error::CanonicalStoragePoisoned)
            ));
            assert_eq!(
                kura.block_store
                    .lock()
                    .read_commit_marker()
                    .expect("read selected marker")
                    .expect("marker exists")
                    .count,
                if new_marker_won { 3 } else { 2 }
            );
        }
        assert!(matches!(
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config),
            Err(Error::InvalidSnapshotBootstrapMarker { .. })
        ));
        {
            // Complete only canonical BlockStore recovery. The verified
            // snapshot-tail marker authenticates the appended hash-only
            // entry, so either ambiguous marker readback converges on it.
            let mut recovered_store = new_block_store(&temp_dir);
            recovered_store
                .create_files_if_they_do_not_exist()
                .expect("recover the ambiguous canonical marker outcome");
            assert_eq!(
                recovered_store
                    .read_durable_index_count()
                    .expect("read recovered durable height"),
                3
            );
        }
        let (reopened, BlockCount(count)) = Kura::new_inner(
            &config,
            &lane_config,
            None,
            Some(3),
            false,
            PendingControlSidecarLimits::default(),
        )
        .expect("provisional restart resolves the authenticated marker outcome");
        assert_eq!(count, 3);
        assert_eq!(
            reopened.block_hash_at_height(nonzero!(3_usize)),
            Some(snapshot_tail_hash)
        );
        assert!(!reopened.canonical_storage_poisoned.load(Ordering::Acquire));
    }
}
#[test]
fn verified_snapshot_tail_survives_batched_fsync_reopen_and_second_restart() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = FsyncMode::Batched;
    let lane_config = RuntimeLaneConfig::default();
    let (kura, BlockCount(count)) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).unwrap();
    assert_eq!(count, 2);
    let mut snapshot_hashes = vec![
        kura.get_block_hash(nonzero!(1_usize)).unwrap(),
        kura.get_block_hash(nonzero!(2_usize)).unwrap(),
    ];
    let snapshot_tail_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xa1; 32]));
    snapshot_hashes.push(snapshot_tail_hash);
    assert_eq!(
        kura.extend_hash_only_suffix_from_verified_snapshot(&snapshot_hashes)
            .unwrap(),
        1
    );
    drop(kura);
    for restart in 1..=2 {
        assert!(matches!(
            Kura::open_test_kura_with_configured_lane_config(&config, &lane_config),
            Err(Error::InvalidSnapshotBootstrapMarker { .. })
        ));
        let (reopened, BlockCount(count)) = Kura::new_inner(
            &config,
            &lane_config,
            None,
            Some(3),
            false,
            PendingControlSidecarLimits::default(),
        )
        .expect("open verified hash-only suffix provisionally");
        assert_eq!(
            count, 3,
            "provisional restart {restart} must retain the verified suffix"
        );
        assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 3);
        assert!(reopened.provisional_snapshot_bootstrap_pending());
        assert_eq!(
            reopened.block_hash_at_height(nonzero!(3_usize)),
            Some(snapshot_tail_hash)
        );
        assert!(reopened.get_block(nonzero!(3_usize)).is_none());
        drop(reopened);
    }
}
#[test]
fn unmarked_zero_length_tail_is_pruned_with_batched_fsync() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = FsyncMode::Batched;
    let lane_config = RuntimeLaneConfig::default();
    let blocks_dir = primary_blocks_dir(&temp_dir);
    let mut store = BlockStore::with_fsync(&blocks_dir, FsyncMode::Batched, FSYNC_INTERVAL);
    let unverified_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xa2; 32]));
    store.write_block_index(2, EVICTED_BLOCK_START, 0).unwrap();
    store.write_block_hash(2, unverified_hash).unwrap();
    drop(store);
    let (reopened, BlockCount(count)) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).unwrap();
    assert_eq!(count, 2);
    drop(reopened);
    let mut reopened = BlockStore::with_fsync(&blocks_dir, FsyncMode::Batched, FSYNC_INTERVAL);
    assert_eq!(reopened.read_index_count().unwrap(), 2);
    assert_eq!(reopened.read_hashes_count().unwrap(), 2);
    assert!(!blocks_dir.join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME).exists());
}
#[test]
fn verified_snapshot_tail_with_mismatched_hash_digest_is_pruned() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = FsyncMode::Batched;
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).unwrap();
    let mut snapshot_hashes = vec![
        kura.get_block_hash(nonzero!(1_usize)).unwrap(),
        kura.get_block_hash(nonzero!(2_usize)).unwrap(),
    ];
    snapshot_hashes.push(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0xa3; 32]),
    ));
    kura.extend_hash_only_suffix_from_verified_snapshot(&snapshot_hashes)
        .unwrap();
    drop(kura);
    let blocks_dir = primary_blocks_dir(&temp_dir);
    let mut tampered = BlockStore::with_fsync(&blocks_dir, FsyncMode::Batched, FSYNC_INTERVAL);
    tampered
        .write_block_hash(
            2,
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xff; 32])),
        )
        .unwrap();
    drop(tampered);
    assert!(matches!(
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config),
        Err(Error::InvalidSnapshotBootstrapMarker { .. })
    ));
    let provisional_error = Kura::new_inner(
        &config,
        &lane_config,
        None,
        Some(3),
        false,
        PendingControlSidecarLimits::default(),
    )
    .expect_err("provisional startup must reject the mismatched hash digest");
    assert!(matches!(
        provisional_error,
        Error::InvalidSnapshotBootstrapMarker { ref reason, .. }
            if reason.contains("hash-journal digest")
    ));
    let mut reopened = BlockStore::with_fsync(&blocks_dir, FsyncMode::Batched, FSYNC_INTERVAL);
    assert_eq!(reopened.read_index_count().unwrap(), 3);
    assert_eq!(reopened.read_hashes_count().unwrap(), 3);
    assert!(
        blocks_dir.join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME).exists(),
        "failed authentication must not delete or rewrite the supplied marker"
    );
}
#[test]
fn malformed_verified_snapshot_tail_marker_is_pruned() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = FsyncMode::Batched;
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).unwrap();
    let mut snapshot_hashes = vec![
        kura.get_block_hash(nonzero!(1_usize)).unwrap(),
        kura.get_block_hash(nonzero!(2_usize)).unwrap(),
    ];
    snapshot_hashes.push(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0xa5; 32]),
    ));
    kura.extend_hash_only_suffix_from_verified_snapshot(&snapshot_hashes)
        .unwrap();
    drop(kura);
    let blocks_dir = primary_blocks_dir(&temp_dir);
    std::fs::write(
        blocks_dir.join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME),
        b"malformed marker",
    )
    .unwrap();
    assert!(matches!(
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config),
        Err(Error::InvalidSnapshotBootstrapMarker { .. })
    ));
    let provisional_error = Kura::new_inner(
        &config,
        &lane_config,
        None,
        Some(3),
        false,
        PendingControlSidecarLimits::default(),
    )
    .expect_err("provisional startup must reject malformed marker bytes");
    assert!(matches!(
        provisional_error,
        Error::InvalidSnapshotBootstrapMarker { ref reason, .. }
            if reason.contains("failed to decode marker")
    ));
    let mut reopened = BlockStore::with_fsync(&blocks_dir, FsyncMode::Batched, FSYNC_INTERVAL);
    assert_eq!(reopened.read_index_count().unwrap(), 3);
    assert_eq!(reopened.read_hashes_count().unwrap(), 3);
    assert_eq!(
        std::fs::read(blocks_dir.join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME)).unwrap(),
        b"malformed marker",
        "unauthenticated startup must not repair or delete marker bytes"
    );
}
#[test]
fn commit_marker_reconciliation_caps_durable_count_to_hash_journal() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let mut store = new_block_store(&temp_dir);
    store.truncate_hashes_to_count(3).unwrap();
    let mut reopened = new_block_store(&temp_dir);
    reopened.create_files_if_they_do_not_exist().unwrap();
    assert_eq!(reopened.read_durable_index_count().unwrap(), 3);
    assert_eq!(reopened.read_index_count().unwrap(), 3);
    assert_eq!(reopened.read_hashes_count().unwrap(), 3);
}
#[test]
fn strict_init_prunes_corrupted_index_end_to_end() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = new_block_store(&temp_dir);
    store.create_files_if_they_do_not_exist().unwrap();
    let block: SignedBlock = ValidBlock::new_dummy(checked_keypair().private_key()).into();
    store.append_block_to_chain(&block).unwrap();
    let BlockIndex { start, .. } = store.read_block_index(0).unwrap();
    let huge_len = STRICT_INIT_MAX_BLOCK_BYTES + 1;
    store.write_block_index(0, start, huge_len).unwrap();
    let store_dir = temp_dir.path().to_path_buf();
    drop(store);
    let config = kura_config_for_path(&store_dir, BLOCKS_IN_MEMORY);
    let (kura, BlockCount(count)) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    assert_eq!(count, 0);
    assert_eq!(kura.blocks_count(), 0);
}
#[test]
fn prune_blocks() -> eyre::Result<()> {
    let temp = TempDir::new()?;
    let mut store = BlockStore::new(temp.path());
    store.create_files_if_they_do_not_exist()?;
    // prune on empty store - should be fine
    store.prune(0)?;
    // prune with height greater than there is - should be fine
    store.prune(10)?;
    // add some blocks
    let mut blocks = DummyBlocks::new();
    for _ in 0..10 {
        store.append_block_to_chain(&blocks.next())?;
    }
    assert_eq!(store.read_index_count()?, 10);
    assert_eq!(store.read_block_hashes(0, 10)?.len(), 10);
    store.prune(5)?;
    assert_eq!(store.read_index_count()?, 5);
    assert_eq!(store.read_block_hashes(0, 5)?.len(), 5);
    assert!(store.read_block_hashes(0, 7).is_err());
    for i in 0..5 {
        let block = read_block(&mut store, i)?;
        assert_eq!(block, *blocks.get(i).unwrap());
    }
    assert!(read_block(&mut store, 5).is_err());
    // prune on non-empty state with height greater than there are blocks - should be fine
    store.prune(7)?;
    // can add blocks again
    for i in 5..10 {
        store.append_block_to_chain(&blocks.get(i).unwrap())?;
    }
    for i in 0..10 {
        let block = read_block(&mut store, i)?;
        assert_eq!(block, *blocks.get(i).unwrap());
    }
    Ok(())
}
#[test]
fn kura_prune_to_height_truncates_in_memory_chain() {
    let (kura, mut blocks) = blank_kura_with_blocks();
    let b1 = blocks.next();
    let b2 = blocks.next();
    let b3 = blocks.next();
    let b2_hash = b2.hash();
    let b3_hash = b3.hash();
    kura.store_block(b1).expect("store block");
    kura.store_block(b2).expect("store block");
    kura.store_block(b3.clone()).expect("store block");
    assert_eq!(kura.blocks_count(), 3);
    assert_eq!(
        kura.get_block_height_by_hash(b3_hash),
        Some(nonzero!(3_usize))
    );
    kura.prune_to_height(2).expect("prune to height");
    assert_eq!(kura.blocks_count(), 2);
    assert_eq!(
        kura.get_block_height_by_hash(b2_hash),
        Some(nonzero!(2_usize))
    );
    assert_eq!(kura.get_block_height_by_hash(b3_hash), None);
    kura.store_block(b3).expect("store block after prune");
    assert_eq!(kura.blocks_count(), 3);
    assert_eq!(
        kura.get_block_height_by_hash(b3_hash),
        Some(nonzero!(3_usize))
    );
}
fn populate_prune_recovery_fixture(
    temp_dir: &TempDir,
) -> (KuraConfig, Vec<Arc<SignedBlock>>, Vec<MergeLedgerEntry>) {
    let config = kura_config_for_dir(temp_dir, nonzero!(1_usize));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let mut generator = DummyBlocks::new();
    let mut blocks = Vec::new();
    let mut merge_entries = Vec::new();
    for height in 1..=4 {
        let raw = generator.next();
        if matches!(height, 2 | 4) {
            let epoch = u64::try_from(merge_entries.len() + 1).expect("fixture epoch");
            let entry = sample_merge_entry_for_block(epoch, &raw);
            let carrier = attach_merge_reference(&raw, &entry);
            *generator.blocks.last_mut().expect("raw carrier") = Arc::clone(&carrier);
            kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
                .expect("store merge carrier fixture");
            blocks.push(carrier);
            merge_entries.push(entry);
        } else {
            kura.store_block(Arc::clone(&raw))
                .expect("store ordinary prune fixture block");
            blocks.push(raw);
        }
    }
    for (index, block) in blocks.iter().enumerate() {
        let height = u64::try_from(index + 1).expect("fixture height");
        kura.write_pipeline_metadata(&PipelineRecoverySidecar::new(
            height,
            block.hash(),
            PipelineDagSnapshot {
                fingerprint: [u8::try_from(height).expect("small fixture height"); 32],
                key_count: u32::try_from(height).expect("small fixture height"),
            },
            Vec::new(),
        ));
    }
    for height in [2_u64, 4] {
        let block_hash = blocks[usize::try_from(height - 1).expect("fixture index")].hash();
        let checkpoint_hash = Hash::new(format!("prune checkpoint {height}").as_bytes());
        kura.store_wsv_checkpoint(height, block_hash, checkpoint_hash)
            .expect("store prune fixture checkpoint");
        kura.store_commit_manifest(CommitManifest::new(
            height,
            block_hash,
            None,
            None,
            checkpoint_hash,
            None,
        ))
        .expect("store prune fixture manifest");
    }
    // Seed a removable DA-sidecar suffix without finalizing the block.
    // Production eviction requires signed complete-wire finality, and such
    // a block must not subsequently be pruned by this recovery fixture.
    let (block3_wire, _) = blocks[2]
        .canonical_wire()
        .expect("encode prune fixture block")
        .into_parts();
    kura.block_store
        .lock()
        .write_da_block_bytes(3, &block3_wire)
        .expect("seed prune fixture DA sidecar");
    assert!(kura.block_store.lock().da_block_path(3).exists());
    // The retained carrier at height two is public evidence and therefore
    // requires current-version finality. Height four deliberately remains
    // the sole prepublication tip so prune exercises exact suffix removal.
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    drop(kura);
    (config, blocks, merge_entries)
}
#[test]
fn active_prune_rejects_consensus_sidecar_enqueues_without_queue_mutation() {
    let temp_dir = TempDir::new().expect("tempdir");
    let (config, blocks, _) = populate_prune_recovery_fixture(&temp_dir);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("reopen prune fixture");
    // The recovery fixture keeps one block body in memory, which also
    // configures a one-entry pipeline queue. This test needs two retained
    // entries to prove that pruning filters the existing queue without
    // mutating it during active-prune rejection.
    kura.set_pipeline_sidecar_queue_cap_for_testing(2);
    let retained_pipeline = PipelineRecoverySidecar::new(
        1,
        blocks[0].hash(),
        PipelineDagSnapshot {
            fingerprint: [0x51; 32],
            key_count: 1,
        },
        Vec::new(),
    );
    assert_eq!(
        kura.enqueue_pipeline_metadata(retained_pipeline),
        PipelineSidecarEnqueueResult::Enqueued { queue_depth: 1 }
    );
    assert_eq!(
        kura.enqueue_pipeline_metadata(PipelineRecoverySidecar::new(
            4,
            blocks[3].hash(),
            PipelineDagSnapshot {
                fingerprint: [0x52; 32],
                key_count: 2,
            },
            Vec::new(),
        )),
        PipelineSidecarEnqueueResult::Enqueued { queue_depth: 2 }
    );
    assert_eq!(
        kura.enqueue_fastpq_proof_snapshot(sample_fastpq_snapshot(1, blocks[0].hash(), 8)),
        FastpqProofEnqueueResult::Enqueued { queue_depth: 1 }
    );
    assert_eq!(
        kura.enqueue_fastpq_proof_snapshot(sample_fastpq_snapshot(4, blocks[3].hash(), 9)),
        FastpqProofEnqueueResult::Enqueued { queue_depth: 2 }
    );
    kura.pause_prune_before_intent
        .store(true, Ordering::Release);
    let (prune_tx, prune_rx) = std::sync::mpsc::sync_channel(1);
    let prune_kura = Arc::clone(&kura);
    let pruner = thread::spawn(move || {
        prune_tx
            .send(prune_kura.prune_to_height(2))
            .expect("report prune result");
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.prune_paused_before_intent.load(Ordering::Acquire) {
        if Instant::now() >= deadline {
            kura.pause_prune_before_intent
                .store(false, Ordering::Release);
            pruner.join().expect("pruner after pause timeout");
            panic!("prune never reached the pre-intent pause");
        }
        thread::yield_now();
    }
    assert!(
        kura.prune_in_progress.load(Ordering::Acquire),
        "real prune must publish the enqueue gate before preflight can pause"
    );
    let rejected_pipeline = PipelineRecoverySidecar::new(
        4,
        blocks[3].hash(),
        PipelineDagSnapshot {
            fingerprint: [0x53; 32],
            key_count: 3,
        },
        Vec::new(),
    );
    let rejected_fastpq = sample_fastpq_snapshot(4, blocks[3].hash(), 10);
    let (enqueue_tx, enqueue_rx) = std::sync::mpsc::sync_channel(1);
    let enqueue_kura = Arc::clone(&kura);
    let enqueuer = thread::spawn(move || {
        enqueue_tx
            .send((
                enqueue_kura.enqueue_pipeline_metadata(rejected_pipeline),
                enqueue_kura.enqueue_fastpq_proof_snapshot(rejected_fastpq),
            ))
            .expect("report active-prune enqueue results");
    });
    let enqueue_results = enqueue_rx.recv_timeout(Duration::from_secs(2));
    if enqueue_results.is_err() {
        kura.pause_prune_before_intent
            .store(false, Ordering::Release);
        let _ = prune_rx.recv_timeout(Duration::from_secs(5));
        pruner.join().expect("pruner after enqueue timeout");
        enqueuer.join().expect("enqueuer after enqueue timeout");
        panic!("consensus enqueue waited behind an active prune");
    }
    let (pipeline_result, fastpq_result) = enqueue_results.expect("checked above");
    enqueuer.join().expect("active-prune enqueuer");
    assert_eq!(
        pipeline_result,
        PipelineSidecarEnqueueResult::RejectedPruneRecovery
    );
    assert_eq!(
        fastpq_result,
        FastpqProofEnqueueResult::RejectedPruneRecovery
    );
    assert_eq!(
        kura.pipeline_sidecar_queue.lock().len(),
        2,
        "active prune rejection must not mutate the pre-prune pipeline queue"
    );
    assert_eq!(
        kura.fastpq_proof_queue.lock().len(),
        2,
        "active prune rejection must not mutate the pre-prune FASTPQ queue"
    );
    kura.pause_prune_before_intent
        .store(false, Ordering::Release);
    prune_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("prune completes after resume")
        .expect("prune succeeds");
    pruner.join().expect("pruner");
    assert!(!kura.prune_in_progress.load(Ordering::Acquire));
    assert!(!kura.prune_recovery_is_required());
    let pipeline_queue = kura.pipeline_sidecar_queue.lock();
    assert_eq!(pipeline_queue.len(), 1);
    assert_eq!(pipeline_queue[0].height, 1);
    drop(pipeline_queue);
    let fastpq_queue = kura.fastpq_proof_queue.lock();
    assert_eq!(fastpq_queue.len(), 1);
    assert_eq!(fastpq_queue[0].snapshot.height, 1);
}
#[test]
#[allow(clippy::too_many_lines)]
fn readers_blocked_behind_inflight_prune_fail_closed_after_durable_intent() {
    let temp_dir = TempDir::new().expect("tempdir");
    let (config, blocks, _) = populate_prune_recovery_fixture(&temp_dir);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("reopen prune fixture");
    assert_eq!(
        kura.get_block_hash(nonzero!(4_usize)),
        Some(blocks[3].hash()),
        "test requires an above-target cached hash before prune"
    );
    assert_eq!(
        kura.get_block(nonzero!(4_usize)).as_deref(),
        Some(blocks[3].as_ref()),
        "test requires an above-target cached block before prune"
    );
    kura.fail_prune_after_stage_for_tests(PRUNE_STAGE_INTENT);
    kura.pause_prune_before_intent
        .store(true, Ordering::Release);
    let (prune_tx, prune_rx) = std::sync::mpsc::sync_channel(1);
    let prune_kura = Arc::clone(&kura);
    let pruner = thread::spawn(move || {
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prune_kura
                .prune_to_height(2)
                .expect("injected prune must panic after intent")
        }))
        .is_err();
        prune_tx.send(crashed).expect("report injected prune");
    });
    let prune_deadline = Instant::now() + Duration::from_secs(5);
    while !kura.prune_paused_before_intent.load(Ordering::Acquire) {
        if Instant::now() >= prune_deadline {
            kura.pause_prune_before_intent
                .store(false, Ordering::Release);
            pruner.join().expect("pruner after hook timeout");
            panic!("prune never reached the pre-intent lock barrier");
        }
        thread::yield_now();
    }
    kura.canonical_read_kinds_after_prune_check
        .store(0, Ordering::Release);
    kura.observe_canonical_reads_after_prune_check
        .store(true, Ordering::Release);
    let (hash_tx, hash_rx) = std::sync::mpsc::sync_channel(1);
    let hash_kura = Arc::clone(&kura);
    let hash_reader = thread::spawn(move || {
        hash_tx
            .send(hash_kura.get_block_hash(nonzero!(4_usize)))
            .expect("report blocked hash read");
    });
    let (block_tx, block_rx) = std::sync::mpsc::sync_channel(1);
    let block_kura = Arc::clone(&kura);
    let block_reader = thread::spawn(move || {
        block_tx
            .send(block_kura.get_block(nonzero!(4_usize)))
            .expect("report blocked block read");
    });
    let readers_deadline = Instant::now() + Duration::from_secs(5);
    while (kura
        .canonical_read_kinds_after_prune_check
        .load(Ordering::Acquire)
        & (CANONICAL_HASH_READER_OBSERVED | CANONICAL_BLOCK_READER_OBSERVED))
        != (CANONICAL_HASH_READER_OBSERVED | CANONICAL_BLOCK_READER_OBSERVED)
    {
        if Instant::now() >= readers_deadline {
            kura.observe_canonical_reads_after_prune_check
                .store(false, Ordering::Release);
            kura.pause_prune_before_intent
                .store(false, Ordering::Release);
            pruner.join().expect("pruner after reader timeout");
            hash_reader.join().expect("hash reader after timeout");
            block_reader.join().expect("block reader after timeout");
            panic!("canonical readers never reached their post-check lock barriers");
        }
        thread::yield_now();
    }
    kura.observe_canonical_reads_after_prune_check
        .store(false, Ordering::Release);
    kura.pause_prune_before_intent
        .store(false, Ordering::Release);
    assert!(
        prune_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("prune reaches injected crash"),
        "prune must fail-stop after publishing its durable intent"
    );
    assert_eq!(
        hash_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("blocked hash reader resumes"),
        None,
        "a reader that passed its precheck must not expose an above-target cached hash after fail-stop"
    );
    assert_eq!(
        block_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("blocked block reader resumes"),
        None,
        "a reader that passed its precheck must not expose an above-target cached block after fail-stop"
    );
    pruner.join().expect("pruner");
    crate::sumeragi::status::clear_consensus_transition_poison_for_tests();
    hash_reader.join().expect("hash reader");
    block_reader.join().expect("block reader");
    assert!(kura.prune_recovery_is_required());
    assert_eq!(
        kura.block_data.lock().len(),
        4,
        "intent-stage fail-stop intentionally leaves the stale in-memory suffix for restart recovery"
    );
    assert_eq!(kura.blocks_count(), 0);
    assert_eq!(kura.block_hash_at_height(nonzero!(4_usize)), None);
}
#[test]
fn prune_crash_boundaries_recover_forward_and_poison_live_kura() {
    for stage in PRUNE_STAGE_INTENT..=PRUNE_STAGE_MEMORY {
        let temp_dir = TempDir::new().expect("tempdir");
        let (config, blocks, merge_entries) = populate_prune_recovery_fixture(&temp_dir);
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("reopen");
        let retained_finality = v2_finality_artifact_for_block(&blocks[0]);
        let retained_receipt = kura
            .store_v2_finality_artifact(&retained_finality)
            .expect("seed retained v2 finality artifact");
        kura.fail_prune_after_stage_for_tests(stage);
        let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            kura.prune_to_height(2).expect("injected prune must panic")
        }));
        assert!(crash.is_err(), "stage {stage} must inject a crash");
        crate::sumeragi::status::clear_consensus_transition_poison_for_tests();
        assert!(
            !kura.prune_in_progress.load(Ordering::Acquire),
            "stage {stage} must clear the in-process gate during unwind"
        );
        assert!(kura.prune_recovery_is_required());
        assert_eq!(
            kura.enqueue_pipeline_metadata(PipelineRecoverySidecar::new(
                1,
                blocks[0].hash(),
                PipelineDagSnapshot {
                    fingerprint: [0x61; 32],
                    key_count: 1,
                },
                Vec::new(),
            )),
            PipelineSidecarEnqueueResult::RejectedPruneRecovery,
            "stage {stage} must reject pipeline enqueue after fail-stop"
        );
        assert_eq!(
            kura.enqueue_fastpq_proof_snapshot(sample_fastpq_snapshot(1, blocks[0].hash(), 8)),
            FastpqProofEnqueueResult::RejectedPruneRecovery,
            "stage {stage} must reject FASTPQ enqueue after fail-stop"
        );
        assert!(kura.pipeline_sidecar_queue.lock().is_empty());
        assert!(kura.fastpq_proof_queue.lock().is_empty());
        assert_eq!(kura.get_block(nonzero!(1_usize)), None);
        assert_eq!(kura.block_hash_at_height(nonzero!(1_usize)), None);
        assert!(matches!(
            kura.merge_carrier_records(),
            Err(Error::PruneRecoveryRequired)
        ));
        assert!(matches!(
            kura.latest_merge_execution_heights(),
            Err(Error::PruneRecoveryRequired)
        ));
        assert!(matches!(
            kura.wsv_checkpoint(2),
            Err(Error::PruneRecoveryRequired)
        ));
        assert!(matches!(
            kura.commit_manifest(2),
            Err(Error::PruneRecoveryRequired)
        ));
        assert!(matches!(
            kura.store_v2_finality_artifact(&retained_finality),
            Err(Error::PruneRecoveryRequired)
        ));
        assert!(matches!(
            kura.v2_finality_artifact(retained_finality.height),
            Err(Error::PruneRecoveryRequired)
        ));
        assert!(matches!(
            kura.v2_finality_artifact_with_receipt(retained_finality.height),
            Err(Error::PruneRecoveryRequired)
        ));
        assert!(matches!(
            kura.prune_to_height(2),
            Err(Error::PruneRecoveryRequired)
        ));
        assert!(matches!(
            kura.store_block(Arc::clone(&blocks[2])),
            Err(Error::PruneRecoveryRequired)
        ));
        drop(kura);
        let (recovered, BlockCount(count)) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("recover prune intent");
        assert_eq!(count, 2, "stage {stage}");
        assert_eq!(recovered.blocks_count(), 2, "stage {stage}");
        assert_eq!(
            recovered.get_block_hash(nonzero!(2_usize)),
            Some(blocks[1].hash()),
            "stage {stage}"
        );
        assert!(
            recovered.read_pipeline_metadata(2).is_some(),
            "stage {stage}"
        );
        assert!(
            recovered.read_pipeline_metadata(3).is_none(),
            "stage {stage}"
        );
        assert_eq!(
            recovered.merge_ledger_all_entries().expect("merge log"),
            vec![merge_entries[0].clone()],
            "stage {stage}"
        );
        let carriers = recovered.merge_carrier_records().expect("carrier records");
        assert_eq!(carriers.len(), 1, "stage {stage}");
        assert_eq!(carriers[0].block_height, 2, "stage {stage}");
        assert!(
            !recovered.block_store.lock().da_block_path(3).exists(),
            "stage {stage}"
        );
        assert!(
            !recovered.retained_block_record_path(3).exists(),
            "stage {stage} must remove retained records above the recovered tip"
        );
        assert!(
            recovered
                .wsv_checkpoint(4)
                .expect("checkpoint query")
                .is_none(),
            "stage {stage}"
        );
        assert!(
            recovered
                .commit_manifest(4)
                .expect("manifest query")
                .is_none(),
            "stage {stage}"
        );
        assert_eq!(
            recovered
                .v2_finality_artifact(retained_finality.height)
                .expect("retained finality query after recovery"),
            Some(retained_finality.clone()),
            "stage {stage}"
        );
        let (recovered_finality, recovered_receipt) = recovered
            .v2_finality_artifact_with_receipt(retained_finality.height)
            .expect("retained finality receipt query after recovery")
            .expect("retained finality artifact survives recovery");
        assert_eq!(recovered_finality, retained_finality, "stage {stage}");
        assert_eq!(
            recovered_receipt.artifact_hash(),
            retained_receipt.artifact_hash(),
            "stage {stage}"
        );
        assert!(
            !Kura::prune_intent_path_for(temp_dir.path()).exists(),
            "stage {stage}"
        );
        recovered
            .store_block(Arc::clone(&blocks[2]))
            .expect("append original successor after recovery");
    }
}
#[test]
fn prune_indexed_sidecar_promotion_failures_preserve_recovery_and_reject_stale_tail() {
    for promotion_stage in [PRUNE_SIDECAR_PROMOTION_DATA, PRUNE_SIDECAR_PROMOTION_INDEX] {
        let temp_dir = TempDir::new().expect("tempdir");
        let (config, blocks, _) = populate_prune_recovery_fixture(&temp_dir);
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("reopen");
        kura.fail_prune_sidecar_promotion_for_tests(promotion_stage);
        let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = kura.prune_to_height(2);
        }));
        assert!(crash.is_err(), "promotion stage {promotion_stage}");
        crate::sumeragi::status::clear_consensus_transition_poison_for_tests();
        let pipeline_dir = primary_blocks_dir(&temp_dir).join(PIPELINE_DIR_NAME);
        let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
        let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
        let temp_data = data_path.with_extension("norito.tmp");
        let temp_index = index_path.with_extension("index.tmp");
        if promotion_stage == PRUNE_SIDECAR_PROMOTION_DATA {
            assert!(
                temp_data.exists(),
                "staged data must survive failed promotion"
            );
        } else {
            assert!(
                !temp_data.exists(),
                "data is already canonical at the between-renames boundary"
            );
        }
        assert!(
            temp_index.exists(),
            "the only authoritative compact index must survive failed promotion"
        );
        drop(kura);
        let (recovered, BlockCount(count)) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("retry prune recovery");
        assert_eq!(count, 2);
        assert_eq!(
            recovered
                .read_pipeline_metadata(2)
                .map(|sidecar| sidecar.block_hash),
            Some(blocks[1].hash())
        );
        assert!(recovered.read_pipeline_metadata(3).is_none());
        assert!(!temp_data.exists());
        assert!(!temp_index.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&index_path, &temp_index).expect("create adversarial temp-index symlink");
            let _guard = recovered.sidecar_lock.lock();
            assert!(matches!(
                recovered.validate_pipeline_sidecars_for_prune(2, true),
                Err(Error::PruneIntentConflict(message))
                    if message.contains("regular no-follow")
            ));
            drop(_guard);
            std::fs::remove_file(&temp_index).expect("remove temp-index symlink");
        }
        std::fs::OpenOptions::new()
            .append(true)
            .open(&data_path)
            .and_then(|mut file| file.write_all(b"stale pruned payload tail"))
            .expect("append adversarial stale data tail");
        let _guard = recovered.sidecar_lock.lock();
        assert!(matches!(
            recovered.validate_pipeline_sidecars_for_prune(2, true),
            Err(Error::PruneIntentConflict(message))
                if message.contains("unreferenced bytes")
        ));
    }
}
#[test]
fn prune_intent_tampering_fails_closed() {
    let temp_dir = TempDir::new().expect("tempdir");
    let (config, blocks, merge_entries) = populate_prune_recovery_fixture(&temp_dir);
    let intent_path = Kura::prune_intent_path_for(temp_dir.path());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("open tampering fixture for exact sidecar projection");
    let sidecar_rewrite = {
        let _guard = kura.sidecar_lock.lock();
        kura.reconcile_and_project_prune_sidecar_rewrites_locked(2)
            .expect("project tampering fixture retained sidecars")
    };
    let valid_intent = admit_prune_intent_fixture(
        &kura,
        KuraPruneIntentV3 {
            version: 3,
            source_height: 4,
            source_tip_hash: Some(blocks[3].hash()),
            target_height: 2,
            target_tip_hash: Some(blocks[1].hash()),
            retained_merge_entries: 1,
            retained_merge_tip_hash: Some(merge_entries[0].canonical_hash()),
            sidecar_rewrite,
            capacity: unsealed_prune_capacity_fixture(),
        },
    );
    drop(kura);
    let mut tampered = valid_intent.clone();
    tampered.target_tip_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
        b"tampered prune target",
    )));
    std::fs::write(
        &intent_path,
        norito::to_bytes(&tampered).expect("encode tampered intent"),
    )
    .expect("write tampered intent");
    assert!(matches!(
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default()),
        Err(Error::PruneIntentConflict(_))
    ));
    assert!(
        intent_path.exists(),
        "tampered intent must not be discarded"
    );
    std::fs::write(&intent_path, vec![0_u8; PRUNE_INTENT_MAX_BYTES + 1])
        .expect("write oversized intent");
    assert!(matches!(
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default()),
        Err(Error::PruneIntentConflict(message)) if message.contains("invalid byte length")
    ));
    let temp_intent_path = Kura::prune_intent_temp_path_for(temp_dir.path());
    std::fs::write(
        &intent_path,
        norito::to_bytes(&valid_intent).expect("encode canonical intent"),
    )
    .expect("write canonical intent");
    std::fs::write(
        &temp_intent_path,
        norito::to_bytes(&tampered).expect("encode disagreeing temporary intent"),
    )
    .expect("write disagreeing temporary intent");
    assert!(matches!(
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default()),
        Err(Error::PruneIntentConflict(message))
            if message.contains("not one authenticated two-link publication object")
    ));
    std::fs::remove_file(&temp_intent_path).expect("remove disagreeing temporary intent");
    std::fs::remove_file(&intent_path).expect("remove canonical intent");
    std::fs::create_dir(&intent_path).expect("create non-file intent");
    assert!(matches!(
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default()),
        Err(Error::PruneIntentConflict(message)) if message.contains("regular no-follow")
    ));
    std::fs::remove_dir(&intent_path).expect("remove non-file intent");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let symlink_target = temp_dir.path().join("prune-intent-symlink-target");
        std::fs::write(
            &symlink_target,
            norito::to_bytes(&valid_intent).expect("encode symlink target intent"),
        )
        .expect("write symlink target intent");
        symlink(&symlink_target, &intent_path).expect("create intent symlink");
        assert!(matches!(
            Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default()),
            Err(Error::PruneIntentConflict(message)) if message.contains("regular no-follow")
        ));
        std::fs::remove_file(&intent_path).expect("remove intent symlink");
        std::fs::remove_file(&symlink_target).expect("remove symlink target");
    }
    std::fs::write(
        &intent_path,
        norito::to_bytes(&valid_intent).expect("encode valid intent"),
    )
    .expect("write valid intent");
    let (recovered, BlockCount(count)) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("retry valid prune recovery");
    assert_eq!(count, 2);
    assert_eq!(
        recovered.get_block_hash(nonzero!(2_usize)),
        Some(blocks[1].hash())
    );
    assert!(!intent_path.exists());
    drop(recovered);
    let (_, BlockCount(repeated_count)) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("idempotent recovered reopen");
    assert_eq!(repeated_count, 2);
}
#[test]
fn concurrent_store_waits_for_prune_and_revalidates_the_tip() {
    let (_temp_dir, config) = kura_storage_fixture("tempdir", BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let mut blocks = DummyBlocks::new();
    kura.store_block(blocks.next()).expect("store block 1");
    kura.store_block(blocks.next()).expect("store block 2");
    let block3 = blocks.next();
    kura.pause_prune_before_intent
        .store(true, Ordering::Release);
    let (prune_tx, prune_rx) = std::sync::mpsc::sync_channel(1);
    let pruning_kura = Arc::clone(&kura);
    let prune_thread = thread::spawn(move || {
        prune_tx
            .send(pruning_kura.prune_to_height(1))
            .expect("report prune result");
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.prune_paused_before_intent.load(Ordering::Acquire) {
        if Instant::now() >= deadline {
            kura.pause_prune_before_intent
                .store(false, Ordering::Release);
            prune_thread.join().expect("join timed-out prune");
            panic!("prune did not reach the pre-intent pause");
        }
        thread::yield_now();
    }
    let (store_tx, store_rx) = std::sync::mpsc::sync_channel(1);
    let storing_kura = Arc::clone(&kura);
    let store_thread = thread::spawn(move || {
        store_tx
            .send(storing_kura.store_block(block3))
            .expect("report store result");
    });
    assert!(
        store_rx.recv_timeout(Duration::from_millis(50)).is_err(),
        "canonical store must remain serialized behind the active prune"
    );
    kura.pause_prune_before_intent
        .store(false, Ordering::Release);
    prune_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("prune must not deadlock with direct block storage")
        .expect("prune to height 1");
    let store_error = store_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("direct block storage must finish after prune")
        .expect_err("height-3 append must be revalidated against the pruned tip");
    match store_error {
        Error::BlockHeightGap {
            expected_next_height,
            actual_height,
        } => {
            assert_eq!(expected_next_height, 2);
            assert_eq!(actual_height, 3);
        }
        other => panic!("unexpected store error after prune: {other}"),
    }
    prune_thread.join().expect("join prune thread");
    store_thread.join().expect("join store thread");
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
}
#[test]
fn prune_intent_fault_matrix_reopens_at_one_coherent_committed_boundary() {
    // The first-release prune protocol uses the forward-recovery prune
    // intent. Reuse its exhaustive crash-boundary matrix here so the
    // formerly rollback-named regression continues to cover the selected
    // production protocol without retaining a second writer path.
    prune_crash_boundaries_recover_forward_and_poison_live_kura();
}
#[test]
fn prune_unfinalized_suffix_removes_stale_sidecars_above_new_tip() {
    let (_temp_dir, config) =
        unwrapped_kura_storage_fixture(NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let blocks = store_dummy_block_arcs(&kura, 4);
    let block2_hash = blocks[1].hash();
    let block3_hash = blocks[2].hash();
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let (block2_sidecar, block3_sidecar) = {
        let store = kura.block_store.lock();
        store
            .write_da_block_bytes(2, &blocks[1].encode_wire().expect("block-two wire"))
            .expect("plant unfinalized block-two cache candidate");
        store
            .write_da_block_bytes(3, &blocks[2].encode_wire().expect("block-three wire"))
            .expect("plant unfinalized block-three cache candidate");
        (store.da_block_path(2), store.da_block_path(3))
    };
    kura.persist_retained_block_record(&blocks_dir, block2_hash, blocks[1].as_ref())
        .expect("plant unfinalized block-two retained record");
    kura.persist_retained_block_record(&blocks_dir, block3_hash, blocks[2].as_ref())
        .expect("plant unfinalized block-three retained record");
    let block2_retained = kura.retained_block_record_path(2);
    let block3_retained = kura.retained_block_record_path(3);
    assert!(block2_sidecar.exists());
    assert!(block3_sidecar.exists());
    assert!(block2_retained.exists());
    assert!(block3_retained.exists());
    kura.prune_to_height(2).expect("prune to height 2");
    assert!(
        block2_sidecar.exists(),
        "sidecar at the retained tip should stay available"
    );
    assert!(
        !block3_sidecar.exists(),
        "sidecar above the pruned tip should be removed"
    );
    assert!(
        block2_retained.exists(),
        "retained block evidence at the retained tip should stay available"
    );
    assert!(
        !block3_retained.exists(),
        "retained block evidence above the pruned tip should be removed"
    );
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(2_usize)),
        Some(block2_hash)
    );
    assert_eq!(kura.get_durable_block_hash(nonzero!(3_usize)), None);
    kura.block_data.lock()[1].1 = None;
    assert_eq!(
        kura.block_body_status_by_hash(block2_hash),
        Some(BlockBodyStatus::Inline)
    );
    assert_eq!(kura.block_body_status_by_hash(block3_hash), None);
}
#[test]
fn wsv_checkpoint_roundtrips_and_requires_matching_block_hash() {
    let kura = Kura::blank_kura_for_testing();
    let blocks = store_dummy_block_arcs(&kura, 2);
    let state_hash = Hash::new(b"canonical state surface");
    assert!(
        !kura
            .has_wsv_checkpoint_at_or_before(2)
            .expect("scan before checkpoint")
    );
    assert_eq!(
        kura.latest_wsv_checkpoint_height_at_or_before(2)
            .expect("latest before checkpoint"),
        None
    );
    kura.store_wsv_checkpoint(2, blocks[1].hash(), state_hash)
        .expect("store checkpoint");
    let checkpoint = kura
        .wsv_checkpoint(2)
        .expect("read checkpoint")
        .expect("checkpoint present");
    assert_eq!(checkpoint.state_hash(), state_hash);
    assert!(
        kura.has_wsv_checkpoint_at_or_before(2)
            .expect("scan after checkpoint")
    );
    assert_eq!(
        kura.latest_wsv_checkpoint_height_at_or_before(1)
            .expect("latest below checkpoint"),
        None
    );
    assert_eq!(
        kura.latest_wsv_checkpoint_height_at_or_before(2)
            .expect("latest at checkpoint"),
        Some(2)
    );
    assert_eq!(
        kura.latest_wsv_checkpoint_height_at_or_before(3)
            .expect("latest above checkpoint"),
        Some(2)
    );
    assert!(
        kura.wsv_checkpoint(1)
            .expect("missing lower checkpoint")
            .is_none()
    );
    let err = kura
        .store_wsv_checkpoint(2, blocks[0].hash(), Hash::new(b"wrong block"))
        .expect_err("checkpoint must match durable block hash");
    assert!(matches!(err, Error::BlockHeightConflict { height: 2, .. }));
}
#[test]
fn unbound_wsv_checkpoint_identity_is_immutable() {
    let kura = Kura::blank_kura_for_testing();
    let blocks = store_dummy_block_arcs(&kura, 1);
    let block_hash = blocks[0].hash();
    let original_state_hash = Hash::new(b"pre-commit staged state");
    kura.store_wsv_checkpoint(1, block_hash, original_state_hash)
        .expect("store unbound staged checkpoint");
    kura.store_wsv_checkpoint(1, block_hash, original_state_hash)
        .expect("an exact replay may confirm the unbound checkpoint");
    let error = kura
        .store_wsv_checkpoint(1, block_hash, Hash::new(b"divergent replay state"))
        .expect_err("an unbound checkpoint must already be immutable");
    assert!(
        matches!(
            error,
            Error::NoritoFrame(norito::core::Error::Message(ref message))
                if message.contains("immutable WSV checkpoint #1")
        ),
        "unexpected divergent replay rejection: {error:?}"
    );
    assert_eq!(
        kura.wsv_checkpoint(1)
            .expect("read checkpoint")
            .expect("checkpoint remains present")
            .state_hash(),
        original_state_hash,
        "a rejected replay must not replace the durable identity"
    );
}
#[test]
fn commit_manifest_roundtrips_and_requires_matching_block_hash() {
    let kura = Kura::blank_kura_for_testing();
    let blocks = store_dummy_block_arcs(&kura, 2);
    let parent_state_root = Hash::new(b"parent state root");
    let post_state_root = Hash::new(b"post state root");
    let wsv_checkpoint_hash = Hash::new(b"canonical memory wsv");
    let commit_qc_hash = Hash::new(b"commit qc");
    let manifest = CommitManifest::new(
        2,
        blocks[1].hash(),
        Some(parent_state_root),
        Some(post_state_root),
        wsv_checkpoint_hash,
        Some(commit_qc_hash),
    );
    kura.store_commit_manifest(manifest.clone())
        .expect("store commit manifest");
    let stored = kura
        .commit_manifest(2)
        .expect("read commit manifest")
        .expect("commit manifest present");
    assert_eq!(stored, manifest);
    assert_eq!(stored.height, 2);
    assert_eq!(stored.block_hash, blocks[1].hash());
    assert_eq!(stored.wsv_checkpoint_hash, wsv_checkpoint_hash);
    assert!(
        kura.commit_manifest(1)
            .expect("missing lower manifest")
            .is_none()
    );
    let err = kura
        .store_commit_manifest(CommitManifest::new(
            2,
            blocks[0].hash(),
            None,
            None,
            Hash::new(b"wrong block"),
            None,
        ))
        .expect_err("manifest must match durable block hash");
    assert!(matches!(err, Error::BlockHeightConflict { height: 2, .. }));
}
#[test]
fn commit_manifest_requires_matching_wsv_checkpoint_hash() {
    let kura = Kura::blank_kura_for_testing();
    let blocks = store_dummy_block_arcs(&kura, 1);
    let checkpoint_hash = Hash::new(b"checkpoint hash");
    kura.store_wsv_checkpoint(1, blocks[0].hash(), checkpoint_hash)
        .expect("store checkpoint");
    let err = kura
        .store_commit_manifest(CommitManifest::new(
            1,
            blocks[0].hash(),
            None,
            None,
            Hash::new(b"different checkpoint hash"),
            None,
        ))
        .expect_err("manifest must match the checkpoint sidecar when present");
    assert!(matches!(
        err,
        Error::NoritoFrame(norito::core::Error::Message(message))
            if message.contains("WSV checkpoint hash mismatch")
    ));
    let path = kura.commit_manifest_path(1);
    std::fs::create_dir_all(path.parent().expect("manifest parent")).expect("create manifest dir");
    std::fs::write(
        &path,
        CommitManifest::new(
            1,
            blocks[0].hash(),
            None,
            None,
            Hash::new(b"different checkpoint hash"),
            None,
        )
        .encode(),
    )
    .expect("write mismatched manifest");
    let err = kura
        .commit_manifest(1)
        .expect_err("manifest read must validate checkpoint sidecar");
    assert!(matches!(
        err,
        Error::NoritoFrame(norito::core::Error::Message(message))
            if message.contains("WSV checkpoint hash mismatch")
    ));
}
#[test]
fn commit_manifest_roots_require_v2_finality_binding_after_correlated_sidecar_tamper() {
    let kura = Kura::blank_kura_for_testing();
    let blocks = store_dummy_block_arcs(&kura, 1);
    let block_hash = blocks[0].hash();
    let checkpoint_hash = Hash::new(b"checkpoint hash");
    kura.store_wsv_checkpoint(1, block_hash, checkpoint_hash)
        .expect("store checkpoint");
    let artifact = v2_finality_artifact_for_block(blocks[0].as_ref());
    let manifest = CommitManifest::new(1, block_hash, None, None, checkpoint_hash, None)
        .with_authenticated_v2_commit_authority(&artifact);
    assert!(manifest.binds_authenticated_v2_commit_authority(&artifact));
    kura.store_commit_manifest(manifest.clone())
        .expect("store bound manifest");
    assert!(
        kura.commit_manifest_has_wsv_binding(&manifest)
            .expect("check WSV binding"),
        "checkpoint must bind the complete durable manifest"
    );
    let mut tampered = manifest;
    tampered.parent_state_root = Some(Hash::new(b"tampered parent root"));
    tampered.post_state_root = Some(Hash::new(b"tampered post root"));
    assert!(
        !tampered.binds_authenticated_v2_commit_authority(&artifact),
        "the v2 authority seal must not bless altered root fields"
    );
    std::fs::write(kura.commit_manifest_path(1), tampered.encode()).expect("tamper manifest roots");
    let decoded = kura
        .commit_manifest(1)
        .expect("read structurally valid tampered manifest")
        .expect("tampered manifest remains present");
    assert!(
        !kura
            .commit_manifest_has_wsv_binding(&decoded)
            .expect("check tampered WSV binding"),
        "tampered roots must invalidate the external WSV binding"
    );
    assert_eq!(
        kura.commit_manifest_binding_state(&decoded)
            .expect("classify tampered WSV binding"),
        CommitManifestBindingState::Mismatched,
        "a published different digest is corruption, not a resumable unbound window"
    );
    // A correlated local tamper can rewrite both mutable sidecars consistently. This proves the
    // WSV cross-link is useful for diagnostics and crash detection, but is not an authenticated
    // root anchor for a safety-halt decision.
    let checkpoint_path = kura.wsv_checkpoint_path(1);
    let mut correlated_checkpoint = Kura::decode_wsv_checkpoint_at(&checkpoint_path)
        .expect("decode checkpoint for correlated tamper")
        .expect("checkpoint remains present");
    correlated_checkpoint.commit_manifest_hash = Some(tampered.encoded_hash());
    std::fs::write(&checkpoint_path, correlated_checkpoint.encode())
        .expect("correlate checkpoint with tampered manifest");
    let correlated = kura
        .commit_manifest(1)
        .expect("read correlated manifest")
        .expect("correlated manifest remains present");
    assert!(
        kura.commit_manifest_has_wsv_binding(&correlated)
            .expect("check correlated WSV binding"),
        "a correlated two-file rewrite can preserve the diagnostic cross-link"
    );
    assert_eq!(
        kura.commit_manifest_binding_state(&correlated)
            .expect("classify correlated WSV binding"),
        CommitManifestBindingState::Bound,
    );
    assert!(
        !correlated.binds_authenticated_v2_commit_authority(&artifact),
        "correlated mutable sidecars must not replace exact authenticated-v2 root binding"
    );
}
#[test]
fn commit_manifest_v2_authority_binds_exact_artifact_and_execution_roots() {
    let block = DummyBlocks::new().next();
    let artifact = v2_finality_artifact_for_block(block.as_ref());
    let commitment = artifact.commit_qc.execution_commitment;
    let manifest = CommitManifest::new(
        artifact.height,
        artifact.block_hash,
        None,
        None,
        Hash::new(b"v2 checkpoint"),
        None,
    )
    .with_authenticated_v2_commit_authority(&artifact);
    assert_eq!(
        manifest.parent_state_root.zip(manifest.post_state_root),
        Some((commitment.parent_state_root, commitment.post_state_root))
    );
    assert!(manifest.binds_authenticated_v2_commit_authority(&artifact));
    let mut altered_artifact = artifact.clone();
    altered_artifact.commit_qc.aggregate_signature[0] ^= 0x01;
    assert!(
        !manifest.binds_authenticated_v2_commit_authority(&altered_artifact),
        "changing authenticated certificate bytes must invalidate the authority seal"
    );
    let mut altered_manifest = manifest.clone();
    altered_manifest.post_state_root = Some(Hash::new(b"altered post root"));
    assert!(
        !altered_manifest.binds_authenticated_v2_commit_authority(&artifact),
        "the authority seal must not bless altered replay roots"
    );
}
#[test]
fn published_commit_manifest_digest_cannot_be_erased_or_replaced() {
    let kura = Kura::blank_kura_for_testing();
    let blocks = store_dummy_block_arcs(&kura, 1);
    let block_hash = blocks[0].hash();
    let state_hash = Hash::new(b"published manifest state");
    kura.store_wsv_checkpoint(1, block_hash, state_hash)
        .expect("store checkpoint");
    let manifest = CommitManifest::new(
        1,
        block_hash,
        Some(Hash::new(b"published parent")),
        Some(Hash::new(b"published post")),
        state_hash,
        None,
    );
    kura.store_commit_manifest(manifest.clone())
        .expect("publish manifest");
    kura.store_wsv_checkpoint(1, block_hash, state_hash)
        .expect("identical checkpoint retry must preserve binding");
    let retained = kura
        .commit_manifest(1)
        .expect("read retained manifest")
        .expect("manifest remains present");
    assert_eq!(retained, manifest);
    assert_eq!(
        kura.commit_manifest_binding_state(&retained)
            .expect("binding survives checkpoint retry"),
        CommitManifestBindingState::Bound,
    );
    let replacement = CommitManifest::new(
        1,
        block_hash,
        Some(Hash::new(b"replacement parent")),
        Some(Hash::new(b"published post")),
        state_hash,
        None,
    );
    assert!(kura.store_commit_manifest(replacement).is_err());
    assert!(
        kura.store_wsv_checkpoint(1, block_hash, Hash::new(b"replacement state"))
            .is_err()
    );
    assert_eq!(
        kura.commit_manifest(1)
            .expect("read manifest after rejected replacements"),
        Some(manifest),
    );
}
#[test]
fn kura_reopen_rejects_missing_or_corrupt_published_manifest_binding() {
    for corrupt_checkpoint in [false, true] {
        let (_temp_dir, config) =
            kura_storage_fixture("tempdir", NonZeroUsize::new(1).expect("non-zero"));
        {
            let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
                &config,
                &RuntimeLaneConfig::default(),
            )
            .expect("initialize Kura");
            let blocks = store_dummy_block_arcs(&kura, 1);
            let block_hash = blocks[0].hash();
            let state_hash = Hash::new(b"published reopen state");
            kura.store_wsv_checkpoint(1, block_hash, state_hash)
                .expect("store checkpoint");
            kura.store_commit_manifest(CommitManifest::new(
                1,
                block_hash,
                Some(Hash::new(b"published reopen parent")),
                Some(Hash::new(b"published reopen post")),
                state_hash,
                None,
            ))
            .expect("publish manifest");
            if corrupt_checkpoint {
                std::fs::write(kura.wsv_checkpoint_path(1), b"corrupt checkpoint")
                    .expect("corrupt published checkpoint");
            } else {
                kura.remove_commit_manifest_without_binding_for_tests(1)
                    .expect("remove published manifest");
            }
        }
        let error = match Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        ) {
            Ok(_) => panic!("published binding corruption must fail Kura reopen"),
            Err(error) => error,
        };
        if corrupt_checkpoint {
            assert!(matches!(error, Error::NoritoFrame(_)), "{error:?}");
        } else {
            assert!(
                matches!(
                    error,
                    Error::NoritoFrame(norito::core::Error::Message(ref message))
                        if message.contains("manifest is missing")
                ),
                "{error:?}"
            );
        }
    }
}
#[test]
fn commit_manifest_survives_kura_reopen_and_is_validated_on_init() {
    let (_temp_dir, config) =
        kura_storage_fixture("tempdir", NonZeroUsize::new(1).expect("non-zero"));
    let blocks = {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 2);
        kura.store_commit_manifest(CommitManifest::new(
            1,
            blocks[0].hash(),
            Some(Hash::new(b"genesis parent root")),
            Some(Hash::new(b"genesis post root")),
            Hash::new(b"genesis checkpoint"),
            Some(Hash::new(b"genesis commit qc")),
        ))
        .expect("store genesis commit manifest");
        kura.store_commit_manifest(CommitManifest::new(
            2,
            blocks[1].hash(),
            Some(Hash::new(b"parent root")),
            Some(Hash::new(b"post root")),
            Hash::new(b"checkpoint"),
            Some(Hash::new(b"commit qc")),
        ))
        .expect("store commit manifest");
        blocks
    };
    let (reopened, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("reopen kura");
    let stored = reopened
        .commit_manifest(2)
        .expect("read manifest after reopen")
        .expect("manifest present after reopen");
    assert_eq!(stored.block_hash, blocks[1].hash());
    assert_eq!(stored.wsv_checkpoint_hash, Hash::new(b"checkpoint"));
}
#[test]
fn kura_init_rejects_mismatched_retained_commit_manifest() {
    let (_temp_dir, config) =
        kura_storage_fixture("tempdir", NonZeroUsize::new(1).expect("non-zero"));
    {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 2);
        let path = kura.commit_manifest_path(1);
        std::fs::create_dir_all(path.parent().expect("manifest parent"))
            .expect("create manifest dir");
        let bad_manifest = CommitManifest::new(
            1,
            blocks[1].hash(),
            None,
            None,
            Hash::new(b"wrong checkpoint"),
            None,
        );
        std::fs::write(&path, bad_manifest.encode()).expect("write bad manifest");
    }
    let error = match Kura::open_test_kura_with_configured_lane_config(
        &config,
        &RuntimeLaneConfig::default(),
    ) {
        Ok(_) => panic!("retained manifest mismatch must fail Kura initialization"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::BlockHeightConflict { height: 1, .. }
    ));
}
#[test]
fn kura_init_rejects_mismatched_retained_checkpoint_and_manifest() {
    let (_temp_dir, config) =
        kura_storage_fixture("tempdir", NonZeroUsize::new(1).expect("non-zero"));
    {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 1);
        let block_hash = blocks[0].hash();
        kura.store_wsv_checkpoint(1, block_hash, Hash::new(b"checkpoint"))
            .expect("store checkpoint");
        let path = kura.commit_manifest_path(1);
        std::fs::create_dir_all(path.parent().expect("manifest parent"))
            .expect("create manifest dir");
        let bad_manifest = CommitManifest::new(
            1,
            block_hash,
            None,
            None,
            Hash::new(b"different checkpoint"),
            None,
        );
        std::fs::write(&path, bad_manifest.encode()).expect("write bad manifest");
    }
    let error = match Kura::open_test_kura_with_configured_lane_config(
        &config,
        &RuntimeLaneConfig::default(),
    ) {
        Ok(_) => panic!("retained checkpoint/manifest mismatch must fail Kura initialization"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::NoritoFrame(_)), "{error:?}");
}
#[test]
fn kura_init_prunes_checkpoint_above_durable_blocks_without_manifests() {
    let (_temp_dir, config) =
        kura_storage_fixture("tempdir", NonZeroUsize::new(1).expect("non-zero"));
    {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 1);
        let path = kura.wsv_checkpoint_path(2);
        std::fs::create_dir_all(path.parent().expect("checkpoint parent"))
            .expect("create checkpoint dir");
        let stale = WsvCheckpoint::new(2, blocks[0].hash(), Hash::new(b"stale checkpoint"));
        std::fs::write(&path, stale.encode()).expect("write stale checkpoint");
    }
    let (reopened, count) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("reopen kura");
    assert_eq!(count.0, 1);
    assert!(
        reopened
            .wsv_checkpoint(2)
            .expect("stale checkpoint should be pruned")
            .is_none()
    );
}
#[test]
fn kura_init_prunes_commit_manifests_above_recovered_tip() {
    let (_temp_dir, config) =
        kura_storage_fixture("tempdir", NonZeroUsize::new(1).expect("non-zero"));
    {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 2);
        kura.store_commit_manifest(CommitManifest::new(
            1,
            blocks[0].hash(),
            None,
            None,
            Hash::new(b"checkpoint 1"),
            None,
        ))
        .expect("store manifest 1");
        kura.store_commit_manifest(CommitManifest::new(
            2,
            blocks[1].hash(),
            None,
            None,
            Hash::new(b"checkpoint 2"),
            None,
        ))
        .expect("store manifest 2");
        let path = kura.commit_manifest_path(3);
        std::fs::create_dir_all(path.parent().expect("manifest parent"))
            .expect("create manifest dir");
        let stale_manifest = CommitManifest::new(
            3,
            blocks[1].hash(),
            None,
            None,
            Hash::new(b"stale checkpoint"),
            None,
        );
        std::fs::write(&path, stale_manifest.encode()).expect("write stale manifest");
    }
    let (reopened, count) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("reopen kura");
    assert_eq!(count.0, 2);
    assert!(
        reopened
            .commit_manifest(3)
            .expect("read pruned manifest")
            .is_none()
    );
}
