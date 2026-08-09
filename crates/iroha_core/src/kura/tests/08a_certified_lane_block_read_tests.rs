// Certified lane-block read and sparse-height regression tests.

#[test]
fn certified_lane_block_read_rejects_qc_signature_mismatch() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let (session, signer_pops) = sample_committed_lane_block_session_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified lane block");
    let mut tampered = CertifiedLaneBlockArtifact::new(session, signer_pops);
    tampered.commit_qc.bls_aggregate_signature[0] ^= 0x01;
    let payload = tampered
        .encode_framed()
        .expect("encode tampered certified lane block");
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "certified lane block",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "tampered sidecar overwrite should be written for read rejection test"
    );

    assert!(
        kura.read_certified_lane_block_artifact(lane_id, lane_block_height)
            .is_none(),
        "certified lane block reads must reject invalid QC aggregate signatures"
    );
}

#[test]
fn certified_lane_block_read_rejects_qc_body_mismatch() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let (session, signer_pops) = sample_committed_lane_block_session_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified lane block");
    let mut tampered = CertifiedLaneBlockArtifact::new(session, signer_pops);
    tampered.commit_qc.body.descriptor_hash = Hash::new(b"tampered descriptor");
    let payload = tampered
        .encode_framed()
        .expect("encode tampered certified lane block");
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "certified lane block",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "tampered sidecar overwrite should be written for read rejection test"
    );

    assert!(
        kura.read_certified_lane_block_artifact(lane_id, lane_block_height)
            .is_none(),
        "certified lane block reads must reject QC bodies that drift from the proposal"
    );
}

#[test]
fn latest_lane_block_artifact_returns_highest_valid_height() {
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
    let expected = later
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(first).expect("store first lane artifact");
    kura.store_block(later)
        .expect("store sparse later artifact");

    let latest = kura
        .latest_lane_block_artifact(lane_id)
        .expect("latest lane block artifact");
    assert_eq!(latest.ownership, expected);
    assert_eq!(latest.ownership.lane_block_height, 3);
    assert!(
        kura.read_lane_block_artifact(lane_id, 2).is_none(),
        "sparse placeholder entries must not decode as artifacts"
    );
}
