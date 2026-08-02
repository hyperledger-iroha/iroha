    #[test]
    fn autonomous_entrypoint_claim_release_repairs_crash_and_allows_reproposal() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_entry = lane_config.entry(LaneId::new(1)).expect("lane entry");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) = autonomous_lane_payload_for_kura(
            lane_entry.lane_id,
            lane_entry.dataspace_id,
            1,
            &signer,
        );
        let successor = rebind_autonomous_lane_payload_for_kura(
            &payload,
            lane_entry.lane_id,
            lane_entry.dataspace_id,
            2,
            b"kura-autonomous-view-incarnation",
            &signer,
        );
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist first autonomous payload");
        let claim_path = Kura::autonomous_lane_entrypoint_claim_path(
            temp_dir.path(),
            &chain_id_hash,
            &payload.entrypoint_hashes[0],
        );
        let claim_temp_path = Kura::autonomous_lane_entrypoint_claim_temp_path(&claim_path);
        fs::rename(&claim_path, &claim_temp_path)
            .expect("model crash after payload append but before claim promotion");
        assert!(
            !claim_path.exists() && claim_temp_path.exists(),
            "crash fixture retains only the staged exact owner",
        );
        let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
        kura.persist_autonomous_lane_slot_retirement(&retirement, chain_id_hash, epoch)
            .expect("retirement promotes and marks the staged exact owner release-pending");

        let pending =
            Kura::decode_autonomous_lane_entrypoint_claim(&claim_path).expect("pending claim");
        assert_eq!(
            pending.state,
            AutonomousLaneEntrypointClaimStateV3::ReleasePending(
                retirement.digest().expect("retirement digest")
            ),
        );
        assert!(
            kura.persist_lane_executable_payload(&successor, chain_id_hash, epoch)
                .is_err(),
            "ReleasePending must remain exclusive until the exact Queue barrier is durable",
        );

        // Model a crash immediately after the slot tombstone reached disk but
        // before this particular claim replacement did.
        let active = AutonomousLaneEntrypointClaimV3::new(&payload, payload.entrypoint_hashes[0]);
        fs::write(
            &claim_path,
            norito::to_bytes(&active).expect("encode active claim"),
        )
        .expect("restore interrupted active claim");
        drop(kura);

        let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
        reopened
            .persist_autonomous_lane_slot_retirement(&retirement, chain_id_hash, epoch)
            .expect("startup retry completes ReleasePending");
        let repaired =
            Kura::decode_autonomous_lane_entrypoint_claim(&claim_path).expect("repaired claim");
        assert_eq!(
            repaired.state,
            AutonomousLaneEntrypointClaimStateV3::ReleasePending(
                retirement.digest().expect("retirement digest")
            ),
        );

        let barrier = retirement
            .queue_release_barrier()
            .expect("build exact queue barrier");
        let mut conflicting_barrier = barrier.clone();
        conflicting_barrier.executable_payload_hash =
            Hash::new(b"conflicting-queue-release-payload");
        assert!(
            reopened
                .finalize_autonomous_lane_slot_release(
                    &retirement,
                    &conflicting_barrier,
                    chain_id_hash,
                    epoch,
                )
                .is_err(),
            "a barrier with different payload identity must not release claims",
        );
        reopened
            .finalize_autonomous_lane_slot_release(&retirement, &barrier, chain_id_hash, epoch)
            .expect("exact durable Queue barrier releases claims");
        reopened
            .finalize_autonomous_lane_slot_release(&retirement, &barrier, chain_id_hash, epoch)
            .expect("released claim retry is idempotent");
        let released =
            Kura::decode_autonomous_lane_entrypoint_claim(&claim_path).expect("released claim");
        assert_eq!(
            released.state,
            AutonomousLaneEntrypointClaimStateV3::Released(
                retirement.digest().expect("retirement digest")
            ),
        );
        reopened
            .persist_lane_executable_payload(&successor, chain_id_hash, epoch)
            .expect("released entrypoint can be reproposed at the next exact slot");
        let successor_claim =
            Kura::decode_autonomous_lane_entrypoint_claim(&claim_path).expect("successor claim");
        assert!(successor_claim.active_for_payload(&successor));
        assert!(
            reopened
                .persist_lane_executable_payload(&payload, chain_id_hash, epoch)
                .is_err(),
            "the delayed retired payload must not reclaim its old slot",
        );

        drop(reopened);
        let (restarted, _) = Kura::new(&config, &lane_config).expect("restart after reproposal");
        restarted
            .persist_lane_executable_payload(&successor, chain_id_hash, epoch)
            .expect("successor ownership remains idempotent after restart");
    }

    #[test]
    fn autonomous_claim_runtime_inventory_enforces_boundary_without_partial_staging() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (_, _, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        let first_filler = write_autonomous_claim_inventory_fixture(
            temp_dir.path(),
            &payload,
            Hash::new(b"claim-inventory-first-filler"),
            false,
        );
        let first_filler_bytes = fs::read(&first_filler).expect("read first filler");
        let target_path = Kura::autonomous_lane_entrypoint_claim_path(
            temp_dir.path(),
            &payload.chain_id_hash,
            &payload.entrypoint_hashes[0],
        );
        let target_temp = Kura::autonomous_lane_entrypoint_claim_temp_path(&target_path);

        let _guard = kura.sidecar_lock.lock();
        let staged = kura
            .prepare_autonomous_lane_entrypoint_claims_with_limit_locked(&payload, 2)
            .expect("one free inventory slot admits one crash-staged claim");
        assert_eq!(staged.len(), 1);
        assert!(
            target_temp.is_file(),
            "the exact boundary must include the staged temp"
        );
        assert_eq!(
            kura.inspect_autonomous_lane_entrypoint_claim_inventory(2)
                .expect("inspect exact boundary"),
            2,
        );

        fs::remove_file(&target_temp).expect("remove first boundary-stage fixture");
        let second_filler = write_autonomous_claim_inventory_fixture(
            temp_dir.path(),
            &payload,
            Hash::new(b"claim-inventory-second-filler"),
            false,
        );
        let second_filler_bytes = fs::read(&second_filler).expect("read second filler");
        assert!(
            kura.prepare_autonomous_lane_entrypoint_claims_with_limit_locked(&payload, 2)
                .is_err(),
            "a full live inventory must reject staging before creating any file",
        );
        assert!(!target_path.exists());
        assert!(!target_temp.exists());
        assert_eq!(
            fs::read(&first_filler).expect("first filler remains"),
            first_filler_bytes,
        );
        assert_eq!(
            fs::read(&second_filler).expect("second filler remains"),
            second_filler_bytes,
        );
    }

    #[test]
    fn autonomous_claim_startup_inventory_bound_fails_before_temp_reconciliation() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (_, _, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        let main = write_autonomous_claim_inventory_fixture(
            temp_dir.path(),
            &payload,
            Hash::new(b"claim-startup-bound-main"),
            false,
        );
        let orphan_temp = write_autonomous_claim_inventory_fixture(
            temp_dir.path(),
            &payload,
            Hash::new(b"claim-startup-bound-orphan"),
            true,
        );
        let main_bytes = fs::read(&main).expect("read main claim");
        let orphan_bytes = fs::read(&orphan_temp).expect("read orphan temp");

        let _guard = kura.sidecar_lock.lock();
        assert!(
            kura.reconcile_autonomous_lane_entrypoint_claim_temps_on_startup_with_limit_locked(1)
                .is_err(),
            "startup must reject an oversized main-plus-temp inventory",
        );
        assert_eq!(
            fs::read(&main).expect("main survives rejected startup reconciliation"),
            main_bytes,
        );
        assert_eq!(
            fs::read(&orphan_temp).expect("temp survives rejected startup reconciliation"),
            orphan_bytes,
            "the read-only inventory pass must reject before cleaning a prefix",
        );
        kura.reconcile_autonomous_lane_entrypoint_claim_temps_on_startup_with_limit_locked(2)
            .expect("the exact startup boundary is admitted");
        assert!(main.is_file());
        assert!(
            !orphan_temp.exists(),
            "bounded startup reconciliation removes the proven orphan"
        );
        assert_eq!(
            kura.inspect_autonomous_lane_entrypoint_claim_inventory(1)
                .expect("one stable main remains"),
            1,
        );
    }

    #[test]
    fn autonomous_claim_inventory_rejects_unexpected_artifacts_before_any_cleanup_or_stage() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (_, _, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        let orphan_temp = write_autonomous_claim_inventory_fixture(
            temp_dir.path(),
            &payload,
            Hash::new(b"claim-unexpected-orphan"),
            true,
        );
        let orphan_bytes = fs::read(&orphan_temp).expect("read orphan temp");
        let unexpected = orphan_temp
            .parent()
            .expect("claim shard")
            .join("unexpected.claim.backup");
        fs::write(&unexpected, b"not a claim").expect("write unexpected claim artifact");
        let target_path = Kura::autonomous_lane_entrypoint_claim_path(
            temp_dir.path(),
            &payload.chain_id_hash,
            &payload.entrypoint_hashes[0],
        );
        let target_temp = Kura::autonomous_lane_entrypoint_claim_temp_path(&target_path);

        {
            let _guard = kura.sidecar_lock.lock();
            assert!(
                kura.prepare_autonomous_lane_entrypoint_claims_with_limit_locked(&payload, 8)
                    .is_err(),
                "live preparation must fail closed on an unexpected namespace artifact",
            );
            assert!(
                kura.reconcile_autonomous_lane_entrypoint_claim_temps_on_startup_with_limit_locked(
                    8,
                )
                .is_err(),
                "startup reconciliation must share the same fail-closed inventory",
            );
        }
        assert!(!target_path.exists());
        assert!(!target_temp.exists());
        assert_eq!(
            fs::read(&orphan_temp).expect("orphan remains before rejected cleanup"),
            orphan_bytes,
        );
        drop(kura);
        assert!(
            Kura::new(&config, &lane_config).is_err(),
            "a real restart must reject the unexpected claim artifact",
        );
    }

    #[test]
    fn autonomous_entrypoint_claim_rejects_legacy_and_unknown_states() {
        #[derive(Encode)]
        struct LegacyClaimV2 {
            version: u16,
            chain_id_hash: Hash,
            epoch: u64,
            entrypoint_hash: Hash,
            lane_id: LaneId,
            dataspace_id: DataSpaceId,
            lane_incarnation: Hash,
            proposal_height: u64,
            lane_block_height: u64,
            origin_proposal_hash: Hash,
            executable_payload_hash: Hash,
            released_by_retirement_hash: Option<Hash>,
        }

        #[derive(Encode)]
        enum UnknownClaimState {
            #[codec(index = 99)]
            Unknown,
        }

        #[derive(Encode)]
        struct UnknownClaimV3 {
            version: u16,
            chain_id_hash: Hash,
            epoch: u64,
            entrypoint_hash: Hash,
            lane_id: LaneId,
            dataspace_id: DataSpaceId,
            lane_incarnation: Hash,
            proposal_height: u64,
            lane_block_height: u64,
            origin_proposal_hash: Hash,
            executable_payload_hash: Hash,
            state: UnknownClaimState,
        }

        let temp_dir = TempDir::new().expect("temp dir");
        let chain_id_hash = Hash::new(b"legacy-claim-chain");
        let entrypoint_hash = Hash::new(b"legacy-claim-entrypoint");
        let path = Kura::autonomous_lane_entrypoint_claim_path(
            temp_dir.path(),
            &chain_id_hash,
            &entrypoint_hash,
        );
        fs::create_dir_all(path.parent().expect("claim parent")).expect("create claim parent");
        let common = (
            chain_id_hash,
            entrypoint_hash,
            LaneId::new(1),
            DataSpaceId::new(3),
            Hash::new(b"legacy-claim-incarnation"),
            Hash::new(b"legacy-claim-proposal"),
            Hash::new(b"legacy-claim-payload"),
        );
        let legacy = LegacyClaimV2 {
            version: 2,
            chain_id_hash: common.0,
            epoch: 4,
            entrypoint_hash: common.1,
            lane_id: common.2,
            dataspace_id: common.3,
            lane_incarnation: common.4,
            proposal_height: 8,
            lane_block_height: 5,
            origin_proposal_hash: common.5,
            executable_payload_hash: common.6,
            released_by_retirement_hash: None,
        };
        fs::write(
            &path,
            norito::to_bytes(&legacy).expect("encode legacy claim"),
        )
        .expect("write legacy claim");
        assert!(
            Kura::decode_autonomous_lane_entrypoint_claim(&path).is_err(),
            "version-two claim layouts must fail closed"
        );

        let unknown = UnknownClaimV3 {
            version: AutonomousLaneEntrypointClaimV3::VERSION,
            chain_id_hash: common.0,
            epoch: 4,
            entrypoint_hash: common.1,
            lane_id: common.2,
            dataspace_id: common.3,
            lane_incarnation: common.4,
            proposal_height: 8,
            lane_block_height: 5,
            origin_proposal_hash: common.5,
            executable_payload_hash: common.6,
            state: UnknownClaimState::Unknown,
        };
        fs::write(
            &path,
            norito::to_bytes(&unknown).expect("encode unknown claim"),
        )
        .expect("write unknown claim");
        assert!(
            Kura::decode_autonomous_lane_entrypoint_claim(&path).is_err(),
            "unknown claim state tags must fail closed"
        );
    }

    #[test]
    fn autonomous_lane_slot_retirement_rejects_conflict_and_incarnation_aba() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_entry = lane_config.entry(LaneId::new(1)).expect("lane entry");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) = autonomous_lane_payload_for_kura(
            lane_entry.lane_id,
            lane_entry.dataspace_id,
            1,
            &signer,
        );
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist autonomous payload");

        let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
        let mut conflicting = retirement.clone();
        conflicting.origin_proposal_hash = Hash::new(b"conflicting-retirement-proposal");
        assert!(
            kura.persist_autonomous_lane_slot_retirement(&conflicting, chain_id_hash, epoch,)
                .is_err(),
            "a caller cannot retire a different proposal identity",
        );
        assert!(
            kura.read_autonomous_lane_slot_retirement(lane_entry.lane_id, 1, chain_id_hash, epoch,)
                .expect("conflicting attempt leaves a readable slot")
                .is_none(),
        );
        kura.persist_autonomous_lane_slot_retirement(&retirement, chain_id_hash, epoch)
            .expect("persist exact retirement");

        let recreated = rebind_autonomous_lane_payload_for_kura(
            &payload,
            lane_entry.lane_id,
            lane_entry.dataspace_id,
            1,
            b"kura-autonomous-retirement-recreated-incarnation",
            &signer,
        );
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &recreated);
        assert!(
            kura.persist_autonomous_lane_slot_retirement(&retirement, chain_id_hash, epoch,)
                .is_err(),
            "a delayed exact tombstone cannot target a recreated lane incarnation",
        );
        assert!(
            kura.persist_autonomous_lane_slot_retirement(
                &AutonomousLaneSlotRetirementV1::from_payload(&recreated),
                chain_id_hash,
                epoch,
            )
            .is_err(),
            "a fresh-incarnation tombstone requires its own durable payload first",
        );
        assert!(
            kura.read_autonomous_lane_slot_retirement(lane_entry.lane_id, 1, chain_id_hash, epoch,)
                .is_err(),
            "the old tombstone must never validate under the recreated active marker",
        );
    }

    #[test]
    fn autonomous_lane_slot_retirement_repairs_temp_and_rejects_bad_files() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::new(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist autonomous payload");
        let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
        kura.persist_autonomous_lane_slot_retirement(&retirement, chain_id_hash, epoch)
            .expect("persist retirement");

        let view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
            lane_entry,
            temp_dir.path(),
            1,
            payload.origin_proposal.descriptor.proposal_height,
        );
        let temp_path = Kura::autonomous_lane_block_view_state_temp_path(&view_path);
        let valid_bytes = fs::read(&view_path).expect("read canonical retirement");
        fs::rename(&view_path, &temp_path).expect("stage retirement rename crash");
        fs::write(&view_path, &valid_bytes[..valid_bytes.len() / 2])
            .expect("stage truncated main after crash");
        assert_eq!(
            kura.read_autonomous_lane_slot_retirement(lane_id, 1, chain_id_hash, epoch)
                .expect("promote valid retirement temp"),
            Some(retirement.clone()),
        );
        assert!(!temp_path.exists(), "recovered retirement temp is removed");

        fs::write(&view_path, &valid_bytes[..valid_bytes.len() / 2])
            .expect("truncate retirement without recovery temp");
        assert!(
            kura.read_autonomous_lane_slot_retirement(lane_id, 1, chain_id_hash, epoch)
                .is_err(),
            "truncated retirement must fail closed",
        );
        fs::write(&view_path, &valid_bytes).expect("restore retirement after truncation");

        fs::write(&view_path, [0xFF, 0x00, 0xAA]).expect("corrupt retirement");
        assert!(
            kura.read_autonomous_lane_slot_retirement(lane_id, 1, chain_id_hash, epoch)
                .is_err(),
            "corrupt retirement must fail closed",
        );
        fs::write(&view_path, &valid_bytes).expect("restore retirement after corruption");

        fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&view_path)
            .expect("open retirement for oversized fixture")
            .set_len(
                u64::try_from(AUTONOMOUS_LANE_BLOCK_VIEW_STATE_MAX_BYTES)
                    .expect("view limit fits u64")
                    .saturating_add(1),
            )
            .expect("make sparse oversized retirement");
        assert!(
            kura.read_autonomous_lane_slot_retirement(lane_id, 1, chain_id_hash, epoch)
                .is_err(),
            "oversized retirement must fail before allocation or decode",
        );
        fs::write(&view_path, &valid_bytes).expect("restore retirement after oversize");

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let real_path = view_path.with_extension("norito.real");
            fs::rename(&view_path, &real_path).expect("move retirement behind symlink");
            symlink(&real_path, &view_path).expect("symlink retirement");
            assert!(
                kura.read_autonomous_lane_slot_retirement(lane_id, 1, chain_id_hash, epoch)
                    .is_err(),
                "symlinked retirement must fail closed",
            );
            fs::remove_file(&view_path).expect("remove retirement symlink");
            fs::rename(&real_path, &view_path).expect("restore regular retirement");

            symlink(&view_path, &temp_path).expect("symlink crash temp");
            assert!(
                kura.read_autonomous_lane_slot_retirement(lane_id, 1, chain_id_hash, epoch)
                    .is_err(),
                "symlinked retirement temp must fail closed even beside a valid main",
            );
            fs::remove_file(&temp_path).expect("remove crash-temp symlink");
        }
        assert_eq!(
            kura.read_autonomous_lane_slot_retirement(lane_id, 1, chain_id_hash, epoch)
                .expect("regular retirement survives adversarial files"),
            Some(retirement),
        );
    }

    #[test]
    fn autonomous_lane_slot_retirement_rejects_already_certified_slot() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::new(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist autonomous payload");
        let (session, signer_pops) =
            committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, &signer);
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .expect("persist certified autonomous slot");
        assert!(
            kura.persist_autonomous_lane_slot_retirement(
                &AutonomousLaneSlotRetirementV1::from_payload(&payload),
                chain_id_hash,
                epoch,
            )
            .is_err(),
            "a certified autonomous slot cannot release its queue ownership",
        );
        assert!(
            kura.read_autonomous_lane_slot_retirement(lane_id, 1, chain_id_hash, epoch)
                .expect("certified slot remains readable")
                .is_none(),
        );
    }

    #[test]
    fn autonomous_merge_bundle_certifies_origin_while_new_view_advances_cursor() {
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::new(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
        let origin = payload.origin_proposal.clone();
        let availability = durable_lane_payload_availability_for_kura(&payload, &origin, &signer);
        let new_view = next_durable_lane_view_certificate_for_kura(
            &origin,
            &payload,
            &signer,
            chain_id_hash,
            epoch,
        );
        let cursor = crate::lane_consensus::retarget_lane_block_proposal_view(
            &origin,
            new_view.certificate.body.target_view,
        )
        .expect("synthetic NewView cursor");
        let signer_pops = BTreeMap::from([(
            signer.public_key().clone(),
            bls_normal_pop_prove(signer.private_key()).expect("certification signer PoP"),
        )]);
        let origin_commit_vote =
            signed_lane_block_vote_for_kura(&origin, CertPhase::Commit, &signer);
        let origin_commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            origin_commit_vote.body.clone(),
            origin.descriptor.validator_set.clone(),
            std::slice::from_ref(&origin_commit_vote),
        )
        .expect("origin commit QC");
        let autonomous = AutonomousLaneBlockArtifact {
            format: AutonomousLaneBlockArtifactFormat::Current,
            executable_payload: payload.clone(),
            availability_certificate: Some(availability.clone()),
            view_checkpoint: None,
            new_view_certificates: vec![new_view],
        };
        let certified_origin = CertifiedLaneBlockArtifact::new(
            crate::lane_consensus::CommittedLaneBlockSession {
                proposal: origin.clone(),
                prepare_qc: availability.certificate.clone(),
                commit_qc: origin_commit_qc,
            },
            signer_pops.clone(),
        );
        let bundle = AutonomousLaneMergeBundleV1 {
            version: 1,
            autonomous: autonomous.clone(),
            certified: certified_origin,
        };
        Kura::validate_autonomous_lane_merge_bundle(&bundle, chain_id_hash, epoch)
            .expect("origin certification remains valid after the cursor advances");

        let cursor_availability =
            durable_lane_payload_availability_for_kura(&payload, &cursor, &signer);
        let cursor_commit_vote =
            signed_lane_block_vote_for_kura(&cursor, CertPhase::Commit, &signer);
        let cursor_commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            cursor_commit_vote.body.clone(),
            cursor.descriptor.validator_set.clone(),
            std::slice::from_ref(&cursor_commit_vote),
        )
        .expect("synthetic cursor commit QC");
        let cursor_bundle = AutonomousLaneMergeBundleV1 {
            version: 1,
            autonomous: autonomous.clone(),
            certified: CertifiedLaneBlockArtifact::new(
                crate::lane_consensus::CommittedLaneBlockSession {
                    proposal: cursor,
                    prepare_qc: cursor_availability.certificate.clone(),
                    commit_qc: cursor_commit_qc,
                },
                signer_pops,
            ),
        };
        assert_eq!(
            Kura::validate_autonomous_lane_merge_bundle(&cursor_bundle, chain_id_hash, epoch,),
            Err("autonomous lane merge bundle must certify the immutable origin proposal"),
            "a fully signed synthetic cursor must not become the merge certification subject",
        );

        let mut poisoned_availability = bundle;
        poisoned_availability.autonomous.availability_certificate = Some(cursor_availability);
        assert_eq!(
            Kura::validate_autonomous_lane_merge_bundle(
                &poisoned_availability,
                chain_id_hash,
                epoch,
            ),
            Err("invalid autonomous lane payload availability certificate"),
            "the durable artifact must reject a next-view READY QC before merge validation",
        );
    }

    #[test]
    fn autonomous_execution_input_validation_does_not_repair_view_sidecars() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::new(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist autonomous payload");
        let recovered = kura
            .recover_autonomous_lane_block_payload(&payload.origin_proposal, chain_id_hash, epoch)
            .expect("recover execution input before crash");

        let view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
            lane_entry,
            &kura.store_root,
            1,
            payload.origin_proposal.descriptor.proposal_height,
        );
        let temp_path = Kura::autonomous_lane_block_view_state_temp_path(&view_path);
        let canonical_bytes = fs::read(&view_path).expect("read canonical view state");
        fs::write(&temp_path, &canonical_bytes).expect("stage valid crash temp");
        let truncated_bytes = canonical_bytes[..canonical_bytes.len() / 2].to_vec();
        fs::write(&view_path, &truncated_bytes).expect("truncate main view state");
        let main_before = fs::read(&view_path).expect("snapshot truncated main");
        let temp_before = fs::read(&temp_path).expect("snapshot valid crash temp");

        assert!(
            kura.persist_lane_block_execution_input(&recovered).is_err(),
            "execution-input validation must fail closed on the malformed main view sidecar",
        );
        assert_eq!(
            fs::read(&view_path).expect("main view state after validation"),
            main_before,
            "non-repair validation must not promote the autonomous crash temp",
        );
        assert_eq!(
            fs::read(&temp_path).expect("crash temp after validation"),
            temp_before,
            "non-repair validation must not delete the autonomous crash temp",
        );

        assert_eq!(
            kura.recover_autonomous_lane_block_payload(
                &payload.origin_proposal,
                chain_id_hash,
                epoch,
            )
            .expect("ordinary recovery promotes the valid crash temp"),
            recovered,
        );
        assert_eq!(
            fs::read(&view_path).expect("repaired main view state"),
            canonical_bytes,
        );
        assert!(
            !temp_path.exists(),
            "ordinary recovery must remove the promoted crash temp",
        );
    }

    #[test]
    fn autonomous_lane_view_compacts_at_257_and_recovers_crash_atomically() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::new(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist payload");

        let mut current = payload.origin_proposal.clone();
        let mut certificate_prefix = Vec::with_capacity(256);
        for _ in 1..=256 {
            let durable = next_durable_lane_view_certificate_for_kura(
                &current,
                &payload,
                &signer,
                chain_id_hash,
                epoch,
            );
            current = crate::lane_consensus::retarget_lane_block_proposal_view(
                &current,
                durable.certificate.body.target_view,
            )
            .expect("advance fixture view");
            certificate_prefix.push(durable);
        }
        let store_root = kura.store_root();
        let view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
            lane_entry,
            &store_root,
            1,
            payload.origin_proposal.descriptor.proposal_height,
        );
        {
            let _guard = kura.sidecar_lock.lock();
            kura.write_autonomous_lane_block_view_state_locked(
                &AutonomousLaneBlockArtifact {
                    format: AutonomousLaneBlockArtifactFormat::Current,
                    executable_payload: payload.clone(),
                    availability_certificate: None,
                    view_checkpoint: None,
                    new_view_certificates: certificate_prefix,
                },
                &view_path,
                chain_id_hash,
                epoch,
            )
            .expect("persist bounded certificate prefix");
        }
        let artifact = kura
            .read_autonomous_lane_block_artifact(lane_id, 1, chain_id_hash, epoch)
            .expect("view 256 artifact");
        assert!(artifact.view_checkpoint.is_none());
        assert_eq!(artifact.new_view_certificates.len(), 256);

        for target_view in 257..=258 {
            let durable = next_durable_lane_view_certificate_for_kura(
                &current,
                &payload,
                &signer,
                chain_id_hash,
                epoch,
            );
            current = kura
                .persist_lane_new_view_certificate(lane_id, 1, durable, chain_id_hash, epoch)
                .expect("persist NewView certificate");
            if target_view == 257 {
                let artifact = kura
                    .read_autonomous_lane_block_artifact(lane_id, 1, chain_id_hash, epoch)
                    .expect("view 257 artifact");
                let checkpoint = artifact.view_checkpoint.expect("compacted checkpoint");
                assert_eq!(checkpoint.source_proposal.descriptor.lane_block_view, 256);
                assert_eq!(checkpoint.target_proposal.descriptor.lane_block_view, 257);
                assert!(artifact.new_view_certificates.is_empty());
            }
        }
        assert_eq!(current.descriptor.lane_block_view, 258);
        drop(kura);

        let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
        let recovered = reopened
            .current_autonomous_lane_payload(lane_id, 1, chain_id_hash, epoch)
            .expect("restart recovery");
        assert_eq!(recovered.1.descriptor.lane_block_view, 258);
        let snapshot = reopened
            .latest_autonomous_lane_block_artifacts_snapshot(chain_id_hash, 1, |_| epoch)
            .expect("load bounded route-latest snapshot");
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].1.descriptor.lane_block_view, 258);
        assert!(
            reopened
                .latest_autonomous_lane_block_artifacts_snapshot(chain_id_hash, 0, |_| epoch)
                .expect("zero-cap snapshot is empty")
                .is_empty(),
            "a zero global recovery limit must not enumerate durable history"
        );

        let temp_path = Kura::autonomous_lane_block_view_state_temp_path(&view_path);
        let valid_bytes = fs::read(&view_path).expect("read valid view state");
        fs::write(&temp_path, &valid_bytes).expect("stage crash temp");
        fs::write(&view_path, &valid_bytes[..valid_bytes.len() / 2])
            .expect("truncate main view state");
        assert_eq!(
            reopened
                .current_autonomous_lane_payload(lane_id, 1, chain_id_hash, epoch)
                .expect("valid crash temp is promoted")
                .1
                .descriptor
                .lane_block_view,
            258
        );
        assert!(!temp_path.exists(), "recovered temp should be promoted");

        fs::write(&view_path, [0xFF, 0x00, 0xAA]).expect("corrupt view state");
        assert!(
            reopened
                .read_autonomous_lane_block_artifact(lane_id, 1, chain_id_hash, epoch)
                .is_none(),
            "corrupt view state must not fall back to the origin proposal"
        );
    }

    #[test]
    fn autonomous_payload_promotes_hint_free_bytes_to_one_exact_carrier_hint() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, mut hint_free) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let hint = hint_free
            .origin_proposal
            .payload_block_hint
            .take()
            .expect("fixture carrier hint");
        hint_free
            .validate(chain_id_hash, epoch)
            .expect("hint-free payload remains authenticated");
        let hinted = hint_free
            .attach_global_hint_exact(hint, chain_id_hash, epoch)
            .expect("attach exact carrier hint");
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &hint_free);
        kura.persist_lane_executable_payload(&hint_free, chain_id_hash, epoch)
            .expect("persist hint-free local payload");
        kura.persist_lane_executable_payload(&hinted, chain_id_hash, epoch)
            .expect("promote to exact carrier-hinted payload");
        assert_eq!(
            kura.current_autonomous_lane_payload(lane.lane_id, 1, chain_id_hash, epoch)
                .expect("current promoted payload")
                .0,
            hinted,
        );
        assert!(
            kura.persist_lane_executable_payload(&hint_free, chain_id_hash, epoch)
                .is_err(),
            "carrier-hint promotion must never be reversed",
        );

        drop(kura);
        let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
        assert_eq!(
            reopened
                .current_autonomous_lane_payload(lane.lane_id, 1, chain_id_hash, epoch)
                .expect("restart recovers promoted payload")
                .0,
            hinted,
        );
    }

    #[test]
    fn autonomous_payload_rejects_a_conflicting_carrier_hint_after_promotion() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, mut hint_free) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let first_hint = hint_free
            .origin_proposal
            .payload_block_hint
            .take()
            .expect("fixture carrier hint");
        let first = hint_free
            .attach_global_hint_exact(first_hint, chain_id_hash, epoch)
            .expect("attach first carrier hint");
        let conflicting_hint = iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
            proposal_height: first_hint.proposal_height,
            proposal_view: first_hint.proposal_view.saturating_add(1),
            proposal_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"conflicting-autonomous-carrier-hint",
            )),
        };
        let conflicting = hint_free
            .attach_global_hint_exact(conflicting_hint, chain_id_hash, epoch)
            .expect("build independently authenticated conflicting hint form");
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &hint_free);
        kura.persist_lane_executable_payload(&hint_free, chain_id_hash, epoch)
            .expect("persist hint-free local payload");
        kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
            .expect("promote first carrier hint");
        assert!(
            kura.persist_lane_executable_payload(&conflicting, chain_id_hash, epoch)
                .is_err(),
            "a different carrier identity must not replace the promoted hint",
        );
        assert_eq!(
            kura.current_autonomous_lane_payload(lane.lane_id, 1, chain_id_hash, epoch)
                .expect("first carrier remains current")
                .0,
            first,
        );
    }

    #[test]
    fn autonomous_first_attempt_uses_only_versioned_files_and_repairs_missing_pointers() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let proposal_height = payload.origin_proposal.descriptor.proposal_height;
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist first versioned attempt");

        let artifact_dir = Kura::lane_artifact_dir(&lane.blocks_dir(temp_dir.path()));
        let attempt_path = Kura::autonomous_lane_block_attempt_path_for_entry(
            lane,
            temp_dir.path(),
            1,
            proposal_height,
        );
        let view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
            lane,
            temp_dir.path(),
            1,
            proposal_height,
        );
        let height_pointer =
            Kura::autonomous_lane_block_latest_attempt_path_for_entry(lane, temp_dir.path(), 1);
        let route_pointer =
            Kura::autonomous_lane_route_latest_attempt_path_for_entry(lane, temp_dir.path());
        assert!(attempt_path.is_file());
        assert!(view_path.is_file());
        assert!(height_pointer.is_file());
        assert!(route_pointer.is_file());
        assert!(
            !artifact_dir
                .join(OBSOLETE_AUTONOMOUS_LANE_BLOCKS_DATA_FILE)
                .exists()
                && !artifact_dir
                    .join(OBSOLETE_AUTONOMOUS_LANE_BLOCKS_INDEX_FILE)
                    .exists(),
            "the coordinated first release must never emit the deleted indexed layout",
        );

        fs::remove_file(&view_path).expect("model crash before initial view publication");
        fs::remove_file(&height_pointer).expect("model crash before height-pointer publication");
        fs::remove_file(&route_pointer).expect("model crash before route-pointer publication");
        let staged_claims = payload
            .entrypoint_hashes
            .iter()
            .map(|entrypoint_hash| {
                let claim = Kura::autonomous_lane_entrypoint_claim_path(
                    temp_dir.path(),
                    &chain_id_hash,
                    entrypoint_hash,
                );
                let staged = Kura::autonomous_lane_entrypoint_claim_temp_path(&claim);
                fs::rename(&claim, &staged)
                    .expect("model crash after staging claim but before claim promotion");
                (claim, staged)
            })
            .collect::<Vec<_>>();
        let atomic_temp = artifact_dir.join(".kura-sidecar-crash-residue");
        fs::write(&atomic_temp, b"unpublished atomic write")
            .expect("stage pre-rename atomic crash residue");
        drop(kura);

        let (reopened, _) = Kura::new(&config, &lane_config)
            .expect("startup reconstructs the exact immutable attempt");
        assert!(!atomic_temp.exists());
        assert!(view_path.is_file());
        assert!(height_pointer.is_file());
        assert!(route_pointer.is_file());
        for (claim, staged) in staged_claims {
            assert!(claim.is_file());
            assert!(!staged.exists());
        }
        assert_eq!(
            reopened
                .current_autonomous_lane_payload(lane.lane_id, 1, chain_id_hash, epoch)
                .expect("reconstructed first attempt")
                .0,
            payload,
        );
    }

    #[test]
    fn autonomous_startup_rejects_a_view_removed_after_pointer_publication() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let proposal_height = payload.origin_proposal.descriptor.proposal_height;
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist complete attempt");
        let view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
            lane,
            temp_dir.path(),
            1,
            proposal_height,
        );
        fs::remove_file(view_path).expect("remove view after pointer and claim publication");
        drop(kura);

        assert!(
            Kura::new(&config, &lane_config).is_err(),
            "startup must not reconstruct a view whose later durability boundaries prove it once existed",
        );
    }

    #[test]
    fn autonomous_startup_rejects_an_unretired_same_height_orphan_successor() {
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
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first);
        kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
            .expect("persist active first attempt");
        {
            let _guard = kura.sidecar_lock.lock();
            let artifact = AutonomousLaneBlockArtifact::new(successor);
            let state = AutonomousLaneBlockViewState::from_artifact(&artifact);
            kura.write_autonomous_lane_block_attempt_locked(
                lane,
                &artifact,
                &state,
                chain_id_hash,
                epoch,
            )
            .expect("stage a crash-orphaned successor attempt");
        }
        drop(kura);

        assert!(
            Kura::new(&config, &lane_config).is_err(),
            "startup must not select a successor until the prior attempt is durably retired",
        );
    }

    #[test]
    fn autonomous_startup_rejects_an_aggregate_oversized_attempt_namespace() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        let artifact_dir = Kura::lane_artifact_dir(&lane.blocks_dir(temp_dir.path()));
        fs::create_dir_all(&artifact_dir).expect("create lane artifact directory");
        drop(kura);

        let file_len = u64::try_from(MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES)
            .expect("autonomous source limit fits u64");
        let file_count = AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES
            .checked_div(MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES)
            .expect("non-zero autonomous source limit")
            .saturating_add(1);
        assert!(file_count < MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES);
        for index in 0..file_count {
            let lane_block_height = u64::try_from(index)
                .expect("bounded test index fits u64")
                .saturating_add(1);
            let path = artifact_dir.join(format!(
                "{AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX}_{lane_block_height:020}_{lane_block_height:020}.norito"
            ));
            fs::File::create(path)
                .expect("create sparse autonomous view")
                .set_len(file_len)
                .expect("extend sparse autonomous view");
        }

        assert!(
            Kura::new(&config, &lane_config).is_err(),
            "startup must reject a namespace whose individually bounded files exceed the aggregate byte budget",
        );
    }

    #[test]
    fn finalized_release_allows_a_later_proposal_attempt_at_the_same_lane_height() {
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
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first);
        kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
            .expect("persist first attempt");
        let retirement = AutonomousLaneSlotRetirementV1::from_payload(&first);
        kura.persist_autonomous_lane_slot_retirement(&retirement, chain_id_hash, epoch)
            .expect("retire first attempt");
        let barrier = retirement
            .queue_release_barrier()
            .expect("exact Queue release barrier");
        kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, chain_id_hash, epoch)
            .expect("finalize first attempt release");
        kura.persist_lane_executable_payload(&successor, chain_id_hash, epoch)
            .expect("persist successor at the same lane-local height");
        assert_eq!(
            kura.current_autonomous_lane_payload(lane.lane_id, 1, chain_id_hash, epoch)
                .expect("successor is current")
                .0,
            successor,
        );
        assert_eq!(
            kura.read_autonomous_lane_slot_retirement(lane.lane_id, 1, chain_id_hash, epoch)
                .expect("current attempt retirement lookup"),
            None,
            "the old tombstone must not retire the fresh attempt",
        );
        assert!(
            kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
                .is_err(),
            "the delayed old payload must not reclaim the current attempt",
        );
        let route_latest_path =
            Kura::autonomous_lane_route_latest_attempt_path_for_entry(lane, temp_dir.path());
        drop(kura);
        fs::remove_file(&route_latest_path)
            .expect("model restart with a missing derived route-latest index");
        let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
        assert!(
            route_latest_path.is_file(),
            "startup must explicitly reconstruct the missing route-latest index",
        );
        assert_eq!(
            reopened
                .current_autonomous_lane_payload(lane.lane_id, 1, chain_id_hash, epoch)
                .expect("restart resolves the route-latest successor")
                .0,
            successor,
        );
    }

    #[test]
    fn autonomous_route_latest_snapshot_rejects_runtime_index_corruption() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist autonomous payload");

        let route_latest_path =
            Kura::autonomous_lane_route_latest_attempt_path_for_entry(lane, temp_dir.path());
        fs::write(&route_latest_path, [0xFF, 0x00, 0xAA])
            .expect("corrupt the live route-latest pointer");
        assert!(
            kura.latest_autonomous_lane_block_artifacts_snapshot(chain_id_hash, 1, |_| epoch)
                .is_err(),
            "runtime hydration must fail closed instead of hiding durable queue ownership",
        );
    }

    #[test]
    fn old_reservation_retirement_remains_exactly_addressable_after_same_height_reproposal() {
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
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first);
        kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
            .expect("persist first attempt");
        let retirement = AutonomousLaneSlotRetirementV1::from_payload(&first);
        kura.persist_autonomous_lane_slot_retirement(&retirement, chain_id_hash, epoch)
            .expect("retire first attempt");
        let barrier = retirement
            .queue_release_barrier()
            .expect("exact Queue release barrier");
        kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, chain_id_hash, epoch)
            .expect("finalize first attempt release");
        kura.persist_lane_executable_payload(&successor, chain_id_hash, epoch)
            .expect("persist successor attempt");

        assert_eq!(
            kura.autonomous_lane_retirement_matching_reservation(
                &first.reservation_keys[0],
                chain_id_hash,
                epoch,
            )
            .expect("proposal-height-indexed old retirement lookup"),
            Some(retirement.clone()),
        );
        kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, chain_id_hash, epoch)
            .expect("old finalized release remains idempotently provable");
        assert!(
            kura.autonomous_lane_payload_matches_reservation(
                &successor.reservation_keys[0],
                chain_id_hash,
                epoch,
            ),
            "current reservation lookup must resolve only the fresh attempt",
        );
        drop(kura);
        let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
        assert_eq!(
            reopened
                .autonomous_lane_retirement_matching_reservation(
                    &first.reservation_keys[0],
                    chain_id_hash,
                    epoch,
                )
                .expect("restart old retirement lookup"),
            Some(retirement),
        );
    }

    #[test]
    fn autonomous_payload_duplicate_requires_exact_producer_authenticated_bytes() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let first_signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let second_signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, template) =
            autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &first_signer);

        let mut proposal = template.origin_proposal.clone();
        let mut validator_set = vec![
            PeerId::new(first_signer.public_key().clone()),
            PeerId::new(second_signer.public_key().clone()),
        ];
        validator_set.sort();
        proposal.descriptor.validator_set = validator_set.clone();
        proposal.descriptor.validator_set_hash = HashOf::new(&validator_set);
        proposal.descriptor.validator_count =
            u32::try_from(validator_set.len()).expect("validator count");
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();

        let mut reservation_keys = template.reservation_keys.clone();
        for reservation in &mut reservation_keys {
            reservation.proposal_identity_hash = proposal.proposal_hash;
        }
        let build_payload = |signer: &KeyPair| {
            LaneExecutablePayloadV1::new_signed_with_reservations(
                chain_id_hash,
                epoch,
                proposal.clone(),
                template.entrypoints.clone(),
                reservation_keys.clone(),
                template.routing_plans.clone(),
                template.native_amx_receipts.clone(),
                PeerId::new(signer.public_key().clone()),
                signer.private_key(),
            )
            .expect("construct producer-authenticated payload")
        };
        let first = build_payload(&first_signer);
        let second = build_payload(&second_signer);
        assert_eq!(first.payload_hash, second.payload_hash);
        assert_ne!(first, second);

        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first);
        kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
            .expect("persist first producer identity");
        assert!(
            kura.persist_lane_executable_payload(&second, chain_id_hash, epoch)
                .is_err(),
            "same payload hash from another producer must not alias the durable payload"
        );
        assert_eq!(
            kura.read_autonomous_lane_block_artifact(lane.lane_id, 1, chain_id_hash, epoch,)
                .expect("read exact durable payload")
                .executable_payload,
            first
        );
    }

    #[test]
    fn autonomous_entrypoint_claim_rejects_replay_after_restart_and_recovers_temp() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_one = lane_config.entry(LaneId::new(1)).expect("lane one");
        let lane_zero = lane_config.entry(LaneId::SINGLE).expect("lane zero");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, payload) =
            autonomous_lane_payload_for_kura(lane_one.lane_id, lane_one.dataspace_id, 1, &signer);
        let cross_lane = rebind_autonomous_lane_payload_for_kura(
            &payload,
            lane_zero.lane_id,
            lane_zero.dataspace_id,
            1,
            b"claim-cross-lane-incarnation",
            &signer,
        );
        let later_height = rebind_autonomous_lane_payload_for_kura(
            &payload,
            lane_one.lane_id,
            lane_one.dataspace_id,
            2,
            b"kura-autonomous-view-incarnation",
            &signer,
        );
        let recreated_incarnation = rebind_autonomous_lane_payload_for_kura(
            &payload,
            lane_one.lane_id,
            lane_one.dataspace_id,
            1,
            b"claim-recreated-lane-incarnation",
            &signer,
        );

        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("persist payload");
        kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
            .expect("exact payload retry is idempotent");
        for replay in [&cross_lane, &later_height, &recreated_incarnation] {
            assert!(
                kura.persist_lane_executable_payload(replay, chain_id_hash, epoch)
                    .is_err(),
                "another live lane execution domain must not claim the entrypoint"
            );
        }

        let claim_path = Kura::autonomous_lane_entrypoint_claim_path(
            temp_dir.path(),
            &chain_id_hash,
            &payload.entrypoint_hashes[0],
        );
        assert!(claim_path.is_file(), "durable exact-key claim is missing");
        drop(kura);

        let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
        for replay in [&cross_lane, &later_height, &recreated_incarnation] {
            assert!(
                reopened
                    .persist_lane_executable_payload(replay, chain_id_hash, epoch)
                    .is_err(),
                "restart must retain cross-session entrypoint ownership"
            );
        }

        let claim_temp = Kura::autonomous_lane_entrypoint_claim_temp_path(&claim_path);
        fs::rename(&claim_path, &claim_temp).expect("simulate claim promotion crash");
        drop(reopened);
        let (recovered, _) =
            Kura::new(&config, &lane_config).expect("startup promotes the exact durable owner");
        assert!(claim_path.is_file());
        assert!(!claim_temp.exists());
        assert!(
            recovered
                .persist_lane_executable_payload(&cross_lane, chain_id_hash, epoch)
                .is_err(),
            "recovered claim must reject a delayed conflicting lane payload"
        );

        let orphan_entrypoint_hash = Hash::new(b"orphan-startup-entrypoint-claim");
        let orphan_claim =
            AutonomousLaneEntrypointClaimV3::new(&later_height, orphan_entrypoint_hash);
        let orphan_path = Kura::autonomous_lane_entrypoint_claim_path(
            temp_dir.path(),
            &chain_id_hash,
            &orphan_entrypoint_hash,
        );
        let orphan_temp = Kura::autonomous_lane_entrypoint_claim_temp_path(&orphan_path);
        fs::create_dir_all(orphan_path.parent().expect("orphan claim parent"))
            .expect("create orphan claim shard");
        fs::write(
            &orphan_temp,
            norito::to_bytes(&orphan_claim).expect("encode orphan staged claim"),
        )
        .expect("stage claim whose payload never became durable");
        drop(recovered);
        let (recovered, _) =
            Kura::new(&config, &lane_config).expect("startup discards an unpublished claim temp");
        assert!(!orphan_path.exists());
        assert!(!orphan_temp.exists());

        fs::write(&claim_path, [0xFF, 0x00, 0xAA]).expect("corrupt claim");
        assert!(
            recovered
                .persist_lane_executable_payload(&payload, chain_id_hash, epoch)
                .is_err(),
            "a present malformed claim must fail closed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn autonomous_claim_startup_rejects_symlinks_and_multiple_links_without_following_them() {
        use std::os::unix::fs::symlink;

        for corruption in ["symlink", "hardlink"] {
            let temp_dir = TempDir::new().expect("temp dir");
            let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
            let lane_config = two_lane_runtime_config();
            let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
            let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
            let (chain_id_hash, epoch, payload) =
                autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
            let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
            install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
            kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
                .expect("persist payload and claim");
            let claim_path = Kura::autonomous_lane_entrypoint_claim_path(
                temp_dir.path(),
                &chain_id_hash,
                &payload.entrypoint_hashes[0],
            );
            let original = fs::read(&claim_path).expect("read durable claim");
            let outside = temp_dir.path().join(format!("{corruption}-claim-sentinel"));
            match corruption {
                "symlink" => {
                    fs::write(&outside, &original).expect("write outside claim sentinel");
                    fs::remove_file(&claim_path).expect("remove canonical claim");
                    symlink(&outside, &claim_path).expect("install claim symlink");
                }
                "hardlink" => {
                    fs::hard_link(&claim_path, &outside).expect("install second claim link");
                }
                _ => unreachable!("enumerated corruption"),
            }
            let claim_temp = Kura::autonomous_lane_entrypoint_claim_temp_path(&claim_path);
            {
                let _guard = kura.sidecar_lock.lock();
                assert!(
                    kura.prepare_autonomous_lane_entrypoint_claims_with_limit_locked(&payload, 8)
                        .is_err(),
                    "{corruption} claim must fail live inventory preflight closed",
                );
            }
            assert!(
                !claim_temp.exists(),
                "failed live preflight must not stage a replacement claim",
            );
            drop(kura);

            assert!(
                Kura::new(&config, &lane_config).is_err(),
                "{corruption} claim must fail startup closed",
            );
            assert_eq!(
                fs::read(&outside).expect("outside claim sentinel remains readable"),
                original,
                "claim validation must not mutate an external link target",
            );
        }
    }

    #[test]
    fn autonomous_payload_slot_is_bound_to_the_active_incarnation_marker() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_entry = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (chain_id_hash, epoch, first) = autonomous_lane_payload_for_kura(
            lane_entry.lane_id,
            lane_entry.dataspace_id,
            1,
            &signer,
        );
        let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
        assert!(
            kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
                .is_err(),
            "an executable payload must not define an uninitialized storage incarnation"
        );
        kura.install_lane_incarnation_marker_for_test(
            lane_entry,
            first.origin_proposal.descriptor.lane_incarnation,
            first.origin_proposal.descriptor.proposal_height,
        )
        .expect("install activation-fence marker");
        assert!(
            kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
                .is_err(),
            "an executable payload at the incarnation activation height must be rejected",
        );

        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first);
        kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
            .expect("persist first marker-bound incarnation");
        let first_execution_input = kura
            .recover_autonomous_lane_block_payload(&first.origin_proposal, chain_id_hash, epoch)
            .expect("recover first marker-bound execution input");
        kura.persist_lane_block_execution_input(&first_execution_input)
            .expect("persist first marker-bound execution input");
        assert_eq!(
            kura.read_lane_block_execution_input(lane_entry.lane_id, 1)
                .expect("read first marker-bound execution input")
                .proposal,
            first.origin_proposal,
        );
        let (first_session, first_signer_pops) =
            committed_lane_block_session_for_kura_proposal(&first.origin_proposal, &signer);
        kura.persist_committed_lane_block_session(&first_session, &first_signer_pops)
            .expect("persist first marker-bound certified session");
        let application_state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"marker-bound direct application state"),
        ));
        let application_result =
            TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
        let first_input = kura
            .read_lane_block_execution_input(lane_entry.lane_id, 1)
            .expect("read first execution input for preflight");
        kura.persist_lane_block_execution_preflight(
            &first_input,
            7,
            application_state_hash,
            vec![application_result.clone()],
        )
        .expect("persist first marker-bound preflight");
        let first_preflight = kura
            .read_lane_block_execution_preflight(lane_entry.lane_id, 1)
            .expect("read first marker-bound preflight");
        kura.persist_direct_lane_block_application_receipt(&first_input, &first_preflight)
            .expect("persist first marker-bound direct receipt");
        let first_direct_snapshot =
            kura.active_direct_lane_block_application_receipts_structural_snapshot();
        assert_eq!(first_direct_snapshot.len(), 1);

        let recreated = rebind_autonomous_lane_payload_for_kura(
            &first,
            lane_entry.lane_id,
            lane_entry.dataspace_id,
            1,
            b"kura-autonomous-recreated-incarnation",
            &signer,
        );

        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &recreated);
        assert!(
            !kura.active_direct_lane_block_application_receipts_match_structural_snapshot(
                &first_direct_snapshot,
            ),
            "snapshot revalidation must reject a receipt set captured before lane recreation",
        );
        assert!(
            kura.read_lane_block_execution_input(lane_entry.lane_id, 1)
                .is_none(),
            "switching the authoritative marker must hide the retired execution input",
        );
        assert!(
            kura.read_certified_lane_block_artifact(lane_entry.lane_id, 1)
                .is_none(),
            "switching the marker must hide a retired certified session",
        );
        assert!(
            kura.read_lane_block_execution_preflight(lane_entry.lane_id, 1)
                .is_none(),
            "switching the marker must hide a retired execution preflight",
        );
        assert!(
            kura.read_lane_block_application_receipt(lane_entry.lane_id, 1)
                .is_none(),
            "switching the marker must hide a retired direct receipt",
        );
        assert!(
            kura.direct_lane_block_application_receipts_snapshot()
                .is_empty(),
            "direct receipt snapshots must not mix in a retired incarnation",
        );
        kura.persist_lane_executable_payload(&recreated, chain_id_hash, epoch)
            .expect("the authoritative fresh marker admits the recreated incarnation");
        let recreated_execution_input = kura
            .recover_autonomous_lane_block_payload(&recreated.origin_proposal, chain_id_hash, epoch)
            .expect("recover recreated marker-bound execution input");
        kura.persist_lane_block_execution_input(&recreated_execution_input)
            .expect("replace the retired execution input under the fresh marker");
        let (recreated_session, recreated_signer_pops) =
            committed_lane_block_session_for_kura_proposal(&recreated.origin_proposal, &signer);
        kura.persist_committed_lane_block_session(&recreated_session, &recreated_signer_pops)
            .expect("replace the retired certified session under the fresh marker");
        let recreated_input = kura
            .read_lane_block_execution_input(lane_entry.lane_id, 1)
            .expect("read recreated execution input for preflight");
        kura.persist_lane_block_execution_preflight(
            &recreated_input,
            8,
            application_state_hash,
            vec![application_result],
        )
        .expect("replace the retired preflight under the fresh marker");
        let recreated_preflight = kura
            .read_lane_block_execution_preflight(lane_entry.lane_id, 1)
            .expect("read recreated marker-bound preflight");
        kura.persist_direct_lane_block_application_receipt(&recreated_input, &recreated_preflight)
            .expect("replace the retired direct receipt under the fresh marker");
        assert!(
            kura.persist_lane_executable_payload(&first, chain_id_hash, epoch)
                .is_err(),
            "a delayed first-incarnation replay must not replace the recreated lane slot"
        );
        assert!(
            kura.persist_committed_lane_block_session(&first_session, &first_signer_pops)
                .is_err(),
            "a delayed certified-session replay must not replace the recreated lane slot",
        );
        assert!(
            kura.persist_lane_block_execution_input(&first_execution_input)
                .is_err(),
            "a delayed execution-input replay must not replace the recreated lane slot",
        );
        assert!(
            kura.persist_lane_block_execution_preflight(
                &first_input,
                7,
                application_state_hash,
                first_preflight.results.clone(),
            )
            .is_err(),
            "a delayed preflight replay must not replace the recreated lane slot",
        );
        assert!(
            kura.persist_direct_lane_block_application_receipt(&first_input, &first_preflight)
                .is_err(),
            "a delayed direct-receipt replay must not replace the recreated lane slot",
        );
        assert_eq!(
            kura.read_autonomous_lane_block_artifact(lane_entry.lane_id, 1, chain_id_hash, epoch,)
                .expect("read recreated marker-bound payload")
                .executable_payload,
            recreated
        );
        assert_eq!(
            kura.read_lane_block_execution_input(lane_entry.lane_id, 1)
                .expect("read recreated marker-bound execution input")
                .proposal,
            recreated.origin_proposal,
        );
        assert_eq!(
            kura.read_certified_lane_block_artifact(lane_entry.lane_id, 1)
                .expect("read recreated certified session")
                .proposal,
            recreated.origin_proposal,
        );
        assert_eq!(
            kura.read_lane_block_execution_preflight(lane_entry.lane_id, 1)
                .expect("read recreated preflight")
                .proposal,
            recreated.origin_proposal,
        );
        assert_eq!(
            kura.read_lane_block_application_receipt(lane_entry.lane_id, 1)
                .expect("read recreated direct receipt")
                .proposal,
            recreated.origin_proposal,
        );
        assert_eq!(
            kura.direct_lane_block_application_receipts_snapshot()
                .into_iter()
                .map(|receipt| receipt.proposal)
                .collect::<Vec<_>>(),
            vec![recreated.origin_proposal.clone()],
        );
        drop(kura);

        let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
        assert!(
            reopened
                .persist_lane_executable_payload(&first, chain_id_hash, epoch)
                .is_err(),
            "restart must retain the fresh incarnation marker against ABA replay"
        );
        assert_eq!(
            reopened
                .read_autonomous_lane_block_artifact(lane_entry.lane_id, 1, chain_id_hash, epoch,)
                .expect("restart recovers recreated payload")
                .executable_payload,
            recreated
        );
        assert_eq!(
            reopened
                .read_lane_block_execution_input(lane_entry.lane_id, 1)
                .expect("restart recovers recreated execution input")
                .proposal,
            recreated.origin_proposal,
        );
        assert_eq!(
            reopened
                .read_certified_lane_block_artifact(lane_entry.lane_id, 1)
                .expect("restart recovers recreated certified session")
                .proposal,
            recreated.origin_proposal,
        );
        assert_eq!(
            reopened
                .read_lane_block_execution_preflight(lane_entry.lane_id, 1)
                .expect("restart recovers recreated preflight")
                .proposal,
            recreated.origin_proposal,
        );
        assert_eq!(
            reopened
                .read_lane_block_application_receipt(lane_entry.lane_id, 1)
                .expect("restart recovers recreated direct receipt")
                .proposal,
            recreated.origin_proposal,
        );
    }

    fn assert_lane_artifact_files_absent_or_empty(
        lane_entry: &LaneConfigEntry,
        store_root: &std::path::Path,
    ) {
        let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, store_root);
        for path in [data_path, index_path] {
            if let Ok(metadata) = fs::metadata(&path) {
                assert_eq!(
                    metadata.len(),
                    0,
                    "lane artifact file was not rolled back: {path:?}"
                );
            }
        }
    }

    #[test]
    fn lane_block_artifact_persists_under_lane_segment_and_reloads() {
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
        let block_hash = block.hash();
        let expected_ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(Arc::clone(&block))
            .expect("store block with lane artifact");

        let artifact = kura
            .read_lane_block_artifact(lane_id, lane_block_height)
            .expect("lane block artifact");
        assert_eq!(artifact.format_label(), "lane.block_artifact");
        assert_eq!(artifact.proposal_block_hash, block_hash);
        assert_eq!(artifact.ownership, expected_ownership);

        let (data_path, index_path) =
            Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
        assert!(data_path.is_file(), "lane artifact data file missing");
        assert!(index_path.is_file(), "lane artifact index file missing");

        drop(kura);
        let (reloaded, _) = Kura::new(&config, &lane_config).expect("reopen kura");
        assert_eq!(
            reloaded.read_lane_block_artifact(lane_id, lane_block_height),
            Some(artifact)
        );
    }

    #[test]
    fn latest_lane_block_artifact_scan_counts_sparse_and_malformed_slots_exactly() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let block = dummy_block_with_lane_payload_ownership(lane_id, lane_entry.dataspace_id, 1);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store canonical lane artifact");
        let active = kura
            .read_lane_block_artifact(lane_id, 1)
            .expect("read canonical lane artifact");
        let malformed_high = active
            .encode_framed()
            .expect("encode mismatched-height lane artifact");
        let (data_path, index_path) =
            Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
        let boundary_height =
            u64::try_from(CONSENSUS_SIDECAR_MATCH_SCAN_BUDGET).expect("scan budget fits u64");

        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            boundary_height,
            &malformed_high,
            "lane block artifact",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert_eq!(
            kura.latest_lane_block_artifact(lane_id),
            Some(active.clone()),
            "one malformed slot, every absent gap, and the active slot fit exactly in the budget",
        );

        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            boundary_height.saturating_add(1),
            &malformed_high,
            "lane block artifact",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert!(
            kura.latest_lane_block_artifact(lane_id).is_none(),
            "the same valid match one slot beyond the budget must fail closed",
        );
        assert_eq!(
            kura.read_lane_block_artifact(lane_id, 1),
            Some(active),
            "bounded latest lookup must not mutate the canonical low slot",
        );
    }

    #[test]
    fn lane_block_artifact_recreation_repairs_canonical_slot_and_bounds_retired_history_scan() {
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
        let first_artifact = LaneBlockArtifact::new(
            first.hash(),
            first
                .execution_context()
                .expect("first execution context")
                .lane_payload_ownerships[0]
                .clone(),
        );
        let first_incarnation = first_artifact.ownership.lane_incarnation;
        let recreated_incarnation = Hash::new(b"recreated-lane-ownership-sidecar-incarnation");

        let mut second = generator.next().as_ref().clone();
        let mut second_ownership = sample_lane_payload_ownership_for_kura(
            &second,
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        );
        rebind_lane_payload_ownership_for_kura(
            &mut second_ownership,
            recreated_incarnation,
            lane_block_height,
        );
        let second_context = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
                b"recreated-lane-artifact-entrypoint",
            )),
            lane_id,
            lane_entry.dataspace_id,
        )])
        .with_lane_payload_ownerships(vec![second_ownership.clone()]);
        second.set_execution_context(Some(second_context));
        let second = Arc::new(second);
        let second_artifact = LaneBlockArtifact::new(second.hash(), second_ownership.clone());
        let second_proposal = lane_block_proposal_from_ownership(&second_ownership);

        let (kura, _) = Kura::new(&config, &lane_config).expect("init Kura");
        assert!(
            kura.store_block(Arc::clone(&first)).is_err(),
            "ownership persistence must reject an uninitialized active marker"
        );
        kura.install_lane_incarnation_marker_for_test(
            lane_entry,
            first_incarnation,
            first_artifact.ownership.proposal_height,
        )
        .expect("install activation-fence marker");
        assert!(
            kura.store_block(Arc::clone(&first)).is_err(),
            "proposal height equal to activation must be rejected"
        );
        kura.install_lane_incarnation_marker_for_test(lane_entry, first_incarnation, 0)
            .expect("install first active marker");
        kura.store_block(Arc::clone(&first))
            .expect("store first-incarnation ownership");
        assert_eq!(
            kura.read_lane_block_artifact(lane_id, lane_block_height),
            Some(first_artifact.clone())
        );

        kura.install_lane_incarnation_marker_for_test(
            lane_entry,
            recreated_incarnation,
            first_artifact.ownership.proposal_height,
        )
        .expect("install recreated active marker");
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "the recreated marker must hide retired ownership bytes"
        );
        assert!(kura.latest_lane_block_artifact(lane_id).is_none());
        assert!(kura.lane_block_artifacts_snapshot().is_empty());
        assert!(
            kura.canonical_lane_block_artifacts_at_proposal_height_matching(
                first_artifact.ownership.proposal_height,
                8,
                |_| true,
            )
            .is_empty(),
            "canonical recovery must not hydrate retired ownership into a recreated lane",
        );

        kura.store_block(Arc::clone(&second))
            .expect("recreated lane may replace the retired canonical slot");
        assert_eq!(
            kura.read_lane_block_artifact(lane_id, lane_block_height),
            Some(second_artifact.clone())
        );
        assert_eq!(
            kura.canonical_lane_block_artifacts_at_proposal_height_matching(
                second_artifact.ownership.proposal_height,
                8,
                |_| true,
            ),
            vec![second_artifact.clone()],
        );
        assert!(
            kura.store_block(first).is_err(),
            "a delayed old-incarnation block replay must fail closed"
        );

        let (data_path, index_path) =
            Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
        let mut malformed_active = second_artifact.clone();
        malformed_active.ownership.accepted_transaction_hashes[0] =
            Hash::new(b"malformed active ownership replay hash");
        let malformed_active_payload = malformed_active
            .encode_framed()
            .expect("encode malformed active ownership artifact");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &malformed_active_payload,
            "lane block artifact",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "malformed active ownership bytes must fail closed"
        );
        assert_eq!(
            kura.recover_lane_block_payload(&second_proposal)
                .expect("repair malformed active ownership from canonical block")
                .artifact,
            second_artifact,
        );

        let retired_payload = first_artifact
            .encode_framed()
            .expect("encode retired ownership artifact");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &retired_payload,
            "lane block artifact",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "retired canonical bytes must not be served from the active segment"
        );
        let repaired = kura
            .recover_lane_block_payload(&second_proposal)
            .expect("repair recreated ownership from its canonical block");
        assert_eq!(repaired.artifact, second_artifact);
        assert_eq!(
            kura.read_lane_block_artifact(lane_id, lane_block_height),
            Some(second_artifact.clone()),
            "canonical repair must replace a retired same-slot artifact"
        );

        for stale_height in 2..=u64::try_from(CONSENSUS_SIDECAR_MATCH_SCAN_BUDGET)
            .expect("scan budget fits u64")
            .saturating_add(1)
        {
            let mut stale_ownership = first_artifact.ownership.clone();
            rebind_lane_payload_ownership_for_kura(
                &mut stale_ownership,
                first_incarnation,
                stale_height,
            );
            let stale = LaneBlockArtifact::new(first_artifact.proposal_block_hash, stale_ownership)
                .encode_framed()
                .expect("encode stale high ownership artifact");
            assert!(Kura::append_indexed_sidecar(
                &data_path,
                &index_path,
                stale_height,
                &stale,
                "lane block artifact",
                FsyncMode::Batched,
                None,
                SidecarIndexOrigin::FirstWrite,
            ));
        }
        assert!(
            kura.latest_lane_block_artifact(lane_id).is_none(),
            "sixty-four retired high entries must consume the complete bounded scan",
        );
        assert_eq!(
            kura.lane_block_artifacts_snapshot(),
            vec![second_artifact.clone()],
            "all-artifact replay must exclude retired incarnation history"
        );

        drop(kura);
        let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
        reopened.replace_lane_storage_entries_for_test(&lane_config);
        reopened
            .install_lane_incarnation_marker_for_test(
                lane_entry,
                recreated_incarnation,
                first_artifact.ownership.proposal_height,
            )
            .expect("restore the isolated fixture's authoritative recreated marker");
        reopened
            .store_block(Arc::clone(&second))
            .expect("restore the recreated lane association with the exact canonical block");
        assert!(Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &retired_payload,
            "lane block artifact",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ));
        assert!(
            reopened
                .read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "restart repair must not serve a retired same-slot association"
        );
        let repaired = reopened
            .recover_lane_block_payload(&second_proposal)
            .expect("rehydrate the recreated slot from its canonical block after restart");
        assert_eq!(repaired.artifact, second_artifact);
        assert_eq!(
            reopened.read_lane_block_artifact(lane_id, lane_block_height),
            Some(second_artifact.clone())
        );
        assert!(
            reopened.latest_lane_block_artifact(lane_id).is_none(),
            "restart must preserve the same bounded fail-closed lookup",
        );
        assert_eq!(
            reopened.lane_block_artifacts_snapshot(),
            vec![second_artifact]
        );
    }

    #[test]
    fn lane_block_artifact_canonical_validation_releases_sidecar_before_block_data() {
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

        let block_data_guard = kura.block_data.lock();
        kura.canonical_read_kinds_after_prune_check
            .store(0, Ordering::Release);
        kura.observe_canonical_reads_after_prune_check
            .store(true, Ordering::Release);
        let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
        let reader_kura = Arc::clone(&kura);
        let reader = thread::spawn(move || {
            let artifact = reader_kura.read_lane_block_artifact(lane_id, lane_block_height);
            result_tx.send(artifact).expect("report lane artifact read");
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while (kura
            .canonical_read_kinds_after_prune_check
            .load(Ordering::Acquire)
            & CANONICAL_HASH_READER_OBSERVED)
            == 0
        {
            if Instant::now() >= deadline {
                kura.observe_canonical_reads_after_prune_check
                    .store(false, Ordering::Release);
                drop(block_data_guard);
                reader.join().expect("lane artifact reader");
                panic!("lane artifact reader never reached canonical hash validation");
            }
            thread::yield_now();
        }

        let sidecar_guard = kura
            .sidecar_lock
            .try_lock()
            .expect("lane artifact reader must release sidecar_lock before waiting for block_data");
        drop(sidecar_guard);
        kura.observe_canonical_reads_after_prune_check
            .store(false, Ordering::Release);
        drop(block_data_guard);
        assert!(
            result_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("lane artifact reader completes")
                .is_some(),
            "canonical artifact must remain readable after the lock-order probe"
        );
        reader.join().expect("lane artifact reader");
    }

    #[test]
    fn lane_block_payload_availability_recovers_entrypoints_from_canonical_block() {
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
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let expected_entrypoint = block
            .external_entrypoints_cloned()
            .next()
            .expect("dummy block entrypoint");
        let proposal = lane_block_proposal_from_ownership(&ownership);
        assert!(Kura::lane_block_artifact_matches_descriptor(
            &ownership,
            &proposal.descriptor
        ));
        let mut wrong_height_ownership = ownership.clone();
        wrong_height_ownership.proposal_height =
            wrong_height_ownership.proposal_height.saturating_add(1);
        assert!(
            !Kura::lane_block_artifact_matches_descriptor(
                &wrong_height_ownership,
                &proposal.descriptor,
            ),
            "an ownership from another canonical height must not satisfy artifact recovery"
        );

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store block with lane artifact");

        assert_eq!(
            kura.lane_block_payload_availability(&proposal),
            LaneBlockPayloadAvailability::Available
        );
        let recovered = kura
            .recover_lane_block_payload(&proposal)
            .expect("recover executable lane payload");
        assert_eq!(recovered.proposal, proposal);
        assert_eq!(recovered.artifact.ownership, ownership);
        assert_eq!(recovered.entrypoints, vec![expected_entrypoint]);
    }

    #[test]
    fn lane_block_payload_availability_rebuilds_missing_artifact_sidecar_from_canonical_block() {
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
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let expected_entrypoint = block
            .external_entrypoints_cloned()
            .next()
            .expect("dummy block entrypoint");
        let proposal = lane_block_proposal_from_ownership(&ownership);

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store block with lane artifact");
        let (data_path, index_path) =
            Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
        std::fs::remove_file(&data_path).expect("remove lane artifact data sidecar");
        std::fs::remove_file(&index_path).expect("remove lane artifact index sidecar");
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "test setup should remove the lane artifact sidecar"
        );
        {
            let mut block_data = kura.block_data.lock();
            for (_, block) in block_data.iter_mut() {
                *block = None;
            }
            assert!(
                block_data.iter().all(|(_, block)| block.is_none()),
                "test setup should force recovery through durable block rehydration"
            );
        }

        assert_eq!(
            kura.lane_block_payload_availability(&proposal),
            LaneBlockPayloadAvailability::Available
        );
        let recovered = kura
            .recover_lane_block_payload(&proposal)
            .expect("recover executable lane payload after sidecar rebuild");
        assert_eq!(recovered.proposal, proposal);
        assert_eq!(recovered.artifact.ownership, ownership);
        assert_eq!(recovered.entrypoints, vec![expected_entrypoint]);
        assert_eq!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .expect("rebuilt lane artifact sidecar")
                .ownership,
            ownership
        );
    }

    #[test]
    fn canonical_height_recovery_applies_lifecycle_filter_before_sidecar_write() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let block = dummy_block_with_lane_payload_ownership(lane_id, lane_entry.dataspace_id, 1);
        let proposal_height = block.header().height().get();

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store block with lane artifact");
        let (data_path, index_path) =
            Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
        std::fs::remove_file(&data_path).expect("remove lane artifact data sidecar");
        std::fs::remove_file(&index_path).expect("remove lane artifact index sidecar");

        assert!(
            kura.canonical_lane_block_artifacts_at_proposal_height_matching(
                proposal_height,
                8,
                |_| false,
            )
            .is_empty()
        );
        assert!(
            kura.read_lane_block_artifact(lane_id, 1).is_none(),
            "a rejected old-incarnation ownership must not be written into active storage"
        );
        assert_eq!(
            kura.canonical_lane_block_artifacts_at_proposal_height_matching(
                proposal_height,
                8,
                |_| true,
            )
            .len(),
            1
        );
        assert!(kura.read_lane_block_artifact(lane_id, 1).is_some());
    }

    #[test]
    fn lane_block_payload_availability_rejects_ownership_from_wrong_global_height() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let lane_block_height = 1;
        let mut generator = DummyBlocks::new();
        let canonical_proposal_block = generator.next();
        let mut later_block = generator.next().as_ref().clone();
        let mut ownership = sample_lane_payload_ownership_for_kura(
            &later_block,
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        );
        ownership.proposal_height = canonical_proposal_block.header().height().get();
        let replay_hashes = ownership
            .compute_replay_hashes()
            .expect("wrong-height ownership replay hashes compute");
        ownership.subject_hash = replay_hashes.subject_hash;
        ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
        ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
        let proposal = lane_block_proposal_from_ownership(&ownership);
        let external_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            ownership.accepted_transaction_hashes[0],
        );
        later_block.set_execution_context(Some(
            BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                external_hash,
                lane_id,
                lane_entry.dataspace_id,
            )])
            .with_lane_payload_ownerships(vec![ownership]),
        ));

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(canonical_proposal_block)
            .expect("store canonical proposal-height block");
        kura.store_block(Arc::new(later_block))
            .expect("store later block with forged proposal-height ownership");
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "a sidecar anchored to the wrong global block must not be canonical"
        );

        assert_eq!(
            kura.lane_block_payload_availability(&proposal),
            LaneBlockPayloadAvailability::MissingLaneArtifact,
            "payload recovery must inspect only the descriptor's exact global proposal height"
        );
    }

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

    #[test]
    fn lane_execution_evidence_overrides_batched_fsync_and_reissues_failed_barriers() {
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
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let proposal = lane_block_proposal_from_ownership(&ownership);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store canonical lane payload source");
        let recovered = kura
            .recover_lane_block_payload(&proposal)
            .expect("recover exact execution input");

        for (label, inject_failure) in strict_indexed_sidecar_failure_modes() {
            inject_failure();
            assert!(
                kura.persist_lane_block_execution_input(&recovered).is_err(),
                "injected {label} execution-input barrier failure must be reported"
            );
        }
        kura.persist_lane_block_execution_input(&recovered)
            .expect("execution-input retry must reissue every strict barrier");
        let input = kura
            .read_lane_block_execution_input(lane_id, lane_block_height)
            .expect("strict execution input");

        let state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"strict lane execution preflight state",
        )));
        let results = vec![TransactionResult::new(TransactionResultInner::Ok(
            DataTriggerSequence::new(),
        ))];
        for (label, inject_failure) in strict_indexed_sidecar_failure_modes() {
            inject_failure();
            assert!(
                kura.persist_lane_block_execution_preflight(
                    &input,
                    7,
                    state_hash.clone(),
                    results.clone(),
                )
                .is_err(),
                "injected {label} execution-preflight barrier failure must be reported"
            );
        }
        kura.persist_lane_block_execution_preflight(&input, 7, state_hash, results.clone())
            .expect("execution-preflight retry must reissue every strict barrier");
        let preflight = kura
            .read_lane_block_execution_preflight(lane_id, lane_block_height)
            .expect("strict execution preflight");
        assert_eq!(preflight.results, results);
    }

    #[test]
    fn lane_block_execution_input_persists_recovered_payload_and_reloads() {
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
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let proposal = lane_block_proposal_from_ownership(&ownership);

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store block with lane artifact");
        let recovered = kura
            .recover_lane_block_payload(&proposal)
            .expect("recover executable lane payload");
        kura.persist_lane_block_execution_input(&recovered)
            .expect("persist lane execution input");
        kura.persist_lane_block_execution_input(&recovered)
            .expect("duplicate lane execution input persistence is idempotent");

        let input = kura
            .read_lane_block_execution_input(lane_id, lane_block_height)
            .expect("lane execution input");
        assert_eq!(input.format_label(), "lane.execution_input");
        assert_eq!(input.proposal, proposal);
        assert_eq!(input.artifact, recovered.artifact);
        assert_eq!(
            input.entrypoint_hashes,
            proposal.descriptor.accepted_transaction_hashes
        );
        assert_eq!(input.entrypoints, recovered.entrypoints);
        assert!(kura.lane_block_execution_input_available(&proposal));

        let (data_path, index_path) =
            Kura::lane_block_execution_input_paths_for_entry(lane_entry, temp_dir.path());
        assert!(
            data_path.is_file(),
            "lane execution input data file missing"
        );
        assert!(
            index_path.is_file(),
            "lane execution input index file missing"
        );

        drop(kura);
        let (reloaded, _) = Kura::new(&config, &lane_config).expect("reopen kura");
        assert_eq!(
            reloaded.read_lane_block_execution_input(lane_id, lane_block_height),
            Some(input)
        );
        assert!(reloaded.lane_block_execution_input_available(&proposal));
    }

    #[test]
    fn lane_execution_sidecars_validate_without_recursive_prune_repair() {
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
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let proposal = lane_block_proposal_from_ownership(&ownership);

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(block)
            .expect("store block with lane artifact");
        let recovered = kura
            .recover_lane_block_payload(&proposal)
            .expect("recover executable lane payload");
        let (artifact_data_path, artifact_index_path) =
            Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
        std::fs::remove_file(&artifact_data_path).expect("remove lane artifact data sidecar");
        std::fs::remove_file(&artifact_index_path).expect("remove lane artifact index sidecar");
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "test setup must remove the repairable lane artifact sidecar",
        );

        let worker_kura = Arc::clone(&kura);
        let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let outcome = (|| -> std::result::Result<(), String> {
                worker_kura
                    .persist_lane_block_execution_input(&recovered)
                    .map_err(|error| format!("persist execution input: {error:?}"))?;
                let input = worker_kura
                    .read_lane_block_execution_input_with_repair_policy(
                        lane_id,
                        lane_block_height,
                        false,
                    )
                    .ok_or_else(|| "read execution input after persistence".to_owned())?;
                let state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"missing lane artifact direct application state",
                )));
                let result =
                    TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
                worker_kura
                    .persist_lane_block_execution_preflight(&input, 7, state_hash, vec![result])
                    .map_err(|error| format!("persist execution preflight: {error:?}"))?;
                let preflight = worker_kura
                    .read_lane_block_execution_preflight_with_repair_policy(
                        lane_id,
                        lane_block_height,
                        false,
                    )
                    .ok_or_else(|| "read execution preflight after persistence".to_owned())?;
                worker_kura
                    .persist_direct_lane_block_application_receipt(&input, &preflight)
                    .map_err(|error| format!("persist direct receipt: {error:?}"))?;
                Ok(())
            })();
            done_tx.send(outcome).expect("report sidecar outcome");
        });
        done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("lane sidecar validation must not recursively lock prune_lock")
            .unwrap_or_else(|error| panic!("lane sidecar validation failed: {error}"));
        worker.join().expect("lane sidecar validation worker");

        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "validation under prune_lock must not repair the missing lane artifact sidecar",
        );
        let receipt = kura
            .read_active_lane_block_application_receipt_structural(
                lane_id,
                lane_block_height,
                false,
            )
            .expect("read direct receipt without sidecar repair");
        assert!(
            kura.lane_block_application_receipt_matches_available_evidence(&receipt, false),
            "execution input, preflight, and direct receipt must remain usable without repair",
        );
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_none(),
            "nonrepair evidence validation must leave the missing lane artifact absent",
        );
        assert_eq!(
            kura.read_lane_block_application_receipt(lane_id, lane_block_height),
            Some(receipt),
            "the public repair-enabled receipt reader must retain valid evidence",
        );
        assert!(
            kura.read_lane_block_artifact(lane_id, lane_block_height)
                .is_some(),
            "the public repair-enabled reader must recover the missing lane artifact",
        );
    }
