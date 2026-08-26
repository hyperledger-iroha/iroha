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
fn assert_catalog_paths_absent(store_root: &Path, catalog: &LaneCatalog) {
    let lane_config = RuntimeLaneConfig::from_catalog(catalog);
    let primary = lane_config.primary();
    assert!(
        !primary.blocks_dir(store_root).exists(),
        "rejected startup must not create the attempted block-store path"
    );
    assert!(
        !primary.merge_log_path(store_root).exists(),
        "rejected startup must not create the attempted merge-ledger path"
    );
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
    drop(kura);
    let blocks = lane_config.primary().blocks_dir(temp.path());
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
fn configured_primary_open_rejects_merge_file_inode_swap_before_mutation() {
    let temp = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let configured = configured_primary_catalog("merge-identity");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
        .expect("establish authenticated configured primary");
    publish_configured_catalog_baseline(&kura, &configured);
    drop(kura);
    let merge = lane_config.primary().merge_log_path(temp.path());
    let expected = fs::read(&merge).expect("read configured-primary merge log");
    let replacement = configured_primary_open_identity_test_path(
        &merge,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_SWAP_SUFFIX,
    )
    .expect("merge replacement path");
    fs::copy(&merge, &replacement).expect("copy replacement merge log");
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured)
        .expect_err("a post-preflight merge-file replacement must fail closed");
    assert!(matches!(
        error,
        Error::IO(ref source, _)
            if source.kind() == ErrorKind::InvalidData
                && source.to_string().contains("path identity changed")
    ));
    assert_eq!(
        fs::read(&merge).expect("read rejected replacement merge log"),
        expected,
        "MergeLedgerLog must not rewrite the replacement file"
    );
    let displaced = configured_primary_open_identity_test_path(
        &merge,
        CONFIGURED_PRIMARY_OPEN_IDENTITY_DISPLACED_SUFFIX,
    )
    .expect("displaced merge path");
    assert_eq!(
        fs::read(displaced).expect("read displaced original merge log"),
        expected
    );
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
    drop(kura);
    let lane_config_b = RuntimeLaneConfig::from_catalog(&configured_b);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config_b, &configured_b)
        .expect_err("a reconstructed process must reject configured catalog drift");
    assert!(matches!(
        error,
        Error::IO(ref source, _) if source.to_string().contains("baseline mismatch")
    ));
    assert_catalog_paths_absent(dir.path(), &configured_b);
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
    let block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    kura.store_block(Arc::new(block))
        .expect("persist genesis before State reconstruction");
    drop(kura);
    let lane_config_b = RuntimeLaneConfig::from_catalog(&configured_b);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config_b, &configured_b)
        .expect_err("durable Kura with State at height zero must not rebase its catalog");
    assert!(matches!(
        error,
        Error::IO(ref source, _) if source.to_string().contains("baseline mismatch")
    ));
    assert_catalog_paths_absent(dir.path(), &configured_b);
    let (_, BlockCount(reopened_count)) =
        Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
            .expect("the exact configured catalog must recover durable genesis");
    assert_eq!(reopened_count, 1);
}
#[test]
fn configured_catalog_preflight_rejects_existing_journal_without_baseline() {
    let dir = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&dir, BLOCKS_IN_MEMORY);
    let configured_b = configured_primary_catalog("unbound-b");
    let lane_config_a = RuntimeLaneConfig::default();
    let (kura, _) = Kura::new_fresh_single_lane(&config, &lane_config_a)
        .expect("initialize canonical fresh Kura");
    let incarnations = BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0xB1; Hash::LENGTH]))]);
    let activation_heights = BTreeMap::from([(LaneId::SINGLE, 0)]);
    kura.mark_lane_geometry_catalog_published(
        &lane_config_a,
        &incarnations,
        &activation_heights,
        None,
    )
    .expect("persist a v4 journal without a configured baseline");
    drop(kura);
    let lane_config_b = RuntimeLaneConfig::from_catalog(&configured_b);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config_b, &configured_b)
        .expect_err("first-release startup must reject an unbound existing journal");
    assert!(matches!(
        error,
        Error::IO(ref source, _)
            if source
                .to_string()
                .contains("has no configured lane catalog baseline")
    ));
    assert_catalog_paths_absent(dir.path(), &configured_b);
}
fn populate_store(dir: &TempDir, count: usize) {
    let blocks_dir = primary_blocks_dir(dir);
    let mut block_store = BlockStore::new(&blocks_dir);
    block_store.create_files_if_they_do_not_exist().unwrap();
    let leader_key = checked_keypair();
    let mut prev_hash = None;
    for index in 0..count {
        let height = u64::try_from(index)
            .expect("fixture block index fits u64")
            .saturating_add(1);
        let block: SignedBlock =
            ValidBlock::new_dummy_and_modify_header(leader_key.private_key(), |header| {
                header.set_height(
                    core::num::NonZeroU64::new(height).expect("fixture block height is non-zero"),
                );
                header.set_prev_block_hash(prev_hash);
            })
            .into();
        prev_hash = Some(block.hash());
        block_store.append_block_to_chain(&block).unwrap();
    }
}
fn advertised_block_metadata(kura: &Kura, height: NonZeroUsize) -> (HashOf<BlockHeader>, u64) {
    let block_hash = kura
        .get_block_hash(height)
        .or_else(|| kura.get_durable_block_hash(height))
        .expect("hash available");
    let payload_len = {
        let mut store = kura.block_store.lock();
        store
            .read_block_index(u64::try_from(height.get() - 1).expect("height fits"))
            .expect("block index")
            .length
    };
    (block_hash, payload_len)
}
fn advertise_unfinalized_required_replicas(
    kura: &Kura,
    height: NonZeroUsize,
) -> (HashOf<BlockHeader>, u64) {
    let (block_hash, payload_len) = advertised_block_metadata(kura, height);
    for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
        let peer = checked_peer_id();
        kura.record_block_replica_advert(
            peer,
            u64::try_from(height.get()).expect("height fits"),
            block_hash,
            payload_len,
        );
    }
    (block_hash, payload_len)
}
#[test]
fn unauthenticated_replica_test_injection_requires_exact_finality_and_bounds() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(1_usize);
    let (block_hash, payload_len) = advertised_block_metadata(&kura, height);
    let peer = checked_peer_id();
    kura.record_block_replica_advert(peer.clone(), 0, block_hash, payload_len);
    kura.record_block_replica_advert(peer, height.get() as u64, block_hash, 0);
    assert!(
        kura.replica_registry.lock().is_empty(),
        "invalid adverts must not enter the replica registry"
    );
    kura.record_block_replica_advert(
        checked_peer_id(),
        height.get() as u64,
        block_hash,
        payload_len,
    );
    assert!(
        kura.replica_registry.lock().is_empty(),
        "even well-shaped test observations require retained finality authority"
    );
}
#[test]
fn unknown_hash_has_no_body_status_or_durable_payload_len() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 2);
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let unknown_hash: HashOf<BlockHeader> = HashOf::from_untyped_unchecked(Hash::new([0xEE]));
    assert_eq!(kura.get_block_height_by_hash(unknown_hash), None);
    assert_eq!(kura.block_body_status_by_hash(unknown_hash), None);
    assert!(!kura.block_payload_available_by_hash(unknown_hash));
    assert_eq!(kura.durable_block_payload_len_by_hash(unknown_hash), None);
}
fn store_dummy_block_arcs(kura: &Kura, count: usize) -> Vec<Arc<SignedBlock>> {
    let mut generator = DummyBlocks::new();
    let blocks: Vec<_> = (0..count).map(|_| generator.next()).collect();
    for block in &blocks {
        kura.store_block(Arc::clone(block))
            .expect("store dummy block through durable Kura path");
    }
    blocks
}
fn finalize_chain_through_for_eviction(kura: &Kura, height: NonZeroUsize) {
    let target_height = u64::try_from(height.get()).expect("fixture height fits u64");
    if kura.v2_finality_artifact_path(target_height).is_file() {
        return;
    }
    let blocks = (1..=height.get())
        .map(|height| {
            let height = NonZeroUsize::new(height).expect("positive fixture height");
            if let Some(block) = kura.get_block(height) {
                return block;
            }
            // A few batched-fsync tests append directly through `BlockStore`
            // so they can leave a later index entry unpublished.  Such
            // bodies are deliberately absent from Kura's in-memory image;
            // recover the exact inline fixture body without weakening the
            // production canonical reader.
            let mut store = kura.block_store.lock();
            let index = height.get() - 1;
            let block = read_block(&mut store, index)
                .expect("inline canonical fixture body is available before eviction");
            let expected_hash = store
                .read_block_hashes(
                    u64::try_from(index).expect("fixture block index fits u64"),
                    1,
                )
                .expect("read canonical fixture hash")
                .into_iter()
                .next()
                .expect("canonical fixture hash is present");
            assert_eq!(
                block.hash(),
                expected_hash,
                "fixture body must match its canonical hash journal entry"
            );
            Arc::new(block)
        })
        .collect::<Vec<_>>();
    let artifact = v2_finality_artifacts_for_chain(&blocks)
        .pop()
        .expect("target-height finality artifact");
    assert_eq!(artifact.height, target_height);
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist exact signed complete-wire finality before fixture eviction");
}
fn advertise_required_replicas(kura: &Kura, height: NonZeroUsize) -> (HashOf<BlockHeader>, u64) {
    finalize_chain_through_for_eviction(kura, height);
    let metadata = advertised_block_metadata(kura, height);
    assert_eq!(
        kura.advertise_required_replicas_for_bench(height),
        Some(metadata.1),
        "fixture must install every exact deterministic remote keeper"
    );
    metadata
}
fn sample_merge_entry(epoch: u64) -> MergeLedgerEntry {
    let epoch_u8 = u8::try_from(epoch).expect("test epoch must fit in a u8");
    let epoch_plus_one = epoch_u8
        .checked_add(1)
        .expect("test epoch offset 1 must fit in a u8");
    let epoch_plus_three = epoch_u8
        .checked_add(3)
        .expect("test epoch offset 3 must fit in a u8");
    let lane_snapshots = vec![iroha_data_model::merge::MergeLaneSnapshot {
        lane_id: LaneId::SINGLE,
        lane_incarnation: Hash::new(b"kura-merge-test-lane-incarnation"),
        incarnation_activation_height: 1,
        proposal_height: epoch.max(1),
        dataspace_id: DataSpaceId::UNIVERSAL,
        lane_block_height: epoch,
        tip_hash: HashOf::from_untyped_unchecked(Hash::new([epoch_u8])),
        merge_hint_root: Hash::new([epoch_plus_one]),
        settlement_commitment: iroha_data_model::block::consensus::LaneBlockCommitment {
            block_height: epoch,
            lane_id: LaneId::SINGLE,
            lane_incarnation: Hash::new(b"kura-merge-test-lane-incarnation"),
            dataspace_id: DataSpaceId::UNIVERSAL,
            tx_count: 0,
            total_local_amount: "0".parse().expect("valid settlement quantity"),
            total_xor_due: "0".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
            total_xor_variance: "0".parse().expect("valid settlement quantity"),
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        },
        settlement_hash: iroha_data_model::nexus::compute_settlement_hash(
            &iroha_data_model::block::consensus::LaneBlockCommitment {
                block_height: epoch,
                lane_id: LaneId::SINGLE,
                lane_incarnation: Hash::new(b"kura-merge-test-lane-incarnation"),
                dataspace_id: DataSpaceId::UNIVERSAL,
                tx_count: 0,
                total_local_amount: "0".parse().expect("valid settlement quantity"),
                total_xor_due: "0".parse().expect("valid settlement quantity"),
                total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
                total_xor_variance: "0".parse().expect("valid settlement quantity"),
                swap_metadata: None,
                receipts: Vec::new(),
                nexus_fee_receipts: Vec::new(),
                native_amx_receipts: Vec::new(),
            },
        )
        .expect("test settlement should hash canonically"),
        relay_envelope: None,
    }];
    let merge_hint_roots: Vec<Hash> = lane_snapshots
        .iter()
        .map(|snapshot| snapshot.merge_hint_root)
        .collect();
    let global_state_root = reduce_merge_hint_roots(&merge_hint_roots);
    MergeLedgerEntry {
        version: MergeLedgerEntry::VERSION,
        epoch_id: epoch,
        lane_catalog_hash: Hash::new(b"kura-merge-test-catalog"),
        active_lanes: vec![iroha_data_model::merge::MergeLaneBinding {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_config_hash: Hash::new(b"kura-merge-test-config"),
            incarnation: Hash::new(b"kura-merge-test-lane-incarnation"),
            activation_height: 1,
        }],
        incarnation_root: Hash::new(b"kura-merge-test-incarnation-root"),
        activation_root: Hash::new(b"kura-merge-test-activation-root"),
        lane_snapshots,
        execution_batch: None,
        lane_drain_certificates: Vec::new(),
        global_state_root,
        merge_qc: MergeQuorumCertificate::new(
            epoch,
            epoch,
            epoch,
            HashOf::from_untyped_unchecked(Hash::new([epoch_plus_three; Hash::LENGTH])),
            test_network_id(b"kura-merge-test-chain"),
            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            HashOf::new(&Vec::<PeerId>::new()),
            Vec::new(),
            vec![0x01],
            Vec::new(),
            vec![0xAA, 0xBB],
            Hash::new([epoch_plus_three]),
        ),
    }
}
fn sample_merge_entry_for_block(epoch: u64, block: &SignedBlock) -> MergeLedgerEntry {
    let mut entry = sample_merge_entry(epoch);
    entry.merge_qc.epoch_id = epoch;
    entry.merge_qc.view = block.header().view_change_index();
    entry.merge_qc.carrier_height = block.header().height().get();
    entry.merge_qc.carrier_parent_hash = block
        .header()
        .prev_block_hash()
        .expect("merge carrier fixture must not be genesis");
    entry
}
fn attach_merge_reference(block: &SignedBlock, entry: &MergeLedgerEntry) -> Arc<SignedBlock> {
    let mut block = block.clone();
    let context = block
        .execution_context()
        .cloned()
        .unwrap_or_else(|| BlockExecutionContextBundle::new(Vec::new()))
        .with_merge_entry(iroha_data_model::block::CertifiedMergeLedgerReference::new(
            entry,
        ));
    block.set_execution_context(Some(context));
    Arc::new(block)
}
fn bind_merge_entry_to_carrier(
    block: Arc<SignedBlock>,
    entry: &mut MergeLedgerEntry,
) -> Arc<SignedBlock> {
    entry.merge_qc.epoch_id = entry.epoch_id;
    entry.merge_qc.view = block.header().view_change_index();
    entry.merge_qc.carrier_height = block.header().height().get();
    entry.merge_qc.carrier_parent_hash = block
        .header()
        .prev_block_hash()
        .expect("merge carrier fixture must not be the genesis block");
    let mut block = block.as_ref().clone();
    let execution_context = block
        .execution_context()
        .cloned()
        .unwrap_or_else(|| BlockExecutionContextBundle::new(Vec::new()))
        .with_merge_entry(CertifiedMergeLedgerReference::new(entry));
    block.set_execution_context(Some(execution_context));
    Arc::new(block)
}
fn next_merge_carrier(
    generator: &mut DummyBlocks,
    entry: &mut MergeLedgerEntry,
) -> Arc<SignedBlock> {
    let carrier = bind_merge_entry_to_carrier(generator.next(), entry);
    *generator
        .blocks
        .last_mut()
        .expect("dummy generator contains the carrier") = Arc::clone(&carrier);
    carrier
}
fn store_genesis_and_build_merge_carrier(
    kura: &Kura,
    epoch: u64,
) -> (Arc<SignedBlock>, MergeLedgerEntry) {
    let mut blocks = DummyBlocks::new();
    let genesis = blocks.next();
    kura.store_block(genesis).expect("store fixture genesis");
    let mut entry = sample_merge_entry(epoch);
    let carrier = next_merge_carrier(&mut blocks, &mut entry);
    (carrier, entry)
}
#[test]
fn pending_certified_merge_sidecar_is_scoped_to_exact_carrier_round() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let symlinked = Kura::blank_kura_for_testing();
        let outside = TempDir::new().expect("create external pending merge directory");
        let external_entry = sample_merge_entry(100);
        let external_hash = external_entry.canonical_hash();
        let external_temp = outside.path().join(format!(
            "{}.norito.tmp",
            hex::encode(external_hash.as_ref())
        ));
        fs::write(&external_temp, external_entry.canonical_bytes())
            .expect("stage external pending merge temporary");
        symlink(outside.path(), symlinked.pending_merge_entry_dir())
            .expect("replace pending merge directory with a symlink");
        assert!(
            symlinked
                .validate_pending_merge_entries_on_startup()
                .is_err(),
            "startup must reject a symlinked pending merge directory before recovery"
        );
        assert!(
            external_temp.is_file(),
            "startup rejection must not mutate the symlink target"
        );
    }
    {
        let malformed_inventory = Kura::blank_kura_for_testing();
        let directory = malformed_inventory.pending_merge_entry_dir();
        fs::create_dir(&directory).expect("create malformed pending merge inventory");
        let recovery_entry = sample_merge_entry(102);
        let recovery_hash = recovery_entry.canonical_hash();
        let temp_path = malformed_inventory
            .pending_merge_entry_path(recovery_hash)
            .with_extension("norito.tmp");
        fs::write(&temp_path, recovery_entry.canonical_bytes())
            .expect("stage valid pending merge temporary");
        fs::write(directory.join("ABC.norito"), b"malformed stable filename")
            .expect("stage malformed stable pending merge name");
        assert!(
            malformed_inventory
                .validate_pending_merge_entries_on_startup()
                .is_err(),
            "recovery must validate the complete stable inventory before publication"
        );
        assert!(temp_path.is_file());
        assert!(
            !malformed_inventory
                .pending_merge_entry_path(recovery_hash)
                .exists(),
            "invalid stable inventory must leave an unpublished temporary unpublished"
        );
    }
    {
        let unpublished = Kura::blank_kura_for_testing();
        let directory = unpublished.pending_merge_entry_dir();
        fs::create_dir(&directory).expect("create unpublished pending merge directory");
        let recovery_entry = sample_merge_entry(105);
        let recovery_hash = recovery_entry.canonical_hash();
        let target_path = unpublished.pending_merge_entry_path(recovery_hash);
        let temp_path = target_path.with_extension("norito.tmp");
        fs::write(&temp_path, recovery_entry.canonical_bytes())
            .expect("stage unpublished pending merge temporary");
        sync_dir(&directory).expect("sync unpublished pending merge directory");
        unpublished
            .validate_pending_merge_entries_on_startup()
            .expect("publish lone pending merge temporary");
        assert!(!temp_path.exists());
        assert_eq!(
            unpublished
                .read_pending_merge_entry_path(&target_path, Some(recovery_hash))
                .expect("read recovered pending merge entry"),
            Some(recovery_entry)
        );
    }
    {
        let no_clobber = Kura::blank_kura_for_testing();
        let entry = sample_merge_entry(103);
        let path = no_clobber.pending_merge_entry_path(entry.canonical_hash());
        fs::create_dir(no_clobber.pending_merge_entry_dir())
            .expect("create no-clobber pending merge directory");
        let sentinel = b"attacker-controlled pending merge target";
        fs::write(&path, sentinel).expect("stage conflicting pending merge target");
        assert!(
            no_clobber
                .persist_pending_certified_merge_entry(&entry)
                .is_err(),
            "a conflicting hash-addressed target must fail closed"
        );
        assert_eq!(
            fs::read(path).expect("read preserved pending merge target"),
            sentinel,
            "durable publication must never clobber an existing target"
        );
    }
    {
        let saturated = Kura::blank_kura_for_testing();
        let merge_directory = saturated.pending_merge_entry_dir();
        let admission_directory = saturated.pending_queue_plan_admission_dir();
        fs::create_dir(&merge_directory).expect("create pending merge saturation directory");
        fs::create_dir(&admission_directory).expect("create admission saturation directory");
        let recovery_entry = sample_merge_entry(104);
        let recovery_hash = recovery_entry.canonical_hash();
        let target_path = saturated.pending_merge_entry_path(recovery_hash);
        let temp_path = target_path.with_extension("norito.tmp");
        fs::write(&temp_path, recovery_entry.canonical_bytes())
            .expect("stage pending merge temporary above the shared cap");
        let one_mebibyte = u64::try_from(MAX_PENDING_QUEUE_PLAN_ADMISSION_CERTIFICATE_BYTES)
            .expect("admission byte limit fits u64");
        let saturation_files = saturated.pending_control_sidecar_limits.aggregate_bytes
            / MAX_PENDING_QUEUE_PLAN_ADMISSION_CERTIFICATE_BYTES;
        for index in 0..saturation_files {
            let hash = Hash::new(format!("shared-cap-admission-{index}"));
            let path = saturated.pending_queue_plan_admission_path(hash);
            let file = fs::File::create(path).expect("create sparse admission saturation file");
            file.set_len(one_mebibyte)
                .expect("size sparse admission saturation file");
        }
        assert!(
            saturated
                .validate_pending_merge_entries_on_startup()
                .is_err(),
            "merge recovery must enforce the shared pending-control byte cap before publication"
        );
        assert!(temp_path.is_file());
        assert!(!target_path.exists());
    }
    let kura = Kura::blank_kura_for_testing();
    #[cfg(any(unix, windows))]
    {
        let recovery_entry = sample_merge_entry(101);
        let recovery_bytes = recovery_entry.canonical_bytes();
        let recovery_hash = recovery_entry.canonical_hash();
        let directory = kura.pending_merge_entry_dir();
        fs::create_dir(&directory).expect("create pending merge recovery directory");
        let target_path = kura.pending_merge_entry_path(recovery_hash);
        let temp_path = target_path.with_extension("norito.tmp");
        fs::write(&temp_path, &recovery_bytes).expect("write durable pending merge temporary");
        fs::hard_link(&temp_path, &target_path).expect("publish pending merge recovery hard link");
        sync_dir(&directory).expect("sync pending merge recovery directory");
        let staged_total = kura
            .refresh_disk_usage_bytes()
            .expect("cache hard-linked pending merge recovery bytes");
        kura.validate_pending_merge_entries_on_startup()
            .expect("finish hard-linked pending merge publication");
        assert!(!temp_path.exists());
        assert!(target_path.is_file());
        assert!(Kura::sidecar_is_single_link(
            &fs::symlink_metadata(&target_path)
                .expect("read recovered pending merge target metadata")
        ));
        let path_bytes = u64::try_from(recovery_bytes.len()).expect("fixture length fits u64");
        assert_eq!(
            kura.disk_usage_bytes()
                .expect("read published pending merge recovery accounting"),
            staged_total.saturating_sub(path_bytes),
            "startup recovery must publish the crash-temporary removal delta"
        );
        kura.remove_pending_certified_merge_entry(recovery_hash)
            .expect("remove recovered pending merge fixture");
        assert_eq!(
            kura.disk_usage_bytes()
                .expect("read pending merge accounting after fixture removal"),
            staged_total.saturating_sub(path_bytes.saturating_mul(2))
        );
    }
    let mut entry = sample_merge_entry(1);
    let carrier_parent =
        HashOf::from_untyped_unchecked(Hash::new(b"exact pending merge carrier parent"));
    let wrong_parent =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong pending merge carrier parent"));
    entry.merge_qc.carrier_height = 7;
    entry.merge_qc.carrier_parent_hash = carrier_parent;
    entry.merge_qc.view = 3;
    for (height, parent, view, drift) in [
        (8, carrier_parent, 3, "height"),
        (7, wrong_parent, 3, "parent"),
        (7, carrier_parent, 4, "view"),
    ] {
        kura.persist_pending_certified_merge_entry(&entry)
            .expect("persist exact-round pending merge fixture");
        assert_eq!(
            kura.prune_pending_certified_merge_entries_not_bound_to(height, parent, view)
                .expect("prune mismatched pending merge fixture"),
            1,
            "{drift} drift must retire the pending merge sidecar"
        );
        assert!(
            kura.select_pending_certified_merge_entry()
                .expect("pending merge store remains readable")
                .is_none(),
            "{drift} drift must leave no reusable pending merge sidecar"
        );
    }
    let entry_hash = kura
        .persist_pending_certified_merge_entry(&entry)
        .expect("persist exact pending merge sidecar");
    assert_eq!(
        kura.prune_pending_certified_merge_entries_not_bound_to(7, carrier_parent, 3)
            .expect("retain exact pending merge fixture"),
        0
    );
    let (selected_hash, selected) = kura
        .select_pending_certified_merge_entry()
        .expect("pending merge store remains readable")
        .expect("exact round retains the sidecar");
    assert_eq!(selected_hash, entry_hash);
    assert_eq!(selected, entry);
}
#[test]
fn pending_certified_merge_work_binds_routing_legs_to_exact_active_incarnation() {
    let target_lane = LaneId::new(7);
    let target_dataspace = DataSpaceId::new(11);
    let retired_incarnation = Hash::new(b"pending-route-retired-incarnation");
    let recreated_incarnation = Hash::new(b"pending-route-recreated-incarnation");
    let routing_plan = crate::queue::RoutingPlan::native_amx(
        crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        vec![crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(target_lane, target_dataspace),
            crate::queue::RouteLegRole::Participant,
        )],
    );
    let entrypoint = offline_top_up_entrypoint_for_index([0x71; 32], [0x72; 32]);
    let mut retired_entry = merge_entry_with_indexed_entrypoint(entrypoint);
    let execution = retired_entry
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("merge execution fixture");
    let origin = execution.origin_proposal.clone();
    let mut participant_descriptor = origin.descriptor.clone();
    participant_descriptor.lane_id = target_lane;
    participant_descriptor.dataspace_id = target_dataspace;
    participant_descriptor.lane_incarnation = retired_incarnation;
    participant_descriptor.descriptor_hash = participant_descriptor.computed_descriptor_hash();
    let mut participant_proposal = LaneBlockProposalV1 {
        descriptor: participant_descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    participant_proposal.proposal_hash = participant_proposal.computed_proposal_hash();
    let participant_settlement = LaneBlockCommitment {
        block_height: participant_proposal.descriptor.lane_block_height,
        lane_id: target_lane,
        lane_incarnation: retired_incarnation,
        dataspace_id: target_dataspace,
        tx_count: 0,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let participant_settlement_hash =
        iroha_data_model::nexus::compute_settlement_hash(&participant_settlement)
            .expect("participant settlement hashes");
    let participant_validator_set = Vec::<PeerId>::new();
    let participant_validator_set_hash = HashOf::new(&participant_validator_set);
    let source_id = [0x73; Hash::LENGTH];
    let mut prepare_body = iroha_data_model::block::consensus::NativeAmxAttestationBodyV2 {
        round: iroha_data_model::block::consensus_v2::ConsensusRound {
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"pending-route-native-context",
            ))),
            height: origin.descriptor.proposal_height,
            view: origin.descriptor.lane_block_view,
        },
        epoch: 1,
        network_id: execution.autonomous_network_id,
        source_id,
        tx_entrypoint_hash: execution.entrypoints[0].hash(),
        plan_digest: routing_plan.digest(),
        phase: iroha_data_model::block::consensus::NativeAmxPhase::Prepare,
        coordinator_lane_id: origin.descriptor.lane_id,
        coordinator_dataspace_id: origin.descriptor.dataspace_id,
        coordinator_lane_incarnation: origin.descriptor.lane_incarnation,
        participant_lane_id: target_lane,
        participant_dataspace_id: target_dataspace,
        participant_lane_incarnation: retired_incarnation,
        participant_previous_block_height: participant_proposal
            .descriptor
            .previous_lane_block_height,
        participant_previous_block_descriptor_hash: participant_proposal
            .descriptor
            .previous_lane_block_descriptor_hash,
        participant_lane_block_height: participant_proposal.descriptor.lane_block_height,
        participant_lane_block_view: participant_proposal.descriptor.lane_block_view,
        participant_proposal_hash: participant_proposal.proposal_hash,
        participant_settlement_commitment: Hash::from(participant_settlement_hash),
        participant_validator_set_hash,
        participant_validator_count: 0,
        participant_min_quorum: 0,
        authority_context_height: origin.descriptor.proposal_height,
        planned_coordinator_block_height: origin.descriptor.lane_block_height,
        coordinator_lane_block_view: origin.descriptor.lane_block_view,
        coordinator_proposal_hash: origin.proposal_hash,
    };
    let native_qc = |body| {
        iroha_data_model::block::consensus::NativeAmxAttestationQcV2::try_new(
            body,
            VALIDATOR_SET_HASH_VERSION_V1,
            participant_validator_set_hash,
            participant_validator_set.clone(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("preflight fixture validator set and proofs must align")
    };
    let prepare_qc = native_qc(prepare_body);
    prepare_body.phase = iroha_data_model::block::consensus::NativeAmxPhase::Commit;
    let commit_qc = native_qc(prepare_body);
    let receipt = NativeAmxReceipt {
        version: 2,
        source_id,
        network_id: execution.autonomous_network_id,
        plan_digest: routing_plan.digest(),
        lane_id: origin.descriptor.lane_id,
        dataspace_id: origin.descriptor.dataspace_id,
        lane_incarnation: origin.descriptor.lane_incarnation,
        authority_context_height: origin.descriptor.proposal_height,
        lane_block_height: origin.descriptor.lane_block_height,
        lane_block_view: origin.descriptor.lane_block_view,
        coordinator_proposal_hash: origin.proposal_hash,
        legs: vec![iroha_data_model::block::consensus::NativeAmxLegRecordV2 {
            lane_id: target_lane,
            dataspace_id: target_dataspace,
            participant_proposal,
            participant_settlement,
            participant_settlement_hash,
            prepare_qc,
            commit_qc,
        }],
    };
    let execution = retired_entry
        .execution_batch
        .as_mut()
        .and_then(|batch| batch.lanes.first_mut())
        .expect("merge execution fixture");
    execution.routing_plans =
        vec![norito::to_bytes(&routing_plan).expect("encode canonical pending-route fixture")];
    execution.native_amx_receipts = vec![Some(receipt)];
    retired_entry
        .active_lanes
        .push(iroha_data_model::merge::MergeLaneBinding {
            lane_id: target_lane,
            dataspace_id: target_dataspace,
            lane_config_hash: Hash::new(b"pending-route-retired-config"),
            incarnation: retired_incarnation,
            activation_height: 1,
        });
    let kura = Kura::blank_kura_for_testing();
    kura.persist_pending_certified_merge_entry(&retired_entry)
        .expect("persist retired-incarnation pending merge fixture");
    assert!(
        kura.pending_certified_merge_work_for_lane(
            target_lane,
            target_dataspace,
            retired_incarnation,
        )
        .expect("scan retired incarnation"),
        "the pending routing leg must block its exact historical incarnation"
    );
    assert!(
        !kura
            .pending_certified_merge_work_for_lane(
                target_lane,
                target_dataspace,
                recreated_incarnation,
            )
            .expect("scan recreated incarnation"),
        "an old-incarnation sidecar must not ABA-block the recreated lane"
    );
    let mut malformed_entry = retired_entry.clone();
    malformed_entry
        .execution_batch
        .as_mut()
        .and_then(|batch| batch.lanes.first_mut())
        .expect("merge execution fixture")
        .routing_plans = vec![vec![0xFF]];
    let malformed_kura = Kura::blank_kura_for_testing();
    malformed_kura
        .persist_pending_certified_merge_entry(&malformed_entry)
        .expect("persist malformed-routing drain fixture");
    assert!(
        malformed_kura
            .pending_certified_merge_work_for_lane(
                target_lane,
                target_dataspace,
                recreated_incarnation,
            )
            .expect("scan malformed routing fixture"),
        "undecodable routing evidence must fail closed"
    );
    let mut missing_receipt_entry = retired_entry.clone();
    missing_receipt_entry
        .execution_batch
        .as_mut()
        .and_then(|batch| batch.lanes.first_mut())
        .expect("merge execution fixture")
        .native_amx_receipts = vec![None];
    let missing_receipt_kura = Kura::blank_kura_for_testing();
    missing_receipt_kura
        .persist_pending_certified_merge_entry(&missing_receipt_entry)
        .expect("persist missing-receipt drain fixture");
    assert!(
        missing_receipt_kura
            .pending_certified_merge_work_for_lane(
                target_lane,
                target_dataspace,
                recreated_incarnation,
            )
            .expect("scan missing receipt fixture"),
        "Native routing without its aligned receipt must fail closed"
    );
    let mut unbound_entry = retired_entry.clone();
    unbound_entry
        .active_lanes
        .retain(|binding| binding.lane_id != target_lane);
    let unbound_kura = Kura::blank_kura_for_testing();
    unbound_kura
        .persist_pending_certified_merge_entry(&unbound_entry)
        .expect("persist unbound routing fixture");
    assert!(
        unbound_kura
            .pending_certified_merge_work_for_lane(
                target_lane,
                target_dataspace,
                recreated_incarnation,
            )
            .expect("scan unbound routing fixture"),
        "a matching route without an authenticated active binding must fail closed"
    );
    let mut ambiguous_entry = retired_entry;
    ambiguous_entry
        .active_lanes
        .push(iroha_data_model::merge::MergeLaneBinding {
            lane_id: target_lane,
            dataspace_id: target_dataspace,
            lane_config_hash: Hash::new(b"pending-route-recreated-config"),
            incarnation: recreated_incarnation,
            activation_height: 2,
        });
    let ambiguous_kura = Kura::blank_kura_for_testing();
    ambiguous_kura
        .persist_pending_certified_merge_entry(&ambiguous_entry)
        .expect("persist ambiguous routing fixture");
    assert!(
        ambiguous_kura
            .pending_certified_merge_work_for_lane(
                target_lane,
                target_dataspace,
                recreated_incarnation,
            )
            .expect("scan ambiguous routing fixture"),
        "duplicate route bindings must fail closed"
    );
}
#[test]
fn pending_certified_merge_work_stops_before_later_malformed_entry() {
    let kura = Kura::blank_kura_for_testing();
    let entry = sample_merge_entry(1);
    let snapshot = entry
        .lane_snapshots
        .first()
        .expect("sample merge entry has one lane snapshot");
    let entry_hash = kura
        .persist_pending_certified_merge_entry(&entry)
        .expect("persist blocking pending merge entry");
    let malformed_path = kura
        .pending_merge_entry_dir()
        .join(format!("{}.norito", "ff".repeat(Hash::LENGTH)));
    assert!(
        malformed_path > kura.pending_merge_entry_path(entry_hash),
        "malformed fixture must sort after the blocking sidecar"
    );
    fs::write(&malformed_path, b"malformed pending merge entry")
        .expect("write later malformed pending merge fixture");
    assert!(
        kura.pending_certified_merge_work_for_lane(
            snapshot.lane_id,
            snapshot.dataspace_id,
            snapshot.lane_incarnation,
        )
        .expect("the first matching entry short-circuits the drain scan"),
        "the exact lane snapshot must block drain"
    );
    assert!(
        kura.pending_certified_merge_work_for_lane(
            LaneId::new(snapshot.lane_id.as_u32().saturating_add(1)),
            snapshot.dataspace_id,
            snapshot.lane_incarnation,
        )
        .is_err(),
        "without an earlier blocker the later malformed entry must still fail closed"
    );
}
#[test]
fn bounded_pending_merge_hash_scan_filters_orders_and_reports_overflow() {
    let kura = Kura::blank_kura_for_testing();
    let entries = [
        sample_merge_entry(3),
        sample_merge_entry(1),
        sample_merge_entry(2),
    ];
    for entry in &entries {
        kura.persist_pending_certified_merge_entry(entry)
            .expect("persist bounded pending scan fixture");
    }
    let predicate_calls = Cell::new(0usize);
    let PendingCertifiedMergeEvidenceScan::Complete(hashes) = kura
        .pending_certified_merge_entry_hashes_matching_bounded(3, |entry| {
            predicate_calls.set(predicate_calls.get().saturating_add(1));
            entry.epoch_id % 2 == 1
        })
        .expect("scan the complete bounded pending inventory")
    else {
        panic!("the complete bounded inventory must not overflow");
    };
    assert_eq!(predicate_calls.get(), 3);
    assert_eq!(
        hashes,
        vec![entries[1].canonical_hash(), entries[0].canonical_hash()],
        "matching hashes must retain canonical epoch/hash order"
    );
    fs::write(
        kura.pending_merge_entry_path(entries[2].canonical_hash()),
        b"malformed bounded pending scan fixture",
    )
    .expect("corrupt an entry beyond the smaller scan limit");
    predicate_calls.set(0);
    assert_eq!(
        kura.pending_certified_merge_entry_hashes_matching_bounded(2, |_| {
            predicate_calls.set(predicate_calls.get().saturating_add(1));
            true
        })
        .expect("report bounded pending scan overflow"),
        PendingCertifiedMergeEvidenceScan::LimitExceeded,
    );
    assert_eq!(
        predicate_calls.get(),
        0,
        "overflow must be reported before any large entry is decoded"
    );
    assert!(
        kura.pending_certified_merge_entry_hashes_matching_bounded(3, |_| true)
            .is_err(),
        "a complete scan must still fail closed on malformed evidence"
    );
}
#[test]
fn late_stale_pending_merge_sidecar_does_not_evict_current_round() {
    let kura = Kura::blank_kura_for_testing();
    let carrier_parent =
        HashOf::from_untyped_unchecked(Hash::new(b"current pending merge carrier parent"));
    let mut current = sample_merge_entry(2);
    current.merge_qc.carrier_height = 9;
    current.merge_qc.carrier_parent_hash = carrier_parent;
    current.merge_qc.view = 4;
    let mut stale = sample_merge_entry(1);
    stale.merge_qc.carrier_height = 9;
    stale.merge_qc.carrier_parent_hash = carrier_parent;
    stale.merge_qc.view = 3;
    let current_hash = kura
        .persist_pending_certified_merge_entry(&current)
        .expect("persist current-round certified merge sidecar");
    let stale_hash = kura
        .persist_pending_certified_merge_entry(&stale)
        .expect("persist late stale certified merge sidecar without implicit pruning");
    assert_ne!(current_hash, stale_hash);
    let pending = kura
        .pending_certified_merge_entries()
        .expect("pending merge store remains readable");
    assert_eq!(pending.len(), 2);
    let bounded = kura
        .pending_certified_merge_entries_bounded(1)
        .expect("bounded pending merge diagnostics remain readable");
    assert_eq!(bounded.len(), 1);
    assert!(
        [current_hash, stale_hash].contains(&bounded[0].0),
        "bounded selection must retain one exact authenticated sidecar"
    );
    assert_eq!(
        kura.pending_certified_merge_entries_bounded(1)
            .expect("repeat bounded pending merge diagnostics"),
        bounded,
        "bounded selection must be deterministic"
    );
    assert!(
        kura.pending_certified_merge_entries_bounded(0)
            .expect("zero bounded pending merge diagnostics")
            .is_empty()
    );
    assert!(
        pending
            .iter()
            .any(|(hash, entry)| *hash == current_hash && entry == &current),
        "late stale persistence must retain the current-round sidecar"
    );
    assert!(
        pending
            .iter()
            .any(|(hash, entry)| *hash == stale_hash && entry == &stale),
        "authenticated stale evidence remains available until the round owner prunes it"
    );
    assert_eq!(
        kura.prune_pending_certified_merge_entries_not_bound_to(9, carrier_parent, 4)
            .expect("explicitly prune outside the current carrier round"),
        1
    );
    let pending = kura
        .pending_certified_merge_entries()
        .expect("pending merge store remains readable after pruning");
    assert_eq!(pending, vec![(current_hash, current)]);
}
#[test]
fn bounded_pending_merge_selection_skips_committed_prefix_without_underfill() {
    let kura = Kura::blank_kura_for_testing();
    let mut left = sample_merge_entry(1);
    left.lane_catalog_hash = Hash::new(b"bounded pending merge left");
    let mut right = sample_merge_entry(1);
    right.lane_catalog_hash = Hash::new(b"bounded pending merge right");
    let (committed, pending) = if left.canonical_hash() < right.canonical_hash() {
        (left, right)
    } else {
        (right, left)
    };
    let committed_hash = kura
        .persist_pending_certified_merge_entry(&committed)
        .expect("persist sidecar that sorts first");
    let pending_hash = kura
        .persist_pending_certified_merge_entry(&pending)
        .expect("persist uncommitted sidecar that sorts second");
    assert!(committed_hash < pending_hash);
    kura.append_merge_entry_for_test(&committed)
        .expect("commit the lexicographic sidecar prefix");
    assert_eq!(
        kura.pending_certified_merge_entries_bounded(1)
            .expect("select one uncommitted pending entry"),
        vec![(pending_hash, pending)],
        "committed sidecars must not consume the bounded result budget"
    );
}
#[test]
fn locked_and_finalized_carrier_cleanup_preserves_only_authorized_sidecars() {
    let kura = Kura::blank_kura_for_testing();
    let parent = HashOf::from_untyped_unchecked(Hash::new(b"cleanup carrier parent"));
    let mut locked = sample_merge_entry(1);
    locked.merge_qc.carrier_height = 9;
    locked.merge_qc.carrier_parent_hash = parent;
    locked.merge_qc.view = 2;
    let mut losing = sample_merge_entry(2);
    losing.merge_qc.carrier_height = 9;
    losing.merge_qc.carrier_parent_hash = parent;
    losing.merge_qc.view = 3;
    let mut future = sample_merge_entry(3);
    future.merge_qc.carrier_height = 10;
    future.merge_qc.carrier_parent_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"future cleanup parent"));
    future.merge_qc.view = 0;
    for entry in [&locked, &losing, &future] {
        kura.persist_pending_certified_merge_entry(entry)
            .expect("persist cleanup fixture");
    }
    let reference = CertifiedMergeLedgerReference::new(&locked);
    assert_eq!(
        kura.retain_pending_certified_merge_entry_for_locked_carrier(9, Some(&reference))
            .expect("retain exact locked sidecar"),
        1
    );
    let pending = kura
        .pending_certified_merge_entries()
        .expect("read retained locked sidecars");
    assert_eq!(pending.len(), 2);
    assert!(pending.iter().any(|(_, entry)| entry == &locked));
    assert!(pending.iter().any(|(_, entry)| entry == &future));
    assert_eq!(
        kura.prune_finalized_pending_certified_merge_entries(9)
            .expect("retire finalized carrier sidecars"),
        1
    );
    assert_eq!(
        kura.pending_certified_merge_entries()
            .expect("read future sidecars"),
        vec![(future.canonical_hash(), future)]
    );
}
#[test]
fn pending_queue_plan_admission_store_is_exact_bounded_and_deterministic() {
    let kura = Kura::blank_kura_for_testing();
    let first = b"queue-plan-admission-v1:first".to_vec();
    let second = b"queue-plan-admission-v1:second".to_vec();
    let first_hash = kura
        .persist_pending_queue_plan_admission_certificate(&first)
        .expect("persist first admission certificate");
    let second_hash = kura
        .persist_pending_queue_plan_admission_certificate(&second)
        .expect("persist second admission certificate");
    assert_ne!(first_hash, second_hash);
    assert_eq!(
        kura.persist_pending_queue_plan_admission_certificate(&first)
            .expect("idempotently persist exact certificate"),
        first_hash
    );
    assert_eq!(
        kura.pending_queue_plan_admission_certificate(first_hash)
            .expect("read exact admission certificate"),
        Some(first.clone())
    );
    let mut expected = vec![(first_hash, first), (second_hash, second)];
    expected.sort_by_key(|(hash, _)| *hash);
    assert_eq!(
        kura.pending_queue_plan_admission_certificates()
            .expect("read pending admission certificates"),
        expected
    );
    assert_eq!(
        kura.pending_queue_plan_admission_certificates_bounded(1)
            .expect("read bounded admission certificates"),
        expected[..1]
    );
    assert!(
        kura.pending_queue_plan_admission_certificates_bounded(0)
            .expect("zero admission diagnostic budget")
            .is_empty()
    );
    assert_eq!(
        kura.retain_pending_queue_plan_admission_certificates(|hash, _| hash == second_hash)
            .expect("retain one exact admission certificate"),
        1
    );
    let retained = expected
        .iter()
        .find(|(hash, _)| *hash == second_hash)
        .expect("second certificate remains in expected set")
        .clone();
    assert_eq!(
        kura.pending_queue_plan_admission_certificates()
            .expect("read retained admission certificate"),
        vec![retained]
    );
    kura.remove_pending_queue_plan_admission_certificate(second_hash)
        .expect("remove committed admission certificate");
    assert!(
        kura.pending_queue_plan_admission_certificates()
            .expect("read empty admission store")
            .is_empty()
    );
    kura.remove_pending_queue_plan_admission_certificate(second_hash)
        .expect("idempotently remove absent admission certificate");
}
#[test]
fn pending_queue_plan_admission_store_rejects_empty_and_oversized_bytes() {
    let kura = Kura::blank_kura_for_testing();
    assert!(
        kura.persist_pending_queue_plan_admission_certificate(&[])
            .is_err()
    );
    let oversized = vec![0xA5; MAX_PENDING_QUEUE_PLAN_ADMISSION_CERTIFICATE_BYTES + 1];
    assert!(
        kura.persist_pending_queue_plan_admission_certificate(&oversized)
            .is_err()
    );
    assert!(
        !kura.pending_queue_plan_admission_dir().exists(),
        "rejected bytes must not create the admission store"
    );
}
#[test]
fn pending_queue_plan_admission_bytes_participate_in_exact_disk_accounting() {
    let kura = Kura::blank_kura_for_testing();
    let baseline_enforced = kura
        .kura_disk_usage_bytes()
        .expect("measure baseline enforced bytes");
    let baseline_total = kura
        .kura_total_disk_usage_bytes()
        .expect("measure baseline total bytes");
    let bytes = b"queue-plan-admission-v1:disk-accounting".to_vec();
    let hash = kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .expect("persist accounted admission certificate");
    let expected_delta = u64::try_from(bytes.len()).expect("fixture length fits u64");
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("measure enforced bytes with admission certificate"),
        baseline_enforced.saturating_add(expected_delta)
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("measure total bytes with admission certificate"),
        baseline_total.saturating_add(expected_delta)
    );
    kura.remove_pending_queue_plan_admission_certificate(hash)
        .expect("remove accounted admission certificate");
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("measure enforced bytes after removal"),
        baseline_enforced
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("measure total bytes after removal"),
        baseline_total
    );
}
#[test]
fn pending_queue_plan_admission_startup_recovers_unpublished_temporary() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let symlinked = Kura::blank_kura_for_testing();
        let outside = TempDir::new().expect("create external admission directory");
        let external_bytes = b"queue-plan-admission-v1:external-crash-temp";
        let external_hash = Hash::new(external_bytes);
        let external_temp = outside.path().join(format!(
            "{}.norito.tmp",
            hex::encode(external_hash.as_ref())
        ));
        fs::write(&external_temp, external_bytes).expect("stage external admission temporary");
        symlink(outside.path(), symlinked.pending_queue_plan_admission_dir())
            .expect("replace admission directory with a symlink");
        assert!(
            symlinked
                .validate_pending_merge_entries_on_startup()
                .is_err(),
            "startup must reject a symlinked admission directory before recovery"
        );
        assert!(
            external_temp.is_file(),
            "startup rejection must not mutate the admission symlink target"
        );
    }
    let kura = Kura::blank_kura_for_testing();
    let bytes = b"queue-plan-admission-v1:recover-unpublished".to_vec();
    let hash = Hash::new(&bytes);
    let directory = kura.pending_queue_plan_admission_dir();
    fs::create_dir(&directory).expect("create admission recovery directory");
    let temp_path = kura
        .pending_queue_plan_admission_path(hash)
        .with_extension("norito.tmp");
    fs::write(&temp_path, &bytes).expect("write durable admission temporary");
    fs::File::open(&temp_path)
        .expect("open admission temporary")
        .sync_all()
        .expect("sync admission temporary");
    sync_dir(&directory).expect("sync admission recovery directory");
    kura.validate_pending_merge_entries_on_startup()
        .expect("recover unpublished admission temporary");
    assert!(!temp_path.exists());
    assert_eq!(
        kura.pending_queue_plan_admission_certificate(hash)
            .expect("read recovered admission certificate"),
        Some(bytes)
    );
    let simultaneous = Kura::blank_kura_for_testing();
    let simultaneous_merge_directory = simultaneous.pending_merge_entry_dir();
    let simultaneous_admission_directory = simultaneous.pending_queue_plan_admission_dir();
    fs::create_dir(&simultaneous_merge_directory)
        .expect("create simultaneous merge recovery directory");
    fs::create_dir(&simultaneous_admission_directory)
        .expect("create simultaneous admission recovery directory");
    let simultaneous_entry = sample_merge_entry(106);
    let simultaneous_entry_hash = simultaneous_entry.canonical_hash();
    let simultaneous_merge_target = simultaneous.pending_merge_entry_path(simultaneous_entry_hash);
    let simultaneous_merge_temp = simultaneous_merge_target.with_extension("norito.tmp");
    fs::write(
        &simultaneous_merge_temp,
        simultaneous_entry.canonical_bytes(),
    )
    .expect("stage simultaneous pending merge temporary");
    let simultaneous_admission_bytes = b"queue-plan-admission-v1:simultaneous-crash-temp".to_vec();
    let simultaneous_admission_hash = Hash::new(&simultaneous_admission_bytes);
    let simultaneous_admission_target =
        simultaneous.pending_queue_plan_admission_path(simultaneous_admission_hash);
    let simultaneous_admission_temp = simultaneous_admission_target.with_extension("norito.tmp");
    fs::write(&simultaneous_admission_temp, &simultaneous_admission_bytes)
        .expect("stage simultaneous admission temporary");
    simultaneous
        .validate_pending_merge_entries_on_startup()
        .expect("recover one crash temporary in each pending-control namespace");
    assert!(!simultaneous_merge_temp.exists());
    assert!(!simultaneous_admission_temp.exists());
    assert_eq!(
        simultaneous
            .read_pending_merge_entry_path(
                &simultaneous_merge_target,
                Some(simultaneous_entry_hash),
            )
            .expect("read simultaneously recovered pending merge entry"),
        Some(simultaneous_entry)
    );
    assert_eq!(
        simultaneous
            .pending_queue_plan_admission_certificate(simultaneous_admission_hash)
            .expect("read simultaneously recovered admission certificate"),
        Some(simultaneous_admission_bytes)
    );
    let saturated = Kura::blank_kura_for_testing();
    let saturated_directory = saturated.pending_queue_plan_admission_dir();
    fs::create_dir(&saturated_directory).expect("create saturated admission recovery directory");
    let mut removable_path = None;
    for index in 0..saturated
        .pending_control_sidecar_limits
        .queue_plan_admissions
    {
        let stable_bytes = format!("queue-plan-admission-v1:saturated:{index}").into_bytes();
        let stable_hash = Hash::new(&stable_bytes);
        let stable_path = saturated.pending_queue_plan_admission_path(stable_hash);
        fs::write(&stable_path, stable_bytes).expect("write saturated admission certificate");
        removable_path.get_or_insert(stable_path);
    }
    let overflow_bytes = b"queue-plan-admission-v1:saturated:overflow".to_vec();
    let overflow_hash = Hash::new(&overflow_bytes);
    let overflow_target = saturated.pending_queue_plan_admission_path(overflow_hash);
    let overflow_temp = overflow_target.with_extension("norito.tmp");
    fs::write(&overflow_temp, &overflow_bytes)
        .expect("write saturated unpublished admission temporary");
    sync_dir(&saturated_directory).expect("sync saturated admission recovery directory");
    assert!(
        saturated
            .validate_pending_merge_entries_on_startup()
            .is_err(),
        "recovery must not publish a 1,025th stable admission certificate"
    );
    assert!(overflow_temp.is_file());
    assert!(!overflow_target.exists());
    fs::remove_file(removable_path.expect("one saturated certificate path"))
        .expect("free one stable admission slot");
    sync_dir(&saturated_directory).expect("sync freed admission recovery slot");
    saturated
        .validate_pending_merge_entries_on_startup()
        .expect("recover unpublished admission temporary after one slot is free");
    assert!(!overflow_temp.exists());
    assert_eq!(
        saturated
            .pending_queue_plan_admission_certificate(overflow_hash)
            .expect("read capacity-bound recovered admission certificate"),
        Some(overflow_bytes)
    );
    assert_eq!(
        saturated
            .pending_queue_plan_admission_certificates()
            .expect("read exact saturated admission store")
            .len(),
        saturated
            .pending_control_sidecar_limits
            .queue_plan_admissions
    );
    let shared_saturated = Kura::blank_kura_for_testing();
    let merge_directory = shared_saturated.pending_merge_entry_dir();
    let admission_directory = shared_saturated.pending_queue_plan_admission_dir();
    fs::create_dir(&merge_directory).expect("create shared-cap merge directory");
    fs::create_dir(&admission_directory).expect("create shared-cap admission directory");
    let merge_file_bytes =
        u64::try_from(MAX_MERGE_LEDGER_ENTRY_BYTES).expect("merge byte limit fits u64");
    let merge_saturation_files = shared_saturated
        .pending_control_sidecar_limits
        .aggregate_bytes
        / MAX_MERGE_LEDGER_ENTRY_BYTES;
    for index in 0..merge_saturation_files {
        let hash = HashOf::<MergeLedgerEntry>::from_untyped_unchecked(Hash::new(format!(
            "shared-cap-merge-{index}"
        )));
        let file = fs::File::create(shared_saturated.pending_merge_entry_path(hash))
            .expect("create sparse merge saturation file");
        file.set_len(merge_file_bytes)
            .expect("size sparse merge saturation file");
    }
    let overflow_bytes = b"queue-plan-admission-v1:shared-cap-overflow".to_vec();
    let overflow_hash = Hash::new(&overflow_bytes);
    let overflow_target = shared_saturated.pending_queue_plan_admission_path(overflow_hash);
    let overflow_temp = overflow_target.with_extension("norito.tmp");
    fs::write(&overflow_temp, overflow_bytes)
        .expect("stage admission temporary above the shared cap");
    assert!(
        shared_saturated
            .validate_pending_merge_entries_on_startup()
            .is_err(),
        "admission recovery must enforce the shared pending-control byte cap before publication"
    );
    assert!(overflow_temp.is_file());
    assert!(!overflow_target.exists());
}
#[cfg(any(unix, windows))]
#[test]
fn pending_queue_plan_admission_startup_completes_hard_link_publication() {
    let kura = Kura::blank_kura_for_testing();
    let bytes = b"queue-plan-admission-v1:recover-linked".to_vec();
    let hash = Hash::new(&bytes);
    let directory = kura.pending_queue_plan_admission_dir();
    fs::create_dir(&directory).expect("create admission recovery directory");
    let target_path = kura.pending_queue_plan_admission_path(hash);
    let temp_path = target_path.with_extension("norito.tmp");
    fs::write(&temp_path, &bytes).expect("write durable admission temporary");
    fs::hard_link(&temp_path, &target_path).expect("publish admission hard link");
    sync_dir(&directory).expect("sync linked admission recovery directory");
    let staged_enforced = kura
        .refresh_disk_usage_bytes()
        .expect("cache hard-linked admission recovery bytes");
    let staged_total = kura
        .disk_usage_bytes()
        .expect("read cached hard-linked admission recovery bytes");
    assert_eq!(
        kura.persist_pending_queue_plan_admission_certificate(&bytes)
            .expect("idempotent retry must finish linked admission publication"),
        hash
    );
    assert!(!temp_path.exists());
    assert!(target_path.is_file());
    let path_bytes = u64::try_from(bytes.len()).expect("fixture length fits u64");
    assert_eq!(
        kura.pending_queue_plan_admission_certificate(hash)
            .expect("read recovered linked certificate"),
        Some(bytes)
    );
    assert!(Kura::sidecar_is_single_link(
        &fs::symlink_metadata(&target_path).expect("read recovered target metadata")
    ));
    let recovered_enforced = kura
        .kura_disk_usage_bytes()
        .expect("scan recovered admission bytes");
    let recovered_total = kura
        .kura_total_disk_usage_bytes()
        .expect("scan total recovered admission bytes");
    assert_eq!(
        recovered_enforced,
        staged_enforced.saturating_sub(path_bytes)
    );
    assert_eq!(recovered_total, staged_total.saturating_sub(path_bytes));
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("read published recovered admission accounting"),
        recovered_total,
        "idempotent retry must publish the crash-temporary removal delta"
    );
}
#[test]
fn pending_queue_plan_admission_survives_retired_purge_and_process_reopen() {
    let directory = TempDir::new().expect("temporary Kura root");
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize Kura");
    let bytes = b"queue-plan-admission-v1:survive-purge-and-reopen".to_vec();
    let hash = kura
        .persist_pending_queue_plan_admission_certificate(&bytes)
        .expect("persist admission certificate before purge");
    let retired_root = directory.path().join("retired");
    let retired_blocks = lane_config.primary().blocks_dir(&retired_root);
    fs::create_dir_all(&retired_blocks).expect("create disposable retired blocks");
    fs::write(retired_blocks.join(DATA_FILE_NAME), b"disposable")
        .expect("write disposable retired block bytes");
    assert!(
        kura.purge_retired_segments()
            .expect("purge unrelated retired storage")
    );
    assert_eq!(
        kura.pending_queue_plan_admission_certificate(hash)
            .expect("read admission certificate after purge"),
        Some(bytes.clone())
    );
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen with durable pending admission certificate");
    assert_eq!(
        reopened
            .pending_queue_plan_admission_certificate(hash)
            .expect("read admission certificate after process reopen"),
        Some(bytes)
    );
}
#[test]
fn pending_queue_plan_admission_startup_rejects_conflicting_publication_without_deletion() {
    let kura = Kura::blank_kura_for_testing();
    let temp_bytes = b"queue-plan-admission-v1:conflict-temp";
    let target_bytes = b"queue-plan-admission-v1:conflict-target";
    let hash = Hash::new(temp_bytes);
    let directory = kura.pending_queue_plan_admission_dir();
    fs::create_dir(&directory).expect("create admission recovery directory");
    let target_path = kura.pending_queue_plan_admission_path(hash);
    let temp_path = target_path.with_extension("norito.tmp");
    fs::write(&temp_path, temp_bytes).expect("write conflicting admission temporary");
    fs::write(&target_path, target_bytes).expect("write conflicting admission target");
    sync_dir(&directory).expect("sync conflicting admission recovery directory");
    assert!(
        kura.validate_pending_merge_entries_on_startup().is_err(),
        "conflicting publication identities must fail closed"
    );
    assert_eq!(
        fs::read(&temp_path).expect("conflicting temporary remains"),
        temp_bytes
    );
    assert_eq!(
        fs::read(&target_path).expect("conflicting target remains"),
        target_bytes
    );
}
#[test]
fn pending_queue_plan_admission_startup_rejects_oversized_artifact() {
    let kura = Kura::blank_kura_for_testing();
    let bytes = b"queue-plan-admission-v1:oversized-path-identity";
    let hash = Hash::new(bytes);
    let directory = kura.pending_queue_plan_admission_dir();
    fs::create_dir(&directory).expect("create oversized admission directory");
    let path = kura.pending_queue_plan_admission_path(hash);
    let file = fs::File::create(&path).expect("create oversized admission artifact");
    file.set_len(
        u64::try_from(MAX_PENDING_QUEUE_PLAN_ADMISSION_CERTIFICATE_BYTES)
            .expect("certificate limit fits u64")
            .saturating_add(1),
    )
    .expect("materialize oversized admission artifact");
    file.sync_all().expect("sync oversized admission artifact");
    sync_dir(&directory).expect("sync oversized admission directory");
    assert!(
        kura.validate_pending_merge_entries_on_startup().is_err(),
        "oversized admission artifact must fail closed at startup"
    );
    assert_eq!(
        fs::metadata(&path)
            .expect("oversized admission artifact remains")
            .len(),
        u64::try_from(MAX_PENDING_QUEUE_PLAN_ADMISSION_CERTIFICATE_BYTES)
            .expect("certificate limit fits u64")
            .saturating_add(1)
    );
}
#[cfg(unix)]
#[test]
fn pending_queue_plan_admission_store_rejects_symlink_and_unexpected_artifacts() {
    use std::os::unix::fs::symlink;
    for unexpected in ["symlink", "unexpected"] {
        let kura = Kura::blank_kura_for_testing();
        let directory = kura.pending_queue_plan_admission_dir();
        fs::create_dir(&directory).expect("create malformed admission directory");
        match unexpected {
            "symlink" => {
                let bytes = b"queue-plan-admission-v1:symlink";
                let target = kura.store_root().join("outside-admission.norito");
                fs::write(&target, bytes).expect("write symlink target");
                let hash = Hash::new(bytes);
                symlink(&target, kura.pending_queue_plan_admission_path(hash))
                    .expect("create admission symlink");
            }
            "unexpected" => {
                fs::write(directory.join("surprise.txt"), b"unexpected")
                    .expect("write unexpected admission artifact");
            }
            _ => unreachable!("fixed malformed artifact table"),
        }
        assert!(
            kura.pending_queue_plan_admission_certificates().is_err(),
            "{unexpected} admission artifact must fail closed"
        );
    }
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
fn merge_ledger_entries_persist_across_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = kura_config_for_dir(&dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("init kura");
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let mut entry1 = sample_merge_entry(1);
    let block1 = next_merge_carrier(&mut blocks, &mut entry1);
    let block1_height = NonZeroUsize::new(
        usize::try_from(block1.header().height().get()).expect("carrier height fits usize"),
    )
    .expect("carrier height is non-zero");
    let block1_hash = block1.hash();
    let mut entry2 = sample_merge_entry(2);
    let block2 = next_merge_carrier(&mut blocks, &mut entry2);
    let block2_height = NonZeroUsize::new(
        usize::try_from(block2.header().height().get()).expect("carrier height fits usize"),
    )
    .expect("carrier height is non-zero");
    let block2_hash = block2.hash();
    kura.store_block(parent).expect("store carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&block1), &entry1)
        .expect("store block+entry1");
    kura.store_block_with_merge_entry(Arc::clone(&block2), &entry2)
        .expect("store block+entry2");
    let _ = persist_v2_finality_chain_through(&kura, block2_height);
    assert_eq!(
        kura.merge_ledger_snapshot(),
        vec![entry1.clone(), entry2.clone()]
    );
    let carrier_records = [
        (entry1.canonical_hash(), block1.hash()),
        (entry2.canonical_hash(), block2.hash()),
    ];
    drop(kura);
    let (kura_reloaded, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("reopen kura");
    let snapshot = kura_reloaded.merge_ledger_snapshot();
    assert_eq!(snapshot, vec![entry1.clone(), entry2.clone()]);
    for (entry_hash, carrier_hash) in carrier_records {
        assert_eq!(
            kura_reloaded
                .merge_carrier_for_entry(entry_hash)
                .expect("lookup carrier after restart")
                .map(|record| record.block_hash),
            Some(carrier_hash),
            "merge log and sparse carrier index must survive together"
        );
    }
    for (height, hash, expected) in [
        (block1_height, block1_hash, &entry1),
        (block2_height, block2_hash, &entry2),
    ] {
        let block = kura_reloaded
            .get_block(height)
            .expect("reloaded merge carrier body");
        assert_eq!(block.hash(), hash);
        let resolved = kura_reloaded
            .get_merge_entry_by_carrier_height(height)
            .expect("validate reloaded merge carrier")
            .expect("merge entry at carrier height");
        assert_eq!(resolved.canonical_hash(), expected.canonical_hash());
    }
}
#[test]
fn committed_merge_entry_lookup_reconstructs_from_canonical_indexes_after_restart() {
    let dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&dir, nonzero!(2_usize));
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("open Kura");
    let (entrypoint_hash, reservation, _) = store_indexed_reservation_carrier(kura.as_ref(), 0x61);
    let assert_exact_reservation = |kura: &Kura| {
        let entry = kura
            .get_merge_entry_by_carrier_height(nonzero!(2_usize))
            .expect("resolve indexed merge carrier")
            .expect("merge carrier has an indexed entry");
        assert_eq!(
            crate::state::certified_merge_queue_reservations(&entry)
                .expect("decode exact committed reservation"),
            vec![(entrypoint_hash, reservation)]
        );
    };
    assert_exact_reservation(kura.as_ref());
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen Kura");
    reopened.reset_merge_query_read_counters_for_test();
    assert_exact_reservation(reopened.as_ref());
    let (full_history_scans, _, indexed_lookups) = reopened.merge_query_read_counters_for_test();
    assert_eq!(
        full_history_scans, 0,
        "carrier lookup must not materialize merge history"
    );
    assert_eq!(
        indexed_lookups, 1,
        "carrier lookup must decode only its exact merge sidecar"
    );
}
#[test]
fn merge_frontier_startup_requires_geometry_only_after_committed_execution() {
    let dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&dir, nonzero!(2_usize));
    let lane_config = RuntimeLaneConfig::default();
    let (fresh, _) = Kura::new_fresh_single_lane(&config, &lane_config).expect("open fresh Kura");
    let entry = fresh
        .lane_storage_entry(LaneId::SINGLE)
        .expect("fresh primary lane storage entry");
    let marker_path = entry
        .blocks_dir(&fresh.store_root)
        .join(".lane-incarnation.norito");
    assert!(
        !marker_path.exists(),
        "a fresh single-lane route starts without execution geometry"
    );
    drop(fresh);
    let configured_catalog = LaneCatalog::default();
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&config, &lane_config, &configured_catalog)
            .expect("a fresh route without frontier or execution may reopen");
    let _ = store_indexed_reservation_carrier(kura.as_ref(), 0x62);
    assert!(
        kura.merge_log
            .lock()
            .has_execution_for_route(entry.lane_id, entry.dataspace_id),
        "fixture must commit an autonomous merge execution before removing geometry"
    );
    assert!(
        marker_path.is_file(),
        "the committed-execution fixture must install its exact incarnation marker"
    );
    fs::remove_file(&marker_path).expect("remove exact committed-execution geometry");
    sync_dir(
        marker_path
            .parent()
            .expect("lane incarnation marker has a parent"),
    )
    .expect("sync missing-geometry crash image");
    drop(kura);
    let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
        Ok(_) => panic!("committed execution without its exact geometry must fail closed"),
        Err(error) => error,
    };
    assert!(
        matches!(
            &error,
            Error::IO(source, path)
                if source.kind() == ErrorKind::NotFound && path == &marker_path
        ),
        "startup must identify the missing lane-incarnation geometry: {error}"
    );
}
#[test]
fn committed_merge_entry_lookup_fails_closed_on_log_mutation() {
    for mutation in ["corrupt", "truncate", "oversize"] {
        let dir = TempDir::new().expect("tempdir");
        let config = kura_config_for_dir(&dir, nonzero!(2_usize));
        let lane_config = RuntimeLaneConfig::default();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("open Kura");
        let (_, _, frame) = store_indexed_reservation_carrier(kura.as_ref(), 0x71);
        let path = kura.active_merge_path.lock().clone();
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open merge log for mutation");
        match mutation {
            "corrupt" => {
                let offset = frame
                    .frame_offset
                    .saturating_add(4)
                    .saturating_add(u64::from(frame.payload_len))
                    .saturating_sub(1);
                file.seek(SeekFrom::Start(offset))
                    .expect("seek reservation frame tail");
                let mut byte = [0u8; 1];
                file.read_exact(&mut byte).expect("read reservation frame");
                byte[0] ^= 0x80;
                file.seek(SeekFrom::Start(offset))
                    .expect("rewind reservation frame");
                file.write_all(&byte).expect("corrupt reservation frame");
            }
            "truncate" => {
                file.set_len(
                    frame
                        .frame_offset
                        .saturating_add(4)
                        .saturating_add(u64::from(frame.payload_len))
                        .saturating_sub(1),
                )
                .expect("truncate reservation frame");
            }
            "oversize" => {
                file.seek(SeekFrom::End(0)).expect("seek merge log tail");
                let oversized =
                    u32::try_from(MAX_MERGE_LEDGER_ENTRY_BYTES + 1).expect("limit fits u32");
                file.write_all(&oversized.to_le_bytes())
                    .expect("append oversized frame length");
                file.set_len(
                    frame
                        .frame_offset
                        .saturating_add(4)
                        .saturating_add(u64::from(frame.payload_len))
                        .saturating_add(4)
                        .saturating_add(u64::from(oversized)),
                )
                .expect("materialize oversized sparse tail");
            }
            _ => unreachable!("fixed mutation table"),
        }
        file.sync_all().expect("sync merge log mutation");
        assert!(
            kura.get_merge_entry_by_carrier_height(nonzero!(2_usize))
                .is_err(),
            "carrier lookup must fail closed after {mutation}"
        );
    }
}
#[test]
fn canonical_transaction_index_exposes_completeness_and_all_carrier_heights() {
    let dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&dir, nonzero!(2_usize));
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("open Kura");
    let (entrypoint_hash, _, _) = store_indexed_reservation_carrier(kura.as_ref(), 0x81);
    kura.transaction_entrypoint_index.lock().complete = false;
    assert_eq!(
        kura.get_block_heights_by_entrypoint_hash(entrypoint_hash),
        None,
        "an incomplete canonical transaction index must fail closed"
    );
    {
        let mut index = kura.transaction_entrypoint_index.lock();
        index.complete = true;
        index
            .heights_by_entrypoint
            .get_mut(&entrypoint_hash)
            .expect("fixture transaction is indexed")
            .insert(nonzero!(3_usize));
    }
    assert_eq!(
        kura.get_block_heights_by_entrypoint_hash(entrypoint_hash),
        Some(BTreeSet::from([nonzero!(2_usize), nonzero!(3_usize)])),
        "the planner must receive every conflicting canonical carrier height"
    );
}
#[test]
fn merge_log_hash_index_reads_evicted_frames_without_full_scan() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("merge.log");
    let first_hash;
    {
        let mut log = MergeLedgerLog::open_at(&path, 2).expect("open merge log");
        first_hash = sample_merge_entry(1).canonical_hash();
        for epoch in 1..=128 {
            log.append(&sample_merge_entry(epoch))
                .expect("append indexed merge frame");
        }
        assert_eq!(log.snapshot().len(), 2, "test must evict old cache entries");
    }
    let mut reopened = MergeLedgerLog::open_at(&path, 2).expect("reopen indexed merge log");
    reopened.full_history_scans = 0;
    reopened.indexed_lookups = 0;
    let first = reopened
        .entry_by_hash(first_hash)
        .expect("bounded indexed read")
        .expect("old indexed frame");
    assert_eq!(first.epoch_id, 1);
    assert_eq!(reopened.full_history_scans, 0);
    assert_eq!(reopened.indexed_lookups, 1);
}
#[test]
fn merge_log_complete_snapshot_streams_existing_frame_index_once() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("merge.log");
    {
        let mut log = MergeLedgerLog::open_at(&path, 2).expect("open merge log");
        for epoch in 1..=64 {
            log.append(&sample_merge_entry(epoch))
                .expect("append merge frame");
        }
    }
    let mut reopened = MergeLedgerLog::open_at(&path, 2).expect("reopen merge log");
    let frames_by_hash = reopened.frames_by_hash.clone();
    let frames_by_epoch = reopened.frames_by_epoch.clone();
    reopened.full_history_scans = 0;
    reopened.indexed_lookups = 0;
    let entries = reopened.all_entries().expect("stream complete snapshot");
    assert_eq!(entries.len(), 64);
    assert_eq!(reopened.snapshot().len(), 2);
    assert_eq!(reopened.frames_by_hash, frames_by_hash);
    assert_eq!(reopened.frames_by_epoch, frames_by_epoch);
    assert_eq!(reopened.full_history_scans, 1);
    assert_eq!(
        reopened.indexed_lookups, 0,
        "sequential recovery must not perform hash-index point lookups"
    );
}
#[test]
fn merge_log_indexed_lookup_fails_closed_on_corruption_and_truncation() {
    for truncate in [false, true] {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("merge.log");
        let mut log = MergeLedgerLog::open_at(&path, 1).expect("open merge log");
        for epoch in 1..=3 {
            log.append(&sample_merge_entry(epoch))
                .expect("append indexed merge frame");
        }
        let target = sample_merge_entry(3).canonical_hash();
        let frame = log.frames_by_hash[&target];
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open merge bytes for adversarial mutation");
        if truncate {
            file.set_len(
                frame
                    .frame_offset
                    .saturating_add(4)
                    .saturating_add(u64::from(frame.payload_len))
                    .saturating_sub(1),
            )
            .expect("truncate indexed payload");
        } else {
            let mut byte = [0u8; 1];
            file.seek(SeekFrom::Start(
                frame
                    .frame_offset
                    .saturating_add(4)
                    .saturating_add(u64::from(frame.payload_len))
                    .saturating_sub(1),
            ))
            .expect("seek indexed payload tail");
            file.read_exact(&mut byte).expect("read payload tail");
            byte[0] ^= 0x80;
            file.seek(SeekFrom::Current(-1))
                .expect("rewind payload tail");
            file.write_all(&byte).expect("corrupt payload tail");
            file.sync_all().expect("sync corrupt payload");
        }
        assert!(
            log.entry_by_hash(target).is_err(),
            "indexed lookup must fail closed after {}",
            if truncate { "truncation" } else { "corruption" }
        );
    }
}
#[test]
fn merge_log_truncate_rebuilds_exact_indexes_and_survives_reopen() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("merge.log");
    let hashes = (1..=6)
        .map(|epoch| sample_merge_entry(epoch).canonical_hash())
        .collect::<Vec<_>>();
    {
        let mut log = MergeLedgerLog::open_at(&path, 2).expect("open merge log");
        for epoch in 1..=6 {
            log.append(&sample_merge_entry(epoch))
                .expect("append merge frame");
        }
        log.truncate_to_len(3).expect("truncate indexed log");
        assert_eq!(log.total_entries, 3);
        assert_eq!(log.frames_by_hash.len(), 3);
        assert_eq!(log.frames_by_epoch.len(), 3);
        assert_eq!(log.snapshot().len(), 2, "cache remains capacity bounded");
        assert!(log.entry_by_hash(hashes[0]).expect("old lookup").is_some());
        assert!(
            log.entry_by_hash(hashes[3])
                .expect("removed lookup")
                .is_none()
        );
        log.append(&sample_merge_entry(4))
            .expect("append after truncate");
    }
    let mut reopened = MergeLedgerLog::open_at(&path, 1).expect("reopen truncated log");
    assert_eq!(reopened.total_entries, 4);
    assert_eq!(reopened.frames_by_hash.len(), 4);
    assert_eq!(reopened.frames_by_epoch.len(), 4);
    assert_eq!(reopened.snapshot().len(), 1);
    assert_eq!(
        reopened
            .entry_by_hash(sample_merge_entry(4).canonical_hash())
            .expect("indexed tail lookup")
            .expect("retained tail")
            .epoch_id,
        4
    );
    assert!(
        reopened
            .entry_by_hash(hashes[4])
            .expect("discarded suffix lookup")
            .is_none()
    );
}
#[test]
fn merge_log_reopen_rejects_duplicate_or_noncontiguous_complete_frame() {
    for (label, appended) in [
        ("duplicate hash and epoch", sample_merge_entry(1)),
        ("non-contiguous epoch", sample_merge_entry(3)),
    ] {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("merge.log");
        {
            let mut log = MergeLedgerLog::open_at(&path, 2).expect("open merge log");
            log.append(&sample_merge_entry(1))
                .expect("append epoch one");
        }
        let bytes = appended.encode();
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open raw merge log");
        file.write_all(
            &u32::try_from(bytes.len())
                .expect("frame length")
                .to_le_bytes(),
        )
        .expect("write frame length");
        file.write_all(&bytes).expect("write complete frame");
        file.sync_all().expect("sync complete frame");
        let err =
            MergeLedgerLog::open_at(&path, 2).expect_err("invalid complete frame must fail closed");
        assert!(
            matches!(err, Error::MergeCarrierConflict(_)),
            "unexpected {label} error: {err}"
        );
    }
}
#[test]
fn merge_log_reopen_rejects_zero_length_or_complete_oversized_frame() {
    for oversized in [false, true] {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("merge.log");
        let declared = if oversized {
            u32::try_from(MAX_MERGE_LEDGER_ENTRY_BYTES + 1).expect("entry limit fits u32")
        } else {
            0
        };
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .expect("create malformed merge log");
        file.write_all(&declared.to_le_bytes())
            .expect("write malformed frame length");
        if oversized {
            file.set_len(4 + u64::from(declared))
                .expect("materialize complete oversized sparse frame");
        }
        file.sync_all().expect("sync malformed frame");
        assert!(matches!(
            MergeLedgerLog::open_at(&path, 1),
            Err(Error::MergeCarrierConflict(_))
        ));
    }
}
#[test]
fn pending_selection_performs_indexed_checks_only_for_pending_entries() {
    let kura = Kura::blank_kura_for_testing();
    for epoch in 1..=128 {
        kura.append_merge_entry_for_test(&sample_merge_entry(epoch))
            .expect("seed committed merge history");
    }
    for salt in 0_u8..8 {
        let mut pending = sample_merge_entry(129);
        pending.lane_catalog_hash = Hash::new([salt]);
        kura.persist_pending_certified_merge_entry(&pending)
            .expect("persist pending sidecar");
    }
    {
        let mut log = kura.merge_log.lock();
        log.full_history_scans = 0;
        log.indexed_membership_checks = 0;
    }
    let selected = kura
        .select_pending_certified_merge_entry()
        .expect("select pending entry")
        .expect("pending entry exists");
    assert_eq!(selected.1.epoch_id, 129);
    let log = kura.merge_log.lock();
    assert_eq!(log.full_history_scans, 0);
    assert_eq!(
        log.indexed_membership_checks, 8,
        "selection must scale with pending entries, not committed history"
    );
}
#[test]
fn carrier_point_lookup_uses_initialized_height_and_hash_maps() {
    let kura = Kura::blank_kura_for_testing();
    let (carrier, entry) = store_genesis_and_build_merge_carrier(&kura, 1);
    let carrier_hash = carrier.hash();
    let entry_hash = entry.canonical_hash();
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("store merge carrier");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    let scans = kura.merge_carrier_index.lock().directory_scans;
    for _ in 0..32 {
        assert_eq!(
            kura.merge_carrier_for_entry(entry_hash)
                .expect("lookup carrier by entry")
                .map(|record| (record.block_height, record.block_hash)),
            Some((2, carrier_hash))
        );
        assert_eq!(
            kura.merge_entry_for_carrier(2, carrier_hash)
                .expect("lookup entry by carrier")
                .as_ref(),
            Some(&entry)
        );
    }
    assert_eq!(
        kura.merge_carrier_index.lock().directory_scans,
        scans,
        "point lookups must not rescan the carrier directory"
    );
}
#[test]
fn carrier_lookup_requires_finality_even_while_body_is_present() {
    let kura = Kura::blank_kura_for_testing();
    let (carrier, entry) = store_genesis_and_build_merge_carrier(&kura, 1);
    let carrier_height = carrier.header().height().get();
    let carrier_hash = carrier.hash();
    let entry_hash = entry.canonical_hash();
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("store merge carrier without finality");
    assert!(
        kura.get_block_without_merge_sidecar(nonzero!(2_usize))
            .is_some()
    );
    assert!(matches!(
        kura.merge_carrier_for_entry(entry_hash),
        Err(Error::MergeCarrierConflict(_))
    ));
    assert!(matches!(
        kura.merge_entry_for_carrier(carrier_height, carrier_hash),
        Err(Error::MergeCarrierConflict(_))
    ));
    assert!(matches!(
        kura.merge_carrier_records(),
        Err(Error::MergeCarrierConflict(_))
    ));
}
#[test]
fn finality_store_rejects_missing_or_wrong_merge_carrier_projection() {
    let wrong_entry_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong finality merge-carrier entry"));
    let projections = [
        None,
        Some(
            iroha_data_model::block::consensus_v2::MergeCarrierCommitmentV1::new(wrong_entry_hash),
        ),
    ];
    for projection in projections {
        let (kura, mut blocks) = blank_kura_with_blocks();
        let genesis = blocks.next();
        let mut entry = sample_merge_entry(1);
        let carrier = next_merge_carrier(&mut blocks, &mut entry);
        kura.store_block(Arc::clone(&genesis))
            .expect("store carrier parent");
        kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
            .expect("store merge carrier");
        let keypairs = v2_finality_fixture_keys();
        let parent = v2_finality_artifact_for_block_with_keys(
            genesis.as_ref(),
            None,
            &keypairs,
            v2_finality_fixture_execution_commitment(),
        );
        let _ = kura
            .store_v2_finality_artifact(&parent)
            .expect("persist parent finality");
        let malformed = v2_finality_artifact_for_block_with_keys_and_merge_carrier(
            carrier.as_ref(),
            Some(&parent),
            &keypairs,
            v2_finality_fixture_execution_commitment(),
            projection,
        );
        malformed
            .verify()
            .expect("malformed projection remains self-consistently signed");
        assert!(matches!(
            kura.store_v2_finality_artifact(&malformed),
            Err(Error::MergeCarrierConflict(_))
        ));
        assert!(!kura.v2_finality_artifact_path(2).exists());
    }
}
#[test]
fn finality_authenticated_carrier_survives_body_removal_and_restart() {
    let dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&dir, nonzero!(1_usize));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize Kura");
    let mut blocks = DummyBlocks::new();
    let genesis = blocks.next();
    let mut entry = sample_merge_entry(1);
    let carrier = next_merge_carrier(&mut blocks, &mut entry);
    let tail = blocks.next();
    let entry_hash = entry.canonical_hash();
    let carrier_hash = carrier.hash();
    kura.store_block(genesis).expect("store carrier parent");
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("store merge carrier");
    kura.store_block(tail).expect("store eviction tail");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    assert_eq!(
        kura.merge_carrier_for_entry(entry_hash)
            .expect("validate body-present carrier")
            .map(|record| record.block_hash),
        Some(carrier_hash)
    );
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    assert!(
        kura.evict_block_bodies(payload_len)
            .expect("evict finalized carrier body")
            >= payload_len
    );
    kura.remove_evicted_block_sidecar_for_testing(nonzero!(2_usize))
        .expect("remove local remote-only cache");
    assert!(
        kura.get_block_without_merge_sidecar(nonzero!(2_usize))
            .is_none()
    );
    assert_eq!(
        kura.merge_carrier_for_entry(entry_hash)
            .expect("validate bodyless carrier")
            .map(|record| record.block_hash),
        Some(carrier_hash)
    );
    drop(kura);
    let (reopened, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("reopen bodyless carrier");
    assert!(
        reopened
            .get_block_without_merge_sidecar(nonzero!(2_usize))
            .is_none()
    );
    assert_eq!(
        reopened
            .merge_carrier_for_entry(entry_hash)
            .expect("validate bodyless carrier after restart")
            .map(|record| record.block_hash),
        Some(carrier_hash)
    );
}
#[test]
fn bodyless_finalized_execution_carrier_rebuilds_merge_entrypoint_index() {
    let dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&dir, nonzero!(1_usize));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize Kura");
    let entrypoint = offline_top_up_entrypoint_for_index([0x71; 32], [0x72; 32]);
    let entrypoint_hash = entrypoint.hash();
    let mut entry = merge_entry_with_indexed_entrypoint(entrypoint);
    let genesis: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    let genesis = Arc::new(genesis);
    let mut raw_carrier: SignedBlock =
        BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
            .chain(0, Some(genesis.as_ref()))
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into();
    raw_carrier
        .set_transaction_results(Vec::new(), &[], Vec::new())
        .expect("attach empty ordinary carrier results");
    assert!(raw_carrier.header().merkle_root().is_none());
    assert!(raw_carrier.header().result_merkle_root().is_none());
    let batch = entry
        .execution_batch
        .as_mut()
        .expect("index fixture has an execution batch");
    batch.application_block_header =
        crate::merge::merge_application_header_from_carrier(&raw_carrier.header());
    batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
    let descriptor = batch
        .lanes
        .first()
        .expect("index fixture has one lane")
        .proposal
        .descriptor
        .clone();
    let lane_entry = kura
        .lane_storage_entry(descriptor.lane_id)
        .expect("index fixture targets an active lane");
    kura.install_lane_incarnation_marker_for_test(&lane_entry, descriptor.lane_incarnation, 0)
        .expect("install index fixture lane incarnation");
    let carrier = bind_merge_entry_to_carrier(Arc::new(raw_carrier), &mut entry);
    let carrier_hash = carrier.hash();
    kura.store_block(genesis).expect("store carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("store execution carrier");
    let tail: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, Some(carrier.as_ref()))
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    kura.store_block(Arc::new(tail))
        .expect("store eviction tail");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    assert!(
        kura.evict_block_bodies(payload_len)
            .expect("evict finalized execution carrier")
            >= payload_len
    );
    kura.remove_evicted_block_sidecar_for_testing(nonzero!(2_usize))
        .expect("remove local remote-only carrier cache");
    drop(kura);
    let (reopened, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("reopen bodyless carrier");
    assert!(
        reopened
            .get_block_without_merge_sidecar(nonzero!(2_usize))
            .is_none()
    );
    assert_eq!(
        reopened.get_block_heights_by_entrypoint_hash(entrypoint_hash),
        Some(BTreeSet::from([nonzero!(2_usize)]))
    );
    assert_eq!(
        reopened
            .merge_carrier_for_entry(entry.canonical_hash())
            .expect("validate rebuilt bodyless execution carrier")
            .map(|record| record.block_hash),
        Some(carrier_hash)
    );
}
#[test]
fn carrier_point_and_write_paths_do_not_clone_the_full_inventory() {
    let kura = Kura::blank_kura_for_testing();
    let (carrier, entry) = store_genesis_and_build_merge_carrier(&kura, 1);
    let carrier_hash = carrier.hash();
    let entry_hash = entry.canonical_hash();
    let clones_before = kura.merge_carrier_index.lock().full_inventory_clones;
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("store merge carrier through indexed write path");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    for _ in 0..16 {
        assert!(
            kura.merge_carrier_for_entry(entry_hash)
                .expect("point lookup by entry")
                .is_some()
        );
        assert!(
            kura.merge_entry_for_carrier(2, carrier_hash)
                .expect("point lookup by carrier")
                .is_some()
        );
    }
    assert_eq!(
        kura.merge_carrier_index.lock().full_inventory_clones,
        clones_before,
        "point reads and writes must operate directly on initialized maps"
    );
}
#[test]
fn carrier_index_rejects_duplicate_height_or_entry_without_rescan() {
    let kura = Kura::blank_kura_for_testing();
    let (carrier, entry) = store_genesis_and_build_merge_carrier(&kura, 1);
    let carrier_hash = carrier.hash();
    let entry_hash = entry.canonical_hash();
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("store merge carrier");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    let scans = kura.merge_carrier_index.lock().directory_scans;
    let existing = kura
        .merge_carrier_for_entry(entry_hash)
        .expect("lookup carrier")
        .expect("carrier exists");
    let duplicate_height = MergeLedgerCarrierRecord {
        entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"different-entry")),
        epoch_id: 2,
        ..existing
    };
    let duplicate_entry = MergeLedgerCarrierRecord {
        block_height: 3,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"different-block")),
        ..existing
    };
    let _guard = kura.merge_carrier_lock.lock();
    assert!(matches!(
        kura.write_merge_carrier_record_unlocked(duplicate_height),
        Err(Error::MergeCarrierConflict(_))
    ));
    assert!(matches!(
        kura.write_merge_carrier_record_unlocked(duplicate_entry),
        Err(Error::MergeCarrierConflict(_))
    ));
    drop(_guard);
    assert_eq!(
        kura.merge_carrier_index.lock().directory_scans,
        scans,
        "duplicate checks must use the initialized maps"
    );
    assert_eq!(
        kura.merge_entry_for_carrier(2, carrier_hash)
            .expect("canonical record remains readable")
            .as_ref(),
        Some(&entry)
    );
}
#[test]
fn carrier_index_reopen_rejects_duplicate_entry_hash_at_another_height() {
    let dir = TempDir::new().expect("tempdir");
    let config = kura_config_for_dir(&dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("initialize Kura");
    let (carrier, entry) = store_genesis_and_build_merge_carrier(&kura, 1);
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("store merge carrier");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    let existing = kura
        .merge_carrier_for_entry(entry.canonical_hash())
        .expect("lookup carrier")
        .expect("carrier exists");
    let duplicate = MergeLedgerCarrierRecord {
        block_height: 3,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"duplicate-carrier-block")),
        ..existing
    };
    fs::write(
        kura.merge_carrier_path(3),
        norito::to_bytes(&duplicate).expect("encode duplicate carrier"),
    )
    .expect("inject duplicate carrier file");
    drop(kura);
    assert!(
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .is_err(),
        "restart must reject duplicate entry hashes in sparse carrier files"
    );
}
#[test]
fn carrier_point_lookup_fails_closed_after_sidecar_corruption() {
    let kura = Kura::blank_kura_for_testing();
    let (carrier, entry) = store_genesis_and_build_merge_carrier(&kura, 1);
    let entry_hash = entry.canonical_hash();
    kura.store_block_with_merge_entry(carrier, &entry)
        .expect("store merge carrier");
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    let path = kura.merge_carrier_path(2);
    let mut bytes = fs::read(&path).expect("read carrier record");
    let last = bytes.last_mut().expect("non-empty carrier record");
    *last ^= 0x80;
    fs::write(&path, bytes).expect("corrupt carrier record");
    assert!(matches!(
        kura.merge_carrier_for_entry(entry_hash),
        Err(Error::MergeCarrierConflict(_)) | Err(Error::NoritoFrame(_))
    ));
}
