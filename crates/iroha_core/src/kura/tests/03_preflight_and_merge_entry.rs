// Exact current canonical storage preflight and structural index controls.
#[test]
fn fresh_single_lane_preflight_rejects_nonempty_root_without_mutation() {
    let temp = TempDir::new().expect("temporary parent directory");
    let store_root = temp.path().join("nonempty-kura");
    fs::create_dir(&store_root).expect("create nonempty root");
    let sentinel = store_root.join("blocks.data");
    let sentinel_bytes = b"unbound Kura bytes";
    fs::write(&sentinel, sentinel_bytes).expect("write unbound sentinel");
    let entries_before = fs::read_dir(&store_root)
        .expect("read nonempty root")
        .map(|entry| entry.expect("unbound entry").file_name())
        .collect::<Vec<_>>();
    let config = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let error = Kura::new_fresh_single_lane(&config, &RuntimeLaneConfig::default())
        .expect_err("a nonempty root without a catalog journal must fail closed");
    assert!(matches!(
        error,
        Error::IO(ref source, ref path)
            if source.kind() == ErrorKind::InvalidData
                && source.to_string().contains("nonempty store")
                && source.to_string().contains("new_with_configured_lane_catalog")
                && path == &sentinel
    ));
    assert_eq!(
        fs::read(&sentinel).expect("read unchanged sentinel"),
        sentinel_bytes
    );
    assert_eq!(
        fs::read_dir(&store_root)
            .expect("read rejected nonempty root")
            .map(|entry| entry.expect("unbound entry").file_name())
            .collect::<Vec<_>>(),
        entries_before,
        "rejected nonempty root must not gain geometry or storage artifacts"
    );
}

#[test]
fn fresh_single_lane_preflight_accepts_missing_or_empty_default_root_without_mutation() {
    let temp = TempDir::new().expect("temporary parent directory");
    let missing_root = temp.path().join("missing-kura");
    let missing_config = kura_config_for_path(&missing_root, BLOCKS_IN_MEMORY);
    Kura::validate_fresh_single_lane_store(&missing_config, &RuntimeLaneConfig::default())
        .expect("missing canonical root is fresh");
    assert!(!missing_root.exists());
    let empty_root = temp.path().join("empty-kura");
    fs::create_dir(&empty_root).expect("create empty canonical root");
    let empty_config = kura_config_for_path(&empty_root, BLOCKS_IN_MEMORY);
    Kura::validate_fresh_single_lane_store(&empty_config, &RuntimeLaneConfig::default())
        .expect("empty canonical root is fresh");
    assert!(
        fs::read_dir(&empty_root)
            .expect("read empty root")
            .next()
            .is_none(),
        "fresh-store validation itself must not provision storage"
    );
}

fn publish_configured_catalog_baseline(kura: &Kura, catalog: &LaneCatalog) {
    kura.bind_lane_storage_network(native_storage_network_id())
        .expect("bind the configured-catalog fixture network before H0 admission");
    let lane_config = RuntimeLaneConfig::from_catalog(catalog);
    let incarnations = BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0xA1; Hash::LENGTH]))]);
    let activation_heights = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let baseline = LaneLifecycleParameterV1::catalog_hash(catalog);
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        incarnations[&LaneId::SINGLE],
        baseline,
    )
    .expect("anchor configured primary geometry");
    kura.mark_lane_geometry_catalog_published(
        &lane_config,
        &incarnations,
        &activation_heights,
        Some(baseline),
    )
    .expect("publish configured lane catalog baseline");
}


#[cfg(unix)]
#[test]
fn configured_primary_open_rejects_store_root_inode_swap_before_block_open() {
    let temp = TempDir::new().expect("temporary directory");
    let store_root = temp.path().join("kura");
    let config = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let configured = configured_primary_catalog("root-identity");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
        .expect("establish authenticated configured primary");
    publish_configured_catalog_baseline(&kura, &configured);
    drop(kura);
    let expected = snapshot_regular_test_tree(&store_root);
    let replacement = configured_primary_open_identity_test_path(
        &store_root,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_SWAP_SUFFIX,
    )
    .expect("root replacement path");
    copy_regular_test_tree(&store_root, &replacement);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
        .expect_err("a post-preflight store-root replacement must fail closed");
    assert!(matches!(
        error,
        Error::IO(ref source, _)
            if source.kind() == ErrorKind::InvalidData
                && source.to_string().contains("store root changed")
    ));
    assert_eq!(
        snapshot_regular_test_tree(&store_root),
        expected,
        "Kura must reject the replacement root before opening its block store"
    );
    let displaced = configured_primary_open_identity_test_path(
        &store_root,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_DISPLACED_SUFFIX,
    )
    .expect("displaced root path");
    assert_eq!(snapshot_regular_test_tree(&displaced), expected);
}

#[cfg(unix)]
#[test]
fn configured_primary_open_rejects_block_directory_inode_swap_before_mutation() {
    let temp = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let configured = configured_primary_catalog("blocks-identity");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
        .expect("establish authenticated configured primary");
    publish_configured_catalog_baseline(&kura, &configured);
    let blocks = kura
        .lane_storage_entry(LaneId::SINGLE)
        .unwrap()
        .blocks_dir(kura.store_root());
    drop(kura);
    let expected = snapshot_regular_test_tree(&blocks);
    let replacement = configured_primary_open_identity_test_path(
        &blocks,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_SWAP_SUFFIX,
    )
    .expect("block replacement path");
    copy_regular_test_tree(&blocks, &replacement);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
        .expect_err("a post-preflight block-directory replacement must fail closed");
    assert!(matches!(
        error,
        Error::IO(ref source, _)
            if source.kind() == ErrorKind::InvalidData
                && source.to_string().contains("path identity changed")
    ));
    assert_eq!(
        snapshot_regular_test_tree(&blocks),
        expected,
        "BlockStore must not create or rewrite files in the replacement directory"
    );
    let displaced = configured_primary_open_identity_test_path(
        &blocks,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_DISPLACED_SUFFIX,
    )
    .expect("displaced block path");
    assert_eq!(snapshot_regular_test_tree(&displaced), expected);
}

#[cfg(unix)]
#[test]
fn canonical_storage_open_rejects_block_directory_inode_swap_before_mutation() {
    let temp = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let configured = configured_primary_catalog("blocks-identity");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
        .expect("establish authenticated configured primary");
    publish_configured_catalog_baseline(&kura, &configured);
    drop(kura);
    let blocks = Kura::canonical_storage_path(temp.path());
    let expected = snapshot_regular_test_tree(&blocks);
    let replacement = configured_primary_open_identity_test_path(
        &blocks,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_SWAP_SUFFIX,
    )
    .expect("block replacement path");
    copy_regular_test_tree(&blocks, &replacement);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
        .expect_err("a post-preflight block-directory replacement must fail closed");
    assert!(matches!(
        error,
        Error::IO(ref source, _)
            if source.kind() == ErrorKind::InvalidData
                && source.to_string().contains("path identity changed")
    ));
    assert_eq!(
        snapshot_regular_test_tree(&blocks),
        expected,
        "BlockStore must not create or rewrite files in the replacement directory"
    );
    let displaced = configured_primary_open_identity_test_path(
        &blocks,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_DISPLACED_SUFFIX,
    )
    .expect("displaced block path");
    assert_eq!(snapshot_regular_test_tree(&displaced), expected);
}

#[cfg(unix)]
#[test]
fn canonical_storage_preflight_rejects_symlinks_before_external_write() {
    use std::os::unix::fs::symlink;

    for target in [
        "directory",
        INDEX_FILE_NAME,
        DATA_FILE_NAME,
        HASHES_FILE_NAME,
        COUNT_FILE_NAME,
    ] {
        let temp = TempDir::new().expect("temporary parent directory");
        let root = temp.path().join("kura");
        let config = kura_config_for_path(&root, BLOCKS_IN_MEMORY);
        let configured = configured_primary_catalog("canonical-symlink");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
            .expect("initialize exact configured catalog");
        publish_configured_catalog_baseline(&kura, &configured);
        let blocks = Kura::canonical_storage_path(&kura.store_root());
        let path = match target {
            "directory" => blocks,
            file => blocks.join(file),
        };
        drop(kura);
        let outside = temp.path().join("operator-owned");
        fs::rename(&path, &outside).expect("move exact bytes outside Kura ownership");
        let expected = snapshot_regular_test_tree(&outside);
        symlink(&outside, &path).expect("substitute canonical path with symlink");
        let error = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
            .expect_err("canonical symlink must be refused before recovery writes");
        assert!(
            matches!(error, Error::IO(ref source, _) if source.kind() == ErrorKind::InvalidData),
            "wrong refusal for {target}: {error:?}"
        );
        assert!(
            path.is_symlink(),
            "startup replaced the symlink for {target}"
        );
        assert_eq!(
            snapshot_regular_test_tree(&outside),
            expected,
            "startup changed externally owned bytes for {target}"
        );
    }
}

#[cfg(unix)]
#[test]
fn canonical_storage_preflight_rejects_hardlinked_files_before_mutation() {
    for target in [
        INDEX_FILE_NAME,
        DATA_FILE_NAME,
        HASHES_FILE_NAME,
        COUNT_FILE_NAME,
    ] {
        let temp = TempDir::new().expect("temporary parent directory");
        let root = temp.path().join("kura");
        let config = kura_config_for_path(&root, BLOCKS_IN_MEMORY);
        let configured = configured_primary_catalog("canonical-hardlink");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
            .expect("initialize exact configured catalog");
        publish_configured_catalog_baseline(&kura, &configured);
        let blocks = Kura::canonical_storage_path(&kura.store_root());
        let path = blocks.join(target);
        drop(kura);
        let outside = temp.path().join("operator-owned");
        fs::hard_link(&path, &outside).expect("retain an external alias of canonical file");
        let expected_tree = snapshot_regular_test_tree(&root);
        let expected_bytes = fs::read(&outside).expect("read external bytes");
        let error = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
            .expect_err("canonical files cannot share a mutable external inode");
        assert!(
            matches!(error, Error::IO(ref source, _) if source.kind() == ErrorKind::InvalidData),
            "wrong refusal for {target}: {error:?}"
        );
        assert_eq!(
            snapshot_regular_test_tree(&root),
            expected_tree,
            "startup mutated canonical storage before refusing {target}"
        );
        assert_eq!(
            fs::read(&outside).expect("read retained external alias"),
            expected_bytes,
            "startup changed externally owned bytes for {target}"
        );
    }
}

#[test]
fn configured_catalog_preflight_rejects_zero_block_reopen_before_path_mutation() {
    let dir = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&dir, BLOCKS_IN_MEMORY);
    let configured_a = configured_primary_catalog("configured-a");
    let configured_b = configured_primary_catalog("configured-b");
    let lane_config_a = RuntimeLaneConfig::from_catalog(&configured_a);
    let (kura, BlockCount(count)) =
        Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
            .expect("an absent journal is an authenticated first startup");
    assert_eq!(count, 0);
    publish_configured_catalog_baseline(&kura, &configured_a);
    let before = snapshot_regular_test_tree(dir.path());
    drop(kura);
    let lane_config_b = RuntimeLaneConfig::from_catalog(&configured_b);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config_b, &configured_b)
        .expect_err("a reconstructed process must reject configured catalog drift");
    assert!(matches!(
        error,
        Error::IO(ref source, _) if source.to_string().contains("baseline mismatch")
    ));
    assert_eq!(snapshot_regular_test_tree(dir.path()), before);
    let (_, BlockCount(reopened_count)) =
        Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
            .expect("the exact configured catalog must reopen");
    assert_eq!(reopened_count, 0);
}

#[test]
fn configured_catalog_preflight_rejects_drift_with_durable_genesis_and_state_zero() {
    let dir = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&dir, BLOCKS_IN_MEMORY);
    let configured_a = configured_primary_catalog("durable-a");
    let configured_b = configured_primary_catalog("durable-b");
    let lane_config_a = RuntimeLaneConfig::from_catalog(&configured_a);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
        .expect("first startup");
    publish_configured_catalog_baseline(&kura, &configured_a);
    kura.store_block(native_storage_frames(1).pop().unwrap())
        .expect("persist genuine native genesis before State reconstruction");
    let before = snapshot_regular_test_tree(dir.path());
    drop(kura);
    let lane_config_b = RuntimeLaneConfig::from_catalog(&configured_b);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config_b, &configured_b)
        .expect_err("durable Kura with State at height zero must not rebase its catalog");
    assert!(matches!(
        error,
        Error::IO(ref source, _) if source.to_string().contains("baseline mismatch")
    ));
    assert_eq!(snapshot_regular_test_tree(dir.path()), before);
    let (_, BlockCount(reopened_count)) =
        Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
            .expect("the exact configured catalog must recover durable genesis");
    assert_eq!(reopened_count, 1);
}

fn populate_store(dir: &TempDir, count: usize) {
    let config = kura_config_for_dir(dir, BLOCKS_IN_MEMORY);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &RuntimeLaneConfig::default());
    let _ = store_dummy_block_arcs(&kura, count);
}

fn populate_raw_block_store(dir: &TempDir, count: usize) {
    let blocks_dir = primary_blocks_dir(dir);
    let mut block_store = BlockStore::new(&blocks_dir);
    block_store.create_files_if_they_do_not_exist().unwrap();
    if count == 0 {
        return;
    }
    for block in native_storage_frames(count) {
        block_store.append_block_to_chain(&block).unwrap();
    }
}

#[test]
fn unknown_hash_cannot_select_an_occupied_native_frame() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let unknown_hash: HashOf<BlockHeader> = HashOf::from_untyped_unchecked(Hash::new([0xEE]));
    assert_eq!(kura.get_block_height_by_hash(unknown_hash), None);
    assert!(matches!(
        kura.native_frame_read(1, unknown_hash),
        Err(Error::CanonicalBlockWireMismatch { height: 1 })
    ));
    assert!(kura.native_frame_read(3, unknown_hash).unwrap().is_none());
}

fn store_dummy_block_arcs(
    kura: &Kura,
    count: usize,
) -> Vec<iroha_data_model::block::SharedSignedBlock> {
    establish_dummy_store_primary_anchor(kura);
    let mut generator = NativeBlocks::new();
    let blocks: Vec<_> = (0..count).map(|_| generator.next()).collect();
    for block in &blocks {
        kura.store_block((block).clone())
            .expect("store original native block through durable Kura path");
    }
    blocks
}

fn establish_dummy_store_primary_anchor(kura: &Kura) {
    let Some(baseline) = kura
        .configured_lane_catalog_baseline()
        .expect("read native store configured baseline")
    else {
        return;
    };
    if let Ok(primary) = kura.lane_storage_entry(LaneId::SINGLE) {
        kura.active_lane_incarnation_marker(&primary)
            .expect("existing State-owned primary marker remains exact");
        return;
    }
    kura.bind_lane_storage_network(native_storage_network_id())
        .expect("bind the original native fixture genesis network");
    let config = RuntimeLaneConfig::default();
    let primary = config.primary();
    let incarnation = Hash::new(
        format!(
            "kura-lane-incarnation:{}:{}",
            primary.lane_id.as_u32(),
            primary.dataspace_id.as_u64()
        )
        .as_bytes(),
    );
    kura.establish_or_verify_configured_primary_geometry_anchor(primary, incarnation, baseline)
        .expect("admit the exact fixture H0 reference before provisioning");
}

#[test]
fn read_and_write_to_blockchain_index() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    block_store.write_block_index(0, 5, 7).unwrap();
    assert_eq!(block_store.read_block_index(0).unwrap(), (5, 7));
    block_store.write_block_index(0, 2, 9).unwrap();
    assert_ne!(block_store.read_block_index(0).unwrap(), (5, 7));
    block_store.write_block_index(3, 1, 2).unwrap();
    block_store.write_block_index(2, 6, 3).unwrap();
    assert_eq!(block_store.read_block_index(0).unwrap(), (2, 9));
    assert_eq!(block_store.read_block_index(2).unwrap(), (6, 3));
    assert_eq!(block_store.read_block_index(3).unwrap(), (1, 2));
    // or equivalent
    {
        let should_be = indices([(2, 9), (0, 0), (6, 3), (1, 2)]);
        let mut is = indices([(0, 0), (0, 0), (0, 0), (0, 0)]);
        block_store.read_block_indices(0, &mut is).unwrap();
        assert_eq!(should_be, is);
    }
    assert_eq!(block_store.read_index_count().unwrap(), 4);
    block_store.write_index_count(0).unwrap();
    assert_eq!(block_store.read_index_count().unwrap(), 0);
    block_store.write_index_count(12).unwrap();
    assert_eq!(block_store.read_index_count().unwrap(), 12);
}

#[test]
fn block_index_encoding_is_fixed_little_endian_layout() {
    let entry = BlockIndex {
        start: 0x0102_0304_0506_0708,
        length: 0x1112_1314_1516_1718,
    };
    let bytes = entry.encode();
    assert_eq!(bytes.len() as u64, BlockIndex::SIZE);
    assert_eq!(
        &bytes[..core::mem::size_of::<u64>()],
        &entry.start.to_le_bytes()
    );
    assert_eq!(
        &bytes[core::mem::size_of::<u64>()..],
        &entry.length.to_le_bytes()
    );
}

#[test]
fn canonical_transaction_index_keeps_empty_and_nonempty_resultless_bodies_incomplete() {
    for entry_count in [0, 1] {
        for attach_results in [false, true] {
            let dir = TempDir::new().expect("temporary canonical index store");
            let config = kura_config_for_dir(&dir, nonzero!(2_usize));
            let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
                &config,
                &RuntimeLaneConfig::default(),
            )
            .expect("open canonical index store");
            let transactions = (0..entry_count)
                .map(|_| {
                    let transaction = TransactionBuilder::new(
                        test_network_id(b"resultless-canonical-index"),
                        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
                        iroha_data_model::transaction::FeePaymentIntent::authority(
                            Vec::new(),
                            None,
                        ),
                    )
                    .with_instructions([Log::new(
                        Level::INFO,
                        "resultless index control".to_owned(),
                    )])
                    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
                    AcceptedTransaction::new_unchecked(Cow::Owned(transaction))
                })
                .collect::<Vec<_>>();
            let mut block: SignedBlock = BlockBuilder::new(transactions)
                .chain(0, None)
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
                .unpack(|_| {})
                .into();
            assert!(!block.has_results());
            if attach_results {
                attach_ok_results_to_block(&mut block);
            }
            // This is an untrusted storage/index shape control, not execution authority.
            let expected_wire = block.encode_wire().expect("encode exact index test body");
            let probe = block
                .network_entrypoints()
                .cloned()
                .next()
                .map(|entry| entry.hash())
                .unwrap_or_else(|| {
                    HashOf::from_untyped_unchecked(Hash::new(b"absent index probe"))
                });
            let block = share_storage_fixture(block);
            kura.store_block((block).clone())
                .expect("store canonical body without panicking in the derived index");
            assert_eq!(
                kura.canonical_block_wire_bytes_for_testing(nonzero!(1_usize))
                    .expect("read actual durable canonical body"),
                expected_wire,
            );
            assert_eq!(
                kura.get_durable_block_hash(nonzero!(1_usize)),
                Some(block.hash())
            );
            let index = kura.transaction_entrypoint_index.lock();
            assert_eq!(index.complete, attach_results);
            assert_eq!(
                index.incomplete_heights.contains(&nonzero!(1_usize)),
                !attach_results,
            );
            drop(index);
            assert_eq!(
                kura.get_block_heights_by_entrypoint_hash(probe).is_some(),
                attach_results,
                "resultless bodies cannot authorize an empty complete lookup",
            );
        }
    }
}
