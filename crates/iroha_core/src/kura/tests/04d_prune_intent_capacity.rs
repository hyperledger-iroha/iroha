fn canonical_prune_intent_artifact_fixture() -> KuraPruneIntentV3 {
    seal_prune_intent_fixture(KuraPruneIntentV3 {
        version: 3,
        source_height: 2,
        source_tip_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"canonical prune source tip",
        ))),
        target_height: 1,
        target_tip_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"canonical prune target tip",
        ))),
        retained_merge_entries: 0,
        retained_merge_tip_hash: None,
        sidecar_rewrite: KuraPruneSidecarRewriteProjectionV3::none(),
        capacity: unsealed_prune_capacity_fixture(),
    })
}
fn canonical_prune_intent_artifact_bytes() -> Vec<u8> {
    norito::encode_canonical(&canonical_prune_intent_artifact_fixture())
        .expect("encode canonical prune-intent fixture")
}
#[test]
fn canonical_prune_intent_scanners_account_stable_temp_crash_inode_once() {
    let temp_dir = TempDir::new().expect("prune accounting temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("prune accounting Kura");
    let baseline_enforced = kura
        .kura_disk_usage_bytes()
        .expect("scan baseline enforced bytes");
    let baseline_total = kura
        .kura_total_disk_usage_bytes()
        .expect("scan baseline total bytes");
    let bytes = canonical_prune_intent_artifact_bytes();
    let bytes_len = u64::try_from(bytes.len()).expect("intent fixture length fits u64");
    let stable_path = Kura::prune_intent_path_for(temp_dir.path());
    let temporary_path = Kura::prune_intent_temp_path_for(temp_dir.path());
    fs::write(&temporary_path, &bytes).expect("write exact deterministic prune temp");
    fs::hard_link(&temporary_path, &stable_path)
        .expect("construct portable no-clobber crash window");
    let inventory = Kura::canonical_prune_intent_artifact_inventory(temp_dir.path())
        .expect("authenticate stable+temp crash inventory");
    assert!(inventory.stable.is_some());
    assert!(inventory.temporary.is_some());
    assert_eq!(
        inventory.tracked_bytes().expect("account crash inventory"),
        bytes_len,
        "two authenticated names for one inode consume the maintenance reserve once",
    );
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("scan enforced crash-window bytes"),
        baseline_enforced + bytes_len,
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("scan total crash-window bytes"),
        baseline_total + bytes_len,
    );
    assert_eq!(
        Kura::read_prune_intent(temp_dir.path()).expect("normalize crash-window publication"),
        Some(canonical_prune_intent_artifact_fixture()),
    );
    assert!(stable_path.is_file());
    assert!(!temporary_path.exists());
    assert_eq!(
        Kura::canonical_prune_intent_artifact_inventory(temp_dir.path())
            .expect("authenticate normalized stable intent")
            .tracked_bytes()
            .expect("account normalized stable intent"),
        bytes_len,
    );
    kura.refresh_disk_usage_bytes()
        .expect("publish normalized live accounting");
    kura.clear_prune_intent()
        .expect("descriptor-authenticate and clear exact stable intent");
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("scan enforced bytes after clear"),
        baseline_enforced,
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("scan total bytes after clear"),
        baseline_total,
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("read live total accounting after clear"),
        baseline_total,
    );
}
#[test]
fn canonical_prune_publication_updates_both_live_usage_counters() {
    let temp_dir = TempDir::new().expect("prune live-accounting temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("prune live-accounting Kura");
    let intent = canonical_prune_intent_artifact_fixture();
    let intent_len = u64::try_from(canonical_prune_intent_artifact_bytes().len())
        .expect("prune intent fixture length fits u64");
    let baseline_enforced = kura
        .refresh_disk_usage_bytes()
        .expect("refresh prune live-accounting baseline");
    let baseline_total = kura
        .disk_usage_bytes()
        .expect("read prune total-accounting baseline");
    kura.persist_prune_intent(&intent)
        .expect("publish canonical prune intent");
    assert_eq!(
        kura.disk_usage.load(Ordering::Relaxed),
        baseline_enforced + intent_len,
        "stable publication must increment the enforced live counter",
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("read total accounting after prune publication"),
        baseline_total + intent_len,
        "stable publication must increment the total live counter",
    );
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("scan enforced bytes after prune publication"),
        baseline_enforced + intent_len,
    );
    assert_eq!(
        kura.kura_total_disk_usage_bytes()
            .expect("scan total bytes after prune publication"),
        baseline_total + intent_len,
    );
    kura.finish_prune_intent()
        .expect("clear canonical prune intent and recovery latch");
    assert_eq!(kura.disk_usage.load(Ordering::Relaxed), baseline_enforced);
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("read total accounting after prune clearance"),
        baseline_total,
    );
}
#[test]
fn canonical_prune_intent_exact_artifacts_fail_closed_for_every_untrusted_shape() {
    let temp_dir = TempDir::new().expect("prune artifact adversarial temp dir");
    let root = temp_dir.path();
    let stable_path = Kura::prune_intent_path_for(root);
    let temporary_path = Kura::prune_intent_temp_path_for(root);
    let bytes = canonical_prune_intent_artifact_bytes();
    fs::write(&stable_path, b"not canonical Norito").expect("write malformed stable intent");
    assert!(Kura::canonical_prune_intent_artifact_inventory(root).is_err());
    fs::remove_file(&stable_path).expect("remove malformed stable intent");
    fs::write(&stable_path, vec![0_u8; PRUNE_INTENT_MAX_BYTES + 1])
        .expect("write oversized stable intent");
    assert!(matches!(
        Kura::canonical_prune_intent_artifact_inventory(root),
        Err(Error::PruneIntentConflict(message)) if message.contains("invalid byte length")
    ));
    fs::remove_file(&stable_path).expect("remove oversized stable intent");
    let unexpected = root.join(format!("{PRUNE_INTENT_FILE_NAME}.bak"));
    fs::write(&unexpected, &bytes).expect("write unexpected reserved-name artifact");
    assert!(matches!(
        Kura::canonical_prune_intent_artifact_inventory(root),
        Err(Error::PruneIntentConflict(message)) if message.contains("unexpected reserved publication name")
    ));
    fs::remove_file(&unexpected).expect("remove unexpected reserved-name artifact");
    let forbidden_random_temp = root.join(format!(
        "{FORBIDDEN_ROOT_ATOMIC_TEMP_PREFIX}unbound-prune-residue"
    ));
    fs::write(&forbidden_random_temp, &bytes).expect("write unbound random prune temp");
    assert!(matches!(
        Kura::canonical_prune_intent_artifact_inventory(root),
        Err(Error::PruneIntentConflict(message)) if message.contains("unexpected reserved publication name")
    ));
    fs::remove_file(&forbidden_random_temp).expect("remove unbound random prune temp");
    let hardlink_source = root.join("prune-hardlink-source");
    fs::write(&hardlink_source, &bytes).expect("write hardlink source");
    fs::hard_link(&hardlink_source, &stable_path).expect("hardlink lone stable artifact");
    assert!(matches!(
        Kura::canonical_prune_intent_artifact_inventory(root),
        Err(Error::PruneIntentConflict(message)) if message.contains("multiply-linked lone stable")
    ));
    fs::remove_file(&stable_path).expect("remove hardlinked stable name");
    fs::remove_file(&hardlink_source).expect("remove hardlink source");
    fs::write(&stable_path, &bytes).expect("write independent stable intent");
    fs::write(&temporary_path, &bytes).expect("write independent temporary intent");
    assert!(matches!(
        Kura::canonical_prune_intent_artifact_inventory(root),
        Err(Error::PruneIntentConflict(message)) if message.contains("not one authenticated two-link publication object")
    ));
    fs::remove_file(&stable_path).expect("remove mismatched stable object");
    fs::remove_file(&temporary_path).expect("remove mismatched temporary object");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let symlink_target = root.join("prune-symlink-target");
        fs::write(&symlink_target, &bytes).expect("write symlink target");
        symlink(&symlink_target, &stable_path).expect("symlink stable prune intent");
        assert!(matches!(
            Kura::canonical_prune_intent_artifact_inventory(root),
            Err(Error::PruneIntentConflict(message)) if message.contains("regular no-follow")
        ));
        fs::remove_file(&stable_path).expect("remove stable symlink");
        fs::remove_file(&symlink_target).expect("remove symlink target");
    }
}
#[test]
fn canonical_prune_publication_consumes_the_exact_reserved_boundary() {
    let temp_dir = TempDir::new().expect("prune reserved-boundary temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("prune boundary Kura");
    let blocks = store_dummy_block_arcs(&kura, 2);
    let preview = admit_prune_intent_fixture(
        &kura,
        KuraPruneIntentV3 {
            version: 3,
            source_height: 2,
            source_tip_hash: Some(blocks[1].hash()),
            target_height: 1,
            target_tip_hash: Some(blocks[0].hash()),
            retained_merge_entries: 0,
            retained_merge_tip_hash: None,
            sidecar_rewrite: KuraPruneSidecarRewriteProjectionV3::none(),
            capacity: unsealed_prune_capacity_fixture(),
        },
    );
    let exact_boundary = preview.capacity.admitted_peak_bytes;
    Arc::get_mut(&mut kura)
        .expect("prune boundary Kura remains exclusive")
        .max_disk_usage_bytes = exact_boundary - 1;
    assert!(matches!(
        kura.prune_to_height(1),
        Err(Error::StorageBudgetExceeded { limit, required, .. })
            if limit == exact_boundary - 1 && required == exact_boundary
    ));
    assert_eq!(kura.blocks_count(), 2);
    assert!(!Kura::prune_intent_path_for(temp_dir.path()).exists());
    Arc::get_mut(&mut kura)
        .expect("prune boundary Kura remains exclusive after rejection")
        .max_disk_usage_bytes = exact_boundary;
    kura.prune_to_height(1)
        .expect("the exact maintenance reserve must admit canonical prune publication");
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(
        kura.get_block(nonzero!(1_usize)).as_deref(),
        Some(blocks[0].as_ref())
    );
    assert!(!Kura::prune_intent_path_for(temp_dir.path()).exists());
    assert!(!Kura::prune_intent_temp_path_for(temp_dir.path()).exists());
}
#[test]
fn canonical_prune_temp_crash_restarts_without_stale_disk_accounting() {
    let temp_dir = TempDir::new().expect("prune temp-crash temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("prune temp-crash Kura");
    let mut generator = DummyBlocks::new();
    let blocks: Vec<_> = (0..3).map(|_| generator.next()).collect();
    for block in &blocks[..2] {
        kura.store_block(Arc::clone(block))
            .expect("store temp-crash canonical fixture block");
    }
    let intent = admit_prune_intent_fixture(
        &kura,
        KuraPruneIntentV3 {
            version: 3,
            source_height: 2,
            source_tip_hash: Some(blocks[1].hash()),
            target_height: 1,
            target_tip_hash: Some(blocks[0].hash()),
            retained_merge_entries: 0,
            retained_merge_tip_hash: None,
            sidecar_rewrite: KuraPruneSidecarRewriteProjectionV3::none(),
            capacity: unsealed_prune_capacity_fixture(),
        },
    );
    let encoded_len = u64::try_from(
        norito::encode_canonical(&intent)
            .expect("encode temp-crash intent")
            .len(),
    )
    .expect("temp-crash intent length fits u64");
    let baseline = kura
        .refresh_disk_usage_bytes()
        .expect("measure temp-crash baseline");
    let temporary_path = Kura::prune_intent_temp_path_for(temp_dir.path());
    let stable_path = Kura::prune_intent_path_for(temp_dir.path());
    kura.fail_next_atomic_write_after_temporary_sync_for_test();
    assert!(kura.persist_prune_intent(&intent).is_err());
    assert!(temporary_path.is_file());
    assert!(!stable_path.exists());
    assert!(matches!(
        kura.store_block(Arc::clone(&blocks[2])),
        Err(Error::PruneRecoveryRequired)
    ));
    assert_eq!(
        kura.blocks_count(),
        2,
        "the unpublished temp crash must latch out in-process canonical mutation",
    );
    assert_eq!(
        kura.refresh_disk_usage_bytes()
            .expect("refresh live accounting after temp crash"),
        baseline + encoded_len,
    );
    assert_eq!(
        kura.disk_usage_bytes()
            .expect("read total accounting after temp crash"),
        kura.kura_total_disk_usage_bytes()
            .expect("scan total accounting after temp crash"),
    );
    drop(kura);
    let (reopened, BlockCount(block_count)) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("startup removes the authenticated unpublished prune temp");
    assert_eq!(block_count, 2);
    assert!(!temporary_path.exists());
    assert!(!stable_path.exists());
    assert_eq!(
        reopened
            .refresh_disk_usage_bytes()
            .expect("refresh accounting after startup cleanup"),
        reopened
            .kura_disk_usage_bytes()
            .expect("scan enforced bytes after startup cleanup"),
    );
    assert_eq!(
        reopened
            .disk_usage_bytes()
            .expect("read total bytes after startup cleanup"),
        reopened
            .kura_total_disk_usage_bytes()
            .expect("scan total bytes after startup cleanup"),
    );
}
#[test]
fn canonical_prune_stable_temp_publication_crash_recovers_forward_on_startup() {
    let temp_dir = TempDir::new().expect("prune publication-crash temp dir");
    let (config, blocks, merge_entries) = populate_prune_recovery_fixture(&temp_dir);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("open publication-crash fixture for exact sidecar projection");
    let sidecar_rewrite = {
        let _guard = kura.sidecar_lock.lock();
        kura.reconcile_and_project_prune_sidecar_rewrites_locked(2)
            .expect("project publication-crash retained sidecars")
    };
    let intent = admit_prune_intent_fixture(
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
    let bytes = norito::encode_canonical(&intent).expect("encode publication-crash prune intent");
    let temporary_path = Kura::prune_intent_temp_path_for(temp_dir.path());
    let stable_path = Kura::prune_intent_path_for(temp_dir.path());
    fs::write(&temporary_path, &bytes).expect("write exact deterministic prune temp");
    fs::hard_link(&temporary_path, &stable_path)
        .expect("construct stable+temp no-clobber publication crash window");
    let inventory = Kura::canonical_prune_intent_artifact_inventory(temp_dir.path())
        .expect("authenticate stable+temp publication crash inventory");
    assert_eq!(
        inventory
            .tracked_bytes()
            .expect("account publication crash inventory"),
        u64::try_from(bytes.len()).expect("publication-crash intent length fits u64"),
    );
    let (recovered, BlockCount(block_count)) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("startup normalizes publication and completes the durable prune");
    assert_eq!(block_count, 2);
    assert_eq!(recovered.blocks_count(), 2);
    assert_eq!(
        recovered.get_block(nonzero!(2_usize)).as_deref(),
        Some(blocks[1].as_ref()),
    );
    assert!(!temporary_path.exists());
    assert!(!stable_path.exists());
    assert_eq!(
        recovered
            .refresh_disk_usage_bytes()
            .expect("refresh accounting after publication-crash recovery"),
        recovered
            .kura_disk_usage_bytes()
            .expect("scan enforced bytes after publication-crash recovery"),
    );
    assert_eq!(
        recovered
            .disk_usage_bytes()
            .expect("read total bytes after publication-crash recovery"),
        recovered
            .kura_total_disk_usage_bytes()
            .expect("scan total bytes after publication-crash recovery"),
    );
}
#[test]
fn active_prune_recovery_never_allocates_missing_retained_merge_carrier() {
    let temp_dir = TempDir::new().expect("missing retained-carrier temp dir");
    let (config, blocks, merge_entries) = populate_prune_recovery_fixture(&temp_dir);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("open missing retained-carrier fixture");
    let sidecar_rewrite = {
        let _guard = kura.sidecar_lock.lock();
        kura.reconcile_and_project_prune_sidecar_rewrites_locked(2)
            .expect("project missing retained-carrier sidecars")
    };
    let intent = admit_prune_intent_fixture(
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
    kura.persist_prune_intent(&intent)
        .expect("persist missing retained-carrier prune intent");
    let retained_carrier = kura.merge_carrier_path(2);
    fs::remove_file(&retained_carrier).expect("remove retained carrier fixture");
    sync_dir(
        retained_carrier
            .parent()
            .expect("retained carrier has parent directory"),
    )
    .expect("sync missing retained-carrier fixture");
    drop(kura);
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::PruneIntentConflict(message))
            if message.contains("cannot allocate missing retained merge carrier")
    ));
    assert!(!retained_carrier.exists());
    assert!(Kura::prune_intent_path_for(temp_dir.path()).is_file());
}
#[derive(Encode)]
struct LegacyKuraPruneSidecarPairProjectionV2Fixture {
    required: bool,
    retained_data_bytes: u64,
    retained_index_bytes: u64,
}
#[derive(Encode)]
struct LegacyKuraPruneSidecarRewriteProjectionV2Fixture {
    pipeline: LegacyKuraPruneSidecarPairProjectionV2Fixture,
    roster: LegacyKuraPruneSidecarPairProjectionV2Fixture,
    sequential_peak_bytes: u64,
}
#[derive(Encode)]
struct RetiredRosterPruneProjectionV2Fixture {
    required: bool,
    current_digest: Option<[u8; 32]>,
    retained_digest: Option<[u8; 32]>,
    retained_payload_bytes: u64,
    generation_allocation_bytes: u64,
    pointer_temporary_bytes: u64,
    current_pointer_growth_bytes: u64,
}
#[derive(Encode)]
struct LegacyKuraPruneCapacityAdmissionV2Fixture {
    source_physical_bytes: u64,
    pending_canonical_bytes: u64,
    post_wsv_reserved_bytes: u64,
    certified_bundle_reserved_bytes: u64,
    autonomous_terminal_reserved_bytes: u64,
    intent_bytes: u64,
    marker_temporary_bytes: u64,
    marker_stable_growth_bytes: u64,
    roster: RetiredRosterPruneProjectionV2Fixture,
    admitted_peak_bytes: u64,
}
#[derive(Encode)]
struct LegacyKuraPruneIntentV2Fixture {
    version: u8,
    source_height: u64,
    source_tip_hash: Option<HashOf<BlockHeader>>,
    target_height: u64,
    target_tip_hash: Option<HashOf<BlockHeader>>,
    retained_merge_entries: u64,
    retained_merge_tip_hash: Option<HashOf<MergeLedgerEntry>>,
    sidecar_rewrite: LegacyKuraPruneSidecarRewriteProjectionV2Fixture,
    capacity: LegacyKuraPruneCapacityAdmissionV2Fixture,
}
fn seed_large_current_tip_sidecar_rewrite(kura: &Kura, tip_hash: HashOf<BlockHeader>) {
    let mut retained = PipelineRecoverySidecar::new(
        1,
        tip_hash,
        PipelineDagSnapshot {
            fingerprint: [0xA1; 32],
            key_count: 1,
        },
        Vec::new(),
    );
    retained.proofs.push(PipelineProofSnapshot {
        backend: "prune-capacity-fixture".to_owned(),
        proof: vec![0x5A; PRUNE_INTENT_MAX_BYTES * 2],
        code_hash: [0xB2; 32],
        tx_hash: None,
    });
    kura.write_pipeline_metadata(&retained);
    kura.write_pipeline_metadata(&PipelineRecoverySidecar::new(
        2,
        HashOf::from_untyped_unchecked(Hash::new(b"stale pipeline successor")),
        PipelineDagSnapshot {
            fingerprint: [0xC3; 32],
            key_count: 2,
        },
        Vec::new(),
    ));
}
fn canonical_prune_sidecar_files(kura: &Kura) -> Vec<(PathBuf, Vec<u8>)> {
    let directory = kura.active_blocks_dir.lock().join(PIPELINE_DIR_NAME);
    [PIPELINE_SIDECARS_DATA_FILE, PIPELINE_SIDECARS_INDEX_FILE]
        .into_iter()
        .map(|name| {
            let path = directory.join(name);
            let bytes = fs::read(&path).expect("snapshot canonical prune sidecar file");
            (path, bytes)
        })
        .collect()
}
#[test]
fn pending_v2_prune_intents_are_rejected_before_mutation() {
    for temporary in [false, true] {
        let temp_dir = TempDir::new().expect("V2 prune-intent temp dir");
        let pair = || LegacyKuraPruneSidecarPairProjectionV2Fixture {
            required: false,
            retained_data_bytes: 0,
            retained_index_bytes: 0,
        };
        let legacy = LegacyKuraPruneIntentV2Fixture {
            version: 2,
            source_height: 2,
            source_tip_hash: Some(HashOf::from_untyped_unchecked(Hash::new(b"V2 source"))),
            target_height: 1,
            target_tip_hash: Some(HashOf::from_untyped_unchecked(Hash::new(b"V2 target"))),
            retained_merge_entries: 0,
            retained_merge_tip_hash: None,
            sidecar_rewrite: LegacyKuraPruneSidecarRewriteProjectionV2Fixture {
                pipeline: pair(),
                roster: pair(),
                sequential_peak_bytes: 0,
            },
            capacity: LegacyKuraPruneCapacityAdmissionV2Fixture {
                source_physical_bytes: 0,
                pending_canonical_bytes: 0,
                post_wsv_reserved_bytes: 0,
                certified_bundle_reserved_bytes: 0,
                autonomous_terminal_reserved_bytes: 0,
                intent_bytes: 1,
                marker_temporary_bytes: 1,
                marker_stable_growth_bytes: 0,
                roster: RetiredRosterPruneProjectionV2Fixture {
                    required: false,
                    current_digest: None,
                    retained_digest: None,
                    retained_payload_bytes: 0,
                    generation_allocation_bytes: 0,
                    pointer_temporary_bytes: 0,
                    current_pointer_growth_bytes: 0,
                },
                admitted_peak_bytes: 0,
            },
        };
        let stable = Kura::prune_intent_path_for(temp_dir.path());
        let path = if temporary {
            Kura::prune_intent_temp_path_for(temp_dir.path())
        } else {
            stable
        };
        let bytes = norito::encode_canonical(&legacy).expect("encode V2 prune intent");
        fs::write(&path, &bytes).expect("write V2 prune intent");
        let sentinel = temp_dir.path().join("mutation-sentinel");
        fs::write(&sentinel, b"unchanged").expect("write mutation sentinel");
        let before = snapshot_regular_test_tree(temp_dir.path());
        assert!(matches!(
            Kura::read_prune_intent(temp_dir.path()),
            Err(Error::PruneIntentConflict(_))
        ));
        assert_eq!(
            snapshot_regular_test_tree(temp_dir.path()),
            before,
            "unsupported V2 prune evidence must be rejected before any recovery mutation",
        );
    }
}
#[test]
fn empty_current_tip_cleanup_authenticates_header_only_retained_index() {
    let temp_dir = TempDir::new().expect("empty current-tip prune temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("empty current-tip Kura");
    let directory = kura.active_blocks_dir.lock().join(PIPELINE_DIR_NAME);
    fs::create_dir_all(&directory).expect("create empty current-tip pipeline directory");
    let data_path = directory.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = directory.join(PIPELINE_SIDECARS_INDEX_FILE);
    fs::write(&data_path, b"future-only-pipeline-payload")
        .expect("write future-only pipeline payload");
    let mut index = SidecarIndexLayout::base_header(1).to_vec();
    index.extend_from_slice(
        &SidecarIndexEntry {
            offset: 0,
            len: u64::try_from(b"future-only-pipeline-payload".len())
                .expect("fixture length fits u64"),
        }
        .to_bytes(),
    );
    fs::write(&index_path, index).expect("write future-only pipeline index");
    let projection = {
        let _guard = kura.sidecar_lock.lock();
        kura.reconcile_and_project_prune_sidecar_rewrites_locked(0)
            .expect("project header-only retained index")
    };
    assert!(projection.has_work());
    assert_eq!(
        projection.sequential_peak_bytes,
        INDEXED_SIDECAR_BASE_HEADER_SIZE_U64
    );
    kura.fail_prune_after_stage_for_tests(PRUNE_STAGE_INTENT);
    let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = kura.prune_to_height(0);
    }));
    assert!(crash.is_err());
    crate::sumeragi::status::clear_consensus_transition_poison_for_tests();
    let intent = Kura::read_prune_intent(temp_dir.path())
        .expect("read empty current-tip intent")
        .expect("empty current-tip intent is durable");
    assert_eq!(intent.source_height, 0);
    assert_eq!(intent.target_height, 0);
    assert!(intent.sidecar_rewrite.has_work());
    assert_eq!(
        intent.sidecar_rewrite.sequential_peak_bytes,
        INDEXED_SIDECAR_BASE_HEADER_SIZE_U64
    );
    drop(kura);
    let (recovered, BlockCount(count)) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("recover empty current-tip sidecar cleanup");
    assert_eq!(count, 0);
    assert_eq!(
        fs::metadata(&data_path)
            .expect("retained empty pipeline data")
            .len(),
        0,
    );
    assert_eq!(
        fs::metadata(&index_path)
            .expect("retained empty pipeline index")
            .len(),
        INDEXED_SIDECAR_BASE_HEADER_SIZE_U64,
    );
    recovered
        .validate_pipeline_sidecars_for_prune(0, true)
        .expect("empty retained pair is structurally canonical");
    assert!(!Kura::prune_intent_path_for(temp_dir.path()).exists());
}
#[test]
fn current_tip_sidecar_rewrite_uses_v3_intent_and_exact_peak_capacity() {
    let temp_dir = TempDir::new().expect("current-tip prune-capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("current-tip Kura");
    let blocks = store_dummy_block_arcs(&kura, 1);
    seed_large_current_tip_sidecar_rewrite(&kura, blocks[0].hash());
    let projection = {
        let _guard = kura.sidecar_lock.lock();
        kura.reconcile_and_project_prune_sidecar_rewrites_locked(1)
            .expect("project current-tip retained pairs")
    };
    assert!(projection.pipeline.required);
    assert!(
        projection.pipeline.retained_data_bytes > PRUNE_INTENT_MAX_BYTES as u64,
        "the retained pipeline payload must prove that 4 KiB alone is insufficient",
    );
    assert_eq!(
        projection.sequential_peak_bytes,
        projection
            .pipeline
            .temp_pair_bytes()
            .expect("pipeline pair bytes"),
        "the rewrite reserves the exact retained pipeline pair",
    );
    let preview = admit_prune_intent_fixture(
        &kura,
        KuraPruneIntentV3 {
            version: 3,
            source_height: 1,
            source_tip_hash: Some(blocks[0].hash()),
            target_height: 1,
            target_tip_hash: Some(blocks[0].hash()),
            retained_merge_entries: 0,
            retained_merge_tip_hash: None,
            sidecar_rewrite: projection,
            capacity: unsealed_prune_capacity_fixture(),
        },
    );
    let exact = preview.capacity.admitted_peak_bytes;
    let before = canonical_prune_sidecar_files(&kura);
    Arc::get_mut(&mut kura)
        .expect("current-tip Kura remains exclusive")
        .max_disk_usage_bytes = exact - 1;
    assert!(matches!(
        kura.prune_to_height(1),
        Err(Error::StorageBudgetExceeded { limit, required, .. })
            if limit == exact - 1 && required == exact
    ));
    assert_eq!(canonical_prune_sidecar_files(&kura), before);
    assert!(!Kura::prune_intent_path_for(temp_dir.path()).exists());
    Arc::get_mut(&mut kura)
        .expect("current-tip Kura remains exclusive after rejection")
        .max_disk_usage_bytes = exact;
    kura.fail_prune_after_stage_for_tests(PRUNE_STAGE_INTENT);
    let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = kura.prune_to_height(1);
    }));
    assert!(
        crash.is_err(),
        "intent failpoint must stop before sidecar allocation"
    );
    crate::sumeragi::status::clear_consensus_transition_poison_for_tests();
    let intent = Kura::read_prune_intent(temp_dir.path())
        .expect("read exact current-tip intent")
        .expect("current-tip intent is durable");
    assert_eq!(intent.version, 3);
    assert_eq!(intent.source_height, intent.target_height);
    assert_eq!(intent.source_tip_hash, intent.target_tip_hash);
    assert_eq!(intent.sidecar_rewrite, projection);
    assert_eq!(intent.capacity, preview.capacity);
    assert_eq!(canonical_prune_sidecar_files(&kura), before);
    drop(kura);
    let (recovered, BlockCount(count)) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("recover current-tip V3 sidecar intent");
    assert_eq!(count, 1);
    assert_eq!(recovered.blocks_count(), 1);
    assert!(recovered.read_pipeline_metadata(2).is_none());
    recovered
        .validate_pipeline_sidecars_for_prune(1, true)
        .expect("current-tip sidecars recover to one compact retained prefix");
    assert!(!Kura::prune_intent_path_for(temp_dir.path()).exists());
}
#[test]
fn startup_rewrite_capacity_rejects_one_under_without_sidecar_mutation() {
    let temp_dir = TempDir::new().expect("startup prune-capacity temp dir");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("startup-capacity Kura");
    let blocks = store_dummy_block_arcs(&kura, 1);
    seed_large_current_tip_sidecar_rewrite(&kura, blocks[0].hash());
    let sidecar_rewrite = {
        let _guard = kura.sidecar_lock.lock();
        kura.reconcile_and_project_prune_sidecar_rewrites_locked(1)
            .expect("project startup retained pairs")
    };
    let intent = admit_prune_intent_fixture(
        &kura,
        KuraPruneIntentV3 {
            version: 3,
            source_height: 1,
            source_tip_hash: Some(blocks[0].hash()),
            target_height: 1,
            target_tip_hash: Some(blocks[0].hash()),
            retained_merge_entries: 0,
            retained_merge_tip_hash: None,
            sidecar_rewrite,
            capacity: unsealed_prune_capacity_fixture(),
        },
    );
    kura.persist_prune_intent(&intent)
        .expect("persist startup-capacity intent");
    let physical = kura
        .kura_disk_usage_bytes()
        .expect("measure startup physical bytes with intent");
    let exact = intent.capacity.admitted_peak_bytes;
    assert!(
        exact >= physical,
        "admitted startup peak includes the now-durable intent and every remaining stage",
    );
    let before = canonical_prune_sidecar_files(&kura);
    drop(kura);
    config.max_disk_usage_bytes = iroha_config::base::util::Bytes(exact - 1);
    assert!(matches!(
        Kura::new(&config, &RuntimeLaneConfig::default()),
        Err(Error::StorageBudgetExceeded { limit, required, .. })
            if limit == exact - 1 && required == exact
    ));
    for (path, bytes) in &before {
        assert_eq!(
            fs::read(path)
                .expect("sidecar remains after one-under startup rejection")
                .as_slice(),
            bytes.as_slice(),
        );
    }
    assert!(Kura::prune_intent_path_for(temp_dir.path()).is_file());
    let pipeline_directory = before[0].0.parent().expect("pipeline sidecar has parent");
    assert!(
        !pipeline_directory
            .join(PIPELINE_SIDECARS_DATA_FILE)
            .with_extension("norito.tmp")
            .exists()
    );
    config.max_disk_usage_bytes = iroha_config::base::util::Bytes(exact);
    let (inspection, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("exact startup recovery succeeds with sufficient capacity");
    assert_eq!(inspection.blocks_count(), 1);
    inspection
        .validate_pipeline_sidecars_for_prune(1, true)
        .expect("startup exact-limit recovery compacts sidecars");
    assert_ne!(canonical_prune_sidecar_files(&inspection), before);
    assert!(!Kura::prune_intent_path_for(temp_dir.path()).exists());
}
