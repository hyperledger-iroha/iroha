#[test]
fn autonomous_claim_release_rejects_noncanonical_groups_before_any_write() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) = two_reservation_autonomous_lane_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        &signer,
    );
    let payload = historical_capacity_bound_payload_for_fixture(&payload, &signer);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    persist_historical_capacity_payload_fixture(&kura, &payload, &signer);
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
    let retirement_hash = retirement.digest().expect("retirement digest");
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
        .expect("persist exact retirement and pending prefix");
    let paths = payload
        .entrypoint_hashes
        .iter()
        .map(|entrypoint_hash| {
            Kura::autonomous_lane_entrypoint_claim_path(
                temp_dir.path(),
                &network_id,
                entrypoint_hash,
            )
        })
        .collect::<Vec<_>>();
    let encode_claim = |claim: &AutonomousLaneEntrypointClaimV1| {
        norito::to_bytes(claim).expect("encode adversarial claim")
    };
    let pending = payload
        .entrypoint_hashes
        .iter()
        .map(|entrypoint_hash| {
            AutonomousLaneEntrypointClaimV1::release_pending_for_payload(
                &payload,
                *entrypoint_hash,
                retirement_hash,
            )
        })
        .collect::<Vec<_>>();
    let released = payload
        .entrypoint_hashes
        .iter()
        .map(|entrypoint_hash| {
            AutonomousLaneEntrypointClaimV1::released_for_payload(
                &payload,
                *entrypoint_hash,
                retirement_hash,
            )
        })
        .collect::<Vec<_>>();
    // Pending*/Active* is the only crash-reachable prepare ordering. An
    // Active/ReleasePending inversion must fail before the first claim is
    // normalized, leaving the entire adversarial group byte-identical.
    let active_first = AutonomousLaneEntrypointClaimV1::new(&payload, payload.entrypoint_hashes[0]);
    fs::write(&paths[0], encode_claim(&active_first)).expect("write inverted active claim");
    let before_prepare = paths
        .iter()
        .map(|path| fs::read(path).expect("read claim before rejected prepare"))
        .collect::<Vec<_>>();
    assert!(
        kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
            .is_err(),
        "an Active/ReleasePending inversion must fail closed",
    );
    assert_eq!(
        paths
            .iter()
            .map(|path| fs::read(path).expect("read claim after rejected prepare"))
            .collect::<Vec<_>>(),
        before_prepare,
        "prepare rejection must occur before any claim or temp mutation",
    );
    fs::write(&paths[0], encode_claim(&pending[0])).expect("restore pending first claim");
    fs::write(&paths[1], encode_claim(&released[1])).expect("write released suffix");
    let barrier = retirement
        .queue_release_barrier()
        .expect("exact Queue release barrier");
    let before_finalize = paths
        .iter()
        .map(|path| fs::read(path).expect("read claim before rejected finalize"))
        .collect::<Vec<_>>();
    assert!(
        kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch,)
            .is_err(),
        "a ReleasePending/Released inversion must fail closed",
    );
    assert_eq!(
        paths
            .iter()
            .map(|path| fs::read(path).expect("read claim after rejected finalize"))
            .collect::<Vec<_>>(),
        before_finalize,
        "finalize rejection must occur before any claim or temp mutation",
    );
    // Released*/ReleasePending* is the exact crash prefix produced by the
    // finalizer. It must resume deterministically and remain idempotent
    // after reopening Kura.
    fs::write(&paths[0], encode_claim(&released[0])).expect("write released prefix");
    fs::write(&paths[1], encode_claim(&pending[1])).expect("restore pending suffix");
    kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch)
        .expect("resume canonical Released prefix");
    for (path, expected) in paths.iter().zip(&released) {
        assert_eq!(
            Kura::decode_autonomous_lane_entrypoint_claim(path).expect("released claim"),
            *expected,
        );
    }
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen Kura");
    restore_autonomous_lane_fixture_geometry(&reopened, &lane_config, &payload)
        .expect("restore authenticated secondary lane artifacts");
    reopened
        .finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch)
        .expect("exact Released prefix retry is a storage stutter");
}
#[test]
fn strict_reservation_batch_reads_historical_attempt_instead_of_later_latest() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, first) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let first = historical_capacity_bound_payload_for_fixture(&first, &signer);
    let successor = repropose_autonomous_lane_payload_for_kura(
        &first,
        first
            .origin_proposal
            .descriptor
            .proposal_height
            .saturating_add(1),
        &signer,
    );
    let successor = historical_capacity_bound_payload_for_fixture(&successor, &signer);
    let first_group = autonomous_reservation_reconciliation_group(first.reservation_keys.clone());
    let successor_group =
        autonomous_reservation_reconciliation_group(successor.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first);
    persist_historical_capacity_payload_fixture(&kura, &first, &signer);
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&first);
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
        .expect("retire first attempt");
    let barrier = retirement
        .queue_release_barrier()
        .expect("first release barrier");
    kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch)
        .expect("finish first release");
    persist_historical_capacity_payload_fixture(&kura, &successor, &signer);
    let groups = [first_group, successor_group];
    let expected_epochs = [epoch, epoch];
    let assert_exact_attempts = |kura: &Kura| {
        let classified = kura
            .classify_autonomous_lane_reservation_groups(&groups, network_id, &expected_epochs)
            .expect("classify both exact proposal-height attempts");
        assert!(matches!(
            &classified[0],
            AutonomousLaneReservationEvidenceV1::ExactRetired {
                payload,
                retirement: exact_retirement,
                certification: AutonomousLaneReservationCertificationV1::Uncertified,
            } if payload == &first && exact_retirement == &retirement
        ));
        assert!(matches!(
            &classified[1],
            AutonomousLaneReservationEvidenceV1::ExactLive {
                payload,
                certification: AutonomousLaneReservationCertificationV1::Uncertified,
            } if payload == &successor
        ));
    };
    assert_exact_attempts(kura.as_ref());
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen Kura");
    restore_autonomous_lane_fixture_geometry(&reopened, &lane_config, &successor)
        .expect("restore exact secondary-lane lifecycle authority");
    assert_exact_attempts(reopened.as_ref());
}
#[test]
fn strict_reservation_classifier_rejects_reordered_and_partial_groups() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) = two_reservation_autonomous_lane_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        &signer,
    );
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist two-reservation payload");
    let exact = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&exact, network_id, epoch),
        Ok(AutonomousLaneReservationEvidenceV1::ExactLive { .. })
    ));
    let mut reordered_keys = payload.reservation_keys.clone();
    reordered_keys.reverse();
    let reordered = autonomous_reservation_reconciliation_group(reordered_keys);
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&reordered, network_id, epoch),
        Err(AutonomousLaneReservationEvidenceError::ReservationVectorConflict)
    ));
    let partial = autonomous_reservation_reconciliation_group(vec![payload.reservation_keys[0]]);
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&partial, network_id, epoch),
        Err(AutonomousLaneReservationEvidenceError::ReservationVectorConflict)
    ));
}
#[test]
fn strict_reservation_classifier_reports_malformed_attempt_as_error() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let descriptor = &payload.origin_proposal.descriptor;
    let lane = kura
        .lane_storage_entry(lane.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane = &lane;
    let attempt_path = Kura::autonomous_lane_block_attempt_path_for_entry(
        lane,
        temp_dir.path(),
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let malformed = vec![0xFF, 0x00, 0xAA, 0x55];
    fs::write(&attempt_path, &malformed).expect("corrupt exact attempt");
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&group, network_id, epoch),
        Err(AutonomousLaneReservationEvidenceError::Kura(_))
    ));
    assert_eq!(
        fs::read(&attempt_path).expect("read malformed attempt after classification"),
        malformed,
        "read-only classification must not recover or rewrite malformed evidence",
    );
}
#[test]
fn strict_reservation_classifier_treats_missing_artifact_directory_as_stable_absence() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    let artifact_directory = Kura::lane_artifact_dir(
        &kura
            .lane_storage_entry(lane.lane_id)
            .expect("exact persisted identity")
            .blocks_dir(temp_dir.path()),
    );
    fs::remove_dir(&artifact_directory).expect("remove empty fixture artifact directory");
    assert!(!artifact_directory.exists());
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&group, network_id, epoch),
        Ok(AutonomousLaneReservationEvidenceV1::StrictlyAbsent)
    ));
    assert!(
        !artifact_directory.exists(),
        "read-only strict absence classification must not create storage"
    );
}
#[test]
fn strict_reservation_classifier_exposes_exact_certification() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let (session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, &signer);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist exact certified lane artifact");
    let classified = kura
        .classify_autonomous_lane_reservation_group(&group, network_id, epoch)
        .expect("strict certified classification");
    assert!(matches!(
        classified,
        AutonomousLaneReservationEvidenceV1::ExactLive {
            payload: exact_payload,
            certification,
        } if exact_payload == payload && certification.is_certified()
    ));
}
#[test]
fn strict_reservation_classifier_preserves_unresolved_temp_without_mutation() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let descriptor = &payload.origin_proposal.descriptor;
    let lane = kura
        .lane_storage_entry(lane.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane = &lane;
    let attempt_path = Kura::autonomous_lane_block_attempt_path_for_entry(
        lane,
        temp_dir.path(),
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let temp_path = attempt_path.with_extension("norito.tmp");
    let staged = b"unresolved exact-attempt crash stage";
    fs::write(&temp_path, staged).expect("write unresolved attempt temp");
    let canonical_temp_path =
        fs::canonicalize(&temp_path).expect("canonicalize unresolved attempt temp");
    let outcome = kura.classify_autonomous_lane_reservation_group(&group, network_id, epoch);
    assert!(
        matches!(
            &outcome,
            Err(AutonomousLaneReservationEvidenceError::UnresolvedTemporary { path })
                if path == &canonical_temp_path
        ),
        "unexpected unresolved-attempt classification: {outcome:?}"
    );
    assert_eq!(
        fs::read(&temp_path).expect("read unresolved temp after classification"),
        staged,
        "read-only classification must not promote or remove crash evidence",
    );
}
#[test]
fn strict_reservation_classifier_rejects_same_height_other_attempt_when_exact_is_absent() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, missing) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let other = repropose_autonomous_lane_payload_for_kura(
        &missing,
        missing
            .origin_proposal
            .descriptor
            .proposal_height
            .saturating_add(1),
        &signer,
    );
    let missing_group =
        autonomous_reservation_reconciliation_group(missing.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &missing);
    kura.persist_lane_executable_payload(&other, network_id, epoch)
        .expect("persist only the competing proposal-height attempt");
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&missing_group, network_id, epoch,),
        Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)
    ));
}
#[test]
fn strict_reservation_classifier_rejects_conflicting_certified_artifact() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let conflicting = repropose_autonomous_lane_payload_for_kura(
        &payload,
        payload
            .origin_proposal
            .descriptor
            .proposal_height
            .saturating_add(1),
        &signer,
    );
    let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist exact autonomous payload");
    let (session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&conflicting.origin_proposal, &signer);
    let conflicting_artifact = CertifiedLaneBlockArtifact::new(session, signer_pops);
    let conflicting_payload = conflicting_artifact
        .encode_framed()
        .expect("encode conflicting same-height certification");
    let lane = kura
        .lane_storage_entry(lane.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane = &lane;
    let (data_path, index_path) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    assert!(Kura::append_indexed_sidecar(
        &data_path,
        &index_path,
        payload.origin_proposal.descriptor.lane_block_height,
        &conflicting_payload,
        "strict reservation conflicting certification fixture",
        FsyncMode::Always,
        None,
    ));
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&group, network_id, epoch),
        Err(AutonomousLaneReservationEvidenceError::CertifiedArtifactConflict)
    ));
}
#[cfg(unix)]
#[test]
fn strict_reservation_classifier_rejects_symlinked_attempt_without_following_it() {
    use std::os::unix::fs::symlink;
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    let descriptor = &payload.origin_proposal.descriptor;
    let lane = kura
        .lane_storage_entry(lane.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane = &lane;
    let attempt_path = Kura::autonomous_lane_block_attempt_path_for_entry(
        lane,
        temp_dir.path(),
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    fs::create_dir_all(attempt_path.parent().expect("attempt fixture parent"))
        .expect("create symlinked-attempt fixture directory");
    let target_path = temp_dir.path().join("outside-autonomous-attempt");
    let target_bytes = b"must not be followed or changed";
    fs::write(&target_path, target_bytes).expect("write symlink target");
    symlink(&target_path, &attempt_path).expect("install symlinked attempt");
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&group, network_id, epoch),
        Err(AutonomousLaneReservationEvidenceError::Kura(_))
    ));
    assert_eq!(
        fs::read(&target_path).expect("read untouched symlink target"),
        target_bytes,
    );
    assert!(
        fs::symlink_metadata(&attempt_path)
            .expect("symlink remains after read-only classification")
            .file_type()
            .is_symlink()
    );
}
#[test]
fn strict_reservation_classifier_rejects_oversized_certified_index_without_recovery() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    let lane = kura
        .lane_storage_entry(lane.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane = &lane;
    let (data_path, index_path) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    fs::create_dir_all(data_path.parent().expect("certified fixture parent"))
        .expect("create oversized-certified-index fixture directory");
    let entries_len = (MAX_AUTONOMOUS_RESERVATION_CERTIFIED_INDEX_ENTRIES + 1)
        .checked_mul(PIPELINE_INDEX_ENTRY_SIZE)
        .expect("oversized certified index length");
    let mut index_bytes = SidecarIndexLayout::base_header(1).to_vec();
    index_bytes.resize(
        INDEXED_SIDECAR_BASE_HEADER_SIZE
            .checked_add(entries_len)
            .expect("oversized certified V1 index length"),
        0,
    );
    fs::write(&data_path, b"").expect("write empty certified data");
    fs::write(&index_path, &index_bytes).expect("write oversized certified index");
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&group, network_id, epoch),
        Err(AutonomousLaneReservationEvidenceError::Kura(_))
    ));
    assert_eq!(
        fs::metadata(&index_path)
            .expect("oversized index remains")
            .len(),
        u64::try_from(index_bytes.len()).expect("index length fits u64"),
        "read-only classification must not truncate the oversized index",
    );
}
#[test]
fn strict_reservation_classifier_rejects_live_exact_with_unretired_same_height_attempt() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let other = repropose_autonomous_lane_payload_for_kura(
        &payload,
        payload
            .origin_proposal
            .descriptor
            .proposal_height
            .saturating_add(1),
        &signer,
    );
    let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist current exact payload");
    let other_lane_block_height = other.origin_proposal.descriptor.lane_block_height;
    let other_proposal_height = other.origin_proposal.descriptor.proposal_height;
    let lane = kura
        .lane_storage_entry(lane.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane = &lane;
    let other_attempt_path = Kura::autonomous_lane_block_attempt_path_for_entry(
        lane,
        temp_dir.path(),
        other_lane_block_height,
        other_proposal_height,
    );
    let other_artifact = AutonomousLaneBlockArtifact::new(other);
    let other_attempt_bytes =
        norito::encode_canonical(&other_artifact).expect("encode competing attempt");
    fs::write(&other_attempt_path, &other_attempt_bytes).expect("write competing attempt");
    let other_view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
        lane,
        temp_dir.path(),
        other_lane_block_height,
        other_proposal_height,
    );
    let other_view_bytes = norito::encode_canonical(&AutonomousLaneBlockViewState::from_artifact(
        &other_artifact,
    ))
    .expect("encode competing view state");
    fs::write(&other_view_path, &other_view_bytes).expect("write competing view state");
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&group, network_id, epoch),
        Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)
    ));
    assert_eq!(
        fs::read(&other_attempt_path).expect("read competing attempt"),
        other_attempt_bytes,
    );
    assert_eq!(
        fs::read(&other_view_path).expect("read competing view state"),
        other_view_bytes,
        "conflict classification must not recover either attempt",
    );
}
#[test]
fn strict_reservation_classifier_rejects_live_historical_attempt_named_by_later_pointer() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, historical) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let later = repropose_autonomous_lane_payload_for_kura(
        &historical,
        historical
            .origin_proposal
            .descriptor
            .proposal_height
            .saturating_add(1),
        &signer,
    );
    let group = autonomous_reservation_reconciliation_group(historical.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &historical);
    kura.persist_lane_executable_payload(&historical, network_id, epoch)
        .expect("persist historical live payload");
    let later_descriptor = &later.origin_proposal.descriptor;
    let lane = kura
        .lane_storage_entry(lane.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane = &lane;
    let later_attempt_path = Kura::autonomous_lane_block_attempt_path_for_entry(
        lane,
        temp_dir.path(),
        later_descriptor.lane_block_height,
        later_descriptor.proposal_height,
    );
    let later_artifact = AutonomousLaneBlockArtifact::new(later.clone());
    let later_attempt_bytes =
        norito::encode_canonical(&later_artifact).expect("encode later attempt");
    fs::write(&later_attempt_path, &later_attempt_bytes).expect("write later attempt");
    let latest_path = Kura::autonomous_lane_block_latest_attempt_path_for_entry(
        lane,
        temp_dir.path(),
        later_descriptor.lane_block_height,
    );
    let latest_bytes =
        norito::encode_canonical(&AutonomousLaneBlockLatestAttemptV1::from_payload(&later))
            .expect("encode later latest pointer");
    fs::write(&latest_path, &latest_bytes).expect("replace latest pointer with later attempt");
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(&group, network_id, epoch),
        Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)
    ));
    assert_eq!(
        fs::read(&latest_path).expect("read unchanged later pointer"),
        latest_bytes,
    );
    assert_eq!(
        fs::read(&later_attempt_path).expect("read unchanged later attempt"),
        later_attempt_bytes,
        "historical-live conflict classification must remain read-only",
    );
}
#[test]
fn strict_reservation_classifier_rejects_conflicting_claim_temp_without_mutation() {
    let temp_dir = TempDir::new().expect("temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let conflicting = repropose_autonomous_lane_payload_for_kura(
        &payload,
        payload
            .origin_proposal
            .descriptor
            .proposal_height
            .saturating_add(1),
        &signer,
    );
    let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist exact payload and claims");
    let entrypoint_hash = payload.entrypoint_hashes[0];
    let claim_path =
        Kura::autonomous_lane_entrypoint_claim_path(temp_dir.path(), &network_id, &entrypoint_hash);
    let temp_path = Kura::autonomous_lane_entrypoint_claim_temp_path(&claim_path);
    let conflicting_claim = AutonomousLaneEntrypointClaimV1::new(&conflicting, entrypoint_hash);
    let temp_bytes =
        norito::encode_canonical(&conflicting_claim).expect("encode conflicting claim temp");
    fs::write(&temp_path, &temp_bytes).expect("write conflicting claim temp");
    let canonical_temp_path =
        fs::canonicalize(&temp_path).expect("canonicalize conflicting claim temp");
    let outcome = kura.classify_autonomous_lane_reservation_group(&group, network_id, epoch);
    assert!(
        matches!(
            &outcome,
            Err(AutonomousLaneReservationEvidenceError::EntrypointClaimConflict { path })
                if path == &canonical_temp_path
        ),
        "unexpected conflicting-claim classification: {outcome:?}"
    );
    assert_eq!(
        fs::read(&temp_path).expect("read unchanged conflicting claim temp"),
        temp_bytes,
        "claim preflight must not remove or promote a conflicting stage",
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn historical_autonomous_recovery_rejects_live_certified_same_input_recreation() {
    let temp_dir = TempDir::new().expect("temp dir");
    let accounting_dir = TempDir::new().expect("accounting probe dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, first_b) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let (first_b, first_b_carrier, first_b_finality, first_b_record) =
        finalized_aba_recovery_source_for_kura(&first_b, &signer, None);
    let incarnation_b = first_b.origin_proposal.descriptor.lane_incarnation;
    let rebound_a = rebind_autonomous_lane_payload_for_kura(
        &first_b,
        lane.lane_id,
        lane.dataspace_id,
        1,
        b"historical-autonomous-recovery-incarnation-a",
        &signer,
    );
    let incarnation_a = rebound_a.origin_proposal.descriptor.lane_incarnation;
    let (
        incarnation_a_payload,
        incarnation_a_carrier,
        incarnation_a_finality,
        incarnation_a_record,
    ) = finalized_aba_recovery_source_for_kura(
        &rebound_a,
        &signer,
        Some((&first_b_carrier, &first_b_finality)),
    );
    let rebound_b = rebind_autonomous_lane_payload_for_kura(
        &incarnation_a_payload,
        lane.lane_id,
        lane.dataspace_id,
        1,
        b"kura-autonomous-view-incarnation",
        &signer,
    );
    let (recreated_b, recreated_b_carrier, recreated_b_finality, recreated_b_record) =
        finalized_aba_recovery_source_for_kura(
            &rebound_b,
            &signer,
            Some((&incarnation_a_carrier, &incarnation_a_finality)),
        );
    assert_ne!(incarnation_a, incarnation_b);
    assert_eq!(
        recreated_b.origin_proposal.descriptor.lane_incarnation, incarnation_b,
        "the final candidate deliberately aliases the first incarnation hash"
    );
    assert_eq!(
        first_b.reservation_keys[0].entrypoint_hash,
        incarnation_a_payload.reservation_keys[0].entrypoint_hash
    );
    assert_eq!(
        first_b.reservation_keys[0].entrypoint_hash,
        recreated_b.reservation_keys[0].entrypoint_hash,
        "all proposed generations contend for the same unconsumed transaction"
    );
    assert_ne!(first_b_record.recovery_id, incarnation_a_record.recovery_id);
    assert_ne!(first_b_record.recovery_id, recreated_b_record.recovery_id);
    assert_ne!(
        incarnation_a_record.recovery_id,
        recreated_b_record.recovery_id
    );
    let (first_b_session, first_b_pops) =
        committed_lane_block_session_for_kura_proposal(&first_b.origin_proposal, &signer);
    let (incarnation_a_session, incarnation_a_pops) =
        committed_lane_block_session_for_kura_proposal(
            &incarnation_a_payload.origin_proposal,
            &signer,
        );
    let (recreated_b_session, recreated_b_pops) =
        committed_lane_block_session_for_kura_proposal(&recreated_b.origin_proposal, &signer);
    let (kura, _) = open_historical_recovery_fixture(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first_b);
    // These are authenticated source envelopes, not economic application.
    // Retain all three so missing historical finality cannot explain any refusal.
    for (carrier, finality) in [
        (&first_b_carrier, &first_b_finality),
        (&incarnation_a_carrier, &incarnation_a_finality),
        (&recreated_b_carrier, &recreated_b_finality),
    ] {
        assert!(carrier.execution_context().unwrap().merge_entry.is_none());
        assert_eq!(carrier.external_entrypoint_count(), 0);
        kura.store_block(Arc::clone(carrier))
            .expect("retain canonical source envelope");
        let _receipt = kura
            .store_v2_finality_artifact(finality)
            .expect("retain exact three-of-four signed source finality");
    }
    persist_historical_capacity_payload_fixture_at_context(
        &kura,
        &first_b,
        &signer,
        first_b_finality.height_context.id(),
    );
    assert_eq!(
        kura.persist_historical_autonomous_lane_recovery_record(&first_b_record)
            .expect("persist first-B historical recovery"),
        HistoricalAutonomousLaneRecoveryPersistOutcome::Installed
    );
    kura.persist_committed_lane_block_session(&first_b_session, &first_b_pops)
        .expect("certify the first-B attempt");
    let group = autonomous_reservation_reconciliation_group(first_b.reservation_keys.clone());
    let lane = kura
        .lane_storage_entry(lane.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane = &lane;
    let first_b_record_path = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        temp_dir.path(),
        first_b_record.recovery_id,
    );
    let first_b_record_bytes = fs::read(&first_b_record_path).expect("retain exact first-B seal");
    let primary = kura
        .lane_storage_entry(lane_config.primary().lane_id)
        .expect("capture the exact primary storage identity");
    let (primary_incarnation, primary_activation) = kura
        .active_lane_incarnation_marker(&primary)
        .expect("read unchanged primary geometry");
    let original_incarnations = BTreeMap::from([
        (primary.lane_id, primary_incarnation),
        (lane.lane_id, incarnation_b),
    ]);
    let original_activations =
        BTreeMap::from([(primary.lane_id, primary_activation), (lane.lane_id, 0)]);
    let assert_exact_live_owner = |store: &Kura| {
        assert_eq!(
            store.active_lane_incarnation_marker(lane).unwrap(),
            (incarnation_b, 0)
        );
        assert_eq!(
            store
                .historical_autonomous_lane_recovery_records_bounded(3)
                .unwrap(),
            vec![first_b_record.clone()]
        );
        assert!(
            store
                .historical_autonomous_lane_recovery_record_matches(&first_b_record)
                .unwrap()
        );
        assert_eq!(
            store
                .read_autonomous_lane_block_artifact(lane.lane_id, 1, network_id, epoch)
                .unwrap()
                .executable_payload,
            first_b
        );
        assert_eq!(
            store
                .read_lane_block_execution_input(lane.lane_id, 1)
                .unwrap()
                .proposal,
            first_b.origin_proposal
        );
        assert_eq!(
            store
                .read_certified_lane_block_artifact(lane.lane_id, 1)
                .unwrap()
                .proposal,
            first_b.origin_proposal
        );
        assert!(
            matches!(store.classify_autonomous_lane_reservation_group(&group, network_id, epoch),
            Ok(AutonomousLaneReservationEvidenceV1::ExactLive { payload, certification })
                if payload == first_b && certification.is_certified())
        );
        assert!(
            store
                .read_autonomous_lane_slot_retirement(lane.lane_id, 1, network_id, epoch)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            fs::read(&first_b_record_path).unwrap(),
            first_b_record_bytes
        );
    };
    let reject_unsettled_recreation = |store: &Kura| {
        let before = snapshot_regular_test_tree(temp_dir.path());
        let journal = store.lane_geometry_journal_state_for_test().unwrap();
        let error = store
            .persist_autonomous_lane_slot_retirement(
                &AutonomousLaneSlotRetirementV1::from_payload(&first_b),
                network_id,
                epoch,
            )
            .expect_err("a certified attempt cannot be converted into release authority");
        assert!(
            error
                .to_string()
                .contains("certified autonomous lane block cannot be retired")
        );
        for (candidate_incarnation, activation) in [(incarnation_a, 1), (incarnation_b, 2)] {
            let mut incarnations = original_incarnations.clone();
            incarnations.insert(lane.lane_id, candidate_incarnation);
            let mut activations = original_activations.clone();
            activations.insert(lane.lane_id, activation);
            // A route drain identity alone must not consume a live local attempt.
            let error = store
                .apply_lane_geometry_transition_with_certified_retirements(
                    &lane_config,
                    &lane_config,
                    &original_incarnations,
                    &incarnations,
                    &original_activations,
                    &activations,
                    &BTreeSet::from([lane.lane_id]),
                    &BTreeSet::from([(lane.lane_id, lane.dataspace_id, incarnation_b)]),
                )
                .expect_err("certified work must settle before its namespace is archived");
            assert!(
                matches!(&error, Error::IO(error, _) if error.kind() == ErrorKind::WouldBlock),
                "expected an owned terminalization dependency: {error:?}"
            );
            assert!(error.to_string().contains("slot retirement is durable"));
            assert_eq!(
                store.lane_geometry_journal_state_for_test().unwrap(),
                journal
            );
            assert_eq!(
                snapshot_regular_test_tree(temp_dir.path()),
                before,
                "neither alias may create an archive, successor marker, claim or journal phase"
            );
        }
        for (payload, record, session, pops) in [
            (
                &incarnation_a_payload,
                &incarnation_a_record,
                &incarnation_a_session,
                &incarnation_a_pops,
            ),
            (
                &recreated_b,
                &recreated_b_record,
                &recreated_b_session,
                &recreated_b_pops,
            ),
        ] {
            assert!(
                store
                    .persist_lane_executable_payload(payload, network_id, epoch)
                    .is_err(),
                "same-input replay must not replace the original live payload or claim"
            );
            assert!(
                store
                    .persist_historical_autonomous_lane_recovery_record(record)
                    .is_err(),
                "a signed later source cannot replace the original local attempt"
            );
            assert!(
                store
                    .persist_committed_lane_block_session(session, pops)
                    .is_err(),
                "a different incarnation or same-hash successor QC cannot replace the live certificate"
            );
            let conflicting_group =
                autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
            let classification = store.classify_autonomous_lane_reservation_group(
                &conflicting_group,
                network_id,
                epoch,
            );
            if payload.origin_proposal.descriptor.lane_incarnation != incarnation_b {
                assert!(
                    matches!(
                        classification,
                        Err(AutonomousLaneReservationEvidenceError::Kura(_))
                    ),
                    "another incarnation must fail the active marker boundary"
                );
            } else {
                assert!(
                    matches!(
                        classification,
                        Err(AutonomousLaneReservationEvidenceError::OtherAttemptConflict)
                    ),
                    "same-hash later B must conflict with the original live proposal-height attempt"
                );
            }
            assert_eq!(
                snapshot_regular_test_tree(temp_dir.path()),
                before,
                "rejected replay must preserve every durable owner byte"
            );
        }
        assert_exact_live_owner(store);
    };
    reject_unsettled_recreation(&kura);
    for record in [&incarnation_a_record, &recreated_b_record] {
        let injected = Kura::historical_autonomous_recovery_path_for_entry(
            lane,
            temp_dir.path(),
            record.recovery_id,
        );
        assert!(!injected.exists());
        let before = snapshot_regular_test_tree(temp_dir.path());
        fs::write(
            &injected,
            historical_autonomous_recovery_record_bytes(record),
        )
        .expect("inject exact authenticated competing historical bytes");
        let damaged = snapshot_regular_test_tree(temp_dir.path());
        assert!(
            kura.historical_autonomous_lane_recovery_records_bounded(3)
                .is_err(),
            "inventory cannot treat competing historical input as absent or as a new live owner"
        );
        assert_eq!(
            snapshot_regular_test_tree(temp_dir.path()),
            damaged,
            "inventory is read-only even when a competing record is physically present"
        );
        assert_eq!(
            fs::read(&first_b_record_path).unwrap(),
            first_b_record_bytes
        );
        fs::remove_file(injected).expect("remove only injected fault bytes");
        assert_eq!(snapshot_regular_test_tree(temp_dir.path()), before);
    }
    assert_exact_live_owner(&kura);
    drop(kura);
    let (reopened, _) = open_historical_recovery_fixture(&config, &lane_config)
        .expect("restart preserves the original unsettled owner");
    reject_unsettled_recreation(&reopened);
    assert_exact_live_owner(&reopened);
    let lane_blocks = reopened
        .lane_storage_entry(lane.lane_id)
        .expect("exact restored identity")
        .blocks_dir(temp_dir.path());
    let byte_limit = reopened.historical_autonomous_recovery_aggregate_byte_limit();
    let with_recovery = Kura::block_store_bytes_with_historical_limit(&lane_blocks, byte_limit)
        .expect("measure retained historical recovery");
    let probe = accounting_dir
        .path()
        .join("first-b-accounting-probe.norito");
    fs::rename(&first_b_record_path, &probe).expect("temporarily move the accounting probe");
    let without_recovery = Kura::block_store_bytes_with_historical_limit(&lane_blocks, byte_limit)
        .expect("measure storage without only the exact recovery frame");
    fs::rename(&probe, &first_b_record_path).expect("restore the exact recovery frame");
    assert_eq!(
        with_recovery.checked_sub(without_recovery),
        Some(u64::try_from(first_b_record_bytes.len()).unwrap()),
        "nested historical recovery bytes are counted exactly once"
    );
    let accounting = reopened.disk_usage_accounting_snapshot_for_tests().unwrap();
    assert!(accounting.enforced_initialized && accounting.total_initialized);
    assert_eq!(
        accounting.cached_enforced_bytes,
        accounting.exact_enforced_bytes
    );
    assert_eq!(accounting.cached_total_bytes, accounting.exact_total_bytes);
}

/// Real three-carrier source for the ABA storage-replay fixture. The carrier
/// contains the hint-free input; only afterward is its exact hash attached.
fn finalized_aba_recovery_source_for_kura(
    template: &LaneExecutablePayloadV1,
    signer: &KeyPair,
    previous: Option<(&SignedBlock, &V2FinalityArtifact)>,
) -> (
    LaneExecutablePayloadV1,
    Arc<SignedBlock>,
    V2FinalityArtifact,
    HistoricalAutonomousLaneRecoveryRecordV1,
) {
    let height = previous.map_or(1, |(block, _)| block.header().height().get() + 1);
    let keys = v2_finality_fixture_keys();
    let mut source = repropose_autonomous_lane_payload_for_kura(template, height, signer);
    source.origin_proposal.payload_block_hint = None;
    let probe: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, previous.map(|(block, _)| block))
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    let build_finality = |block: &SignedBlock| {
        v2_finality_artifact_for_block_with_keys_and_context_policy(
            block,
            previous.map(|(_, finality)| finality),
            &keys,
            v2_finality_fixture_execution_commitment(),
            None,
            template.network_id,
            template.epoch,
            100,
            iroha_data_model::block::consensus_v2::recommended_data_availability_layout(),
        )
    };
    // The frozen context depends on the parent, never on this carrier's body.
    // Use its actual identity for the producer's signed lifecycle custody.
    let context = build_finality(&probe).height_context;
    let source = lifecycle_terminal_bound_payload_for_test(&source, context.id(), signer);
    let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
        &source,
        source.network_id,
        source.epoch,
    )
    .expect("exact hint-free autonomous input envelope");
    let mut block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, previous.map(|(block, _)| block))
        .with_execution_context(Some(
            BlockExecutionContextBundle::new(Vec::new())
                .with_autonomous_lane_payloads(vec![envelope]),
        ))
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    attach_ok_results_to_block(&mut block);
    let finality = build_finality(&block);
    assert_eq!(
        finality.height_context, context,
        "the payload cannot change its frozen authority"
    );
    let wire = block
        .encode_wire()
        .expect("canonical complete carrier wire");
    assert!(u64::try_from(wire.len()).unwrap() <= context.da_layout.max_payload_size_bytes);
    finality
        .verify()
        .expect("actual exact 3-of-4 global Commit signatures");
    finality
        .validate_for_header(&block.header())
        .expect("finality binds exact carrier header");
    assert_eq!(
        finality
            .commit_qc
            .execution_commitment
            .executed_block_wire_len,
        u64::try_from(wire.len()).unwrap()
    );
    assert_eq!(
        finality
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash,
        Hash::new(&wire)
    );
    let payload = source
        .attach_global_hint_exact(
            iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
                proposal_height: height,
                proposal_view: block.header().view_change_index(),
                proposal_block_hash: block.hash(),
            },
            source.network_id,
            source.epoch,
        )
        .expect("attach only the actual finalized carrier identity");
    let canonical_body = crate::sumeragi::message::CanonicalExecutedBlockNeedV1 {
        height,
        block_hash: block.hash(),
        finality_artifact_hash: HashOf::new(&finality),
        execution_commitment: finality.commit_qc.execution_commitment,
        executed_block_wire_len: u64::try_from(wire.len()).unwrap(),
        executed_block_wire_hash: Hash::new(&wire),
    };
    let mut install = crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1 {
        version: crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1::VERSION,
        recovery_id: Hash::prehashed([0; Hash::LENGTH]),
        canonical_body,
        historical_context_id: context.id(),
        historical_context_hash: HashOf::new(&context),
        historical_context: context,
        carrier_view: block.header().view_change_index(),
        payload: payload.clone(),
        reservation_group: autonomous_reservation_reconciliation_group(
            payload.reservation_keys.clone(),
        ),
    };
    install.recovery_id = install.computed_recovery_id();
    assert!(install.has_valid_identity());
    let record = HistoricalAutonomousLaneRecoveryRecordV1::from_install(
        &install,
        vec![bls_normal_pop_prove(signer.private_key()).expect("lane producer PoP")],
    );
    (payload, Arc::new(block), finality, record)
}

/// Persist an actual signed carrier and its independently authenticated recovery seal.
pub(super) fn historical_geometry_observation_fixture(
    temp_dir: &TempDir,
) -> (Arc<Kura>, PathBuf, HistoricalAutonomousLaneRecoveryRecordV1) {
    let config = kura_config_for_dir(temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, template) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let (payload, carrier, finality, record) =
        finalized_aba_recovery_source_for_kura(&template, &signer, None);
    let (kura, _) = open_historical_recovery_fixture(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.store_block(carrier)
        .expect("store actual canonical carrier");
    let receipt = kura
        .store_v2_finality_artifact(&finality)
        .expect("store exact three-of-four signed finality");
    assert_eq!(receipt.height(), record.canonical_body.height);
    assert_eq!(receipt.block_hash(), record.canonical_body.block_hash);
    assert_eq!(
        receipt.artifact_hash(),
        record.canonical_body.finality_artifact_hash
    );
    persist_historical_capacity_payload_fixture_at_context(
        &kura,
        &payload,
        &signer,
        finality.height_context.id(),
    );
    kura.persist_historical_autonomous_lane_recovery_record(&record)
        .expect("persist authenticated historical recovery");
    let lane = kura
        .lane_storage_entry(lane.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane = &lane;
    let path = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        &kura.store_root(),
        record.recovery_id,
    );
    (kura, path, record)
}

#[allow(clippy::too_many_lines)]
fn historical_autonomous_recovery_record_for_kura(
    payload: &LaneExecutablePayloadV1,
    signer: &KeyPair,
    fixture_tag: &[u8],
) -> HistoricalAutonomousLaneRecoveryRecordV1 {
    let descriptor = &payload.origin_proposal.descriptor;
    let hint = payload
        .origin_proposal
        .payload_block_hint
        .expect("historical recovery fixture has a canonical carrier hint");
    assert_eq!(descriptor.proposal_height, hint.proposal_height);
    assert_eq!(descriptor.validator_set.len(), 1);
    assert_eq!(
        descriptor.validator_set[0].public_key(),
        signer.public_key()
    );
    let mut roster = descriptor
        .validator_set
        .iter()
        .cloned()
        .map(|validator| ValidatorPower {
            validator,
            power: 1,
        })
        .collect::<Vec<_>>();
    for seed in 0xB1_u8..=0xB4 {
        if roster.len() == 4 {
            break;
        }
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("derive deterministic historical recovery consensus validator");
        let validator = PeerId::new(keypair.public_key().clone());
        if roster.iter().all(|entry| entry.validator != validator) {
            roster.push(ValidatorPower {
                validator,
                power: 1,
            });
        }
    }
    roster.sort_by(|left, right| left.validator.cmp(&right.validator));
    // Bind the recovery context and its derived mint roster to the signed payload.
    let network_id = payload.network_id;
    let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
        crate::kagemusha_v1_test_fixtures::mint_finality_authorization_and_authority(
            network_id,
            payload.epoch,
            (payload.epoch)
                .checked_add(1)
                .expect("fixture epoch fits positive heights"),
            descriptor.proposal_height.saturating_add(100),
            &roster,
        );
    let historical_context = HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height: descriptor.proposal_height,
        epoch: payload.epoch,
        epoch_end_height: descriptor.proposal_height.saturating_add(100),
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: Some(
            iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor {
                snapshot_height: descriptor.proposal_height.saturating_sub(1),
                snapshot_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                    Hash::new_from_chunks(&[
                        b"kura:test:historical-recovery:snapshot-block:v1\0",
                        fixture_tag,
                    ]),
                ),
                snapshot_block_creation_time_ms: descriptor.proposal_height,
                snapshot_state_hash: Hash::new_from_chunks(&[
                    b"kura:test:historical-recovery:snapshot-state:v1\0",
                    fixture_tag,
                ]),
            },
        ),
        quorum: DualQuorum::from_roster(&roster).expect("historical recovery fixture quorum"),
        roster,
        kagemusha_mint_finality_authorization,
        kagemusha_mint_finality_authority,
        nexus_amx_context_hash: Hash::new_from_chunks(&[
            b"kura:test:historical-recovery:nexus:v1\0",
            fixture_tag,
        ]),
        execution_policy_hash: Hash::new_from_chunks(&[
            b"kura:test:historical-recovery:policy:v1\0",
            fixture_tag,
        ]),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0xA7; 32],
    };
    historical_context
        .validate()
        .expect("valid historical recovery fixture context");
    assert_eq!(
        historical_context.network_id, payload.network_id,
        "fixture carrier context must bind the executable payload chain",
    );
    let executed_wire =
        norito::encode_canonical(payload).expect("encode historical recovery fixture wire");
    let executed_block_wire_len =
        u64::try_from(executed_wire.len()).expect("fixture wire length fits u64");
    let executed_block_wire_hash = Hash::new(&executed_wire);
    let execution_commitment = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        Hash::new_from_chunks(&[
            b"kura:test:historical-recovery:parent-state:v1\0",
            fixture_tag,
        ]),
        Hash::new_from_chunks(&[
            b"kura:test:historical-recovery:post-state:v1\0",
            fixture_tag,
        ]),
        Hash::new_from_chunks(&[b"kura:test:historical-recovery:writes:v1\0", fixture_tag]),
        executed_block_wire_len,
        executed_block_wire_hash,
    );
    execution_commitment
        .validate()
        .expect("valid historical recovery execution commitment");
    let canonical_body = crate::sumeragi::message::CanonicalExecutedBlockNeedV1 {
        height: descriptor.proposal_height,
        block_hash: hint.proposal_block_hash,
        finality_artifact_hash: HashOf::<V2FinalityArtifact>::from_untyped_unchecked(
            Hash::new_from_chunks(&[b"kura:test:historical-recovery:finality:v1\0", fixture_tag]),
        ),
        execution_commitment,
        executed_block_wire_len,
        executed_block_wire_hash,
    };
    let reservation_group =
        autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
    let mut install = crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1 {
        version: crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1::VERSION,
        recovery_id: Hash::prehashed([0; Hash::LENGTH]),
        canonical_body,
        historical_context_id: historical_context.id(),
        historical_context_hash: HashOf::new(&historical_context),
        historical_context,
        carrier_view: hint.proposal_view,
        payload: payload.clone(),
        reservation_group,
    };
    install.recovery_id = install.computed_recovery_id();
    assert!(install.has_valid_identity());
    HistoricalAutonomousLaneRecoveryRecordV1::from_install(
        &install,
        vec![
            bls_normal_pop_prove(signer.private_key())
                .expect("historical recovery fixture signer PoP"),
        ],
    )
}
