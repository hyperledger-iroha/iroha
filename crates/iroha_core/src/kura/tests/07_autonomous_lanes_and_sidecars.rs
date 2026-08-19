/// Exact autonomous attempt shared with Sumeragi handoff regressions.
pub(crate) struct AutonomousLaneAttemptFixture {
    /// Kura containing the immutable attempt and its terminal view state.
    pub(crate) kura: Arc<Kura>,
    /// Producer-authenticated payload stored in the exact attempt namespace.
    pub(crate) payload: LaneExecutablePayloadV1,
    /// Locally signed vote retained by the durable NewView certificate.
    pub(crate) new_view_vote: crate::lane_consensus::LaneBlockNewViewVoteV1,
    /// Exact NewView certificate retained in the durable attempt.
    pub(crate) new_view_certificate: crate::lane_consensus::LaneBlockNewViewCertificateV1,
    /// Keeps the isolated Kura root alive for the fixture lifetime.
    _root: TempDir,
}

/// Persist and retire one proposal-height-one autonomous attempt.
fn autonomous_lane_attempt_fixture(signer: &KeyPair, retire: bool) -> AutonomousLaneAttemptFixture {
    let root = TempDir::new().expect("create retired autonomous attempt root");
    let config = kura_config_for_dir(&root, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("configured fixture lane");
    let (_, _, seed_payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, signer);
    let payload = repropose_autonomous_lane_payload_for_kura(&seed_payload, 1, signer);
    let network_id = payload.network_id;
    let epoch = payload.epoch;
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize retired-attempt Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist retired-attempt payload");
    let durable_new_view = next_durable_lane_view_certificate_for_kura(
        &payload.origin_proposal,
        &payload,
        signer,
        network_id,
        epoch,
    );
    let new_view_certificate = durable_new_view.certificate.clone();
    let new_view_vote = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
        new_view_certificate.body.clone(),
        PeerId::new(signer.public_key().clone()),
        signer.private_key(),
    )
    .expect("sign exact retired-attempt NewView vote");
    kura.persist_lane_new_view_certificate(
        lane_id,
        payload.origin_proposal.descriptor.lane_block_height,
        durable_new_view,
        network_id,
        epoch,
    )
    .expect("persist retired-attempt NewView certificate");
    if retire {
        let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
        kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
            .expect("persist exact autonomous attempt retirement");
    }
    AutonomousLaneAttemptFixture {
        kura,
        payload,
        new_view_vote,
        new_view_certificate,
        _root: root,
    }
}

/// Persist and retire one exact autonomous attempt for a handoff test.
pub(crate) fn retired_autonomous_lane_attempt_fixture(
    signer: &KeyPair,
) -> AutonomousLaneAttemptFixture {
    autonomous_lane_attempt_fixture(signer, true)
}

/// Persist one exact autonomous attempt without a terminal retirement.
pub(crate) fn unretired_autonomous_lane_attempt_fixture(
    signer: &KeyPair,
) -> AutonomousLaneAttemptFixture {
    autonomous_lane_attempt_fixture(signer, false)
}

#[test]
fn exact_retired_autonomous_attempt_accessor_uses_proposal_height_namespace() {
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let fixture = retired_autonomous_lane_attempt_fixture(&signer);
    let descriptor = &fixture.payload.origin_proposal.descriptor;
    let exact = fixture
        .kura
        .read_autonomous_lane_retired_attempt(
            descriptor.lane_id,
            descriptor.lane_block_height,
            descriptor.proposal_height,
            fixture.payload.network_id,
            fixture.payload.epoch,
        )
        .expect("revalidate exact retired attempt")
        .expect("exact retired attempt exists");
    assert_eq!(exact.artifact.executable_payload, fixture.payload);
    assert_eq!(exact.current_proposal.descriptor.lane_block_view, 1);
    assert_eq!(
        exact.retirement,
        AutonomousLaneSlotRetirementV1::from_payload(&exact.artifact.executable_payload)
    );
    assert!(
        fixture
            .kura
            .read_autonomous_lane_retired_attempt(
                descriptor.lane_id,
                descriptor.lane_block_height,
                descriptor.proposal_height.saturating_add(1),
                fixture.payload.network_id,
                fixture.payload.epoch,
            )
            .expect("wrong exact attempt coordinate is a clean miss")
            .is_none()
    );
}

#[test]
fn autonomous_entrypoint_claim_release_repairs_crash_and_allows_reproposal() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_entry = lane_config.entry(LaneId::new(1)).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_entry.lane_id, lane_entry.dataspace_id, 1, &signer);
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
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist first autonomous payload");
    let claim_path = Kura::autonomous_lane_entrypoint_claim_path(
        temp_dir.path(),
        &network_id,
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
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
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
        kura.persist_lane_executable_payload(&successor, network_id, epoch)
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
        .persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
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
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(barrier.ordered_keys.iter())
            .expect("bind exact retirement reservation group");
    let prepared_evidence = reopened
        .authenticate_autonomous_lane_retirement_snapshot_evidence(
            &payload,
            &retirement,
            reservation_group,
            AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared,
        )
        .expect("authenticate exact pending retirement snapshot after restart");
    assert_eq!(
        prepared_evidence.phase(),
        AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared
    );
    assert_eq!(prepared_evidence.reservation_group(), reservation_group);
    assert_eq!(
        prepared_evidence.retirement_hash(),
        retirement.digest().expect("retirement digest")
    );
    assert_eq!(
        prepared_evidence.recovered_state().release.pending_prefix,
        1
    );
    assert_eq!(
        prepared_evidence.recovered_state().release.released_prefix,
        0
    );
    let mut conflicting_barrier = barrier.clone();
    conflicting_barrier.executable_payload_hash = Hash::new(b"conflicting-queue-release-payload");
    assert!(
        reopened
            .finalize_autonomous_lane_slot_release(
                &retirement,
                &conflicting_barrier,
                network_id,
                epoch,
            )
            .is_err(),
        "a barrier with different payload identity must not release claims",
    );
    reopened
        .finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch)
        .expect("exact durable Queue barrier releases claims");
    reopened
        .finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch)
        .expect("released claim retry is idempotent");
    let completed_evidence = reopened
        .authenticate_autonomous_lane_retirement_snapshot_evidence(
            &payload,
            &retirement,
            reservation_group,
            AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed,
        )
        .expect("authenticate exact fully released retirement snapshot");
    assert_eq!(
        completed_evidence.phase(),
        AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed
    );
    assert_eq!(completed_evidence.reservation_group(), reservation_group);
    assert_eq!(
        completed_evidence.recovered_state().release.pending_prefix,
        1
    );
    assert_eq!(
        completed_evidence.recovered_state().release.released_prefix,
        1
    );
    let released =
        Kura::decode_autonomous_lane_entrypoint_claim(&claim_path).expect("released claim");
    assert_eq!(
        released.state,
        AutonomousLaneEntrypointClaimStateV3::Released(
            retirement.digest().expect("retirement digest")
        ),
    );
    reopened
        .persist_lane_executable_payload(&successor, network_id, epoch)
        .expect("released entrypoint can be reproposed at the next exact slot");
    let successor_claim =
        Kura::decode_autonomous_lane_entrypoint_claim(&claim_path).expect("successor claim");
    assert!(successor_claim.active_for_payload(&successor));
    assert!(
        reopened
            .persist_lane_executable_payload(&payload, network_id, epoch)
            .is_err(),
        "the delayed retired payload must not reclaim its old slot",
    );
    drop(reopened);
    let (restarted, _) = Kura::new(&config, &lane_config).expect("restart after reproposal");
    restarted
        .persist_lane_executable_payload(&successor, network_id, epoch)
        .expect("successor ownership remains idempotent after restart");
}
#[test]
fn autonomous_claim_runtime_inventory_enforces_boundary_without_partial_staging() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
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
        &payload.network_id,
        &payload.entrypoint_hashes[0],
    );
    let target_temp = Kura::autonomous_lane_entrypoint_claim_temp_path(&target_path);
    let _guard = kura.sidecar_lock.lock();
    let staged = kura
        .prepare_autonomous_lane_entrypoint_claims_with_limit_locked(0, &payload, 2)
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
        kura.prepare_autonomous_lane_entrypoint_claims_with_limit_locked(0, &payload, 2)
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
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
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
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
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
        &payload.network_id,
        &payload.entrypoint_hashes[0],
    );
    let target_temp = Kura::autonomous_lane_entrypoint_claim_temp_path(&target_path);
    {
        let _guard = kura.sidecar_lock.lock();
        assert!(
            kura.prepare_autonomous_lane_entrypoint_claims_with_limit_locked(0, &payload, 8)
                .is_err(),
            "live preparation must fail closed on an unexpected namespace artifact",
        );
        assert!(
            kura.reconcile_autonomous_lane_entrypoint_claim_temps_on_startup_with_limit_locked(8,)
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
        network_id: iroha_data_model::NetworkId,
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
        network_id: iroha_data_model::NetworkId,
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
    let network_id = test_network_id(b"legacy-claim-genesis");
    let entrypoint_hash = Hash::new(b"legacy-claim-entrypoint");
    let path =
        Kura::autonomous_lane_entrypoint_claim_path(temp_dir.path(), &network_id, &entrypoint_hash);
    fs::create_dir_all(path.parent().expect("claim parent")).expect("create claim parent");
    let common = (
        network_id,
        entrypoint_hash,
        LaneId::new(1),
        DataSpaceId::new(3),
        Hash::new(b"legacy-claim-incarnation"),
        Hash::new(b"legacy-claim-proposal"),
        Hash::new(b"legacy-claim-payload"),
    );
    let legacy = LegacyClaimV2 {
        version: 2,
        network_id: common.0,
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
        network_id: common.0,
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
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_entry = lane_config.entry(LaneId::new(1)).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_entry.lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
    let mut conflicting = retirement.clone();
    conflicting.origin_proposal_hash = Hash::new(b"conflicting-retirement-proposal");
    assert!(
        kura.persist_autonomous_lane_slot_retirement(&conflicting, network_id, epoch,)
            .is_err(),
        "a caller cannot retire a different proposal identity",
    );
    assert!(
        kura.read_autonomous_lane_slot_retirement(lane_entry.lane_id, 1, network_id, epoch,)
            .expect("conflicting attempt leaves a readable slot")
            .is_none(),
    );
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
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
        kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch,)
            .is_err(),
        "a delayed exact tombstone cannot target a recreated lane incarnation",
    );
    assert!(
        kura.persist_autonomous_lane_slot_retirement(
            &AutonomousLaneSlotRetirementV1::from_payload(&recreated),
            network_id,
            epoch,
        )
        .is_err(),
        "a fresh-incarnation tombstone requires its own durable payload first",
    );
    assert!(
        kura.read_autonomous_lane_slot_retirement(lane_entry.lane_id, 1, network_id, epoch,)
            .is_err(),
        "the old tombstone must never validate under the recreated active marker",
    );
}
#[test]
fn autonomous_lane_slot_retirement_repairs_temp_and_rejects_bad_files() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
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
        kura.read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
            .expect("promote valid retirement temp"),
        Some(retirement.clone()),
    );
    assert!(!temp_path.exists(), "recovered retirement temp is removed");
    fs::write(&view_path, &valid_bytes[..valid_bytes.len() / 2])
        .expect("truncate retirement without recovery temp");
    assert!(
        kura.read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
            .is_err(),
        "truncated retirement must fail closed",
    );
    fs::write(&view_path, &valid_bytes).expect("restore retirement after truncation");
    fs::write(&view_path, [0xFF, 0x00, 0xAA]).expect("corrupt retirement");
    assert!(
        kura.read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
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
        kura.read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
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
            kura.read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
                .is_err(),
            "symlinked retirement must fail closed",
        );
        fs::remove_file(&view_path).expect("remove retirement symlink");
        fs::rename(&real_path, &view_path).expect("restore regular retirement");
        symlink(&view_path, &temp_path).expect("symlink crash temp");
        assert!(
            kura.read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
                .is_err(),
            "symlinked retirement temp must fail closed even beside a valid main",
        );
        fs::remove_file(&temp_path).expect("remove crash-temp symlink");
    }
    assert_eq!(
        kura.read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
            .expect("regular retirement survives adversarial files"),
        Some(retirement),
    );
}
#[test]
fn autonomous_lane_slot_retirement_rejects_already_certified_slot() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let (session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, &signer);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified autonomous slot");
    assert!(
        kura.persist_autonomous_lane_slot_retirement(
            &AutonomousLaneSlotRetirementV1::from_payload(&payload),
            network_id,
            epoch,
        )
        .is_err(),
        "a certified autonomous slot cannot release its queue ownership",
    );
    assert!(
        kura.read_autonomous_lane_slot_retirement(lane_id, 1, network_id, epoch)
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
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let origin = payload.origin_proposal.clone();
    let availability = durable_lane_payload_availability_for_kura(&payload, &origin, &signer);
    let new_view =
        next_durable_lane_view_certificate_for_kura(&origin, &payload, &signer, network_id, epoch);
    let cursor = crate::lane_consensus::retarget_lane_block_proposal_view(
        &origin,
        new_view.certificate.body.target_view,
    )
    .expect("synthetic NewView cursor");
    let signer_pops = BTreeMap::from([(
        signer.public_key().clone(),
        bls_normal_pop_prove(signer.private_key()).expect("certification signer PoP"),
    )]);
    let origin_commit_vote = signed_lane_block_vote_for_kura(&origin, CertPhase::Commit, &signer);
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
        version: AutonomousLaneMergeBundleV1::VERSION,
        autonomous: autonomous.clone(),
        certified: certified_origin,
    };
    Kura::validate_autonomous_lane_merge_bundle(&bundle, network_id, epoch)
        .expect("origin certification remains valid after the cursor advances");
    let cursor_availability =
        durable_lane_payload_availability_for_kura(&payload, &cursor, &signer);
    let cursor_commit_vote = signed_lane_block_vote_for_kura(&cursor, CertPhase::Commit, &signer);
    let cursor_commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        cursor_commit_vote.body.clone(),
        cursor.descriptor.validator_set.clone(),
        std::slice::from_ref(&cursor_commit_vote),
    )
    .expect("synthetic cursor commit QC");
    let cursor_bundle = AutonomousLaneMergeBundleV1 {
        version: AutonomousLaneMergeBundleV1::VERSION,
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
        Kura::validate_autonomous_lane_merge_bundle(&cursor_bundle, network_id, epoch,),
        Err("autonomous lane merge bundle must certify the immutable origin proposal"),
        "a fully signed synthetic cursor must not become the merge certification subject",
    );
    let mut poisoned_availability = bundle;
    poisoned_availability.autonomous.availability_certificate = Some(cursor_availability);
    assert_eq!(
        Kura::validate_autonomous_lane_merge_bundle(&poisoned_availability, network_id, epoch,),
        Err("invalid autonomous lane payload availability certificate"),
        "the durable artifact must reject a next-view READY QC before merge validation",
    );
}
#[test]
fn autonomous_view_state_latest_read_only_selects_crash_temp_without_mutation() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let new_view = next_durable_lane_view_certificate_for_kura(
        &payload.origin_proposal,
        &payload,
        &signer,
        network_id,
        epoch,
    );
    let mut advanced = kura
        .read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch)
        .expect("read origin view state");
    advanced.new_view_certificates.push(new_view);
    let advanced_state = AutonomousLaneBlockViewState::from_artifact(&advanced);
    let view_state_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
        lane_entry,
        &kura.store_root,
        1,
        payload.origin_proposal.descriptor.proposal_height,
    );
    let view_state_temp = Kura::autonomous_lane_block_view_state_temp_path(&view_state_path);
    let temp_bytes =
        norito::encode_canonical(&advanced_state).expect("encode crash-temp view state");
    fs::write(&view_state_temp, &temp_bytes).expect("stage higher-view crash temp");
    let main_before = fs::read(&view_state_path).expect("read stable main view state");
    let record = {
        let _prune_guard = kura.prune_lock.lock();
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.read_autonomous_lane_block_record_read_only_latest_locked(
            lane_entry, lane_id, 1, network_id, epoch,
        )
        .expect("read logical view-state winner")
        .expect("read retained autonomous attempt")
    };
    let current =
        Kura::validate_autonomous_lane_block_artifact(&record.artifact, network_id, epoch)
            .expect("validate read-only logical winner");
    assert_eq!(current.descriptor.lane_block_view, 1);
    assert_eq!(
        fs::read(&view_state_path).expect("reread stable main view state"),
        main_before,
        "read-only winner selection must not promote the crash temp",
    );
    assert_eq!(
        fs::read(&view_state_temp).expect("reread higher-view crash temp"),
        temp_bytes,
        "read-only winner selection must not delete or rewrite the crash temp",
    );
}
#[test]
fn durable_autonomous_merge_source_requires_every_exact_component_and_survives_restart() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let availability =
        durable_lane_payload_availability_for_kura(&payload, &payload.origin_proposal, &signer);
    let (mut session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, &signer);
    session.prepare_qc = availability.certificate.clone();
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "a certificate must not publish before the exact execution input and READY evidence"
    );
    assert!(
        kura.read_certified_lane_block_artifact(lane_id, 1)
            .is_none(),
        "failed autonomous prepublication must not leave a certified pair"
    );
    let recovered = kura
        .recover_autonomous_lane_block_payload(&payload.origin_proposal, network_id, epoch)
        .expect("recover exact autonomous execution input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist exact autonomous execution input");
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "the execution input alone must not substitute for durable READY evidence"
    );
    kura.persist_lane_payload_availability_certificate(lane_id, 1, availability, network_id, epoch)
        .expect("persist exact READY certificate");
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch),
        Err("certified lane block pair lacks the exact autonomous slot"),
        "READY plus an execution input cannot substitute for the exact certified pair",
    );
    fail_next_autonomous_merge_bundle_persistence_for_tests();
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "an injected crash boundary must stop after certificate durability and before bundle eligibility",
    );
    assert!(
        kura.read_certified_lane_block_artifact(lane_id, 1)
            .is_some(),
        "the independently durable certificate must survive the bundle crash boundary",
    );
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch),
        Err("durable autonomous merge bundle is unavailable"),
        "a certificate must not become merge eligible before the bundle's own barrier",
    );
    drop(kura);
    let (kura, _) = Kura::new(&config, &lane_config).expect("repair bundle on startup");
    let source = kura
        .durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch)
        .expect("read complete durable autonomous source");
    let exact_input_authorization = kura
        .authorize_autonomous_execution_input_persistence(
            source.bundle.executable_payload(),
            &source.input,
        )
        .expect("authorize the exact autonomous execution input");
    assert!(exact_input_authorization.matches_input(&source.input));
    let exact_input_projection = exact_input_authorization
        .consume_for_persistence(&source.input)
        .expect("consume the exact autonomous execution-input authorization");
    assert_eq!(
        exact_input_projection.action,
        IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT,
    );
    assert!(
        check_production_in_flight_first_release_transition(exact_input_projection)
            .is_some_and(|checked| checked.into_projection() == exact_input_projection),
        "the exact durable payload must produce a valid execution-input projection",
    );
    let substituted_input_authorization = kura
        .authorize_autonomous_execution_input_persistence(
            source.bundle.executable_payload(),
            &source.input,
        )
        .expect("authorize the execution input before substitution");
    let mut substituted_input = source.input.clone();
    substituted_input.entrypoint_hashes[0] = Hash::new(b"substituted execution input");
    assert!(
        substituted_input_authorization
            .consume_for_persistence(&substituted_input)
            .is_none(),
        "a substituted execution input must not consume exact persistence authority",
    );
    let exact_authorization =
        Kura::authorize_autonomous_lane_commit_persistence(&source, &source.bundle.certified)
            .expect("authorize the exact autonomous lane Commit");
    assert!(exact_authorization.matches_artifact(&source.bundle.certified));
    let exact_projection = exact_authorization
        .consume_for_persistence(&source.bundle.certified)
        .expect("consume the exact autonomous lane-Commit authorization");
    assert_eq!(
        exact_projection.action,
        IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT,
    );
    assert!(
        check_production_in_flight_first_release_transition(exact_projection)
            .is_some_and(|checked| checked.into_projection() == exact_projection),
        "the exact durable source must produce a valid LaneCommit projection",
    );
    let substituted_authorization =
        Kura::authorize_autonomous_lane_commit_persistence(&source, &source.bundle.certified)
            .expect("authorize before substitution");
    let mut substituted_artifact = source.bundle.certified.clone();
    substituted_artifact.signer_pops.clear();
    assert!(
        substituted_authorization
            .consume_for_persistence(&substituted_artifact)
            .is_none(),
        "a substituted certified artifact must not consume exact lane-Commit authority",
    );
    assert_eq!(source.bundle.certified.proposal, payload.origin_proposal);
    assert_eq!(
        source.input,
        LaneBlockExecutionInputArtifact::new(recovered)
    );
    assert_eq!(
        source.source_bundle,
        source.bundle.encode_framed().expect("canonical bundle")
    );
    assert_eq!(
        source.bundle_hash,
        source.bundle.bundle_hash().expect("canonical bundle hash")
    );
    let delayed_new_view = next_durable_lane_view_certificate_for_kura(
        &payload.origin_proposal,
        &payload,
        &signer,
        network_id,
        epoch,
    );
    assert!(
        kura.persist_lane_new_view_certificate(lane_id, 1, delayed_new_view, network_id, epoch,)
            .is_err(),
        "a durable certificate must freeze the exact reconstructed bundle bytes"
    );
    let view_state_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
        lane_entry,
        &kura.store_root,
        1,
        payload.origin_proposal.descriptor.proposal_height,
    );
    let view_state_temp = Kura::autonomous_lane_block_view_state_temp_path(&view_state_path);
    fs::copy(&view_state_path, &view_state_temp).expect("stage exact view-state crash temp");
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch),
        Err("autonomous lane view state has unresolved recovery state"),
        "merge admission must not choose a view while startup recovery can still replace it",
    );
    drop(kura);
    let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
    assert_eq!(
        reopened
            .durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch)
            .expect("restart must recover the same exact source"),
        source
    );
}
#[test]
fn durable_autonomous_merge_source_rejects_execution_input_drift() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let recovered = kura
        .recover_autonomous_lane_block_payload(&payload.origin_proposal, network_id, epoch)
        .expect("recover autonomous execution input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist autonomous execution input");
    let availability =
        durable_lane_payload_availability_for_kura(&payload, &payload.origin_proposal, &signer);
    kura.persist_lane_payload_availability_certificate(
        lane_id,
        1,
        availability.clone(),
        network_id,
        epoch,
    )
    .expect("persist READY evidence");
    let (mut session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, &signer);
    session.prepare_qc = availability.certificate;
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified autonomous source");
    kura.durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch)
        .expect("complete source is initially eligible");
    let mut drifted = LaneBlockExecutionInputArtifact::new(recovered);
    drifted.source = LaneBlockExecutionSourceV1::autonomous_lane(
        network_id,
        epoch,
        Hash::new(b"drifted autonomous input hash"),
    );
    let drifted_bytes = drifted.encode_framed().expect("encode drifted input");
    let (data_path, index_path) =
        Kura::lane_block_execution_input_paths_for_entry(lane_entry, temp_dir.path());
    assert!(Kura::append_indexed_sidecar(
        &data_path,
        &index_path,
        1,
        &drifted_bytes,
        "lane block execution input",
        FsyncMode::Always,
        None,
    ));
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch),
        Err("durable execution input differs from the certified autonomous payload"),
        "a self-consistent but payload-divergent input must lose merge eligibility"
    );
}
#[test]
fn durable_autonomous_merge_source_rejects_persisted_bundle_drift() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let recovered = kura
        .recover_autonomous_lane_block_payload(&payload.origin_proposal, network_id, epoch)
        .expect("recover autonomous execution input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist autonomous execution input");
    let availability =
        durable_lane_payload_availability_for_kura(&payload, &payload.origin_proposal, &signer);
    kura.persist_lane_payload_availability_certificate(
        lane_id,
        1,
        availability.clone(),
        network_id,
        epoch,
    )
    .expect("persist READY evidence");
    let (mut session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, &signer);
    session.prepare_qc = availability.certificate;
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified autonomous source");
    let source = kura
        .durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch)
        .expect("complete source is initially eligible");
    let mut drifted = source.bundle;
    drifted
        .autonomous
        .new_view_certificates
        .push(next_durable_lane_view_certificate_for_kura(
            &payload.origin_proposal,
            &payload,
            &signer,
            network_id,
            epoch,
        ));
    Kura::validate_autonomous_lane_merge_bundle(&drifted, network_id, epoch)
        .expect("drift fixture remains internally valid");
    let drifted_bytes = drifted.encode_framed().expect("encode drifted bundle");
    let (data_path, index_path) =
        Kura::autonomous_lane_merge_bundle_paths_for_entry(lane_entry, temp_dir.path());
    fs::write(&data_path, &drifted_bytes).expect("write divergent canonical bundle data");
    let mut index = SidecarIndexLayout::base_header(1).to_vec();
    index.extend_from_slice(
        &SidecarIndexEntry {
            offset: 0,
            len: u64::try_from(drifted_bytes.len()).expect("bundle length fits u64"),
        }
        .to_bytes(),
    );
    fs::write(&index_path, index).expect("write divergent canonical bundle index");
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch),
        Err("persisted autonomous merge bundle differs from exact durable components"),
        "an internally valid but separately divergent persisted bundle must lose eligibility",
    );
}
#[test]
fn autonomous_merge_bundle_pair_rejects_malformed_truncated_oversized_partial_and_linked_artifacts()
{
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let recovered = kura
        .recover_autonomous_lane_block_payload(&payload.origin_proposal, network_id, epoch)
        .expect("recover autonomous execution input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist autonomous execution input");
    let availability =
        durable_lane_payload_availability_for_kura(&payload, &payload.origin_proposal, &signer);
    kura.persist_lane_payload_availability_certificate(
        lane_id,
        1,
        availability.clone(),
        network_id,
        epoch,
    )
    .expect("persist READY evidence");
    let (mut session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, &signer);
    session.prepare_qc = availability.certificate;
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified autonomous source");
    kura.durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch)
        .expect("complete source is initially eligible");
    let (data_path, index_path) =
        Kura::autonomous_lane_merge_bundle_paths_for_entry(lane_entry, temp_dir.path());
    let backup_dir = TempDir::new().expect("bundle backup dir");
    let data_backup = backup_dir.path().join("merge_bundle_data.backup");
    let index_backup = backup_dir.path().join("merge_bundle_index.backup");
    fs::copy(&data_path, &data_backup).expect("backup bundle data");
    fs::copy(&index_path, &index_backup).expect("backup bundle index");
    let canonical_data = fs::read(&data_backup).expect("read canonical bundle data");
    let canonical_index = fs::read(&index_backup).expect("read canonical bundle index");
    let restore_pair = || {
        if fs::symlink_metadata(&data_path).is_ok() {
            fs::remove_file(&data_path).expect("remove mutated bundle data");
        }
        if fs::symlink_metadata(&index_path).is_ok() {
            fs::remove_file(&index_path).expect("remove mutated bundle index");
        }
        fs::copy(&data_backup, &data_path).expect("restore bundle data");
        fs::copy(&index_backup, &index_path).expect("restore bundle index");
    };
    let assert_rejected = |case: &str| {
        assert!(
            kura.durable_autonomous_lane_merge_source(lane_id, 1, network_id, epoch)
                .is_err(),
            "{case} must fail closed before merge eligibility",
        );
    };
    let mut malformed_data = canonical_data.clone();
    let malformed_midpoint = malformed_data.len() / 2;
    malformed_data[malformed_midpoint] ^= 0x80;
    fs::write(&data_path, malformed_data).expect("write malformed bundle data");
    assert_rejected("malformed canonical bundle bytes");
    restore_pair();
    fs::write(&index_path, &canonical_index[..canonical_index.len() - 1])
        .expect("write truncated bundle index");
    assert_rejected("truncated bundle index");
    restore_pair();
    let mut trailing_index = canonical_index.clone();
    trailing_index.push(0);
    fs::write(&index_path, trailing_index).expect("write trailing bundle index byte");
    assert_rejected("partial trailing bundle index entry");
    restore_pair();
    let oversized_index_len = u64::try_from(
        kura.autonomous_lane_merge_bundle_pair_entry_limit()
            .saturating_add(1),
    )
    .expect("entry limit fits u64")
    .saturating_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
    .saturating_add(INDEXED_SIDECAR_BASE_HEADER_SIZE_U64);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&index_path)
        .expect("open bundle index for oversizing")
        .set_len(oversized_index_len)
        .expect("oversize bundle index sparsely");
    assert_rejected("oversized bundle index");
    restore_pair();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&data_path)
        .expect("open bundle data for oversizing")
        .set_len(
            u64::try_from(kura.autonomous_lane_merge_bundle_pair_byte_limit())
                .expect("aggregate budget fits u64")
                .saturating_add(1),
        )
        .expect("oversize bundle data sparsely");
    assert_rejected("oversized bundle data");
    restore_pair();
    fs::remove_file(&index_path).expect("remove one bundle pair half");
    assert_rejected("partial bundle data/index pair");
    restore_pair();
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        fs::remove_file(&data_path).expect("remove bundle data before symlink");
        symlink(&data_backup, &data_path).expect("symlink bundle data");
        assert_rejected("symlinked bundle data");
        restore_pair();
        fs::remove_file(&data_path).expect("remove bundle data before hardlink");
        fs::hard_link(&data_backup, &data_path).expect("hardlink bundle data");
        assert_rejected("hardlinked bundle data");
        drop(kura);
        assert!(
            Kura::new(&config, &lane_config).is_err(),
            "startup must reject a hardlinked canonical bundle pair",
        );
        fs::remove_file(&data_path).expect("remove hardlink fixture");
    }
}
#[test]
fn autonomous_execution_input_validation_does_not_repair_view_sidecars() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload");
    let recovered = kura
        .recover_autonomous_lane_block_payload(&payload.origin_proposal, network_id, epoch)
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
        kura.recover_autonomous_lane_block_payload(&payload.origin_proposal, network_id, epoch,)
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
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) =
        autonomous_lane_payload_for_kura(lane_id, lane_entry.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist payload");
    let mut current = payload.origin_proposal.clone();
    let mut certificate_prefix = Vec::with_capacity(256);
    for _ in 1..=256 {
        let durable = next_durable_lane_view_certificate_for_kura(
            &current, &payload, &signer, network_id, epoch,
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
        let _prune_guard = kura.prune_lock.lock();
        kura.ensure_prune_recovery_not_required()
            .expect("view-state fixture has no prune recovery");
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("measure pending canonical bytes before view-state write");
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.write_autonomous_lane_block_view_state_locked(
            pending_canonical_bytes,
            &AutonomousLaneBlockArtifact {
                format: AutonomousLaneBlockArtifactFormat::Current,
                executable_payload: payload.clone(),
                availability_certificate: None,
                view_checkpoint: None,
                new_view_certificates: certificate_prefix,
            },
            &view_path,
            network_id,
            epoch,
        )
        .expect("persist bounded certificate prefix");
    }
    let artifact = kura
        .read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch)
        .expect("view 256 artifact");
    assert!(artifact.view_checkpoint.is_none());
    assert_eq!(artifact.new_view_certificates.len(), 256);
    for target_view in 257..=258 {
        let durable = next_durable_lane_view_certificate_for_kura(
            &current, &payload, &signer, network_id, epoch,
        );
        current = match kura
            .persist_lane_new_view_certificate(lane_id, 1, durable, network_id, epoch)
            .expect("persist NewView certificate")
        {
            LaneBlockNewViewPersistenceOutcome::Persisted(cursor) => cursor,
            LaneBlockNewViewPersistenceOutcome::AlreadyTerminal => {
                panic!("non-terminal checkpoint fixture unexpectedly reached a terminal receipt")
            }
        };
        if target_view == 257 {
            let artifact = kura
                .read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch)
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
        .current_autonomous_lane_payload(lane_id, 1, network_id, epoch)
        .expect("restart recovery");
    assert_eq!(recovered.1.descriptor.lane_block_view, 258);
    let snapshot = reopened
        .latest_autonomous_lane_block_artifacts_snapshot(network_id, 1, |_| epoch)
        .expect("load bounded route-latest snapshot");
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].1.descriptor.lane_block_view, 258);
    assert!(
        reopened
            .latest_autonomous_lane_block_artifacts_snapshot(network_id, 0, |_| epoch)
            .expect("zero-cap snapshot is empty")
            .is_empty(),
        "a zero global recovery limit must not enumerate durable history"
    );
    let temp_path = Kura::autonomous_lane_block_view_state_temp_path(&view_path);
    let valid_bytes = fs::read(&view_path).expect("read valid view state");
    fs::write(&temp_path, &valid_bytes).expect("stage crash temp");
    fs::write(&view_path, &valid_bytes[..valid_bytes.len() / 2]).expect("truncate main view state");
    assert_eq!(
        reopened
            .current_autonomous_lane_payload(lane_id, 1, network_id, epoch)
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
            .read_autonomous_lane_block_artifact(lane_id, 1, network_id, epoch)
            .is_none(),
        "corrupt view state must not fall back to the origin proposal"
    );
}
#[test]
fn autonomous_payload_promotes_hint_free_bytes_to_one_exact_carrier_hint() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, mut hint_free) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let hint = hint_free
        .origin_proposal
        .payload_block_hint
        .take()
        .expect("fixture carrier hint");
    hint_free
        .validate(network_id, epoch)
        .expect("hint-free payload remains authenticated");
    let hinted = hint_free
        .attach_global_hint_exact(hint, network_id, epoch)
        .expect("attach exact carrier hint");
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &hint_free);
    kura.persist_lane_executable_payload(&hint_free, network_id, epoch)
        .expect("persist hint-free local payload");
    kura.persist_lane_executable_payload(&hinted, network_id, epoch)
        .expect("promote to exact carrier-hinted payload");
    assert_eq!(
        kura.current_autonomous_lane_payload(lane.lane_id, 1, network_id, epoch)
            .expect("current promoted payload")
            .0,
        hinted,
    );
    assert!(
        kura.persist_lane_executable_payload(&hint_free, network_id, epoch)
            .is_err(),
        "carrier-hint promotion must never be reversed",
    );
    drop(kura);
    let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
    assert_eq!(
        reopened
            .current_autonomous_lane_payload(lane.lane_id, 1, network_id, epoch)
            .expect("restart recovers promoted payload")
            .0,
        hinted,
    );
}
#[test]
fn autonomous_payload_rejects_a_conflicting_carrier_hint_after_promotion() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, mut hint_free) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let first_hint = hint_free
        .origin_proposal
        .payload_block_hint
        .take()
        .expect("fixture carrier hint");
    let first = hint_free
        .attach_global_hint_exact(first_hint, network_id, epoch)
        .expect("attach first carrier hint");
    let conflicting_hint = iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
        proposal_height: first_hint.proposal_height,
        proposal_view: first_hint.proposal_view.saturating_add(1),
        proposal_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"conflicting-autonomous-carrier-hint",
        )),
    };
    let conflicting = hint_free
        .attach_global_hint_exact(conflicting_hint, network_id, epoch)
        .expect("build independently authenticated conflicting hint form");
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &hint_free);
    kura.persist_lane_executable_payload(&hint_free, network_id, epoch)
        .expect("persist hint-free local payload");
    kura.persist_lane_executable_payload(&first, network_id, epoch)
        .expect("promote first carrier hint");
    assert!(
        kura.persist_lane_executable_payload(&conflicting, network_id, epoch)
            .is_err(),
        "a different carrier identity must not replace the promoted hint",
    );
    assert_eq!(
        kura.current_autonomous_lane_payload(lane.lane_id, 1, network_id, epoch)
            .expect("first carrier remains current")
            .0,
        first,
    );
}
#[test]
fn autonomous_first_attempt_uses_only_versioned_files_and_repairs_missing_pointers() {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload_template) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"kura-autonomous-lifecycle-height-context"),
    ));
    let local_peer = PeerId::new(signer.public_key().clone());
    let (reservation_owner_hash, proposal_identity_hash) =
        autonomous_lane_reservation_identity_hashes_for_proposal(
            network_id,
            height_context_id,
            epoch,
            &payload_template.origin_proposal,
            &local_peer,
        )
        .expect("derive exact lifecycle reservation identities");
    let mut reservation_keys = payload_template.reservation_keys.clone();
    for reservation in &mut reservation_keys {
        reservation.reservation_owner_hash = reservation_owner_hash;
        reservation.proposal_identity_hash = proposal_identity_hash;
    }
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        payload_template.origin_proposal.clone(),
        payload_template.entrypoints.clone(),
        reservation_keys,
        payload_template.routing_plans.clone(),
        payload_template.native_amx_receipts.clone(),
        local_peer.clone(),
        signer.private_key(),
    )
    .expect("construct height-context-bound lifecycle payload");
    let proposal_height = payload.origin_proposal.descriptor.proposal_height;
    let (kura, _) = Kura::new(&config, &lane_config).expect("Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind local lifecycle key identity");
    let generation_one = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim first durable lifecycle process generation");
    assert_eq!(generation_one.generation(), 1);
    assert_eq!(generation_one.network_id(), network_id);
    assert_eq!(generation_one.local_peer_id(), &local_peer);
    assert_eq!(
        kura.claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
            .expect("repeat first process-generation claim"),
        generation_one,
        "one live Kura instance must not consume two generations",
    );
    assert!(
        kura.claim_autonomous_lifecycle_process_generation(
            test_network_id(b"wrong lifecycle process genesis"),
            &local_peer,
        )
        .is_err(),
        "one process cannot drift its durable chain identity",
    );
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist first versioned attempt");
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
            .expect("bind exact lifecycle reservation group");
    let binding = AutonomousLifecycleAttemptBindingV1::from_payload(
        height_context_id,
        1,
        &payload,
        reservation_group,
        &local_peer,
    )
    .expect("bind exact lifecycle attempt");
    let wrong_height_context = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"wrong-kura-autonomous-lifecycle-height-context"),
    ));
    assert!(
        AutonomousLifecycleAttemptBindingV1::from_payload(
            wrong_height_context,
            1,
            &payload,
            reservation_group,
            &local_peer,
        )
        .is_err(),
        "reservation identities must be rederived from the exact height context",
    );
    let mut wrong_predecessor = binding.clone();
    wrong_predecessor.previous_lane_block_descriptor_hash =
        Some(Hash::new(b"invented lifecycle predecessor"));
    assert!(
        kura.read_autonomous_lifecycle_cursor(&payload, &wrong_predecessor, &generation_one)
            .is_err(),
        "every cursor read must revalidate the exact predecessor",
    );
    let binding_a = canonical_lane_queue_reservation_group_identity_projection(reservation_group);
    let initial = ProductionInFlightFirstReleaseStateProjection {
        validator_count: 1,
        producer: 1,
        producer_selected_owner: 1,
        replicated_carrier_owners: 0,
        payload_binding_a: 1,
        binding_a,
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
            selected_count: reservation_group.reservation_count,
            reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection::default(),
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: 1,
            producer_alive: true,
            ..ProductionInFlightFirstReleaseSessionProjection::default()
        },
        history: ProductionInFlightFirstReleaseHistoryProjection {
            ever_queue_plan_v4: true,
            ever_reservation_v5: true,
            ..ProductionInFlightFirstReleaseHistoryProjection::default()
        },
        decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
        release: ProductionInFlightFirstReleaseReleaseProjection::default(),
    };
    assert!(production_in_flight_first_release_state_kernel(initial));
    let sign_cursor = |sequence, previous_cursor_hash, phase: AutonomousLifecycleCursorPhaseV2| {
        let unsigned = AutonomousLifecycleCursorUnsignedV2::new(
            sequence,
            previous_cursor_hash,
            binding.clone(),
            phase,
            local_peer.clone(),
        )
        .expect("construct lifecycle cursor body");
        let preimage = unsigned
            .signing_preimage()
            .expect("encode lifecycle signing preimage");
        let signature =
            Signature::try_new(signer.private_key(), &preimage).expect("sign lifecycle cursor");
        unsigned
            .finalize(
                <[u8; 96]>::try_from(signature.payload())
                    .expect("BLS-normal cursor signature is exactly 96 bytes"),
                &payload.origin_proposal.descriptor.validator_set,
            )
            .expect("finalize lifecycle cursor")
    };
    assert!(
        Kura::validate_autonomous_lifecycle_cursor_cas_budget(
            MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES,
            AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64,
            AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES as u64,
            AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES as u64,
            true,
        )
        .is_ok(),
        "replacement at the full stable boundary must consume only the one bounded temp",
    );
    assert!(
        Kura::validate_autonomous_lifecycle_cursor_cas_budget(
            MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES,
            AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64,
            0,
            1,
            false,
        )
        .is_err(),
        "create-at-cap must fail its final stable file budget",
    );
    assert!(
        Kura::validate_autonomous_lifecycle_cursor_cas_budget(
            MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES,
            AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64,
            AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES as u64 + 1,
            AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES as u64 + 1,
            true,
        )
        .is_err(),
        "replacement must reject more than one maximum cursor of temporary exposure",
    );
    let absent = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_one)
        .expect("read absent lifecycle cursor");
    assert!(absent.cursor().is_none());
    let (_, absent_lease) = absent.into_parts();
    let unsound_pre_persistence_live = sign_cursor(
        1,
        None,
        AutonomousLifecycleCursorPhaseV2::live(1, initial)
            .expect("construct deliberately pre-persistence live lifecycle phase"),
    );
    assert!(
        Kura::validate_autonomous_lifecycle_cursor_successor(
            &absent_lease,
            None,
            &unsound_pre_persistence_live,
        )
        .is_err(),
        "the ordinary cursor API must not fabricate a pre-ActivateKura state after payload durability",
    );
    let mut activated = initial;
    activated.carrier.kura_active = 1;
    let live_activated = sign_cursor(
        1,
        None,
        AutonomousLifecycleCursorPhaseV2::live(1, activated)
            .expect("construct already-durable Kura live lifecycle phase"),
    );
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(absent_lease, live_activated.clone(),)
            .expect("create lifecycle cursor from already-durable Kura state")
            .cursor(),
        Some(&live_activated),
        "successful lifecycle creation must return its exact durable cursor",
    );
    let foreign_temp_dir = TempDir::new().expect("foreign Kura root");
    let foreign_config = kura_config_for_dir(&foreign_temp_dir, BLOCKS_IN_MEMORY);
    let (foreign_kura, _) =
        Kura::new(&foreign_config, &lane_config).expect("foreign Kura instance");
    foreign_kura
        .bind_local_peer_id(local_peer.clone())
        .expect("bind same local key in foreign Kura root");
    let foreign_generation = foreign_kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim same generation in foreign Kura root");
    let cloned_foreign_generation = foreign_generation.clone();
    assert_eq!(cloned_foreign_generation, foreign_generation);
    install_autonomous_lane_marker_for_kura(&foreign_kura, &lane_config, &payload);
    assert!(
        kura.read_autonomous_lifecycle_cursor(&payload, &binding, &cloned_foreign_generation,)
            .is_err(),
        "a process-generation claim cloned from another Kura root must not authorize a cursor read",
    );
    let (_, cross_root_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_one)
        .expect("mint root-bound cursor lease")
        .into_parts();
    let first_read = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_one)
        .expect("read first lifecycle cursor");
    let second_read = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_one)
        .expect("mint competing lifecycle cursor lease");
    let (_, first_lease) = first_read.into_parts();
    let (_, stale_lease) = second_read.into_parts();
    let mut execution_input_durable = activated;
    execution_input_durable.carrier.execution_input_durable = 1;
    execution_input_durable.history.ever_execution_input_durable = 1;
    let persist_execution_input_transition = ProductionInFlightFirstReleaseTransitionProjection {
        action: crate::sumeragi::v2_core::IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT,
        actor: 1,
        target: 0,
        before: activated,
        after: execution_input_durable,
    };
    let prepared_execution_input = sign_cursor(
        2,
        Some(live_activated.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::prepared(1, persist_execution_input_transition)
            .expect("construct prepared PersistExecutionInput phase"),
    );
    assert!(
        foreign_kura
            .compare_and_swap_autonomous_lifecycle_cursor(
                cross_root_lease,
                prepared_execution_input.clone(),
            )
            .is_err(),
        "a CAS lease minted under another Kura root must not authorize a cursor write",
    );
    drop(foreign_kura);
    let prepared_read = kura
        .compare_and_swap_autonomous_lifecycle_cursor(first_lease, prepared_execution_input.clone())
        .expect("publish prepared PersistExecutionInput phase");
    let stale_error = match kura
        .compare_and_swap_autonomous_lifecycle_cursor(stale_lease, prepared_execution_input.clone())
    {
        Ok(_) => panic!("a stale cursor lease must not overwrite its successor"),
        Err(error) => error,
    };
    assert!(matches!(
        stale_error,
        Error::IO(ref error, _) if error.kind() == ErrorKind::AlreadyExists
    ));
    let (_, prepared_lease) = prepared_read.into_parts();
    let live_execution_input = sign_cursor(
        3,
        Some(prepared_execution_input.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::live(1, execution_input_durable)
            .expect("construct execution-input-durable live phase"),
    );
    let execution_input_read = kura
        .compare_and_swap_autonomous_lifecycle_cursor(prepared_lease, live_execution_input.clone())
        .expect("publish execution-input-durable live phase");
    assert_eq!(
        binding.route_identity(),
        (
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation
        ),
    );
    assert_eq!(
        binding.attempt_coordinates(),
        (
            proposal_height,
            1,
            payload.origin_proposal.descriptor.lane_block_view
        ),
    );
    assert_eq!(binding.height_context_id(), height_context_id);
    assert_eq!(binding.executable_payload_hash(), payload.payload_hash);
    assert_eq!(
        binding.reservation_group_hash(),
        reservation_group.reservation_group_hash,
    );
    assert_eq!(binding.local_validator_identity(), (0, 1));
    assert_eq!(binding.producer_actor_projection(), 1);
    assert_eq!(binding.reservation_group_binding(), reservation_group);
    let inventory = kura
        .active_autonomous_lifecycle_attempt_inventory(
            &generation_one,
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect("read bounded active lifecycle inventory");
    assert_eq!(inventory.len(), 1);
    assert_eq!(inventory[0].executable_payload(), &payload);
    let inventory_cursor = inventory[0]
        .cursor()
        .expect("inventory includes local cursor");
    assert_eq!(
        inventory_cursor.phase_kind(),
        AutonomousLifecycleCursorPhaseKindV2::Live
    );
    assert_eq!(inventory_cursor.owner_generation(), 1);
    assert_eq!(inventory_cursor.source_generation(), None);
    assert_eq!(
        inventory_cursor
            .before_projection()
            .expect("checked live projection"),
        execution_input_durable,
    );
    assert_eq!(
        inventory_cursor
            .after_projection()
            .expect("checked optional projection"),
        None
    );
    assert_eq!(
        inventory_cursor
            .prepared_transition_projection()
            .expect("checked optional transition"),
        None,
    );
    drop(execution_input_read);
    drop(kura);
    let (kura, _) = Kura::new(&config, &lane_config).expect("restart before Crash observation");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("rebind exact lifecycle key identity");
    assert!(
        kura.read_only_active_autonomous_lifecycle_attempt_inventory(
            test_network_id(b"wrong read-only lifecycle inventory genesis"),
            &local_peer,
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .is_err(),
        "read-only lifecycle inventory must authenticate the exact durable chain identity",
    );
    let wrong_local_peer = PeerId::new(
        checked_keypair_with_algorithm(Algorithm::BlsNormal)
            .public_key()
            .clone(),
    );
    assert!(
        kura.read_only_active_autonomous_lifecycle_attempt_inventory(
            network_id,
            &wrong_local_peer,
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .is_err(),
        "read-only lifecycle inventory must authenticate the exact durable local peer",
    );
    let observer_inventory = kura
        .read_only_active_autonomous_lifecycle_attempt_inventory(
            network_id,
            &local_peer,
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect("observer reads the prior durable lifecycle inventory without claiming generation");
    assert_eq!(observer_inventory.len(), 1);
    assert_eq!(observer_inventory[0].executable_payload(), &payload);
    assert_eq!(
        observer_inventory[0]
            .cursor()
            .expect("observer inventory includes the prior signed cursor")
            .owner_generation(),
        1,
    );
    let generation_two = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim second durable lifecycle process generation");
    assert_eq!(
        generation_two.generation(),
        2,
        "read-only observer inventory must not claim or increment process generation",
    );
    let mut crashed = execution_input_durable;
    crashed.session.crashed = 1;
    crashed.session.bodies = 0;
    crashed.session.ready_authorized = 0;
    crashed.session.producer_alive = false;
    let crash_transition = ProductionInFlightFirstReleaseTransitionProjection {
        action: crate::sumeragi::v2_core::IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH,
        actor: 1,
        target: 0,
        before: execution_input_durable,
        after: crashed,
    };
    assert!(
        check_production_in_flight_first_release_transition(crash_transition).is_some(),
        "crash fixture must be accepted by the production transition kernel",
    );
    let (_, execution_input_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_two)
        .expect("new generation reads the prior live cursor")
        .into_parts();
    let prepared_crash = sign_cursor(
        4,
        Some(live_execution_input.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::prepared(2, crash_transition)
            .expect("the production gate accepts the exact Crash transition"),
    );
    assert!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(execution_input_lease, prepared_crash)
            .is_err(),
        "Live -> Prepared must not bypass the generation-aware Crashed phase",
    );
    let (_, crash_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_two)
        .expect("read prior live cursor after rejected direct Crash preparation")
        .into_parts();
    let crashed_cursor = sign_cursor(
        4,
        Some(live_execution_input.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::crashed(1, 2, execution_input_durable, crashed)
            .expect("construct generation-aware crash phase"),
    );
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(crash_lease, crashed_cursor.clone(),)
            .expect("publish generation-aware crash phase")
            .cursor(),
        Some(&crashed_cursor),
        "successful Crash publication must return its exact durable cursor",
    );
    let mut recovered = crashed;
    recovered.session.crashed = 0;
    let recover_transition = ProductionInFlightFirstReleaseTransitionProjection {
        action: crate::sumeragi::v2_core::IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER,
        actor: 1,
        target: 0,
        before: crashed,
        after: recovered,
    };
    let prepared_recover = sign_cursor(
        5,
        Some(crashed_cursor.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::prepared(2, recover_transition)
            .expect("construct exact Recover transition"),
    );
    let (_, recover_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_two)
        .expect("read crash with its exact observing generation")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(recover_lease, prepared_recover.clone(),)
            .expect("publish exact prepared Recover phase")
            .cursor(),
        Some(&prepared_recover),
        "successful Recover preparation must return its exact durable cursor",
    );
    drop(kura);
    let (kura, _) = Kura::new(&config, &lane_config).expect("restart over prepared Recover");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("rebind exact lifecycle key identity after prepared Recover");
    let generation_three = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim third durable lifecycle process generation");
    assert_eq!(generation_three.generation(), 3);
    let direct_live_three = sign_cursor(
        6,
        Some(prepared_recover.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::live(3, recovered)
            .expect("construct deliberately direct recovered live phase"),
    );
    let (_, direct_live_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_three)
        .expect("read old prepared phase from a newer process")
        .into_parts();
    assert!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(direct_live_lease, direct_live_three)
            .is_err(),
        "a newer process must not complete an older process's Prepared phase directly",
    );
    let takeover_three = sign_cursor(
        6,
        Some(prepared_recover.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::observed_crashed(2, 3, crashed)
            .expect("transfer the already-crashed prepared state to generation three"),
    );
    let (_, takeover_three_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_three)
        .expect("reread prepared phase for generation-aware takeover")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(
            takeover_three_lease,
            takeover_three.clone(),
        )
        .expect("publish generation-three crashed takeover")
        .cursor(),
        Some(&takeover_three),
        "successful generation-three takeover must return its exact durable cursor",
    );
    drop(kura);
    let (kura, _) = Kura::new(&config, &lane_config).expect("restart over crashed takeover");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("rebind exact lifecycle key identity after crashed takeover");
    let generation_four = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim fourth durable lifecycle process generation");
    assert_eq!(generation_four.generation(), 4);
    let takeover_four = sign_cursor(
        7,
        Some(takeover_three.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::observed_crashed(3, 4, crashed)
            .expect("transfer an already-crashed observation again"),
    );
    let (_, takeover_four_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_four)
        .expect("read generation-three crash observation")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(
            takeover_four_lease,
            takeover_four.clone(),
        )
        .expect("publish repeated crash takeover without a fabricated second Crash")
        .cursor(),
        Some(&takeover_four),
        "successful repeated takeover must return its exact durable cursor",
    );
    let prepared_recover_four = sign_cursor(
        8,
        Some(takeover_four.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::prepared(4, recover_transition)
            .expect("construct generation-four Recover transition"),
    );
    let (_, recover_four_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_four)
        .expect("read repeated crash takeover")
        .into_parts();
    let prepared_recover_four_read = kura
        .compare_and_swap_autonomous_lifecycle_cursor(
            recover_four_lease,
            prepared_recover_four.clone(),
        )
        .expect("publish generation-four prepared Recover");
    let (_, prepared_recover_four_lease) = prepared_recover_four_read.into_parts();
    let live_recovered = sign_cursor(
        9,
        Some(prepared_recover_four.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::live(4, recovered)
            .expect("construct generation-four recovered live phase"),
    );
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(
            prepared_recover_four_lease,
            live_recovered.clone(),
        )
        .expect("publish recovered live phase after repeated takeover")
        .cursor(),
        Some(&live_recovered),
        "successful Live recovery must return its exact durable cursor",
    );
    let crashed_live_phase = AutonomousLifecycleCursorPhaseV2::live(4, crashed)
        .expect("the state kernel alone admits a crashed-member projection");
    assert!(
        AutonomousLifecycleCursorUnsignedV2::new(
            10,
            Some(live_recovered.cursor_hash()),
            binding.clone(),
            crashed_live_phase.clone(),
            local_peer.clone(),
        )
        .is_err(),
        "a Live cursor must not name its local signer as crashed",
    );
    let (_, mut recovered_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_four)
        .expect("read recovered cursor for bypass check")
        .into_parts();
    let mut synthetic_crashed_live = live_recovered.clone();
    synthetic_crashed_live.body.phase = crashed_live_phase;
    let prepared_recover_bypass = sign_cursor(
        10,
        Some(live_recovered.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::prepared(4, recover_transition)
            .expect("rebuild exact Recover phase for direct successor check"),
    );
    assert!(
        Kura::validate_autonomous_lifecycle_cursor_successor(
            &recovered_lease,
            Some(&synthetic_crashed_live),
            &prepared_recover_bypass,
        )
        .is_err(),
        "Live -> Prepared must not bypass the generation-aware Recover path",
    );
    recovered_lease.sequence = u64::MAX;
    assert!(
        Kura::validate_autonomous_lifecycle_cursor_successor(
            &recovered_lease,
            Some(&synthetic_crashed_live),
            &prepared_recover_bypass,
        )
        .is_err(),
        "cursor sequence exhaustion must fail closed instead of saturating",
    );
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
    let lifecycle_path =
        Kura::autonomous_lifecycle_cursor_path_for_entry(lane, temp_dir.path(), 1, proposal_height);
    let process_generation_path =
        Kura::autonomous_lifecycle_process_generation_path_for(temp_dir.path());
    let process_generation_temp_path =
        Kura::autonomous_lifecycle_process_generation_temp_path_for(temp_dir.path());
    assert!(attempt_path.is_file());
    assert!(view_path.is_file());
    assert!(height_pointer.is_file());
    assert!(route_pointer.is_file());
    assert!(lifecycle_path.is_file());
    assert!(process_generation_path.is_file());
    assert!(!process_generation_temp_path.exists());
    let process_generation_bytes =
        fs::read(&process_generation_path).expect("read durable process generation");
    let process_generation_record = Kura::decode_autonomous_lifecycle_process_generation_record(
        &process_generation_path,
        &process_generation_bytes,
    )
    .expect("decode durable process generation");
    assert_eq!(process_generation_record.body.generation, 4);
    assert_eq!(process_generation_record.body.network_id, network_id);
    assert_eq!(process_generation_record.body.local_peer_id, local_peer);
    let lifecycle_name = lifecycle_path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("lifecycle path has UTF-8 filename");
    assert_eq!(
        Kura::autonomous_lifecycle_cursor_coordinates(lifecycle_name),
        Some((1, proposal_height)),
    );
    assert!(
        Kura::autonomous_lifecycle_cursor_coordinates("autonomous_lifecycle_v2_1_42.norito")
            .is_none(),
        "unpadded attempt coordinates are noncanonical",
    );
    assert!(
        Kura::autonomous_lifecycle_cursor_coordinates(
            "autonomous_lifecycle_v1_00000000000000000001_00000000000000000042.norito"
        )
        .is_none(),
        "the first-release V2 hard cut must never recognize a V1 cursor path",
    );
    assert!(
        Kura::autonomous_lifecycle_cursor_coordinates(&format!("{lifecycle_name}.tmp")).is_none(),
        "a named cursor temporary is never a stable path",
    );
    assert!(
        Kura::autonomous_lifecycle_cursor_coordinates(
            "autonomous_lifecycle_v2_00000000000000000000_00000000000000000042.norito"
        )
        .is_none(),
        "zero lane heights are noncanonical",
    );
    let canonical_cursor_bytes = fs::read(&lifecycle_path).expect("read canonical cursor");
    assert_eq!(
        Kura::decode_autonomous_lifecycle_cursor(&lifecycle_path, &canonical_cursor_bytes)
            .expect("decode canonical cursor"),
        live_recovered,
    );
    let mut bad_signature = live_recovered.clone();
    bad_signature.signature[0] ^= 0x80;
    fs::write(
        &lifecycle_path,
        bad_signature
            .encode_framed()
            .expect("encode signature-tampered cursor"),
    )
    .expect("write signature-tampered cursor");
    assert!(
        kura.read_autonomous_lifecycle_cursor(&payload, &binding, &generation_four)
            .is_err(),
        "runtime reads must reject a tampered exact-size BLS signature",
    );
    fs::write(&lifecycle_path, &canonical_cursor_bytes).expect("restore canonical cursor");
    fs::File::options()
        .write(true)
        .open(&lifecycle_path)
        .expect("open cursor for oversized fixture")
        .set_len(AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES as u64 + 1)
        .expect("extend oversized cursor fixture");
    assert!(
        kura.read_autonomous_lifecycle_cursor(&payload, &binding, &generation_four)
            .is_err(),
        "runtime reads must reject an oversized lifecycle cursor before decoding",
    );
    fs::write(&lifecycle_path, &canonical_cursor_bytes).expect("restore bounded cursor");
    let hardlink_alias = temp_dir.path().join("lifecycle-cursor-hardlink-alias");
    fs::hard_link(&lifecycle_path, &hardlink_alias).expect("create cursor hardlink alias");
    assert!(
        kura.read_autonomous_lifecycle_cursor(&payload, &binding, &generation_four)
            .is_err(),
        "runtime reads must reject multiply linked cursor artifacts",
    );
    fs::remove_file(&hardlink_alias).expect("remove cursor hardlink alias");
    assert_eq!(
        kura.read_autonomous_lifecycle_cursor(&payload, &binding, &generation_four)
            .expect("single-link cursor is readable again")
            .cursor(),
        Some(&live_recovered),
        "restoring the single-link artifact must preserve the exact durable cursor",
    );
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
                &network_id,
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
    let named_cursor_temp = lifecycle_path.with_extension("norito.tmp");
    fs::write(&named_cursor_temp, &canonical_cursor_bytes)
        .expect("stage forbidden named cursor temporary");
    drop(kura);
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject a named lifecycle temporary rather than select it",
    );
    fs::remove_file(&named_cursor_temp).expect("remove rejected named cursor temporary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        fs::remove_file(&lifecycle_path).expect("remove cursor before symlink fixture");
        symlink(&attempt_path, &lifecycle_path).expect("install lifecycle cursor symlink");
        assert!(
            Kura::new(&config, &lane_config).is_err(),
            "startup must reject a symlinked lifecycle cursor",
        );
        fs::remove_file(&lifecycle_path).expect("remove lifecycle cursor symlink");
        fs::write(&lifecycle_path, &canonical_cursor_bytes)
            .expect("restore lifecycle cursor after symlink rejection");
    }
    fs::File::options()
        .write(true)
        .open(&lifecycle_path)
        .expect("open cursor for startup oversized fixture")
        .set_len(AUTONOMOUS_LIFECYCLE_CURSOR_MAX_BYTES as u64 + 1)
        .expect("extend startup oversized cursor fixture");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject an oversized lifecycle cursor before decoding",
    );
    fs::write(&lifecycle_path, &canonical_cursor_bytes)
        .expect("restore lifecycle cursor after startup size rejection");
    fs::remove_file(&process_generation_path)
        .expect("remove process generation for missing-record fixture");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject a retained cursor whose Kura root lost its process-generation record",
    );
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after missing-record rejection");
    let mut rolled_back_generation = process_generation_record.clone();
    rolled_back_generation.body.generation = 3;
    rolled_back_generation.record_hash = rolled_back_generation
        .body
        .canonical_hash()
        .expect("hash rolled-back process generation");
    let rolled_back_generation_bytes = rolled_back_generation
        .encode_framed()
        .expect("encode rolled-back process generation");
    fs::write(&process_generation_path, &rolled_back_generation_bytes)
        .expect("write rolled-back process generation");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject process-generation rollback below an active cursor",
    );
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after active rollback rejection");
    let archived_cursor_dir = temp_dir
        .path()
        .join("retired")
        .join("lane_geometry")
        .join("generation-audit-fixture");
    let archived_cursor_path = archived_cursor_dir.join(lifecycle_name);
    fs::create_dir_all(&archived_cursor_dir).expect("create retained-cursor audit fixture");
    fs::write(&archived_cursor_path, &canonical_cursor_bytes)
        .expect("write retained archived cursor");
    fs::remove_file(&lifecycle_path).expect("isolate retained archived cursor");
    fs::write(&process_generation_path, &rolled_back_generation_bytes)
        .expect("write rollback visible only to retained-cursor audit");
    let archived_rollback_error = match Kura::new(&config, &lane_config) {
        Ok(_) => panic!("startup accepted an archived cursor generation rollback"),
        Err(error) => error,
    };
    assert!(
        matches!(
            archived_rollback_error,
            Error::IO(ref source, ref path)
                if source.kind() == ErrorKind::InvalidData
                    && path == &archived_cursor_path
                    && source.to_string().contains(
                        "process generation was rolled back below its durable cursor"
                    )
        ),
        "startup must reject this exact archived cursor at the retained-generation audit, not for unrelated archive geometry",
    );
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after retained-cursor rollback rejection");
    fs::remove_dir_all(&archived_cursor_dir).expect("remove retained-cursor audit fixture");
    fs::write(&lifecycle_path, &canonical_cursor_bytes)
        .expect("restore active cursor after retained-cursor audit");
    fs::write(&process_generation_temp_path, &process_generation_bytes)
        .expect("write forbidden deterministic process-generation temporary");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject a deterministic process-generation temporary",
    );
    fs::remove_file(&process_generation_temp_path)
        .expect("remove deterministic process-generation temporary");
    let process_generation_atomic_temp = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}crash-residue"
    ));
    let generation_five_record =
        AutonomousLifecycleProcessGenerationRecordV1::new(network_id, local_peer.clone(), 5)
            .expect("construct unpublished generation-five successor");
    let generation_five_bytes = generation_five_record
        .encode_framed()
        .expect("encode unpublished generation-five successor");
    fs::write(&process_generation_atomic_temp, generation_five_bytes)
        .expect("stage pre-rename process-generation successor");
    let (recovered_generation_kura, _) = Kura::new(&config, &lane_config)
        .expect("startup cleans an exact unpublished process-generation successor");
    assert!(!process_generation_atomic_temp.exists());
    assert_eq!(
        fs::read(&process_generation_path).expect("read authoritative process generation"),
        process_generation_bytes,
        "startup recovery must retain stable generation four instead of promoting generation five",
    );
    drop(recovered_generation_kura);
    fs::write(&process_generation_atomic_temp, &process_generation_bytes)
        .expect("write non-successor process-generation atomic temporary");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject a process-generation temporary that is not the exact successor",
    );
    assert!(
        process_generation_atomic_temp.exists(),
        "failed process-generation classification must preserve the residue",
    );
    fs::remove_file(&process_generation_atomic_temp)
        .expect("remove rejected process-generation atomic temporary");
    fs::write(
        &process_generation_path,
        &process_generation_bytes[..process_generation_bytes.len() - 1],
    )
    .expect("write truncated process generation");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject a truncated process-generation record",
    );
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after truncation rejection");
    let mut invalid_hash_generation = process_generation_record.clone();
    invalid_hash_generation.record_hash = Hash::new(b"invalid process-generation record hash");
    fs::write(
        &process_generation_path,
        invalid_hash_generation
            .encode_framed()
            .expect("encode invalid-hash process generation"),
    )
    .expect("write invalid-hash process generation");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject a process-generation record whose self-hash is invalid",
    );
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after self-hash rejection");
    let mut zero_generation = process_generation_record.clone();
    zero_generation.body.generation = 0;
    zero_generation.record_hash = zero_generation
        .body
        .canonical_hash()
        .expect("hash zero process generation fixture");
    fs::write(
        &process_generation_path,
        zero_generation
            .encode_framed()
            .expect("encode zero process generation fixture"),
    )
    .expect("write zero process generation fixture");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject generation zero even when its record hash is internally consistent",
    );
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after zero rejection");
    fs::File::options()
        .write(true)
        .open(&process_generation_path)
        .expect("open process generation for oversized fixture")
        .set_len(AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES as u64 + 1)
        .expect("extend oversized process-generation fixture");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject an oversized process-generation record before decoding",
    );
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after size rejection");
    let process_generation_hardlink = temp_dir.path().join("process-generation-hardlink-alias");
    fs::hard_link(&process_generation_path, &process_generation_hardlink)
        .expect("create process-generation hardlink alias");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject a multiply linked process-generation record",
    );
    fs::remove_file(&process_generation_hardlink)
        .expect("remove process-generation hardlink alias");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let symlink_target = temp_dir.path().join("process-generation-symlink-target");
        fs::write(&symlink_target, &process_generation_bytes)
            .expect("write process-generation symlink target");
        fs::remove_file(&process_generation_path)
            .expect("remove stable process generation before symlink fixture");
        symlink(&symlink_target, &process_generation_path)
            .expect("install process-generation symlink");
        assert!(
            Kura::new(&config, &lane_config).is_err(),
            "startup must reject a symlinked process-generation record",
        );
        fs::remove_file(&process_generation_path).expect("remove process-generation symlink");
        fs::remove_file(&symlink_target).expect("remove process-generation symlink target");
        fs::write(&process_generation_path, &process_generation_bytes)
            .expect("restore process generation after symlink rejection");
    }
    let mut drifted_chain_generation = process_generation_record.clone();
    drifted_chain_generation.body.network_id =
        test_network_id(b"drifted-process-generation-genesis");
    drifted_chain_generation.record_hash = drifted_chain_generation
        .body
        .canonical_hash()
        .expect("hash drifted process-generation chain");
    fs::write(
        &process_generation_path,
        drifted_chain_generation
            .encode_framed()
            .expect("encode drifted process-generation chain"),
    )
    .expect("write drifted process-generation chain");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject process-generation chain identity drift against retained cursors",
    );
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after chain-drift rejection");
    let alternate_local_key = checked_keypair_with_algorithm(Algorithm::Ed25519);
    let mut drifted_local_peer_generation = process_generation_record.clone();
    drifted_local_peer_generation.body.local_peer_id =
        PeerId::new(alternate_local_key.public_key().clone());
    drifted_local_peer_generation.record_hash = drifted_local_peer_generation
        .body
        .canonical_hash()
        .expect("hash drifted process-generation local peer");
    fs::write(
        &process_generation_path,
        drifted_local_peer_generation
            .encode_framed()
            .expect("encode drifted process-generation local peer"),
    )
    .expect("write drifted process-generation local peer");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject process-generation local-peer identity drift against retained cursors",
    );
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after local-peer drift rejection");
    let obsolete_cursor_path = artifact_dir
        .join("autonomous_lifecycle_v1_00000000000000000001_00000000000000000042.norito");
    fs::write(&obsolete_cursor_path, &canonical_cursor_bytes)
        .expect("write obsolete V1 cursor path fixture");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must fail closed on a legacy V1 cursor path instead of decoding it",
    );
    fs::remove_file(&obsolete_cursor_path).expect("remove obsolete V1 cursor path fixture");
    let mut exhausted_generation = process_generation_record.clone();
    exhausted_generation.body.generation = u64::MAX;
    exhausted_generation.record_hash = exhausted_generation
        .body
        .canonical_hash()
        .expect("hash exhausted process generation");
    fs::write(
        &process_generation_path,
        exhausted_generation
            .encode_framed()
            .expect("encode exhausted process generation"),
    )
    .expect("write exhausted process generation");
    let (exhausted_kura, _) = Kura::new(&config, &lane_config)
        .expect("generation maximum remains readable for fail-closed exhaustion");
    exhausted_kura
        .bind_local_peer_id(local_peer.clone())
        .expect("bind local key before exhausted claim");
    assert!(
        exhausted_kura
            .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
            .is_err(),
        "the process-generation claim must use checked addition at u64::MAX",
    );
    drop(exhausted_kura);
    fs::write(&process_generation_path, &process_generation_bytes)
        .expect("restore process generation after exhaustion rejection");
    let (reopened, _) =
        Kura::new(&config, &lane_config).expect("startup reconstructs the exact immutable attempt");
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
            .current_autonomous_lane_payload(lane.lane_id, 1, network_id, epoch)
            .expect("reconstructed first attempt")
            .0,
        payload,
    );
    let pending_temp_dir = TempDir::new().expect("pending cursor temp dir");
    let pending_config = kura_config_for_dir(&pending_temp_dir, BLOCKS_IN_MEMORY);
    let (pending_kura, _) = Kura::new(&pending_config, &lane_config).expect("pending cursor Kura");
    install_autonomous_lane_marker_for_kura(&pending_kura, &lane_config, &payload);
    let pending_cursor_path = Kura::autonomous_lifecycle_cursor_path_for_entry(
        lane,
        pending_temp_dir.path(),
        1,
        proposal_height,
    );
    fs::create_dir_all(
        pending_cursor_path
            .parent()
            .expect("pending cursor has parent"),
    )
    .expect("create pending cursor namespace");
    fs::write(
        &pending_cursor_path,
        live_activated
            .encode_framed()
            .expect("encode pending first lifecycle cursor"),
    )
    .expect("write pending lifecycle cursor without payload");
    drop(pending_kura);
    assert!(
        Kura::new(&pending_config, &lane_config).is_err(),
        "startup must reject even a signed first cursor until the State/Queue adapter authenticates its exact payload",
    );
}
