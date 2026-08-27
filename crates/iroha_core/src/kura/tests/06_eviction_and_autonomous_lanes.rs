#[test]
fn cache_block_body_rejects_same_header_equal_length_wire_substitution_before_write() {
    let (_temp_dir, config) = unwrapped_kura_storage_fixture(nonzero!(1_usize));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let blocks = store_dummy_block_arcs(&kura, 4);
    let height = nonzero!(2_usize);
    let canonical = Arc::clone(&blocks[1]);
    let (_, payload_len) = advertise_required_replicas(&kura, height);
    assert!(
        kura.evict_block_bodies(payload_len)
            .expect("evict canonical block body")
            >= payload_len
    );
    let da_path = kura.block_store.lock().da_block_path(2);
    let da_bytes = fs::metadata(&da_path)
        .expect("evicted canonical DA sidecar metadata")
        .len();
    let total_with_da = kura
        .refresh_total_disk_usage_bytes()
        .expect("cache total usage with the evicted DA sidecar");
    kura.remove_evicted_block_sidecar_for_testing(height)
        .expect("make canonical body remote-only");
    assert!(!da_path.exists());
    assert!(kura.get_block(height).is_none());
    let total_without_da = kura
        .kura_total_disk_usage_bytes()
        .expect("scan total usage after remote-only test shaping");
    assert_eq!(total_without_da, total_with_da.saturating_sub(da_bytes));
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("read cached total usage after DA-sidecar removal"),
        total_without_da,
        "the test hook must publish the exact DA removal delta"
    );
    assert!(
        kura.remove_evicted_block_sidecar_for_testing(height)
            .is_err(),
        "the remote-only test hook must reject an already absent DA sidecar"
    );
    assert_eq!(
        kura.refresh_total_disk_usage_bytes()
            .expect("scan total usage after absent-sidecar rejection"),
        total_without_da
    );
    let mut substituted = canonical.as_ref().clone();
    let substitute_key =
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519).expect("substitute block key");
    let substitute_signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(substitute_key.private_key(), substituted.hash())
            .expect("sign substituted canonical header"),
    );
    substituted
        .replace_signatures([substitute_signature].into_iter().collect())
        .expect("replace substituted signature");
    assert_eq!(substituted.header(), canonical.header());
    let canonical_wire = canonical.encode_wire().expect("canonical block wire");
    let substituted_wire = substituted.encode_wire().expect("substituted block wire");
    assert_eq!(substituted_wire.len(), canonical_wire.len());
    assert_ne!(Hash::new(&substituted_wire), Hash::new(&canonical_wire));
    let total_before = kura
        .refresh_total_disk_usage_bytes()
        .expect("exact total usage before rejected cache");
    assert!(matches!(
        kura.cache_block_body(&substituted),
        Err(Error::CanonicalBlockWireMismatch { height: 2 })
    ));
    assert!(!da_path.exists(), "rejection must precede the DA write");
    assert!(kura.get_block(height).is_none());
    assert_eq!(
        kura.refresh_total_disk_usage_bytes()
            .expect("exact total usage after rejected cache"),
        total_before,
        "rejected wire substitution must not mutate accounting"
    );
    kura.cache_block_body(canonical.as_ref())
        .expect("the exact canonical body can be cached");
    assert_eq!(
        std::fs::read(&da_path).expect("read canonical DA sidecar"),
        canonical_wire
    );
    assert_eq!(kura.get_block(height).as_deref(), Some(canonical.as_ref()));
}
#[test]
fn live_evicted_read_rejects_same_header_equal_length_da_substitution() {
    let (_temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let blocks = store_dummy_block_arcs(&kura, 4);
    let height = nonzero!(2_usize);
    let canonical = Arc::clone(&blocks[1]);
    let (_, payload_len) = advertise_required_replicas(&kura, height);
    assert!(
        kura.evict_block_bodies(payload_len)
            .expect("evict finalized canonical body")
            >= payload_len
    );
    let mut substituted = canonical.as_ref().clone();
    let substitute_key =
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519).expect("substitute block key");
    substituted
        .replace_signatures(
            [BlockSignature::new(
                0,
                SignatureOf::try_from_hash(substitute_key.private_key(), substituted.hash())
                    .expect("sign the unchanged canonical header"),
            )]
            .into_iter()
            .collect(),
        )
        .expect("replace only the signed-block envelope signature");
    let canonical_wire = canonical.encode_wire().expect("canonical complete wire");
    let substituted_wire = substituted
        .encode_wire()
        .expect("substituted complete wire");
    assert_eq!(substituted.header(), canonical.header());
    assert_eq!(substituted_wire.len(), canonical_wire.len());
    assert_ne!(Hash::new(&substituted_wire), Hash::new(&canonical_wire));
    let da_path = kura.block_store.lock().da_block_path(2);
    std::fs::write(&da_path, substituted_wire).expect("substitute the live DA cache");
    kura.block_data.lock()[1].1 = None;
    assert_ne!(
        kura.block_body_status_by_hash(canonical.hash()),
        Some(BlockBodyStatus::LocalSidecar),
        "an equal-length DA substitution must not be reported as authenticated local data"
    );
    assert!(
        !kura.block_payload_available_by_hash(canonical.hash()),
        "an equal-length DA substitution must not satisfy local availability"
    );
    assert!(
        kura.get_block(height).is_none(),
        "a matching header hash cannot bypass the signed complete-wire binding"
    );
    kura.cache_block_body(canonical.as_ref())
        .expect("the exact signed wire repairs the hostile cache");
    assert_eq!(
        std::fs::read(da_path).expect("read repaired DA cache"),
        canonical_wire
    );
    assert_eq!(kura.get_block(height).as_deref(), Some(canonical.as_ref()));
}
#[test]
fn reopened_evicted_read_never_enters_memory_cache_or_survives_finality_loss() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let small_config = kura_config_for_dir(&temp_dir, nonzero!(1_usize));
    let (canonical, finality_bytes) = {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &small_config,
            &RuntimeLaneConfig::default(),
        )
        .expect("open small Kura");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let canonical = Arc::clone(&blocks[1]);
        let height = nonzero!(2_usize);
        let (_, payload_len) = advertise_required_replicas(&kura, height);
        kura.block_data.lock()[1].1 = Some(Arc::clone(&canonical));
        assert!(kura.block_data.lock()[1].1.is_some());
        assert!(
            kura.evict_block_bodies(payload_len)
                .expect("evict finalized height two")
                >= payload_len
        );
        assert!(
            kura.block_data.lock()[1].1.is_none(),
            "eviction publication must clear an in-memory copy of the evicted body"
        );
        let finality_bytes =
            fs::read(kura.v2_finality_artifact_path(2)).expect("read exact finality record");
        (canonical, finality_bytes)
    };
    let large_config = kura_config_for_dir(&temp_dir, nonzero!(16_usize));
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
        &large_config,
        &RuntimeLaneConfig::default(),
    )
    .expect("reopen large Kura");
    let height = nonzero!(2_usize);
    assert_eq!(
        kura.get_block(height).as_deref(),
        Some(canonical.as_ref()),
        "the durable finality record authenticates the local DA sidecar"
    );
    assert!(
        kura.block_data.lock()[1].1.is_none(),
        "an authenticated evicted read must remain sidecar-backed even inside the new retention window"
    );
    assert_eq!(
        kura.block_body_status_by_hash(canonical.hash()),
        Some(BlockBodyStatus::LocalSidecar)
    );
    let finality_path = kura.v2_finality_artifact_path(2);
    fs::remove_file(&finality_path).expect("delete signed finality after authenticated read");
    assert_eq!(
        kura.block_body_status_by_hash(canonical.hash()),
        Some(BlockBodyStatus::Missing)
    );
    assert!(!kura.block_payload_available_by_hash(canonical.hash()));
    assert!(kura.get_block(height).is_none());
    assert!(kura.block_data.lock()[1].1.is_none());
    fs::write(&finality_path, &finality_bytes).expect("restore exact finality record");
    assert_eq!(kura.get_block(height).as_deref(), Some(canonical.as_ref()));
    assert!(kura.block_data.lock()[1].1.is_none());
    let mut tampered_finality = finality_bytes;
    *tampered_finality
        .last_mut()
        .expect("finality record is non-empty") ^= 0x80;
    fs::write(&finality_path, tampered_finality).expect("tamper signed finality record");
    assert_eq!(
        kura.block_body_status_by_hash(canonical.hash()),
        Some(BlockBodyStatus::Missing)
    );
    assert!(!kura.block_payload_available_by_hash(canonical.hash()));
    assert!(kura.get_block(height).is_none());
    assert!(kura.block_data.lock()[1].1.is_none());
}
#[test]
fn store_existing_block_rejects_same_header_wire_substitution_without_index_effects() {
    let kura = Kura::blank_kura_for_testing();
    let canonical = store_dummy_block_arcs(&kura, 1)
        .pop()
        .expect("canonical block");
    let mut substituted = canonical.as_ref().clone();
    let substitute_key =
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519).expect("substitute block key");
    substituted
        .replace_signatures(
            [BlockSignature::new(
                0,
                SignatureOf::try_from_hash(substitute_key.private_key(), substituted.hash())
                    .expect("sign substituted canonical header"),
            )]
            .into_iter()
            .collect(),
        )
        .expect("replace substituted signature");
    assert_eq!(substituted.header(), canonical.header());
    assert_ne!(
        Kura::canonical_block_wire_hash(&substituted).expect("substituted wire hash"),
        Kura::canonical_block_wire_hash(&canonical).expect("canonical wire hash")
    );
    let index_before = format!("{:?}", *kura.transaction_entrypoint_index.lock());
    let merge_len_before = kura.merge_log.lock().total_entries;
    let total_before = kura
        .refresh_total_disk_usage_bytes()
        .expect("exact total usage before existing-body rejection");
    assert!(matches!(
        kura.store_block(Arc::new(substituted)),
        Err(Error::CanonicalBlockWireMismatch { height: 1 })
    ));
    assert_eq!(
        kura.get_block(nonzero!(1_usize)).as_deref(),
        Some(canonical.as_ref())
    );
    assert_eq!(
        format!("{:?}", *kura.transaction_entrypoint_index.lock()),
        index_before
    );
    assert_eq!(kura.merge_log.lock().total_entries, merge_len_before);
    assert_eq!(
        kura.refresh_total_disk_usage_bytes()
            .expect("exact total usage after existing-body rejection"),
        total_before
    );
}
#[cfg(unix)]
#[test]
fn evicted_existing_block_rejects_correlated_retained_wire_substitution_without_effects() {
    let (temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let blocks = store_dummy_block_arcs(&kura, 4);
    let canonical = Arc::clone(&blocks[1]);
    let height = nonzero!(2_usize);
    let (_, payload_len) = advertise_required_replicas(&kura, height);
    assert!(
        kura.evict_block_bodies(payload_len)
            .expect("evict finalized canonical body")
            >= payload_len
    );
    kura.remove_evicted_block_sidecar_for_testing(height)
        .expect("make finalized body remote-only");
    let mut substituted = canonical.as_ref().clone();
    let substitute_key =
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519).expect("substitute block key");
    substituted
        .replace_signatures(
            [BlockSignature::new(
                0,
                SignatureOf::try_from_hash(substitute_key.private_key(), substituted.hash())
                    .expect("sign unchanged canonical header"),
            )]
            .into_iter()
            .collect(),
        )
        .expect("replace only the signed-block envelope signature");
    assert_eq!(substituted.header(), canonical.header());
    let substituted_wire_hash =
        Kura::canonical_block_wire_hash(&substituted).expect("substituted wire hash");
    assert_ne!(
        substituted_wire_hash,
        Kura::canonical_block_wire_hash(canonical.as_ref()).expect("canonical wire hash")
    );
    let retained_path = kura.retained_block_record_path(2);
    let retained_bytes = fs::read(&retained_path).expect("read retained block record");
    let mut retained_input = retained_bytes.as_slice();
    let mut retained = KuraRetainedBlockRecord::decode_all(&mut retained_input)
        .expect("decode retained block record");
    retained.executed_block_wire_hash = substituted_wire_hash;
    fs::write(&retained_path, retained.encode())
        .expect("correlate unsigned retained hash with substituted wire");
    let enforced_before = kura
        .refresh_disk_usage_bytes()
        .expect("refresh enforced usage before rejection");
    let total_before = kura
        .disk_usage_bytes()
        .expect("read total usage before rejection");
    let files_before = snapshot_regular_test_tree(temp_dir.path());
    let block_data_before = kura.block_data.lock().clone();
    let block_height_index_before = kura.block_height_index.lock().clone();
    let transaction_index_before = format!("{:?}", *kura.transaction_entrypoint_index.lock());
    let merge_carrier_index_before = format!("{:?}", *kura.merge_carrier_index.lock());
    let merge_entries_before = kura.merge_log.lock().total_entries;
    assert!(matches!(
        kura.store_block(Arc::new(substituted)),
        Err(Error::V2FinalityExecutedBlockWireHashMismatch { height: 2 })
    ));
    assert_eq!(
        kura.block_data.lock().as_slice(),
        block_data_before.as_slice()
    );
    assert_eq!(&*kura.block_height_index.lock(), &block_height_index_before);
    assert_eq!(
        format!("{:?}", *kura.transaction_entrypoint_index.lock()),
        transaction_index_before
    );
    assert_eq!(
        format!("{:?}", *kura.merge_carrier_index.lock()),
        merge_carrier_index_before
    );
    assert_eq!(kura.merge_log.lock().total_entries, merge_entries_before);
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), files_before);
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("scan enforced usage after rejection"),
        enforced_before
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("scan total usage after rejection"),
        total_before
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("read cached total usage after rejection"),
        total_before
    );
}
#[cfg(unix)]
#[test]
fn evicted_existing_exact_block_requires_finality_without_effects() {
    let (temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let blocks = store_dummy_block_arcs(&kura, 4);
    let canonical = Arc::clone(&blocks[1]);
    let height = nonzero!(2_usize);
    let (_, payload_len) = advertise_required_replicas(&kura, height);
    assert!(
        kura.evict_block_bodies(payload_len)
            .expect("evict finalized canonical body")
            >= payload_len
    );
    kura.remove_evicted_block_sidecar_for_testing(height)
        .expect("make finalized body remote-only");
    fs::remove_file(kura.v2_finality_artifact_path(2))
        .expect("delete signed finality while retaining the unsigned record");
    assert!(kura.retained_block_record_path(2).is_file());
    let enforced_before = kura
        .refresh_disk_usage_bytes()
        .expect("refresh enforced usage before rejection");
    let total_before = kura
        .disk_usage_bytes()
        .expect("read total usage before rejection");
    let files_before = snapshot_regular_test_tree(temp_dir.path());
    let block_data_before = kura.block_data.lock().clone();
    let block_height_index_before = kura.block_height_index.lock().clone();
    let transaction_index_before = format!("{:?}", *kura.transaction_entrypoint_index.lock());
    let merge_carrier_index_before = format!("{:?}", *kura.merge_carrier_index.lock());
    let merge_entries_before = kura.merge_log.lock().total_entries;
    assert!(matches!(
        kura.store_block(Arc::clone(&canonical)),
        Err(Error::MissingV2FinalityArtifact { height: 2 })
    ));
    assert_eq!(
        kura.block_data.lock().as_slice(),
        block_data_before.as_slice()
    );
    assert_eq!(&*kura.block_height_index.lock(), &block_height_index_before);
    assert_eq!(
        format!("{:?}", *kura.transaction_entrypoint_index.lock()),
        transaction_index_before
    );
    assert_eq!(
        format!("{:?}", *kura.merge_carrier_index.lock()),
        merge_carrier_index_before
    );
    assert_eq!(kura.merge_log.lock().total_entries, merge_entries_before);
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), files_before);
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("scan enforced usage after rejection"),
        enforced_before
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("scan total usage after rejection"),
        total_before
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("read cached total usage after rejection"),
        total_before
    );
}
#[cfg(unix)]
#[test]
fn hash_only_existing_block_rejects_correlated_unsigned_retained_wire_without_effects() {
    let (temp_dir, _config, kura) = kura_root_fixture(nonzero!(1_usize));
    let canonical = store_dummy_block_arcs(&kura, 1)
        .pop()
        .expect("canonical block");
    let height = nonzero!(1_usize);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    kura.persist_retained_block_record(&blocks_dir, canonical.hash(), canonical.as_ref())
        .expect("persist unsigned retained record");
    kura.force_hash_only_block_for_testing(height)
        .expect("convert canonical height to authenticated hash-only shape");
    let mut substituted = canonical.as_ref().clone();
    let substitute_key =
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519).expect("substitute block key");
    substituted
        .replace_signatures(
            [BlockSignature::new(
                0,
                SignatureOf::try_from_hash(substitute_key.private_key(), substituted.hash())
                    .expect("sign unchanged canonical header"),
            )]
            .into_iter()
            .collect(),
        )
        .expect("replace only the signed-block envelope signature");
    assert_eq!(substituted.header(), canonical.header());
    let substituted_wire_hash =
        Kura::canonical_block_wire_hash(&substituted).expect("substituted wire hash");
    assert_ne!(
        substituted_wire_hash,
        Kura::canonical_block_wire_hash(canonical.as_ref()).expect("canonical wire hash")
    );
    let retained_path = kura.retained_block_record_path(1);
    let retained_bytes = fs::read(&retained_path).expect("read retained block record");
    let mut retained_input = retained_bytes.as_slice();
    let mut retained = KuraRetainedBlockRecord::decode_all(&mut retained_input)
        .expect("decode retained block record");
    retained.executed_block_wire_hash = substituted_wire_hash;
    fs::write(&retained_path, retained.encode())
        .expect("correlate unsigned retained hash with substituted wire");
    assert!(!kura.v2_finality_artifact_path(1).exists());
    let enforced_before = kura
        .refresh_disk_usage_bytes()
        .expect("refresh enforced usage before rejection");
    let total_before = kura
        .disk_usage_bytes()
        .expect("read total usage before rejection");
    let files_before = snapshot_regular_test_tree(temp_dir.path());
    let block_data_before = kura.block_data.lock().clone();
    let block_height_index_before = kura.block_height_index.lock().clone();
    let transaction_index_before = format!("{:?}", *kura.transaction_entrypoint_index.lock());
    let merge_carrier_index_before = format!("{:?}", *kura.merge_carrier_index.lock());
    let merge_entries_before = kura.merge_log.lock().total_entries;
    let hash_only_prefix_before = kura.hard_fork_hash_only_block_count.load(Ordering::Acquire);
    assert!(matches!(
        kura.store_block(Arc::new(substituted)),
        Err(Error::MissingV2FinalityArtifact { height: 1 })
    ));
    assert_eq!(
        kura.block_data.lock().as_slice(),
        block_data_before.as_slice()
    );
    assert_eq!(&*kura.block_height_index.lock(), &block_height_index_before);
    assert_eq!(
        format!("{:?}", *kura.transaction_entrypoint_index.lock()),
        transaction_index_before
    );
    assert_eq!(
        format!("{:?}", *kura.merge_carrier_index.lock()),
        merge_carrier_index_before
    );
    assert_eq!(kura.merge_log.lock().total_entries, merge_entries_before);
    assert_eq!(
        kura.hard_fork_hash_only_block_count.load(Ordering::Acquire),
        hash_only_prefix_before
    );
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), files_before);
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("scan enforced usage after rejection"),
        enforced_before
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("scan total usage after rejection"),
        total_before
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("read cached total usage after rejection"),
        total_before
    );
}
#[test]
fn cache_block_body_rejects_wrong_rehydrated_hash() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    let block_hash = kura
        .get_block_hash(height)
        .expect("canonical block hash before eviction");
    advertise_required_replicas(&kura, height);
    let evict_len = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index").length
    };
    kura.evict_block_bodies(evict_len)
        .expect("evict block bodies");
    let conflicting: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(2_u64));
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    let conflicting_hash = conflicting.hash();
    assert_ne!(block_hash, conflicting_hash);
    let err = kura
        .cache_block_body(&conflicting)
        .expect_err("wrong rehydrated body must be rejected");
    assert!(matches!(
        err,
        Error::BlockHeightConflict {
            height: 2,
            expected,
            actual,
        } if expected == block_hash && actual == conflicting_hash
    ));
    let da_path = {
        let store = kura.block_store.lock();
        store.da_block_path(2)
    };
    assert!(
        da_path.exists(),
        "rejecting a conflicting body must not remove the existing DA sidecar"
    );
    assert!(
        kura.get_block(height).is_some(),
        "the canonical sidecar should remain readable after rejecting the wrong body"
    );
}
#[test]
fn cache_block_body_rejects_height_gap_before_sidecar_write() {
    let kura = Kura::blank_kura_for_testing();
    let block: SignedBlock = ValidBlock::new_dummy(checked_keypair().private_key()).into();
    let height = block.header().height().get();
    let da_path = {
        let store = kura.block_store.lock();
        store.da_block_path(height)
    };
    let err = kura
        .cache_block_body(&block)
        .expect_err("non-durable block body must not be cached");
    assert!(matches!(
        err,
        Error::BlockHeightGap {
            expected_next_height: 1,
            actual_height,
        } if actual_height == height
    ));
    assert!(
        !da_path.exists(),
        "height-gap body must not be written into the sidecar cache"
    );
    assert_eq!(kura.blocks_count(), 0);
}
#[test]
fn cache_block_body_rejects_length_mismatch() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    let block = kura
        .get_block(height)
        .expect("inline block before eviction");
    advertise_required_replicas(&kura, height);
    let evict_len = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index").length
    };
    kura.evict_block_bodies(evict_len)
        .expect("evict block bodies");
    {
        let mut store = kura.block_store.lock();
        let tampered_len = evict_len.saturating_add(1);
        store
            .write_block_index(1, EVICTED_BLOCK_START, tampered_len)
            .expect("tamper evicted block length");
    }
    let err = kura
        .cache_block_body(block.as_ref())
        .expect_err("length mismatch must be rejected");
    assert!(matches!(
        err,
        Error::CorruptedBlockRange {
            start: EVICTED_BLOCK_START,
            length,
            data_len,
        } if length == evict_len && data_len == evict_len.saturating_add(1)
    ));
    let da_path = {
        let store = kura.block_store.lock();
        store.da_block_path(2)
    };
    assert!(
        da_path.exists(),
        "length-mismatched recache must not remove the existing DA sidecar"
    );
}
#[test]
fn evicted_remote_body_rehydrates_after_restart_and_new_adverts() {
    let (_temp_dir, config) =
        unwrapped_kura_storage_fixture(NonZeroUsize::new(1).expect("non-zero"));
    let height = nonzero!(2_usize);
    let (block, block_hash) = {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let block = Arc::clone(&blocks[1]);
        let block_hash = block.hash();
        advertise_required_replicas(&kura, height);
        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        kura.evict_block_bodies(evict_len)
            .expect("evict block bodies");
        {
            let store = kura.block_store.lock();
            store
                .remove_da_block_file(height.get() as u64)
                .expect("remove sidecar to exercise remote-only restart");
        }
        assert!(
            kura.get_block(height).is_none(),
            "evicted remote body should not be readable before rehydrate"
        );
        (block, block_hash)
    };
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura reopen");
    assert_eq!(
        kura.get_durable_block_hash(height),
        Some(block_hash),
        "durable hash metadata should survive restart"
    );
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Missing),
        "remote-only body needs fresh replica evidence after restart"
    );
    advertise_required_replicas(&kura, height);
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::RemoteOnly {
            replicas: EVICTION_REQUIRED_REPLICAS.get()
        })
    );
    kura.cache_block_body(block.as_ref())
        .expect("cache block after fresh adverts");
    let rehydrated = kura.get_block(height).expect("rehydrated block");
    assert_eq!(rehydrated.hash(), block_hash);
}
#[test]
fn fast_init_preserves_remote_only_hash_without_rebuilding_status_index() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let lane_config = RuntimeLaneConfig::default();
    let height = nonzero!(2_usize);
    let block_hash = {
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        let blocks = store_dummy_block_arcs(&kura, 4);
        let block_hash = blocks[1].hash();
        advertise_required_replicas(&kura, height);
        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        kura.evict_block_bodies(evict_len)
            .expect("evict block bodies");
        {
            let store = kura.block_store.lock();
            store
                .remove_da_block_file(height.get() as u64)
                .expect("remove sidecar to preserve remote-only fixture");
        }
        block_hash
    };
    config.init_mode = InitMode::Fast;
    let (kura, block_count) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("fast reopen");
    assert_eq!(block_count.0, 4);
    assert_eq!(
        kura.get_durable_block_hash(height),
        Some(block_hash),
        "fast init should preserve hash metadata for remote-only blocks"
    );
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        None,
        "Fast must not rebuild the historical hash-to-height status index"
    );
}
#[test]
fn strict_init_prunes_remote_only_tail_without_hash_metadata() {
    let (temp_dir, config) =
        unwrapped_kura_storage_fixture(NonZeroUsize::new(1).expect("non-zero"));
    let height = nonzero!(2_usize);
    let block_hash = {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let block_hash = blocks[1].hash();
        advertise_required_replicas(&kura, height);
        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        kura.evict_block_bodies(evict_len)
            .expect("evict block bodies");
        {
            let store = kura.block_store.lock();
            store
                .remove_da_block_file(height.get() as u64)
                .expect("remove sidecar to preserve remote-only fixture");
        }
        assert_eq!(kura.get_durable_block_hash(height), Some(block_hash));
        block_hash
    };
    let hashes_path = primary_blocks_dir(&temp_dir).join(HASHES_FILE_NAME);
    let hashes_file = std::fs::OpenOptions::new()
        .write(true)
        .open(&hashes_path)
        .expect("open hashes file");
    hashes_file
        .set_len(SIZE_OF_BLOCK_HASH)
        .expect("truncate hashes below index height");
    assert!(matches!(
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default()),
        Err(Error::FinalizedV2BlockMutation {
            rewrite_from_height: 2,
            finalized_height: 2,
        })
    ));
    let mut store = BlockStore::new(&primary_blocks_dir(&temp_dir));
    assert_eq!(
        store
            .read_commit_marker()
            .expect("read unchanged durable marker")
            .map(|marker| marker.count),
        Some(4),
        "strict startup must not truncate a suffix covered by durable finality"
    );
    assert_eq!(
        store.read_hashes_count().expect("read truncated fixture"),
        1
    );
    assert!(
        Kura::v2_finality_artifact_path_for(&primary_blocks_dir(&temp_dir), 2).is_file(),
        "failed startup must retain the finality evidence that blocked truncation"
    );
    let _ = block_hash;
}
#[test]
fn malformed_sidecar_status_is_missing_without_fresh_adverts() {
    let (_temp_dir, config) =
        unwrapped_kura_storage_fixture(NonZeroUsize::new(1).expect("non-zero"));
    let height = nonzero!(2_usize);
    let (block_hash, da_path) = {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let block = Arc::clone(&blocks[1]);
        let block_hash = block.hash();
        advertise_required_replicas(&kura, height);
        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        kura.evict_block_bodies(evict_len)
            .expect("evict block bodies");
        kura.cache_block_body(block.as_ref())
            .expect("cache rehydrated block");
        let da_path = {
            let store = kura.block_store.lock();
            store.da_block_path(2)
        };
        (block_hash, da_path)
    };
    std::fs::write(&da_path, b"short").expect("corrupt local sidecar length");
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura reopen");
    assert_eq!(
        kura.get_durable_block_hash(height),
        Some(block_hash),
        "corrupted sidecar should not erase durable hash metadata"
    );
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Missing),
        "bad local sidecar without fresh adverts should be treated as unavailable"
    );
    assert!(
        !kura.block_payload_available_by_hash(block_hash),
        "malformed sidecar must not count as local payload availability"
    );
}
#[test]
fn strict_init_removes_malformed_sidecar_with_matching_length() {
    let (_temp_dir, config) =
        unwrapped_kura_storage_fixture(NonZeroUsize::new(1).expect("non-zero"));
    let height = nonzero!(2_usize);
    let (block_hash, da_path) = {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let block = Arc::clone(&blocks[1]);
        let block_hash = block.hash();
        advertise_required_replicas(&kura, height);
        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        kura.evict_block_bodies(evict_len)
            .expect("evict block bodies");
        kura.cache_block_body(block.as_ref())
            .expect("cache rehydrated block");
        let da_path = {
            let store = kura.block_store.lock();
            store.da_block_path(2)
        };
        let mut payload = std::fs::read(&da_path).expect("read local sidecar");
        assert!(
            payload.len() > 1,
            "stored block frame should include a header"
        );
        let original_len = payload.len();
        payload[1] = payload[1].wrapping_add(1);
        std::fs::write(&da_path, &payload).expect("corrupt local sidecar");
        assert_eq!(
            std::fs::metadata(&da_path).expect("sidecar metadata").len(),
            original_len as u64,
            "test corruption must preserve payload length"
        );
        (block_hash, da_path)
    };
    let (kura, block_count) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("strict reopen");
    assert_eq!(
        block_count.0, 4,
        "malformed sidecar cache must not truncate canonical hash metadata"
    );
    assert_eq!(kura.get_durable_block_hash(height), Some(block_hash));
    assert!(
        !da_path.exists(),
        "strict init should remove malformed local sidecar cache files"
    );
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Missing)
    );
    assert!(!kura.block_payload_available_by_hash(block_hash));
    assert!(
        kura.get_block(height).is_none(),
        "removed malformed sidecar should not be readable until rehydrated"
    );
}
#[test]
fn strict_init_rejects_conflicting_sidecar_hash_without_rewriting_chain() {
    let (_temp_dir, config) =
        unwrapped_kura_storage_fixture(NonZeroUsize::new(1).expect("non-zero"));
    let height = nonzero!(2_usize);
    let (canonical_hash, conflicting_hash, da_path) = {
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let blocks = store_dummy_block_arcs(&kura, 4);
        let genesis_hash = blocks[0].hash();
        let canonical_hash = blocks[1].hash();
        advertise_required_replicas(&kura, height);
        let evict_len = {
            let mut store = kura.block_store.lock();
            store.read_block_index(1).expect("block index").length
        };
        kura.evict_block_bodies(evict_len)
            .expect("evict block bodies");
        let conflicting: SignedBlock =
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(2_u64));
                header.set_prev_block_hash(Some(genesis_hash));
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into();
        let conflicting_hash = conflicting.hash();
        assert_ne!(canonical_hash, conflicting_hash);
        let (frame, _versioned) = conflicting
            .canonical_wire()
            .expect("encode conflicting sidecar")
            .into_parts();
        let da_path = {
            let store = kura.block_store.lock();
            store
                .write_da_block_bytes(2, &frame)
                .expect("write conflicting DA sidecar");
            store.da_block_path(2)
        };
        assert!(da_path.exists());
        (canonical_hash, conflicting_hash, da_path)
    };
    let (kura, block_count) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("strict reopen");
    assert_eq!(
        block_count.0, 4,
        "conflicting sidecar cache must not truncate the canonical chain"
    );
    assert_eq!(kura.get_durable_block_hash(height), Some(canonical_hash));
    assert_ne!(kura.get_durable_block_hash(height), Some(conflicting_hash));
    assert!(
        !da_path.exists(),
        "strict init should remove local sidecars whose hash conflicts with Kura metadata"
    );
    assert_eq!(
        kura.block_body_status_by_hash(canonical_hash),
        Some(BlockBodyStatus::Missing)
    );
}
#[test]
fn block_payload_available_by_hash_requires_local_body_after_eviction() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    let block = kura
        .get_block(height)
        .expect("inline block before eviction");
    let (block_hash, _) = advertise_required_replicas(&kura, height);
    let evict_len = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index").length
    };
    kura.evict_block_bodies(evict_len)
        .expect("evict block bodies");
    let da_path = {
        let store = kura.block_store.lock();
        store.da_block_path(2)
    };
    assert!(da_path.exists(), "eviction should create a DA sidecar");
    assert!(
        kura.block_payload_available_by_hash(block_hash),
        "DA-sidecar-backed payloads are locally available"
    );
    kura.cache_block_body(block.as_ref())
        .expect("cache rehydrated block");
    assert!(
        kura.block_payload_available_by_hash(block_hash),
        "payload should be available when local sidecar cache exists"
    );
    std::fs::remove_file(&da_path).expect("remove DA payload");
    assert!(
        !kura.block_payload_available_by_hash(block_hash),
        "payload should be unavailable after DA payload removal"
    );
}
#[test]
fn evicted_blocks_survive_restart() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = NonZeroUsize::new(2).expect("non-zero");
    let block = kura
        .get_block(height)
        .expect("inline block before eviction");
    advertise_required_replicas(&kura, height);
    let evict_len = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index").length
    };
    kura.evict_block_bodies(evict_len)
        .expect("evict block bodies");
    kura.cache_block_body(block.as_ref())
        .expect("cache rehydrated block");
    drop(kura);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura reopen");
    let expected_hash = kura.get_block_hash(height).expect("hash available");
    let block = kura
        .get_block(height)
        .expect("rehydrated block after restart");
    assert_eq!(block.hash(), expected_hash);
}
#[test]
fn deep_history_get_block_uses_cached_bytes() {
    const BLOCK_COUNT: usize = 192;
    let temp_dir = TempDir::new().unwrap();
    let mut store = new_block_store(&temp_dir);
    store.create_files_if_they_do_not_exist().unwrap();
    let mut blocks = DummyBlocks::new();
    let mut expected_hashes = Vec::with_capacity(BLOCK_COUNT);
    for _ in 0..BLOCK_COUNT {
        let block = blocks.next();
        expected_hashes.push(block.hash());
        store.append_block_to_chain(block.as_ref()).unwrap();
    }
    drop(store);
    let config = kura_config_for_dir(&temp_dir, nonzero!(16_usize));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let heights: Vec<_> = (1..=BLOCK_COUNT)
        .map(|height| NonZeroUsize::new(height).expect("nonzero height"))
        .collect();
    for _ in 0..3 {
        for (idx, height) in heights.iter().enumerate() {
            let block = kura
                .get_block(*height)
                .unwrap_or_else(|| panic!("block missing at height {height}"));
            assert_eq!(block.hash(), expected_hashes[idx]);
        }
    }
    let store_guard = kura.block_store.lock();
    let mirror = store_guard
        .data_mmap
        .as_ref()
        .expect("expected data mirror to be primed");
    assert_eq!(
        mirror.kind(),
        MemoryMirrorKind::MemoryMapped,
        "expected data mirror to use a memory-mapped backend"
    );
    let mapped_len = mirror.len();
    assert_eq!(
        u64::try_from(mapped_len).expect("mirror length fits in u64"),
        store_guard.data_mmap_len,
        "data mirror length should match recorded length"
    );
}
#[test]
fn debug_output_new_blocks_writes_jsonl() {
    let temp_dir = TempDir::new().expect("temp dir");
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.debug_output_new_blocks = true;
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    kura.bind_local_peer_id(checked_peer_id())
        .expect("bind local peer before Kura start");
    let _handle = {
        let _rt_guard = rt.enter();
        Kura::start(kura.clone(), ShutdownSignal::new())
    };
    let block = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&block)).expect("store block");
    wait_for_block_hash(&kura, 1, block.hash());
    let blocks_dir = RuntimeLaneConfig::default()
        .primary()
        .blocks_dir(temp_dir.path());
    let dump_path = blocks_dir.join("blocks.jsonl");
    let contents = fs::read_to_string(&dump_path).expect("read debug block dump");
    let mut lines = contents.lines();
    let first = lines.next().expect("first JSON line");
    assert!(lines.next().is_none(), "expected one JSON line");
    let _: norito::json::Value =
        norito::json::from_slice(first.as_bytes()).expect("valid JSON line");
}
#[allow(clippy::too_many_lines)]
fn create_blocks(rt: &tokio::runtime::Runtime, temp_dir: &TempDir) -> Vec<CommittedBlock> {
    let mut blocks = Vec::new();
    let (leader_public_key, leader_private_key) =
        checked_keypair_with_algorithm(Algorithm::BlsNormal).into_parts();
    let peer_id = PeerId::new(leader_public_key.clone());
    let topology = Topology::new(vec![peer_id]);
    let topology_entries = vec![GenesisTopologyEntry::new(
        PeerId::new(leader_public_key.clone()),
        bls_normal_pop_prove(&leader_private_key).expect("generate BLS PoP"),
    )];
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let (genesis_id, genesis_key_pair) = gen_account_in("genesis");
    let genesis_domain_id = DomainId::try_new("genesis", "universal").expect("Valid");
    let genesis_domain = Domain::new(genesis_domain_id.clone()).build(&genesis_id);
    let genesis_account = Account::new(genesis_id.clone()).build(&genesis_id);
    let (account_id, account_keypair) = gen_account_in("wonderland");
    let domain_id = DomainId::try_new("wonderland", "universal").expect("Valid");
    let domain = Domain::new(domain_id.clone()).build(&genesis_id);
    let account = Account::new(account_id.clone()).build(&genesis_id);
    let live_query_store = {
        let _rt_guard = rt.enter();
        LiveQueryStore::start_test()
    };
    let config = kura_config_for_dir(temp_dir, BLOCKS_IN_MEMORY);
    let (kura, block_count) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    assert_eq!(block_count.0, 0);
    let state = State::new(
        World::with([domain, genesis_domain], [account, genesis_account], []),
        Arc::clone(&kura),
        live_query_store,
    );
    let genesis_validator = ManifestValidatorBinding {
        validator: genesis_id.clone(),
        peer_id: PeerId::new(genesis_key_pair.public_key().clone()),
        torii_url: None,
    };
    let lane_manifest = LaneManifestStatus {
        lane: LaneId::SINGLE,
        alias: "default".to_owned(),
        dataspace: DataSpaceId::UNIVERSAL,
        visibility: LaneVisibility::Public,
        storage: LaneStorageProfile::FullReplica,
        governance: Some("genesis".to_owned()),
        manifest_path: Some(temp_dir.path().join("lane-0-manifest.json")),
        governance_rules: Some(GovernanceRules {
            validators: vec![genesis_id.clone()],
            validator_bindings: vec![genesis_validator],
            ..GovernanceRules::default()
        }),
        privacy_commitments: Vec::new(),
    };
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(
        BTreeMap::from([(LaneId::SINGLE, lane_manifest)]),
    )));
    let genesis = GenesisBuilder::new_without_executor(chain_id.clone(), "ivm/libs/not/installed")
        .set_topology(topology_entries)
        .build_and_sign(&genesis_key_pair)
        .expect("genesis block should be built");
    {
        let time_source = TimeSource::new_system();
        let mut voting_block = None;
        let (valid_genesis, mut state_block) =
            ValidBlock::validate_signed_genesis_keep_voting_block(
                genesis.0.clone(),
                &topology,
                &genesis_id,
                &time_source,
                &state,
                &mut voting_block,
                iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            )
            .unpack(|_| {})
            .unwrap();
        let block_genesis = valid_genesis.commit_unchecked().unpack(|_| {});
        let _events =
            state_block.apply_without_execution(&block_genesis, topology.as_ref().to_owned());
        state_block.commit().unwrap();
        blocks.push(block_genesis.clone());
        kura.store_block(block_genesis.clone())
            .expect("store genesis block");
        wait_for_block_hash(&kura, 1, block_genesis.as_ref().hash());
    }
    let (max_clock_drift, tx_limits) = {
        let view = state.view();
        let params = view.world.parameters.get();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let tx1 = TransactionBuilder::new(
        *state.network_id_ref(),
        account_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "msg1".to_string())])
    .sign(account_keypair.private_key());
    let tx2 = TransactionBuilder::new(
        *state.network_id_ref(),
        account_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "msg2".to_string())])
    .sign(account_keypair.private_key());
    let crypto_cfg = state.crypto();
    let tx1 = AcceptedTransaction::accept(
        tx1,
        state.network_id_ref(),
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .unwrap();
    let tx2 = AcceptedTransaction::accept(
        tx2,
        state.network_id_ref(),
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .unwrap();
    {
        let unverified_block = BlockBuilder::new(vec![tx1.clone()])
            .chain(0, state.view().latest_block().as_deref())
            .sign(&leader_private_key)
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header());
        let block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit_unchecked()
            .unpack(|_| {});
        let _events = state_block.apply_without_execution(&block, topology.as_ref().to_owned());
        state_block.commit().unwrap();
        let block_hash = block.as_ref().hash();
        blocks.push(block.clone());
        kura.store_block(block).expect("store block");
        wait_for_block_hash(&kura, 2, block_hash);
    }
    {
        let unverified_block_soft_fork = BlockBuilder::new(vec![tx1])
            .chain(1, Some(&genesis.0))
            .sign(&leader_private_key)
            .unpack(|_| {});
        let mut state_block = state.block_and_revert(unverified_block_soft_fork.header());
        let block_soft_fork = unverified_block_soft_fork
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit_unchecked()
            .unpack(|_| {});
        let _events =
            state_block.apply_without_execution(&block_soft_fork, topology.as_ref().to_owned());
        state_block.commit().unwrap();
        let soft_fork_hash = block_soft_fork.as_ref().hash();
        blocks.push(block_soft_fork.clone());
        kura.replace_top_block(block_soft_fork)
            .expect("replace top block");
        wait_for_block_hash(&kura, 2, soft_fork_hash);
    }
    {
        let unverified_block_next = BlockBuilder::new(vec![tx2])
            .chain(0, state.view().latest_block().as_deref())
            .sign(&leader_private_key)
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block_next.header());
        let block_next = unverified_block_next
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit_unchecked()
            .unpack(|_| {});
        let _events =
            state_block.apply_without_execution(&block_next, topology.as_ref().to_owned());
        state_block.commit().unwrap();
        let next_hash = block_next.as_ref().hash();
        blocks.push(block_next.clone());
        kura.store_block(block_next).expect("store block");
        wait_for_block_hash(&kura, 3, next_hash);
    }
    {
        let expected_count = kura.blocks_count() as u64;
        let mut store = kura.block_store.lock();
        store
            .flush_pending_fsync(true)
            .expect("flush pending block data for strict reload");
        let durable = store
            .read_durable_index_count()
            .expect("read durable block count");
        assert_eq!(
            durable, expected_count,
            "durable block count should match in-memory block count before reload"
        );
    }
    blocks
}
struct DummyBlocks {
    blocks: Vec<Arc<SignedBlock>>,
}
impl DummyBlocks {
    fn new() -> Self {
        Self {
            blocks: <_>::default(),
        }
    }
    fn next(&mut self) -> Arc<SignedBlock> {
        let tx = {
            let builder = TransactionBuilder::new(
                test_network_id(b"test"),
                SAMPLE_GENESIS_ACCOUNT_ID.to_owned(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            );
            let tx = if self.blocks.is_empty() {
                builder.with_instructions([Upgrade::new(Executor::new(
                    IvmBytecode::from_compiled(vec![]),
                ))])
            } else {
                builder.with_instructions([Log::new(Level::INFO, "test".to_owned())])
            }
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
            AcceptedTransaction::new_unchecked(Cow::Owned(tx))
        };
        let prev = self.blocks.last().cloned();
        let mut block: SignedBlock = BlockBuilder::new(vec![tx])
            .chain(0, prev.as_ref().map(AsRef::as_ref))
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into();
        // `DummyBlocks` is the generic canonical-storage fixture.  Retain
        // the fixed-width routing context used by wire-shape tests, but do
        // not reuse the block builder's synthetic lane-height-one evidence
        // across a multi-block chain: the second block would claim a
        // conflicting durable lane artifact.  Tests exercising lane
        // evidence attach their own exact ownership via
        // `block_with_lane_payload_ownership_for_kura`.
        if let Some(external) = block
            .execution_context()
            .map(|context| context.external.clone())
        {
            block.set_execution_context(Some(BlockExecutionContextBundle::new(external)));
        }
        let block = Arc::new(block);
        self.blocks.push(block.clone());
        block
    }
    fn get(&self, i: usize) -> Option<Arc<SignedBlock>> {
        self.blocks.get(i).cloned()
    }
}
fn store_dummy_blocks(kura: &Arc<Kura>, count: usize) -> Vec<HashOf<BlockHeader>> {
    let mut blocks = DummyBlocks::new();
    let mut hashes = Vec::with_capacity(count);
    for _ in 0..count {
        let block = blocks.next();
        let hash = block.hash();
        kura.store_block(block).expect("store block");
        hashes.push(hash);
    }
    hashes
}
fn read_block(store: &mut BlockStore, index: usize) -> eyre::Result<SignedBlock> {
    let BlockIndex { start, length } = store.read_block_index(index as u64)?;
    let len: usize = length.try_into().unwrap();
    let mut buff = vec![0_u8; len];
    store.read_block_data(start, &mut buff)?;
    let block = decode_versioned_signed_block(&buff).map_err(eyre::Report::new)?;
    Ok(block)
}
fn sample_lane_payload_ownership_for_kura(
    block: &SignedBlock,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
) -> SumeragiLanePayloadOwnership {
    let proposal_height = block.header().height().get();
    let proposal_view = block.header().view_change_index();
    let lane_block_view = proposal_view;
    let accepted_candidate_indices = vec![0_u64];
    let accepted_transaction_hash = Kura::block_entrypoint_hash_at(block, 0)
        .expect("dummy block has a first external entrypoint");
    let mut validator_set = vec![checked_peer_id()];
    validator_set.sort();
    let validator_count = u32::try_from(validator_set.len()).expect("validator count fits u32");
    let min_quorum = validator_count;
    let mut ownership = SumeragiLanePayloadOwnership {
        proposal_height,
        proposal_view,
        lane_id,
        dataspace_id,
        lane_incarnation: Hash::new(
            format!(
                "kura-lane-incarnation:{}:{}",
                lane_id.as_u32(),
                dataspace_id.as_u64()
            )
            .as_bytes(),
        ),
        lane_block_height,
        lane_block_view,
        subject_hash: Hash::new(b"kura-lane-subject-placeholder"),
        qc_mode_tag: "kura-lane-artifact-test".to_string(),
        accepted_candidate_indices,
        accepted_transaction_hashes: vec![accepted_transaction_hash],
        previous_lane_block_height: lane_block_height.saturating_sub(1),
        previous_lane_block_descriptor_hash: lane_block_height
            .checked_sub(1)
            .filter(|height| *height > 0)
            .map(|previous| {
                Hash::new(
                    format!(
                        "kura-lane-previous-descriptor:{}:{}:{}",
                        lane_id.as_u32(),
                        dataspace_id.as_u64(),
                        previous
                    )
                    .into_bytes(),
                )
            }),
        lane_block_descriptor_hash: Some(Hash::new(b"kura-lane-descriptor-placeholder")),
        lane_block_descriptor_validator_set: validator_set,
        lane_block_descriptor_validator_count: validator_count,
        lane_block_descriptor_min_quorum: min_quorum,
        payload_ownership_hash: Hash::new(b"kura-lane-payload-placeholder"),
        rbc_instance_hash: Hash::new(b"kura-lane-rbc-placeholder"),
    };
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("kura lane artifact replay hashes compute");
    ownership.subject_hash = replay_hashes.subject_hash;
    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
    ownership
}
fn signed_lane_block_vote_for_kura(
    proposal: &LaneBlockProposalV1,
    phase: CertPhase,
    keypair: &KeyPair,
) -> crate::lane_consensus::LaneBlockVoteV1 {
    let body = proposal.vote_body(phase);
    let signature = Signature::try_new(keypair.private_key(), &body.signature_preimage())
        .expect("kura lane block vote signature");
    crate::lane_consensus::LaneBlockVoteV1 {
        body,
        signer: PeerId::new(keypair.public_key().clone()),
        bls_signature: signature.payload().to_vec(),
        payload_availability_vote: None,
    }
}
fn lane_block_proposal_from_ownership(
    ownership: &SumeragiLanePayloadOwnership,
) -> LaneBlockProposalV1 {
    let validator_set = ownership.lane_block_descriptor_validator_set.clone();
    let descriptor = LaneBlockDescriptorV1 {
        lane_id: ownership.lane_id,
        dataspace_id: ownership.dataspace_id,
        lane_incarnation: ownership.lane_incarnation,
        proposal_height: ownership.proposal_height,
        previous_lane_block_height: ownership.previous_lane_block_height,
        previous_lane_block_descriptor_hash: ownership.previous_lane_block_descriptor_hash,
        lane_block_height: ownership.lane_block_height,
        lane_block_view: ownership.lane_block_view,
        subject_hash: ownership.subject_hash,
        payload_ownership_hash: ownership.payload_ownership_hash,
        rbc_instance_hash: ownership.rbc_instance_hash,
        accepted_candidate_indices: ownership.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        validator_count: ownership.lane_block_descriptor_validator_count,
        min_quorum: ownership.lane_block_descriptor_min_quorum,
        qc_mode_tag: ownership.qc_mode_tag.clone(),
        descriptor_hash: ownership
            .lane_block_descriptor_hash
            .expect("ownership has descriptor hash"),
    };
    assert_eq!(
        descriptor.descriptor_hash,
        descriptor.computed_descriptor_hash(),
        "fixture ownership descriptor hash must be canonical"
    );
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    proposal
}
fn rebind_lane_payload_ownership_for_kura(
    ownership: &mut SumeragiLanePayloadOwnership,
    lane_incarnation: Hash,
    lane_block_height: u64,
) {
    ownership.lane_incarnation = lane_incarnation;
    ownership.lane_block_height = lane_block_height;
    ownership.previous_lane_block_height = lane_block_height.saturating_sub(1);
    ownership.previous_lane_block_descriptor_hash = lane_block_height
        .checked_sub(1)
        .filter(|height| *height > 0)
        .map(|height| Hash::new(format!("rebound-lane-predecessor:{height}").as_bytes()));
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("rebound lane ownership replay hashes compute");
    ownership.subject_hash = replay_hashes.subject_hash;
    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
}
fn sample_committed_lane_block_session_for_kura(
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
) -> (
    crate::lane_consensus::CommittedLaneBlockSession,
    BTreeMap<PublicKey, Vec<u8>>,
) {
    sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        dataspace_id,
        lane_block_height,
        lane_block_height,
    )
}
fn sample_committed_lane_block_session_at_proposal_height_for_kura(
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
    proposal_height: u64,
) -> (
    crate::lane_consensus::CommittedLaneBlockSession,
    BTreeMap<PublicKey, Vec<u8>>,
) {
    let keypair = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let signer_pop =
        bls_normal_pop_prove(keypair.private_key()).expect("kura certified lane signer PoP");
    let peer_id = PeerId::new(keypair.public_key().clone());
    let validator_set = vec![peer_id];
    let mut signer_pops = BTreeMap::new();
    signer_pops.insert(keypair.public_key().clone(), signer_pop);
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id,
        dataspace_id,
        lane_incarnation: Hash::new(
            format!(
                "kura-lane-incarnation:{}:{}",
                lane_id.as_u32(),
                dataspace_id.as_u64()
            )
            .as_bytes(),
        ),
        proposal_height,
        previous_lane_block_height: lane_block_height.saturating_sub(1),
        previous_lane_block_descriptor_hash: lane_block_height
            .checked_sub(1)
            .filter(|height| *height > 0)
            .map(|previous| {
                Hash::new(
                    format!(
                        "kura-certified-previous:{}:{}:{}",
                        lane_id.as_u32(),
                        dataspace_id.as_u64(),
                        previous
                    )
                    .into_bytes(),
                )
            }),
        lane_block_height,
        lane_block_view: 2,
        subject_hash: Hash::new(
            format!(
                "kura-certified-subject:{}:{}:{}",
                lane_id.as_u32(),
                dataspace_id.as_u64(),
                lane_block_height
            )
            .into_bytes(),
        ),
        payload_ownership_hash: Hash::new(
            format!(
                "kura-certified-ownership:{}:{}:{}",
                lane_id.as_u32(),
                dataspace_id.as_u64(),
                lane_block_height
            )
            .into_bytes(),
        ),
        rbc_instance_hash: Hash::new(
            format!(
                "kura-certified-rbc:{}:{}:{}",
                lane_id.as_u32(),
                dataspace_id.as_u64(),
                lane_block_height
            )
            .into_bytes(),
        ),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::new(
            format!(
                "kura-certified-tx:{}:{}:{}",
                lane_id.as_u32(),
                dataspace_id.as_u64(),
                lane_block_height
            )
            .into_bytes(),
        )],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        validator_count: 1,
        min_quorum: 1,
        qc_mode_tag: "permissioned:kura-certified-lane-block".to_string(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let prepare_vote = signed_lane_block_vote_for_kura(&proposal, CertPhase::Prepare, &keypair);
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        prepare_vote.body.clone(),
        validator_set.clone(),
        std::slice::from_ref(&prepare_vote),
    )
    .expect("kura certified lane prepare QC");
    let commit_vote = signed_lane_block_vote_for_kura(&proposal, CertPhase::Commit, &keypair);
    let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        commit_vote.body.clone(),
        validator_set,
        std::slice::from_ref(&commit_vote),
    )
    .expect("kura certified lane commit QC");
    (
        crate::lane_consensus::CommittedLaneBlockSession {
            proposal,
            prepare_qc,
            commit_qc,
        },
        signer_pops,
    )
}
fn committed_lane_block_session_for_kura_proposal(
    proposal: &LaneBlockProposalV1,
    signer: &KeyPair,
) -> (
    crate::lane_consensus::CommittedLaneBlockSession,
    BTreeMap<PublicKey, Vec<u8>>,
) {
    let prepare_vote = signed_lane_block_vote_for_kura(proposal, CertPhase::Prepare, signer);
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        prepare_vote.body.clone(),
        proposal.descriptor.validator_set.clone(),
        std::slice::from_ref(&prepare_vote),
    )
    .expect("proposal prepare QC");
    let commit_vote = signed_lane_block_vote_for_kura(proposal, CertPhase::Commit, signer);
    let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        commit_vote.body.clone(),
        proposal.descriptor.validator_set.clone(),
        std::slice::from_ref(&commit_vote),
    )
    .expect("proposal commit QC");
    let signer_pops = BTreeMap::from([(
        signer.public_key().clone(),
        bls_normal_pop_prove(signer.private_key()).expect("proposal signer PoP"),
    )]);
    (
        crate::lane_consensus::CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc,
            commit_qc,
        },
        signer_pops,
    )
}
fn dummy_block_with_lane_payload_ownership(
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
) -> Arc<SignedBlock> {
    let mut generator = DummyBlocks::new();
    dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        dataspace_id,
        lane_block_height,
    )
}
fn dummy_block_with_lane_payload_ownership_from_generator(
    generator: &mut DummyBlocks,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
) -> Arc<SignedBlock> {
    block_with_lane_payload_ownership_for_kura(
        generator.next().as_ref().clone(),
        lane_id,
        dataspace_id,
        lane_block_height,
    )
}
fn block_with_lane_payload_ownership_for_kura(
    mut block: SignedBlock,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
) -> Arc<SignedBlock> {
    let entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"kura-lane-artifact-entrypoint",
    ));
    block.set_execution_context(Some(BlockExecutionContextBundle::new(vec![
        ExternalExecutionContext::new(entrypoint_hash, lane_id, dataspace_id),
    ])));
    let ownership =
        sample_lane_payload_ownership_for_kura(&block, lane_id, dataspace_id, lane_block_height);
    let execution_context = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
        entrypoint_hash,
        lane_id,
        dataspace_id,
    )])
    .with_lane_payload_ownerships(vec![ownership]);
    block.set_execution_context(Some(execution_context));
    Arc::new(block)
}
fn rebind_kura_lane_payload_predecessor(
    block: &mut SignedBlock,
    previous_lane_block_descriptor_hash: Hash,
) -> SumeragiLanePayloadOwnership {
    let mut ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    ownership.previous_lane_block_descriptor_hash = Some(previous_lane_block_descriptor_hash);
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("rebinding predecessor keeps replay hashes canonical");
    ownership.subject_hash = replay_hashes.subject_hash;
    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
    let entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"kura-lane-artifact-entrypoint",
    ));
    let execution_context = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
        entrypoint_hash,
        ownership.lane_id,
        ownership.dataspace_id,
    )])
    .with_lane_payload_ownerships(vec![ownership.clone()]);
    block.set_execution_context(Some(execution_context));
    ownership
}
fn attach_ok_results_to_block(block: &mut SignedBlock) {
    let entrypoint_hashes: Vec<_> = block
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect();
    let results = entrypoint_hashes
        .iter()
        .map(|_| TransactionResultInner::Ok(DataTriggerSequence::default()))
        .collect();
    block
        .set_transaction_results(Vec::new(), &entrypoint_hashes, results)
        .expect("attach deterministic transaction results");
}
fn two_lane_runtime_config() -> RuntimeLaneConfig {
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::from(1),
        alias: "beta".to_string(),
        ..ModelLaneConfig::default()
    };
    let catalog = LaneCatalog::new(nonzero!(2_u32), vec![lane0, lane1]).expect("catalog");
    RuntimeLaneConfig::from_catalog(&catalog)
}
type DefaultKuraFixture = (TempDir, KuraConfig, Arc<Kura>);
type ConfiguredKuraFixture = (TempDir, KuraConfig, RuntimeLaneConfig, Arc<Kura>);

fn kura_storage_fixture(
    temp_context: &str,
    blocks_in_memory: NonZeroUsize,
) -> (TempDir, KuraConfig) {
    let temp_dir = TempDir::new().expect(temp_context);
    let config = kura_config_for_dir(&temp_dir, blocks_in_memory);
    (temp_dir, config)
}

fn unwrapped_kura_storage_fixture(blocks_in_memory: NonZeroUsize) -> (TempDir, KuraConfig) {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, blocks_in_memory);
    (temp_dir, config)
}

fn expect_default_kura_fixture(
    temp_context: &str,
    blocks_in_memory: NonZeroUsize,
    open_context: &str,
) -> DefaultKuraFixture {
    let (temp_dir, config) = kura_storage_fixture(temp_context, blocks_in_memory);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect(open_context);
    (temp_dir, config, kura)
}

fn kura_root_fixture(blocks_in_memory: NonZeroUsize) -> DefaultKuraFixture {
    expect_default_kura_fixture("create Kura root", blocks_in_memory, "open Kura")
}

fn unwrapped_kura_fixture() -> DefaultKuraFixture {
    let (temp_dir, config) = unwrapped_kura_storage_fixture(BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    (temp_dir, config, kura)
}

fn unwrapped_inline_kura_fixture_with_fsync(fsync_mode: FsyncMode) -> (TempDir, Arc<Kura>) {
    let temp_dir = TempDir::new().unwrap();
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = fsync_mode;
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    (temp_dir, kura)
}

fn expect_configured_kura_fixture(
    temp_context: &str,
    blocks_in_memory: NonZeroUsize,
    open_context: &str,
) -> ConfiguredKuraFixture {
    let (temp_dir, config) = kura_storage_fixture(temp_context, blocks_in_memory);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect(open_context);
    (temp_dir, config, lane_config, kura)
}

fn temporary_kura_fixture() -> ConfiguredKuraFixture {
    expect_configured_kura_fixture(
        "temporary Kura directory",
        BLOCKS_IN_MEMORY,
        "initialize Kura",
    )
}

fn expect_two_lane_storage_fixture(temp_context: &str) -> (TempDir, KuraConfig, RuntimeLaneConfig) {
    let (temp_dir, config) = kura_storage_fixture(temp_context, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    (temp_dir, config, lane_config)
}

fn two_lane_storage_fixture() -> (TempDir, KuraConfig, RuntimeLaneConfig) {
    expect_two_lane_storage_fixture("create temp dir")
}

fn autonomous_lane_storage_fixture() -> (TempDir, KuraConfig, RuntimeLaneConfig) {
    expect_two_lane_storage_fixture("temp dir")
}

type MarkedLaneBlockFixtureParts<B> = (
    (TempDir, KuraConfig, RuntimeLaneConfig),
    (
        LaneId,
        iroha_config::parameters::actual::LaneConfigEntry,
        u64,
    ),
    (B, SumeragiLanePayloadOwnership, LaneBlockProposalV1),
    Arc<Kura>,
);

struct MarkedLaneBlockFixture<B>(MarkedLaneBlockFixtureParts<B>);

impl<B> MarkedLaneBlockFixture<B> {
    fn into_parts(self) -> MarkedLaneBlockFixtureParts<B> {
        self.0
    }
}

impl MarkedLaneBlockFixture<Arc<SignedBlock>> {
    fn uncommitted() -> Self {
        Self::uncommitted_at(1)
    }

    fn uncommitted_at(lane_block_height: u64) -> Self {
        let (temp_dir, config, lane_config) = two_lane_storage_fixture();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry").clone();
        let block = dummy_block_with_lane_payload_ownership(
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        );
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let proposal = lane_block_proposal_from_ownership(&ownership);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        Self((
            (temp_dir, config, lane_config),
            (lane_id, lane_entry, lane_block_height),
            (block, ownership, proposal),
            kura,
        ))
    }
}

impl MarkedLaneBlockFixture<SignedBlock> {
    fn committed() -> Self {
        let (temp_dir, config, lane_config) = two_lane_storage_fixture();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry").clone();
        let lane_block_height = 1;
        let mut block = dummy_block_with_lane_payload_ownership(
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        )
        .as_ref()
        .clone();
        attach_ok_results_to_block(&mut block);
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let proposal = lane_block_proposal_from_ownership(&ownership);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        Self((
            (temp_dir, config, lane_config),
            (lane_id, lane_entry, lane_block_height),
            (block, ownership, proposal),
            kura,
        ))
    }
}

fn blank_kura_with_next_block() -> (Arc<Kura>, Arc<SignedBlock>) {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    (kura, block)
}

fn blank_kura_with_blocks() -> (Arc<Kura>, DummyBlocks) {
    let kura = Kura::blank_kura_for_testing();
    let blocks = DummyBlocks::new();
    (kura, blocks)
}

fn default_pipeline_sidecar_fixture() -> (
    TempDir,
    KuraConfig,
    Arc<Kura>,
    HashOf<BlockHeader>,
    PipelineRecoverySidecar,
) {
    let (temp_dir, config) = unwrapped_kura_storage_fixture(BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let block_hash = store_dummy_blocks(&kura, 1)[0];
    let sidecar = PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [0u8; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    (temp_dir, config, kura, block_hash, sidecar)
}

fn test_kura_with_default_lane_markers(
    config: &Config,
    lane_config: &RuntimeLaneConfig,
) -> (Arc<Kura>, BlockCount) {
    let (kura, block_count) = Kura::open_test_kura_with_configured_lane_config(config, lane_config)
        .expect("init Kura test fixture");
    establish_configured_lane_markers_for_test(&kura, lane_config);
    (kura, block_count)
}

fn establish_configured_lane_markers_for_test(kura: &Kura, lane_config: &RuntimeLaneConfig) {
    let configured_catalog_hash = kura
        .configured_lane_catalog_baseline()
        .expect("read configured Kura catalog baseline")
        .expect("configured Kura catalog baseline");
    for entry in lane_config.entries() {
        let incarnation = Hash::new(
            format!(
                "kura-lane-incarnation:{}:{}",
                entry.lane_id.as_u32(),
                entry.dataspace_id.as_u64()
            )
            .as_bytes(),
        );
        if entry.lane_id == lane_config.primary().lane_id {
            kura.establish_or_verify_configured_primary_geometry_anchor(
                entry,
                incarnation,
                configured_catalog_hash,
            )
            .expect("bind configured primary Kura test geometry");
        } else {
            kura.install_lane_incarnation_marker_for_test(entry, incarnation, 0)
                .expect("install explicit Kura test lane marker");
        }
    }
}

fn populate_strict_kura_store(dir: &TempDir, count: usize) {
    let config = kura_config_for_dir(dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let _ = store_dummy_blocks(&kura, count);
}
fn autonomous_lane_payload_for_kura(
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
    signer: &KeyPair,
) -> (NetworkId, u64, LaneExecutablePayloadV1) {
    let transaction = TransactionBuilder::new(
        test_network_id(b"kura-autonomous-view-checkpoint"),
        (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "autonomous checkpoint payload".to_owned(),
    )])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let entrypoint = TransactionEntrypoint::External(transaction);
    let entrypoint_hash = Hash::from(entrypoint.hash());
    let validator_set = vec![PeerId::new(signer.public_key().clone())];
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id,
        dataspace_id,
        lane_incarnation: Hash::new(b"kura-autonomous-view-incarnation"),
        proposal_height: 42,
        previous_lane_block_height: lane_block_height.saturating_sub(1),
        previous_lane_block_descriptor_hash: lane_block_height
            .checked_sub(1)
            .filter(|height| *height > 0)
            .map(|height| Hash::new(height.to_le_bytes())),
        lane_block_height,
        lane_block_view: 0,
        subject_hash: Hash::new(b"kura-autonomous-view-subject"),
        payload_ownership_hash: Hash::new(b"kura-autonomous-view-ownership"),
        rbc_instance_hash: Hash::new(b"kura-autonomous-view-rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![entrypoint_hash],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        validator_count: 1,
        min_quorum: 1,
        qc_mode_tag: "permissioned:kura-autonomous-view".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: Some(
            iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
                proposal_height: 42,
                proposal_view: 3,
                proposal_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"kura-autonomous-view-anchor",
                )),
            },
        ),
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let network_id = test_network_id(b"kura-autonomous-genesis");
    let epoch = 7;
    let routing_plan = RoutingPlan::single(crate::queue::RoutingDecision::new(
        proposal.descriptor.lane_id,
        proposal.descriptor.dataspace_id,
    ));
    let reservation = LaneQueueReservationKeyV1 {
        version: LaneQueueReservationKeyV1::VERSION,
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new(
            b"kura-autonomous-view-queue-plan-admission-binding",
        ),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"kura-autonomous-view-reservation-owner"),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        proposal,
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        validator_set[0].clone(),
        signer.private_key(),
    )
    .expect("autonomous payload");
    (network_id, epoch, payload)
}
fn write_autonomous_claim_inventory_fixture(
    store_root: &Path,
    payload: &LaneExecutablePayloadV1,
    entrypoint_hash: Hash,
    staged: bool,
) -> PathBuf {
    let mut claim = AutonomousLaneEntrypointClaimV1::new(payload, entrypoint_hash);
    if !staged {
        claim.state = AutonomousLaneEntrypointClaimStateV1::Released(Hash::new_from_chunks(&[
            b"iroha:kura:test-claim-inventory-retirement:v1\0",
            entrypoint_hash.as_ref(),
        ]));
    }
    let path = Kura::autonomous_lane_entrypoint_claim_path(
        store_root,
        &claim.network_id,
        &claim.entrypoint_hash,
    );
    let path = if staged {
        Kura::autonomous_lane_entrypoint_claim_temp_path(&path)
    } else {
        path
    };
    fs::create_dir_all(path.parent().expect("claim fixture parent"))
        .expect("create claim fixture shard");
    fs::write(
        &path,
        norito::to_bytes(&claim).expect("encode claim fixture"),
    )
    .expect("write claim fixture");
    path
}
fn install_autonomous_lane_marker_for_kura(
    kura: &Kura,
    lane_config: &RuntimeLaneConfig,
    payload: &LaneExecutablePayloadV1,
) {
    let descriptor = &payload.origin_proposal.descriptor;
    let entry = lane_config
        .entry(descriptor.lane_id)
        .expect("autonomous payload lane has configured storage");
    kura.install_lane_incarnation_marker_for_test(entry, descriptor.lane_incarnation, 0)
        .expect("install authoritative autonomous lane marker");
}
fn rebind_autonomous_lane_payload_for_kura(
    source: &LaneExecutablePayloadV1,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
    incarnation_tag: &[u8],
    signer: &KeyPair,
) -> LaneExecutablePayloadV1 {
    let mut proposal = source.origin_proposal.clone();
    proposal.descriptor.lane_id = lane_id;
    proposal.descriptor.dataspace_id = dataspace_id;
    proposal.descriptor.lane_incarnation = Hash::new(incarnation_tag);
    proposal.descriptor.previous_lane_block_height = lane_block_height.saturating_sub(1);
    proposal.descriptor.previous_lane_block_descriptor_hash = lane_block_height
        .checked_sub(1)
        .filter(|height| *height > 0)
        .map(|height| Hash::new(height.to_le_bytes()));
    proposal.descriptor.lane_block_height = lane_block_height;
    proposal.descriptor.subject_hash =
        Hash::new(format!("claim-subject:{}:{lane_block_height}", lane_id.as_u32()).into_bytes());
    proposal.descriptor.payload_ownership_hash =
        Hash::new(format!("claim-ownership:{}:{lane_block_height}", lane_id.as_u32()).into_bytes());
    proposal.descriptor.rbc_instance_hash =
        Hash::new(format!("claim-rbc:{}:{lane_block_height}", lane_id.as_u32()).into_bytes());
    proposal.descriptor.qc_mode_tag = format!(
        "permissioned:claim:lane:{}:dataspace:{}",
        lane_id.as_u32(),
        dataspace_id.as_u64()
    );
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let routing_plans: Vec<_> = source
        .entrypoints
        .iter()
        .map(|_| {
            RoutingPlan::single(crate::queue::RoutingDecision::new(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
            ))
        })
        .collect();
    let reservation_keys = source
        .entrypoints
        .iter()
        .zip(&routing_plans)
        .enumerate()
        .map(
            |(index, (entrypoint, routing_plan))| LaneQueueReservationKeyV1 {
                version: LaneQueueReservationKeyV1::VERSION,
                entrypoint_hash: entrypoint.hash(),
                queue_plan_admission_binding_hash: Hash::new_from_chunks(&[
                    b"kura-autonomous-bundle-queue-plan-admission-binding",
                    &u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes(),
                ]),
                routing_plan_digest: routing_plan.digest(),
                coordinator_leg: routing_plan.coordinator_leg(),
                lane_id: proposal.descriptor.lane_id,
                dataspace_id: proposal.descriptor.dataspace_id,
                lane_incarnation: proposal.descriptor.lane_incarnation,
                proposal_height: proposal.descriptor.proposal_height,
                lane_block_height: proposal.descriptor.lane_block_height,
                lane_block_view: proposal.descriptor.lane_block_view,
                reservation_owner_hash: Hash::new_from_chunks(&[
                    b"iroha:kura:test-autonomous-reservation-owner:v1\0",
                    proposal.proposal_hash.as_ref(),
                    entrypoint.hash().as_ref(),
                ]),
                proposal_identity_hash: proposal.proposal_hash,
            },
        )
        .collect();
    let receipt_slots = vec![None; source.entrypoints.len()];
    LaneExecutablePayloadV1::new_signed_with_reservations(
        source.network_id,
        source.epoch,
        proposal,
        source.entrypoints.clone(),
        reservation_keys,
        routing_plans,
        receipt_slots,
        PeerId::new(signer.public_key().clone()),
        signer.private_key(),
    )
    .expect("rebound autonomous payload")
}
fn repropose_autonomous_lane_payload_for_kura(
    source: &LaneExecutablePayloadV1,
    proposal_height: u64,
    signer: &KeyPair,
) -> LaneExecutablePayloadV1 {
    let mut proposal = source.origin_proposal.clone();
    proposal.descriptor.proposal_height = proposal_height;
    proposal.descriptor.subject_hash =
        Hash::new(format!("kura-autonomous-reproposal-subject:{proposal_height}").into_bytes());
    proposal.descriptor.payload_ownership_hash =
        Hash::new(format!("kura-autonomous-reproposal-ownership:{proposal_height}").into_bytes());
    proposal.descriptor.rbc_instance_hash =
        Hash::new(format!("kura-autonomous-reproposal-rbc:{proposal_height}").into_bytes());
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    proposal.proposal_hash = proposal.computed_proposal_hash();
    if let Some(hint) = proposal.payload_block_hint.as_mut() {
        hint.proposal_height = proposal_height;
        hint.proposal_view = hint.proposal_view.saturating_add(1);
        hint.proposal_block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            format!("kura-autonomous-reproposal-anchor:{proposal_height}").into_bytes(),
        ));
    }
    let reservation_keys = source
        .reservation_keys
        .iter()
        .zip(&source.entrypoint_hashes)
        .map(|(source_key, entrypoint_hash)| {
            let mut key = *source_key;
            key.proposal_height = proposal_height;
            key.proposal_identity_hash = proposal.proposal_hash;
            key.reservation_owner_hash = Hash::new_from_chunks(&[
                b"iroha:kura:test-autonomous-reproposal-owner:v1\0",
                proposal.proposal_hash.as_ref(),
                entrypoint_hash.as_ref(),
            ]);
            key
        })
        .collect();
    LaneExecutablePayloadV1::new_signed_with_reservations(
        source.network_id,
        source.epoch,
        proposal,
        source.entrypoints.clone(),
        reservation_keys,
        source.routing_plans.clone(),
        source.native_amx_receipts.clone(),
        PeerId::new(signer.public_key().clone()),
        signer.private_key(),
    )
    .expect("reproposed autonomous payload")
}
fn next_durable_lane_view_certificate_for_kura(
    source: &LaneBlockProposalV1,
    payload: &LaneExecutablePayloadV1,
    signer: &KeyPair,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
) -> DurableLaneBlockNewViewCertificateV1 {
    let target_view = source
        .descriptor
        .lane_block_view
        .checked_add(1)
        .expect("fixture view");
    let body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        source,
        payload,
        target_view,
        network_id,
        epoch,
    )
    .expect("NewView body");
    let vote = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
        body.clone(),
        PeerId::new(signer.public_key().clone()),
        signer.private_key(),
    )
    .expect("NewView vote");
    let certificate = crate::lane_consensus::aggregate_lane_block_new_view_votes(
        body,
        payload.origin_proposal.descriptor.validator_set.clone(),
        &[vote],
    )
    .expect("NewView certificate");
    DurableLaneBlockNewViewCertificateV1 {
        certificate,
        signer_pops: BTreeMap::from([(
            signer.public_key().clone(),
            bls_normal_pop_prove(signer.private_key()).expect("signer PoP"),
        )]),
    }
}
fn durable_lane_payload_availability_for_kura(
    payload: &LaneExecutablePayloadV1,
    proposal: &LaneBlockProposalV1,
    signer: &KeyPair,
) -> DurableLanePayloadAvailabilityCertificateV1 {
    let body = proposal.vote_body(Phase::Prepare);
    let signature = Signature::try_new(signer.private_key(), &body.signature_preimage())
        .expect("availability READY signature");
    let validator_set_pops =
        vec![bls_normal_pop_prove(signer.private_key()).expect("availability signer PoP")];
    let availability_body = crate::lane_consensus::lane_payload_availability_body(
        payload,
        proposal,
        payload.network_id,
        payload.epoch,
    )
    .expect("availability body");
    let availability_vote = crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
        availability_body,
        PeerId::new(signer.public_key().clone()),
        validator_set_pops,
        signer.private_key(),
    )
    .expect("availability READY vote");
    let vote = crate::lane_consensus::LaneBlockVoteV1 {
        body: body.clone(),
        signer: PeerId::new(signer.public_key().clone()),
        bls_signature: signature.payload().to_vec(),
        payload_availability_vote: Some(availability_vote),
    };
    let certificate = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        body,
        proposal.descriptor.validator_set.clone(),
        &[vote],
    )
    .expect("availability DELIVER QC");
    DurableLanePayloadAvailabilityCertificateV1 { certificate }
}
#[test]
fn prune_poison_rejects_lane_sidecar_writers_before_mutation() {
    let (_temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let dataspace_id = lane_config.entry(lane_id).expect("lane entry").dataspace_id;
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, dataspace_id, 1, &signer);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("seed autonomous payload");
    let availability =
        durable_lane_payload_availability_for_kura(&payload, &payload.origin_proposal, &signer);
    kura.persist_lane_payload_availability_certificate(
        lane_id,
        1,
        availability.clone(),
        network_id,
        epoch,
    )
    .expect("seed availability certificate");
    let recovered = kura
        .recover_autonomous_lane_block_payload(&payload.origin_proposal, network_id, epoch)
        .expect("recover autonomous input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("seed execution input");
    let input = kura
        .read_lane_block_execution_input(lane_id, 1)
        .expect("seeded execution input");
    let next_view = next_durable_lane_view_certificate_for_kura(
        &payload.origin_proposal,
        &payload,
        &signer,
        network_id,
        epoch,
    );
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, dataspace_id, 1);
    kura.prune_recovery_required.store(true, Ordering::Release);
    assert!(matches!(
        kura.persist_committed_lane_block_session(&session, &signer_pops),
        Err(Error::PruneRecoveryRequired)
    ));
    assert!(matches!(
        kura.persist_lane_executable_payload(&payload, network_id, epoch),
        Err(Error::PruneRecoveryRequired)
    ));
    assert!(matches!(
        kura.persist_lane_payload_availability_certificate(
            lane_id,
            1,
            availability,
            network_id,
            epoch,
        ),
        Err(Error::PruneRecoveryRequired)
    ));
    assert!(matches!(
        kura.persist_lane_new_view_certificate(lane_id, 1, next_view, network_id, epoch,),
        Err(Error::PruneRecoveryRequired)
    ));
    assert!(matches!(
        kura.persist_lane_block_execution_input(&recovered),
        Err(Error::PruneRecoveryRequired)
    ));
    assert!(matches!(
        kura.persist_lane_block_execution_preflight(&input, 0, None, Vec::new()),
        Err(Error::PruneRecoveryRequired)
    ));
    assert!(
        kura.read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch,)
            .is_none(),
        "prune poison must fail closed instead of serving autonomous lane sidecars"
    );
    assert!(
        kura.read_lane_block_execution_input(lane_id, 1).is_none(),
        "prune poison must fail closed instead of serving cached lane execution input"
    );
    kura.prune_recovery_required.store(false, Ordering::Release);
    assert!(
        kura.read_certified_lane_block_artifact(lane_id, 1)
            .is_none(),
        "poisoned certified-session write must leave no sidecar"
    );
    let autonomous = kura
        .read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch)
        .expect("original autonomous payload remains readable");
    assert!(
        autonomous.new_view_certificates.is_empty(),
        "poisoned NewView write must not mutate the durable view chain"
    );
    assert!(
        kura.read_lane_block_execution_preflight(lane_id, 1)
            .is_none(),
        "poisoned preflight write must leave no sidecar"
    );
}
#[test]
fn lane_sidecar_writer_waits_for_geometry_transition_lock() {
    let (_temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let dataspace_id = lane_config.entry(lane_id).expect("lane entry").dataspace_id;
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, dataspace_id, 1, &signer);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    let geometry_guard = kura.lane_geometry_lock.lock();
    let (started_tx, started_rx) = std::sync::mpsc::sync_channel(0);
    let (done_tx, done_rx) = std::sync::mpsc::sync_channel(0);
    let writer_kura = Arc::clone(&kura);
    let writer = thread::spawn(move || {
        started_tx.send(()).expect("announce sidecar writer");
        let result = writer_kura.persist_lane_executable_payload(&payload, network_id, epoch);
        done_tx.send(result).expect("report sidecar writer result");
    });
    started_rx.recv().expect("sidecar writer started");
    let writer_lock_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if kura.prune_lock.try_lock().is_none() {
            break;
        }
        assert!(
            Instant::now() < writer_lock_deadline,
            "sidecar writer never reached its prune/geometry critical section"
        );
        thread::yield_now();
    }
    assert!(
        matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ),
        "lane sidecar writer must not resolve or recreate a path while geometry is locked"
    );
    drop(geometry_guard);
    done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("writer resumes after geometry publication")
        .expect("writer persists on the current geometry");
    writer.join().expect("sidecar writer thread");
    assert!(
        kura.read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch)
            .is_some(),
        "writer must publish only after the current geometry becomes available"
    );
}
#[test]
fn autonomous_lane_availability_deliver_is_durable_and_fails_closed() {
    let (_temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist payload before READY");
    assert!(
        !kura.autonomous_lane_payload_availability_delivered(
            &payload.origin_proposal,
            network_id,
            epoch,
        ),
        "payload bytes alone are not an availability DELIVER certificate"
    );
    let deliver =
        durable_lane_payload_availability_for_kura(&payload, &payload.origin_proposal, &signer);
    kura.persist_lane_payload_availability_certificate(
        lane_id,
        1,
        deliver.clone(),
        network_id,
        epoch,
    )
    .expect("persist availability DELIVER");
    kura.persist_lane_payload_availability_certificate(
        lane_id,
        1,
        deliver.clone(),
        network_id,
        epoch,
    )
    .expect("exact availability replay is idempotent");
    assert!(kura.autonomous_lane_payload_availability_delivered(
        &payload.origin_proposal,
        network_id,
        epoch,
    ));
    let new_view = next_durable_lane_view_certificate_for_kura(
        &payload.origin_proposal,
        &payload,
        &signer,
        network_id,
        epoch,
    );
    let cursor = match kura
        .persist_lane_new_view_certificate(lane_id, 1, new_view, network_id, epoch)
        .expect("persist synthetic NewView cursor")
    {
        LaneBlockNewViewPersistenceOutcome::Persisted(cursor) => cursor,
        LaneBlockNewViewPersistenceOutcome::AlreadyTerminal => {
            panic!("non-terminal NewView fixture unexpectedly reached a terminal receipt")
        }
    };
    assert_eq!(cursor.descriptor.lane_block_view, 1);
    let next_view_deliver = durable_lane_payload_availability_for_kura(&payload, &cursor, &signer);
    assert!(
        kura.persist_lane_payload_availability_certificate(
            lane_id,
            1,
            next_view_deliver,
            network_id,
            epoch,
        )
        .is_err(),
        "a validly signed NewView Prepare QC must not replace the immutable origin READY QC",
    );
    assert!(kura.autonomous_lane_payload_availability_delivered(
        &payload.origin_proposal,
        network_id,
        epoch,
    ));
    assert!(
        !kura.autonomous_lane_payload_availability_delivered(&cursor, network_id, epoch,),
        "the synthetic cursor is not a second availability subject",
    );
    let (_, recovered_cursor) = kura
        .current_autonomous_lane_payload(lane_id, 1, network_id, epoch)
        .expect("recover current NewView cursor");
    assert_eq!(recovered_cursor, cursor);
    let (_, certification_proposal) = kura
        .autonomous_lane_certification_payload(lane_id, 1, network_id, epoch)
        .expect("recover immutable certification subject");
    assert_eq!(certification_proposal, payload.origin_proposal);
    let mut conflicting = deliver.clone();
    conflicting
        .certificate
        .payload_availability_qc
        .as_mut()
        .expect("availability QC")
        .body
        .executable_payload_hash = Hash::new(b"conflicting-availability-body");
    assert!(
        kura.persist_lane_payload_availability_certificate(
            lane_id,
            1,
            conflicting,
            network_id,
            epoch,
        )
        .is_err(),
        "a conflicting body must not replace a durable DELIVER certificate"
    );
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen Kura");
    assert!(reopened.autonomous_lane_payload_availability_delivered(
        &payload.origin_proposal,
        network_id,
        epoch,
    ));
    let reopened_artifact = reopened
        .read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch)
        .expect("reopen origin-certified autonomous payload");
    assert_eq!(
        reopened_artifact.availability_certificate.as_ref(),
        Some(&deliver),
        "failed replacement attempts must leave the first origin READY QC unchanged",
    );
    let view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
        lane_entry,
        &reopened.store_root,
        1,
        payload.origin_proposal.descriptor.proposal_height,
    );
    fs::write(&view_path, [0xFF, 0x00, 0xAA]).expect("corrupt availability state");
    assert!(
        !reopened.autonomous_lane_payload_availability_delivered(
            &payload.origin_proposal,
            network_id,
            epoch,
        ),
        "malformed durable availability state must fail closed after restart"
    );
}
#[test]
fn autonomous_lane_slot_retirement_is_terminal_idempotent_and_restart_durable() {
    let (_temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let availability =
        durable_lane_payload_availability_for_kura(&payload, &payload.origin_proposal, &signer);
    kura.persist_lane_payload_availability_certificate(lane_id, 1, availability, network_id, epoch)
        .expect("persist availability before retirement");
    let (session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, &signer);
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
    assert_eq!(
        kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch,)
            .expect("persist terminal slot retirement"),
        retirement,
    );
    assert_eq!(
        kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch,)
            .expect("exact retirement retry is idempotent"),
        retirement,
    );
    assert_eq!(
        kura.read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
            .expect("read durable retirement"),
        Some(retirement.clone()),
    );
    assert!(
        kura.read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch)
            .is_none(),
        "retired payload must be hidden from autonomous recovery",
    );
    assert!(
        kura.latest_autonomous_lane_block_artifacts_snapshot(network_id, 1, |_| Ok(epoch))
            .expect("load bounded route-latest snapshot")
            .is_empty(),
        "retired payload must be ineligible for startup merge recovery",
    );
    assert!(matches!(
        kura.recover_autonomous_lane_block_payload(&payload.origin_proposal, network_id, epoch,),
        Err(LaneBlockPayloadAvailability::MissingLaneArtifact)
    ));
    let reservation_group =
        autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let classified = kura
        .classify_autonomous_lane_reservation_groups(&[reservation_group], network_id, &[epoch])
        .expect("classify tombstoned reservation through the strict grouped predicate");
    assert!(matches!(
        classified.as_slice(),
        [AutonomousLaneReservationEvidenceV1::ExactRetired {
            payload: exact_payload,
            retirement: exact_retirement,
            ..
        }] if exact_payload == &payload && exact_retirement == &retirement
    ));
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch),
        Err("retired autonomous lane slot is not merge eligible"),
        "a locally supplied delayed QC cannot make a retired slot merge eligible",
    );
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "certification after durable retirement must fail closed",
    );
    assert!(
        kura.read_certified_lane_block_artifact(lane_id, 1)
            .is_none(),
        "rejected delayed certification must leave no certified sidecar",
    );
    assert!(
        kura.persist_lane_executable_payload(&payload, network_id, epoch)
            .is_err(),
        "the same payload cannot reclaim a terminal slot",
    );
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen Kura");
    assert_eq!(
        reopened
            .read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
            .expect("revalidate retirement after restart"),
        Some(retirement),
    );
    assert!(
        reopened
            .read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch)
            .is_none(),
        "restart must not resurrect the retired executable payload",
    );
}
