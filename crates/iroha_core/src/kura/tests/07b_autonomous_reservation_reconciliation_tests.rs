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
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist two-reservation payload");
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
    let successor = repropose_autonomous_lane_payload_for_kura(
        &first,
        first
            .origin_proposal
            .descriptor
            .proposal_height
            .saturating_add(1),
        &signer,
    );
    let first_group = autonomous_reservation_reconciliation_group(first.reservation_keys.clone());
    let successor_group =
        autonomous_reservation_reconciliation_group(successor.reservation_keys.clone());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first);
    kura.persist_lane_executable_payload(&first, network_id, epoch)
        .expect("persist first attempt");
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&first);
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
        .expect("retire first attempt");
    let barrier = retirement
        .queue_release_barrier()
        .expect("first release barrier");
    kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch)
        .expect("finish first release");
    kura.persist_lane_executable_payload(&successor, network_id, epoch)
        .expect("persist later latest attempt");
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
    let artifact_directory = Kura::lane_artifact_dir(&lane.blocks_dir(temp_dir.path()));
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
fn historical_autonomous_recovery_is_safe_across_same_lane_b_a_b_recreation() {
    let temp_dir = TempDir::new().expect("temp dir");
    let archive_dir = TempDir::new().expect("fixture archive dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, first_b) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
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
    let incarnation_a_payload = repropose_autonomous_lane_payload_for_kura(&rebound_a, 84, &signer);
    let rebound_b = rebind_autonomous_lane_payload_for_kura(
        &incarnation_a_payload,
        lane.lane_id,
        lane.dataspace_id,
        1,
        b"kura-autonomous-view-incarnation",
        &signer,
    );
    let recreated_b = repropose_autonomous_lane_payload_for_kura(&rebound_b, 126, &signer);
    assert_ne!(incarnation_a, incarnation_b);
    assert_eq!(
        recreated_b.origin_proposal.descriptor.lane_incarnation, incarnation_b,
        "the final leg deliberately exercises an incarnation-hash ABA replay",
    );
    assert_eq!(
        first_b.reservation_keys[0].entrypoint_hash,
        incarnation_a_payload.reservation_keys[0].entrypoint_hash,
    );
    assert_eq!(
        first_b.reservation_keys[0].entrypoint_hash,
        recreated_b.reservation_keys[0].entrypoint_hash,
        "all three generations contend for the exact same FIFO transaction",
    );
    let first_b_record =
        historical_autonomous_recovery_record_for_kura(&first_b, &signer, b"incarnation-b-first");
    let incarnation_a_record = historical_autonomous_recovery_record_for_kura(
        &incarnation_a_payload,
        &signer,
        b"incarnation-a",
    );
    let recreated_b_record = historical_autonomous_recovery_record_for_kura(
        &recreated_b,
        &signer,
        b"incarnation-b-recreated",
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
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    let recreate_lane_storage = |stage: &str| {
        kura.reconcile_lane_segments_for_testing(&[], &[], &[(lane, lane)])
            .unwrap_or_else(|error| panic!("provision {stage} lane storage: {error:?}"));
        let blocks = lane.blocks_dir(temp_dir.path());
        for file_name in [INDEX_FILE_NAME, DATA_FILE_NAME, HASHES_FILE_NAME] {
            assert!(
                blocks.join(file_name).is_file(),
                "{stage} lane storage is missing {file_name}",
            );
        }
        let lane_artifacts = blocks.join(LANE_ARTIFACTS_DIR_NAME);
        let lane_artifacts_metadata = fs::symlink_metadata(&lane_artifacts)
            .unwrap_or_else(|error| panic!("inspect {stage} lane artifacts: {error:?}"));
        assert!(
            lane_artifacts_metadata.is_dir() && !lane_artifacts_metadata.file_type().is_symlink(),
            "{stage} lane artifacts must be a direct directory",
        );
        assert_eq!(
            fs::read_dir(&lane_artifacts)
                .unwrap_or_else(|error| panic!("read {stage} lane artifacts: {error:?}"))
                .count(),
            0,
            "{stage} lane artifacts must start empty",
        );
        let merge_path = lane.merge_log_path(temp_dir.path());
        let merge_metadata = fs::metadata(&merge_path)
            .unwrap_or_else(|error| panic!("inspect {stage} lane merge log: {error:?}"));
        assert!(
            merge_metadata.is_file() && merge_metadata.len() == 0,
            "{stage} lane merge log must be a fresh empty file",
        );
    };
    kura.install_lane_incarnation_marker_for_test(lane, incarnation_b, 0)
        .expect("activate first incarnation B");
    assert_eq!(
        kura.active_lane_incarnation_marker(lane)
            .expect("read first incarnation-B marker"),
        (incarnation_b, 0),
    );
    assert_eq!(
        kura.persist_historical_autonomous_lane_recovery_record(&first_b_record)
            .expect("persist first-B historical recovery"),
        HistoricalAutonomousLaneRecoveryPersistOutcome::Installed,
    );
    kura.persist_committed_lane_block_session(&first_b_session, &first_b_pops)
        .expect("persist first-B QC evidence");
    let first_b_record_path = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        temp_dir.path(),
        first_b_record.recovery_id,
    );
    let first_b_record_relative = first_b_record_path
        .strip_prefix(lane.blocks_dir(temp_dir.path()))
        .expect("first-B recovery lives below its lane segment")
        .to_path_buf();
    let first_b_archive = archive_dir.path().join("incarnation-b-first");
    fs::rename(lane.blocks_dir(temp_dir.path()), &first_b_archive)
        .expect("archive first incarnation B");
    let archived_first_b_record = first_b_archive.join(first_b_record_relative);
    assert!(archived_first_b_record.is_file());
    recreate_lane_storage("incarnation-A");
    assert!(
        !Kura::historical_autonomous_recovery_path_for_entry(
            lane,
            temp_dir.path(),
            incarnation_a_record.recovery_id,
        )
        .exists(),
        "incarnation-A storage must not inherit first-B recovery bytes",
    );
    kura.install_lane_incarnation_marker_for_test(lane, incarnation_a, 60)
        .expect("activate intermediate incarnation A");
    assert_eq!(
        kura.active_lane_incarnation_marker(lane)
            .expect("read incarnation-A marker"),
        (incarnation_a, 60),
    );
    assert_eq!(
        kura.persist_historical_autonomous_lane_recovery_record(&incarnation_a_record)
            .expect("persist incarnation-A historical recovery"),
        HistoricalAutonomousLaneRecoveryPersistOutcome::Installed,
    );
    kura.persist_committed_lane_block_session(&incarnation_a_session, &incarnation_a_pops)
        .expect("persist incarnation-A QC evidence");
    let incarnation_a_record_path = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        temp_dir.path(),
        incarnation_a_record.recovery_id,
    );
    let incarnation_a_record_relative = incarnation_a_record_path
        .strip_prefix(lane.blocks_dir(temp_dir.path()))
        .expect("incarnation-A recovery lives below its lane segment")
        .to_path_buf();
    let incarnation_a_archive = archive_dir.path().join("incarnation-a");
    fs::rename(lane.blocks_dir(temp_dir.path()), &incarnation_a_archive)
        .expect("archive intermediate incarnation A");
    let archived_incarnation_a_record = incarnation_a_archive.join(incarnation_a_record_relative);
    assert!(archived_incarnation_a_record.is_file());
    recreate_lane_storage("recreated-B");
    assert!(
        !Kura::historical_autonomous_recovery_path_for_entry(
            lane,
            temp_dir.path(),
            recreated_b_record.recovery_id,
        )
        .exists(),
        "recreated-B storage must not inherit earlier B/A recovery bytes",
    );
    kura.install_lane_incarnation_marker_for_test(lane, incarnation_b, 100)
        .expect("activate recreated incarnation B with a fresh activation fence");
    assert_eq!(
        kura.active_lane_incarnation_marker(lane)
            .expect("read recreated-B marker"),
        (incarnation_b, 100),
    );
    assert_eq!(
        kura.persist_historical_autonomous_lane_recovery_record(&recreated_b_record)
            .expect("persist recreated-B historical recovery"),
        HistoricalAutonomousLaneRecoveryPersistOutcome::Installed,
    );
    kura.persist_committed_lane_block_session(&recreated_b_session, &recreated_b_pops)
        .expect("persist recreated-B QC evidence");
    let recreated_b_group =
        autonomous_reservation_reconciliation_group(recreated_b.reservation_keys.clone());
    assert!(matches!(
        kura.classify_autonomous_lane_reservation_group(
            &recreated_b_group,
            network_id,
            epoch,
        ),
        Ok(AutonomousLaneReservationEvidenceV1::ExactLive {
            payload,
            certification,
        }) if payload == recreated_b && certification.is_certified()
    ));
    for stale_record in [&first_b_record, &incarnation_a_record] {
        assert!(
            kura.persist_historical_autonomous_lane_recovery_record(stale_record)
                .is_err(),
            "an earlier B/A recovery record must not hydrate into recreated-B storage",
        );
    }
    for (stale_session, stale_pops) in [
        (&first_b_session, &first_b_pops),
        (&incarnation_a_session, &incarnation_a_pops),
    ] {
        assert!(
            kura.persist_committed_lane_block_session(stale_session, stale_pops)
                .is_err(),
            "an earlier B/A QC must not overwrite the recreated-B certificate",
        );
    }
    for stale_payload in [&first_b, &incarnation_a_payload] {
        let stale_group =
            autonomous_reservation_reconciliation_group(stale_payload.reservation_keys.clone());
        assert!(matches!(
            kura.classify_autonomous_lane_reservation_group(&stale_group, network_id, epoch,),
            Err(AutonomousLaneReservationEvidenceError::Kura(_))
        ));
    }
    let recreated_b_record_path = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        temp_dir.path(),
        recreated_b_record.recovery_id,
    );
    let recreated_b_record_bytes =
        fs::read(&recreated_b_record_path).expect("read recreated-B recovery bytes");
    for archived_stale_record in [&archived_first_b_record, &archived_incarnation_a_record] {
        let stale_target = recreated_b_record_path.with_file_name(
            archived_stale_record
                .file_name()
                .expect("archived recovery file name"),
        );
        fs::copy(archived_stale_record, &stale_target)
            .expect("inject delayed archived recovery record");
        assert!(
            kura.historical_autonomous_lane_recovery_records_bounded(3)
                .is_err(),
            "a physically delayed B/A record must fail the active marker boundary",
        );
        assert_eq!(
            fs::read(&recreated_b_record_path)
                .expect("read recreated-B record after stale injection"),
            recreated_b_record_bytes,
            "stale inventory bytes must not overwrite the recreated-B seal",
        );
        fs::remove_file(&stale_target).expect("remove delayed stale recovery fixture");
    }
    assert_eq!(
        kura.historical_autonomous_lane_recovery_records_bounded(3)
            .expect("read exact recreated-B recovery inventory"),
        vec![recreated_b_record.clone()],
    );
    assert!(
        kura.historical_autonomous_lane_recovery_record_matches(&recreated_b_record)
            .expect("revalidate recreated-B recovery dependencies"),
    );
    assert_eq!(
        kura.read_autonomous_lane_block_artifact(lane.lane_id, 1, network_id, epoch)
            .expect("read recreated-B autonomous payload")
            .executable_payload,
        recreated_b,
    );
    assert_eq!(
        kura.read_lane_block_execution_input(lane.lane_id, 1)
            .expect("read recreated-B execution input")
            .proposal,
        recreated_b.origin_proposal,
    );
    assert_eq!(
        kura.read_certified_lane_block_artifact(lane.lane_id, 1)
            .expect("read recreated-B certified artifact")
            .proposal,
        recreated_b.origin_proposal,
    );
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("reopen recreated-B Kura");
    assert_eq!(
        reopened
            .historical_autonomous_lane_recovery_records_bounded(3)
            .expect("recover recreated-B inventory after restart"),
        vec![recreated_b_record.clone()],
    );
    assert!(
        reopened
            .historical_autonomous_lane_recovery_record_matches(&recreated_b_record)
            .expect("revalidate recreated-B recovery after restart"),
    );
    assert_eq!(
        reopened
            .read_autonomous_lane_block_artifact(lane.lane_id, 1, network_id, epoch)
            .expect("recover recreated-B payload after restart")
            .executable_payload,
        recreated_b,
    );
    assert_eq!(
        reopened
            .read_certified_lane_block_artifact(lane.lane_id, 1)
            .expect("recover recreated-B QC after restart")
            .proposal,
        recreated_b.origin_proposal,
    );
    assert!(matches!(
        reopened.classify_autonomous_lane_reservation_group(
            &recreated_b_group,
            network_id,
            epoch,
        ),
        Ok(AutonomousLaneReservationEvidenceV1::ExactLive {
            payload,
            certification,
        }) if payload == recreated_b && certification.is_certified()
    ));
    let lane_blocks = lane.blocks_dir(temp_dir.path());
    let historical_byte_limit = reopened.historical_autonomous_recovery_aggregate_byte_limit();
    let with_recovery =
        Kura::block_store_bytes_with_historical_limit(&lane_blocks, historical_byte_limit)
            .expect("measure recreated-B block store");
    let accounting_probe = archive_dir
        .path()
        .join("recreated-b-accounting-probe.norito");
    fs::rename(&recreated_b_record_path, &accounting_probe)
        .expect("temporarily move recreated-B recovery for exact accounting");
    let without_recovery =
        Kura::block_store_bytes_with_historical_limit(&lane_blocks, historical_byte_limit)
            .expect("measure recreated-B block store without recovery");
    fs::rename(&accounting_probe, &recreated_b_record_path)
        .expect("restore recreated-B recovery after exact accounting");
    assert_eq!(
        with_recovery.checked_sub(without_recovery),
        Some(
            u64::try_from(recreated_b_record_bytes.len())
                .expect("recreated-B recovery length fits u64")
        ),
        "nested historical recovery bytes must be counted exactly once",
    );
    let accounting = reopened
        .disk_usage_accounting_snapshot_for_tests()
        .expect("read post-restart disk accounting");
    assert!(
        accounting.enforced_initialized && accounting.total_initialized,
        "restart must publish both disk-accounting caches",
    );
    assert_eq!(
        accounting.cached_enforced_bytes, accounting.exact_enforced_bytes,
        "restart enforced accounting must include nested recovery evidence",
    );
    assert_eq!(
        accounting.cached_total_bytes, accounting.exact_total_bytes,
        "restart total accounting must include nested recovery evidence",
    );
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
    let roster = descriptor
        .validator_set
        .iter()
        .cloned()
        .map(|validator| ValidatorPower {
            validator,
            power: 1,
        })
        .collect::<Vec<_>>();
    let historical_context = HeightContext {
        network_id: crate::sumeragi::synthetic_network_id("kura-autonomous-chain"),
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
    let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
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
