#[test]
fn sidecar_index_rejects_overflowing_based_header() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
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
    let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
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
fn block_store_reads_do_not_recreate_missing_journals() {
    for missing_name in [DATA_FILE_NAME, INDEX_FILE_NAME, HASHES_FILE_NAME] {
        let dir = TempDir::new().expect("temporary store");
        let mut store = BlockStore::new(dir.path());
        store
            .create_files_if_they_do_not_exist()
            .expect("initialize journals");
        store
            .append_block_to_chain(&NativeBlocks::new().next())
            .expect("append nonempty block");
        store.drop_cached_handles();
        let missing_path = dir.path().join(missing_name);
        fs::remove_file(&missing_path).expect("remove journal after closing cached handles");
        let unchanged = [
            DATA_FILE_NAME,
            INDEX_FILE_NAME,
            HASHES_FILE_NAME,
            COUNT_FILE_NAME,
        ]
        .into_iter()
        .filter(|name| *name != missing_name)
        .map(|name| {
            let path = dir.path().join(name);
            let bytes = fs::read(&path).expect("read remaining canonical journal");
            (path, bytes)
        })
        .collect::<Vec<_>>();

        let reads = match missing_name {
            DATA_FILE_NAME => vec![
                store.data_file_len().map(|_| ()),
                store.read_block_data(0, &mut [0_u8; 1]),
                store.block_bytes(0, 1).map(|_| ()),
            ],
            INDEX_FILE_NAME => vec![
                store.read_block_index(0).map(|_| ()),
                store.read_index_count().map(|_| ()),
                store.read_exact_durable_index_count().map(|_| ()),
            ],
            HASHES_FILE_NAME => vec![
                store.read_block_hashes(0, 1).map(|_| ()),
                store.read_hashes_count().map(|_| ()),
                store.read_exact_durable_index_count().map(|_| ()),
            ],
            _ => unreachable!("only canonical journals are removed"),
        };
        for result in reads {
            let error = result.expect_err("a read must reject its missing canonical journal");
            assert!(
                matches!(&error, Error::IO(cause, path)
                    if cause.kind() == ErrorKind::NotFound && *path == missing_path),
                "unexpected {missing_name} read failure: {error:?}",
            );
        }
        assert!(
            !missing_path.exists(),
            "reads must not recreate missing {missing_name}",
        );
        for (path, bytes) in unchanged {
            assert_eq!(
                fs::read(&path).expect("read unchanged canonical journal"),
                bytes,
                "rejecting a missing journal must not repair {}",
                path.display(),
            );
        }
    }
}

#[test]
fn block_store_existing_journals_reopen_for_reads_and_writes() {
    let dir = TempDir::new().expect("temporary store");
    let mut store = BlockStore::new(dir.path());
    store
        .create_files_if_they_do_not_exist()
        .expect("explicit initialization creates journals");
    assert_eq!(store.read_exact_durable_index_count().unwrap(), 0);
    let mut blocks = NativeBlocks::new();
    let first = blocks.next();
    store
        .append_block_to_chain(&first)
        .expect("write newly initialized journals");
    store.drop_cached_handles();

    let first_index = store.read_block_index(0).expect("reopen existing index");
    assert_eq!(
        store
            .read_block_hashes(0, 1)
            .expect("reopen existing hashes"),
        [first.hash()],
    );
    let first_wire = first.encode_wire().expect("encode first block");
    assert_eq!(
        store
            .block_bytes(first_index.start, first_index.length)
            .expect("reopen existing data"),
        first_wire,
    );

    let second = blocks.next();
    store
        .append_block_to_chain(&second)
        .expect("read-reopened handles remain writable");
    store.drop_cached_handles();
    assert_eq!(store.read_exact_durable_index_count().unwrap(), 2);
    assert_eq!(
        store.read_block_hashes(0, 2).unwrap(),
        [first.hash(), second.hash()],
    );
    let second_index = store.read_block_index(1).expect("read appended index");
    assert_eq!(
        store
            .block_bytes(second_index.start, second_index.length)
            .expect("read appended block"),
        second.encode_wire().expect("encode second block"),
    );
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
fn strict_init_rejects_oversized_block_length_without_pruning() {
    let dir = TempDir::new().unwrap();
    let mut store = BlockStore::new(dir.path());
    store.create_files_if_they_do_not_exist().unwrap();
    let mut blocks = NativeBlocks::new();
    let block = blocks.next();
    store.append_block_to_chain(&block).unwrap();
    let BlockIndex { start, .. } = store.read_block_index(0).unwrap();
    let huge_len = STRICT_INIT_MAX_BLOCK_BYTES + 1;
    store.write_block_index(0, start, huge_len).unwrap();
    let before = fs::read(dir.path().join(INDEX_FILE_NAME)).unwrap();
    assert!(matches!(
        Kura::init_canonical_chain(&mut store, 1),
        Err(Error::CorruptedBlockLength { .. })
    ));
    assert_eq!(store.read_index_count().unwrap(), 1);
    assert_eq!(fs::read(dir.path().join(INDEX_FILE_NAME)).unwrap(), before);
}
#[test]
fn strict_init_does_not_prune_committed_corruption() {
    let dir = TempDir::new().unwrap();
    let mut store = BlockStore::new(dir.path());
    store.create_files_if_they_do_not_exist().unwrap();
    let mut blocks = NativeBlocks::new();
    let first = blocks.next();
    let carrier = blocks.next();
    store.append_block_to_chain(&first).unwrap();
    store.append_block_to_chain(&carrier).unwrap();
    let BlockIndex { start, .. } = store.read_block_index(0).unwrap();
    store
        .write_block_index(0, start, STRICT_INIT_MAX_BLOCK_BYTES + 1)
        .unwrap();
    let index_path = dir.path().join(INDEX_FILE_NAME);
    let original_index = fs::read(&index_path).expect("read corrupt index before strict init");
    assert!(matches!(
        Kura::init_canonical_chain(&mut store, 2),
        Err(Error::CorruptedBlockLength { .. })
    ));
    assert_eq!(store.read_index_count().unwrap(), 2);
    assert_eq!(
        fs::read(&index_path).expect("read preserved corrupt index after strict init"),
        original_index,
        "strict init must leave the canonical journal byte-exact after corruption of a committed entry",
    );
}
#[test]
fn strict_init_rejects_a_corrupt_committed_hash_suffix() {
    let dir = TempDir::new().unwrap();
    let mut store = new_block_store(&dir);
    store.create_files_if_they_do_not_exist().unwrap();
    let mut generator = NativeBlocks::new();
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
        .expect("corrupt only the final committed hash");
    let corrupt_bytes = fs::read(&hashes_path).unwrap();
    assert!(matches!(
        Kura::init_canonical_chain(&mut store, 3),
        Err(Error::CanonicalBlockWireMismatch { height: 3 })
    ));
    let preserved = fs::read(&hashes_path).unwrap();
    assert_eq!(
        &preserved[..finalized_prefix_len],
        finalized_prefix.as_slice()
    );
    assert_eq!(preserved, corrupt_bytes);
    assert_ne!(preserved, pristine_bytes);
}
#[test]
fn strict_init_rejects_a_missing_committed_hash_suffix() {
    let dir = TempDir::new().unwrap();
    let mut store = new_block_store(&dir);
    store.create_files_if_they_do_not_exist().unwrap();
    let mut generator = NativeBlocks::new();
    let blocks = vec![generator.next(), generator.next(), generator.next()];
    for block in &blocks {
        store.append_block_to_chain(block).unwrap();
    }
    let hashes_path = primary_blocks_dir(&dir).join(HASHES_FILE_NAME);
    let pristine_bytes = std::fs::read(&hashes_path).expect("read pristine hash journal");
    let finalized_prefix_len = usize::try_from(2 * SIZE_OF_BLOCK_HASH).unwrap();
    store
        .truncate_hashes_to_count(2)
        .expect("remove only the final committed hash");
    assert!(matches!(
        Kura::init_canonical_chain(&mut store, 3),
        Err(Error::HashesFileHeightMismatch)
    ));
    assert_eq!(
        fs::read(&hashes_path).unwrap(),
        pristine_bytes[..finalized_prefix_len]
    );
}
#[test]
fn zero_length_hash_metadata_is_diagnosed_as_missing_canonical_body() {
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
    assert!(kura.is_canonical_body_missing(nonzero!(2_usize)));
    assert!(
        kura.get_block(
            nonzero!(2_usize),
            &crate::state::AllocationBudget::new(64 * 1024 * 1024)
        )
        .expect("completed structural storage read")
        .is_none()
    );
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
fn commit_marker_rejects_missing_committed_journals_without_repair() {
    for (name, retained_bytes) in [
        (HASHES_FILE_NAME, 3 * SIZE_OF_BLOCK_HASH),
        (INDEX_FILE_NAME, 3 * BlockIndex::SIZE),
        (DATA_FILE_NAME, 0),
    ] {
        for unlink in [false, true] {
            let temp_dir = TempDir::new().unwrap();
            populate_raw_block_store(&temp_dir, 4);
            let root = primary_blocks_dir(&temp_dir);
            if unlink {
                fs::remove_file(root.join(name)).unwrap();
            } else {
                fs::OpenOptions::new()
                    .write(true)
                    .open(root.join(name))
                    .unwrap()
                    .set_len(retained_bytes)
                    .unwrap();
            }
            let files = [
                INDEX_FILE_NAME,
                HASHES_FILE_NAME,
                DATA_FILE_NAME,
                COUNT_FILE_NAME,
            ];
            let before = files.map(|file| fs::read(root.join(file)).ok());
            let mut reopened = new_block_store(&temp_dir);
            assert!(
                reopened.create_files_if_they_do_not_exist().is_err(),
                "missing committed {name} must not shrink the durable boundary"
            );
            for (file, expected) in files.into_iter().zip(before) {
                assert_eq!(
                    fs::read(root.join(file)).ok(),
                    expected,
                    "failed recovery changed {file} after damage to {name}"
                );
            }
        }
    }
}
#[test]
fn strict_init_rejects_corrupted_index_end_to_end() {
    for damage in ["oversized", "wrong-offset", "shortened-frame"] {
        let temp_dir = TempDir::new().unwrap();
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &RuntimeLaneConfig::default());
        drop(kura);
        let mut store = new_block_store(&temp_dir);
        store.create_files_if_they_do_not_exist().unwrap();
        let mut blocks = NativeBlocks::new();
        store.append_block_to_chain(&blocks.next()).unwrap();
        store.append_block_to_chain(&blocks.next()).unwrap();
        let original = store.read_block_index(1).unwrap();
        let (start, length) = match damage {
            "oversized" => (original.start, STRICT_INIT_MAX_BLOCK_BYTES + 1),
            "wrong-offset" => (0, original.length),
            _ => (original.start, original.length - 1),
        };
        store.write_block_index(1, start, length).unwrap();
        assert_eq!(store.read_durable_index_count().unwrap(), 2);
        drop(store);
        let root = primary_blocks_dir(&temp_dir);
        let files = [
            INDEX_FILE_NAME,
            HASHES_FILE_NAME,
            DATA_FILE_NAME,
            COUNT_FILE_NAME,
        ];
        let before = files.map(|file| fs::read(root.join(file)).unwrap());
        assert!(
            Kura::open_test_kura_with_configured_lane_config(
                &config,
                &RuntimeLaneConfig::default()
            )
            .is_err(),
            "strict startup must reject {damage}"
        );
        for (file, expected) in files.into_iter().zip(before) {
            assert_eq!(
                fs::read(root.join(file)).unwrap(),
                expected,
                "strict startup changed {file} after {damage}"
            );
        }
    }
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
    let mut blocks = NativeBlocks::new();
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
