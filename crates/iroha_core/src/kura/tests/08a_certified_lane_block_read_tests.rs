// Certified lane-block read and sparse-height regression tests.
#[test]
fn certified_lane_block_persists_under_lane_segment_and_reloads() {
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
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("init Kura");
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "a certified session must not define an uninitialized lane incarnation",
    );
    kura.install_lane_incarnation_marker_for_test(
        lane_entry,
        session.proposal.descriptor.lane_incarnation,
        session.proposal.descriptor.proposal_height,
    )
    .expect("install certified-session activation fence");
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "a certified session at the incarnation activation height must be rejected",
    );
    kura.install_lane_incarnation_marker_for_test(
        lane_entry,
        session.proposal.descriptor.lane_incarnation,
        0,
    )
    .expect("install explicit certified-session marker");
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified lane block");
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("duplicate certified lane block persistence is idempotent");
    let artifact = kura
        .read_certified_lane_block_artifact(lane_id, lane_block_height)
        .expect("certified lane block");
    assert_eq!(artifact.format_label(), "lane.certified_block");
    assert_eq!(artifact.proposal, session.proposal);
    assert_eq!(artifact.prepare_qc, session.prepare_qc);
    assert_eq!(artifact.commit_qc, session.commit_qc);
    assert_eq!(artifact.signer_pops, signer_pops);
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        data_path.is_file(),
        "certified lane block data file missing"
    );
    assert!(
        index_path.is_file(),
        "certified lane block index file missing"
    );
    drop(kura);
    let (reloaded, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen kura");
    assert_eq!(
        reloaded.read_certified_lane_block_artifact(lane_id, lane_block_height),
        Some(artifact)
    );
}
#[test]
fn latest_certified_frontier_reloads_and_repairs_a_missing_progress_pair() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        3,
        30,
    );
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone())
    );
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    fs::remove_file(&data_path).expect("remove ordinary certified data");
    fs::remove_file(&index_path).expect("remove ordinary certified index");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "the durable frontier must redo its exact ordinary pair"
    );
    assert_eq!(
        kura.read_certified_lane_block_artifact(lane_id, 3),
        Some(expected.clone())
    );
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen Kura");
    assert_eq!(
        reopened.latest_certified_lane_block_frontier(lane_id),
        Some(expected)
    );
}
#[test]
fn unchanged_latest_certified_frontier_does_not_repeat_pair_fsync() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "the first read must strictly attest the ordinary pair"
    );
    fail_next_indexed_sidecar_data_sync_for_tests();
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected),
        "an unchanged process-local attestation must avoid a repeated pair fsync"
    );
    let (data_path, _) = Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    let data = fs::File::open(data_path).expect("open certified pair data");
    assert!(
        sync_indexed_sidecar_data(&data).is_err(),
        "the cached frontier read must leave the injected fsync fault unconsumed"
    );
}
#[test]
fn unchanged_latest_certified_frontier_does_not_repeat_bls_validation() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "first read must perform full artifact validation"
    );
    fail_next_certified_lane_block_artifact_validation_for_tests();
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "exact stable frontier identity must reuse its bounded BLS attestation"
    );
    assert_eq!(
        Kura::validate_certified_lane_block_artifact(&expected),
        Err("injected certified lane block artifact validation failure"),
        "the unchanged cached read must leave the injected validation fault unconsumed"
    );
}
#[test]
fn latest_certified_matching_reuses_attested_frontier_before_history_scan() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "prime the exact frontier validation attestation"
    );
    fail_next_certified_lane_block_artifact_validation_for_tests();
    assert_eq!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| {
            let geometry_guard = kura
                .lane_geometry_lock
                .try_lock()
                .expect("frontier predicate must run without lane_geometry_lock");
            let sidecar_guard = kura
                .sidecar_lock
                .try_lock()
                .expect("frontier predicate must run without sidecar_lock");
            drop(sidecar_guard);
            drop(geometry_guard);
            true
        }),
        Some(expected.clone()),
        "matching must return the attested frontier without validating historical sidecars"
    );
    assert_eq!(
        Kura::validate_certified_lane_block_artifact(&expected),
        Err("injected certified lane block artifact validation failure"),
        "the frontier short-circuit must leave historical validation untouched"
    );
}
#[test]
fn latest_certified_frontier_validation_attestation_is_exact_artifact_bound() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected),
        "first read must validate and attest the exact artifact"
    );
    let (frontier_path, _) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane_entry, temp_dir.path());
    let stored = fs::read(&frontier_path).expect("read attested frontier");
    let mut frontier = norito::decode_from_bytes::<LatestCertifiedLaneBlockFrontierV1>(&stored)
        .expect("decode attested frontier");
    *frontier
        .artifact
        .commit_qc
        .bls_aggregate_signature
        .first_mut()
        .expect("valid commit aggregate signature is nonempty") ^= 1;
    let invalid = LatestCertifiedLaneBlockFrontierV1::new(frontier.artifact)
        .expect("seal structurally canonical invalid-proof frontier");
    fs::write(
        &frontier_path,
        norito::to_bytes(&invalid).expect("encode invalid-proof frontier"),
    )
    .expect("replace frontier with an invalid proof");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        None,
        "a different artifact hash must never reuse the prior BLS validation attestation"
    );
}
#[test]
fn latest_certified_frontier_rejects_equal_height_conflict_before_publication() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (first, first_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        10,
    );
    let (conflict, conflict_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        11,
    );
    let (older_conflict, older_conflict_pops) =
        sample_committed_lane_block_session_at_proposal_height_for_kura(
            lane_id,
            lane_entry.dataspace_id,
            1,
            9,
        );
    let expected = CertifiedLaneBlockArtifact::new(first.clone(), first_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&first, &first_pops)
        .expect("persist first certificate");
    assert!(
        kura.persist_committed_lane_block_session(&conflict, &conflict_pops)
            .is_err(),
        "a distinct proposal at an occupied lane height must fail before frontier publication"
    );
    assert!(
        kura.persist_committed_lane_block_session(&older_conflict, &older_conflict_pops,)
            .is_err(),
        "equal lane height must conflict even when the distinct proposal has a lower global height"
    );
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone())
    );
    let conflicting_artifact = CertifiedLaneBlockArtifact::new(conflict, conflict_pops);
    let conflicting_payload = conflicting_artifact
        .encode_framed()
        .expect("encode conflicting certificate");
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    assert!(Kura::append_indexed_sidecar(
        &data_path,
        &index_path,
        1,
        &conflicting_payload,
        "certified lane block conflict fixture",
        FsyncMode::Always,
        None,
    ));
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        None,
        "a conflicting active ordinary slot must not be silently repaired without reset authority"
    );
    assert_ne!(
        kura.read_certified_lane_block_artifact(lane_id, 1),
        Some(expected)
    );
}
#[test]
fn latest_certified_frontier_reset_authority_crosses_height_and_repairs_crash() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (old_slot, old_slot_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        90,
    );
    let (old_tip, old_tip_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        513,
        100,
    );
    let (fresh, fresh_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        101,
    );
    let authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        lane_id,
        lane_entry.dataspace_id,
        fresh.proposal.descriptor.lane_incarnation,
        Some(100),
    );
    let expected = CertifiedLaneBlockArtifact::new(fresh.clone(), fresh_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&old_slot, &old_slot_pops)
        .expect("persist pre-reset occupied slot");
    kura.persist_committed_lane_block_session(&old_tip, &old_tip_pops)
        .expect("persist high pre-reset tip");
    fail_next_bound_progress_append_data_sync_for_tests();
    assert!(
        kura.persist_committed_lane_block_session_with_authority(&fresh, &fresh_pops, &authority,)
            .is_err(),
        "fault must interrupt after the lower post-reset frontier wins but before pair replacement"
    );
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen after frontier crash");
    assert_eq!(
        reopened.latest_certified_lane_block_frontier_with_authority(lane_id, &authority,),
        Some(expected.clone()),
        "State-authenticated reset authority must repair the reused lower slot after restart"
    );
    assert_eq!(
        reopened.read_certified_lane_block_artifact(lane_id, 1),
        Some(expected)
    );
}
#[test]
fn read_only_certified_frontier_preflight_plans_reused_slot_without_mutation() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (old_slot, old_slot_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        90,
    );
    let (old_tip, old_tip_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        513,
        100,
    );
    let (fresh, fresh_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        101,
    );
    let authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        lane_id,
        lane_entry.dataspace_id,
        fresh.proposal.descriptor.lane_incarnation,
        Some(100),
    );
    let expected = CertifiedLaneBlockArtifact::new(fresh.clone(), fresh_pops.clone());
    let old_artifact = CertifiedLaneBlockArtifact::new(old_slot.clone(), old_slot_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&old_slot, &old_slot_pops)
        .expect("persist pre-reset occupied slot");
    kura.persist_committed_lane_block_session(&old_tip, &old_tip_pops)
        .expect("persist high pre-reset tip");
    fail_next_bound_progress_append_data_sync_for_tests();
    assert!(
        kura.persist_committed_lane_block_session_with_authority(&fresh, &fresh_pops, &authority,)
            .is_err(),
        "fixture must leave the fresh frontier over the stale ordinary slot"
    );
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen after frontier crash");
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    let (frontier_path, build_path) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane_entry, temp_dir.path());
    let before = [
        fs::read(&data_path).expect("read ordinary data before preflight"),
        fs::read(&index_path).expect("read ordinary index before preflight"),
        fs::read(&frontier_path).expect("read frontier before preflight"),
    ];
    assert!(!build_path.exists());
    let revision = reopened.committed_lane_status_revision();
    let planned = reopened
        .preflight_latest_certified_lane_block_frontier_with_authority(lane_id, &authority)
        .expect("read-only frontier preflight")
        .expect("fresh frontier");
    assert_eq!(planned, (expected, true));
    assert_eq!(
        reopened
            .read_certified_lane_block_artifact_read_only(lane_id, 1)
            .expect("read stale ordinary slot without recovery"),
        Some(old_artifact),
    );
    assert_eq!(
        before,
        [
            fs::read(&data_path).expect("read ordinary data after preflight"),
            fs::read(&index_path).expect("read ordinary index after preflight"),
            fs::read(&frontier_path).expect("read frontier after preflight"),
        ],
        "read-only planning must not repair or rewrite Kura bytes",
    );
    assert_eq!(
        reopened.committed_lane_status_revision(),
        revision,
        "read-only planning must not publish a status generation",
    );
    assert!(!build_path.exists());
}
#[test]
fn latest_certified_frontier_absence_never_bootstraps_from_ordinary_history() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certificate");
    let (frontier_path, _) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane_entry, temp_dir.path());
    fs::remove_file(&frontier_path).expect("remove mandatory frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        None,
        "frontier reads must not fall back to reverse ordinary history"
    );
    assert_eq!(
        kura.read_certified_lane_block_artifact(lane_id, 1),
        Some(expected),
        "fixture must retain valid ordinary history"
    );
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "a nonempty ordinary pair without its frontier is unsupported, not a migration source"
    );
    assert!(!frontier_path.exists());
}
#[test]
fn latest_certified_frontier_corruption_and_post_validation_substitution_fail_closed() {
    let make_kura = || {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry").clone();
        let (session, signer_pops) =
            sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .expect("persist certificate");
        (temp_dir, lane_config, lane_entry, lane_id, kura)
    };
    let (corrupt_dir, _corrupt_config, corrupt_entry, corrupt_lane, corrupt_kura) = make_kura();
    let (corrupt_path, _) = Kura::latest_certified_lane_block_frontier_paths_for_entry(
        &corrupt_entry,
        corrupt_dir.path(),
    );
    let mut noncanonical = fs::read(&corrupt_path).expect("read frontier");
    noncanonical.push(0);
    fs::write(&corrupt_path, noncanonical).expect("write noncanonical frontier");
    assert_eq!(
        corrupt_kura.latest_certified_lane_block_frontier(corrupt_lane),
        None
    );
    let (substitute_dir, _substitute_config, substitute_entry, substitute_lane, substitute_kura) =
        make_kura();
    let (substitute_path, _) = Kura::latest_certified_lane_block_frontier_paths_for_entry(
        &substitute_entry,
        substitute_dir.path(),
    );
    let hook_path = substitute_path.clone();
    set_latest_certified_frontier_post_validation_hook_for_tests(move || {
        let mut bytes = fs::read(&hook_path).expect("read authenticated frontier");
        let last = bytes.last_mut().expect("frontier is nonempty");
        *last ^= 1;
        fs::write(&hook_path, bytes).expect("substitute frontier after validation");
    });
    assert_eq!(
        substitute_kura.latest_certified_lane_block_frontier(substitute_lane),
        None,
        "exact post-BLS reread must reject in-place substitution"
    );
    assert!(
        substitute_kura
            .latest_certified_frontier_storage_unknown
            .load(Ordering::Acquire),
        "post-authentication ambiguity must fail-stop the live frontier"
    );
}
#[cfg(unix)]
#[test]
fn latest_certified_frontier_rejects_hardlink_and_symlink_paths() {
    use std::os::unix::fs::symlink;
    for hardlink in [true, false] {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let (session, signer_pops) =
            sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .expect("persist certificate");
        let (frontier_path, _) =
            Kura::latest_certified_lane_block_frontier_paths_for_entry(lane_entry, temp_dir.path());
        let attacker_path = frontier_path.with_extension("attacker");
        if hardlink {
            fs::hard_link(&frontier_path, &attacker_path).expect("add a second hard link");
        } else {
            fs::rename(&frontier_path, &attacker_path).expect("move frontier to attacker path");
            symlink(&attacker_path, &frontier_path).expect("substitute frontier symlink");
        }
        assert_eq!(
            kura.latest_certified_lane_block_frontier(lane_id),
            None,
            "frontier must reject non-single-link or symlink storage"
        );
    }
}
#[test]
fn certified_lane_block_encoding_enforces_source_envelope() {
    let lane_id = LaneId::from(1);
    let dataspace_id = DataSpaceId::new(7);
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, dataspace_id, 1);
    let mut artifact = CertifiedLaneBlockArtifact::new(session, signer_pops);
    assert!(
        artifact.encode_framed().is_ok(),
        "a normal certified lane source must fit its reserved envelope"
    );
    artifact.commit_qc.bls_aggregate_signature =
        vec![0xA5; MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES];
    assert!(
        artifact.encode_framed().is_err(),
        "an oversized certified source must fail before persistence or recovery fanout"
    );
    assert_eq!(
        Kura::validate_certified_lane_block_artifact(&artifact),
        Err("certified lane block exceeds the merge source envelope byte limit")
    );
}
fn certified_lane_block_strict_retry_reissues_every_barrier() {
    for (label, failure) in strict_progress_sidecar_failure_modes() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        assert_eq!(
            config.fsync_mode,
            FsyncMode::Batched,
            "fixture must prove the certificate overrides ordinary batched durability"
        );
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let lane_block_height = 1;
        let (session, signer_pops) = sample_committed_lane_block_session_for_kura(
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        );
        let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("init Kura");
        kura.install_lane_incarnation_marker_for_test(
            lane_entry,
            session.proposal.descriptor.lane_incarnation,
            0,
        )
        .expect("install explicit certified-session marker");
        let (data_path, index_path) =
            Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
        failure.inject();
        assert!(
            kura.persist_committed_lane_block_session(&session, &signer_pops)
                .is_err(),
            "injected {label} barrier failure must reject certificate persistence"
        );
        let readable = Kura::read_indexed_sidecar_from_paths::<CertifiedLaneBlockArtifact, _>(
            lane_block_height,
            &data_path,
            &index_path,
            norito::decode_from_bytes::<CertifiedLaneBlockArtifact>,
            "certified lane block",
        )
        .expect("failed barrier leaves exact page-cache certificate bytes readable");
        assert_eq!(readable, expected);
        let first_data_len = fs::metadata(&data_path)
            .expect("certified lane data metadata")
            .len();
        drop(kura);
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("reopen Kura after fault");
        failure.inject();
        assert_eq!(
            kura.read_certified_lane_block_artifact(lane_id, lane_block_height),
            None,
            "a reopened public reader must not expose a certificate while its {label} barrier fails"
        );
        failure.inject();
        assert!(
            kura.persist_committed_lane_block_session(&session, &signer_pops)
                .is_err(),
            "exact-existing certificate retry must reissue the {label} barrier"
        );
        assert_eq!(
            fs::metadata(&data_path)
                .expect("certified lane data metadata")
                .len(),
            first_data_len,
            "failed exact certificate retry must not append duplicate bytes"
        );
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .expect("certificate retry after barrier recovery");
        assert_eq!(
            fs::metadata(&data_path)
                .expect("certified lane data metadata")
                .len(),
            first_data_len,
            "successful exact certificate retry must not append duplicate bytes"
        );
        assert_eq!(
            kura.read_certified_lane_block_artifact(lane_id, lane_block_height),
            Some(expected),
            "certificate must become observable after every strict barrier succeeds"
        );
    }
}
#[test]
fn certified_lane_block_rejects_foreign_active_dataspace() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (active, active_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 2);
    let (foreign, foreign_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, DataSpaceId::new(77), 3);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&active, &active_pops)
        .expect("persist active certified lane block");
    assert!(
        kura.persist_committed_lane_block_session(&foreign, &foreign_pops)
            .is_err(),
        "a certified session must not define the dataspace of active lane storage"
    );
    let latest = kura
        .latest_certified_lane_block_artifact_for_dataspace(lane_id, lane_entry.dataspace_id)
        .expect("latest certified active lane block");
    assert_eq!(latest.proposal, active.proposal);
    assert_eq!(latest.proposal.descriptor.lane_block_height, 2);
}
#[test]
fn certified_lane_block_artifacts_for_dataspace_replays_ordered_active_backlog() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (first, first_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let (second, second_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 2);
    let (foreign, foreign_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, DataSpaceId::new(77), 3);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&first, &first_pops)
        .expect("persist first active certified lane block");
    kura.persist_committed_lane_block_session(&second, &second_pops)
        .expect("persist second active certified lane block");
    assert!(
        kura.persist_committed_lane_block_session(&foreign, &foreign_pops)
            .is_err(),
        "foreign-dataspace history must be rejected before entering the active segment"
    );
    let active =
        kura.certified_lane_block_artifacts_for_dataspace(lane_id, lane_entry.dataspace_id);
    assert_eq!(
        active
            .iter()
            .map(|artifact| artifact.proposal.descriptor.lane_block_height)
            .collect::<Vec<_>>(),
        vec![1, 2],
        "all active certified lane blocks should replay in lane-local height order"
    );
    assert_eq!(active[0].proposal, first.proposal);
    assert_eq!(active[1].proposal, second.proposal);
    let latest = kura
        .latest_certified_lane_block_artifact_for_dataspace(lane_id, lane_entry.dataspace_id)
        .expect("latest certified active lane block");
    assert_eq!(latest.proposal, second.proposal);
    let first_from_two = kura
        .first_certified_lane_block_artifact_matching_from(lane_id, 2, |artifact| {
            artifact.proposal.descriptor.dataspace_id == lane_entry.dataspace_id
        })
        .expect("first active certified block from lower bound");
    assert_eq!(first_from_two.proposal, second.proposal);
    assert!(
        kura.first_certified_lane_block_artifact_matching_from(lane_id, 3, |artifact| artifact
            .proposal
            .descriptor
            .dataspace_id
            == lane_entry.dataspace_id,)
            .is_none(),
        "a rejected foreign height must not appear in the active backlog"
    );
    let lifecycle_filtered = kura.certified_lane_block_artifacts_matching(lane_id, |artifact| {
        artifact.proposal.descriptor.dataspace_id == lane_entry.dataspace_id
            && artifact.proposal.descriptor.lane_block_height == 2
    });
    assert_eq!(lifecycle_filtered.len(), 1);
    assert_eq!(lifecycle_filtered[0].proposal, second.proposal);
    let reverse_filtered = kura
        .latest_certified_lane_block_artifact_matching(lane_id, |artifact| {
            artifact.proposal.descriptor.dataspace_id == lane_entry.dataspace_id
                && artifact.proposal.descriptor.lane_block_height < 2
        })
        .expect("reverse scan should continue past rejected newer sidecars");
    assert_eq!(reverse_filtered.proposal, first.proposal);
    let bounded_latest =
        kura.latest_certified_lane_block_artifacts_matching(lane_id, 1, |artifact| {
            artifact.proposal.descriptor.dataspace_id == lane_entry.dataspace_id
        });
    assert_eq!(bounded_latest.len(), 1);
    assert_eq!(bounded_latest[0].proposal, second.proposal);
    assert!(
        kura.latest_certified_lane_block_artifacts_matching(lane_id, 0, |_| true)
            .is_empty(),
        "a zero recovery budget must not scan certified history"
    );
}
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
        .expect("read canonical lane frontier")
        .expect("latest lane block artifact");
    assert_eq!(latest.ownership, expected);
    assert_eq!(latest.ownership.lane_block_height, 3);
    assert!(
        kura.read_lane_block_artifact(lane_id, 2).is_none(),
        "sparse placeholder entries must not decode as artifacts"
    );
}

#[test]
fn consensus_certificate_read_rejects_occupied_corruption_without_repair() {
    let (temp_dir, config, lane_config) = two_lane_storage_fixture();
    let lane_id = LaneId::from(1);
    let lane = lane_config.entry(lane_id).expect("configured lane");
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    assert!(
        kura.read_certified_lane_block_artifact_read_only(lane_id, 1)
            .expect("empty certificate slot")
            .is_none()
    );
    let (session, pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
    kura.persist_committed_lane_block_session(&session, &pops)
        .expect("persist quorum certificate");
    assert!(
        kura.read_lane_completion_certificate(lane_id, 1)
            .expect("attest exact certificate")
            .is_some()
    );
    let (data_path, index_path) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    fs::write(&data_path, b"occupied corrupted certificate").expect("corrupt durable certificate");
    let before = (
        fs::read(&data_path).expect("data evidence"),
        fs::read(&index_path).expect("index evidence"),
    );
    assert!(
        kura.read_certified_lane_block_artifact_read_only(lane_id, 1)
            .is_err()
    );
    assert!(kura.read_lane_completion_certificate(lane_id, 1).is_err());
    assert_eq!(
        (
            fs::read(data_path).expect("retained data"),
            fs::read(index_path).expect("retained index")
        ),
        before
    );
}
