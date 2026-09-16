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
    publish_initial_configured_lane_geometry_for_test(
        &kura,
        &lane_config,
        &BTreeMap::from([(lane_id, session.proposal.descriptor.lane_incarnation)]),
    );
    kura.replace_lane_storage_entries_for_test(&lane_config);
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
    let mut incarnations = BTreeMap::new();
    let mut activations = BTreeMap::new();
    for entry in lane_config.entries() {
        let (incarnation, activation) = kura
            .active_lane_incarnation_marker(entry)
            .expect("retain exact State fixture geometry before restart");
        incarnations.insert(entry.lane_id, incarnation);
        activations.insert(entry.lane_id, activation);
    }
    drop(kura);
    let (reloaded, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen kura");
    assert!(
        reloaded.lane_storage_entry(lane_id).is_err(),
        "secondary storage remains inactive until exact catalog recovery"
    );
    reloaded
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
        .expect("recover the retained State fixture geometry before secondary reads");
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
    let mut incarnations = BTreeMap::new();
    let mut activations = BTreeMap::new();
    for entry in lane_config.entries() {
        let (incarnation, activation) = kura
            .active_lane_incarnation_marker(entry)
            .expect("retain exact State fixture geometry before restart");
        incarnations.insert(entry.lane_id, incarnation);
        activations.insert(entry.lane_id, activation);
    }
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen Kura");
    assert!(
        reopened.lane_storage_entry(lane_id).is_err(),
        "secondary storage remains inactive until exact catalog recovery"
    );
    reopened
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
        .expect("recover the retained State fixture geometry before secondary reads");
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
        })
        .expect("authenticate certified storage"),
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
    let mut incarnations = BTreeMap::new();
    let mut activations = BTreeMap::new();
    for entry in lane_config.entries() {
        let (incarnation, activation) = kura
            .active_lane_incarnation_marker(entry)
            .expect("retain exact State fixture geometry before restart");
        incarnations.insert(entry.lane_id, incarnation);
        activations.insert(entry.lane_id, activation);
    }
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen after frontier crash");
    assert!(
        reopened.lane_storage_entry(lane_id).is_err(),
        "secondary storage remains inactive until exact catalog recovery"
    );
    reopened
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
        .expect("recover the retained State fixture geometry before secondary reads");
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    let (frontier_path, _) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane_entry, temp_dir.path());
    let intent_path = Kura::bound_progress_append_intent_path(&index_path);
    let snapshot = || {
        [
            fs::read(&data_path).expect("pending append data"),
            fs::read(&index_path).expect("pending append index"),
            fs::read(&frontier_path).expect("pending append frontier"),
            fs::read(&intent_path).expect("pending append intent"),
        ]
    };
    let before_passive_reads = snapshot();
    assert!(
        reopened
            .preflight_latest_certified_lane_block_frontier_with_authority(lane_id, &authority)
            .is_err(),
        "passive planning cannot recover the append journal"
    );
    assert!(
        reopened
            .read_certified_lane_block_artifact_read_only(lane_id, 1)
            .is_err()
    );
    assert_eq!(snapshot(), before_passive_reads);
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
    // Model the crash boundary after frontier durability, before the ordinary
    // append intent exists. A data-sync failure instead leaves a pending append
    // journal, which passive preflight must reject until its writer recovers it.
    {
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        Kura::validate_certified_lane_block_artifact(&expected)
            .expect("fixture uses a fully valid native certificate");
        assert!(
            kura.publish_latest_certified_lane_block_frontier_locked(
                lane_entry,
                &expected,
                Some(&authority),
            )
            .expect("publish only the State-authorized frontier crash image")
        );
    }
    let mut incarnations = BTreeMap::new();
    let mut activations = BTreeMap::new();
    for entry in lane_config.entries() {
        let (incarnation, activation) = kura
            .active_lane_incarnation_marker(entry)
            .expect("retain exact fixture geometry before restart");
        incarnations.insert(entry.lane_id, incarnation);
        activations.insert(entry.lane_id, activation);
    }
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen after frontier crash");
    reopened
        .recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
        .expect("recover the retained geometry before testing certificate barriers");
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
        publish_initial_configured_lane_geometry_for_test(
            &kura,
            &lane_config,
            &BTreeMap::from([(lane_id, session.proposal.descriptor.lane_incarnation)]),
        );
        kura.replace_lane_storage_entries_for_test(&lane_config);
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
        let mut incarnations = BTreeMap::new();
        let mut activations = BTreeMap::new();
        for entry in lane_config.entries() {
            let (incarnation, activation) = kura
                .active_lane_incarnation_marker(entry)
                .expect("retain exact fixture geometry before restart");
            incarnations.insert(entry.lane_id, incarnation);
            activations.insert(entry.lane_id, activation);
        }
        drop(kura);
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("reopen Kura after fault");
        kura.recover_lane_geometry_journal(&lane_config, &incarnations, &activations)
            .expect("recover the retained geometry before testing certificate barriers");
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
        .expect("authenticate certified storage")
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
        .expect("authenticate certified storage")
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
        .expect("authenticate certified storage")
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
    let (read_only, validations) = count_certified_artifact_validations_for_tests(|| {
        kura.read_certified_lane_block_artifact_read_only(lane_id, lane_block_height)
    });
    assert_eq!(read_only.unwrap().unwrap().proposal, session.proposal);
    assert_eq!(
        validations, 1,
        "strict retirement reads authenticate valid evidence"
    );
    let (completion, validations) = count_certified_artifact_validations_for_tests(|| {
        kura.read_lane_completion_certificate(lane_id, lane_block_height)
    });
    assert_eq!(completion.unwrap().unwrap().proposal, session.proposal);
    assert_eq!(
        validations, 1,
        "strict hydration reads authenticate valid evidence"
    );
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
    let retained = (
        fs::read(&data_path).unwrap(),
        fs::read(&index_path).unwrap(),
    );
    let (read_only, validations) = count_certified_artifact_validations_for_tests(|| {
        kura.read_certified_lane_block_artifact_read_only(lane_id, lane_block_height)
    });
    assert!(
        read_only.is_err(),
        "strict retirement reader must reject invalid QC evidence"
    );
    assert_eq!(
        validations, 1,
        "the strict reader authenticates the occupied certificate"
    );
    let (completion, validations) = count_certified_artifact_validations_for_tests(|| {
        kura.read_lane_completion_certificate(lane_id, lane_block_height)
    });
    assert!(
        completion.is_err(),
        "strict hydration reader must reject invalid QC evidence"
    );
    assert_eq!(
        validations, 1,
        "the strict reader authenticates the occupied certificate"
    );
    assert_eq!(
        (
            fs::read(&data_path).unwrap(),
            fs::read(&index_path).unwrap()
        ),
        retained,
        "strict readers must not repair occupied invalid evidence"
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
    let retained = (
        fs::read(&data_path).unwrap(),
        fs::read(&index_path).unwrap(),
    );
    let (read_only, validations) = count_certified_artifact_validations_for_tests(|| {
        kura.read_certified_lane_block_artifact_read_only(lane_id, lane_block_height)
    });
    assert!(
        read_only.is_err(),
        "strict retirement reader must reject invalid QC evidence"
    );
    assert_eq!(
        validations, 1,
        "the strict reader authenticates the occupied certificate"
    );
    let (completion, validations) = count_certified_artifact_validations_for_tests(|| {
        kura.read_lane_completion_certificate(lane_id, lane_block_height)
    });
    assert!(
        completion.is_err(),
        "strict hydration reader must reject invalid QC evidence"
    );
    assert_eq!(
        validations, 1,
        "the strict reader authenticates the occupied certificate"
    );
    assert_eq!(
        (
            fs::read(&data_path).unwrap(),
            fs::read(&index_path).unwrap()
        ),
        retained,
        "strict readers must not repair occupied invalid evidence"
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
fn live_certificate_completion_reuses_exact_durability_attestation() {
    let (temp_dir, config, lane_config) = two_lane_storage_fixture();
    let lane_id = LaneId::from(1);
    let lane = lane_config.entry(lane_id).expect("configured lane");
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let (session, pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), pops.clone());
    kura.persist_committed_lane_block_session(&session, &pops)
        .expect("persist certificate");
    kura.certified_pair_durability.lock().clear();
    assert_eq!(
        kura.read_lane_completion_certificate(lane_id, 1)
            .expect("first completion read performs its durability barriers"),
        Some(expected.clone())
    );
    fail_next_indexed_sidecar_data_sync_for_tests();
    assert_eq!(
        kura.read_lane_completion_certificate(lane_id, 1)
            .expect("unchanged exact completion reuses durability"),
        Some(expected)
    );
    let (data_path, _) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    let data = fs::File::open(data_path).expect("open unchanged data");
    assert!(
        sync_indexed_sidecar_data(&data).is_err(),
        "the repeated completion read must leave the injected sync failure unconsumed"
    );
}

#[test]
fn live_certificate_completion_attests_every_uncached_barrier() {
    for (label, barrier) in strict_progress_sidecar_failure_modes() {
        let (_temp_dir, config, lane_config) = two_lane_storage_fixture();
        let lane_id = LaneId::from(1);
        let lane = lane_config.entry(lane_id).expect("configured lane");
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        let (session, pops) =
            sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
        let expected = CertifiedLaneBlockArtifact::new(session.clone(), pops.clone());
        kura.persist_committed_lane_block_session(&session, &pops)
            .expect("persist certificate");
        kura.certified_pair_durability.lock().clear();
        barrier.inject();
        assert!(
            kura.read_lane_completion_certificate(lane_id, 1).is_err(),
            "cold completion read must fail at the {label} durability boundary"
        );
        assert!(
            !kura.certified_pair_durability.lock().contains_key(&lane_id),
            "failed {label} durability must not publish an attestation"
        );
        assert_eq!(
            kura.read_lane_completion_certificate(lane_id, 1)
                .expect("retry after the one-shot durability failure"),
            Some(expected),
            "{label} failure must leave the original certificate recoverable"
        );
    }
}

#[test]
fn live_certificate_completion_changed_data_requires_new_attestation() {
    let (temp_dir, config, lane_config) = two_lane_storage_fixture();
    let lane_id = LaneId::from(1);
    let lane = lane_config.entry(lane_id).expect("configured lane");
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let (session, pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), pops.clone());
    kura.persist_committed_lane_block_session(&session, &pops)
        .expect("persist certificate");
    assert_eq!(
        kura.read_lane_completion_certificate(lane_id, 1)
            .expect("prime durability"),
        Some(expected.clone())
    );
    let (data_path, _) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    let data = fs::File::options()
        .write(true)
        .open(data_path)
        .expect("open exact data");
    data.set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1))
        .expect("change data metadata while preserving every proof byte");
    fail_next_indexed_sidecar_data_sync_for_tests();
    assert!(
        kura.read_lane_completion_certificate(lane_id, 1).is_err(),
        "changed data metadata must invalidate the earlier durability attestation"
    );
    assert_eq!(
        kura.read_lane_completion_certificate(lane_id, 1)
            .expect("reattest changed metadata through the actual barriers"),
        Some(expected)
    );
}

#[test]
fn live_certificate_completion_revalidates_proof_after_durability_cache_hit() {
    let (_temp_dir, config, lane_config) = two_lane_storage_fixture();
    let lane_id = LaneId::from(1);
    let lane = lane_config.entry(lane_id).expect("configured lane");
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let (session, pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
    kura.persist_committed_lane_block_session(&session, &pops)
        .expect("persist certificate");
    assert!(
        kura.read_lane_completion_certificate(lane_id, 1)
            .expect("prime durability")
            .is_some()
    );
    fail_next_certified_lane_block_artifact_validation_for_tests();
    assert!(
        kura.read_lane_completion_certificate(lane_id, 1).is_err(),
        "a durability attestation must never replace the current read's proof validation"
    );
    assert!(
        kura.read_lane_completion_certificate(lane_id, 1)
            .expect("retry after the one-shot certificate validation failure")
            .is_some()
    );
}

#[test]
fn passive_certificate_read_does_not_mint_durability_attestation() {
    let (_temp_dir, config, lane_config) = two_lane_storage_fixture();
    let lane_id = LaneId::from(1);
    let lane = lane_config.entry(lane_id).expect("configured lane");
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let (session, pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
    kura.persist_committed_lane_block_session(&session, &pops)
        .expect("persist certificate");
    kura.certified_pair_durability.lock().clear();
    assert!(
        kura.read_certified_lane_block_artifact_read_only(lane_id, 1)
            .expect("read without durability publication")
            .is_some()
    );
    assert!(!kura.certified_pair_durability.lock().contains_key(&lane_id));
    fail_next_indexed_sidecar_data_sync_for_tests();
    assert!(
        kura.read_lane_completion_certificate(lane_id, 1).is_err(),
        "the passive read cannot authorize skipping a completion durability barrier"
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

#[test]
fn certified_frontier_recovery_preserves_occupied_corruption() {
    let (temp_dir, config, lane_config) = two_lane_storage_fixture();
    let lane_id = LaneId::from(1);
    let lane = lane_config.entry(lane_id).expect("configured lane");
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let (session, pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), pops.clone());
    let authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        lane_id,
        lane.dataspace_id,
        session.proposal.descriptor.lane_incarnation,
        None,
    );
    assert_eq!(
        kura.preflight_latest_certified_lane_block_frontier_with_authority(lane_id, &authority)
            .expect("empty certified namespace"),
        None,
    );
    kura.persist_committed_lane_block_session(&session, &pops)
        .expect("publish actual certificate and frontier");
    assert_eq!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| true)
            .expect("authenticate certified storage"),
        Some(expected.clone()),
        "warm the actual frontier and pair attestations",
    );
    let (data_path, index_path) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    let (frontier_path, _) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane, temp_dir.path());
    let healthy_data = fs::read(&data_path).expect("read exact certified bytes");
    fs::write(&data_path, vec![0xA5; healthy_data.len()])
        .expect("damage occupied payload without changing indexed geometry");
    let snapshot = || {
        [
            fs::read(&data_path).expect("read certificate data"),
            fs::read(&index_path).expect("read certificate index"),
            fs::read(&frontier_path).expect("read certificate frontier"),
        ]
    };
    let damaged = snapshot();
    assert!(
        kura.preflight_latest_certified_lane_block_frontier_with_authority(lane_id, &authority)
            .is_err(),
        "occupied corruption cannot become a planned missing-pair repair",
    );
    assert_eq!(snapshot(), damaged);
    assert!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| true)
            .is_err(),
        "a latest-frontier observation cannot recover over corrupted occupied bytes",
    );
    assert_eq!(snapshot(), damaged);
    assert!(
        kura.persist_committed_lane_block_session(&session, &pops)
            .is_err(),
        "an exact certificate retry cannot replace corrupted occupied bytes",
    );
    assert_eq!(snapshot(), damaged);
    // Without covering frontier authority, malformed occupied bytes must not
    // authorize mutable autonomous evidence as an uncertified slot.
    let healthy_frontier = fs::read(&frontier_path).expect("retain frontier control");
    fs::remove_file(&frontier_path).expect("remove covering frontier for predicate control");
    {
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        assert!(
            kura.autonomous_lane_slot_is_certified_locked(lane, 1)
                .is_err()
        );
    }
    fs::write(&frontier_path, healthy_frontier).expect("restore fixture frontier");
    assert_eq!(snapshot(), damaged);

    // Genuine missing-pair recovery remains owned by the exact retained frontier.
    fs::write(&data_path, healthy_data).expect("restore fixture evidence");
    fs::remove_file(&data_path).expect("remove complete data pair for recovery control");
    fs::remove_file(&index_path).expect("remove complete index pair for recovery control");
    assert_eq!(
        kura.preflight_latest_certified_lane_block_frontier_with_authority(lane_id, &authority)
            .expect("plan genuine missing pair"),
        Some((expected.clone(), true)),
    );
    assert_eq!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| true)
            .expect("read independently authenticated frontier without repair"),
        Some(expected.clone()),
    );
    assert!(!data_path.exists() && !index_path.exists());
    kura.persist_committed_lane_block_session(&session, &pops)
        .expect("recover a genuinely missing pair from its exact frontier");
    assert_eq!(
        kura.read_lane_completion_certificate(lane_id, 1)
            .expect("strict recovered certificate read"),
        Some(expected),
    );
}

#[test]
fn certified_latest_rejects_damaged_mandatory_frontier_without_history_fallback() {
    let (temp_dir, config, lane_config) = two_lane_storage_fixture();
    let lane_id = LaneId::from(1);
    let lane = lane_config.entry(lane_id).expect("configured lane");
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    assert!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| true)
            .expect("genuine empty namespace")
            .is_none()
    );
    let (session, pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
    kura.persist_committed_lane_block_session(&session, &pops)
        .expect("publish real certificate");
    let expected = kura
        .latest_certified_lane_block_artifact_matching(lane_id, |_| true)
        .expect("warm latest read")
        .expect("published frontier");
    let (frontier_path, _) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane, temp_dir.path());
    let (data_path, index_path) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    let healthy = fs::read(&frontier_path).expect("read frontier");
    let snapshot = || {
        [
            fs::read(&frontier_path).expect("frontier evidence"),
            fs::read(&data_path).expect("pair data"),
            fs::read(&index_path).expect("pair index"),
        ]
    };
    fs::write(&frontier_path, vec![0xA5; healthy.len()]).expect("damage mandatory frontier");
    let damaged = snapshot();
    assert!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| true)
            .is_err()
    );
    assert_eq!(
        snapshot(),
        damaged,
        "valid indexed history cannot replace mandatory frontier authority"
    );
    fs::write(&frontier_path, healthy).expect("restore fixture frontier");
    assert_eq!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| true)
            .expect("restored independent evidence"),
        Some(expected)
    );
    assert!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| {
            let mut changed = fs::read(&frontier_path).expect("read during predicate");
            changed.push(0);
            fs::write(&frontier_path, changed)
                .expect("change frontier after initial authentication");
            true
        })
        .is_err(),
        "post-predicate revalidation must reject actual frontier substitution"
    );
}

#[test]
fn certified_latest_scan_exhaustion_cannot_prove_absence() {
    let (_temp_dir, config, lane_config) = two_lane_storage_fixture();
    let lane_id = LaneId::from(1);
    let lane = lane_config.entry(lane_id).expect("configured lane");
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    for height in [
        1,
        u64::try_from(CONSENSUS_SIDECAR_MATCH_SCAN_BUDGET).expect("scan bound") + 1,
    ] {
        let (session, pops) =
            sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, height);
        kura.persist_committed_lane_block_session(&session, &pops)
            .expect("publish bounded-history control");
    }
    assert!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| true)
            .expect("exact latest frontier does not require historical traversal")
            .is_some()
    );
    assert!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |artifact| {
            artifact.proposal.descriptor.lane_block_height == 1
        })
        .is_err(),
        "a bounded miss must not authorize an apparently empty lane"
    );
}

/// Exercise the cache against an actual, durably persisted certified pair.
#[cfg(unix)]
fn with_certified_pair_ancestor_attestation_fixture(
    check: impl FnOnce(&Kura, LaneId, &CertifiedLaneBlockArtifact, BoundProgressSidecar),
) {
    let temp = TempDir::new().expect("create certified pair fixture");
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let lanes = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let entry = lanes.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, entry.dataspace_id, 1);
    let artifact = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lanes);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified pair");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(artifact.clone()),
        "prime the real durability cache through frontier recovery"
    );
    assert_eq!(
        fs::canonicalize(temp.path()).expect("resolve configured fixture root"),
        kura.store_root,
        "private sidecar binding must use Kura's resolved storage root"
    );
    let (data, index) = Kura::certified_lane_block_paths_for_entry(entry, &kura.store_root);
    let bound = kura
        .open_bound_progress_sidecar(&data, &index)
        .expect("bind the actual certified pair");
    assert_eq!(bound.namespace.directories.len(), 4);
    assert!(kura.certified_pair_durability_is_attested(lane_id, &artifact, &bound));
    check(&kura, lane_id, &artifact, bound);
}

/// Force a directory generation change without sleeps or filesystem clock-resolution assumptions.
#[cfg(unix)]
fn change_certified_pair_directory_generation(directory: &BoundProgressDirectory) {
    let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(123_456_789);
    assert_ne!(directory.metadata.modified().unwrap(), time);
    directory
        .file
        .set_times(std::fs::FileTimes::new().set_modified(time))
        .expect("change exact bound directory timestamp");
    let current = secure_file_metadata::from_file(&directory.file).unwrap();
    assert!(!Kura::sidecar_directory_metadata_unchanged(
        &directory.metadata,
        &current,
    ));
}

#[cfg(unix)]
#[test]
fn certified_pair_attestation_rejects_every_changed_ancestor_generation() {
    for ordinal in 0..4 {
        with_certified_pair_ancestor_attestation_fixture(|kura, lane_id, artifact, bound| {
            change_certified_pair_directory_generation(&bound.namespace.directories[ordinal]);
            let reopened = kura
                .open_bound_progress_sidecar(
                    &bound.namespace.data_path,
                    &bound.namespace.index_path,
                )
                .expect("reopen unchanged pair through current namespace");
            assert!(kura.bound_progress_sidecar_unchanged(&reopened));
            assert!(
                !kura.certified_pair_durability_is_attested(lane_id, artifact, &reopened),
                "old barrier must not cover changed directory {ordinal}"
            );
            assert!(kura.sync_bound_progress_sidecar(&reopened, "ancestor generation test"));
            kura.note_certified_pair_durability(lane_id, artifact, &reopened);
            assert!(kura.certified_pair_durability_is_attested(lane_id, artifact, &reopened));
        });
    }
}

#[cfg(unix)]
#[test]
fn certified_pair_attestation_rejects_ancestor_mutation_after_binding() {
    for ordinal in 1..4 {
        with_certified_pair_ancestor_attestation_fixture(|kura, lane_id, artifact, bound| {
            // The cache and this handle hold identical old snapshots. A comparison
            // against only those snapshots would miss this later mutation.
            change_certified_pair_directory_generation(&bound.namespace.directories[ordinal]);
            assert!(kura.bound_progress_sidecar_unchanged(&bound));
            assert!(
                !kura.certified_pair_durability_is_attested(lane_id, artifact, &bound),
                "fresh descriptor metadata must reject changed ancestor {ordinal}"
            );
            // Even a successful barrier must not promote fresh post-barrier metadata:
            // the retained pre-barrier chain remains stale until a fresh bind/sync.
            assert!(kura.sync_bound_progress_sidecar(&bound, "stale snapshot test"));
            kura.note_certified_pair_durability(lane_id, artifact, &bound);
            assert!(!kura.certified_pair_durability_is_attested(lane_id, artifact, &bound));
        });
    }
}

#[cfg(unix)]
#[test]
fn certified_pair_attestation_rejects_replaced_higher_ancestor() {
    with_certified_pair_ancestor_attestation_fixture(|kura, lane_id, artifact, bound| {
        let blocks = &bound.namespace.directories[2].expected_path;
        let lane = &bound.namespace.directories[1].expected_path;
        let displaced = blocks.with_file_name("displaced-certified-blocks");
        fs::rename(blocks, &displaced).expect("displace blocks ancestor");
        fs::create_dir(blocks).expect("install a different blocks ancestor");
        fs::rename(
            displaced.join(lane.file_name().expect("lane segment name")),
            lane,
        )
        .expect("restore the same lane subtree under the replaced ancestor");
        let reopened = kura
            .open_bound_progress_sidecar(&bound.namespace.data_path, &bound.namespace.index_path)
            .expect("bind current direct namespace");
        assert!(Kura::stable_sidecar_metadata_unchanged(
            &bound.data_metadata,
            &reopened.data_metadata,
        ));
        assert!(Kura::stable_sidecar_metadata_unchanged(
            &bound.index_metadata,
            &reopened.index_metadata,
        ));
        assert!(kura.bound_progress_sidecar_unchanged(&reopened));
        assert!(!kura.bound_progress_sidecar_unchanged(&bound));
        assert!(
            !kura.certified_pair_durability_is_attested(lane_id, artifact, &reopened),
            "exact file/immediate-parent metadata cannot attest a replaced higher ancestor"
        );
        assert!(kura.sync_bound_progress_sidecar(&reopened, "replaced ancestor test"));
        kura.note_certified_pair_durability(lane_id, artifact, &reopened);
        assert!(kura.certified_pair_durability_is_attested(lane_id, artifact, &reopened));
    });
}

#[cfg(unix)]
#[test]
fn certified_pair_attestation_requires_exact_chain_shape_and_artifact() {
    with_certified_pair_ancestor_attestation_fixture(|kura, lane_id, artifact, bound| {
        let original = kura.certified_pair_durability.lock()[&lane_id].clone();
        #[derive(Debug)]
        enum ChainMutation {
            Missing,
            Order,
            ExpectedPath,
            CanonicalPath,
            EntryName,
        }
        for kind in [
            ChainMutation::Missing,
            ChainMutation::Order,
            ChainMutation::ExpectedPath,
            ChainMutation::CanonicalPath,
            ChainMutation::EntryName,
        ] {
            let mut changed = original.clone();
            match kind {
                ChainMutation::Missing => {
                    changed.directories.pop();
                }
                ChainMutation::Order => changed.directories.swap(1, 2),
                ChainMutation::ExpectedPath => {
                    changed.directories[1].expected_path.push("different")
                }
                ChainMutation::CanonicalPath => {
                    changed.directories[1].canonical_path.push("different")
                }
                ChainMutation::EntryName => {
                    changed.directories[1].entry_name = Some("different".into())
                }
            }
            kura.certified_pair_durability
                .lock()
                .insert(lane_id, changed);
            assert!(
                !kura.certified_pair_durability_is_attested(lane_id, artifact, &bound),
                "malformed cache chain {kind:?} must not be accepted"
            );
        }
        kura.certified_pair_durability
            .lock()
            .insert(lane_id, original);
        assert!(kura.certified_pair_durability_is_attested(lane_id, artifact, &bound));
        assert!(!kura.certified_pair_durability_is_attested(LaneId::from(99), artifact, &bound));
        let mut different_artifact = artifact.clone();
        *different_artifact
            .commit_qc
            .bls_aggregate_signature
            .first_mut()
            .unwrap() ^= 1;
        assert!(!kura.certified_pair_durability_is_attested(lane_id, &different_artifact, &bound));
    });
}

#[cfg(unix)]
#[test]
fn certified_pair_changed_ancestor_reissues_every_durability_barrier() {
    for (label, failure) in strict_progress_sidecar_failure_modes() {
        with_certified_pair_ancestor_attestation_fixture(|kura, lane_id, artifact, bound| {
            change_certified_pair_directory_generation(&bound.namespace.directories[2]);
            failure.inject();
            assert_eq!(
                kura.latest_certified_lane_block_frontier(lane_id),
                None,
                "changed ancestor must reissue and observe {label} barrier failure"
            );
            let reopened = kura
                .open_bound_progress_sidecar(
                    &bound.namespace.data_path,
                    &bound.namespace.index_path,
                )
                .expect("reopen pair after failed barrier");
            assert!(!kura.certified_pair_durability_is_attested(lane_id, artifact, &reopened));
            assert_eq!(
                kura.latest_certified_lane_block_frontier(lane_id),
                Some(artifact.clone()),
                "retry after {label} failure must complete the whole barrier"
            );
            let reattested = kura
                .open_bound_progress_sidecar(
                    &bound.namespace.data_path,
                    &bound.namespace.index_path,
                )
                .expect("reopen reattested pair");
            assert!(kura.certified_pair_durability_is_attested(lane_id, artifact, &reattested));
        });
    }
}

#[cfg(unix)]
#[test]
#[ignore = "owned optimized completion-read measurement; requires explicit execution"]
fn certified_pair_durability_reuse_measurement() {
    const PAIRS: usize = 8;
    let (temp_dir, config, lane_config) = two_lane_storage_fixture();
    let lane_id = LaneId::from(1);
    let lane = lane_config.entry(lane_id).expect("configured lane");
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let (session, pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), pops.clone());
    let artifact_hash = HashOf::new(&expected).to_string();
    kura.persist_committed_lane_block_session(&session, &pops)
        .expect("persist the actual certificate before measurement");
    assert_eq!(
        fs::canonicalize(temp_dir.path()).expect("resolve configured measurement root"),
        kura.store_root,
        "the measured attestation probe must use Kura's resolved storage root"
    );
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane, &kura.store_root);
    let attestation_is_current = || {
        let Ok(bound) = kura.open_bound_progress_sidecar(&data_path, &index_path) else {
            return false;
        };
        kura.certified_pair_durability_is_attested(lane_id, &expected, &bound)
    };
    let mut total_read_calls = 0usize;
    let mut priming_calls = 0usize;
    let mut measured_call_ordinal = 0usize;
    let mut measure = |pair: usize, order: usize, attested: bool, initial: bool| {
        // Each paired arm starts with the same explicitly reported untimed read.
        // Removing only this lane's entry then selects the forced-miss arm.
        if !initial {
            kura.certified_pair_durability.lock().remove(&lane_id);
            total_read_calls += 1;
            priming_calls += 1;
            assert_eq!(
                kura.read_lane_completion_certificate(lane_id, 1)
                    .expect("prime this arm outside its measurement clock"),
                Some(expected.clone())
            );
        }
        if !attested {
            kura.certified_pair_durability.lock().remove(&lane_id);
        }
        let cache_before = attestation_is_current();
        assert_eq!(cache_before, attested, "exact cache precondition");
        total_read_calls += 1;
        let started = std::time::Instant::now();
        let result = kura.read_lane_completion_certificate(lane_id, 1);
        let elapsed_ns = started.elapsed().as_nanos();
        // Result comparison, additional attestation inspection and output are untimed.
        let (outcome, artifact_matches) = match &result {
            Ok(Some(artifact)) if artifact == &expected => ("ok", true),
            Ok(Some(_)) => ("mismatched_artifact", false),
            Ok(None) => ("absent", false),
            Err(_) => ("error", false),
        };
        let cache_after = attestation_is_current();
        let condition = if attested {
            "attested"
        } else {
            "forced_attestation_miss"
        };
        let phase = if initial { "initial" } else { "paired" };
        eprintln!(
            "BCK26_CERTIFICATE_READ_V1 {{\"schema\":1,\"pid\":{},\"phase\":\"{}\",\"pair\":{},\"order\":{},\"condition\":\"{}\",\"elapsed_ns\":{},\"outcome\":\"{}\",\"artifact_matches\":{},\"lane_id\":1,\"height\":1,\"artifact_hash\":\"{}\",\"cache_before\":{},\"cache_after\":{},\"measured_call_ordinal\":{},\"total_read_calls\":{},\"priming_calls\":{}}}",
            std::process::id(),
            phase,
            pair,
            order,
            condition,
            elapsed_ns,
            outcome,
            artifact_matches,
            artifact_hash,
            cache_before,
            cache_after,
            measured_call_ordinal,
            total_read_calls,
            priming_calls,
        );
        measured_call_ordinal += 1;
        assert!(artifact_matches, "measured completion failed: {result:?}");
        assert!(
            cache_after,
            "measured read did not leave an exact current attestation"
        );
    };
    measure(0, 0, false, true);
    for pair in 1..=PAIRS {
        let first_attested = pair % 2 == 0;
        measure(pair, 1, first_attested, false);
        measure(pair, 2, !first_attested, false);
    }
}
