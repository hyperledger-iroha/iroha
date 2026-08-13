#[test]
fn canonical_lane_ownership_crosses_strict_barriers_before_batched_block_commit() {
    for (label, inject_failure) in strict_indexed_sidecar_failure_modes() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        assert_eq!(
            config.fsync_mode,
            FsyncMode::Batched,
            "fixture must exercise the shipped batched fsync mode"
        );
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let lane_block_height = 1;
        let block = dummy_block_with_lane_payload_ownership(
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        );
        let block_hash = block.hash();
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        inject_failure();
        assert!(
            kura.store_block(Arc::clone(&block)).is_err(),
            "injected {label} lane-ownership barrier failure unexpectedly stored block"
        );
        assert_eq!(
            kura.blocks_count(),
            0,
            "a canonical block must not publish after its {label} ownership barrier fails"
        );
        assert_eq!(
            kura.get_durable_block_hash(nonzero!(1_usize)),
            None,
            "the durable block journal must not outrun lane ownership"
        );
        assert_eq!(
            kura.read_lane_block_artifact(lane_id, lane_block_height),
            None,
            "failed pre-commit ownership staging must roll back durably"
        );
        kura.store_block(block)
            .unwrap_or_else(|error| panic!("retry after {label} barrier failure: {error:?}"));
        assert_eq!(
            kura.get_durable_block_hash(nonzero!(1_usize)),
            Some(block_hash)
        );
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_some(),
            "successful canonical publication must retain its strict ownership sidecar"
        );
    }
}
