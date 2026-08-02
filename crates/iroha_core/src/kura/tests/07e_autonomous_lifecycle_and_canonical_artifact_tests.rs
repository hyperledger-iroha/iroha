// Autonomous lifecycle recovery and canonical lane-artifact regressions.

#[test]
fn autonomous_lifecycle_bootstrap_recovers_every_signed_crash_boundary() {
    let temp_dir = TempDir::new().expect("bootstrap temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (chain_id_hash, epoch, payload_template) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"kura-autonomous-bootstrap-height-context"),
    ));
    let local_peer = PeerId::new(signer.public_key().clone());
    let (reservation_owner_hash, proposal_identity_hash) =
        autonomous_lane_reservation_identity_hashes_for_proposal(
            chain_id_hash,
            height_context_id,
            epoch,
            &payload_template.origin_proposal,
            &local_peer,
        )
        .expect("derive bootstrap reservation identities");
    let mut reservation_keys = payload_template.reservation_keys.clone();
    for reservation in &mut reservation_keys {
        reservation.reservation_owner_hash = reservation_owner_hash;
        reservation.proposal_identity_hash = proposal_identity_hash;
    }
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        chain_id_hash,
        epoch,
        payload_template.origin_proposal.clone(),
        payload_template.entrypoints.clone(),
        reservation_keys,
        payload_template.routing_plans.clone(),
        payload_template.native_amx_receipts.clone(),
        local_peer.clone(),
        signer.private_key(),
    )
    .expect("construct bootstrap-bound payload");
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
            .expect("bind bootstrap reservation group");
    let binding = AutonomousLifecycleAttemptBindingV1::from_payload(
        height_context_id,
        1,
        &payload,
        reservation_group,
        &local_peer,
    )
    .expect("bind bootstrap lifecycle attempt");
    let binding_a = canonical_lane_queue_reservation_group_identity_projection(reservation_group);
    let before_activate = ProductionInFlightFirstReleaseStateProjection {
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
    let mut activated = before_activate;
    activated.carrier.kura_active = 1;
    let activate = ProductionInFlightFirstReleaseTransitionProjection {
        action: IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA,
        actor: 1,
        target: 0,
        before: before_activate,
        after: activated,
    };
    assert!(check_production_in_flight_first_release_transition(activate).is_some());
    let sign_cursor = |sequence, previous_cursor_hash, phase: AutonomousLifecycleCursorPhaseV2| {
        let unsigned = AutonomousLifecycleCursorUnsignedV2::new(
            sequence,
            previous_cursor_hash,
            binding.clone(),
            phase,
            local_peer.clone(),
        )
        .expect("construct bootstrap lifecycle cursor");
        let preimage = unsigned
            .signing_preimage()
            .expect("encode bootstrap cursor signing preimage");
        let signature = Signature::try_new(signer.private_key(), &preimage)
            .expect("sign bootstrap lifecycle cursor");
        unsigned
            .finalize(
                <[u8; 96]>::try_from(signature.payload())
                    .expect("BLS-normal bootstrap signature is exactly 96 bytes"),
                &payload.origin_proposal.descriptor.validator_set,
            )
            .expect("finalize bootstrap lifecycle cursor")
    };
    let prepared_activate = sign_cursor(
        1,
        None,
        AutonomousLifecycleCursorPhaseV2::prepared(1, activate)
            .expect("construct signed Prepared ActivateKura"),
    );
    let live_activate = sign_cursor(
        2,
        Some(prepared_activate.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::live(1, activated)
            .expect("construct signed Live ActivateKura successor"),
    );
    let authentication_facts = (height_context_id, 1, 1, reservation_group);

    let (kura, _) = Kura::new(&config, &lane_config).expect("bootstrap Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind bootstrap local peer");
    let generation_one = kura
        .claim_autonomous_lifecycle_process_generation(chain_id_hash, &local_peer)
        .expect("claim bootstrap signing generation");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    let bootstrap_preimage = kura
        .autonomous_lifecycle_bootstrap_signing_preimage_for_tests(
            &generation_one,
            &payload,
            binding.clone(),
            prepared_activate.clone(),
            live_activate.clone(),
            authentication_facts,
        )
        .expect("build full bootstrap signature preimage");
    let bootstrap_signature = <[u8; 96]>::try_from(
        Signature::try_new(signer.private_key(), &bootstrap_preimage)
            .expect("sign full lifecycle bootstrap")
            .payload(),
    )
    .expect("BLS-normal bootstrap signature is exactly 96 bytes");
    let authority = kura
        .persist_autonomous_lifecycle_bootstrap_for_tests(
            &generation_one,
            &payload,
            binding.clone(),
            prepared_activate.clone(),
            live_activate.clone(),
            bootstrap_signature,
            authentication_facts,
        )
        .expect("persist signed lifecycle bootstrap before payload mutation");
    assert_eq!(
        authority.stage(),
        AutonomousLifecycleBootstrapRecoveryStage::BootstrapOnly,
    );
    assert_eq!(
        authority.custody_source(),
        AutonomousLifecyclePayloadCustodySourceV1::ProducerQueue,
    );
    assert_eq!(authority.executable_payload(), &payload);
    assert_eq!(authority.binding(), &binding);
    assert!(
        kura.current_autonomous_lane_payload(lane.lane_id, 1, chain_id_hash, epoch)
            .is_none(),
        "bootstrap durability must precede every payload mutation",
    );
    assert!(
        kura.delete_completed_autonomous_lifecycle_bootstrap(&authority)
            .is_err(),
        "bootstrap deletion must fail before exact Live durability",
    );
    let bootstrap_path = Kura::autonomous_lifecycle_bootstrap_path_for_entry(
        lane,
        temp_dir.path(),
        1,
        payload.origin_proposal.descriptor.proposal_height,
    );
    let bootstrap_bytes = fs::read(&bootstrap_path).expect("read canonical bootstrap bytes");
    assert!(bootstrap_path.is_file());
    let retirement_error = kura
        .first_release_lane_retirement_admissible_for_test(
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect_err("a signed unfinished bootstrap must block lane retirement");
    assert!(
        retirement_error
            .to_string()
            .contains("unfinished lifecycle bootstrap"),
        "unexpected signed-bootstrap retirement error: {retirement_error}",
    );
    let mut retirement_corrupted =
        Kura::decode_autonomous_lifecycle_bootstrap(&bootstrap_path, &bootstrap_bytes)
            .expect("decode bootstrap before retirement corruption");
    retirement_corrupted.signature[0] ^= 0x20;
    fs::write(
        &bootstrap_path,
        retirement_corrupted
            .encode_framed()
            .expect("encode retirement-corrupted bootstrap"),
    )
    .expect("write retirement-corrupted bootstrap");
    let retirement_error = kura
        .first_release_lane_retirement_admissible_for_test(
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect_err("retirement must validate a bootstrap before treating it as a blocker");
    assert!(
        retirement_error
            .to_string()
            .contains("signature verification failed"),
        "unexpected malformed-bootstrap retirement error: {retirement_error}",
    );
    fs::write(&bootstrap_path, &bootstrap_bytes)
        .expect("restore bootstrap after retirement validation test");
    fs::File::options()
        .write(true)
        .open(&bootstrap_path)
        .expect("open bootstrap for retirement oversize fixture")
        .set_len(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES as u64 + 1)
        .expect("extend retirement oversize fixture");
    let retirement_error = kura
        .first_release_lane_retirement_admissible_for_test(
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect_err(
            "retirement must reject an oversized bootstrap before treating it as a blocker",
        );
    assert!(
        matches!(
            retirement_error,
            Error::IO(ref source, _) if source.kind() == ErrorKind::InvalidData
        ),
        "oversized bootstrap must be invalid retirement evidence, not a drain blocker",
    );
    fs::write(&bootstrap_path, &bootstrap_bytes)
        .expect("restore bootstrap after retirement oversize test");
    drop(kura);

    let hardlink_alias = temp_dir.path().join("lifecycle-bootstrap-hardlink-alias");
    fs::hard_link(&bootstrap_path, &hardlink_alias).expect("create bootstrap hardlink alias");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject a multiply linked lifecycle bootstrap",
    );
    fs::remove_file(&hardlink_alias).expect("remove bootstrap hardlink alias");

    let mut tampered_bootstrap = bootstrap_bytes.clone();
    let tampered_byte = tampered_bootstrap
        .last_mut()
        .expect("bootstrap encoding is non-empty");
    *tampered_byte ^= 0x80;
    fs::write(&bootstrap_path, &tampered_bootstrap).expect("write tampered bootstrap");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject bootstrap signature, canonical, or self-hash drift",
    );
    fs::write(&bootstrap_path, &bootstrap_bytes).expect("restore canonical bootstrap");

    let canonical_bootstrap =
        Kura::decode_autonomous_lifecycle_bootstrap(&bootstrap_path, &bootstrap_bytes)
            .expect("decode canonical signed bootstrap");
    let mut corrupted_signature = canonical_bootstrap.clone();
    corrupted_signature.signature[0] ^= 0x40;
    fs::write(
        &bootstrap_path,
        corrupted_signature
            .encode_framed()
            .expect("encode signature-corrupted bootstrap"),
    )
    .expect("write signature-corrupted bootstrap");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject an exact-size corrupted bootstrap signature",
    );
    fs::write(&bootstrap_path, &bootstrap_bytes).expect("restore bootstrap after signature test");

    let mut substituted_source = canonical_bootstrap.clone();
    substituted_source.body.custody.source =
        AutonomousLifecyclePayloadCustodySourceV1::ProtectedCarrierReceive;
    substituted_source.bootstrap_hash = substituted_source
        .body
        .canonical_hash()
        .expect("hash source-substituted bootstrap body");
    fs::write(
        &bootstrap_path,
        substituted_source
            .encode_framed()
            .expect("encode source-substituted bootstrap"),
    )
    .expect("write source-substituted bootstrap");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "a ProducerQueue signature must not authorize another custody source",
    );
    fs::write(&bootstrap_path, &bootstrap_bytes)
        .expect("restore bootstrap after source-substitution test");

    let mut corrupted_evidence = canonical_bootstrap.clone();
    corrupted_evidence.body.custody.evidence_hash =
        Hash::new(b"corrupted bootstrap custody evidence hash");
    corrupted_evidence.bootstrap_hash = corrupted_evidence
        .body
        .canonical_hash()
        .expect("hash evidence-corrupted bootstrap body");
    fs::write(
        &bootstrap_path,
        corrupted_evidence
            .encode_framed()
            .expect("encode evidence-corrupted bootstrap"),
    )
    .expect("write evidence-corrupted bootstrap");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "the full-body signature must reject custody evidence-hash substitution",
    );
    fs::write(&bootstrap_path, &bootstrap_bytes)
        .expect("restore bootstrap after evidence-hash test");

    let wrong_local_signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let mut wrong_signer_bootstrap = canonical_bootstrap.clone();
    wrong_signer_bootstrap.signature = <[u8; 96]>::try_from(
        Signature::try_new(wrong_local_signer.private_key(), &bootstrap_preimage)
            .expect("sign bootstrap with wrong local key")
            .payload(),
    )
    .expect("wrong BLS-normal bootstrap signature is exactly 96 bytes");
    fs::write(
        &bootstrap_path,
        wrong_signer_bootstrap
            .encode_framed()
            .expect("encode wrong-signer bootstrap"),
    )
    .expect("write wrong-signer bootstrap");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "bootstrap signatures from another local key must fail closed",
    );
    fs::write(&bootstrap_path, &bootstrap_bytes)
        .expect("restore bootstrap after wrong-signer test");

    let process_generation_path =
        Kura::autonomous_lifecycle_process_generation_path_for(temp_dir.path());
    let generation_one_bytes =
        fs::read(&process_generation_path).expect("read generation-one record");
    let generation_two_record =
        AutonomousLifecycleProcessGenerationRecordV1::new(chain_id_hash, local_peer.clone(), 2)
            .expect("construct generation-two replay record");
    let generation_two_bytes = generation_two_record
        .encode_framed()
        .expect("encode generation-two replay record");
    let prepared_activate_two = sign_cursor(
        1,
        None,
        AutonomousLifecycleCursorPhaseV2::prepared(2, activate)
            .expect("construct generation-two Prepared ActivateKura"),
    );
    let live_activate_two = sign_cursor(
        2,
        Some(prepared_activate_two.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::live(2, activated)
            .expect("construct generation-two Live ActivateKura"),
    );
    let mut generation_replay = canonical_bootstrap.clone();
    generation_replay.body.process_generation = 2;
    generation_replay.body.process_generation_record_hash = generation_two_record.record_hash;
    generation_replay.body.prepared_activate = prepared_activate_two;
    generation_replay.body.live_activate = live_activate_two;
    generation_replay.bootstrap_hash = generation_replay
        .body
        .canonical_hash()
        .expect("hash generation-replayed bootstrap body");
    fs::write(&process_generation_path, generation_two_bytes)
        .expect("install generation-two replay record");
    fs::write(
        &bootstrap_path,
        generation_replay
            .encode_framed()
            .expect("encode generation-replayed bootstrap"),
    )
    .expect("write generation-replayed bootstrap");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "a bootstrap signature from generation one must not authorize a generation-two body",
    );
    fs::write(&process_generation_path, generation_one_bytes)
        .expect("restore generation-one record after replay test");
    fs::write(&bootstrap_path, &bootstrap_bytes)
        .expect("restore bootstrap after generation replay test");

    fs::File::options()
        .write(true)
        .open(&bootstrap_path)
        .expect("open bootstrap for oversized fixture")
        .set_len(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES as u64 + 1)
        .expect("extend oversized bootstrap fixture");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject an oversized bootstrap before decoding",
    );
    fs::write(&bootstrap_path, &bootstrap_bytes).expect("restore bounded bootstrap");

    let bootstrap_parent = bootstrap_path.parent().expect("bootstrap path has parent");
    let bootstrap_atomic_temp = bootstrap_parent.join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}crash-residue"
    ));
    fs::write(&bootstrap_atomic_temp, &bootstrap_bytes).expect("write bootstrap atomic temporary");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject an unresolved bootstrap atomic temporary",
    );
    fs::remove_file(&bootstrap_atomic_temp).expect("remove bootstrap atomic temporary");

    let legacy_bootstrap = bootstrap_parent.join(format!(
        "autonomous_lifecycle_bootstrap_v0_{:020}_{:020}.norito",
        1, payload.origin_proposal.descriptor.proposal_height,
    ));
    fs::write(&legacy_bootstrap, &bootstrap_bytes).expect("write legacy bootstrap path");
    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "startup must reject legacy bootstrap paths without decoding them",
    );
    fs::remove_file(&legacy_bootstrap).expect("remove legacy bootstrap path");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let symlink_target = temp_dir.path().join("lifecycle-bootstrap-symlink-target");
        fs::write(&symlink_target, &bootstrap_bytes).expect("write bootstrap symlink target");
        fs::remove_file(&bootstrap_path).expect("remove stable bootstrap for symlink fixture");
        symlink(&symlink_target, &bootstrap_path).expect("install bootstrap symlink");
        assert!(
            Kura::new(&config, &lane_config).is_err(),
            "startup must reject a symlinked lifecycle bootstrap",
        );
        fs::remove_file(&bootstrap_path).expect("remove bootstrap symlink");
        fs::remove_file(&symlink_target).expect("remove bootstrap symlink target");
        fs::write(&bootstrap_path, &bootstrap_bytes).expect("restore bootstrap after symlink test");
    }

    let (kura, _) = Kura::new(&config, &lane_config)
        .expect("bootstrap-only startup retains intent without mutating payload");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("rebind bootstrap local peer");
    let generation_two = kura
        .claim_autonomous_lifecycle_process_generation(chain_id_hash, &local_peer)
        .expect("claim bootstrap-only recovery generation");
    let mut inventory = kura
        .autonomous_lifecycle_bootstrap_recovery_inventory(
            &generation_two,
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect("inventory bootstrap-only crash boundary");
    assert_eq!(inventory.len(), 1);
    assert_eq!(
        inventory[0].stage(),
        AutonomousLifecycleBootstrapRecoveryStage::BootstrapOnly,
    );
    let wrong_height_context = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"wrong-bootstrap-height-context"),
    ));
    assert!(
        kura.authenticate_autonomous_lifecycle_bootstrap_recovery_for_tests(
            inventory.pop().expect("bootstrap authority"),
            (wrong_height_context, 1, 1, reservation_group),
        )
        .is_err(),
        "recovery must never advance without fresh exact Queue and height authentication",
    );
    kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
        .expect("persist exact payload after signed bootstrap");
    drop(kura);

    let (kura, _) = Kura::new(&config, &lane_config)
        .expect("payload-durable bootstrap crash boundary is restartable");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("rebind payload-durable local peer");
    let generation_three = kura
        .claim_autonomous_lifecycle_process_generation(chain_id_hash, &local_peer)
        .expect("claim payload-durable recovery generation");
    let authority = kura
        .autonomous_lifecycle_bootstrap_recovery_inventory(
            &generation_three,
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect("inventory payload-durable crash boundary")
        .pop()
        .expect("payload-durable bootstrap authority");
    assert_eq!(
        authority.stage(),
        AutonomousLifecycleBootstrapRecoveryStage::PayloadDurable,
    );
    assert!(
        kura.delete_completed_autonomous_lifecycle_bootstrap(&authority)
            .is_err(),
        "payload durability alone must not authorize bootstrap deletion",
    );
    kura.publish_autonomous_lifecycle_bootstrap_cursor_stage(
        &authority,
        AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable,
    )
    .expect("publish exact signed Prepared ActivateKura cursor");
    drop(kura);

    let (kura, _) = Kura::new(&config, &lane_config)
        .expect("Prepared-durable bootstrap crash boundary is restartable");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("rebind Prepared-durable local peer");
    let generation_four = kura
        .claim_autonomous_lifecycle_process_generation(chain_id_hash, &local_peer)
        .expect("claim Prepared-durable recovery generation");
    let authority = kura
        .autonomous_lifecycle_bootstrap_recovery_inventory(
            &generation_four,
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect("inventory Prepared-durable crash boundary")
        .pop()
        .expect("Prepared-durable bootstrap authority");
    assert_eq!(
        authority.stage(),
        AutonomousLifecycleBootstrapRecoveryStage::PreparedDurable,
    );
    assert!(
        kura.delete_completed_autonomous_lifecycle_bootstrap(&authority)
            .is_err(),
        "Prepared durability alone must not authorize bootstrap deletion",
    );
    kura.publish_autonomous_lifecycle_bootstrap_cursor_stage(
        &authority,
        AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
    )
    .expect("publish exact signed Live successor");
    drop(kura);

    let (kura, _) = Kura::new(&config, &lane_config)
        .expect("Live-durable bootstrap crash boundary is restartable");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("rebind Live-durable local peer");
    let generation_five = kura
        .claim_autonomous_lifecycle_process_generation(chain_id_hash, &local_peer)
        .expect("claim Live-durable recovery generation");
    let authority = kura
        .autonomous_lifecycle_bootstrap_recovery_inventory(
            &generation_five,
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect("inventory Live-durable crash boundary")
        .pop()
        .expect("Live-durable bootstrap authority");
    assert_eq!(
        authority.stage(),
        AutonomousLifecycleBootstrapRecoveryStage::LiveDurable,
    );
    assert!(
        kura.authenticate_autonomous_lifecycle_bootstrap_recovery_from_durable_custody(authority)
            .is_err(),
        "ProducerQueue recovery must retain a fresh live Queue fence through deletion",
    );
    let authority = kura
        .autonomous_lifecycle_bootstrap_recovery_inventory(
            &generation_five,
            lane.lane_id,
            lane.dataspace_id,
            payload.origin_proposal.descriptor.lane_incarnation,
        )
        .expect("re-inventory Live-durable bootstrap after rejected custody substitution")
        .pop()
        .expect("Live-durable bootstrap authority remains durable");
    let permit = kura
        .authenticate_autonomous_lifecycle_bootstrap_recovery_for_tests(
            authority,
            authentication_facts,
        )
        .expect("authenticate Live-durable bootstrap under fresh Queue facts");
    let completion = kura
        .complete_autonomous_lifecycle_bootstrap(permit)
        .expect("complete exact bootstrap and synced deletion");
    assert!(completion.takeover_required());
    assert_eq!(completion.cursor(), &live_activate);
    assert!(!bootstrap_path.exists());
    assert!(
        kura.persist_autonomous_lifecycle_bootstrap_for_tests(
            &generation_five,
            &payload,
            binding.clone(),
            prepared_activate,
            live_activate.clone(),
            bootstrap_signature,
            authentication_facts,
        )
        .is_err(),
        "completed or historical bootstrap bytes must never be replayed around durable state",
    );

    let (current, takeover_lease) = completion.into_cursor_read().into_parts();
    let current = current.expect("completion returns exact historical Live cursor");
    let direct_current_live = sign_cursor(
        3,
        Some(current.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::live(5, activated)
            .expect("construct prohibited direct current-generation Live"),
    );
    assert!(
        Kura::validate_autonomous_lifecycle_cursor_successor(
            &takeover_lease,
            Some(&current),
            &direct_current_live,
        )
        .is_err(),
        "an old-generation Live bootstrap must force generation-aware crash takeover",
    );
    let mut crashed = activated;
    crashed.session.crashed = 1;
    crashed.session.bodies = 0;
    crashed.session.ready_authorized = 0;
    crashed.session.producer_alive = false;
    let crashed_cursor = sign_cursor(
        3,
        Some(current.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::crashed(1, 5, activated, crashed)
            .expect("construct required old-generation crash takeover"),
    );
    kura.compare_and_swap_autonomous_lifecycle_cursor(takeover_lease, crashed_cursor.clone())
        .expect("publish required old-generation crash takeover");

    let mut recovered = crashed;
    recovered.session.crashed = 0;
    let recover = ProductionInFlightFirstReleaseTransitionProjection {
        action: crate::sumeragi::v2_core::IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER,
        actor: 1,
        target: 0,
        before: crashed,
        after: recovered,
    };
    let prepared_recover = sign_cursor(
        4,
        Some(crashed_cursor.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::prepared(5, recover)
            .expect("construct exact current-generation Recover"),
    );
    let (_, recover_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_five)
        .expect("read crashed takeover")
        .into_parts();
    let prepared_read = kura
        .compare_and_swap_autonomous_lifecycle_cursor(recover_lease, prepared_recover.clone())
        .expect("publish current-generation Prepared Recover");
    let (_, prepared_lease) = prepared_read.into_parts();
    let live_recovered = sign_cursor(
        5,
        Some(prepared_recover.cursor_hash()),
        AutonomousLifecycleCursorPhaseV2::live(5, recovered)
            .expect("construct current-generation recovered Live"),
    );
    kura.compare_and_swap_autonomous_lifecycle_cursor(prepared_lease, live_recovered)
        .expect("publish current-generation Live only after Crash and Recover");
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
    let validator_count = u32::try_from(validator_set.len()).expect("validator count fits u32");
    let min_quorum = u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
        validator_set.len(),
    ))
    .expect("validator quorum fits u32");
    proposal.descriptor.validator_set_hash_version = VALIDATOR_SET_HASH_VERSION_V1;
    proposal.descriptor.validator_set = validator_set.clone();
    proposal.descriptor.validator_set_hash = HashOf::new(&validator_set);
    proposal.descriptor.validator_count = validator_count;
    proposal.descriptor.min_quorum = min_quorum;
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
    let producer = crate::lane_consensus::deterministic_lane_author(&validator_set, 1)
        .expect("fixture has a deterministic lane author");
    let producer_signer = [&first_signer, &second_signer]
        .into_iter()
        .find(|signer| signer.public_key() == producer.public_key())
        .expect("fixture retains the deterministic lane-author key");
    let first = build_payload(producer_signer);
    let mut second = first.clone();
    second.producer = validator_set
        .iter()
        .find(|validator| *validator != producer)
        .expect("fixture contains a non-author committee member")
        .clone();
    assert_eq!(first.payload_hash, second.payload_hash);
    assert_ne!(first, second);
    assert_eq!(
        second.validate(chain_id_hash, epoch),
        Err(crate::lane_consensus::LaneAutonomousArtifactError::ProducerNotDeterministicAuthor),
        "another committee member must fail before its bytes reach Kura",
    );

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
    let orphan_claim = AutonomousLaneEntrypointClaimV3::new(&later_height, orphan_entrypoint_hash);
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
    let (chain_id_hash, epoch, first) =
        autonomous_lane_payload_for_kura(lane_entry.lane_id, lane_entry.dataspace_id, 1, &signer);
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
    let application_state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"marker-bound direct application state",
    )));
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

    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
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
    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
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

    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
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
    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
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
    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
    std::fs::remove_file(&data_path).expect("remove lane artifact data sidecar");
    std::fs::remove_file(&index_path).expect("remove lane artifact index sidecar");

    assert!(
        kura.canonical_lane_block_artifacts_at_proposal_height_matching(proposal_height, 8, |_| {
            false
        },)
            .is_empty()
    );
    assert!(
        kura.read_lane_block_artifact(lane_id, 1).is_none(),
        "a rejected old-incarnation ownership must not be written into active storage"
    );
    assert_eq!(
        kura.canonical_lane_block_artifacts_at_proposal_height_matching(proposal_height, 8, |_| {
            true
        },)
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
