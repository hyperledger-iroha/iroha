    #[test]
    fn strict_reservation_batch_reads_historical_attempt_instead_of_later_latest() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, first) =
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
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first);
        kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
            .expect("persist first attempt");
        let retirement = AutonomousLaneSlotRetirementV1::from_payload(&first);
        kura.persist_autonomous_lane_slot_retirement(&retirement, chain_id_hash, epoch)
            .expect("retire first attempt");
        let barrier = retirement
            .queue_release_barrier()
            .expect("first release barrier");
        kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, chain_id_hash, epoch)
            .expect("finish first release");
        kura.persist_lane_executable_payload(&successor, chain_id_hash, epoch)
            .expect("persist later latest attempt");

        let classified = kura
            .classify_autonomous_lane_reservation_groups(
                &[first_group, successor_group],
                chain_id_hash,
                &[epoch, epoch],
            )
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
    }

    #[test]
    fn strict_reservation_classifier_rejects_reordered_and_partial_groups() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) = two_reservation_autonomous_lane_payload_for_kura(
            lane.lane_id,
            lane.dataspace_id,
            1,
            &signer,
        );
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist two-reservation payload");

        let exact = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
        assert!(matches!(
            kura.classify_autonomous_lane_reservation_group(&exact, chain_id_hash, epoch),
            Ok(AutonomousLaneReservationEvidenceV1::ExactLive { .. })
        ));

        let mut reordered_keys = payload.reservation_keys.clone();
        reordered_keys.reverse();
        let reordered = autonomous_reservation_reconciliation_group(reordered_keys);
        assert!(matches!(
            kura.classify_autonomous_lane_reservation_group(&reordered, chain_id_hash, epoch),
            Err(AutonomousLaneReservationEvidenceError::ReservationVectorConflict)
        ));

        let partial = autonomous_reservation_reconciliation_group(vec![payload.reservation_keys[0]]);
        assert!(matches!(
            kura.classify_autonomous_lane_reservation_group(&partial, chain_id_hash, epoch),
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
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
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
            kura.classify_autonomous_lane_reservation_group(&group, chain_id_hash, epoch),
            Err(AutonomousLaneReservationEvidenceError::Kura(_))
        ));
        assert_eq!(
            fs::read(&attempt_path).expect("read malformed attempt after classification"),
            malformed,
            "read-only classification must not recover or rewrite malformed evidence",
        );
    }

    #[test]
    fn strict_reservation_classifier_exposes_exact_certification() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist autonomous payload");
        let (session, signer_pops) =
            committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, &signer);
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .expect("persist exact certified lane artifact");

        let classified = kura
            .classify_autonomous_lane_reservation_group(&group, chain_id_hash, epoch)
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
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
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

        let outcome =
            kura.classify_autonomous_lane_reservation_group(&group, chain_id_hash, epoch);
        assert!(matches!(
            &outcome,
            Err(AutonomousLaneReservationEvidenceError::UnresolvedTemporary { path })
                if path == &temp_path
        ), "unexpected unresolved-attempt classification: {outcome:?}");
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
        let (chain_id_hash, epoch, missing) =
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
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &missing);
        kura.persist_lane_executable_payload(&other, chain_id_hash, epoch)
            .expect("persist only the competing proposal-height attempt");

        assert!(matches!(
            kura.classify_autonomous_lane_reservation_group(&missing_group, chain_id_hash, epoch,),
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
        let (chain_id_hash, epoch, payload) =
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
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist exact autonomous payload");
        let (session, signer_pops) =
            committed_lane_block_session_for_kura_proposal(&conflicting.origin_proposal, &signer);
        let conflicting_artifact = CertifiedLaneBlockArtifact::new(session, signer_pops);
        let conflicting_payload = conflicting_artifact
            .encode_framed()
            .expect("encode conflicting same-height certification");
        let (data_path, index_path) =
            Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            payload.origin_proposal.descriptor.lane_block_height,
            &conflicting_payload,
            "strict reservation conflicting certification fixture",
            FsyncMode::Always,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));

        assert!(matches!(
            kura.classify_autonomous_lane_reservation_group(&group, chain_id_hash, epoch),
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
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        let descriptor = &payload.origin_proposal.descriptor;
        let attempt_path = Kura::autonomous_lane_block_attempt_path_for_entry(
            lane,
            temp_dir.path(),
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let target_path = temp_dir.path().join("outside-autonomous-attempt");
        let target_bytes = b"must not be followed or changed";
        fs::write(&target_path, target_bytes).expect("write symlink target");
        symlink(&target_path, &attempt_path).expect("install symlinked attempt");

        assert!(matches!(
            kura.classify_autonomous_lane_reservation_group(&group, chain_id_hash, epoch),
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
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let group = autonomous_reservation_reconciliation_group(payload.reservation_keys.clone());
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        let (data_path, index_path) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
        let index_len = (MAX_AUTONOMOUS_RESERVATION_CERTIFIED_INDEX_ENTRIES + 1)
            .checked_mul(PIPELINE_INDEX_ENTRY_SIZE)
            .expect("oversized certified index length");
        fs::write(&data_path, b"").expect("write empty certified data");
        fs::write(&index_path, vec![0_u8; index_len]).expect("write oversized certified index");

        assert!(matches!(
            kura.classify_autonomous_lane_reservation_group(&group, chain_id_hash, epoch),
            Err(AutonomousLaneReservationEvidenceError::Kura(_))
        ));
        assert_eq!(
            fs::metadata(&index_path)
                .expect("oversized index remains")
                .len(),
            u64::try_from(index_len).expect("index length fits u64"),
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
        let (chain_id_hash, epoch, payload) =
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
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
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
            kura.classify_autonomous_lane_reservation_group(&group, chain_id_hash, epoch),
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
        let (chain_id_hash, epoch, historical) =
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
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &historical);
        kura.persist_lane_executable_payload(&historical, chain_id_hash, epoch)
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
            kura.classify_autonomous_lane_reservation_group(&group, chain_id_hash, epoch),
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
        let (chain_id_hash, epoch, payload) =
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
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist exact payload and claims");
        let entrypoint_hash = payload.entrypoint_hashes[0];
        let claim_path = Kura::autonomous_lane_entrypoint_claim_path(
            temp_dir.path(),
            &chain_id_hash,
            &entrypoint_hash,
        );
        let temp_path = Kura::autonomous_lane_entrypoint_claim_temp_path(&claim_path);
        let conflicting_claim = AutonomousLaneEntrypointClaimV3::new(&conflicting, entrypoint_hash);
        let temp_bytes =
            norito::encode_canonical(&conflicting_claim).expect("encode conflicting claim temp");
        fs::write(&temp_path, &temp_bytes).expect("write conflicting claim temp");

        let outcome =
            kura.classify_autonomous_lane_reservation_group(&group, chain_id_hash, epoch);
        assert!(matches!(
            &outcome,
            Err(AutonomousLaneReservationEvidenceError::EntrypointClaimConflict { path })
                if path == &temp_path
        ), "unexpected conflicting-claim classification: {outcome:?}");
        assert_eq!(
            fs::read(&temp_path).expect("read unchanged conflicting claim temp"),
            temp_bytes,
            "claim preflight must not remove or promote a conflicting stage",
        );
    }
