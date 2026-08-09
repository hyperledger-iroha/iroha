
#[test]
fn finalized_remote_only_block_retains_header_across_restart() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, nonzero!(1_usize));
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
    let mut generator = DummyBlocks::new();
    let blocks = (0..4).map(|_| generator.next()).collect::<Vec<_>>();
    for block in &blocks {
        kura.store_block(Arc::clone(block))
            .expect("store canonical block");
    }
    let height = nonzero!(2_usize);
    let expected_header = blocks[1].header();
    let artifacts = v2_finality_artifacts_for_chain(&blocks[..2]);
    let artifact = artifacts[1].clone();
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality with retained header");

    let (_, payload_len) = advertise_required_replicas(&kura, height);
    assert!(
        kura.evict_block_bodies(payload_len)
            .expect("evict finalized body")
            >= payload_len
    );
    {
        let store = kura.block_store.lock();
        store
            .remove_da_block_file(height.get() as u64)
            .expect("remove local DA cache to make the body remote-only");
    }
    assert!(kura.get_block(height).is_none());
    drop(kura);

    let (reopened, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("reopen remote-only Kura");
    assert!(reopened.get_block(height).is_none());
    let finality_height = u64::try_from(height.get()).expect("height fits u64");
    let (retained_header, recovered) = reopened
        .v2_finality_artifact_with_header(finality_height)
        .expect("read retained finality record")
        .expect("finality record exists");
    assert_eq!(retained_header, expected_header);
    assert_eq!(recovered, artifact);
    let _receipt = reopened
        .store_v2_finality_artifact(&artifact)
        .expect("idempotent persistence must not require the evicted body");
}

#[test]
fn eviction_requires_signed_complete_wire_finality_before_retaining_or_removing_body() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, nonzero!(1_usize));
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
    store_dummy_block_arcs(&kura, 4);
    let height = nonzero!(2_usize);
    let (_, payload_len) = advertise_unfinalized_required_replicas(&kura, height);

    assert_eq!(
        kura.evict_block_bodies(payload_len)
            .expect("deny an unfinalized canonical block"),
        0
    );
    assert!(
        !kura.v2_finality_artifact_path(2).exists(),
        "denied eviction must not manufacture consensus finality"
    );
    assert!(
        !kura.retained_block_record_path(2).exists(),
        "denied eviction must not publish an unsigned retained binding"
    );
    assert!(
        !kura
            .block_store
            .lock()
            .read_block_index(1)
            .expect("read denied eviction index")
            .is_evicted(),
        "unfinalized canonical body must remain inline"
    );
}

#[test]
fn eviction_scans_past_an_unfinalized_advertised_height() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, nonzero!(1_usize));
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
    let blocks = store_dummy_block_arcs(&kura, 5);
    let finalized_height = nonzero!(3_usize);
    let artifact = v2_finality_artifacts_for_chain(&blocks[..3])
        .pop()
        .expect("height-three finality artifact");
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist only height-three complete-wire finality");
    let (_, height_two_len) = advertise_unfinalized_required_replicas(&kura, nonzero!(2_usize));
    let (_, height_three_len) = advertise_unfinalized_required_replicas(&kura, finalized_height);

    assert_eq!(
        kura.evict_block_bodies(height_three_len)
            .expect("evict the later finalized candidate"),
        height_three_len
    );
    let mut store = kura.block_store.lock();
    let height_two = store.read_block_index(1).expect("height-two index");
    let height_three = store.read_block_index(2).expect("height-three index");
    assert_eq!(height_two.length, height_two_len);
    assert!(
        !height_two.is_evicted(),
        "an advertised but unfinalized earlier height must remain inline"
    );
    assert!(
        height_three.is_evicted(),
        "an unfinalized earlier candidate must not starve a later finalized candidate"
    );
}

#[test]
fn failed_top_replacement_restores_staged_retained_record_and_exact_accounting() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
    let original = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&original))
        .expect("store original top block");
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    kura.persist_retained_block_record(&blocks_dir, original.hash(), original.as_ref())
        .expect("persist original retained record");
    let retained_path = kura.retained_block_record_path(1);
    let retained_before = std::fs::read(&retained_path).expect("read retained record");
    let total_before = kura
        .refresh_total_disk_usage_bytes()
        .expect("measure total usage before failed replacement");
    let replacement: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into(),
    );
    assert_ne!(replacement.hash(), original.hash());
    kura.fail_next_block_write_for_tests();

    assert!(kura.replace_top_block(Arc::clone(&replacement)).is_err());
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(original.hash())
    );
    assert_eq!(
        std::fs::read(&retained_path).expect("retained record restored after failed rewrite"),
        retained_before
    );
    assert!(
        !Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).exists(),
        "ordinary failure must resolve its rewrite stage"
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("cached total usage after failed replacement"),
        total_before
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("rescan total usage after failed replacement"),
        total_before
    );
}

#[test]
fn successful_top_replacement_discards_staged_record_with_exact_accounting() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
    let original = DummyBlocks::new().next();
    kura.store_block(Arc::clone(&original))
        .expect("store original top block");
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    kura.persist_retained_block_record(&blocks_dir, original.hash(), original.as_ref())
        .expect("persist original retained record");
    let replacement: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(1_u64));
            header.set_prev_block_hash(None);
            header.set_view_change_index(header.view_change_index().saturating_add(1));
        })
        .into(),
    );

    kura.replace_top_block(Arc::clone(&replacement))
        .expect("replace unfinalized canonical top");
    assert_eq!(
        kura.get_durable_block_hash(nonzero!(1_usize)),
        Some(replacement.hash())
    );
    assert!(!kura.retained_block_record_path(1).exists());
    assert!(!Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).exists());
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("cached total usage after successful replacement"),
        kura.kura_total_disk_usage_bytes()
            .expect("exact total usage after successful replacement")
    );
}

#[test]
fn retained_rewrite_stage_restores_old_canonical_record_after_restart() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let retained_before = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let block = DummyBlocks::new().next();
        kura.store_block(Arc::clone(&block))
            .expect("store canonical block");
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        kura.persist_retained_block_record(&blocks_dir, block.hash(), block.as_ref())
            .expect("persist retained record");
        let retained_before =
            std::fs::read(kura.retained_block_record_path(1)).expect("read retained record");
        let _canonical_guard = kura.canonical_chain_lock.lock();
        let stage = kura
            .stage_retained_block_records_for_rewrite(&blocks_dir, 1)
            .expect("stage retained record")
            .expect("staged record exists");
        assert!(!kura.retained_block_record_path(1).exists());
        assert!(Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).is_dir());
        drop(stage);
        retained_before
    };

    let (reopened, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("recover staged rewrite");
    assert_eq!(
        std::fs::read(reopened.retained_block_record_path(1))
            .expect("startup restored retained record for unchanged canonical hash"),
        retained_before
    );
    assert!(
        !Kura::retained_block_rewrite_staging_dir_for(&reopened.active_blocks_dir.lock().clone())
            .exists()
    );
    assert_eq!(
        reopened
            .disk_usage_bytes()
            .expect("cached usage after staged-restore restart"),
        reopened
            .kura_total_disk_usage_bytes()
            .expect("exact usage after staged-restore restart")
    );
}

#[test]
fn staged_v2_record_accepts_exact_published_v3_upgrade() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (current_bytes, retained_path) = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let mut generator = DummyBlocks::new();
        let genesis = generator.next();
        let mut entry = sample_merge_entry(1);
        let carrier = next_merge_carrier(&mut generator, &mut entry);
        kura.store_block(genesis).expect("store carrier parent");
        kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
            .expect("store merge carrier");
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        let current =
            Kura::prepare_retained_block_record(&blocks_dir, carrier.hash(), carrier.as_ref())
                .expect("prepare current retained record");
        assert!(current.merge_reference.is_some());
        let mut legacy_projection = current.clone();
        legacy_projection.format_version = RETAINED_BLOCK_RECORD_VERSION_V2;
        legacy_projection.executed_block_wire_len = 0;
        legacy_projection.merge_reference = None;
        let legacy = KuraRetainedBlockRecordV2::from_current(&legacy_projection)
            .expect("construct exact legacy retained layout");
        let legacy_bytes = legacy.encode();
        let current_bytes = current.encode();
        let retained_directory = kura.retained_block_record_dir();
        let retained_path = kura.retained_block_record_path(2);
        std::fs::create_dir_all(&retained_directory).expect("create retained-record directory");

        // Exercise the same-process error-reconciliation path first.
        std::fs::write(&retained_path, &legacy_bytes).expect("write legacy retained record");
        kura.refresh_total_disk_usage_bytes()
            .expect("initialize accounting for legacy record");
        {
            let _canonical_guard = kura.canonical_chain_lock.lock();
            let stage = kura
                .stage_retained_block_records_for_rewrite(&blocks_dir, 2)
                .expect("stage legacy retained record")
                .expect("legacy retained record exists");
            std::fs::write(&retained_path, &current_bytes)
                .expect("publish exact current retained record");
            kura.refresh_total_disk_usage_bytes()
                .expect("include concurrently published current record");
            kura.reconcile_staged_retained_block_rewrite_after_error(&stage)
                .expect("accept exact legacy-to-current upgrade");
        }
        assert_eq!(
            std::fs::read(&retained_path).expect("read reconciled retained record"),
            current_bytes
        );
        assert!(!Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).exists());
        assert_eq!(
            kura.disk_usage_bytes()
                .expect("cached usage after exact upgrade reconciliation"),
            kura.kura_total_disk_usage_bytes()
                .expect("exact usage after exact upgrade reconciliation")
        );

        // Recreate the publication interleaving and leave it for startup
        // recovery, which must make the same monotonic-upgrade decision.
        std::fs::write(&retained_path, &legacy_bytes)
            .expect("restore legacy retained record for restart case");
        kura.refresh_total_disk_usage_bytes()
            .expect("refresh accounting before restart case");
        {
            let _canonical_guard = kura.canonical_chain_lock.lock();
            let stage = kura
                .stage_retained_block_records_for_rewrite(&blocks_dir, 2)
                .expect("stage legacy retained record for restart")
                .expect("legacy retained record exists for restart");
            std::fs::write(&retained_path, &current_bytes)
                .expect("publish exact current record before restart");
            drop(stage);
        }
        assert!(Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).is_dir());
        (current_bytes, retained_path)
    };

    let (reopened, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("recover exact retained-record upgrade on startup");
    assert_eq!(
        std::fs::read(&retained_path).expect("read startup-recovered retained record"),
        current_bytes
    );
    assert!(
        !Kura::retained_block_rewrite_staging_dir_for(&reopened.active_blocks_dir.lock().clone())
            .exists()
    );
    assert_eq!(
        reopened
            .disk_usage_bytes()
            .expect("cached usage after startup exact-upgrade recovery"),
        reopened
            .kura_total_disk_usage_bytes()
            .expect("exact usage after startup exact-upgrade recovery")
    );
}

#[test]
fn ambiguous_old_marker_keeps_retained_and_finality_evidence_for_startup() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (retained_before, finality_before, original_hash, artifact) = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let original = DummyBlocks::new().next();
        kura.store_block(Arc::clone(&original))
            .expect("store original block");
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        kura.persist_retained_block_record(&blocks_dir, original.hash(), original.as_ref())
            .expect("persist original retained record");
        let artifact = v2_finality_artifact_for_block(&original);
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist original finality record");
        let retained_path = kura.retained_block_record_path(1);
        let finality_path = kura.v2_finality_artifact_path(1);
        let retained_before = std::fs::read(&retained_path).expect("read retained record");
        let finality_before = std::fs::read(&finality_path).expect("read finality record");
        let replacement: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(1_u64));
                header.set_prev_block_hash(None);
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into(),
        );

        let _canonical_guard = kura.canonical_chain_lock.lock();
        let retained_stage = kura
            .stage_retained_block_records_for_rewrite(&blocks_dir, 1)
            .expect("stage retained record")
            .expect("retained record exists");
        {
            let store = kura.block_store.lock();
            store
                .fail_next_commit_marker_write_and_readback
                .store(true, Ordering::Release);
        }
        assert!(matches!(
            kura.persist_block_at_height(&replacement, 1),
            Err(Error::DaBlockRewriteCommitStateUnknown { .. })
        ));
        assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
        assert!(Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).is_dir());
        assert!(
            kura.block_store
                .lock()
                .da_block_rewrite_stage_path()
                .is_file()
        );
        assert_eq!(
            std::fs::read(&finality_path).expect("finality survives ambiguity"),
            finality_before
        );
        drop(retained_stage);
        (retained_before, finality_before, original.hash(), artifact)
    };

    let (reopened, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("startup resolves DA before retained and finality evidence");
    assert_eq!(
        reopened.get_durable_block_hash(nonzero!(1_usize)),
        Some(original_hash)
    );
    assert_eq!(
        std::fs::read(reopened.retained_block_record_path(1))
            .expect("retained record restored after old-marker recovery"),
        retained_before
    );
    assert_eq!(
        std::fs::read(reopened.v2_finality_artifact_path(1))
            .expect("finality record preserved after old-marker recovery"),
        finality_before
    );
    assert_eq!(
        reopened.v2_finality_artifact(1).expect("read finality"),
        Some(artifact)
    );
}

#[test]
fn retained_rewrite_stage_discards_old_record_after_published_crash() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let replacement_hash = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let original = DummyBlocks::new().next();
        kura.store_block(Arc::clone(&original))
            .expect("store original block");
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        kura.persist_retained_block_record(&blocks_dir, original.hash(), original.as_ref())
            .expect("persist original retained record");
        let replacement: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(1_u64));
                header.set_prev_block_hash(None);
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into(),
        );
        let replacement_hash = replacement.hash();
        let _canonical_guard = kura.canonical_chain_lock.lock();
        let stage = kura
            .stage_retained_block_records_for_rewrite(&blocks_dir, 1)
            .expect("stage original retained record")
            .expect("staged record exists");
        kura.persist_block_at_height(&replacement, 1)
            .expect("publish replacement before simulated crash");
        assert!(Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).is_dir());
        drop(stage);
        replacement_hash
    };

    let (reopened, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("restart after replacement publication crash");
    assert_eq!(
        reopened.get_durable_block_hash(nonzero!(1_usize)),
        Some(replacement_hash)
    );
    assert!(
        !reopened.retained_block_record_path(1).exists(),
        "startup must discard retained evidence for the replaced canonical hash"
    );
    assert!(
        !Kura::retained_block_rewrite_staging_dir_for(&reopened.active_blocks_dir.lock().clone())
            .exists()
    );
    assert_eq!(
        reopened
            .disk_usage_bytes()
            .expect("cached usage after staged-discard restart"),
        reopened
            .kura_total_disk_usage_bytes()
            .expect("exact usage after staged-discard restart")
    );
}

#[test]
fn post_publication_rewrite_error_discards_old_evidence_and_restarts_cleanly() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let replacement_hash = {
        let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
        let original = DummyBlocks::new().next();
        kura.store_block(Arc::clone(&original))
            .expect("store original block");
        let blocks_dir = kura.active_blocks_dir.lock().clone();
        kura.persist_retained_block_record(&blocks_dir, original.hash(), original.as_ref())
            .expect("persist original retained record");
        let replacement: Arc<SignedBlock> = Arc::new(
            ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
                header.set_height(nonzero!(1_u64));
                header.set_prev_block_hash(None);
                header.set_view_change_index(header.view_change_index().saturating_add(1));
            })
            .into(),
        );
        let replacement_hash = replacement.hash();
        let result = {
            let _canonical_guard = kura.canonical_chain_lock.lock();
            kura.with_retained_block_records_staged_for_rewrite(&blocks_dir, 1, || {
                kura.persist_block_at_height(&replacement, 1)?;
                Err::<(), _>(Error::IO(
                    std::io::Error::other("injected post-publication failure"),
                    PathBuf::from("post_publication_rewrite_test"),
                ))
            })
        };
        assert!(matches!(
            result,
            Err(Error::IO(error, _)) if error.to_string().contains("post-publication")
        ));
        assert_eq!(
            kura.get_durable_block_hash(nonzero!(1_usize)),
            Some(replacement_hash)
        );
        assert!(
            !kura.retained_block_record_path(1).exists(),
            "old retained evidence must not be restored over a published replacement"
        );
        assert!(
            !Kura::retained_block_rewrite_staging_dir_for(&blocks_dir).exists(),
            "post-publication error reconciliation must resolve the stage"
        );
        assert_eq!(
            kura.disk_usage_bytes()
                .expect("cached usage after post-publication reconciliation"),
            kura.kura_total_disk_usage_bytes()
                .expect("exact usage after post-publication reconciliation")
        );
        replacement_hash
    };

    let (reopened, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("restart after post-publication rewrite error");
    assert_eq!(
        reopened.get_durable_block_hash(nonzero!(1_usize)),
        Some(replacement_hash)
    );
    assert!(!reopened.retained_block_record_path(1).exists());
}
