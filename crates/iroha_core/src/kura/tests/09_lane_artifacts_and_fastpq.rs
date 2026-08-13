#[test]
fn lane_block_artifacts_snapshot_returns_all_valid_artifacts_in_replay_order() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane0 = LaneId::from(0);
    let lane1 = LaneId::from(1);
    let lane0_entry = lane_config.entry(lane0).expect("lane 0 entry");
    let lane1_entry = lane_config.entry(lane1).expect("lane 1 entry");
    let mut generator = DummyBlocks::new();
    let lane1_later = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane1,
        lane1_entry.dataspace_id,
        3,
    );
    let lane0_first = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane0,
        lane0_entry.dataspace_id,
        1,
    );
    let lane1_first = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane1,
        lane1_entry.dataspace_id,
        1,
    );
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(lane1_later)
        .expect("store sparse later lane artifact first");
    kura.store_block(lane0_first)
        .expect("store first lane 0 artifact");
    kura.store_block(lane1_first)
        .expect("store first lane 1 artifact after sparse write");
    let replay_keys = kura
        .lane_block_artifacts_snapshot()
        .into_iter()
        .map(|artifact| {
            (
                artifact.ownership.lane_block_height,
                artifact.ownership.lane_id.as_u32(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(replay_keys, vec![(1, 0), (1, 1), (3, 1)]);
    let at_height_two =
        kura.canonical_lane_block_artifacts_at_proposal_height_matching(2, 8, |_| true);
    assert_eq!(at_height_two.len(), 1);
    assert_eq!(at_height_two[0].ownership.lane_id, lane0);
    assert_eq!(at_height_two[0].ownership.proposal_height, 2);
    assert!(
        kura.canonical_lane_block_artifacts_at_proposal_height_matching(2, 0, |_| true)
            .is_empty(),
        "a zero recovery budget must not scan or hydrate artifacts"
    );
    assert!(
        kura.canonical_lane_block_artifacts_at_proposal_height_matching(99, 8, |_| true)
            .is_empty(),
        "a missing global height must return no canonical artifacts"
    );
}
#[test]
fn latest_lane_block_artifact_for_dataspace_skips_newer_foreign_dataspace() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let foreign_dataspace = DataSpaceId::new(77);
    let mut generator = DummyBlocks::new();
    let active = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        lane_entry.dataspace_id,
        2,
    );
    let foreign = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        foreign_dataspace,
        4,
    );
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(active)
        .expect("store active-dataspace lane artifact");
    assert!(
        kura.store_block(foreign).is_err(),
        "active lane storage must reject a foreign-dataspace artifact"
    );
    let latest_any = kura
        .latest_lane_block_artifact(lane_id)
        .expect("latest lane artifact");
    assert_eq!(latest_any.ownership.dataspace_id, lane_entry.dataspace_id);
    assert_eq!(latest_any.ownership.lane_block_height, 2);
    let latest_active = kura
        .latest_lane_block_artifact_for_dataspace(lane_id, lane_entry.dataspace_id)
        .expect("latest active-dataspace lane artifact");
    assert_eq!(
        latest_active.ownership.dataspace_id,
        lane_entry.dataspace_id
    );
    assert_eq!(latest_active.ownership.lane_block_height, 2);
}
#[test]
fn lane_block_artifact_read_rejects_global_block_hash_mismatch() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let mut forged = kura
        .read_lane_block_artifact(lane_id, lane_block_height)
        .expect("lane block artifact");
    forged.proposal_block_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; 32]));
    let payload = forged.encode_framed().expect("encode forged artifact");
    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "lane block artifact",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "overwrite lane artifact with forged block hash"
    );
    assert!(
        kura.read_lane_block_artifact(lane_id, lane_block_height)
            .is_none(),
        "forged global block hash must make the artifact unreadable"
    );
}
#[test]
fn lane_block_artifact_read_rejects_replay_material_mismatch() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let mut forged = kura
        .read_lane_block_artifact(lane_id, lane_block_height)
        .expect("lane block artifact");
    forged.ownership.accepted_transaction_hashes[0] =
        Hash::new(b"forged accepted transaction hash");
    let payload = forged.encode_framed().expect("encode forged artifact");
    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "lane block artifact",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "overwrite lane artifact with forged replay material"
    );
    assert!(
        kura.read_lane_block_artifact(lane_id, lane_block_height)
            .is_none(),
        "forged descriptor replay material must make the artifact unreadable"
    );
}
#[test]
fn latest_lane_block_artifact_skips_replay_material_mismatch() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let mut generator = DummyBlocks::new();
    let first = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        lane_entry.dataspace_id,
        1,
    );
    let later = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        lane_entry.dataspace_id,
        3,
    );
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(first).expect("store first lane artifact");
    kura.store_block(later)
        .expect("store sparse later artifact");
    let mut forged = kura
        .read_lane_block_artifact(lane_id, 3)
        .expect("later lane block artifact");
    forged.ownership.lane_block_descriptor_validator_count = forged
        .ownership
        .lane_block_descriptor_validator_count
        .saturating_add(1);
    let payload = forged.encode_framed().expect("encode forged artifact");
    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            3,
            &payload,
            "lane block artifact",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "overwrite later lane artifact with forged replay material"
    );
    let latest = kura
        .latest_lane_block_artifact(lane_id)
        .expect("latest valid lane block artifact");
    assert_eq!(
        latest.ownership.lane_block_height, 1,
        "latest artifact scan must skip corrupt newer replay material"
    );
}
#[test]
fn lane_block_artifact_conflicting_rewrite_is_rejected_and_preserves_original() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut generator = DummyBlocks::new();
    let first = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let second = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    assert_ne!(
        first.hash(),
        second.hash(),
        "test setup should produce distinct global proposals"
    );
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::clone(&first))
        .expect("store first lane artifact");
    let original = kura
        .read_lane_block_artifact(lane_id, lane_block_height)
        .expect("original lane block artifact");
    let err = kura
        .store_block(second)
        .expect_err("conflicting lane artifact rewrite must be rejected");
    match err {
        Error::IO(io, path) => {
            assert_eq!(io.kind(), ErrorKind::InvalidData);
            assert!(
                io.to_string().contains("lane artifact already exists"),
                "unexpected lane artifact conflict error: {io}"
            );
            assert!(
                path.display().to_string().contains("lane_artifacts"),
                "unexpected error path: {}",
                path.display()
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(
        kura.read_lane_block_artifact(lane_id, lane_block_height),
        Some(original),
        "conflicting rewrite must not replace the original artifact"
    );
    assert_eq!(
        kura.blocks_count(),
        1,
        "conflicting lane artifact must abort before the second global block is stored"
    );
}
#[test]
fn lane_block_artifact_rolls_back_when_block_write_fails() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut generator = DummyBlocks::new();
    let aborted = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let mut replacement_generator = DummyBlocks::new();
    let replacement = dummy_block_with_lane_payload_ownership_from_generator(
        &mut replacement_generator,
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    assert_ne!(aborted.hash(), replacement.hash());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.fail_next_block_write_for_tests();
    let err = kura
        .store_block(aborted)
        .expect_err("injected block write failure");
    assert!(matches!(err, Error::IO(_, _)));
    assert_eq!(kura.blocks_count(), 0);
    assert!(
        kura.read_lane_block_artifact(lane_id, lane_block_height)
            .is_none(),
        "aborted block must not leave a readable lane artifact"
    );
    assert_lane_artifact_files_absent_or_empty(lane_entry, temp_dir.path());
    kura.store_block(replacement)
        .expect("later valid block at same lane height must not be poisoned");
    assert!(
        kura.read_lane_block_artifact(lane_id, lane_block_height)
            .is_some(),
        "replacement block should persist its lane artifact"
    );
}
#[test]
fn lane_block_artifact_backward_rebase_rolls_back_when_block_write_fails() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let mut generator = DummyBlocks::new();
    let high = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        lane_entry.dataspace_id,
        3,
    );
    let lower = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        lane_entry.dataspace_id,
        1,
    );
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(high)
        .expect("store initial high lane artifact");
    let original = kura
        .read_lane_block_artifact(lane_id, 3)
        .expect("initial high lane artifact");
    kura.fail_next_block_write_for_tests();
    assert!(
        kura.store_block(lower).is_err(),
        "injected block failure should abort the lower artifact"
    );
    assert_eq!(kura.blocks_count(), 1);
    assert_eq!(
        kura.read_lane_block_artifact(lane_id, 3),
        Some(original),
        "rollback must preserve the original compact-index artifact"
    );
    assert!(
        kura.read_lane_block_artifact(lane_id, 1).is_none(),
        "rollback must remove the prepended lower artifact"
    );
    let (_, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
    let mut index = std::fs::File::open(index_path).expect("open rolled-back lane index");
    let index_len = index.metadata().expect("lane index metadata").len();
    let layout = SidecarIndexLayout::read_from(&mut index, index_len)
        .expect("read rolled-back lane index layout");
    assert!(layout.is_based());
    assert_eq!(layout.base_height, 3);
    assert_eq!(layout.height_range(), Some(3..=3));
}
#[test]
fn lane_block_artifact_remains_canonical_when_post_commit_merge_append_fails() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut generator = DummyBlocks::new();
    let parent = generator.next();
    let aborted = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let mut replacement_generator = DummyBlocks {
        blocks: vec![Arc::clone(&parent)],
    };
    let replacement = dummy_block_with_lane_payload_ownership_from_generator(
        &mut replacement_generator,
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    assert_ne!(aborted.hash(), replacement.hash());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(parent).expect("store carrier parent");
    let mut entry = sample_merge_entry(1);
    let aborted = bind_merge_entry_to_carrier(aborted, &mut entry);
    let aborted_hash = aborted.hash();
    let entry_hash = entry.canonical_hash();
    kura.fail_next_merge_append_for_test();
    let err = kura
        .store_block_with_merge_entry(Arc::clone(&aborted), &entry)
        .expect_err("merge log append should fail");
    assert!(matches!(
        err,
        Error::CanonicalBlockCommittedRecoveryRequired { .. }
    ));
    assert!(err.requires_restart_recovery());
    assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert_eq!(
        Kura::read_durable_hash_at_height(&mut kura.block_store.lock(), 2)
            .expect("read committed carrier while poisoned"),
        Some(aborted_hash)
    );
    drop(kura);
    let (kura, BlockCount(count)) =
        Kura::new(&config, &lane_config).expect("restart repairs committed association");
    assert_eq!(count, 2);
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    let artifact = kura
        .read_lane_block_artifact(lane_id, lane_block_height)
        .expect("the committed carrier retains its lane artifact");
    assert_eq!(artifact.proposal_block_hash, aborted_hash);
    assert_eq!(
        kura.merge_carrier_for_entry(entry_hash)
            .expect("carrier index remains readable"),
        Some(MergeLedgerCarrierRecord::new(&entry, &aborted)),
        "restart must complete the durable association stage"
    );
    let replacement_error = kura
        .store_block(replacement)
        .expect_err("a different replacement cannot overwrite the committed carrier");
    assert!(matches!(
        replacement_error,
        Error::BlockHeightConflict { height: 2, .. }
    ));
    assert_eq!(
        kura.read_lane_block_artifact(lane_id, lane_block_height),
        Some(artifact),
        "the canonical lane artifact must survive a conflicting replacement attempt"
    );
    kura.store_block_with_merge_entry(aborted, &entry)
        .expect("exact post-restart retry is idempotent");
    assert_eq!(kura.merge_ledger_snapshot(), vec![entry]);
    assert!(
        kura.merge_carrier_for_entry(entry_hash)
            .expect("read repaired carrier")
            .is_some()
    );
}
#[test]
fn replace_top_block_overwrites_replaced_lane_artifact() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let original = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let mut replacement = original.as_ref().clone();
    let mut replacement_header = replacement.header();
    replacement_header
        .set_view_change_index(replacement_header.view_change_index().saturating_add(1));
    replacement.replace_header_for_testing(replacement_header);
    let entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"kura-lane-artifact-replacement-entrypoint",
    ));
    replacement.set_execution_context(Some(BlockExecutionContextBundle::new(vec![
        ExternalExecutionContext::new(entrypoint_hash, lane_id, lane_entry.dataspace_id),
    ])));
    let replacement_ownership = sample_lane_payload_ownership_for_kura(
        &replacement,
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    replacement.set_execution_context(Some(
        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
            entrypoint_hash,
            lane_id,
            lane_entry.dataspace_id,
        )])
        .with_lane_payload_ownerships(vec![replacement_ownership.clone()]),
    ));
    let replacement = Arc::new(replacement);
    assert_ne!(
        original.hash(),
        replacement.hash(),
        "replacement must change the top block hash"
    );
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(original)
        .expect("store original lane artifact");
    kura.replace_top_block(Arc::clone(&replacement))
        .expect("replace top block with lane artifact");
    let artifact = kura
        .read_lane_block_artifact(lane_id, lane_block_height)
        .expect("replacement lane artifact");
    assert_eq!(artifact.proposal_block_hash, replacement.hash());
    assert_eq!(artifact.ownership, replacement_ownership);
    assert_eq!(
        kura.block_hash_at_height(nonzero!(1_usize)),
        Some(replacement.hash())
    );
}
#[test]
fn store_block_rejects_lane_payload_ownership_for_unconfigured_lane() {
    let kura = Kura::blank_kura_for_testing();
    let block =
        dummy_block_with_lane_payload_ownership(LaneId::from(99), DataSpaceId::UNIVERSAL, 1);
    let err = kura
        .store_block(block)
        .expect_err("unconfigured lane artifact must be rejected");
    match err {
        Error::IO(io, path) => {
            assert_eq!(io.kind(), ErrorKind::NotFound);
            assert!(
                path.display().to_string().contains("lane_099"),
                "unexpected error path: {}",
                path.display()
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(
        kura.blocks_count(),
        0,
        "block must not be committed when lane artifact persistence fails"
    );
}
#[test]
fn pipeline_sidecar_roundtrip() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    kura.write_pipeline_metadata(&sidecar);
    let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
    assert_eq!(got.height, 1);
    assert_eq!(got.block_hash, block_hash);
    assert_eq!(got.dag.key_count, 0);
    assert_eq!(got.format_label(), "pipeline.recovery");
}
#[test]
fn framed_sidecar_boundaries_are_canonical_and_ambient_independent() {
    let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"canonical framed sidecar boundary",
    ));
    let pipeline = PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [0xA5; 32],
            key_count: 1,
        },
        Vec::new(),
    );
    let roster = RosterSidecar::new(1, block_hash, None, None, None);
    let canonical_pipeline =
        norito::encode_canonical(&pipeline).expect("encode canonical pipeline sidecar");
    let canonical_roster =
        norito::encode_canonical(&roster).expect("encode canonical roster sidecar");
    assert_eq!(
        pipeline.encode_framed().expect("frame pipeline sidecar"),
        canonical_pipeline
    );
    assert_eq!(
        roster.encode_framed().expect("frame roster sidecar"),
        canonical_roster
    );
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let (alternate_pipeline, alternate_roster) = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        (
            norito::to_bytes(&pipeline).expect("encode alternate-layout pipeline sidecar"),
            norito::to_bytes(&roster).expect("encode alternate-layout roster sidecar"),
        )
    };
    assert_ne!(alternate_pipeline, canonical_pipeline);
    assert_ne!(alternate_roster, canonical_roster);
    assert!(
        norito::decode_canonical::<PipelineRecoverySidecar>(&alternate_pipeline).is_err(),
        "durable pipeline sidecars must reject alternate layouts"
    );
    assert!(
        norito::decode_canonical::<RosterSidecar>(&alternate_roster).is_err(),
        "durable roster sidecars must reject alternate layouts"
    );
    let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    assert_eq!(
        pipeline
            .encode_framed()
            .expect("frame pipeline sidecar under alternate ambient layout"),
        canonical_pipeline
    );
    assert_eq!(
        roster
            .encode_framed()
            .expect("frame roster sidecar under alternate ambient layout"),
        canonical_roster
    );
}
#[test]
fn bound_progress_intent_identity_is_canonical_and_ambient_independent() {
    let payload = b"bound progress canonical payload";
    let intent = BoundProgressAppendIntentV1 {
        version: BOUND_PROGRESS_APPEND_INTENT_VERSION,
        namespace_components: vec!["blocks".to_owned(), "lane".to_owned()],
        data_file: "progress.data".to_owned(),
        index_file: "progress.index".to_owned(),
        height: 1,
        pair_was_present: false,
        old_data_len: 0,
        new_data_len: u64::try_from(payload.len()).expect("payload length fits u64"),
        payload_hash: BoundProgressAppendIntentV1::payload_digest(payload),
        old_index_len: 0,
        new_index_len: 16,
        index_write_offset: 0,
        old_index_bytes: Vec::new(),
        new_index_bytes: vec![0; 16],
        integrity_hash: Hash::prehashed([0; Hash::LENGTH]),
    }
    .seal();
    let canonical = norito::encode_canonical(&intent).expect("encode canonical progress intent");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&intent).expect("encode alternate-layout progress intent")
    };
    assert_ne!(alternate, canonical);
    assert!(
        norito::decode_canonical::<BoundProgressAppendIntentV1>(&alternate).is_err(),
        "durable progress intents must reject alternate layouts"
    );
    let integrity_hash = intent.integrity_hash;
    let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    assert_eq!(
        intent.computed_integrity_hash(),
        Some(integrity_hash),
        "intent identity must ignore ambient layout"
    );
    assert_eq!(
        norito::encode_canonical(&intent).expect("encode intent under alternate ambient layout"),
        canonical
    );
}
#[test]
fn pipeline_sidecar_exact_candidate_read_preserves_canonical_authority() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default()).expect("open Kura");
    let mut blocks = DummyBlocks::new();
    let candidate = blocks.next();
    let height = candidate.header().height().get();
    let block_hash = candidate.hash();
    let sidecar = PipelineRecoverySidecar::new(
        height,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [0xA5; 32],
            key_count: 1,
        },
        Vec::new(),
    );
    kura.write_pipeline_metadata(&sidecar);
    let exact = kura
        .read_pipeline_metadata_for_block(height, block_hash)
        .expect("an exact candidate may reuse its pre-canonical sidecar");
    assert_eq!(exact.block_hash, block_hash);
    let competing_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"competing pipeline sidecar candidate",
    ));
    assert!(
        kura.read_pipeline_metadata_for_block(height, competing_hash)
            .is_none(),
        "a competing candidate must not reuse the sidecar"
    );
    assert!(
        kura.read_pipeline_metadata(height).is_none(),
        "an exact candidate read must not confer canonical authority"
    );
    kura.store_block(candidate)
        .expect("store the exact candidate as canonical");
    let canonical = kura
        .read_pipeline_metadata(height)
        .expect("the sidecar becomes canonically readable only after block storage");
    assert_eq!(canonical.block_hash, block_hash);
}
#[test]
fn pipeline_sidecar_canonical_boundary_rejects_missing_current_fields() {
    #[derive(Debug, Clone, Encode, Decode)]
    struct LegacyPipelineRecoverySidecar {
        format: PipelineRecoveryFormat,
        height: u64,
        block_hash: HashOf<BlockHeader>,
        dag: PipelineDagSnapshot,
        txs: Vec<PipelineTxSnapshot>,
        #[norito(default)]
        proofs: Vec<PipelineProofSnapshot>,
    }
    let block_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"legacy-pipeline-sidecar"));
    let legacy = LegacyPipelineRecoverySidecar {
        format: PipelineRecoveryFormat::Current,
        height: 1,
        block_hash,
        dag: PipelineDagSnapshot {
            fingerprint: [0u8; 32],
            key_count: 0,
        },
        txs: Vec::new(),
        proofs: Vec::new(),
    };
    let mut bytes = norito::to_bytes(&legacy).expect("encode legacy sidecar");
    let schema = <PipelineRecoverySidecar as norito::core::NoritoSerialize>::schema_hash();
    let schema_start = MAGIC.len() + 2;
    let schema_end = schema_start + schema.len();
    assert!(bytes.len() >= Header::SIZE);
    bytes[schema_start..schema_end].copy_from_slice(&schema);
    let decoded: PipelineRecoverySidecar = norito::decode_from_bytes(&bytes)
        .expect("ordinary decoding accepts the pre-release defaulted field");
    assert_eq!(decoded.height, legacy.height);
    assert_eq!(decoded.block_hash, legacy.block_hash);
    assert!(decoded.proofs.is_empty());
    assert!(decoded.fastpq_proofs.is_empty());
    assert!(
        norito::decode_canonical::<PipelineRecoverySidecar>(&bytes).is_err(),
        "the durable V1 boundary must reject a byte layout missing current fields"
    );
}
#[test]
fn pipeline_tx_snapshot_compact_omits_keys_and_preserves_counts() {
    let hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"compact-pipeline-tx-snapshot",
    ));
    let snapshot = PipelineTxSnapshot::compact(hash, usize::MAX, 7);
    assert!(snapshot.reads.is_empty());
    assert!(snapshot.writes.is_empty());
    assert_eq!(snapshot.read_count(), u32::MAX);
    assert_eq!(snapshot.write_count(), 7);
}
#[test]
fn pipeline_tx_snapshot_legacy_key_lists_contribute_counts() {
    let hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"legacy-pipeline-tx-snapshot",
    ));
    let snapshot = PipelineTxSnapshot {
        hash,
        reads: vec!["state:alpha".to_owned(), "state:beta".to_owned()],
        writes: vec!["state:gamma".to_owned()],
        read_count: 0,
        write_count: 0,
    };
    assert_eq!(snapshot.read_count(), 2);
    assert_eq!(snapshot.write_count(), 1);
}
#[test]
fn pipeline_sidecar_enqueue_flushes() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    assert_eq!(
        kura.enqueue_pipeline_metadata(sidecar),
        PipelineSidecarEnqueueResult::Enqueued { queue_depth: 1 }
    );
    assert!(kura.read_pipeline_metadata(1).is_none());
    kura.flush_pipeline_sidecars();
    let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
    assert_eq!(got.height, 1);
    assert_eq!(got.block_hash, block_hash);
}
#[test]
fn pipeline_sidecar_enqueue_coalesces_writer_notifications() {
    let kura = Kura::blank_kura_for_testing();
    let first_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"pipeline-sidecar-first"));
    let second_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"pipeline-sidecar-second"));
    let snapshot = PipelineDagSnapshot {
        fingerprint: [0u8; 32],
        key_count: 0,
    };
    let first = PipelineRecoverySidecar::new(1, first_hash, snapshot.clone(), Vec::new());
    let second = PipelineRecoverySidecar::new(2, second_hash, snapshot, Vec::new());
    assert_eq!(
        kura.enqueue_pipeline_metadata(first),
        PipelineSidecarEnqueueResult::Enqueued { queue_depth: 1 }
    );
    assert_eq!(
        kura.enqueue_pipeline_metadata(second),
        PipelineSidecarEnqueueResult::Enqueued { queue_depth: 2 }
    );
    assert_eq!(kura.pipeline_sidecar_queue.lock().len(), 2);
    let rx_guard = kura.block_notify_rx.lock();
    let rx = rx_guard
        .as_ref()
        .expect("writer receiver should be present");
    assert_eq!(rx.try_recv(), Ok(BlockNotify::NewBlock));
    assert!(matches!(rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
}
#[test]
fn pipeline_sidecar_enqueue_rejects_queue_overflow() {
    let kura = Kura::blank_kura_for_testing();
    kura.set_pipeline_sidecar_queue_cap_for_testing(1);
    let first_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"pipeline-sidecar-cap-first"));
    let second_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"pipeline-sidecar-cap-second"));
    let snapshot = PipelineDagSnapshot {
        fingerprint: [0u8; 32],
        key_count: 0,
    };
    let first = PipelineRecoverySidecar::new(1, first_hash, snapshot.clone(), Vec::new());
    let second = PipelineRecoverySidecar::new(2, second_hash, snapshot, Vec::new());
    assert_eq!(
        kura.enqueue_pipeline_metadata(first),
        PipelineSidecarEnqueueResult::Enqueued { queue_depth: 1 }
    );
    assert_eq!(
        kura.enqueue_pipeline_metadata(second),
        PipelineSidecarEnqueueResult::RejectedQueueFull { cap: 1 }
    );
    let queue = kura.pipeline_sidecar_queue.lock();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].height, 1);
}
fn sample_fastpq_snapshot(
    height: u64,
    block_hash: HashOf<BlockHeader>,
    proof_len: usize,
) -> FastpqProofSnapshot {
    let proof = vec![0x7a; proof_len];
    FastpqProofSnapshot {
        height,
        block_hash,
        entry_hash: Hash::new(format!("fastpq-entry-{height}-{proof_len}").into_bytes()),
        batch_index: 0,
        parameter: "fastpq-lane-balanced".to_string(),
        transition_count: 0,
        trace_commitment: Hash::new(b"trace-commitment"),
        proof_digest: Hash::new(&proof),
        batch: fastpq_prover::TransitionBatch::new(
            "fastpq-lane-balanced",
            fastpq_prover::PublicInputs::default(),
        ),
        proof,
    }
}
#[test]
fn consensus_sidecar_enqueues_do_not_wait_for_unrelated_prune_lock_holder() {
    let kura = Kura::blank_kura_for_testing();
    let block_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"non-prune writer lock"));
    let pipeline = PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [0x41; 32],
            key_count: 1,
        },
        Vec::new(),
    );
    let fastpq = sample_fastpq_snapshot(1, block_hash, 8);
    // The writer loop holds this lock while flushing sidecars and enforcing the storage
    // budget. Consensus enqueues must remain memory-only and must not wait for that I/O.
    let prune_guard = kura.prune_lock.lock();
    assert!(!kura.prune_in_progress.load(Ordering::Acquire));
    let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
    let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
    let enqueue_kura = Arc::clone(&kura);
    let enqueuer = thread::spawn(move || {
        started_tx.send(()).expect("announce enqueue start");
        let pipeline_result = enqueue_kura.enqueue_pipeline_metadata(pipeline);
        let fastpq_result = enqueue_kura.enqueue_fastpq_proof_snapshot(fastpq);
        result_tx
            .send((pipeline_result, fastpq_result))
            .expect("report enqueue results");
    });
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("enqueue worker started");
    let results = result_rx.recv_timeout(Duration::from_secs(2));
    drop(prune_guard);
    enqueuer.join().expect("enqueue worker");
    let (pipeline_result, fastpq_result) =
        results.expect("memory-only enqueues must not wait behind prune_lock");
    assert_eq!(
        pipeline_result,
        PipelineSidecarEnqueueResult::Enqueued { queue_depth: 1 }
    );
    assert_eq!(
        fastpq_result,
        FastpqProofEnqueueResult::Enqueued { queue_depth: 1 }
    );
    assert_eq!(kura.pipeline_sidecar_queue.lock().len(), 1);
    assert_eq!(kura.fastpq_proof_queue.lock().len(), 1);
}
#[test]
fn fastpq_proof_snapshot_merges_into_pipeline_sidecar() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    kura.write_pipeline_metadata(&sidecar);
    let proof = b"fastpq-proof".to_vec();
    let snapshot = FastpqProofSnapshot {
        height: 1,
        block_hash,
        entry_hash: Hash::prehashed([0x11; 32]),
        batch_index: 0,
        parameter: "fastpq-lane-balanced".to_string(),
        transition_count: 0,
        trace_commitment: Hash::new(b"trace-commitment"),
        proof_digest: Hash::new(&proof),
        batch: fastpq_prover::TransitionBatch::new(
            "fastpq-lane-balanced",
            fastpq_prover::PublicInputs::default(),
        ),
        proof,
    };
    assert!(matches!(
        kura.enqueue_fastpq_proof_snapshot(snapshot.clone()),
        FastpqProofEnqueueResult::Enqueued { .. }
    ));
    assert_eq!(kura.flush_fastpq_proof_snapshots(), 1);
    let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
    let compact = snapshot.compact_for_sidecar();
    assert_eq!(got.fastpq_proofs, vec![compact.clone()]);
    assert!(got.fastpq_proofs[0].proof.is_empty());
    assert!(got.fastpq_proofs[0].batch.transitions.is_empty());
    assert!(got.fastpq_proofs[0].decode_proof().is_err());
    assert_eq!(kura.fastpq_proofs_for_block(1), vec![compact]);
    let duplicate = got.fastpq_proofs[0].clone();
    assert!(matches!(
        kura.enqueue_fastpq_proof_snapshot(duplicate),
        FastpqProofEnqueueResult::Enqueued { .. }
    ));
    assert_eq!(kura.flush_fastpq_proof_snapshots(), 1);
    let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
    assert_eq!(got.fastpq_proofs.len(), 1);
}
#[test]
fn fastpq_proof_snapshot_persists_compact_metadata_only() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let block_hash = store_dummy_blocks(&kura, 1)[0];
    kura.write_pipeline_metadata(&PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [0u8; 32],
            key_count: 0,
        },
        Vec::new(),
    ));
    let mut snapshot = sample_fastpq_snapshot(1, block_hash, 128 * 1024);
    snapshot.batch.push(fastpq_prover::StateTransition::new(
        b"state-key".to_vec(),
        vec![0x01; 1024],
        vec![0x02; 1024],
        fastpq_prover::OperationKind::Transfer,
    ));
    snapshot.transition_count = u32::try_from(snapshot.batch.transitions.len()).unwrap();
    assert!(matches!(
        kura.enqueue_fastpq_proof_snapshot(snapshot.clone()),
        FastpqProofEnqueueResult::Enqueued { .. }
    ));
    assert_eq!(kura.flush_fastpq_proof_snapshots(), 1);
    let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
    let persisted = got.fastpq_proofs.first().expect("proof summary persisted");
    assert_eq!(persisted.proof_digest, snapshot.proof_digest);
    assert_eq!(persisted.trace_commitment, snapshot.trace_commitment);
    assert_eq!(persisted.transition_count, 1);
    assert!(persisted.proof.is_empty());
    assert!(persisted.batch.transitions.is_empty());
    assert!(persisted.batch.metadata.is_empty());
    assert_eq!(persisted.batch.public_inputs, snapshot.batch.public_inputs);
}
#[test]
fn fastpq_proof_snapshots_for_same_block_flush_as_single_sidecar_update() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let base_payload_len = sidecar.encode_framed().expect("encode sidecar").len() as u64;
    kura.write_pipeline_metadata(&sidecar);
    let snapshot1 = sample_fastpq_snapshot(1, block_hash, 8);
    let snapshot2 = sample_fastpq_snapshot(1, block_hash, 9);
    assert!(matches!(
        kura.enqueue_fastpq_proof_snapshot(snapshot1),
        FastpqProofEnqueueResult::Enqueued { .. }
    ));
    assert!(matches!(
        kura.enqueue_fastpq_proof_snapshot(snapshot2),
        FastpqProofEnqueueResult::Enqueued { .. }
    ));
    assert_eq!(kura.flush_fastpq_proof_snapshots(), 2);
    let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
    assert_eq!(got.fastpq_proofs.len(), 2);
    let updated_payload_len = got.encode_framed().expect("encode updated sidecar").len() as u64;
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    let data_len = fs::metadata(pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE))
        .expect("sidecar data metadata")
        .len();
    assert_eq!(
        data_len,
        base_payload_len + updated_payload_len,
        "proof attachments for one block should not append an intermediate sidecar copy"
    );
}
#[test]
fn fastpq_proof_snapshot_rejects_queue_overflow() {
    let kura = Kura::blank_kura_for_testing();
    kura.set_fastpq_proof_sidecar_limits_for_testing(1, usize::MAX, 2);
    let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
    assert!(matches!(
        kura.enqueue_fastpq_proof_snapshot(sample_fastpq_snapshot(1, block_hash, 8)),
        FastpqProofEnqueueResult::Enqueued { queue_depth: 1 }
    ));
    assert_eq!(
        kura.enqueue_fastpq_proof_snapshot(sample_fastpq_snapshot(1, block_hash, 9)),
        FastpqProofEnqueueResult::RejectedQueueFull { cap: 1 }
    );
    assert_eq!(kura.fastpq_proof_queue.lock().len(), 1);
}
#[test]
fn fastpq_proof_snapshot_rejects_oversized_snapshot() {
    let kura = Kura::blank_kura_for_testing();
    kura.set_fastpq_proof_sidecar_limits_for_testing(8, 1, 2);
    let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
    match kura.enqueue_fastpq_proof_snapshot(sample_fastpq_snapshot(1, block_hash, 8)) {
        FastpqProofEnqueueResult::RejectedTooLarge { actual, max } => {
            assert!(actual > max);
            assert_eq!(max, 1);
        }
        result => panic!("expected oversized rejection, got {result:?}"),
    }
    assert!(kura.fastpq_proof_queue.lock().is_empty());
}
#[test]
fn fastpq_proof_snapshot_retries_missing_sidecar_until_limit() {
    let kura = Kura::blank_kura_for_testing();
    kura.set_fastpq_proof_sidecar_limits_for_testing(8, usize::MAX, 2);
    let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
    assert!(matches!(
        kura.enqueue_fastpq_proof_snapshot(sample_fastpq_snapshot(1, block_hash, 8)),
        FastpqProofEnqueueResult::Enqueued { .. }
    ));
    assert_eq!(kura.flush_fastpq_proof_snapshots(), 0);
    assert_eq!(kura.fastpq_proof_queue.lock().len(), 1);
    assert_eq!(kura.flush_fastpq_proof_snapshots(), 0);
    assert!(kura.fastpq_proof_queue.lock().is_empty());
}
#[test]
fn pipeline_sidecar_promotes_temp_index_on_read() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let payload = sidecar.encode_framed().expect("encode sidecar");
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    fs::write(&data_path, &payload).expect("write sidecar data");
    std::fs::File::create(&index_path).expect("create empty index");
    let temp_index_path = index_path.with_extension("index.tmp");
    let entry = SidecarIndexEntry {
        offset: 0,
        len: payload.len() as u64,
    }
    .to_bytes();
    let mut temp = std::fs::File::create(&temp_index_path).expect("create temp index");
    temp.write_all(&entry).expect("write temp index entry");
    temp.flush().expect("flush temp index");
    temp.sync_data().expect("sync temp index");
    let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
    assert_eq!(got.block_hash, block_hash);
    assert!(!temp_index_path.exists(), "temp index should be promoted");
    let index_len = std::fs::metadata(&index_path)
        .expect("index metadata")
        .len();
    assert_eq!(index_len, PIPELINE_INDEX_ENTRY_SIZE_U64);
}
#[test]
fn pipeline_sidecar_promotes_temp_index_after_data_promotion_crash() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let hashes = store_dummy_blocks(&kura, 2);
    let sidecar = PipelineRecoverySidecar::new(
        1,
        hashes[0],
        PipelineDagSnapshot {
            fingerprint: [0u8; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    let payload = sidecar.encode_framed().expect("encode sidecar");
    let temp_sidecar = PipelineRecoverySidecar::new(
        1,
        hashes[0],
        PipelineDagSnapshot {
            fingerprint: [1u8; 32],
            key_count: 1,
        },
        Vec::new(),
    );
    let temp_payload = temp_sidecar.encode_framed().expect("encode temp sidecar");
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let temp_index_path = index_path.with_extension("index.tmp");
    let mut data_file = std::fs::File::create(&data_path).expect("create sidecar data");
    data_file.write_all(&payload).expect("write sidecar data");
    let temp_offset = payload.len() as u64;
    data_file
        .write_all(&temp_payload)
        .expect("write temp sidecar data");
    data_file.flush().expect("flush sidecar data");
    data_file.sync_data().expect("sync sidecar data");
    let entry = SidecarIndexEntry {
        offset: 0,
        len: payload.len() as u64,
    }
    .to_bytes();
    let mut index = std::fs::File::create(&index_path).expect("create sidecar index");
    index.write_all(&entry).expect("write sidecar index");
    index.flush().expect("flush sidecar index");
    index.sync_data().expect("sync sidecar index");
    let temp_entry = SidecarIndexEntry {
        offset: temp_offset,
        len: temp_payload.len() as u64,
    }
    .to_bytes();
    let mut temp_index = std::fs::File::create(&temp_index_path).expect("create temp index");
    temp_index.write_all(&temp_entry).expect("write temp index");
    temp_index.flush().expect("flush temp index");
    temp_index.sync_data().expect("sync temp index");
    let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
    assert_eq!(got.block_hash, hashes[0]);
    assert_eq!(got.dag.fingerprint, [1u8; 32]);
    assert!(
        !temp_index_path.exists(),
        "temp index is the recovery marker after data was already promoted"
    );
    let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
    let mut index_file = std::fs::File::open(&index_path).expect("open sidecar index");
    index_file.read_exact(&mut buf).expect("read sidecar index");
    let entry = SidecarIndexEntry::from_bytes(buf);
    assert_eq!(entry.offset, temp_offset);
    assert_eq!(entry.len, temp_payload.len() as u64);
}
#[test]
fn pipeline_sidecar_recovers_temp_data_before_temp_index() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let block_hash = store_dummy_blocks(&kura, 1)[0];
    let old_sidecar = PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [0u8; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    kura.write_pipeline_metadata(&old_sidecar);
    let recovered_sidecar = PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [2u8; 32],
            key_count: 2,
        },
        Vec::new(),
    );
    let recovered_payload = recovered_sidecar
        .encode_framed()
        .expect("encode recovered sidecar");
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let temp_data_path = data_path.with_extension("norito.tmp");
    let temp_index_path = index_path.with_extension("index.tmp");
    let mut temp_data = std::fs::File::create(&temp_data_path).expect("create temp data");
    temp_data
        .write_all(&recovered_payload)
        .expect("write temp data");
    temp_data.flush().expect("flush temp data");
    temp_data.sync_data().expect("sync temp data");
    let entry = SidecarIndexEntry {
        offset: 0,
        len: recovered_payload.len() as u64,
    }
    .to_bytes();
    let mut temp_index = std::fs::File::create(&temp_index_path).expect("create temp index");
    temp_index.write_all(&entry).expect("write temp index");
    temp_index.flush().expect("flush temp index");
    temp_index.sync_data().expect("sync temp index");
    let got = kura.read_pipeline_metadata(1).expect("recovered sidecar");
    assert_eq!(got.dag.fingerprint, [2u8; 32]);
    assert!(!temp_data_path.exists(), "temp data should be promoted");
    assert!(!temp_index_path.exists(), "temp index should be promoted");
    assert_eq!(
        std::fs::read(&data_path).expect("read promoted data"),
        recovered_payload
    );
}
#[test]
fn pipeline_sidecar_recovery_sync_failure_does_not_expose_new_data_with_old_index() {
    let kura = Kura::blank_kura_for_testing();
    let block_hash = store_dummy_blocks(&kura, 1)[0];
    let old_sidecar = PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [3u8; 32],
            key_count: 3,
        },
        Vec::new(),
    );
    kura.write_pipeline_metadata(&old_sidecar);
    let new_sidecar = PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [4u8; 32],
            key_count: 4,
        },
        Vec::new(),
    );
    let new_payload = new_sidecar.encode_framed().expect("encode new sidecar");
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let old_index = std::fs::read(&index_path).expect("read old index");
    let temp_data_path = data_path.with_extension("norito.tmp");
    let temp_index_path = index_path.with_extension("index.tmp");
    let mut temp_data = std::fs::File::create(&temp_data_path).expect("create temp data");
    temp_data
        .write_all(&new_payload)
        .expect("write new temp data");
    temp_data.flush().expect("flush new temp data");
    temp_data.sync_data().expect("sync new temp data");
    let new_entry = SidecarIndexEntry {
        offset: 0,
        len: new_payload.len() as u64,
    }
    .to_bytes();
    let mut temp_index = std::fs::File::create(&temp_index_path).expect("create temp index");
    temp_index
        .write_all(&new_entry)
        .expect("write new temp index");
    temp_index.flush().expect("flush new temp index");
    temp_index.sync_data().expect("sync new temp index");
    sync_dir(&pipeline_dir).expect("sync recovery markers");
    fail_next_sidecar_promotion_dir_sync_for_tests();
    assert!(
        kura.read_pipeline_metadata(1).is_none(),
        "a failed data-promotion sync must fail closed before the old index can expose new data"
    );
    assert_eq!(
        std::fs::read(&index_path).expect("read unpromoted index"),
        old_index,
        "index must remain unpublished when the data-promotion barrier fails"
    );
    assert!(
        temp_index_path.exists(),
        "durable temp index must remain as the recovery marker"
    );
    let recovered = kura
        .read_pipeline_metadata(1)
        .expect("retry should finish index promotion");
    assert_eq!(recovered.dag.fingerprint, [4u8; 32]);
    assert!(
        !temp_index_path.exists(),
        "retry must consume recovery marker"
    );
}
#[test]
fn pipeline_sidecar_prune_marker_sync_failure_keeps_main_pair_unchanged() {
    let kura = Kura::blank_kura_for_testing();
    let hashes = store_dummy_blocks(&kura, 2);
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    for (index, block_hash) in hashes.into_iter().enumerate() {
        let height = (index + 1) as u64;
        let sidecar = PipelineRecoverySidecar::new(
            height,
            block_hash,
            PipelineDagSnapshot {
                fingerprint: [height as u8; 32],
                key_count: u32::try_from(height).expect("test height fits u32"),
            },
            Vec::new(),
        );
        let payload = sidecar.encode_framed().expect("encode sidecar");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            height,
            &payload,
            "pipeline sidecar test",
            FsyncMode::Always,
            None,
            SidecarIndexOrigin::HeightOne,
        ));
    }
    let old_data = std::fs::read(&data_path).expect("read old data");
    let old_index = std::fs::read(&index_path).expect("read old index");
    fail_next_sidecar_temp_marker_dir_sync_for_tests();
    assert!(
        !Kura::prune_indexed_sidecars(
            &data_path,
            &index_path,
            NonZeroUsize::new(1).expect("non-zero retention"),
            "pipeline sidecar test",
        ),
        "prune must reject a temp recovery marker that was not directory-synced"
    );
    assert_eq!(
        std::fs::read(&data_path).expect("read unchanged data"),
        old_data,
        "new data must not be promoted before the recovery marker is durable"
    );
    assert_eq!(
        std::fs::read(&index_path).expect("read unchanged index"),
        old_index,
        "index must remain paired with the old data after marker sync failure"
    );
    assert!(data_path.with_extension("norito.tmp").exists());
    assert!(index_path.with_extension("index.tmp").exists());
    assert!(Kura::recover_indexed_sidecar_artifacts(
        &data_path,
        &index_path,
        "pipeline sidecar test",
    ));
    assert!(
        Kura::read_indexed_sidecar_from_paths::<PipelineRecoverySidecar, _>(
            1,
            &data_path,
            &index_path,
            norito::decode_from_bytes::<PipelineRecoverySidecar>,
            "pipeline sidecar test",
        )
        .is_none(),
        "pruned height must remain absent after recovery"
    );
    let retained = Kura::read_indexed_sidecar_from_paths::<PipelineRecoverySidecar, _>(
        2,
        &data_path,
        &index_path,
        norito::decode_from_bytes::<PipelineRecoverySidecar>,
        "pipeline sidecar test",
    )
    .expect("retained height after recovery");
    assert_eq!(retained.dag.fingerprint, [2u8; 32]);
}
#[test]
fn pipeline_sidecar_fails_closed_on_corrupt_temp_index() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    kura.write_pipeline_metadata(&sidecar);
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let temp_index_path = index_path.with_extension("index.tmp");
    std::fs::write(&temp_index_path, [0u8; 3]).expect("write corrupt temp index");
    assert!(
        kura.read_pipeline_metadata(1).is_none(),
        "ambiguous recovery state must not expose the old data/index pair"
    );
    assert!(
        temp_index_path.exists(),
        "corrupt temp index should not be promoted"
    );
}
#[test]
fn pipeline_sidecar_fails_closed_on_orphaned_temp_data() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let hashes = store_dummy_blocks(&kura, 2);
    let sidecar = PipelineRecoverySidecar::new(
        1,
        hashes[0],
        PipelineDagSnapshot {
            fingerprint: [0u8; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    kura.write_pipeline_metadata(&sidecar);
    let temp_sidecar = PipelineRecoverySidecar::new(
        1,
        hashes[1],
        PipelineDagSnapshot {
            fingerprint: [1u8; 32],
            key_count: 1,
        },
        Vec::new(),
    );
    let payload = temp_sidecar.encode_framed().expect("encode temp sidecar");
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let temp_data_path = data_path.with_extension("norito.tmp");
    fs::write(&temp_data_path, &payload).expect("write temp data");
    assert!(
        kura.read_pipeline_metadata(1).is_none(),
        "temp data without a recovery index is ambiguous and must fail closed"
    );
}
#[test]
fn pipeline_sidecar_rejects_height_mismatch() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xBB; 32]));
    let sidecar = PipelineRecoverySidecar::new(
        2,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [0x12; 32],
            key_count: 1,
        },
        Vec::new(),
    );
    let payload = sidecar.encode_framed().expect("encode sidecar");
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            1,
            &payload,
            "pipeline sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::HeightOne,
        ),
        "append mismatched sidecar"
    );
    assert!(
        kura.read_pipeline_metadata(1).is_none(),
        "height mismatch should be rejected"
    );
}
#[test]
fn sidecar_fsync_mode_tracks_kura_config() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: BLOCKS_IN_MEMORY,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity:
                iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: iroha_config::kura::FsyncMode::Always,
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
    assert_eq!(kura.sidecar_fsync_mode(), FsyncMode::Always);
}
#[test]
fn pipeline_sidecar_rejects_block_hash_mismatch() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let mut blocks = DummyBlocks::new();
    let block = blocks.next();
    let expected_hash = block.hash();
    kura.store_block(block).expect("store block");
    let mismatch_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xCC; 32]));
    assert_ne!(expected_hash, mismatch_hash, "mismatch hash must differ");
    let sidecar = PipelineRecoverySidecar::new(
        1,
        mismatch_hash,
        PipelineDagSnapshot {
            fingerprint: [0x34; 32],
            key_count: 7,
        },
        Vec::new(),
    );
    kura.write_pipeline_metadata(&sidecar);
    assert!(
        kura.read_pipeline_metadata(1).is_none(),
        "block hash mismatch should be rejected"
    );
}
#[test]
fn pipeline_sidecars_append_to_single_store() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let hashes = store_dummy_blocks(&kura, 2);
    let dag = PipelineDagSnapshot {
        fingerprint: [1u8; 32],
        key_count: 1,
    };
    let sidecar1 = PipelineRecoverySidecar::new(1, hashes[0], dag, Vec::new());
    let sidecar2 = PipelineRecoverySidecar::new(2, hashes[1], dag, Vec::new());
    kura.write_pipeline_metadata(&sidecar1);
    kura.write_pipeline_metadata(&sidecar2);
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    assert!(data_path.is_file(), "pipeline sidecar data file missing");
    assert!(index_path.is_file(), "pipeline sidecar index file missing");
    let index_len = std::fs::metadata(&index_path)
        .expect("index metadata")
        .len();
    assert_eq!(
        index_len,
        2 * PIPELINE_INDEX_ENTRY_SIZE_U64,
        "expected two index entries"
    );
    assert!(
        !pipeline_dir.join("block_1.norito").exists(),
        "per-block sidecar should not be created in aggregated layout"
    );
    let got = kura.read_pipeline_metadata(2).expect("sidecar exists");
    assert_eq!(got.height, 2);
    assert_eq!(got.dag.key_count, 1);
}
#[test]
fn pipeline_sidecar_overwrite_updates_entry() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
    let payload1 = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode dummy sidecar 1");
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            1,
            &payload1,
            "dummy sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::HeightOne,
        ),
        "append height 1 must succeed"
    );
    let payload2 = norito::to_bytes(&DummySidecar { height: 2 }).expect("encode dummy sidecar 2");
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            1,
            &payload2,
            "dummy sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::HeightOne,
        ),
        "overwrite height 1 must succeed"
    );
    let index_len = fs::metadata(&index_path).expect("index metadata").len();
    assert_eq!(
        index_len, PIPELINE_INDEX_ENTRY_SIZE_U64,
        "expected single index entry"
    );
    let mut index = std::fs::File::open(&index_path).expect("index exists");
    let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
    index.read_exact(&mut buf).expect("read index entry");
    let entry = SidecarIndexEntry::from_bytes(buf);
    assert!(entry.len > 0);
    let mut data = std::fs::File::open(&data_path).expect("data exists");
    let len = usize::try_from(entry.len).expect("len fits in usize");
    let mut payload = vec![0u8; len];
    data.seek(SeekFrom::Start(entry.offset))
        .expect("seek to payload");
    data.read_exact(&mut payload).expect("read payload");
    let decoded: DummySidecar = norito::decode_from_bytes(&payload).expect("decode dummy sidecar");
    assert_eq!(decoded.height, 2);
}
#[test]
fn pipeline_sidecar_rejects_overlapping_offsets() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let sidecar1 = PipelineRecoverySidecar::new(
        1,
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32])),
        PipelineDagSnapshot {
            fingerprint: [0x10; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    let sidecar2 = PipelineRecoverySidecar::new(
        2,
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; 32])),
        PipelineDagSnapshot {
            fingerprint: [0x20; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    let payload1 = sidecar1.encode_framed().expect("encode sidecar1");
    let payload2 = sidecar2.encode_framed().expect("encode sidecar2");
    assert_eq!(payload1.len(), payload2.len(), "payload lengths must match");
    fs::write(&data_path, &payload2).expect("write payload data");
    let entry1 = SidecarIndexEntry {
        offset: 0,
        len: payload1.len() as u64,
    };
    let entry2 = SidecarIndexEntry {
        offset: 0,
        len: payload2.len() as u64,
    };
    let mut index = std::fs::File::create(&index_path).expect("create index");
    index.write_all(&entry1.to_bytes()).expect("write entry1");
    index.write_all(&entry2.to_bytes()).expect("write entry2");
    assert!(
        kura.read_pipeline_metadata(2).is_none(),
        "overlapping offsets should be rejected"
    );
}
#[test]
fn pipeline_sidecar_allows_out_of_order_offsets() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let hashes = store_dummy_blocks(&kura, 2);
    let sidecar1 = PipelineRecoverySidecar::new(
        1,
        hashes[0],
        PipelineDagSnapshot {
            fingerprint: [0x30; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    let sidecar2 = PipelineRecoverySidecar::new(
        2,
        hashes[1],
        PipelineDagSnapshot {
            fingerprint: [0x40; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    let payload1 = sidecar1.encode_framed().expect("encode sidecar1");
    let payload2 = sidecar2.encode_framed().expect("encode sidecar2");
    let mut data = std::fs::File::create(&data_path).expect("create data file");
    data.write_all(&payload2).expect("write payload2");
    data.write_all(&payload1).expect("write payload1");
    let entry1 = SidecarIndexEntry {
        offset: payload2.len() as u64,
        len: payload1.len() as u64,
    };
    let entry2 = SidecarIndexEntry {
        offset: 0,
        len: payload2.len() as u64,
    };
    let mut index = std::fs::File::create(&index_path).expect("create index");
    index.write_all(&entry1.to_bytes()).expect("write entry1");
    index.write_all(&entry2.to_bytes()).expect("write entry2");
    let got = kura.read_pipeline_metadata(2).expect("sidecar exists");
    assert_eq!(got.height, 2);
    assert_eq!(got.block_hash, sidecar2.block_hash);
}
#[test]
fn pipeline_sidecar_allows_misaligned_index() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let block_hash = store_dummy_blocks(&kura, 1)[0];
    let sidecar = PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [0x44; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    let payload = sidecar.encode_framed().expect("encode sidecar");
    fs::write(&data_path, &payload).expect("write payload");
    let entry = SidecarIndexEntry {
        offset: 0,
        len: payload.len() as u64,
    };
    let mut index = std::fs::File::create(&index_path).expect("create index");
    index.write_all(&entry.to_bytes()).expect("write entry");
    index.write_all(&[0u8; 3]).expect("write padding");
    let got = kura.read_pipeline_metadata(1).expect("sidecar exists");
    assert_eq!(got.height, 1);
    assert_eq!(got.block_hash, sidecar.block_hash);
}
#[test]
fn sidecar_reader_rejects_oversized_payloads() {
    let temp_dir = TempDir::new().unwrap();
    let store_root = temp_dir.path().join("kura");
    let kura = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(store_root),
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
    .unwrap()
    .0;
    let mut dir = kura.store_dir().expect("store dir");
    dir.push(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&dir).expect("create pipeline dir");
    let data_path = dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let pipeline_limit =
        u64::try_from(MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES).unwrap_or(u64::MAX);
    std::fs::File::create(&data_path)
        .and_then(|file| file.set_len(pipeline_limit + 1))
        .expect("create sparse oversized sidecar data file");
    let entry = SidecarIndexEntry {
        offset: 0,
        len: pipeline_limit + 1,
    }
    .to_bytes();
    std::fs::write(&index_path, entry).expect("write oversized index entry");
    let decoder_called = std::cell::Cell::new(false);
    assert!(
        Kura::read_indexed_sidecar_from_paths_with_recovery_and_limit::<(), _>(
            1,
            &data_path,
            &index_path,
            |_| {
                decoder_called.set(true);
                Ok(())
            },
            "pipeline sidecar",
            false,
            pipeline_limit,
        )
        .is_none()
    );
    assert!(
        !decoder_called.get(),
        "oversized payload must not be decoded"
    );
    assert!(kura.read_pipeline_metadata(1).is_none());
}
#[test]
fn pipeline_sidecar_ignores_invalid_prev_entry() {
    use iroha_config::base::WithOrigin;
    let temp_dir = TempDir::new().unwrap();
    let (kura, _count) = Kura::new(
        &Config {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
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
    let mut pipeline_dir = kura.store_dir().expect("pipeline store dir");
    pipeline_dir.push(PIPELINE_DIR_NAME);
    std::fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");
    let data_path = pipeline_dir.join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = pipeline_dir.join(PIPELINE_SIDECARS_INDEX_FILE);
    let hashes = store_dummy_blocks(&kura, 2);
    let sidecar2 = PipelineRecoverySidecar::new(
        2,
        hashes[1],
        PipelineDagSnapshot {
            fingerprint: [0x66; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    let payload2 = sidecar2.encode_framed().expect("encode sidecar2");
    fs::write(&data_path, &payload2).expect("write payload2");
    let bogus_prev = SidecarIndexEntry {
        offset: 0,
        len: payload2.len() as u64 + 10,
    };
    let entry2 = SidecarIndexEntry {
        offset: 0,
        len: payload2.len() as u64,
    };
    let mut index = std::fs::File::create(&index_path).expect("create index");
    index
        .write_all(&bogus_prev.to_bytes())
        .expect("write bogus entry");
    index.write_all(&entry2.to_bytes()).expect("write entry2");
    let got = kura.read_pipeline_metadata(2).expect("sidecar exists");
    assert_eq!(got.height, 2);
    assert_eq!(got.block_hash, sidecar2.block_hash);
}
#[test]
fn sidecar_append_truncates_misaligned_index() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
    let payload1 = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode dummy sidecar 1");
    let payload2 = norito::to_bytes(&DummySidecar { height: 2 }).expect("encode dummy sidecar 2");
    fs::write(&data_path, &payload1).expect("write payload1");
    let entry1 = SidecarIndexEntry {
        offset: 0,
        len: payload1.len() as u64,
    };
    let mut index = std::fs::File::create(&index_path).expect("create index");
    index.write_all(&entry1.to_bytes()).expect("write entry1");
    index.write_all(&[0u8; 3]).expect("write padding");
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            2,
            &payload2,
            "dummy sidecar",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::HeightOne,
        ),
        "append should succeed and truncate misaligned index"
    );
    let index_len = fs::metadata(&index_path).expect("index metadata").len();
    assert_eq!(
        index_len,
        2 * PIPELINE_INDEX_ENTRY_SIZE_U64,
        "expected aligned index after append"
    );
    let mut index = std::fs::File::open(&index_path).expect("index exists");
    let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
    index.read_exact(&mut buf).expect("read entry1");
    let entry1 = SidecarIndexEntry::from_bytes(buf);
    index.read_exact(&mut buf).expect("read entry2");
    let entry2 = SidecarIndexEntry::from_bytes(buf);
    assert_eq!(entry1.offset, 0);
    assert_eq!(entry1.len, payload1.len() as u64);
    assert_eq!(entry2.offset, payload1.len() as u64);
    assert_eq!(entry2.len, payload2.len() as u64);
}
#[test]
fn sidecar_prune_truncates_misaligned_index() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
    let retention = NonZeroUsize::new(2).expect("non-zero retention");
    let payloads = (1_u64..=3)
        .map(|height| norito::to_bytes(&DummySidecar { height }).expect("encode dummy sidecar"))
        .collect::<Vec<_>>();
    let mut entries = Vec::new();
    let mut data = std::fs::File::create(&data_path).expect("create data");
    let mut offset = 0u64;
    for payload in &payloads {
        data.write_all(payload).expect("write payload");
        entries.push(SidecarIndexEntry {
            offset,
            len: payload.len() as u64,
        });
        offset = offset.saturating_add(payload.len() as u64);
    }
    let mut index = std::fs::File::create(&index_path).expect("create index");
    for entry in &entries {
        index.write_all(&entry.to_bytes()).expect("write entry");
    }
    index.write_all(&[0u8; 3]).expect("write padding");
    assert!(
        Kura::prune_indexed_sidecars(&data_path, &index_path, retention, "dummy sidecar"),
        "prune should tolerate misaligned index"
    );
    let index_len = fs::metadata(&index_path).expect("index metadata").len();
    assert_eq!(
        index_len,
        3 * PIPELINE_INDEX_ENTRY_SIZE_U64,
        "expected aligned index after prune"
    );
    let mut index = std::fs::File::open(&index_path).expect("index exists");
    let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
    let mut pruned_entries = Vec::new();
    for _ in 0..3 {
        index.read_exact(&mut buf).expect("read entry");
        pruned_entries.push(SidecarIndexEntry::from_bytes(buf));
    }
    assert_eq!(pruned_entries[0].len, 0);
    assert!(pruned_entries[1].len > 0);
    assert!(pruned_entries[2].len > 0);
    assert_eq!(pruned_entries[1].offset, 0);
    assert_eq!(pruned_entries[2].offset, pruned_entries[1].len);
    let mut data = std::fs::File::open(&data_path).expect("data exists");
    for (idx, expected_height) in [2_u64, 3_u64].into_iter().enumerate() {
        let entry = &pruned_entries[idx + 1];
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
fn sidecar_prune_skips_entries_past_data_len() {
    let temp_dir = TempDir::new().unwrap();
    let data_path = temp_dir.path().join(PIPELINE_SIDECARS_DATA_FILE);
    let index_path = temp_dir.path().join(PIPELINE_SIDECARS_INDEX_FILE);
    let retention = NonZeroUsize::new(1).expect("non-zero retention");
    let payload1 = norito::to_bytes(&DummySidecar { height: 1 }).expect("encode sidecar");
    fs::write(&data_path, &payload1).expect("write payload");
    let entry1 = SidecarIndexEntry {
        offset: 0,
        len: payload1.len() as u64,
    };
    let entry2 = SidecarIndexEntry {
        offset: payload1.len() as u64 + 8,
        len: 4,
    };
    let mut index = std::fs::File::create(&index_path).expect("create index");
    index.write_all(&entry1.to_bytes()).expect("write entry1");
    index.write_all(&entry2.to_bytes()).expect("write entry2");
    assert!(
        Kura::prune_indexed_sidecars(&data_path, &index_path, retention, "dummy sidecar"),
        "prune should drop invalid entries"
    );
    let mut index = std::fs::File::open(&index_path).expect("index exists");
    let mut buf = [0u8; PIPELINE_INDEX_ENTRY_SIZE];
    index.read_exact(&mut buf).expect("read entry1");
    let entry1 = SidecarIndexEntry::from_bytes(buf);
    index.read_exact(&mut buf).expect("read entry2");
    let entry2 = SidecarIndexEntry::from_bytes(buf);
    assert_eq!(entry1.len, 0);
    assert_eq!(entry2.len, 0);
    assert_eq!(
        fs::metadata(&data_path).expect("data metadata").len(),
        0,
        "invalid kept entry should be dropped from data file"
    );
}
#[test]
fn native_amx_retention_window_advances_base_and_bounds_index() {
    let temp_dir = TempDir::new().expect("temporary sidecar directory");
    let data_path = temp_dir.path().join("bounded-history-dummy.norito");
    let index_path = temp_dir.path().join("bounded-history-dummy.index");
    let retention = NonZeroUsize::new(2).expect("non-zero Native evidence retention");
    for height in 41_u64..=44 {
        let payload =
            norito::to_bytes(&DummySidecar { height }).expect("encode dummy Native evidence");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            height,
            &payload,
            "dummy Native AMX evidence",
            FsyncMode::Always,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert!(Kura::prune_indexed_sidecars_to_retention_window(
            &data_path,
            &index_path,
            retention,
            "dummy Native AMX evidence",
        ));
    }
    let mut index = std::fs::File::open(&index_path).expect("open compact Native index");
    let index_len = index.metadata().expect("compact index metadata").len();
    let layout = SidecarIndexLayout::read_from(&mut index, index_len)
        .expect("decode compact Native index layout");
    assert_eq!(layout.base_height, 43);
    assert_eq!(layout.entry_count, retention.get() as u64);
    assert_eq!(
        index_len,
        INDEXED_SIDECAR_BASE_HEADER_SIZE_U64
            + u64::try_from(retention.get()).expect("retention fits u64")
                * PIPELINE_INDEX_ENTRY_SIZE_U64,
        "Native retention must bound historical index slots as well as payload bytes"
    );
    for height in 41_u64..=42 {
        assert!(
            Kura::read_indexed_sidecar_from_paths(
                height,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "dummy Native AMX evidence",
            )
            .is_none(),
            "height {height} must be outside the retained Native window"
        );
    }
    for height in 43_u64..=44 {
        assert_eq!(
            Kura::read_indexed_sidecar_from_paths(
                height,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "dummy Native AMX evidence",
            ),
            Some(DummySidecar { height }),
        );
    }
    let sparse_height = 44_u64
        .checked_add(MAX_INDEXED_SIDECAR_GAP_ENTRIES)
        .and_then(|height| height.checked_add(1))
        .expect("focused Native sparse height");
    for kind in [
        NativeAmxEvidenceKind::Manifest,
        NativeAmxEvidenceKind::Receipt,
    ] {
        let path = PathBuf::from(Kura::native_amx_evidence_file_name(kind, sparse_height));
        assert_eq!(
            Kura::parse_native_amx_evidence_path(&path)
                .expect("parse canonical sparse Native evidence path"),
            Some((kind, sparse_height, false)),
            "Native standalone evidence must not allocate dense placeholders across a gap larger than 4,096 heights"
        );
        assert_eq!(
            path.file_name()
                .and_then(|name| name.to_str())
                .expect("canonical sparse Native filename")
                .len(),
            kind.prefix().len()
                + NATIVE_AMX_EVIDENCE_HEIGHT_DIGITS
                + NATIVE_AMX_EVIDENCE_FILE_SUFFIX.len(),
            "Native sparse storage cost must be independent of the participant-height gap"
        );
    }
}
#[test]
fn terminal_frontier_compaction_retains_every_later_pending_slot() {
    let temp_dir = TempDir::new().expect("temporary sidecar directory");
    let data_path = temp_dir.path().join("terminal-history.norito");
    let index_path = temp_dir.path().join("terminal-history.index");
    let retention = NonZeroUsize::new(32).expect("non-zero terminal retention");
    for height in 1_u64..=600 {
        let payload =
            norito::to_bytes(&DummySidecar { height }).expect("encode dummy lane history");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            height,
            &payload,
            "dummy terminal lane history",
            FsyncMode::Always,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
    }
    assert!(Kura::prune_indexed_sidecars_through_terminal_frontier(
        &data_path,
        &index_path,
        550,
        retention,
        "dummy terminal lane history",
    ));
    let mut index = std::fs::File::open(&index_path).expect("open compact terminal index");
    let index_len = index
        .metadata()
        .expect("compact terminal index metadata")
        .len();
    let layout = SidecarIndexLayout::read_from(&mut index, index_len)
        .expect("decode compact terminal index layout");
    assert_eq!(layout.base_height, 519);
    assert_eq!(layout.entry_count, 82);
    assert!(
        Kura::read_indexed_sidecar_from_paths(
            518,
            &data_path,
            &index_path,
            norito::decode_from_bytes::<DummySidecar>,
            "dummy terminal lane history",
        )
        .is_none()
    );
    for height in [519_u64, 550, 551, 600] {
        assert_eq!(
            Kura::read_indexed_sidecar_from_paths(
                height,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<DummySidecar>,
                "dummy terminal lane history",
            ),
            Some(DummySidecar { height }),
            "terminal diagnostics and every post-frontier slot must survive"
        );
    }
}
#[test]
fn terminal_frontier_compaction_fails_before_replacing_malformed_pending_slot() {
    let temp_dir = TempDir::new().expect("temporary sidecar directory");
    let data_path = temp_dir.path().join("terminal-malformed.norito");
    let index_path = temp_dir.path().join("terminal-malformed.index");
    for height in 1_u64..=3 {
        let payload =
            norito::to_bytes(&DummySidecar { height }).expect("encode dummy lane history");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            height,
            &payload,
            "dummy malformed terminal history",
            FsyncMode::Always,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
    }
    let mut index = std::fs::OpenOptions::new()
        .write(true)
        .open(&index_path)
        .expect("open terminal index for corruption");
    index
        .seek(SeekFrom::Start(2 * PIPELINE_INDEX_ENTRY_SIZE_U64 + 8_u64))
        .expect("seek to pending entry length");
    index
        .write_all(&STRICT_INIT_MAX_BLOCK_BYTES.saturating_add(1).to_le_bytes())
        .expect("forge oversized pending entry");
    index.sync_all().expect("sync forged pending entry");
    drop(index);
    let corrupted_index = std::fs::read(&index_path).expect("read forged index");
    let original_data = std::fs::read(&data_path).expect("read original data");
    assert!(
        !Kura::prune_indexed_sidecars_through_terminal_frontier(
            &data_path,
            &index_path,
            2,
            NonZeroUsize::new(1).expect("non-zero retention"),
            "dummy malformed terminal history",
        ),
        "terminal compaction must fail closed instead of dropping a later pending slot"
    );
    assert_eq!(
        std::fs::read(&index_path).expect("read preserved forged index"),
        corrupted_index
    );
    assert_eq!(
        std::fs::read(&data_path).expect("read preserved data"),
        original_data
    );
    assert!(!data_path.with_extension("norito.tmp").exists());
    assert!(!index_path.with_extension("index.tmp").exists());
}
#[test]
fn terminal_auxiliary_cleanup_resumes_after_each_mutation_budget() {
    let temp_dir = TempDir::new().expect("temporary Kura directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let lane = lane_config.primary();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let artifact_dir = Kura::lane_artifact_dir(&lane.blocks_dir(temp_dir.path()));
    std::fs::create_dir_all(&artifact_dir).expect("create lane artifact directory");
    let paths = (1_u64..=3)
        .map(|height| {
            Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
                lane,
                temp_dir.path(),
                height,
                height,
            )
        })
        .collect::<Vec<_>>();
    for path in &paths {
        std::fs::write(path, []).expect("stage terminal auxiliary file");
    }
    let _guard = kura.sidecar_lock.lock();
    assert!(
        !kura
            .remove_terminal_autonomous_auxiliary_files_with_budget_locked(lane, 3, 2)
            .expect("first bounded cleanup pass"),
        "exhausting a per-pass budget must request resumption, not fail permanently"
    );
    assert_eq!(
        paths.iter().filter(|path| path.exists()).count(),
        1,
        "the first pass must durably remove exactly its mutation budget"
    );
    assert!(
        kura.remove_terminal_autonomous_auxiliary_files_with_budget_locked(lane, 3, 2)
            .expect("resumed bounded cleanup pass"),
        "a later pass must finish the remaining terminal namespace"
    );
    assert!(paths.iter().all(|path| !path.exists()));
}
