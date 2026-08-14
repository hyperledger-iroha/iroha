#[test]
fn unknown_marker_resolution_applies_or_discards_merge_association_stage() {
    for new_marker_won in [false, true] {
        let temp_dir = TempDir::new().expect("create Kura root");
        let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        config.fsync_mode = FsyncMode::Batched;
        config.fsync_interval = Duration::from_secs(60);
        let expected_entry = {
            let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
            let mut blocks = DummyBlocks::new();
            kura.store_block(blocks.next()).expect("store merge parent");
            let mut entry = sample_merge_entry(1);
            let carrier = next_merge_carrier(&mut blocks, &mut entry);
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
                kura.store_block_with_merge_entry(carrier, &entry),
                Err(Error::DaBlockRewriteCommitStateUnknown { .. })
            ));
            assert!(kura.canonical_association_stage_path().is_file());
            assert!(kura.merge_ledger_snapshot().is_empty());
            entry
        };
        let (reopened, count) = Kura::new(&config, &RuntimeLaneConfig::default())
            .expect("startup resolves merge association stage by marker");
        assert_eq!(count.0, if new_marker_won { 2 } else { 1 });
        assert_eq!(
            reopened.merge_ledger_snapshot(),
            if new_marker_won {
                vec![expected_entry]
            } else {
                Vec::new()
            }
        );
        assert!(!reopened.canonical_association_stage_path().exists());
    }
}
#[test]
fn replace_top_block_does_not_depend_on_writer_channel() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    let block_hash = block.hash();
    kura.store_block(block).expect("store block");
    kura.block_notify_rx.lock().take();
    let replacement: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    let replacement_hash = replacement.hash();
    assert_ne!(block_hash, replacement_hash);
    kura.replace_top_block(replacement)
        .expect("replace top block");
    assert_eq!(kura.blocks_count(), 1);
    let top_hash = kura.block_data.lock().last().map(|(hash, _)| *hash);
    assert_eq!(top_hash, Some(replacement_hash));
}
#[test]
fn replace_top_block_does_not_depend_on_writer_fault() {
    let kura = Kura::blank_kura_for_testing();
    let block = DummyBlocks::new().next();
    let block_hash = block.hash();
    kura.store_block(block).expect("store block");
    kura.record_writer_fault("test", &Error::BlockWriterUnavailable);
    let replacement: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into();
    let replacement_hash = replacement.hash();
    assert_ne!(block_hash, replacement_hash);
    kura.replace_top_block(replacement)
        .expect("replace top block");
    assert_eq!(kura.blocks_count(), 1);
    let top_hash = kura.block_data.lock().last().map(|(hash, _)| *hash);
    assert_eq!(top_hash, Some(replacement_hash));
}
#[test]
fn store_block_with_merge_entry_does_not_depend_on_writer_channel() {
    let kura = Kura::blank_kura_for_testing();
    kura.block_notify_rx.lock().take();
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let mut entry = sample_merge_entry(1);
    let block = next_merge_carrier(&mut blocks, &mut entry);
    kura.store_block(parent).expect("store carrier parent");
    kura.store_block_with_merge_entry(block, &entry)
        .expect("store block with merge entry");
    assert_eq!(kura.blocks_count(), 2);
    assert_eq!(kura.merge_ledger_snapshot().len(), 1);
}
#[test]
fn read_and_write_to_blockchain_data_store() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    block_store
        .write_block_data(43, b"This is some data!")
        .unwrap();
    let mut read_buffer = [0_u8; b"This is some data!".len()];
    block_store.read_block_data(43, &mut read_buffer).unwrap();
    assert_eq!(b"This is some data!", &read_buffer);
}
#[test]
fn block_bytes_matches_direct_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let dummy_block: SignedBlock = ValidBlock::new_dummy(checked_keypair().private_key()).into();
    block_store.append_block_to_chain(&dummy_block).unwrap();
    let BlockIndex { start, length } = block_store.read_block_index(0).unwrap();
    let len: usize = usize::try_from(length).expect("test block length fits in usize");
    let mut direct = vec![0_u8; len];
    block_store.read_block_data(start, &mut direct).unwrap();
    let slice_bytes = {
        let borrowed = block_store.block_bytes(start, length).unwrap();
        borrowed.to_vec()
    };
    assert_eq!(slice_bytes, direct);
}
#[test]
fn fresh_block_store_has_zero_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    assert_eq!(0, block_store.read_index_count().unwrap());
}
#[test]
fn append_block_to_chain_increases_block_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let dummy_block = ValidBlock::new_dummy(checked_keypair().private_key()).into();
    let append_count: usize = 35;
    for _ in 0..append_count {
        block_store.append_block_to_chain(&dummy_block).unwrap();
    }
    let index_count =
        usize::try_from(block_store.read_index_count().unwrap()).expect("index count fits");
    assert_eq!(append_count, index_count);
}
#[test]
fn append_block_to_chain_increases_hashes_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let dummy_block = ValidBlock::new_dummy(checked_keypair().private_key()).into();
    let append_count = 35;
    for _ in 0..append_count {
        block_store.append_block_to_chain(&dummy_block).unwrap();
    }
    assert_eq!(append_count, block_store.read_hashes_count().unwrap());
}
#[test]
fn append_block_to_chain_write_correct_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let dummy_block = ValidBlock::new_dummy(checked_keypair().private_key()).into();
    let append_count = 35;
    for _ in 0..append_count {
        block_store.append_block_to_chain(&dummy_block).unwrap();
    }
    let block_hashes = block_store.read_block_hashes(0, append_count).unwrap();
    for hash in block_hashes {
        assert_eq!(hash, dummy_block.hash())
    }
}
#[test]
fn append_block_to_chain_places_blocks_correctly_in_data_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let dummy_block = ValidBlock::new_dummy(checked_keypair().private_key()).into();
    let append_count: u64 = 35;
    for _ in 0..append_count {
        block_store.append_block_to_chain(&dummy_block).unwrap();
    }
    let block_wire = dummy_block
        .canonical_wire()
        .expect("canonical wire encoding");
    let block_len = block_wire.as_framed().len() as u64;
    for i in 0..append_count {
        let BlockIndex { start, length } = block_store.read_block_index(i).unwrap();
        assert_eq!(i * block_len, start);
        assert_eq!(block_len, length);
    }
}
#[test]
fn append_block_to_chain_roundtrip_decodes() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let block = DummyBlocks::new().next();
    block_store.append_block_to_chain(&block).unwrap();
    let BlockIndex { start, length } = block_store.read_block_index(0).unwrap();
    let len: usize = length.try_into().expect("block length fits in usize");
    let mut bytes = vec![0u8; len];
    block_store.read_block_data(start, &mut bytes).unwrap();
    let versioned = block.encode_versioned();
    let mut payload_cursor = std::io::Cursor::new(&versioned[1..]);
    let decoded_inline =
        SignedBlock::decode(&mut payload_cursor).expect("decode adaptive payload for inline bytes");
    assert_eq!(decoded_inline.hash(), block.hash());
    assert_eq!(bytes[0], versioned[0]);
    assert!(bytes[1..].starts_with(MAGIC.as_slice()));
    assert_eq!(&bytes[1 + Header::SIZE..], &versioned[1..]);
    let decoded = decode_framed_signed_block(&bytes).expect("decode stored block");
    assert_eq!(decoded.hash(), block.hash());
}
#[test]
fn append_block_batch_persists_all_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let leader = checked_keypair();
    let mut prev_hash = None;
    let mut blocks = Vec::new();
    for _ in 0..3 {
        let block: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                header.set_prev_block_hash(prev_hash);
            })
            .into(),
        );
        prev_hash = Some(block.hash());
        blocks.push(block);
    }
    block_store.append_block_batch(&blocks).unwrap();
    assert_eq!(block_store.read_index_count().unwrap(), 3);
    assert_eq!(block_store.read_hashes_count().unwrap(), 3);
    for (idx, block) in blocks.iter().enumerate() {
        let hash = block_store.read_block_hashes(idx as u64, 1).unwrap();
        assert_eq!(hash, vec![block.hash()]);
    }
}
#[test]
fn append_block_batch_sidecars_block_when_inline_budget_exceeded() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let leader = checked_keypair();
    let block1: Arc<SignedBlock> = Arc::new(ValidBlock::new_dummy(leader.private_key()).into());
    let block2: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
            header.set_prev_block_hash(Some(block1.hash()));
        })
        .into(),
    );
    let (block1_len, _) = block1.canonical_wire().expect("block1 wire").into_parts();
    let (block2_frame, _) = block2.canonical_wire().expect("block2 wire").into_parts();
    let block1_len = u64::try_from(block1_len.len()).expect("block1 length");
    let block2_len = u64::try_from(block2_frame.len()).expect("block2 length");
    block_store
        .append_block_batch_at(0, std::slice::from_ref(&block1), 0)
        .expect("append first block");
    let inline_budget = block1_len.saturating_add(2 * (BlockIndex::SIZE + SIZE_OF_BLOCK_HASH));
    block_store
        .append_block_batch_at(1, std::slice::from_ref(&block2), inline_budget)
        .expect("append sidecar block");
    assert_eq!(block_store.read_index_count().unwrap(), 2);
    assert_eq!(block_store.read_hashes_count().unwrap(), 2);
    assert_eq!(
        block_store.data_file_len().expect("data length"),
        block1_len,
        "sidecar append must not grow blocks.data"
    );
    let block2_index = block_store.read_block_index(1).expect("block2 index");
    assert!(block2_index.is_evicted());
    assert_eq!(block2_index.length, block2_len);
    let sidecar_path = block_store.da_block_path(2);
    assert!(sidecar_path.exists(), "sidecar body should be written");
    let sidecar = block_store
        .read_da_block_bytes(2, block2_len)
        .expect("read sidecar body");
    let decoded = decode_framed_signed_block(&sidecar).expect("decode sidecar block");
    assert_eq!(decoded.hash(), block2.hash());
    assert_eq!(
        block_store.read_block_hashes(1, 1).unwrap(),
        vec![block2.hash()]
    );
}
#[test]
fn restart_resolves_staged_evicted_rewrite_by_durable_marker() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let leader = checked_keypair();
    let block1: Arc<SignedBlock> = Arc::new(ValidBlock::new_dummy(leader.private_key()).into());
    let block2: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
            header.set_prev_block_hash(Some(block1.hash()));
        })
        .into(),
    );
    let replacement: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
            header.set_prev_block_hash(Some(block1.hash()));
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into(),
    );
    assert_ne!(replacement.hash(), block2.hash());
    let (block1_frame, _) = block1
        .canonical_wire()
        .expect("block one wire")
        .into_parts();
    let inline_budget = u64::try_from(block1_frame.len())
        .expect("block one length")
        .saturating_add(2 * (BlockIndex::SIZE + SIZE_OF_BLOCK_HASH));
    block_store
        .append_block_batch_at(0, std::slice::from_ref(&block1), 0)
        .expect("append first block");
    block_store
        .append_block_batch_at(1, std::slice::from_ref(&block2), inline_budget)
        .expect("append original evicted block");
    let sidecar_path = block_store.da_block_path(2);
    let original_sidecar = std::fs::read(&sidecar_path).expect("read original sidecar");
    assert_eq!(
        decode_framed_signed_block(&original_sidecar)
            .expect("decode original sidecar")
            .hash(),
        block2.hash()
    );
    block_store
        .fail_next_da_rewrite_before_marker
        .store(true, Ordering::Release);
    let error = block_store
        .append_block_batch_at(1, std::slice::from_ref(&replacement), inline_budget)
        .expect_err("injected pre-marker failure must roll back before returning");
    assert!(matches!(error, Error::IO(_, _)));
    assert_eq!(
        std::fs::read(&sidecar_path).expect("old sidecar survives failed replacement"),
        original_sidecar
    );
    assert!(
        !block_store.da_block_rewrite_stage_path().exists(),
        "in-call rollback must remove the rewrite stage"
    );
    assert_eq!(
        block_store.read_block_hashes(1, 1).unwrap(),
        vec![block2.hash()],
        "live readers must retain the old hash journal after a pre-marker error"
    );
    drop(block_store);
    let mut block_store = BlockStore::new(dir.path());
    block_store
        .create_files_if_they_do_not_exist()
        .expect("restart reconciles the pre-marker rewrite stage");
    assert!(
        !block_store.da_block_rewrite_stage_path().exists(),
        "successful rollback must remove its stage"
    );
    assert_eq!(
        block_store.read_block_hashes(1, 1).unwrap(),
        vec![block2.hash()],
        "the old marker must restore the exact original hash journal"
    );
    assert!(block_store.read_block_index(1).unwrap().is_evicted());
    assert_eq!(
        std::fs::read(&sidecar_path).expect("old sidecar survives restart rollback"),
        original_sidecar,
        "restart must preserve the exact original evicted body"
    );
    block_store
        .fail_next_da_rewrite_after_marker
        .store(true, Ordering::Release);
    block_store
        .append_block_batch_at(1, std::slice::from_ref(&replacement), inline_budget)
        .expect("a durable replacement marker is a committed success");
    assert!(
        !block_store.da_block_rewrite_stage_path().exists(),
        "in-call post-marker recovery must finish body promotion"
    );
    assert!(block_store.take_deferred_da_recovery_fault().is_none());
    let live_replacement_sidecar =
        std::fs::read(&sidecar_path).expect("read live replacement sidecar");
    assert_eq!(
        decode_framed_signed_block(&live_replacement_sidecar)
            .expect("decode live replacement sidecar")
            .hash(),
        replacement.hash(),
        "live readers must observe the committed replacement before return"
    );
    assert_eq!(
        block_store.read_block_hashes(1, 1).unwrap(),
        vec![replacement.hash()]
    );
    drop(block_store);
    let mut block_store = BlockStore::new(dir.path());
    block_store
        .create_files_if_they_do_not_exist()
        .expect("restart promotes a rewrite whose new marker is durable");
    assert!(!block_store.da_block_rewrite_stage_path().exists());
    let replacement_sidecar =
        std::fs::read(&sidecar_path).expect("read atomically replaced sidecar");
    assert_ne!(replacement_sidecar, original_sidecar);
    assert_eq!(
        decode_framed_signed_block(&replacement_sidecar)
            .expect("decode replacement sidecar")
            .hash(),
        replacement.hash()
    );
    assert_eq!(
        block_store.read_block_hashes(1, 1).unwrap(),
        vec![replacement.hash()]
    );
}
#[test]
fn startup_recovers_both_abrupt_da_rewrite_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let leader = checked_keypair();
    let block1: Arc<SignedBlock> = Arc::new(ValidBlock::new_dummy(leader.private_key()).into());
    let block2: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
            header.set_prev_block_hash(Some(block1.hash()));
        })
        .into(),
    );
    let replacement: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
            header.set_prev_block_hash(Some(block1.hash()));
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into(),
    );
    let (block1_frame, _) = block1
        .canonical_wire()
        .expect("block one wire")
        .into_parts();
    let inline_budget = u64::try_from(block1_frame.len())
        .expect("block one length")
        .saturating_add(2 * (BlockIndex::SIZE + SIZE_OF_BLOCK_HASH));
    block_store
        .append_block_batch_at(0, std::slice::from_ref(&block1), 0)
        .expect("append first block");
    block_store
        .append_block_batch_at(1, std::slice::from_ref(&block2), inline_budget)
        .expect("append original evicted block");
    let sidecar_path = block_store.da_block_path(2);
    let original_sidecar = std::fs::read(&sidecar_path).expect("read original sidecar");
    block_store
        .crash_next_da_rewrite_before_marker
        .store(true, Ordering::Release);
    block_store
        .append_block_batch_at(1, std::slice::from_ref(&replacement), inline_budget)
        .expect_err("simulate abrupt stop before marker publication");
    assert!(block_store.da_block_rewrite_stage_path().is_file());
    assert_eq!(
        block_store.read_block_hashes(1, 1).unwrap(),
        vec![replacement.hash()],
        "the simulated crash must occur after replacement journal writes"
    );
    assert_eq!(
        block_store
            .read_commit_marker()
            .unwrap()
            .expect("old marker")
            .tip_hash,
        Some(block2.hash())
    );
    drop(block_store);
    let mut block_store = BlockStore::new(dir.path());
    block_store
        .create_files_if_they_do_not_exist()
        .expect("old marker restores the original rewrite suffix");
    assert!(!block_store.da_block_rewrite_stage_path().exists());
    assert_eq!(
        block_store.read_block_hashes(1, 1).unwrap(),
        vec![block2.hash()]
    );
    assert_eq!(
        std::fs::read(&sidecar_path).expect("old sidecar after startup rollback"),
        original_sidecar
    );
    block_store
        .crash_next_da_rewrite_after_marker
        .store(true, Ordering::Release);
    block_store
        .append_block_batch_at(1, std::slice::from_ref(&replacement), inline_budget)
        .expect_err("simulate abrupt stop after marker publication");
    assert!(block_store.da_block_rewrite_stage_path().is_file());
    assert_eq!(
        block_store
            .read_commit_marker()
            .unwrap()
            .expect("new marker")
            .tip_hash,
        Some(replacement.hash())
    );
    assert_eq!(
        std::fs::read(&sidecar_path).expect("old sidecar before startup promotion"),
        original_sidecar
    );
    drop(block_store);
    let mut block_store = BlockStore::new(dir.path());
    block_store
        .create_files_if_they_do_not_exist()
        .expect("new marker promotes the staged replacement body");
    assert!(!block_store.da_block_rewrite_stage_path().exists());
    let promoted = std::fs::read(&sidecar_path).expect("promoted replacement sidecar");
    assert_eq!(
        decode_framed_signed_block(&promoted)
            .expect("decode promoted replacement")
            .hash(),
        replacement.hash()
    );
    assert_eq!(
        block_store.read_block_hashes(1, 1).unwrap(),
        vec![replacement.hash()]
    );
}
#[test]
fn append_block_batch_at_rewrites_tail() {
    let dir = tempfile::tempdir().unwrap();
    let mut block_store = BlockStore::new(dir.path());
    block_store.create_files_if_they_do_not_exist().unwrap();
    let leader = checked_keypair();
    let block1: Arc<SignedBlock> = Arc::new(ValidBlock::new_dummy(leader.private_key()).into());
    let block2: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
            header.set_prev_block_hash(Some(block1.hash()));
        })
        .into(),
    );
    block_store
        .append_block_batch(&[block1.clone(), block2.clone()])
        .unwrap();
    let replacement: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
            header.set_prev_block_hash(Some(block1.hash()));
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into(),
    );
    assert_ne!(replacement.hash(), block2.hash(), "replacement must differ");
    block_store
        .append_block_batch_at(1, std::slice::from_ref(&replacement), 0)
        .unwrap();
    assert_eq!(block_store.read_index_count().unwrap(), 2);
    assert_eq!(block_store.read_hashes_count().unwrap(), 2);
    let hash = block_store.read_block_hashes(1, 1).unwrap();
    assert_eq!(hash, vec![replacement.hash()]);
    let BlockIndex { start, length } = block_store.read_block_index(1).unwrap();
    let len: usize = length.try_into().expect("block length fits in usize");
    let mut bytes = vec![0_u8; len];
    block_store.read_block_data(start, &mut bytes).unwrap();
    let decoded = decode_framed_signed_block(&bytes).expect("decode replaced block");
    assert_eq!(decoded.hash(), replacement.hash());
}
#[test]
fn strict_init_kura() {
    let temp_dir = TempDir::new().unwrap();
    Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(
                temp_dir.path().to_str().unwrap().into(),
            ),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .unwrap();
}
#[test]
fn kura_not_miss_replace_block() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .unwrap();
    {
        let _rt_guard = rt.enter();
        let _logger = iroha_logger::test_logger();
    }
    // Create kura and write some blocks
    let temp_dir = TempDir::new().unwrap();
    let [block_genesis, _block, block_soft_fork, block_next] =
        create_blocks(&rt, &temp_dir).try_into().unwrap();
    // Reinitialize kura and check that correct blocks are loaded
    {
        let (kura, block_count) = Kura::new(
            &Config {
                init_mode: InitMode::Strict,
                store_dir: iroha_config::base::WithOrigin::inline(
                    temp_dir.path().to_str().unwrap().into(),
                ),
                max_disk_usage_bytes:
                    iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
                blocks_in_memory: BLOCKS_IN_MEMORY,
                debug_output_new_blocks: false,
                merge_ledger_cache_capacity:
                    iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
                fsync_mode: iroha_config::kura::FsyncMode::Batched,
                fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
                block_sync_roster_retention:
                    iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
                roster_sidecar_retention:
                    iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
                replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
            },
            &RuntimeLaneConfig::default(),
        )
        .unwrap();
        assert_eq!(block_count.0, 3);
        assert_eq!(
            kura.get_block(nonzero!(1_usize)).unwrap().hash(),
            block_genesis.as_ref().hash()
        );
        assert_eq!(
            kura.get_block(nonzero!(2_usize)).unwrap().hash(),
            block_soft_fork.as_ref().hash()
        );
        assert_eq!(
            kura.get_block(nonzero!(3_usize)).unwrap().hash(),
            block_next.as_ref().hash()
        );
    }
}
#[test]
fn get_block_caches_loaded_block() {
    let temp_dir = TempDir::new().unwrap();
    let block_count = 3usize;
    populate_store(&temp_dir, block_count);
    let (kura, _) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(
                temp_dir.path().to_str().unwrap().into(),
            ),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .unwrap();
    let height = NonZeroUsize::new(block_count).unwrap();
    assert_eq!(
        kura.block_data.lock().len(),
        block_count,
        "strict init should load all appended blocks"
    );
    let first = kura.get_block(height).expect("block available");
    let second = kura.get_block(height).expect("cached block");
    assert!(Arc::ptr_eq(&first, &second));
}
#[test]
fn transaction_index_completes_after_lazy_loading_reopened_blocks() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .unwrap();
    {
        let _rt_guard = rt.enter();
        let _logger = iroha_logger::test_logger();
    }
    let temp_dir = TempDir::new().unwrap();
    let blocks = create_blocks(&rt, &temp_dir);
    let entrypoint_hash = blocks[2]
        .as_ref()
        .entrypoint_hashes()
        .next()
        .expect("canonical test block has a transaction");
    let (kura, block_count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(
                temp_dir.path().to_str().unwrap().into(),
            ),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .expect("reopen Kura");
    assert_eq!(block_count.0, 3);
    // Startup authenticates every durable body while reconciling sparse
    // merge carriers, which also eagerly rebuilds the transaction index.
    // Recreate the retained-body cache index so this test continues to
    // exercise completion by ordinary lazy block reads.
    let retained_body_index = {
        let block_data = kura.block_data.lock();
        Kura::build_transaction_entrypoint_index(&block_data)
    };
    assert!(
        !retained_body_index.complete,
        "a one-body retained cache must leave a three-block index partial"
    );
    *kura.transaction_entrypoint_index.lock() = retained_body_index;
    assert!(
        kura.get_block_heights_by_entrypoint_hash(entrypoint_hash)
            .is_none(),
        "the retained-body cache starts with a partial transaction index"
    );
    for height in 1..=block_count.0 {
        let height = NonZeroUsize::new(height).expect("non-zero height");
        kura.get_block(height).expect("block loads from disk");
    }
    assert_eq!(
        kura.get_block_heights_by_entrypoint_hash(entrypoint_hash)
            .expect("all reopened blocks have been indexed"),
        BTreeSet::from([nonzero!(2_usize)])
    );
}
#[test]
fn offline_operation_index_distinguishes_missing_partial_and_earliest_height() {
    let kura = Kura::blank_kura_for_testing();
    let operation_id = [0xA5; 32];
    assert_eq!(
        kura.get_earliest_block_height_by_offline_operation_id(
            &SAMPLE_GENESIS_ACCOUNT_ID,
            operation_id,
        ),
        Some(None),
        "an empty complete index reports a definite miss"
    );
    {
        let mut index = kura.transaction_entrypoint_index.lock();
        index.complete = true;
        index.heights_by_offline_operation_id.insert(
            (SAMPLE_GENESIS_ACCOUNT_ID.clone(), operation_id),
            BTreeSet::from([nonzero!(3_usize), nonzero!(1_usize)]),
        );
        index.indexed_heights =
            BTreeSet::from([nonzero!(1_usize), nonzero!(2_usize), nonzero!(3_usize)]);
    }
    assert_eq!(
        kura.get_earliest_block_height_by_offline_operation_id(
            &SAMPLE_GENESIS_ACCOUNT_ID,
            operation_id,
        ),
        Some(Some(nonzero!(1_usize)))
    );
    kura.truncate_transaction_entrypoint_index(2);
    assert_eq!(
        kura.get_earliest_block_height_by_offline_operation_id(
            &SAMPLE_GENESIS_ACCOUNT_ID,
            operation_id,
        ),
        Some(Some(nonzero!(1_usize))),
        "truncation retains the earliest surviving occurrence"
    );
    kura.transaction_entrypoint_index.lock().complete = false;
    assert_eq!(
        kura.get_earliest_block_height_by_offline_operation_id(
            &SAMPLE_GENESIS_ACCOUNT_ID,
            operation_id,
        ),
        None,
        "a partial index must not turn an unknown result into a miss"
    );
}
#[test]
fn offline_operation_index_extracts_authorized_ids_and_ignores_zero_or_unrelated_entries() {
    let operation_id = [0xA6; 32];
    let top_level_mismatch = [0xA7; 32];
    let entrypoint = offline_top_up_entrypoint_for_index(top_level_mismatch, operation_id);
    let mut index = TransactionEntrypointIndex::complete_empty();
    Kura::insert_offline_operation_id_heights(&mut index, nonzero!(3_usize), &entrypoint);
    Kura::insert_offline_operation_id_heights(&mut index, nonzero!(1_usize), &entrypoint);
    let operation_key = (SAMPLE_GENESIS_ACCOUNT_ID.clone(), operation_id);
    assert_eq!(
        index.heights_by_offline_operation_id.get(&operation_key),
        Some(&BTreeSet::from([nonzero!(1_usize), nonzero!(3_usize)])),
        "the signed authorization id is the canonical retry identity"
    );
    assert!(
        !index
            .heights_by_offline_operation_id
            .contains_key(&(SAMPLE_GENESIS_ACCOUNT_ID.clone(), top_level_mismatch)),
        "a malformed duplicate top-level id must not create a second lookup identity"
    );
    let zero = offline_top_up_entrypoint_for_index([0; 32], [0; 32]);
    Kura::insert_offline_operation_id_heights(&mut index, nonzero!(2_usize), &zero);
    assert!(
        !index
            .heights_by_offline_operation_id
            .contains_key(&(SAMPLE_GENESIS_ACCOUNT_ID.clone(), [0; 32]))
    );
    let unrelated = TransactionBuilder::new(
        test_network_id(b"kura-offline-operation-index"),
        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "unrelated".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    Kura::insert_offline_operation_id_heights(
        &mut index,
        nonzero!(2_usize),
        &TransactionEntrypoint::External(unrelated),
    );
    assert_eq!(index.heights_by_offline_operation_id.len(), 1);
    Kura::remove_transaction_entrypoint_height(&mut index, nonzero!(1_usize));
    assert_eq!(
        index
            .heights_by_offline_operation_id
            .get(&operation_key)
            .and_then(|heights| heights.first().copied()),
        Some(nonzero!(3_usize))
    );
    Kura::remove_transaction_entrypoint_height(&mut index, nonzero!(3_usize));
    assert!(index.heights_by_offline_operation_id.is_empty());
}
#[test]
fn offline_operation_index_cannot_be_shadowed_by_another_outer_authority() {
    let operation_id = [0xA7; 32];
    let front_runner = KeyPair::try_from_seed(vec![0xA7; 32], Algorithm::Ed25519)
        .expect("derive unauthorized offline front-run fixture key");
    let front_runner_id = AccountId::new(front_runner.public_key().clone());
    let rejected_front_run = offline_top_up_entrypoint_for_index_with_outer_authority(
        operation_id,
        operation_id,
        &front_runner,
    );
    let issuer_submission = offline_top_up_entrypoint_for_index(operation_id, operation_id);
    let mut index = TransactionEntrypointIndex::complete_empty();
    // The first transaction carries the observed signed request but uses an outer
    // authority that is not the configured Torii issuer, so execution can reject it.
    // Its earlier height must not shadow the later canonical issuer submission.
    Kura::insert_offline_operation_id_heights(&mut index, nonzero!(1_usize), &rejected_front_run);
    Kura::insert_offline_operation_id_heights(&mut index, nonzero!(2_usize), &issuer_submission);
    index.indexed_heights = BTreeSet::from([nonzero!(1_usize), nonzero!(2_usize)]);
    let kura = Kura::blank_kura_for_testing();
    *kura.transaction_entrypoint_index.lock() = index;
    assert_eq!(
        kura.get_earliest_block_height_by_offline_operation_id(
            &SAMPLE_GENESIS_ACCOUNT_ID,
            operation_id,
        ),
        Some(Some(nonzero!(2_usize))),
        "the configured issuer lookup must skip an earlier foreign-authority transaction"
    );
    assert_eq!(
        kura.get_earliest_block_height_by_offline_operation_id(&front_runner_id, operation_id,),
        Some(Some(nonzero!(1_usize))),
        "the foreign transaction remains isolated in its own authority namespace"
    );
}
#[test]
fn transaction_index_refresh_reapplies_merge_side_entries_atomically() {
    let kura = Kura::blank_kura_for_testing();
    let operation_id = [0xA8; 32];
    let merge_entry = merge_entry_with_indexed_entrypoint(offline_top_up_entrypoint_for_index(
        operation_id,
        operation_id,
    ));
    let block = DummyBlocks::new().next();
    for _ in 0..2 {
        kura.set_transaction_entrypoint_index_entry(1, &block, 1, Some(&merge_entry));
        assert_eq!(
            kura.get_earliest_block_height_by_offline_operation_id(
                &SAMPLE_GENESIS_ACCOUNT_ID,
                operation_id,
            ),
            Some(Some(nonzero!(1_usize))),
            "ordinary-index refresh must not discard an operation carried by the merge sidecar"
        );
        let index = kura.transaction_entrypoint_index.lock();
        assert!(index.complete);
        assert_eq!(
            index.heights_by_offline_operation_id
                [&(SAMPLE_GENESIS_ACCOUNT_ID.clone(), operation_id)],
            BTreeSet::from([nonzero!(1_usize)]),
            "repeated refreshes must remain idempotent"
        );
    }
}
#[test]
fn drop_persisted_blocks_keeps_genesis_and_recent_blocks() {
    let mut generator = DummyBlocks::new();
    let mut block_data: BlockData = (0..4)
        .map(|_| {
            let block = generator.next();
            (block.hash(), Some(block))
        })
        .collect();
    Kura::drop_persisted_blocks(&mut block_data, 2, 2);
    assert_eq!(
        block_data
            .iter()
            .filter(|(_, block)| block.is_some())
            .count(),
        4,
        "no blocks should be dropped while within retention"
    );
    Kura::drop_persisted_blocks(&mut block_data, 4, 2);
    assert!(block_data[0].1.is_some(), "genesis block stays cached");
    assert!(
        block_data[1].1.is_none(),
        "oldest non-genesis block should be dropped"
    );
    assert!(block_data[2].1.is_some(), "recent block should stay cached");
    assert!(block_data[3].1.is_some(), "latest block should stay cached");
}
#[test]
fn drop_persisted_blocks_keeps_unpersisted_blocks() {
    let mut generator = DummyBlocks::new();
    let mut block_data: BlockData = (0..6)
        .map(|_| {
            let block = generator.next();
            (block.hash(), Some(block))
        })
        .collect();
    Kura::drop_persisted_blocks(&mut block_data, 4, 2);
    assert!(block_data[0].1.is_some(), "genesis block stays cached");
    assert!(
        block_data[1].1.is_none(),
        "oldest persisted block should be dropped"
    );
    assert!(
        block_data[2].1.is_some(),
        "retained persisted block stays cached"
    );
    assert!(
        block_data[3].1.is_some(),
        "latest persisted block stays cached"
    );
    assert!(block_data[4].1.is_some(), "unpersisted block stays cached");
    assert!(block_data[5].1.is_some(), "unpersisted block stays cached");
}
#[test]
fn get_block_returns_none_when_data_missing() {
    let temp_dir = TempDir::new().unwrap();
    // Keep a genesis block and one cached tail block around the non-cached block under test.
    // Otherwise `get_block` correctly serves the requested block from memory after the data
    // file is removed, and the test never exercises its missing-disk-data path.
    populate_store(&temp_dir, 3);
    let (kura, _) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(
                temp_dir.path().to_str().unwrap().into(),
            ),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Batched,
            fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
            block_sync_roster_retention:
                iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention:
                iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .unwrap();
    let data_path = primary_blocks_dir(&temp_dir).join(DATA_FILE_NAME);
    std::fs::remove_file(&data_path).unwrap();
    assert!(
        kura.get_block(nonzero!(2_usize)).is_none(),
        "expected missing block to yield None"
    );
}
#[test]
fn eviction_requires_remote_replicas() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let (kura, _) = Kura::new(
        &KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .expect("kura init");
    let evict_len = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index").length
    };
    let freed = kura
        .evict_block_bodies(evict_len)
        .expect("evict block bodies");
    assert_eq!(freed, 0, "eviction must wait for remote replicas");
    let index = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index")
    };
    assert!(
        !index.is_evicted(),
        "block body should remain inline without replica adverts"
    );
}
fn open_eviction_compaction_fixture(
    temp_dir: &TempDir,
    block_count: usize,
) -> (KuraConfig, Arc<Kura>, Vec<Arc<SignedBlock>>) {
    let config = kura_config_for_dir(temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open empty Kura");
    let blocks = store_dummy_block_arcs(&kura, block_count);
    for artifact in v2_finality_artifacts_for_chain(&blocks) {
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist signed complete-wire finality before eviction");
    }
    (config, kura, blocks)
}
fn assert_eviction_compaction_restart_rolls_forward(boundary: u8) {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (config, kura, blocks) = open_eviction_compaction_fixture(&temp_dir, 4);
    let expected = Arc::clone(&blocks[1]);
    for artifact in v2_finality_artifacts_for_chain(&blocks) {
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist exact finality before eviction");
    }
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    {
        let store = kura.block_store.lock();
        match boundary {
            0 => store
                .crash_next_eviction_after_stage
                .store(true, Ordering::Release),
            1 => store
                .crash_next_eviction_after_data_promotion
                .store(true, Ordering::Release),
            2 => store
                .crash_next_eviction_after_index_promotion
                .store(true, Ordering::Release),
            _ => panic!("unsupported eviction crash boundary"),
        }
    }
    assert!(matches!(
        kura.evict_block_bodies(payload_len),
        Err(Error::CanonicalStoragePoisoned)
    ));
    let blocks_dir = primary_blocks_dir(&temp_dir);
    assert!(
        blocks_dir
            .join(EVICTION_COMPACTION_STAGE_FILE_NAME)
            .exists(),
        "the durable roll-forward stage must survive the injected stop"
    );
    assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
    drop(kura);
    let (reopened, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("recover staged compaction");
    assert!(
        !blocks_dir
            .join(EVICTION_COMPACTION_STAGE_FILE_NAME)
            .exists()
    );
    assert!(!blocks_dir.join(EVICTION_COMPACTION_DATA_FILE_NAME).exists());
    assert!(
        !blocks_dir
            .join(EVICTION_COMPACTION_INDEX_FILE_NAME)
            .exists()
    );
    let index = reopened
        .block_store
        .lock()
        .read_block_index(1)
        .expect("read recovered eviction index");
    assert!(index.is_evicted());
    let recovered = reopened
        .get_block(nonzero!(2_usize))
        .expect("rehydrate recovered evicted block");
    let recovered_wire = recovered
        .canonical_wire()
        .expect("encode recovered canonical block wire");
    let expected_wire = expected
        .canonical_wire()
        .expect("encode expected canonical block wire");
    assert_eq!(
        recovered_wire.as_framed(),
        expected_wire.as_framed(),
        "recovered block wire must match the pre-compaction canonical bytes"
    );
    assert!(
        reopened
            .v2_finality_artifact(2)
            .expect("read preserved finality")
            .is_some(),
        "compaction recovery must preserve finalized history"
    );
}
#[test]
fn eviction_compaction_restart_rolls_forward_after_stage_publication() {
    assert_eviction_compaction_restart_rolls_forward(0);
}
#[test]
fn eviction_compaction_restart_rolls_forward_after_data_promotion() {
    assert_eviction_compaction_restart_rolls_forward(1);
}
#[test]
fn eviction_compaction_restart_rolls_forward_after_pair_promotion() {
    assert_eviction_compaction_restart_rolls_forward(2);
}
#[test]
fn eviction_compaction_restart_rejects_missing_staged_replacement() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (config, kura, _) = open_eviction_compaction_fixture(&temp_dir, 4);
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.block_store
        .lock()
        .crash_next_eviction_after_stage
        .store(true, Ordering::Release);
    assert!(matches!(
        kura.evict_block_bodies(payload_len),
        Err(Error::CanonicalStoragePoisoned)
    ));
    let blocks_dir = primary_blocks_dir(&temp_dir);
    std::fs::remove_file(blocks_dir.join(EVICTION_COMPACTION_DATA_FILE_NAME))
        .expect("remove staged replacement data");
    drop(kura);
    let error = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect_err("missing staged compaction data must fail closed");
    assert!(
        error
            .to_string()
            .contains("neither live nor temporary eviction file"),
        "unexpected recovery error: {error}"
    );
    assert!(
        blocks_dir
            .join(EVICTION_COMPACTION_STAGE_FILE_NAME)
            .exists(),
        "failed recovery must retain its durable decision record"
    );
}
#[test]
fn eviction_compaction_restart_rejects_tampered_staged_replacement() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (config, kura, _) = open_eviction_compaction_fixture(&temp_dir, 4);
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.block_store
        .lock()
        .crash_next_eviction_after_stage
        .store(true, Ordering::Release);
    assert!(matches!(
        kura.evict_block_bodies(payload_len),
        Err(Error::CanonicalStoragePoisoned)
    ));
    let blocks_dir = primary_blocks_dir(&temp_dir);
    let replacement = blocks_dir.join(EVICTION_COMPACTION_DATA_FILE_NAME);
    let mut bytes = std::fs::read(&replacement).expect("read staged replacement data");
    let first = bytes
        .first_mut()
        .expect("four-block compaction replacement is non-empty");
    *first ^= 0x80;
    std::fs::write(&replacement, bytes).expect("tamper staged replacement data");
    drop(kura);
    let error = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect_err("tampered staged compaction data must fail closed");
    assert!(
        error
            .to_string()
            .contains("neither live nor temporary eviction file"),
        "unexpected recovery error: {error}"
    );
    assert!(
        blocks_dir
            .join(EVICTION_COMPACTION_STAGE_FILE_NAME)
            .exists(),
        "failed authentication must retain its durable decision record"
    );
}
#[test]
fn eviction_compaction_restart_rejects_tampered_retained_wire_binding() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (config, kura, _) = open_eviction_compaction_fixture(&temp_dir, 4);
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.block_store
        .lock()
        .crash_next_eviction_after_stage
        .store(true, Ordering::Release);
    assert!(matches!(
        kura.evict_block_bodies(payload_len),
        Err(Error::CanonicalStoragePoisoned)
    ));
    let retained_path = kura.retained_block_record_path(2);
    let retained_bytes = std::fs::read(&retained_path).expect("read retained wire binding");
    let mut input = retained_bytes.as_slice();
    let mut retained =
        KuraRetainedBlockRecord::decode_all(&mut input).expect("decode retained wire binding");
    retained.executed_block_wire_hash = Hash::new(b"hostile compaction retained executed wire");
    std::fs::write(&retained_path, retained.encode())
        .expect("tamper retained wire binding before restart");
    let stage = primary_blocks_dir(&temp_dir).join(EVICTION_COMPACTION_STAGE_FILE_NAME);
    drop(kura);
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::V2FinalityExecutedBlockWireHashMismatch { height: 2 })
    ));
    assert!(
        stage.exists(),
        "rejected recovery must retain the durable compaction decision for diagnosis"
    );
}
#[cfg(unix)]
#[test]
fn eviction_compaction_restart_rejects_hardlinked_stage_record() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (config, kura, _) = open_eviction_compaction_fixture(&temp_dir, 4);
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.block_store
        .lock()
        .crash_next_eviction_after_stage
        .store(true, Ordering::Release);
    assert!(matches!(
        kura.evict_block_bodies(payload_len),
        Err(Error::CanonicalStoragePoisoned)
    ));
    let blocks_dir = primary_blocks_dir(&temp_dir);
    let stage = blocks_dir.join(EVICTION_COMPACTION_STAGE_FILE_NAME);
    let alias = blocks_dir.join("eviction-compaction-stage.attacker-link");
    std::fs::hard_link(&stage, &alias).expect("create attacker-controlled stage hardlink");
    drop(kura);
    let error = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect_err("hard-linked compaction stage must fail closed");
    assert!(
        error.to_string().contains("single-link regular file"),
        "unexpected recovery error: {error}"
    );
    assert!(stage.exists(), "rejected stage record must remain in place");
}
#[test]
fn eviction_compaction_restart_removes_unpublished_orphan_replacements() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    populate_store(&temp_dir, 2);
    let blocks_dir = primary_blocks_dir(&temp_dir);
    let data_orphan = blocks_dir.join(EVICTION_COMPACTION_DATA_FILE_NAME);
    let index_orphan = blocks_dir.join(EVICTION_COMPACTION_INDEX_FILE_NAME);
    std::fs::write(&data_orphan, b"unpublished replacement data")
        .expect("write orphan data replacement");
    std::fs::write(&index_orphan, b"unpublished replacement index")
        .expect("write orphan index replacement");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (_kura, count) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("open and clean Kura");
    assert_eq!(count.0, 2);
    assert!(!data_orphan.exists());
    assert!(!index_orphan.exists());
}
#[test]
fn eviction_compaction_does_not_promote_after_stage_dirsync_failure() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (config, kura, _) = open_eviction_compaction_fixture(&temp_dir, 4);
    let blocks_dir = primary_blocks_dir(&temp_dir);
    let data_path = blocks_dir.join(DATA_FILE_NAME);
    let index_path = blocks_dir.join(INDEX_FILE_NAME);
    let data_before = std::fs::read(&data_path).expect("read original data file");
    let index_before = std::fs::read(&index_path).expect("read original index file");
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.block_store
        .lock()
        .fail_eviction_stage_syncs_remaining
        .store(2, Ordering::Release);
    assert!(matches!(
        kura.evict_block_bodies(payload_len),
        Err(Error::CanonicalStoragePoisoned)
    ));
    assert_eq!(std::fs::read(&data_path).unwrap(), data_before);
    assert_eq!(std::fs::read(&index_path).unwrap(), index_before);
    assert!(
        blocks_dir
            .join(EVICTION_COMPACTION_STAGE_FILE_NAME)
            .exists(),
        "the visible but unacknowledged decision record remains until crash loss"
    );
    drop(kura);
    std::fs::remove_file(blocks_dir.join(EVICTION_COMPACTION_STAGE_FILE_NAME))
        .expect("simulate loss of the directory-unsynchronized stage after a crash");
    let (reopened, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("reopen original store");
    assert!(
        !reopened
            .block_store
            .lock()
            .read_block_index(1)
            .expect("original index")
            .is_evicted()
    );
}
#[test]
fn eviction_compaction_preserves_remote_only_prior_body() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (config, kura, blocks) = open_eviction_compaction_fixture(&temp_dir, 5);
    let prior_hash = blocks[1].hash();
    let (_, first_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    assert_eq!(
        kura.evict_block_bodies(first_len)
            .expect("evict first canonical body"),
        first_len
    );
    kura.remove_evicted_block_sidecar_for_testing(nonzero!(2_usize))
        .expect("make prior eviction remote-only");
    let (_, second_len) = advertise_required_replicas(&kura, nonzero!(3_usize));
    assert_eq!(
        kura.evict_block_bodies(second_len)
            .expect("compact around remote-only history"),
        second_len
    );
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(2_usize)),
        Some(prior_hash)
    );
    assert!(kura.get_block(nonzero!(2_usize)).is_none());
    drop(kura);
    let (reopened, count) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("reopen compacted history");
    assert_eq!(count.0, 5);
    assert_eq!(
        reopened.get_durable_block_hash(nonzero!(2_usize)),
        Some(prior_hash)
    );
}
#[test]
fn eviction_compaction_preserves_verified_hash_only_tail() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (config, kura, blocks) = open_eviction_compaction_fixture(&temp_dir, 4);
    let mut snapshot_hashes = blocks.iter().map(|block| block.hash()).collect::<Vec<_>>();
    let tail_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA7; Hash::LENGTH]));
    snapshot_hashes.push(tail_hash);
    assert_eq!(
        kura.extend_hash_only_suffix_from_verified_snapshot(&snapshot_hashes)
            .expect("append authenticated hash-only tail"),
        1
    );
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    assert_eq!(
        kura.evict_block_bodies(payload_len)
            .expect("compact with hash-only tail"),
        payload_len
    );
    assert_eq!(
        kura.block_store
            .lock()
            .read_block_index(4)
            .expect("hash-only index"),
        (EVICTED_BLOCK_START, 0)
    );
    drop(kura);
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::InvalidSnapshotBootstrapMarker { .. })
    ));
    let (reopened, count) = Kura::new_inner(
        &config,
        &RuntimeLaneConfig::default(),
        None,
        Some(5),
        false,
        PendingControlSidecarLimits::default(),
    )
    .expect("open hash-only history provisionally for signed-lineage reauthentication");
    assert_eq!(count.0, 5);
    assert!(reopened.provisional_snapshot_bootstrap_pending());
    assert_eq!(
        reopened.block_hash_at_height(nonzero!(5_usize)),
        Some(tail_hash)
    );
    assert!(reopened.get_block(nonzero!(5_usize)).is_none());
}
#[test]
fn eviction_rejects_oversized_index_before_allocation() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (_, kura, _) = open_eviction_compaction_fixture(&temp_dir, 4);
    let oversized = STRICT_INIT_MAX_BLOCK_BYTES.saturating_add(1);
    {
        let mut store = kura.block_store.lock();
        let original = store.read_block_index(1).expect("original index");
        store
            .write_block_index(1, original.start, oversized)
            .expect("inject oversized index");
    }
    let (hash, _) = advertised_block_metadata(&kura, nonzero!(2_usize));
    for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
        kura.record_block_replica_advert(checked_peer_id(), 2, hash, oversized);
    }
    assert!(matches!(
        kura.evict_block_bodies(oversized),
        Err(Error::CorruptedBlockLength { length, limit })
            if length == oversized && limit == STRICT_INIT_MAX_BLOCK_BYTES
    ));
    let blocks_dir = primary_blocks_dir(&temp_dir);
    assert!(
        !blocks_dir
            .join(EVICTION_COMPACTION_STAGE_FILE_NAME)
            .exists()
    );
    assert!(!blocks_dir.join(EVICTION_COMPACTION_DATA_FILE_NAME).exists());
    assert!(
        !blocks_dir
            .join(EVICTION_COMPACTION_INDEX_FILE_NAME)
            .exists()
    );
}
#[test]
fn eviction_digest_is_independent_of_short_reads() {
    struct ShortReader<R> {
        inner: R,
        max: usize,
    }
    impl<R: Read> Read for ShortReader<R> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let limit = buffer.len().min(self.max);
            self.inner.read(&mut buffer[..limit])
        }
    }
    let bytes = (0_u32..200_000)
        .map(|value| value.wrapping_mul(31) as u8)
        .collect::<Vec<_>>();
    let total = u64::try_from(bytes.len()).unwrap();
    let expected =
        BlockStore::eviction_reader_digest(&mut std::io::Cursor::new(bytes.clone()), total)
            .expect("digest ordinary reader");
    let actual = BlockStore::eviction_reader_digest(
        &mut ShortReader {
            inner: std::io::Cursor::new(bytes),
            max: 7,
        },
        total,
    )
    .expect("digest short reader");
    assert_eq!(actual, expected);
}
#[test]
fn eviction_prior_cache_wire_must_match_retained_record() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (_, kura, _) = open_eviction_compaction_fixture(&temp_dir, 5);
    let (_, first_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.evict_block_bodies(first_len)
        .expect("evict first body");
    let cache_path = kura.block_store.lock().da_block_path(2);
    let mut tampered = std::fs::read(&cache_path).expect("read canonical DA cache");
    *tampered.last_mut().expect("non-empty canonical cache") ^= 0x80;
    std::fs::write(&cache_path, &tampered).expect("tamper complete block wire");
    kura.block_data.lock()[1].1 = None;
    assert!(
        kura.get_block(nonzero!(2_usize)).is_none(),
        "a header-preserving complete-wire substitution must be a cache miss"
    );
    let (_, second_len) = advertise_required_replicas(&kura, nonzero!(3_usize));
    assert_eq!(
        kura.evict_block_bodies(second_len)
            .expect("later compaction treats malformed prior cache as a miss"),
        second_len
    );
    assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
}
#[test]
fn eviction_accounting_handles_sidecar_shrink() {
    let temp_dir = TempDir::new().expect("create temp Kura directory");
    let (_, kura, blocks) = open_eviction_compaction_fixture(&temp_dir, 4);
    let canonical = blocks[1]
        .canonical_wire()
        .expect("canonical block wire")
        .into_parts()
        .0;
    let mut oversized = canonical.clone();
    oversized.extend(std::iter::repeat_n(0xA5, 4096));
    let cache_path = {
        let store = kura.block_store.lock();
        store
            .ensure_da_blocks_dir()
            .expect("create authenticated DA cache directory");
        store.da_block_path(2)
    };
    std::fs::write(&cache_path, &oversized)
        .expect("inject an oversized pre-existing corrupt sidecar");
    let _ = kura
        .refresh_total_disk_usage_bytes()
        .expect("refresh baseline total usage");
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    assert_eq!(
        kura.evict_block_bodies(payload_len)
            .expect("replace stale sidecar with canonical bytes"),
        payload_len
    );
    let snapshot = kura
        .disk_usage_accounting_snapshot_for_tests()
        .expect("read exact accounting snapshot");
    assert!(snapshot.total_initialized);
    assert_eq!(snapshot.cached_total_bytes, snapshot.exact_total_bytes);
    assert_eq!(
        std::fs::read(kura.block_store.lock().da_block_path(2)).unwrap(),
        canonical
    );
}
#[test]
fn merge_reads_reject_canonical_storage_poison() {
    let kura = Kura::blank_kura_for_testing();
    let entry = sample_merge_entry(1);
    kura.merge_log
        .lock()
        .append(&entry)
        .expect("append test merge entry");
    assert_eq!(kura.merge_ledger_snapshot(), vec![entry.clone()]);
    let poison = Error::CanonicalStoragePoisoned;
    kura.poison_canonical_storage("test canonical poison", &poison);
    assert!(kura.merge_ledger_snapshot().is_empty());
    assert!(matches!(
        kura.merge_ledger_all_entries(),
        Err(Error::CanonicalStoragePoisoned)
    ));
    assert!(matches!(
        kura.merge_entry_by_hash(entry.canonical_hash()),
        Err(Error::CanonicalStoragePoisoned)
    ));
}
#[test]
fn canonical_bind_before_poison_closes_the_consensus_guard_immediately() {
    let unbound = Kura::blank_kura_for_testing();
    let unrelated_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    assert!(unbound.ensure_canonical_storage_not_poisoned().is_ok());
    assert!(unrelated_guard.acquire().is_some());
    let kura = Kura::blank_kura_for_testing();
    let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    kura.bind_consensus_output_guard(Arc::clone(&output_guard))
        .expect("bind authoritative output guard");
    assert!(output_guard.acquire().is_some());
    let poison = Error::CanonicalStoragePoisoned;
    kura.poison_canonical_storage("injected canonical poison", &poison);
    assert!(output_guard.restart_required());
    assert!(
        output_guard.acquire().is_none(),
        "Kura poison must close consensus admission before returning"
    );
    kura.poison_canonical_storage("duplicate canonical poison", &poison);
    assert!(output_guard.acquire().is_none());
    assert!(matches!(
        kura.bind_consensus_output_guard(
            crate::sumeragi::output_guard::ConsensusOutputGuard::isolated()
        ),
        Err(Error::ConsensusOutputGuardAlreadyBound)
    ));
}
#[test]
fn canonical_poison_before_bind_closes_the_new_consensus_guard() {
    let kura = Kura::blank_kura_for_testing();
    let poison = Error::CanonicalStoragePoisoned;
    kura.poison_canonical_storage("poison before guard binding", &poison);
    let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    assert!(output_guard.acquire().is_some());
    kura.bind_consensus_output_guard(Arc::clone(&output_guard))
        .expect("bind authoritative output guard after poison");
    assert!(output_guard.restart_required());
    assert!(
        output_guard.acquire().is_none(),
        "binding after poison must not return with consensus admission open"
    );
}
#[test]
fn canonical_poison_bind_interleaving_cannot_leave_admission_open() {
    let kura = Kura::blank_kura_for_testing();
    kura.pause_canonical_poison_after_latch
        .store(true, Ordering::Release);
    let poison_kura = Arc::clone(&kura);
    let poisoner = thread::spawn(move || {
        poison_kura.poison_canonical_storage(
            "poison racing guard binding",
            &Error::CanonicalStoragePoisoned,
        );
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura
        .canonical_poison_paused_after_latch
        .load(Ordering::Acquire)
    {
        assert!(
            Instant::now() < deadline,
            "canonical poison did not reach the post-latch race barrier"
        );
        thread::yield_now();
    }
    assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert!(kura.consensus_output_guard.get().is_none());
    let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let bind_result = kura.bind_consensus_output_guard(Arc::clone(&output_guard));
    let restart_required_before_poison_resumes = output_guard.restart_required();
    let admission_closed_before_poison_resumes = output_guard.acquire().is_none();
    kura.canonical_poison_paused_after_latch
        .store(false, Ordering::Release);
    poisoner.join().expect("canonical poison thread completes");
    bind_result.expect("bind while poison is paused before guard lookup");
    assert!(restart_required_before_poison_resumes);
    assert!(
        admission_closed_before_poison_resumes,
        "the bind-side latch recheck must close admission before returning"
    );
    assert!(output_guard.acquire().is_none());
}
